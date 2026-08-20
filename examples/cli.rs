use std::ffi::CString;

use ustplayer_renderer::ffi;

fn main() {
    let ctx = unsafe { ffi::up_create_context() };
    if ctx == 0 {
        eprintln!("create context failed");
        return;
    }

    let json = r#"{"ust":{"tempo":120.0,"notes":[]}}"#;
    let c_json = match CString::new(json) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("json contains nul");
            unsafe { ffi::up_destroy_context(ctx) };
            return;
        }
    };

    unsafe {
        let rc = ffi::up_set_config(ctx, c_json.as_ptr());
        if rc != ffi::UP_OK {
            let err = std::ffi::CStr::from_ptr(ffi::up_last_error(ctx));
            eprintln!("set_config error: {:?}", err);
            ffi::up_destroy_context(ctx);
            return;
        }

        ffi::up_set_progress_callback(ctx, progress_cb);

        let _ = ffi::up_begin_export(ctx);
        for i in 0..180 {
            let _ = ffi::up_render_frame(ctx, (i as f64) / 60.0);
        }
        let _ = ffi::up_end_export(ctx);
        ffi::up_destroy_context(ctx);
    }

    println!("done");
}

extern "C" fn progress_cb(progress: i32) {
    eprintln!("progress: {}/1000", progress);
}
