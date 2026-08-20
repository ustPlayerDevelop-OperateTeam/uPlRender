//! 音名转换、时间格式化、占位符及静默/结束文字规则。

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub fn midi_to_name(num: i32) -> String {
    let idx = num.rem_euclid(12) as usize;
    let octave = num.div_euclid(12) - 1;
    format!("{}{}", NOTE_NAMES[idx], octave)
}

/// 秒数（浮点，百分秒）→ 时间文本（HH:MM:SS:CC 或 MM:SS:CC）。
/// 规格示例 `00:05:03:45` 与 `1:02:03:45` 自洽的唯一解释:
/// 小时 > 0 时 `h:mm:ss:cc`(小时无前导零),否则 `00:mm:ss:cc`。
/// ⚠️ 待 player.py 校准(仓库不含 Python 源码)。
pub fn format_play_time(seconds: f64) -> String {
    let total = seconds.max(0.0) as i64;
    let ms = ((seconds - seconds.trunc()).abs() * 100.0).round() as i64;
    let h = total / 3600;
    let rem = total % 3600;
    let m = rem / 60;
    let s = rem % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}:{:02}", h, m, s, ms)
    } else {
        format!("00:{:02}:{:02}:{:02}", m, s, ms)
    }
}

pub fn silent_text<'a>(display: &'a str, custom: &'a str) -> &'a str {
    match display {
        "r" => "R",
        "dash" => "-",
        "custom" => custom,
        "none" => "",
        _ => "",
    }
}

pub fn end_text<'a>(display: &'a str, custom: &'a str) -> &'a str {
    match display {
        "end" => "END",
        "dash" => "-",
        "custom" => custom,
        "none" => "",
        _ => "",
    }
}

/// 音高占位符处理。带 `#` 的半音音名原样返回。
pub fn apply_pitch_placeholder(name: &str, placeholder: &str, custom: &str) -> String {
    if name.contains('#') {
        return name.to_string();
    }
    let pos = name.find(|c: char| c.is_ascii_digit());
    let (letter, number) = match pos {
        Some(p) if p > 0 => (&name[..p], &name[p..]),
        _ => return name.to_string(),
    };
    if !matches!(letter, "C" | "D" | "E" | "F" | "G" | "A" | "B") {
        return name.to_string();
    }
    match placeholder {
        "none" => format!("{}{}", letter, number),
        "dash" => format!("{}-{}", letter, number),
        "custom" => {
            let suffix = custom.trim();
            if suffix.is_empty() {
                format!("{}{}", letter, number)
            } else {
                format!("{}({}){}", letter, suffix, number)
            }
        }
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_to_name() {
        assert_eq!(midi_to_name(60), "C4");
        assert_eq!(midi_to_name(61), "C#4");
        assert_eq!(midi_to_name(69), "A4");
        assert_eq!(midi_to_name(0), "C-1");
        assert_eq!(midi_to_name(12), "C0");
        assert_eq!(midi_to_name(127), "G9");
    }

    #[test]
    fn test_format_play_time_zero() {
        assert_eq!(format_play_time(0.0), "00:00:00:00");
    }

    #[test]
    fn test_format_play_time_under_hour() {
        // 规格示例:5 分 3.45 秒 → 00:05:03:45
        assert_eq!(format_play_time(303.45), "00:05:03:45");
    }

    #[test]
    fn test_format_play_time_over_hour() {
        // 规格示例:1 小时 2 分 3.45 秒 → 1:02:03:45(小时无前导零)
        assert_eq!(format_play_time(3723.45), "1:02:03:45");
    }

    #[test]
    fn test_format_play_time_boundaries() {
        assert_eq!(format_play_time(59.99), "00:00:59:99");
        assert_eq!(format_play_time(60.0), "00:01:00:00");
        assert_eq!(format_play_time(3599.99), "00:59:59:99");
        assert_eq!(format_play_time(3600.0), "1:00:00:00");
        assert_eq!(format_play_time(86399.99), "23:59:59:99");
    }

    #[test]
    fn test_silent_text_rules() {
        assert_eq!(silent_text("r", "x"), "R");
        assert_eq!(silent_text("dash", "x"), "-");
        assert_eq!(silent_text("custom", "自定义"), "自定义");
        assert_eq!(silent_text("custom", ""), "");
        assert_eq!(silent_text("none", "x"), "");
        assert_eq!(silent_text("unknown", "x"), "");
    }

    #[test]
    fn test_end_text_rules() {
        assert_eq!(end_text("end", "x"), "END");
        assert_eq!(end_text("dash", "x"), "-");
        assert_eq!(end_text("custom", "完结"), "完结");
        assert_eq!(end_text("none", "x"), "");
        assert_eq!(end_text("unknown", "x"), "");
    }

    #[test]
    fn test_pitch_placeholder_sharp_unchanged() {
        // 带 # 的半音原样返回
        assert_eq!(apply_pitch_placeholder("C#4", "dash", "x"), "C#4");
        assert_eq!(apply_pitch_placeholder("F#2", "custom", "x"), "F#2");
    }

    #[test]
    fn test_pitch_placeholder_none() {
        assert_eq!(apply_pitch_placeholder("C4", "none", "x"), "C4");
    }

    #[test]
    fn test_pitch_placeholder_dash() {
        assert_eq!(apply_pitch_placeholder("C4", "dash", "x"), "C-4");
        assert_eq!(apply_pitch_placeholder("A#0", "dash", "x"), "A#0"); // 半音不处理
    }

    #[test]
    fn test_pitch_placeholder_custom() {
        assert_eq!(apply_pitch_placeholder("C4", "custom", "h"), "C(h)4");
        assert_eq!(apply_pitch_placeholder("C4", "custom", ""), "C4"); // 空文本退化
        assert_eq!(apply_pitch_placeholder("C4", "custom", "  "), "C4");
    }

    #[test]
    fn test_pitch_placeholder_unknown_rule() {
        // 未知占位符规则:原样返回
        assert_eq!(apply_pitch_placeholder("C4", "bogus", "x"), "C4");
    }

    #[test]
    fn test_pitch_placeholder_negative_octave() {
        // C-1(八度为负):letter 部分是 "C-" 不匹配纯音名,原样返回
        assert_eq!(apply_pitch_placeholder("C-1", "dash", "x"), "C-1");
    }
}
