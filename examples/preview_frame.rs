//! 临时预览工具：渲染一帧并保存为 BMP/png，用于目视检查音名与歌字是否重合。
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
    let row_size = (width * 3).div_ceil(4) * 4;
    let image_size = row_size * height;
    let file_size = (14 + 40 + image_size) as usize;
    let mut buf = Vec::with_capacity(file_size);
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&(file_size as u32).to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(&(54u32).to_le_bytes());
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&(width as i32).to_le_bytes());
    buf.extend_from_slice(&(height as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&24u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&image_size.to_le_bytes());
    buf.extend_from_slice(&[0u8; 16]);
    for y in (0..height as usize).rev() {
        for x in 0..width as usize {
            let idx = (y * width as usize + x) * 4;
            buf.push(rgba[idx + 2]);
            buf.push(rgba[idx + 1]);
            buf.push(rgba[idx]);
        }
        let pad = (4 - ((width as usize * 3) % 4)) % 4;
        buf.resize(buf.len() + pad, 0);
    }
    if let Some(p) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(p);
    }
    let mut f = File::create(path).map_err(|e| e.to_string())?;
    f.write_all(&buf).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0, "create context failed");

    // 400x225,一个音符(歌词 A,note_num=69 -> 音名 A4),显示全部。
    let json = r##"{
        "ust": {"tempo": 120.0, "notes": [{"index":"0000","length":480,"lyric":"A","note_num":69,"phoneme":"","pitch_bend":[]}]},
        "show": {"bpm": true, "play_time": true, "song_name": true, "song_author": true, "ust_author": true, "lyric": false, "curve_show": false},
        "project": {"song_name":"Test","song_author":"Author","ust_author":"Tuner"},
        "style": {"bg_color":"#000000","note_color":"#6c6c6c","lyric_color":"#FFFFFF","fullscreen":false},
        "width": 1920, "height": 1080, "fps": 60, "output_path": ""
    }"##;
    let c_json = std::ffi::CString::new(json).expect("json contains nul");
    let rc = unsafe { ffi::up_set_config(ctx, c_json.as_ptr()) };
    assert_eq!(rc, 0, "set_config failed: {}", last_error(ctx));

    let w = 1920u32;
    let h = 1080u32;
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let mut ow = 0i32;
    let mut oh = 0i32;
    let rc = unsafe {
        ffi::up_render_to_buffer(
            ctx,
            0.0,
            buf.as_mut_ptr(),
            buf.len() as i32,
            &mut ow,
            &mut oh,
        )
    };
    assert_eq!(rc, 0, "render_to_buffer failed: {}", last_error(ctx));
    save_bmp24("verify/preview.bmp", &buf, w, h).expect("save bmp failed");
    println!("wrote verify/preview.bmp {}x{}", ow, oh);
    unsafe { ffi::up_destroy_context(ctx) };
}
