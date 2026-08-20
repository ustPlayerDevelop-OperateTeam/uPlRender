# ustPlayer Renderer 配置 TUI

一个 OpenCode 风格的终端配置编辑器,用于浏览/编辑渲染配置
(UST 数据、样式、显示开关、项目信息、输出设置),并可生成导出说明。

## 安装(uv 管理)

需要 **uv**(https://docs.astral.sh/uv/)+ Python 3.10+。

```bash
cd tools/tui
uv sync            # 创建 .venv 并按 uv.lock 安装依赖(含 dev 组)
```

> `uv sync` 会自动下载并使用合适的 Python 版本(见 `pyproject.toml` 的
> `requires-python = ">=3.10"`)。依赖锁定在 `uv.lock`(当前 PyPI 清华镜像)。

## 运行

```bash
uv run python main.py                       # 默认加载 ../../examples/frame_config.json
uv run python main.py my_config.json        # 指定配置文件(不存在则从默认配置开始)
```

## 测试

```bash
uv run pytest tests/ -v
```

## 界面

```
┌ ⬡ ustPlayer Renderer TUI ──────────────────────────────┐
├─────────────────┬──────────────────────────────────────┤
│ 配置             │  🎵 UST 数据                          │
│ ├ 🎵 UST 数据    │  BPM (tempo)   [120.0    ]           │
│ ├ 🎨 样式        │  音轨数        [1        ]           │
│ ├ 👁 显示开关     │  音符数: 1                           │
│ ├ 🏷 项目信息     │  [➕ 添加音符] [🗑 删除最后]            │
│ └ 📤 输出设置     │  #0 A MIDI 69 len 480               │
│                  │  歌词 [A   ] 音高 [69 ]              │
│                  │  长度 [480] 音素 [a  ]              │
│                  │  音高弯曲 [0 5 -3      ]             │
├─────────────────┴──────────────────────────────────────┤
│ q 退出  s 保存  S 另存为  e 导出  / 导航  Tab 下一个     │
└─────────────────────────────────────────────────────────┘
```

## 快捷键

| 键 | 功能 |
|----|------|
| `q` | 退出(有未保存修改时弹确认) |
| `s` | 保存到当前文件(自动校验,错误会提示) |
| `S` | 另存为(输入新路径) |
| `e` | 导出:弹窗展示两种导出方式 |
| `/` | 聚焦左侧导航树 |
| `Tab` / `Shift+Tab` | 循环焦点 |
| 鼠标 | 点击树节点切换编辑区、点击输入框编辑、点击按钮 |

## 编辑区

| 导航项 | 内容 |
|--------|------|
| 🎵 UST 数据 | BPM、音轨数、音符列表(歌词/音高/长度/音素/音高弯曲),可增删音符 |
| 🎨 样式 | 颜色字段(校验 #RRGGBB)、枚举选项(歌词位置/静默/结束/音高占位)、自定义文本 |
| 👁 显示开关 | BPM/时间/曲名/作者/调音师/歌词/音高线 复选框 |
| 🏷 项目信息 | 工程名/曲名/MIDI 作者/调音师 |
| 📤 输出设置 | 分辨率/fps/输出路径 |

## 保存与导出

- **保存**:校验(颜色格式、枚举值、数值范围)通过后写入 JSON(UTF-8,保留中文)。
- **导出**:按 `e` 后弹窗展示两种方式:
  1. Rust 示例 `cargo run --release --example cli`(需先 `cargo build --release`)
  2. 生成 `export_script.py`(ctypes 加载 `target/release/ustplayer_renderer.dll`,走
     `up_begin_export → up_render_frame×N → up_end_export` 完整流程),可一键写入当前目录运行。

## 测试

```bash
uv run pytest tests/ -v
```

测试覆盖:配置加载(默认值/完整/往返/缺失文件/BOM)、校验(颜色/枚举/数值)、
导出脚本生成、音符解析、枚举常量。
