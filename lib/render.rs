use crate::fonts::Weight;
use crate::model::NoteInfo;
use crate::model::RenderConfig;
use crate::{fonts, text};
use tiny_skia::*;

pub struct DisplayState<'a> {
    pub note_name: &'a str,
    pub lyric: &'a str,
    pub is_ended: bool,
    pub play_time: f64,
    pub current_note: Option<&'a NoteInfo>,
    pub lrc_lyric: &'a str,
}

pub fn render_frame(
    pixmap: &mut Pixmap,
    config: &RenderConfig,
    fonts: &mut fonts::FontContext,
    state: &DisplayState,
) -> Result<(), String> {
    let w = pixmap.width() as f32;
    let h = pixmap.height() as f32;
    if w <= 0.0 || h <= 0.0 {
        return Err("invalid canvas size".into());
    }
    let cx = w / 2.0;
    let cy = h / 2.0;

    // 全分辨率兼容：以 1080p 为基准，所有字号与边距按高度等比缩放(比例因子 k)，
    // 保证任意分辨率(从极小的预览到 4K/8K)下文字布局的比例一致、不破版。
    // k=1 时各项恰好等于参考(QPainter @1080p)的值。
    let k = h / 1080.0;
    // 边距(单位像素)随 k 缩放,并设下限保证小分辨率下不与边缘贴死/溢出。
    let margin = (20.0 * k).max(16.0);
    // 行距(左上角三行)随 k 缩放。
    let line_gap_song = 27.0 * k;
    let line_gap_author = 25.0 * k;
    // 音名 & 歌字(参考比例 2:1):歌字恒为音名的一半。
    // 避免原先用独立下限(note.min=50 / lyric.min=80)时,小分辨率下歌字被钳得比音名还大。
    let note_fs = (h * (2.0 / 3.0) * 0.4).max(50.0);
    let lyric_fs = note_fs * 0.5;

    fill_rect(pixmap, 0.0, 0.0, w, h, &config.style.bg_color, 255)?;

    let note_name = state.note_name;
    let lyric = state.lyric;

    // 1. 音名
    // 规格:粗体,水平/垂直居中,文本四周留 th*0.2 内边距。
    // 对称内边距不改变文本中心位置(块居中后 padding 相互抵消),故仅需居中绘制。
    if !note_name.is_empty() {
        let color = Color::from_rgba8(
            hex_r(&config.style.note_color),
            hex_g(&config.style.note_color),
            hex_b(&config.style.note_color),
            225,
        );
        draw_text_centered(
            pixmap,
            note_name,
            fonts,
            note_fs,
            color,
            cx,
            cy,
            Weight::Bold,
        )?;
    }

    // 2. 音高线
    if config.show.curve_show {
        if let Some(note) = state.current_note {
            let pb = &note.pitch_bend;
            let note_len = note.length;
            if pb.len() >= 2 && note_len > 0 {
                draw_pitch_curve(pixmap, config, pb, note_len, cx, cy, w, h)?;
            }
        }
    }

    // 3. 歌字
    // 规格:粗体,居中绘制,四周留 th*0.2 内边距(对称内边距不影响居中)。
    if !lyric.is_empty() {
        let color = Color::from_rgba8(
            hex_r(&config.style.lyric_color),
            hex_g(&config.style.lyric_color),
            hex_b(&config.style.lyric_color),
            255,
        );
        draw_text_centered(
            pixmap,
            lyric,
            fonts,
            lyric_fs,
            color,
            cx,
            cy,
            Weight::Bold,
        )?;
    }

    // 4. 左上角静态信息
    // 规格:曲名(title_font 14px 粗体)起始基线 (20, 34),绘制后 y_off += 27;
    //      作者(small_font 14px 常规)后 y_off += 25;调音师最后。
    // 全分辨率:小字随 k 等比缩放(下限保持可读),基线/边距用 margin。
    // 参考 QFont(family,14) 是【点】字号≈19px(96dpi),故取 19px 基准,而非 14px。
    let small_fs = (19.0 * k).max(14.0);
    {
        let color = Color::from_rgba8(
            hex_r(&config.style.other_text_color),
            hex_g(&config.style.other_text_color),
            hex_b(&config.style.other_text_color),
            255,
        );
        let mut y_off = margin;

        if config.show.song_name && !config.project.song_name.is_empty() {
            let baseline_y = y_off + small_fs; // 1080p: 20 + 14 = 34
            draw_text_at_baseline(
                pixmap,
                &config.project.song_name,
                fonts,
                small_fs,
                color,
                margin,
                baseline_y,
                Weight::Bold,
            )?;
            y_off += line_gap_song;
        }
        if config.show.song_author && !config.project.song_author.is_empty() {
            let baseline_y = y_off + small_fs;
            draw_text_at_baseline(
                pixmap,
                &config.project.song_author,
                fonts,
                small_fs,
                color,
                margin,
                baseline_y,
                Weight::Regular,
            )?;
            y_off += line_gap_author;
        }
        if config.show.ust_author && !config.project.ust_author.is_empty() {
            let baseline_y = y_off + small_fs;
            draw_text_at_baseline(
                pixmap,
                &config.project.ust_author,
                fonts,
                small_fs,
                color,
                margin,
                baseline_y,
                Weight::Regular,
            )?;
        }
    }

    // 5. 右上角 BPM
    if config.show.bpm {
        let bpm_text = format!("BPM={}", config.ust.tempo);
        let tw = fonts.measure_text_width(&bpm_text, small_fs, Weight::Regular);
        let color = Color::from_rgba8(
            hex_r(&config.style.other_text_color),
            hex_g(&config.style.other_text_color),
            hex_b(&config.style.other_text_color),
            255,
        );
        let x = w - margin - tw;
        // 参考:painter.drawText(ww-20-bpm_w, 34, ...) —— 1080p 基线 = margin+small_fs = 34。
        let baseline_y = margin + small_fs;
        draw_text_at_baseline(
            pixmap,
            &bpm_text,
            fonts,
            small_fs,
            color,
            x,
            baseline_y,
            Weight::Regular,
        )?;
    }

    // 6. 左下角播放时间（显示结束文字时隐藏秒表）
    if config.show.play_time && !state.is_ended {
        let time_text = text::format_play_time(state.play_time);
        let color = Color::from_rgba8(
            hex_r(&config.style.other_text_color),
            hex_g(&config.style.other_text_color),
            hex_b(&config.style.other_text_color),
            255,
        );
        let x = margin;
        // 参考:painter.drawText(20, wh-20, ...) —— 1080p 基线 = h-margin = h-20。
        let baseline_y = h - margin;
        draw_text_at_baseline(
            pixmap,
            &time_text,
            fonts,
            small_fs,
            color,
            x,
            baseline_y,
            Weight::Regular,
        )?;
    }

    // 7. LRC 歌词
    if config.show.lyric && !state.lrc_lyric.is_empty() {
        let lyric_fs = (h * 0.03).max(10.0);
        let tw = fonts.measure_text_width(state.lrc_lyric, lyric_fs, Weight::Regular);
        let lrc_top = if config.style.lyric_pos == "top" {
            h * 0.3
        } else {
            h * 0.7
        };
        // 参考:painter.drawText(..., lrc_y, ...) —— 固定基线 lrc_y,非 top+ascent。
        let baseline_y = lrc_top;
        let x = w / 2.0 - tw / 2.0;
        let color = Color::from_rgba8(
            hex_r(&config.style.lyric_text_color),
            hex_g(&config.style.lyric_text_color),
            hex_b(&config.style.lyric_text_color),
            255,
        );
        draw_text_at_baseline(
            pixmap,
            state.lrc_lyric,
            fonts,
            lyric_fs,
            color,
            x,
            baseline_y,
            Weight::Regular,
        )?;
    }

    // 8. 底部版权
    {
        let copy_text = format!("Presented with ustPlayer - {}", config.style.app_version);
        let copy_fs = (16.0 * k).max(12.0);
        let tw = fonts.measure_text_width(&copy_text, copy_fs, Weight::Regular);
        let x = w / 2.0 - tw / 2.0;
        // 参考:painter.drawText(..., wh-20, ...) —— 1080p 基线 = h-margin = h-20。
        let baseline_y = h - margin;
        let color = Color::from_rgba8(195, 195, 195, 100);
        draw_text_at_baseline(
            pixmap,
            &copy_text,
            fonts,
            copy_fs,
            color,
            x,
            baseline_y,
            Weight::Regular,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)] // 居中绘制参数较多,保持显式可读
fn draw_text_centered(
    pixmap: &mut Pixmap,
    text: &str,
    fonts: &mut fonts::FontContext,
    font_size: f32,
    color: Color,
    cx: f32,
    cy: f32,
    weight: Weight,
) -> Result<(), String> {
    // 居中按"实际墨迹(ink)"包围盒:cosmic-text 返回每个字形相对(行首/基线)的墨迹位置,
    // 求出整串墨迹包围盒,把其中心对齐到 (cx,cy)。这样音名/歌字、CJK/拉丁、有无下降部
    // 都落到同一中心,不会因字体行高/度量差异而错位。
    let shaped = fonts.shape(text, font_size, weight)?;
    if shaped.glyphs.is_empty() {
        return Ok(());
    }
    let ink_cx = (shaped.ink_left + shaped.ink_right) * 0.5;
    let ink_cy = (shaped.ink_top + shaped.ink_bottom) * 0.5;
    let origin_x = cx - ink_cx;
    let origin_y = cy - ink_cy;
    for g in &shaped.glyphs {
        blit_glyph(
            pixmap,
            &g.data,
            g.w,
            g.h,
            origin_x + g.x,
            origin_y + g.y,
            color,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // 基线绘制参数较多,保持显式可读
fn draw_text_at_baseline(
    pixmap: &mut Pixmap,
    text: &str,
    fonts: &mut fonts::FontContext,
    font_size: f32,
    color: Color,
    x: f32,
    baseline_y: f32,
    weight: Weight,
) -> Result<(), String> {
    // 基线定位:cosmic-text 给的字形位置已相对基线(y=0)与行首(x=0),
    // 直接以 (x, baseline_y) 为原点贴上去即可;亚像素由 cosmic-text 定位,无逐字形抖动。
    let shaped = fonts.shape(text, font_size, weight)?;
    for g in &shaped.glyphs {
        blit_glyph(
            pixmap,
            &g.data,
            g.w,
            g.h,
            x + g.x,
            baseline_y + g.y,
            color,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // 折线绘制参数较多,保持显式可读
fn draw_pitch_curve(
    pixmap: &mut Pixmap,
    config: &RenderConfig,
    pb: &[i32],
    note_len: i32,
    cx: f32,
    cy: f32,
    _w: f32,
    h: f32,
) -> Result<(), String> {
    if pb.len() < 2 {
        return Ok(());
    }
    let curve_width = note_len as f32;
    let start_x = cx - curve_width / 2.0;
    let pb_count = pb.len();
    // 全分辨率:安全区/下限/线宽随高度等比缩放(1080p 基准),避免小分辨率下曲线被固定
    // 100px/50px 安全区过度压缩、或大分辨率下安全区占比过小。
    let k = h / 1080.0;
    let safe_top = 100.0 * k;
    let safe_bottom = h - safe_top;
    let clamp = (50.0 * k).max(1.0);
    let mut points = Vec::with_capacity(pb_count);
    for (i, &bend) in pb.iter().enumerate() {
        let x = start_x + (i as f32 / (pb_count - 1) as f32) * curve_width;
        let y_offset = (bend as f32 / 100.0) * (h * 0.09);
        let mut y = cy - y_offset;
        if y < safe_top {
            let exceed = safe_top - y;
            let scale = 0.3f32.max(1.0 - exceed / h * 2.0);
            y = safe_top - exceed * scale;
        } else if y > safe_bottom {
            let exceed = y - safe_bottom;
            let scale = 0.3f32.max(1.0 - exceed / h * 2.0);
            y = safe_bottom + exceed * scale;
        }
        y = y.clamp(clamp, h - clamp);
        points.push(Point { x, y });
    }
    if points.len() < 2 {
        return Ok(());
    }

    let mut pb = PathBuilder::new();
    pb.move_to(points[0].x, points[0].y);
    for p in &points[1..] {
        pb.line_to(p.x, p.y);
    }
    let Some(path) = pb.finish() else {
        return Ok(());
    };

    let color = Color::from_rgba8(
        hex_r(&config.style.pitch_curve_color),
        hex_g(&config.style.pitch_curve_color),
        hex_b(&config.style.pitch_curve_color),
        255,
    );
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;

    let stroke = Stroke {
        width: 5.0 * k,
        ..Default::default()
    };

    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    Ok(())
}

fn fill_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    hex: &str,
    alpha: u8,
) -> Result<(), String> {
    let rect = Rect::from_xywh(x, y, w, h).ok_or("invalid rect")?;
    let mut paint = Paint::default();
    let (r, g, b) = hex_to_rgb(hex);
    paint.set_color(Color::from_rgba8(r, g, b, alpha));
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    Ok(())
}

fn blit_glyph(
    pixmap: &mut Pixmap,
    bitmap: &[u8],
    gw: usize,
    gh: usize,
    x: f32,
    y: f32,
    color: Color,
) -> Result<(), String> {
    let width = pixmap.width();
    let height = pixmap.height();
    let data = pixmap.data_mut();
    let x0 = x.round() as i32;
    let y0 = y.round() as i32;

    let src_r = color.red();
    let src_g = color.green();
    let src_b = color.blue();
    let src_a = color.alpha();

    for row in 0..gh {
        for col in 0..gw {
            let px = x0 + col as i32;
            let py = y0 + row as i32;
            if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                continue;
            }
            let mask = bitmap[row * gw + col] as f32 / 255.0;
            if mask <= 0.0 {
                continue;
            }
            let idx = (py as u32 * width + px as u32) as usize * 4;
            let dst_r = data[idx] as f32 / 255.0;
            let dst_g = data[idx + 1] as f32 / 255.0;
            let dst_b = data[idx + 2] as f32 / 255.0;
            let dst_a = data[idx + 3] as f32 / 255.0;

            let eff_a = src_a * mask;
            // 预乘语义:颜色通道乘有效 alpha(源 alpha × 覆盖度),
            // 否则半透明源会偏亮(颜色未随 alpha 衰减)。
            let out_a = eff_a + dst_a * (1.0 - eff_a);
            let out_r = src_r * eff_a + dst_r * (1.0 - eff_a);
            let out_g = src_g * eff_a + dst_g * (1.0 - eff_a);
            let out_b = src_b * eff_a + dst_b * (1.0 - eff_a);

            data[idx] = (out_r * 255.0).round() as u8;
            data[idx + 1] = (out_g * 255.0).round() as u8;
            data[idx + 2] = (out_b * 255.0).round() as u8;
            data[idx + 3] = (out_a * 255.0).round() as u8;
        }
    }
    Ok(())
}

fn hex_r(hex: &str) -> u8 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        u8::from_str_radix(&h[0..2], 16).unwrap_or(0)
    } else {
        0
    }
}
fn hex_g(hex: &str) -> u8 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        u8::from_str_radix(&h[2..4], 16).unwrap_or(0)
    } else {
        0
    }
}
fn hex_b(hex: &str) -> u8 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        u8::from_str_radix(&h[4..6], 16).unwrap_or(0)
    } else {
        0
    }
}

fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    (hex_r(hex), hex_g(hex), hex_b(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 半透明白色(alpha=128)叠到不透明黑背景上:
    /// 期望结果是 50% 灰 (128,128,128,255),而不是偏亮的 (255,255,255,255)。
    #[test]
    fn test_blit_glyph_respects_source_alpha() {
        let mut pm = Pixmap::new(4, 4).unwrap();
        pm.fill(Color::BLACK);

        let bitmap = vec![255u8; 1]; // 1x1 全覆盖 glyph
        let color = Color::from_rgba8(255, 255, 255, 128);
        blit_glyph(&mut pm, &bitmap, 1, 1, 0.0, 0.0, color).unwrap();

        let px = pm.pixel(0, 0).unwrap();
        // 0.502 预乘到 premultiplied 缓冲 ≈ 128
        assert!(
            (px.red() as i32 - 128).abs() <= 1,
            "expected red≈128, got {}",
            px.red()
        );
        assert!(
            (px.green() as i32 - 128).abs() <= 1,
            "expected green≈128, got {}",
            px.green()
        );
        assert!(
            (px.blue() as i32 - 128).abs() <= 1,
            "expected blue≈128, got {}",
            px.blue()
        );
        assert!(
            (px.alpha() as i32 - 255).abs() <= 1,
            "expected alpha≈255, got {}",
            px.alpha()
        );
    }

    /// 不透明色(alpha=255)混合结果应与源色一致(回归保护)。
    #[test]
    fn test_blit_glyph_opaque() {
        let mut pm = Pixmap::new(4, 4).unwrap();
        pm.fill(Color::BLACK);

        let bitmap = vec![255u8; 1];
        let color = Color::from_rgba8(108, 108, 108, 255); // note_color 默认
        blit_glyph(&mut pm, &bitmap, 1, 1, 0.0, 0.0, color).unwrap();

        let px = pm.pixel(0, 0).unwrap();
        assert_eq!(px.red(), 108);
        assert_eq!(px.green(), 108);
        assert_eq!(px.blue(), 108);
        assert_eq!(px.alpha(), 255);
    }

    /// 覆盖度为 0 的 glyph 不应改变目标像素。
    #[test]
    fn test_blit_glyph_zero_mask() {
        let mut pm = Pixmap::new(4, 4).unwrap();
        pm.fill(Color::BLACK);

        let bitmap = vec![0u8; 1];
        let color = Color::from_rgba8(255, 255, 255, 128);
        blit_glyph(&mut pm, &bitmap, 1, 1, 0.0, 0.0, color).unwrap();

        let px = pm.pixel(0, 0).unwrap();
        assert_eq!(px.red(), 0);
        assert_eq!(px.alpha(), 255); // 背景不透明黑
    }

    /// 回归:draw_text_centered 应按"实际墨迹(ink)"把中心对齐到 cy —— 这正是参考 QPainter
    /// drawText(rect, AlignCenter) 按文本 boundingRect 居中的语义。
    /// 这样音名("A4")与歌字("A"/"あ")不论字体/字号,墨迹中心都落在同一 cy → 二者重合。
    #[test]
    fn test_draw_text_centered_ink_centered_on_cy() {
        // FontContext 依赖系统字体;无字体时跳过,避免在无字体环境误报。
        let mut fonts = match crate::fonts::FontContext::new() {
            Ok(f) => f,
            Err(_) => return,
        };

        let w = 200u32;
        let h = 120u32;
        let cx = (w as f32) / 2.0;
        let cy = (h as f32) / 2.0;
        let mut pm = Pixmap::new(w, h).unwrap();
        pm.fill(Color::BLACK);

        draw_text_centered(&mut pm, "A4", &mut fonts, 40.0, Color::WHITE, cx, cy, Weight::Bold)
            .unwrap();

        // 统计非黑像素(RGB>0)的包围盒。
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        let mut any = false;
        for y in 0..h {
            for x in 0..w {
                let p = pm.pixel(x, y).unwrap();
                if p.red() > 0 || p.green() > 0 || p.blue() > 0 {
                    any = true;
                    let (xx, yy) = (x as i32, y as i32);
                    if xx < min_x {
                        min_x = xx;
                    }
                    if xx > max_x {
                        max_x = xx;
                    }
                    if yy < min_y {
                        min_y = yy;
                    }
                    if yy > max_y {
                        max_y = yy;
                    }
                }
            }
        }
        assert!(any, "centered text should produce ink");

        let ink_cx = (min_x + max_x) as f32 / 2.0;
        let ink_cy = (min_y + max_y) as f32 / 2.0;
        // 水平居中(推进宽度箱中心在 cx)。侧轴空隙会导致墨迹略偏,故留较宽容差。
        assert!(
            (ink_cx - cx).abs() <= 12.0,
            "ink horizontal center {ink_cx} should be ≈ cx {cx}"
        );
        // 垂直:墨迹中心应严格落在 cy(允许 2px 容差)。
        assert!(
            (ink_cy - cy).abs() <= 2.0,
            "ink vertical center {ink_cy} should be ≈ cy {cy} (min_y={min_y}, max_y={max_y})"
        );
    }
}
