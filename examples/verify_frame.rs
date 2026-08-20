/// 渲染单帧并保存为 BMP24，用于目视/像素级校验。
use std::ffi::{c_char, CStr};
use std::{
    fs::{self, File},
    io::Write,
};

use ustplayer_renderer::ffi;

fn last_error(ctx: u64) -> String {
    unsafe {
        let ptr: *const c_char = ffi::up_last_error(ctx);
        if ptr.is_null() {
            return "<null>".into();
        }
        match CStr::from_ptr(ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => "<invalid utf-8>".into(),
        }
    }
}

fn save_bmp24(path: &str, rgba: &[u8], width: u32, height: u32) -> Result<(), String> {
    let row_size = (width * 3).div_ceil(4) * 4; // each row padded to 4 bytes
    let image_size = row_size * height;
    let file_size = (14 + 40 + image_size) as usize;

    let mut buf = Vec::with_capacity(file_size);
    // BMP file header (14 bytes)
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&(file_size as u32).to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]); // reserved
    buf.extend_from_slice(&(54u32).to_le_bytes()); // offset to pixel data

    // DIB header BITMAPINFOHEADER (40 bytes)
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&(width as i32).to_le_bytes());
    buf.extend_from_slice(&(height as i32).to_le_bytes()); // bottom-up DIB
    buf.extend_from_slice(&1u16.to_le_bytes()); // planes
    buf.extend_from_slice(&24u16.to_le_bytes()); // bpp (24-bit RGB)
    buf.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB (no compression)
    buf.extend_from_slice(&image_size.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // x pixels per meter
    buf.extend_from_slice(&0u32.to_le_bytes()); // y pixels per meter
    buf.extend_from_slice(&0u32.to_le_bytes()); // colors used
    buf.extend_from_slice(&0u32.to_le_bytes()); // important colors

    // pixel data (BGR, bottom-up)
    for y in (0..height as usize).rev() {
        for x in 0..width as usize {
            let idx = (y * width as usize + x) * 4;
            buf.push(rgba[idx + 2]); // B
            buf.push(rgba[idx + 1]); // G
            buf.push(rgba[idx]); // R
        }
        let pad = (4 - ((width as usize * 3) % 4)) % 4;
        buf.resize(buf.len() + pad, 0);
    }

    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut f = File::create(path).map_err(|e| e.to_string())?;
    f.write_all(&buf).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0, "create context failed");

    let json = include_str!("frame_config.json");
    let c_json = std::ffi::CString::new(json).expect("json contains nul");
    let rc = unsafe { ffi::up_set_config(ctx, c_json.as_ptr()) };
    assert_eq!(rc, 0, "set_config failed: {}", last_error(ctx));

    let mut w = 0i32;
    let mut h = 0i32;
    let mut buf = vec![0u8; (192 * 108 * 4) as usize];

    let rc = unsafe {
        ffi::up_render_to_buffer(ctx, 0.0, buf.as_mut_ptr(), buf.len() as i32, &mut w, &mut h)
    };
    assert_eq!(rc, 0, "render_to_buffer failed: {}", last_error(ctx));
    assert_eq!(w, 192);
    assert_eq!(h, 108);

    save_bmp24("verify/frame.bmp", &buf, w as u32, h as u32).expect("save bmp failed");

    // 简单统计：非黑像素数
    let mut non_black = 0usize;
    for chunk in buf.chunks_exact(4) {
        if chunk[0] != 0 || chunk[1] != 0 || chunk[2] != 0 {
            non_black += 1;
        }
    }
    println!("verify/frame.bmp {}x{} non-black={}", w, h, non_black);

    unsafe { ffi::up_destroy_context(ctx) };
}
