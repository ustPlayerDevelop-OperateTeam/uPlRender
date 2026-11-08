> [!CAUTION]
>
> 由于部分原因，此仓库将存档。

<div align="center">

# uPlRender

ustPlayer的视频渲染模块，可通过外部程序调用。

</div>

> [!WARNING]
> 请注意：本仓库只有uPl的渲染模块，若想使用主程序，请看[ustPlayer](https://github.com/SYEternalR/ustPlayer)
>
> 此项目（uPlRender）还处于开发阶段，若发现了bug请及时汇报/提交PR来指正！
<!---
## 简介

本仓库是一个 Rust 动态库（`cdylib`，Windows 下为 `ustplayer_renderer.dll`），
将 ustPlayer 播放器（PySide6 `QPainter`）的渲染逻辑用 Rust 重写：输入 UST 数据 + 样式配置（JSON），
输出与播放器画面一致的视频文件（MP4）或单帧 RGBA 缓冲。宿主程序（Python `ctypes`、C# P/Invoke
或任意支持 C ABI 的语言）通过导出的 `up_*` 函数加载调用，渲染全部在 Rust 侧完成，**不依赖 Qt**。

渲染使用 `tiny-skia`（CPU 光栅化），字体用 `fontdue`（等线 / 微软雅黑 / Segoe UI 回退链），
视频编码经系统 `ffmpeg` 可执行文件（libx264，RGBA rawvideo → yuv420p）。

## 构建

```bash
cargo build --release            # 64 位,产物 target/release/ustplayer_renderer.dll (+ .lib)
cargo test                       # 运行单元与集成测试
cargo clippy --all-targets       # 静态检查
```

> 视频导出需要 `ffmpeg` 在 PATH 中；未找到时 `up_begin_export` 返回 `UP_ERR_ENCODE`。
> 若 `output_path` 不以 `.mp4` 结尾，将使用空编码器（仅验证流程，不产出文件）。

## C ABI 接口

完整声明见 [`bindings/ustplayer_renderer.h`](bindings/ustplayer_renderer.h)。

### 错误码

| 常量 | 值 | 含义 |
|------|----|------|
| `UP_OK` | 0 | 成功 |
| `UP_ERR_INVALID_ARG` | -1 | 空指针 / 非法枚举 / 长度越界 |
| `UP_ERR_PARSE` | -2 | UST / LRC / JSON 解析失败 |
| `UP_ERR_IO` | -3 | 文件读写失败 |
| `UP_ERR_FONT` | -4 | 字体加载失败 |
| `UP_ERR_RENDER` | -5 | 渲染阶段失败 |
| `UP_ERR_ENCODE` | -6 | 编码器初始化 / 写入失败 |
| `UP_ERR_INTERNAL` | -99 | 未分类内部错误（已捕获的 panic） |

### 导出函数

| 函数 | 说明 |
|------|------|
| `uint64_t up_create_context(void)` | 创建渲染上下文（失败返回 0） |
| `void up_destroy_context(uint64_t ctx)` | 销毁上下文，释放资源 |
| `int32_t up_set_config(ctx, const char* json)` | 设置渲染配置（JSON, UTF-8） |
| `int32_t up_set_ust_text(ctx, const char* json)` | 直接传入 UST JSON，重建时序模型 |
| `int32_t up_set_lrc_text(ctx, const char* text)` | 直接传入 LRC 文本（多编码探测） |
| `int32_t up_begin_export(ctx)` | 初始化编码器 |
| `int32_t up_render_frame(ctx, double elapsed)` | 渲染并送入一帧 |
| `int32_t up_end_export(ctx)` | flush 编码器、写尾部、关闭文件 |
| `void up_set_progress_callback(ctx, cb)` | 设置进度回调（千分比 0~1000） |
| `int32_t up_render_to_buffer(ctx, elapsed, buf, len, &w, &h)` | 渲染单帧到宿主缓冲（RGBA8888 premultiplied） |
| `const char* up_last_error(ctx)` | 最近一次错误消息（UTF-8） |

### 内存与线程约定

- **分配归属**：除 `up_render_to_buffer` 的 `buf` 由宿主分配外，其余字符串/句柄一律由库分配、库释放；
  宿主**不得** `free` 任何库返回的指针。
- **线程安全**：不同 `ctx` 之间可并发调用（内部各自独立）；同一 `ctx` 的所有调用必须串行。
- **UTF-8 唯一**：所有跨 FFI 字符串统一 UTF-8 编码。
- **零 panic**：所有导出函数内部以 `catch_unwind` 包裹，非法参数一律返回负错误码，绝不崩溃。

### JSON 配置

`up_set_config` 的 JSON 结构（字段缺失用默认值填充，未知字段忽略）：

```json
{
  "ust": {
    "version": "", "tempo": 120.0, "tracks": 1,
    "notes": [{"index": "0000", "length": 480, "lyric": "あ", "note_num": 69,
               "phoneme": "a", "pitch_bend": []}]
  },
  "show": {"bpm": true, "play_time": true, "song_name": true, "song_author": true,
           "ust_author": true, "lyric": true, "curve_show": false},
  "project": {"project_name": "", "song_name": "", "song_author": "", "ust_author": ""},
  "style": {
    "bg_color": "#000000", "note_color": "#6c6c6c", "lyric_color": "#FFFFFF",
    "lyric_text_color": "#FFFFFF", "other_text_color": "#FFFFFF",
    "lyric_pos": "top", "fullscreen": true,
    "lrc_path": "", "music_path": "",
    "silent_display": "r", "silent_custom_text": "",
    "end_display": "end", "end_custom_text": "",
    "pitch_placeholder": "none", "pitch_custom_text": "",
    "pitch_curve_color": "#FFFFFF", "app_version": "1.1.0 Beta 1"
  },
  "width": 1920, "height": 1080, "fps": 60, "output_path": "out.mp4"
}
```

## Python ctypes 示例

```python
import ctypes
from bindings.ustplayer_renderer import (  # 或直接 import ustplayer_renderer
    up_create_context, up_set_config, up_set_ust_text, up_set_lrc_text,
    up_begin_export, up_render_frame, up_end_export, up_destroy_context,
)

ctx = up_create_context()
assert ctx != 0

config = {
    "ust": {"tempo": 120.0, "notes": [{"length": 480, "lyric": "あ", "note_num": 69}]},
    "show": {"bpm": True, "play_time": True, "lyric": True},
    "project": {"song_name": "测试曲", "song_author": "作者", "ust_author": "调音师"},
    "style": {"bg_color": "#000000", "lrc_path": "song.lrc"},
    "width": 1920, "height": 1080, "fps": 60,
    "output_path": "out.mp4",
}
assert up_set_config(ctx, json.dumps(config, ensure_ascii=False)) == 0

# 直接注入文本(可选,避免二次读盘)
up_set_ust_text(ctx, json.dumps(config["ust"]))
up_set_lrc_text(ctx, "[00:05.00]第一行\n[00:10.00]第二行")

fps = 60
total_sec = 30.0   # = total_tick / tick_per_second,由时序模型得出
assert up_begin_export(ctx) == 0
for i in range(int(total_sec * fps) + fps):   # 末尾多渲染约 1s 结束画面
    rc = up_render_frame(ctx, i / fps)
    assert rc == 0
assert up_end_export(ctx) == 0
up_destroy_context(ctx)
```

### 中断与结束帧驱动

- 帧完全由宿主驱动：`elapsed = frame_index / fps`。**Ctrl+C 中断**只需停止调用
  `up_render_frame` 并调用 `up_end_export` 即可产出已完成帧的文件。
- **结束画面**：规格要求视频总时长 ≈ `total_tick / tick_per_second` + 1s 结束画面。
  宿主在 `current_tick >= total_tick` 后继续渲染约 `fps` 帧（此时库会输出 `end_display` 文字）。

## 目录结构

```
├── Cargo.toml            # [lib] crate-type = ["cdylib", "rlib"]; path = "lib/lib.rs"
├── lib/                  # 动态库源码(src 改名 lib)
│   ├── lib.rs            # cdylib 库入口
│   ├── ffi.rs            # C ABI 层:句柄管理、错误码、panic 捕获、并发句柄表
│   ├── model.rs          # 数据模型与 JSON 反序列化(serde)
│   ├── timing.rs         # 时序模型(tick 换算/音符查找/结束判定)
│   ├── text.rs           # 音名/MIDI/时间格式/占位符/静默与结束文字规则
│   ├── lrc.rs            # LRC 解析(多编码探测:utf-8-sig→utf-8→gbk→gb2312→shift-jis)
│   ├── fonts.rs          # 字体加载与度量(常规/粗体双链,回退)
│   ├── render.rs         # 渲染器:逐元素绘制
│   └── encode.rs         # 帧送入编码器(ffmpeg),MP4 输出
├── bindings/
│   ├── ustplayer_renderer.h    # C ABI 头文件
│   ├── ustplayer_renderer.py   # Python ctypes 封装
│   └── smoke_test_ctypes.py    # ctypes 冒烟测试
├── tools/                # C++ 调试工具(debug_renderer + build_debug.bat)
├── examples/             # cli.rs / dump_frame.rs / verify_frame.rs
└── tests/
    └── c_api_test.rs     # 集成测试:FFI 冒烟/鲁棒性/两入口一致性
```
--->
## 许可证

项目使用`GPL v3.0`许可证开源。详见`LICENSE`。
