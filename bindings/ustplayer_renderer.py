"""ustPlayer Renderer Python ctypes 调用封装。"""

import ctypes
from ctypes import c_uint64, c_int32, c_double, c_uint8, c_void_p, POINTER, byref

_lib = None

def _load():
    global _lib
    if _lib is not None:
        return
    candidates = [
        "./target/release/ustplayer_renderer.dll",
        "ustplayer_renderer.dll",
    ]
    for path in candidates:
        try:
            _lib = ctypes.CDLL(path)
            return
        except OSError:
            continue
    raise RuntimeError("ustplayer_renderer.dll 未加载，请先 `cargo build --release`")

def up_create_context() -> int:
    _load()
    _lib.up_create_context.restype = c_uint64
    return _lib.up_create_context()

def up_destroy_context(ctx: int):
    _load()
    _lib.up_destroy_context.argtypes = [c_uint64]
    _lib.up_destroy_context(ctx)

def up_set_config(ctx: int, json_str: str) -> int:
    _load()
    _lib.up_set_config.restype = c_int32
    _lib.up_set_config.argtypes = [c_uint64, ctypes.c_char_p]
    return _lib.up_set_config(ctx, json_str.encode("utf-8"))

def up_set_ust_text(ctx: int, ust_json: str) -> int:
    """直接传入 UST JSON(不读文件),替换 config.ust 并重建时序模型。"""
    _load()
    _lib.up_set_ust_text.restype = c_int32
    _lib.up_set_ust_text.argtypes = [c_uint64, ctypes.c_char_p]
    return _lib.up_set_ust_text(ctx, ust_json.encode("utf-8"))

def up_set_lrc_text(ctx: int, lrc_text: str) -> int:
    """直接传入 LRC 文本(内部多编码探测),替换 lrc_path 读取结果。"""
    _load()
    _lib.up_set_lrc_text.restype = c_int32
    _lib.up_set_lrc_text.argtypes = [c_uint64, ctypes.c_char_p]
    return _lib.up_set_lrc_text(ctx, lrc_text.encode("utf-8"))

def up_begin_export(ctx: int) -> int:
    _load()
    _lib.up_begin_export.restype = c_int32
    _lib.up_begin_export.argtypes = [c_uint64]
    return _lib.up_begin_export(ctx)

def up_render_frame(ctx: int, elapsed_sec: float) -> int:
    _load()
    _lib.up_render_frame.restype = c_int32
    _lib.up_render_frame.argtypes = [c_uint64, c_double]
    return _lib.up_render_frame(ctx, elapsed_sec)

def up_end_export(ctx: int) -> int:
    _load()
    _lib.up_end_export.restype = c_int32
    _lib.up_end_export.argtypes = [c_uint64]
    return _lib.up_end_export(ctx)

def up_render_to_buffer(ctx: int, elapsed_sec: float, buf: bytearray):
    _load()
    _lib.up_render_to_buffer.restype = c_int32
    _lib.up_render_to_buffer.argtypes = [
        c_uint64,
        c_double,
        POINTER(c_uint8),
        c_int32,
        POINTER(c_int32),
        POINTER(c_int32),
    ]
    out_w = c_int32(0)
    out_h = c_int32(0)
    ptr = (c_uint8 * len(buf)).from_buffer(buf)
    rc = _lib.up_render_to_buffer(ctx, elapsed_sec, ptr, len(buf), byref(out_w), byref(out_h))
    return rc, out_w.value, out_h.value

def up_set_progress_callback(ctx: int, callback):
    """设置进度回调，返回回调对象引用以延长生命周期。"""
    _load()
    _lib.up_set_progress_callback.argtypes = [c_uint64, ctypes.CFUNCTYPE(None, c_int32)]
    cb = ctypes.CFUNCTYPE(None, c_int32)(callback)
    _lib.up_set_progress_callback(ctx, cb)
    return cb

def up_last_error(ctx: int) -> str:
    _load()
    _lib.up_last_error.restype = ctypes.c_char_p
    _lib.up_last_error.argtypes = [c_uint64]
    ptr = _lib.up_last_error(ctx)
    if ptr is None or ptr == 0:
        return ""
    return ptr.decode("utf-8")