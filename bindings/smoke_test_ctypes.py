"""ctypes 冒烟测试:加载 DLL → 完整流程 → 非法参数鲁棒性。"""
import ctypes
import json
import sys

from ctypes import c_uint64, c_int32, c_double, c_uint8, POINTER, byref

dll_path = sys.argv[1] if len(sys.argv) > 1 else "./target/release/ustplayer_renderer.dll"
lib = ctypes.CDLL(dll_path)

lib.up_create_context.restype = c_uint64
lib.up_destroy_context.argtypes = [c_uint64]
lib.up_set_config.restype = c_int32
lib.up_set_config.argtypes = [c_uint64, ctypes.c_char_p]
lib.up_set_ust_text.restype = c_int32
lib.up_set_ust_text.argtypes = [c_uint64, ctypes.c_char_p]
lib.up_set_lrc_text.restype = c_int32
lib.up_set_lrc_text.argtypes = [c_uint64, ctypes.c_char_p]
lib.up_begin_export.restype = c_int32
lib.up_begin_export.argtypes = [c_uint64]
lib.up_render_frame.restype = c_int32
lib.up_render_frame.argtypes = [c_uint64, c_double]
lib.up_end_export.restype = c_int32
lib.up_end_export.argtypes = [c_uint64]
lib.up_render_to_buffer.restype = c_int32
lib.up_render_to_buffer.argtypes = [c_uint64, c_double, POINTER(c_uint8), c_int32, POINTER(c_int32), POINTER(c_int32)]
lib.up_last_error.restype = ctypes.c_char_p
lib.up_last_error.argtypes = [c_uint64]

print("=== 1. create context ===")
ctx = lib.up_create_context()
assert ctx != 0, "create failed"
print(f"ctx = {ctx}")

print("=== 2. set_config(正常 JSON)===")
config = {
    "ust": {"tempo": 120.0, "notes": [{"length": 480, "lyric": "A", "note_num": 69}]},
    "show": {"bpm": True, "play_time": True, "lyric": True},
    "project": {"song_name": "Test", "song_author": "Author", "ust_author": "Tuner"},
    "style": {"bg_color": "#000000", "lyric_pos": "top"},
    "width": 192, "height": 108, "fps": 60,
    "output_path": "",
}
rc = lib.up_set_config(ctx, json.dumps(config).encode("utf-8"))
assert rc == 0, f"set_config failed: {rc} {lib.up_last_error(ctx)}"
print("OK")

print("=== 3. up_set_ust_text ===")
rc = lib.up_set_ust_text(ctx, json.dumps(config["ust"]).encode("utf-8"))
assert rc == 0, f"set_ust_text failed: {rc} {lib.up_last_error(ctx)}"
print("OK")

print("=== 4. up_set_lrc_text ===")
rc = lib.up_set_lrc_text(ctx, "[00:01.00]第一行\n[00:10.00]第二行".encode("utf-8"))
assert rc == 0, f"set_lrc_text failed: {rc} {lib.up_last_error(ctx)}"
print("OK")

print("=== 5. render_to_buffer(预览帧)===")
buf = (c_uint8 * (192 * 108 * 4))()
w = c_int32(0)
h = c_int32(0)
rc = lib.up_render_to_buffer(ctx, 1.0, buf, len(buf), byref(w), byref(h))
assert rc == 0, f"render_to_buffer failed: {rc} {lib.up_last_error(ctx)}"
assert (w.value, h.value) == (192, 108)
nonblack = sum(1 for i in range(0, len(buf), 4) if buf[i] or buf[i + 1] or buf[i + 2])
print(f"OK: {w.value}x{h.value}, non-black pixels = {nonblack}")

print("=== 6. begin/render/end 导出流程(空编码器)===")
rc = lib.up_begin_export(ctx)
assert rc == 0, f"begin_export failed: {rc} {lib.up_last_error(ctx)}"
for i in range(30):
    rc = lib.up_render_frame(ctx, i / 30.0)
    assert rc == 0, f"render_frame[{i}] failed: {rc} {lib.up_last_error(ctx)}"
rc = lib.up_end_export(ctx)
assert rc == 0, f"end_export failed: {rc} {lib.up_last_error(ctx)}"
print("OK")

print("=== 7. 鲁棒性:非法参数 ===")
# 非法 JSON
rc = lib.up_set_config(ctx, b"{not json")
assert rc == c_int32(-2).value, f"bad json should be -2, got {rc}"
# 空指针
rc = lib.up_set_config(ctx, None)
assert rc == c_int32(-1).value, f"null config should be -1, got {rc}"
rc = lib.up_set_ust_text(ctx, None)
assert rc == c_int32(-1).value, f"null ust should be -1, got {rc}"
rc = lib.up_set_lrc_text(ctx, None)
assert rc == c_int32(-1).value, f"null lrc should be -1, got {rc}"
# 无效 ctx
rc = lib.up_render_frame(999999, 0.0)
assert rc == c_int32(-1).value, f"invalid ctx should be -1, got {rc}"
# 缓冲过小
small = (c_uint8 * 4)()
rc = lib.up_render_to_buffer(ctx, 0.0, small, 4, byref(w), byref(h))
assert rc == c_int32(-1).value, f"small buf should be -1, got {rc}"
# 错误消息可查询
err = lib.up_last_error(ctx)
print(f"last_error = {err.decode('utf-8') if err else ''}")
print("OK")

print("=== 8. destroy ===")
lib.up_destroy_context(ctx)
print("OK")

print("\nALL SMOKE TESTS PASSED")
