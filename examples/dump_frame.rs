/// 快速示例：渲染一帧到 PPM 文件，用于目视检查渲染管线。
use std::ffi::{c_char, CStr};
use std::fs::File;
use std::io::Write;

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

fn main() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0, "create context failed");

    let json = include_str!("frame_config.json");
    let c_json = std::ffi::CString::new(json).expect("json contains nul");
    let rc = unsafe { ffi::up_set_config(ctx, c_json.as_ptr()) };
    assert_eq!(rc, 0, "set_config failed: {}", last_error(ctx));

    let mut out_w = 0i32;
    let mut out_h = 0i32;
    let mut buf = vec![0u8; (192 * 108 * 4) as usize];

    let rc = unsafe {
        ffi::up_render_to_buffer(
            ctx,
            0.0,
            buf.as_mut_ptr(),
            buf.len() as i32,
            &mut out_w,
            &mut out_h,
        )
    };
    assert_eq!(rc, 0, "render_to_buffer failed: {}", last_error(ctx));
    assert_eq!(out_w, 192);
    assert_eq!(out_h, 108);

    let out_path = "target/release/frame.ppm";
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = File::create(out_path).expect("create ppm failed");
    f.write_all(b"P6\n192 108\n255\n")
        .expect("write ppm header failed");
    for chunk in buf.chunks_exact(4) {
        f.write_all(&[chunk[0], chunk[1], chunk[2]])
            .expect("write ppm data failed");
    }

    println!(
        "wrote {} -> {} bytes",
        out_path,
        std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0)
    );

    unsafe { ffi::up_destroy_context(ctx) };
}
