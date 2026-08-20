//! LRC 歌词解析。编码探测顺序：utf-8-sig、utf-8、gbk、gb2312、shift-jis。

use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct LrcLine {
    pub time: f64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct LrcFile {
    pub lines: Vec<LrcLine>,
}

impl LrcFile {
    /// 解析 LRC 文本，返回按时间戳升序排列的歌词行。
    pub fn parse(text: &str) -> Result<Self, String> {
        // 去除 UTF-8 BOM
        let data = text.strip_prefix('\u{FEFF}').unwrap_or(text);

        let mut lines = Vec::new();
        for raw_line in data.lines() {
            let bytes = raw_line.as_bytes();
            let mut pos: usize = 0;
            while pos < bytes.len() {
                if bytes[pos] != b'[' {
                    pos += 1;
                    continue;
                }
                if let Some(end_rel) = raw_line[pos + 1..].find(']') {
                    let tag = &raw_line[pos + 1..pos + 1 + end_rel];
                    if let Some((min, sec, ms)) = parse_time_tag(tag) {
                        let lyric_start = pos + 1 + end_rel + 1; // after ']'
                        let rest = &raw_line[lyric_start..];
                        let text = match rest.find('[') {
                            Some(idx) => rest[..idx].trim(),
                            None => rest.trim(),
                        };
                        if !text.is_empty() {
                            lines.push(LrcLine {
                                time: min * 60.0 + sec + ms,
                                text: text.to_string(),
                            });
                        }
                        pos = lyric_start;
                    } else {
                        pos += 1;
                    }
                } else {
                    break;
                }
            }
        }

        lines.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(Ordering::Equal));
        Ok(LrcFile { lines })
    }

    /// 按规格第三节编码顺序解析原始字节：
    /// utf-8-sig → utf-8 → gbk → gb2312 → shift-jis，第一个成功的编码为最终编码。
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, String> {
        let text = decode_lrc(bytes)?;
        Self::parse(&text)
    }

    /// 根据当前播放时间查找最后一个时间戳 <= elapsed 的歌词行（单调推进）。
    pub fn current_line(&self, elapsed: f64) -> Option<&str> {
        let mut current: Option<&LrcLine> = None;
        for line in &self.lines {
            if line.time > elapsed + 1e-6 {
                break;
            }
            current = Some(line);
        }
        current.map(|l| l.text.as_str())
    }
}

/// 多编码探测：UTF-8(含 BOM) 严格验证，失败后依次尝试 GBK / GB2312 / Shift-JIS。
/// 后三者用 replacement 字符 (U+FFFD) 作为"是否可读"的启发式判定，
/// 最终兜底 `from_utf8_lossy`（与旧行为一致）。
fn decode_lrc(bytes: &[u8]) -> Result<String, String> {
    // UTF-8 严格验证（BOM 是合法 UTF-8 字节序，天然覆盖 utf-8-sig）
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok(s.to_string());
    }

    // GBK 为 GB2312 的超集,encoding_rs 的 GBK 解码器同时覆盖 gb2312;
    // 顺序对应规格的 utf-8-sig → utf-8 → gbk → gb2312 → shift-jis。
    for encoding in [encoding_rs::GBK, encoding_rs::SHIFT_JIS] {
        let (decoded, _, had_errors) = encoding.decode(bytes);
        if !had_errors && !decoded.contains('\u{FFFD}') {
            return Ok(decoded.into_owned());
        }
    }

    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn parse_time_tag(tag: &str) -> Option<(f64, f64, f64)> {
    let mut parts = tag.split(':');
    let min = parts.next()?.parse::<f64>().ok()?;
    let rest = parts.next()?;
    let mut sec_parts = rest.split('.');
    let sec = sec_parts.next()?.parse::<f64>().ok()?;
    let ms_str = sec_parts.next()?;
    let mut ms: f64 = ms_str.parse().ok()?;
    // 毫秒两位时需乘以 10（如 .45 -> 450ms）
    if ms_str.len() == 2 {
        ms *= 10.0;
    }
    Some((min, sec, ms / 1000.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lrc_basic() {
        let text = "[00:12.00]第一行\n[00:24.45]第二行\n[01:00.00]第三行";
        let lrc = LrcFile::parse(text).unwrap();
        assert_eq!(lrc.lines.len(), 3);
        assert_eq!(lrc.lines[0].text, "第一行");
        assert_eq!(lrc.lines[1].time, 24.45);
        assert_eq!(lrc.lines[2].time, 60.00);
    }

    #[test]
    fn test_current_line() {
        let text = "[00:05.00]A\n[00:10.00]B";
        let lrc = LrcFile::parse(text).unwrap();
        assert_eq!(lrc.current_line(4.0), None);
        assert_eq!(lrc.current_line(5.0), Some("A"));
        assert_eq!(lrc.current_line(7.0), Some("A"));
        assert_eq!(lrc.current_line(10.0), Some("B"));
    }

    #[test]
    fn test_parse_bytes_utf8() {
        let text = "[00:12.00]第一行\n[00:24.45]第二行";
        let lrc = LrcFile::parse_bytes(text.as_bytes()).unwrap();
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].text, "第一行");
        assert_eq!(lrc.lines[1].time, 24.45);
    }

    #[test]
    fn test_parse_bytes_utf8_bom() {
        let text = "\u{FEFF}[00:05.00]A\n[00:10.00]B";
        let lrc = LrcFile::parse_bytes(text.as_bytes()).unwrap();
        assert_eq!(lrc.lines[0].text, "A");
        assert_eq!(lrc.lines.len(), 2);
    }

    #[test]
    fn test_parse_bytes_gbk() {
        // GBK 编码的中文歌词:UTF-8 严格验证必然失败,应回退到 GBK 并正确解码
        let text = "[00:12.00]第一行\n[00:24.45]第二行";
        let (bytes, _, _) = encoding_rs::GBK.encode(text);
        let lrc = LrcFile::parse_bytes(&bytes).unwrap();
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].text, "第一行");
        assert_eq!(lrc.lines[1].text, "第二行");
        assert!((lrc.lines[1].time - 24.45).abs() < 1e-6);
    }

    #[test]
    fn test_parse_bytes_shift_jis() {
        // 用 Shift-JIS 的单字节半角片假名构造:0xB1('ｱ')在 GBK 中是双字节首字节,
        // 后续 '\n' 不是合法尾字节 → GBK 严格解码失败 → 回退到 Shift-JIS。
        // 而全角日文(如"こんにちは")在 GBK 中可被宽松解码,按规格顺序会被 GBK 拦截,
        // 因此这里刻意选择 GBK 必然失败的输入来验证 Shift-JIS 分支。
        let text = "[00:12.00]\u{FF71}\n"; // ｱ
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(text);
        let lrc = LrcFile::parse_bytes(&bytes).unwrap();
        assert_eq!(lrc.lines.len(), 1);
        assert_eq!(lrc.lines[0].text, "\u{FF71}");
    }

    #[test]
    fn test_parse_bytes_invalid_fallback_lossy() {
        // 完全非法字节序列:兜底 lossy 不崩溃
        let bytes = [0xFFu8, 0xFE, 0x00, 0x80, 0x81];
        let lrc = LrcFile::parse_bytes(&bytes).unwrap();
        assert!(lrc.lines.is_empty()); // 无法解析出任何时间戳行
    }
}
