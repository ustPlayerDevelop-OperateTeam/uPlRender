use std::ffi::CString;

use ustplayer_renderer::ffi;

#[test]
fn test_create_and_destroy() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0, "create context returned 0");
    unsafe {
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_set_config_smoke() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0, "create context returned 0");

    let json = r#"{"ust":{"tempo":120.0,"notes":[]}}"#;
    let c_json = CString::new(json).expect("json contains nul");
    unsafe {
        let rc = ffi::up_set_config(ctx, c_json.as_ptr());
        assert_eq!(rc, 0, "set_config failed");
        let _ = ffi::up_begin_export(ctx);
        let _ = ffi::up_render_frame(ctx, 0.0);
        let _ = ffi::up_end_export(ctx);
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_invalid_config_returns_error() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0);

    // 非法 JSON
    let bad = CString::new("{not json").unwrap();
    unsafe {
        let rc = ffi::up_set_config(ctx, bad.as_ptr());
        assert_eq!(rc, ffi::UP_ERR_PARSE);
        // 错误消息可查询
        let ptr = ffi::up_last_error(ctx);
        assert!(!ptr.is_null());
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_null_config_returns_invalid_arg() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0);
    unsafe {
        let rc = ffi::up_set_config(ctx, std::ptr::null());
        assert_eq!(rc, ffi::UP_ERR_INVALID_ARG);
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_invalid_ctx_returns_invalid_arg() {
    unsafe {
        let json = CString::new(r#"{"ust":{"tempo":120.0}}"#).unwrap();
        assert_eq!(
            ffi::up_set_config(999999, json.as_ptr()),
            ffi::UP_ERR_INVALID_ARG
        );
        assert_eq!(ffi::up_render_frame(999999, 0.0), ffi::UP_ERR_INVALID_ARG);
        assert_eq!(ffi::up_begin_export(999999), ffi::UP_ERR_INVALID_ARG);
        assert_eq!(ffi::up_end_export(999999), ffi::UP_ERR_INVALID_ARG);
        assert!(ffi::up_last_error(999999).is_null());
    }
}

#[test]
fn test_empty_ust_render_to_buffer() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0);

    let json = r##"{
        "ust": {"tempo": 120.0, "notes": []},
        "width": 64, "height": 36, "fps": 30,
        "style": {"bg_color": "#000000"},
        "show": {"bpm": false, "play_time": false, "lyric": false}
    }"##;
    let c_json = CString::new(json).unwrap();
    unsafe {
        assert_eq!(ffi::up_set_config(ctx, c_json.as_ptr()), ffi::UP_OK);
        let mut w = 0i32;
        let mut h = 0i32;
        let mut buf = vec![0u8; 64 * 36 * 4];
        let rc =
            ffi::up_render_to_buffer(ctx, 0.0, buf.as_mut_ptr(), buf.len() as i32, &mut w, &mut h);
        assert_eq!(rc, ffi::UP_OK);
        assert_eq!(w, 64);
        assert_eq!(h, 36);
        // 空 UST:无音名/歌字/左上角信息,仅底部版权行有内容。
        // 顶部 2/3 区域应为纯黑;统计非零像素应远小于总像素。
        let total = 64usize * 36;
        let nonzero = buf
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
            .count();
        assert!(nonzero > 0, "copyright line should be visible");
        assert!(
            nonzero < total / 4,
            "expected only copyright text, got {} nonzero pixels of {}",
            nonzero,
            total
        );
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_render_to_buffer_too_small() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0);

    let json = r#"{"ust":{"tempo":120.0,"notes":[]},"width":64,"height":36}"#;
    let c_json = CString::new(json).unwrap();
    unsafe {
        assert_eq!(ffi::up_set_config(ctx, c_json.as_ptr()), ffi::UP_OK);
        let mut w = 0i32;
        let mut h = 0i32;
        let mut buf = vec![0u8; 10]; // 明显过小
        let rc =
            ffi::up_render_to_buffer(ctx, 0.0, buf.as_mut_ptr(), buf.len() as i32, &mut w, &mut h);
        assert_eq!(rc, ffi::UP_ERR_INVALID_ARG);
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_set_ust_text_and_render() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0);

    let json = r#"{"ust":{"tempo":120.0,"notes":[]},"width":64,"height":36}"#;
    let c_json = CString::new(json).unwrap();
    let ust_text =
        CString::new(r#"{"tempo":120.0,"notes":[{"length":480,"lyric":"A","note_num":69}]}"#)
            .unwrap();
    unsafe {
        assert_eq!(ffi::up_set_config(ctx, c_json.as_ptr()), ffi::UP_OK);
        // 空 UST:仅版权行,设置 UST 文本后渲染应出现歌字(内容显著增多)
        let mut w = 0i32;
        let mut h = 0i32;
        let mut buf = vec![0u8; 64 * 36 * 4];
        assert_eq!(
            ffi::up_render_to_buffer(ctx, 0.0, buf.as_mut_ptr(), buf.len() as i32, &mut w, &mut h),
            ffi::UP_OK
        );
        let nonzero_before = buf
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
            .count();

        assert_eq!(ffi::up_set_ust_text(ctx, ust_text.as_ptr()), ffi::UP_OK);
        assert_eq!(
            ffi::up_render_to_buffer(ctx, 0.0, buf.as_mut_ptr(), buf.len() as i32, &mut w, &mut h),
            ffi::UP_OK
        );
        let nonzero_after = buf
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
            .count();
        assert!(
            nonzero_after > nonzero_before,
            "expected more content after set_ust_text (before={}, after={})",
            nonzero_before,
            nonzero_after
        );
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_set_lrc_text() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0);

    let json = r#"{"ust":{"tempo":120.0,"notes":[]},"width":64,"height":36,"show":{"lyric":true}}"#;
    let c_json = CString::new(json).unwrap();
    let lrc_text = CString::new("[00:01.00]第一行\n[00:10.00]第二行").unwrap();
    unsafe {
        assert_eq!(ffi::up_set_config(ctx, c_json.as_ptr()), ffi::UP_OK);
        assert_eq!(ffi::up_set_lrc_text(ctx, lrc_text.as_ptr()), ffi::UP_OK);
        assert_eq!(
            ffi::up_set_lrc_text(ctx, std::ptr::null()),
            ffi::UP_ERR_INVALID_ARG
        );
        let bad = CString::new("not lrc [").unwrap();
        // 解析不会失败(找不到时间戳 → 空行),应返回 OK
        assert_eq!(ffi::up_set_lrc_text(ctx, bad.as_ptr()), ffi::UP_OK);
        ffi::up_destroy_context(ctx);
    }
}

#[test]
fn test_two_contexts_independent() {
    // 两个 ctx 独立工作(不同 ctx 并发语义的串行验证)
    let ctx1 = unsafe { ffi::up_create_context() };
    let ctx2 = unsafe { ffi::up_create_context() };
    assert!(ctx1 != 0 && ctx2 != 0);

    let json1 =
        CString::new(r#"{"ust":{"tempo":120.0,"notes":[]},"width":32,"height":32}"#).unwrap();
    let json2 =
        CString::new(r#"{"ust":{"tempo":90.0,"notes":[]},"width":48,"height":48}"#).unwrap();
    unsafe {
        assert_eq!(ffi::up_set_config(ctx1, json1.as_ptr()), ffi::UP_OK);
        assert_eq!(ffi::up_set_config(ctx2, json2.as_ptr()), ffi::UP_OK);

        let mut w1 = 0i32;
        let mut h1 = 0i32;
        let mut b1 = vec![0u8; 32 * 32 * 4];
        assert_eq!(
            ffi::up_render_to_buffer(
                ctx1,
                0.0,
                b1.as_mut_ptr(),
                b1.len() as i32,
                &mut w1,
                &mut h1
            ),
            ffi::UP_OK
        );
        assert_eq!((w1, h1), (32, 32));

        let mut w2 = 0i32;
        let mut h2 = 0i32;
        let mut b2 = vec![0u8; 48 * 48 * 4];
        assert_eq!(
            ffi::up_render_to_buffer(
                ctx2,
                0.0,
                b2.as_mut_ptr(),
                b2.len() as i32,
                &mut w2,
                &mut h2
            ),
            ffi::UP_OK
        );
        assert_eq!((w2, h2), (48, 48));

        ffi::up_destroy_context(ctx1);
        ffi::up_destroy_context(ctx2);
    }
}

#[test]
fn test_invalid_color_falls_back_black() {
    let ctx = unsafe { ffi::up_create_context() };
    assert!(ctx != 0);

    // 非法颜色(#GGGGGG)应回退为 0(黑)而不是 panic
    let json = r##"{"ust":{"tempo":120.0,"notes":[]},"width":64,"height":36,"style":{"bg_color":"#GGGGGG"}}"##;
    let c_json = CString::new(json).unwrap();
    unsafe {
        assert_eq!(ffi::up_set_config(ctx, c_json.as_ptr()), ffi::UP_OK);
        let mut w = 0i32;
        let mut h = 0i32;
        let mut buf = vec![0u8; 64 * 36 * 4];
        assert_eq!(
            ffi::up_render_to_buffer(ctx, 0.0, buf.as_mut_ptr(), buf.len() as i32, &mut w, &mut h),
            ffi::UP_OK
        );
        // 顶部 1/3 区域应为纯黑背景(无任何文字元素),证明非法颜色回退为黑
        let top_rows = 12usize;
        for y in 0..top_rows {
            for x in 0..64usize {
                let idx = (y * 64 + x) * 4;
                assert_eq!(
                    &buf[idx..idx + 4],
                    &[0, 0, 0, 255],
                    "pixel ({},{}) not black",
                    x,
                    y
                );
            }
        }
        ffi::up_destroy_context(ctx);
    }
}
