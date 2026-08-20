/// 字体加载与度量，多字体回退策略（同 Python 端）。
///
/// 维护常规 / 粗体两套字体链：音名、歌字、曲名用粗体，其余用常规。
use std::path::PathBuf;

use fontdue::{Font, FontSettings};

/// 字重选择
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Bold,
}

#[derive(Debug)]
pub struct FontContext {
    pub regular: Vec<Font>,
    pub bold: Vec<Font>,
    pub fallback_order: Vec<String>,
}

#[derive(Debug)]
pub struct FontMetrics {
    pub line: fontdue::LineMetrics,
    pub width: f32,
}

/// 常规字体候选（文件名 → 是否可能在 ttc 集合中的 index 1 为粗体）
const REGULAR_FILES: &[&str] = &[
    "DengXian.ttf",
    "msyh.ttc",
    "simhei.ttf",
    "NotoSansCJKsc-Regular.otf",
    "NotoSansSC-Regular.otf",
    "segoeui.ttf",
    "arial.ttf",
];

/// 粗体字体候选
const BOLD_FILES: &[&str] = &[
    "Dengb.ttf",
    "msyhbd.ttc",
    "NotoSansCJKsc-Bold.otf",
    "NotoSansSC-Bold.otf",
    "segoeuib.ttf",
    "arialbd.ttf",
];

impl FontContext {
    pub fn new() -> Result<Self, String> {
        let regular = load_system_fonts(REGULAR_FILES)?;
        // 粗体缺失时退化为常规字体，保证功能可用
        let bold = match load_system_fonts(BOLD_FILES) {
            Ok(f) => f,
            Err(_) => regular.clone(),
        };
        let ctx = Self {
            regular,
            bold,
            fallback_order: vec![
                "DengXian".into(),
                "Microsoft YaHei".into(),
                "SimHei".into(),
                "Noto Sans CJK SC".into(),
                "Noto Sans SC".into(),
                "Segoe UI".into(),
                "Arial".into(),
            ],
        };
        Ok(ctx)
    }

    pub fn font_for_char(&self, ch: char, weight: Weight) -> &Font {
        let chain = match weight {
            Weight::Regular => &self.regular,
            Weight::Bold => &self.bold,
        };
        for font in chain {
            if is_cjk(ch) || has_glyph(font, ch) {
                return font;
            }
        }
        // 兜底：常规链第一个
        self.regular.first().expect("at least one font loaded")
    }

    pub fn line_metrics(&self, font: &Font, font_size: f32) -> Option<fontdue::LineMetrics> {
        font.horizontal_line_metrics(font_size)
    }

    pub fn measure_text_width(&self, text: &str, font_size: f32, weight: Weight) -> f32 {
        let mut width = 0.0;
        for ch in text.chars() {
            let font = self.font_for_char(ch, weight);
            let (metrics, _) = font.rasterize(ch, font_size);
            width += metrics.advance_width;
        }
        width
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}' |
        '\u{3400}'..='\u{4DBF}' |
        '\u{F900}'..='\u{FAFF}' |
        '\u{3040}'..='\u{309F}' |
        '\u{30A0}'..='\u{30FF}' |
        '\u{AC00}'..='\u{D7AF}'
    )
}

fn has_glyph(font: &Font, ch: char) -> bool {
    let idx = font.lookup_glyph_index(ch);
    idx != 0
}

fn load_system_fonts(files: &[&str]) -> Result<Vec<Font>, String> {
    let mut out = Vec::new();
    let mut missing = Vec::new();

    for name in files {
        match find_system_font(name) {
            Ok(p) => {
                let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
                match Font::from_bytes(bytes, FontSettings::default()) {
                    Ok(f) => {
                        out.push(f);
                    }
                    Err(_) => missing.push(p.display().to_string()),
                }
            }
            Err(_) => missing.push(name.to_string()),
        }
    }

    if out.is_empty() {
        return Err(format!("font not found among {:?}", missing));
    }
    Ok(out)
}

fn find_system_font(name: &str) -> Result<PathBuf, String> {
    let paths = [
        std::env::var("WINDIR")
            .ok()
            .map(|w| format!("{}\\Fonts\\{}", w, name)),
        Some(format!("C:\\Windows\\Fonts\\{}", name)),
    ];
    for p in paths.iter().filter_map(|x| x.as_ref()) {
        let buf = PathBuf::from(p);
        if buf.exists() {
            return Ok(buf);
        }
    }
    if name.len() > 4 {
        for dir in ["C:\\Windows\\Fonts", "C:\\Windows\\System32\\Fonts"] {
            let base = PathBuf::from(dir).join(name);
            if base.exists() {
                return Ok(base);
            }
        }
    }
    Err(name.to_string())
}
