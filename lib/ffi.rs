//! C ABI 对外接口层。
//!
//! 所有导出函数均使用 `#[no_mangle] pub unsafe extern "C"`，内部以 `catch_unwind` 包裹，
//! 保证绝不 panic 跨越 FFI 边界。宿主仅需持有 `u64` 句柄，内存由库全权管理。
//!
//! 并发模型：句柄表为 `RwLock<HashMap<u64, Arc<Mutex<ContextInner>>>>`。
//! - 不同 ctx 之间互不阻塞（各自持自己的 `Mutex<ContextInner>`），可并发调用；
//! - 同一 ctx 的所有调用经内部 Mutex 串行化，符合规格"单线程使用"约定。
//!
//! 安全性说明：这些 `extern "C"` 函数是 FFI 边界，`unsafe` 用于接收裸指针/返回裸指针，
//! 属于 C ABI 接口的必要组成部分，其语义（内存归属、线程约定）记录于
//! `bindings/ustplayer_renderer.h`，调用方需遵守该头文件的约定。
#![allow(clippy::missing_safety_doc)]

use std::collections::HashMap;
use std::ffi::{c_char, c_uchar, CString};
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use crate::encode::{FfmpegEncoder, VideoEncoder};
use crate::fonts::FontContext;
use crate::lrc::LrcFile;
use crate::model::NoteInfo;
use crate::timing::TimingModel;
use crate::{render, text};

// ---------- 错误码 ----------

pub const UP_OK: i32 = 0;
pub const UP_ERR_INVALID_ARG: i32 = -1;
pub const UP_ERR_PARSE: i32 = -2;
pub const UP_ERR_IO: i32 = -3;
pub const UP_ERR_FONT: i32 = -4;
pub const UP_ERR_RENDER: i32 = -5;
pub const UP_ERR_ENCODE: i32 = -6;
pub const UP_ERR_INTERNAL: i32 = -99;

/// 内部统一错误：错误码 + 人类可读消息（最终写入 ctx 的 last_error）。
type FfiResult = Result<i32, (i32, String)>;

/// 进度回调
pub type ProgressFn = extern "C" fn(i32);

// ---------- 内部句柄与状态 ----------

struct ContextInner {
    config: Option<crate::model::RenderConfig>,
    last_error: Option<CString>,
    timing: Option<TimingModel>,
    fonts: Option<FontContext>,
    hint: usize,
    last_valid_lyric: Option<String>,
    lrc: Option<LrcFile>,
    encoder: Option<Box<dyn VideoEncoder + Send>>,
    progress_cb: Option<ProgressFn>,
}

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static HANDLES: OnceLock<RwLock<HashMap<u64, Arc<Mutex<ContextInner>>>>> = OnceLock::new();

fn get_handles() -> &'static RwLock<HashMap<u64, Arc<Mutex<ContextInner>>>> {
    HANDLES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// 取句柄对应的 Arc（克隆后释放表锁，避免跨 ctx 阻塞）。
fn get_handle(ctx: u64) -> Option<Arc<Mutex<ContextInner>>> {
    get_handles().read().unwrap().get(&ctx).cloned()
}

fn set_error(entry: &mut ContextInner, msg: impl Into<String>) {
    entry.last_error =
        Some(CString::new(msg.into()).unwrap_or_else(|_| CString::new("unknown error").unwrap()));
}

// ---------- 辅助：LRC 加载 ----------

fn load_lrc_if_any(path: &str) -> Option<LrcFile> {
    if path.trim().is_empty() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    // 多编码探测(utf-8-sig → utf-8 → gbk → gb2312 → shift-jis)在 lrc::parse_bytes 内完成
    LrcFile::parse_bytes(&bytes).ok()
}

/// 把某个 ctx 的错误写入并返回对应的对外错误码。
/// `msg` 为空串时只返回错误码（如参数错误，不覆盖已有错误消息）。
fn finish_err(ctx: u64, code: i32, msg: String) -> i32 {
    if !msg.is_empty() {
        if let Some(entry) = get_handle(ctx) {
            set_error(&mut entry.lock().unwrap(), msg);
        }
    }
    code
}

// ---------- 导出函数 ----------

/// 创建渲染上下文句柄。
#[no_mangle]
pub unsafe extern "C" fn up_create_context() -> u64 {
    match catch_unwind(|| -> Result<u64, String> {
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        let mut table = get_handles().write().unwrap();
        table.insert(
            handle,
            Arc::new(Mutex::new(ContextInner {
                config: None,
                last_error: None,
                timing: None,
                fonts: None,
                hint: 0,
                last_valid_lyric: None,
                lrc: None,
                encoder: None,
                progress_cb: None,
            })),
        );
        Ok(handle)
    }) {
        Ok(Ok(v)) => v,
        Ok(Err(_)) | Err(_) => 0,
    }
}

/// 销毁渲染上下文句柄，释放内部所有资源。
#[no_mangle]
pub unsafe extern "C" fn up_destroy_context(ctx: u64) {
    let _ = catch_unwind(|| {
        let mut table = get_handles().write().unwrap();
        table.remove(&ctx);
    });
}

/// 设置 JSON 配置（UTF-8）。字段缺失按默认值填充，未知字段忽略。
#[no_mangle]
pub unsafe extern "C" fn up_set_config(ctx: u64, json_config: *const c_char) -> i32 {
    match catch_unwind(|| -> FfiResult {
        if json_config.is_null() {
            return Err((UP_ERR_INVALID_ARG, String::new()));
        }

        let cstr = match std::ffi::CStr::from_ptr(json_config).to_str() {
            Ok(s) => s,
            Err(_) => return Err((UP_ERR_PARSE, "config is not valid UTF-8".into())),
        };

        let config: crate::model::RenderConfig = match serde_json::from_str(cstr) {
            Ok(c) => c,
            Err(e) => return Err((UP_ERR_PARSE, format!("parse config failed: {}", e))),
        };

        let fonts = match FontContext::new() {
            Ok(f) => f,
            Err(e) => return Err((UP_ERR_FONT, format!("font init failed: {}", e))),
        };

        let timing = TimingModel::new(config.ust.tempo, config.ust.notes.clone());
        let lrc = load_lrc_if_any(&config.style.lrc_path);

        let entry = match get_handle(ctx) {
            Some(e) => e,
            None => return Err((UP_ERR_INVALID_ARG, String::new())),
        };
        let mut guard = entry.lock().unwrap();
        guard.config = Some(config);
        guard.timing = Some(timing);
        guard.fonts = Some(fonts);
        guard.hint = 0;
        guard.last_valid_lyric = None;
        guard.lrc = lrc;
        Ok(UP_OK)
    }) {
        Ok(Ok(v)) => v,
        Ok(Err((code, msg))) => finish_err(ctx, code, msg),
        Err(_) => finish_err(ctx, UP_ERR_INTERNAL, "panic in up_set_config".into()),
    }
}

/// 设置 UST 文本（UTF-8），替换 config.ust 并重建时序模型。
/// 供宿主把已解析好的 UST 内容直接传入，避免二次读盘。
#[no_mangle]
pub unsafe extern "C" fn up_set_ust_text(ctx: u64, utf8: *const c_char) -> i32 {
    match catch_unwind(|| -> FfiResult {
        if utf8.is_null() {
            return Err((UP_ERR_INVALID_ARG, String::new()));
        }
        let text = match std::ffi::CStr::from_ptr(utf8).to_str() {
            Ok(s) => s,
            Err(_) => return Err((UP_ERR_PARSE, "ust text is not valid UTF-8".into())),
        };
        let ust: crate::model::UstInfo = match serde_json::from_str(text) {
            Ok(u) => u,
            Err(e) => return Err((UP_ERR_PARSE, format!("parse ust text failed: {}", e))),
        };
        let entry = match get_handle(ctx) {
            Some(e) => e,
            None => return Err((UP_ERR_INVALID_ARG, String::new())),
        };
        let mut guard = entry.lock().unwrap();
        let Some(config) = guard.config.as_mut() else {
            return Err((
                UP_ERR_INVALID_ARG,
                "config not set before up_set_ust_text".into(),
            ));
        };
        config.ust = ust.clone();
        guard.timing = Some(TimingModel::new(ust.tempo, ust.notes));
        guard.hint = 0;
        guard.last_valid_lyric = None;
        Ok(UP_OK)
    }) {
        Ok(Ok(v)) => v,
        Ok(Err((code, msg))) => finish_err(ctx, code, msg),
        Err(_) => finish_err(ctx, UP_ERR_INTERNAL, "panic in up_set_ust_text".into()),
    }
}

/// 设置 LRC 歌词文本（UTF-8 或 GBK 等，内部做多编码探测），替换 lrc_path 读取结果。
#[no_mangle]
pub unsafe extern "C" fn up_set_lrc_text(ctx: u64, utf8: *const c_char) -> i32 {
    match catch_unwind(|| -> FfiResult {
        if utf8.is_null() {
            return Err((UP_ERR_INVALID_ARG, String::new()));
        }
        let cstr = std::ffi::CStr::from_ptr(utf8).to_bytes();
        if cstr.is_empty() {
            return Err((UP_ERR_PARSE, "lrc text is empty".into()));
        }
        let lrc = match LrcFile::parse_bytes(cstr) {
            Ok(l) => l,
            Err(e) => return Err((UP_ERR_PARSE, format!("parse lrc text failed: {}", e))),
        };
        let entry = match get_handle(ctx) {
            Some(e) => e,
            None => return Err((UP_ERR_INVALID_ARG, String::new())),
        };
        entry.lock().unwrap().lrc = Some(lrc);
        Ok(UP_OK)
    }) {
        Ok(Ok(v)) => v,
        Ok(Err((code, msg))) => finish_err(ctx, code, msg),
        Err(_) => finish_err(ctx, UP_ERR_INTERNAL, "panic in up_set_lrc_text".into()),
    }
}

/// 初始化编码器，准备导出。
#[no_mangle]
pub unsafe extern "C" fn up_begin_export(ctx: u64) -> i32 {
    match catch_unwind(|| -> FfiResult {
        let entry = match get_handle(ctx) {
            Some(e) => e,
            None => return Err((UP_ERR_INVALID_ARG, String::new())),
        };
        let mut guard = entry.lock().unwrap();

        if guard.config.is_none() {
            return Err((
                UP_ERR_INVALID_ARG,
                "config not set before begin_export".into(),
            ));
        }

        let config = guard.config.as_ref().unwrap();
        let encoder: Box<dyn VideoEncoder + Send> = if config
            .output_path
            .to_ascii_lowercase()
            .ends_with(".mp4")
        {
            match FfmpegEncoder::new(&config.output_path, config.width, config.height, config.fps) {
                Ok(enc) => Box::new(enc),
                Err(e) => return Err((UP_ERR_ENCODE, format!("ffmpeg init failed: {}", e))),
            }
        } else {
            Box::new(crate::encode::NullEncoder::new())
        };

        guard.encoder = Some(encoder);
        Ok(UP_OK)
    }) {
        Ok(Ok(v)) => v,
        Ok(Err((code, msg))) => finish_err(ctx, code, msg),
        Err(_) => finish_err(ctx, UP_ERR_INTERNAL, "panic in up_begin_export".into()),
    }
}

fn do_render_frame(ctx: u64, elapsed_sec: f64) -> FfiResult {
    let entry = match get_handle(ctx) {
        Some(e) => e,
        None => return Err((UP_ERR_INVALID_ARG, String::new())),
    };
    let mut guard = entry.lock().unwrap();

    if let Some(cb) = guard.progress_cb {
        let timing = match guard.timing.as_ref() {
            Some(t) => t,
            None => return Err((UP_ERR_INVALID_ARG, "timing missing in render".into())),
        };
        let total = timing.total_duration_secs();
        let progress = if total > 0.0 {
            (elapsed_sec.min(total) / total * 1000.0) as i32
        } else {
            0
        };
        cb(progress);
    }

    let (note_name, lyric, note_option, is_ended, lrc_lyric) =
        compute_display(&mut guard, elapsed_sec).map_err(|e| (UP_ERR_RENDER, e))?;
    let config = guard
        .config
        .as_ref()
        .ok_or((UP_ERR_RENDER, "config missing in render".into()))?;
    let fonts = guard
        .fonts
        .as_ref()
        .ok_or((UP_ERR_RENDER, "fonts missing in render".into()))?;

    // 先渲染到 pixmap(此时只借用 config/fonts 不可变),再取 encoder 可变借用推送。
    // 提前提取 w/h 标量,让 `config` 的借用止于 render_frame,避免与 encoder 可变借用冲突。
    let cfg_w = config.width;
    let cfg_h = config.height;
    let pixmap = if guard.encoder.is_some() {
        let Some(mut pixmap) = tiny_skia::Pixmap::new(cfg_w, cfg_h) else {
            return Err((UP_ERR_RENDER, "failed to allocate pixmap".into()));
        };
        render::render_frame(
            &mut pixmap,
            config,
            fonts,
            &render::DisplayState {
                note_name: &note_name,
                lyric: &lyric,
                is_ended,
                play_time: elapsed_sec,
                current_note: note_option.as_ref(),
                lrc_lyric: &lrc_lyric,
            },
        )
        .map_err(|e| (UP_ERR_RENDER, format!("render failed: {}", e)))?;
        Some(pixmap)
    } else {
        None
    };

    if let (Some(pixmap), Some(ref mut enc)) = (pixmap, guard.encoder.as_mut()) {
        let data = pixmap.data();
        enc.push_frame(data, cfg_w, cfg_h)
            .map_err(|e| (UP_ERR_RENDER, format!("encode push failed: {}", e)))?;
    }

    Ok(UP_OK)
}

/// 渲染并送入一帧。`elapsed_sec` 为当前播放时间（秒）。
#[no_mangle]
pub unsafe extern "C" fn up_render_frame(ctx: u64, elapsed_sec: f64) -> i32 {
    match catch_unwind(|| do_render_frame(ctx, elapsed_sec)) {
        Ok(Ok(v)) => v,
        Ok(Err((code, msg))) => finish_err(ctx, code, msg),
        Err(_) => finish_err(ctx, UP_ERR_INTERNAL, "panic in up_render_frame".into()),
    }
}

fn compute_display(
    entry: &mut ContextInner,
    elapsed_sec: f64,
) -> Result<(String, String, Option<NoteInfo>, bool, String), String> {
    let timing = entry.timing.as_ref().ok_or("timing missing")?;
    let config = entry.config.as_ref().ok_or("config missing")?;

    let current_tick = elapsed_sec * timing.tick_per_second();

    // LRC 歌词按时间单调推进
    let lrc_lyric = entry
        .lrc
        .as_ref()
        .and_then(|lrc| lrc.current_line(elapsed_sec))
        .unwrap_or("")
        .to_string();

    if current_tick >= timing.total_tick() as f64 {
        let lyric =
            text::end_text(&config.style.end_display, &config.style.end_custom_text).to_string();
        return Ok((String::new(), lyric, None, true, lrc_lyric));
    }

    let current = timing.current_note(elapsed_sec, &mut entry.hint);
    let fallback = entry.last_valid_lyric.clone();
    if let Some(n) = current.note {
        // 对齐 Python _process_note 规则：
        // - "R" 休止符：不显示音名
        // - "-" 延音和普通歌词：显示音名
        let (note_name, lyric) = match n.lyric.as_str() {
            "R" => {
                let silent = text::silent_text(
                    &config.style.silent_display,
                    &config.style.silent_custom_text,
                )
                .to_string();
                (String::new(), silent)
            }
            "-" => {
                let raw_name = text::midi_to_name(n.note_num);
                let note_name = text::apply_pitch_placeholder(
                    &raw_name,
                    &config.style.pitch_placeholder,
                    &config.style.pitch_custom_text,
                );
                let lyric = fallback.unwrap_or_else(|| {
                    text::silent_text(
                        &config.style.silent_display,
                        &config.style.silent_custom_text,
                    )
                    .to_string()
                });
                (note_name, lyric)
            }
            other => {
                let raw_name = text::midi_to_name(n.note_num);
                let note_name = text::apply_pitch_placeholder(
                    &raw_name,
                    &config.style.pitch_placeholder,
                    &config.style.pitch_custom_text,
                );
                entry.last_valid_lyric = Some(other.to_string());
                (note_name, other.to_string())
            }
        };
        Ok((note_name, lyric, Some(n), false, lrc_lyric))
    } else {
        Ok((String::new(), String::new(), None, false, lrc_lyric))
    }
}

/// 结束导出：flush、写尾部、关闭文件。
#[no_mangle]
pub unsafe extern "C" fn up_end_export(ctx: u64) -> i32 {
    match catch_unwind(|| -> FfiResult {
        let entry = match get_handle(ctx) {
            Some(e) => e,
            None => return Err((UP_ERR_INVALID_ARG, String::new())),
        };
        let mut guard = entry.lock().unwrap();

        if let Some(ref mut enc) = guard.encoder {
            if let Err(e) = enc.finish() {
                return Err((UP_ERR_ENCODE, format!("encode finish failed: {}", e)));
            }
        }
        guard.encoder = None;
        guard.progress_cb = None;
        Ok(UP_OK)
    }) {
        Ok(Ok(v)) => v,
        Ok(Err((code, msg))) => finish_err(ctx, code, msg),
        Err(_) => finish_err(ctx, UP_ERR_INTERNAL, "panic in up_end_export".into()),
    }
}

fn do_render_to_buffer(ctx: u64, elapsed_sec: f64, buf: *mut c_uchar, buf_len: i32) -> FfiResult {
    let (w, h) = {
        let entry = match get_handle(ctx) {
            Some(e) => e,
            None => return Err((UP_ERR_INVALID_ARG, String::new())),
        };
        let guard = entry.lock().unwrap();
        (
            guard.config.as_ref().map(|c| c.width).unwrap_or(0),
            guard.config.as_ref().map(|c| c.height).unwrap_or(0),
        )
    };

    if w == 0 || h == 0 {
        return Err((UP_ERR_INVALID_ARG, "config not set or zero size".into()));
    }

    let bytes_needed = (w * h * 4) as i32;
    if buf_len < bytes_needed {
        return Err((UP_ERR_INVALID_ARG, "buffer too small".into()));
    }

    let entry = match get_handle(ctx) {
        Some(e) => e,
        None => return Err((UP_ERR_INVALID_ARG, String::new())),
    };
    let mut guard = entry.lock().unwrap();

    // 与 do_render_frame 共用同一套显示状态计算,保证两入口输出一致
    let (note_name, lyric, note_option, is_ended, lrc_lyric) =
        compute_display(&mut guard, elapsed_sec).map_err(|e| (UP_ERR_RENDER, e))?;

    let config = guard
        .config
        .as_ref()
        .ok_or((UP_ERR_RENDER, "config missing".into()))?;
    let fonts = guard
        .fonts
        .as_ref()
        .ok_or((UP_ERR_RENDER, "fonts missing".into()))?;

    let Some(mut pixmap) = tiny_skia::Pixmap::new(config.width, config.height) else {
        return Err((UP_ERR_RENDER, "failed to allocate pixmap".into()));
    };
    render::render_frame(
        &mut pixmap,
        config,
        fonts,
        &render::DisplayState {
            note_name: &note_name,
            lyric: &lyric,
            is_ended,
            play_time: elapsed_sec,
            current_note: note_option.as_ref(),
            lrc_lyric: &lrc_lyric,
        },
    )
    .map_err(|e| (UP_ERR_RENDER, format!("render failed: {}", e)))?;

    let slice = unsafe { std::slice::from_raw_parts_mut(buf, bytes_needed as usize) };
    slice.copy_from_slice(pixmap.data());
    Ok(UP_OK)
}

/// 渲染单帧到宿主提供的 RGBA8888 缓冲。
#[no_mangle]
pub unsafe extern "C" fn up_render_to_buffer(
    ctx: u64,
    elapsed_sec: f64,
    buf: *mut c_uchar,
    buf_len: i32,
    out_w: *mut i32,
    out_h: *mut i32,
) -> i32 {
    match catch_unwind(|| -> FfiResult {
        if buf.is_null() || out_w.is_null() || out_h.is_null() || buf_len <= 0 {
            return Err((UP_ERR_INVALID_ARG, String::new()));
        }

        let (w, h) = {
            let entry = match get_handle(ctx) {
                Some(e) => e,
                None => return Err((UP_ERR_INVALID_ARG, String::new())),
            };
            let guard = entry.lock().unwrap();
            (
                guard.config.as_ref().map(|c| c.width).unwrap_or(0),
                guard.config.as_ref().map(|c| c.height).unwrap_or(0),
            )
        };

        *out_w = w as i32;
        *out_h = h as i32;

        do_render_to_buffer(ctx, elapsed_sec, buf, buf_len)
    }) {
        Ok(Ok(v)) => v,
        Ok(Err((code, msg))) => finish_err(ctx, code, msg),
        Err(_) => finish_err(ctx, UP_ERR_INTERNAL, "panic in up_render_to_buffer".into()),
    }
}

/// 设置编码进度回调（千分比 0~1000）。
#[no_mangle]
pub unsafe extern "C" fn up_set_progress_callback(ctx: u64, cb: ProgressFn) {
    let _ = catch_unwind(|| {
        if let Some(entry) = get_handle(ctx) {
            entry.lock().unwrap().progress_cb = Some(cb);
        }
    });
}

/// 取最近一次错误消息（UTF-8 静态缓冲）。返回字符串指针，宿主不可 free；
/// 下次调用任何 up_* 前内容仍有效（同一 ctx 串行调用前提下）。
#[no_mangle]
pub unsafe extern "C" fn up_last_error(ctx: u64) -> *const c_char {
    match catch_unwind(|| -> Result<*const c_char, String> {
        let entry = match get_handle(ctx) {
            Some(e) => e,
            None => return Ok(std::ptr::null()),
        };
        let guard = entry.lock().unwrap();
        Ok(guard
            .last_error
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null()))
    }) {
        Ok(Ok(p)) => p,
        Ok(Err(_)) | Err(_) => std::ptr::null(),
    }
}
