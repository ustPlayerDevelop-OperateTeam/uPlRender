//! 文本：字体发现/加载（cosmic-text + fontdb）、排版（shape）与度量。
//!
//! 选用 cosmic-text（而非 fontdue）的原因：
//! - 它是真正的文本布局引擎：自动做字体回退、支持 .ttc、亚像素定位、度量正确；
//!   fontdue 只是"逐字形光栅化器"，无回退/无 ttc/度量与 Qt 不一致，正是此前
//!   音名·歌字错位、小字逐字符抖动的根因。
//! - `FontContext` 对外提供 `shape`（返回字形墨迹 + 包围盒，供居中/基线定位并光栅）
//!   与 `measure_text_width`（只布局不光栅）。

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};

/// 字重选择
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Bold,
}

/// 单个字形：灰度蒙版(0..255)与相对（行首 x=0, 基线 y=0）的墨迹左上角位置。
pub struct GlyphBitmap {
    pub x: f32,
    pub y: f32,
    pub data: Vec<u8>,
    pub w: usize,
    pub h: usize,
}

/// 一次 shape 的产物：字形序列 + 墨迹包围盒（相对行首/基线）+ 行宽。
pub struct ShapedText {
    pub glyphs: Vec<GlyphBitmap>,
    pub ink_left: f32,
    pub ink_top: f32,
    pub ink_right: f32,
    pub ink_bottom: f32,
    pub width: f32,
}

pub struct FontContext {
    font_system: FontSystem,
    cache: SwashCache,
    /// 主字体族（找不到时 cosmic-text 会自动回退）
    family: Family<'static>,
}

impl FontContext {
    pub fn new() -> Result<Self, String> {
        let mut font_system = FontSystem::new();
        font_system.db_mut().load_system_fonts();
        Ok(Self {
            font_system,
            cache: SwashCache::new(),
            // 参考实现用的主字体：等线 DengXian（Deng.ttf 常规 / Dengb.ttf 粗体）。
            // cosmic-text 对缺字符自动回退。
            family: Family::Name("DengXian"),
        })
    }

    /// 把我们的 Weight 映射成 fontdb 的 Weight。
    fn fweight(weight: Weight) -> cosmic_text::Weight {
        match weight {
            Weight::Regular => cosmic_text::Weight::NORMAL,
            Weight::Bold => cosmic_text::Weight::BOLD,
        }
    }

    /// 建一个单行排版的 Buffer（宽度取足够大避免换行）。
    fn layout_buffer(
        &mut self,
        text: &str,
        font_size: f32,
        weight: Weight,
    ) -> Buffer {
        let attrs = Attrs::new()
            .family(self.family)
            .weight(Self::fweight(weight));
        let metrics = Metrics::new(font_size, font_size * 1.25);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        // 宽度足够容纳全部字符 -> 单行；高度给一行量级。
        let wide = (text.chars().count() as f32 * font_size * 4.0).max(font_size * 4.0);
        buffer.set_size(&mut self.font_system, Some(wide), Some(font_size * 2.0));
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, true);
        buffer
    }

    /// 只布局不求墨迹：返回首行行宽（用于对齐/居中计算）。
    pub fn measure_text_width(&mut self, text: &str, font_size: f32, weight: Weight) -> f32 {
        let buffer = self.layout_buffer(text, font_size, weight);
        buffer
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0.0f32, |a, b| if a > b { a } else { b })
    }

    /// 排版 + 光栅化全部字形，返回各字形墨迹位置（相对行首/基线）与墨迹包围盒。
    pub fn shape(
        &mut self,
        text: &str,
        font_size: f32,
        weight: Weight,
    ) -> Result<ShapedText, String> {
        let buffer = self.layout_buffer(text, font_size, weight);

        let mut glyphs: Vec<GlyphBitmap> = Vec::new();
        let (mut ink_l, mut ink_t, mut ink_r, mut ink_b) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        let mut width = 0.0f32;

        // 分离借用：cache 与 font_system 两个独立字段，允许同时可变借用。
        let FontContext { ref mut font_system, ref mut cache, .. } = *self;

        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let key = physical.cache_key;
                let img_opt = cache.get_image(font_system, key);
                let Some(img) = img_opt.as_ref() else {
                    continue;
                };
                if img.content != cosmic_text::SwashContent::Mask {
                    continue; // 只处理 8-bit 灰度蒙版；彩色/亚像素蒙版跳过
                }
                let gw = img.placement.width as usize;
                let gh = img.placement.height as usize;
                if gw == 0 || gh == 0 {
                    continue;
                }
                // 墨迹左上角(相对基线 y=0 与行首 x=0)：
                //   x = physical.x + placement.left
                //   y = physical.y - placement.top
                // 注意 placement.top 是墨迹相对字形格(以基线为界向上)的量,随字形高度变化;
                // 必须"减去"它而非"加再减基线",否则越矮的字形(小写/标点)会被抬得越高、底边不齐。
                let gx = physical.x as f32 + img.placement.left as f32;
                let gy = physical.y as f32 - img.placement.top as f32;
                if gx < ink_l {
                    ink_l = gx;
                }
                if gy < ink_t {
                    ink_t = gy;
                }
                if gx + gw as f32 > ink_r {
                    ink_r = gx + gw as f32;
                }
                if gy + gh as f32 > ink_b {
                    ink_b = gy + gh as f32;
                }
                glyphs.push(GlyphBitmap {
                    x: gx,
                    y: gy,
                    data: img.data.clone(),
                    w: gw,
                    h: gh,
                });
            }
        }

        if ink_l > ink_r {
            // 没有任何可绘制墨迹
            return Ok(ShapedText {
                glyphs,
                ink_left: 0.0,
                ink_top: 0.0,
                ink_right: 0.0,
                ink_bottom: 0.0,
                width,
            });
        }
        Ok(ShapedText {
            glyphs,
            ink_left: ink_l,
            ink_top: ink_t,
            ink_right: ink_r,
            ink_bottom: ink_b,
            width,
        })
    }
}
