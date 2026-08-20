"""RenderConfig 配置模型:加载 / 校验 / 保存 / 导出命令生成。

与 Rust 端 lib/model.rs 的 RenderConfig 结构对应(serde default 语义)。
纯 Python 逻辑,不依赖 textual,可独立单测。
"""

from __future__ import annotations

import json
import shlex
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Optional

# 合法颜色:#RRGGBB(忽略大小写)
HEX_RE = None  # lazy import 避免顶层依赖


def _valid_hex(color: str) -> bool:
    if not isinstance(color, str):
        return False
    c = color.strip().lstrip("#")
    if len(c) != 6:
        return False
    try:
        int(c, 16)
        return True
    except ValueError:
        return False


# ---------- 显示开关 ----------

@dataclass
class ShowConfig:
    bpm: bool = True
    play_time: bool = True
    song_name: bool = True
    song_author: bool = True
    ust_author: bool = True
    lyric: bool = True
    curve_show: bool = False

    FIELDS = [
        ("bpm", "显示 BPM"),
        ("play_time", "显示播放时间"),
        ("song_name", "显示曲名"),
        ("song_author", "显示 MIDI 作者"),
        ("ust_author", "显示调音师"),
        ("lyric", "显示 LRC 歌词"),
        ("curve_show", "显示音高线"),
    ]

    def to_dict(self) -> dict:
        return {k: getattr(self, k) for k, _ in self.FIELDS}

    @classmethod
    def from_dict(cls, d: dict) -> "ShowConfig":
        return cls(**{k: bool(d[k]) for k, _ in cls.FIELDS if k in d})


# ---------- 项目信息 ----------

@dataclass
class ProjectInfo:
    project_name: str = ""
    song_name: str = ""
    song_author: str = ""
    ust_author: str = ""

    FIELDS = [
        ("project_name", "工程名"),
        ("song_name", "曲名"),
        ("song_author", "MIDI 作者"),
        ("ust_author", "调音师"),
    ]

    def to_dict(self) -> dict:
        return {k: getattr(self, k) for k, _ in self.FIELDS}

    @classmethod
    def from_dict(cls, d: dict) -> "ProjectInfo":
        return cls(**{k: str(d.get(k, "")) for k, _ in cls.FIELDS})


# ---------- 单个音符 ----------

@dataclass
class NoteInfo:
    index: str = ""
    length: int = 480
    lyric: str = ""
    note_num: int = 60
    phoneme: str = ""
    pitch_bend: list = field(default_factory=list)

    FIELDS = [
        ("index", "段编号", str),
        ("length", "长度(tick)", int),
        ("lyric", "歌词", str),
        ("note_num", "MIDI 音高", int),
        ("phoneme", "音素", str),
        ("pitch_bend", "音高弯曲", "pitch_bend"),
    ]

    def to_dict(self) -> dict:
        return {
            "index": self.index,
            "length": self.length,
            "lyric": self.lyric,
            "note_num": self.note_num,
            "phoneme": self.phoneme,
            "pitch_bend": list(self.pitch_bend),
        }

    @classmethod
    def from_dict(cls, d: dict) -> "NoteInfo":
        n = cls()
        if "index" in d:
            n.index = str(d["index"])
        if "length" in d:
            n.length = int(d.get("length", 480))
        if "lyric" in d:
            n.lyric = str(d["lyric"])
        if "note_num" in d:
            n.note_num = int(d.get("note_num", 60))
        if "phoneme" in d:
            n.phoneme = str(d["phoneme"])
        if "pitch_bend" in d:
            pb = d["pitch_bend"]
            if isinstance(pb, list):
                out = []
                for x in pb:
                    try:
                        out.append(int(x))
                    except (ValueError, TypeError):
                        pass  # 非法元素跳过,保持容错
                n.pitch_bend = out
            else:
                n.pitch_bend = []
        return n


# ---------- UST 数据 ----------

@dataclass
class UstInfo:
    version: str = ""
    tempo: float = 120.0
    tracks: int = 1
    notes: list = field(default_factory=list)  # list[NoteInfo]

    def to_dict(self) -> dict:
        return {
            "version": self.version,
            "tempo": self.tempo,
            "tracks": self.tracks,
            "notes": [n.to_dict() for n in self.notes],
        }

    @classmethod
    def from_dict(cls, d: dict) -> "UstInfo":
        u = cls()
        u.version = str(d.get("version", ""))
        u.tempo = float(d.get("tempo", 120.0))
        u.tracks = int(d.get("tracks", 1))
        raw_notes = d.get("notes", [])
        u.notes = [NoteInfo.from_dict(x) for x in raw_notes] if isinstance(raw_notes, list) else []
        return u


# ---------- 播放器样式 ----------

# 枚举选项
SILENT_DISPLAY_OPTIONS = ["r", "dash", "custom", "none"]
END_DISPLAY_OPTIONS = ["end", "dash", "custom", "none"]
PITCH_PLACEHOLDER_OPTIONS = ["none", "dash", "custom"]
LYRIC_POS_OPTIONS = ["top", "bottom"]


@dataclass
class PlayerStyle:
    bg_color: str = "#000000"
    note_color: str = "#6c6c6c"
    lyric_color: str = "#FFFFFF"
    lyric_text_color: str = "#FFFFFF"
    other_text_color: str = "#FFFFFF"
    lyric_pos: str = "top"
    fullscreen: bool = True
    lrc_path: str = ""
    music_path: str = ""
    silent_display: str = "r"
    silent_custom_text: str = ""
    end_display: str = "end"
    end_custom_text: str = ""
    pitch_placeholder: str = "none"
    pitch_custom_text: str = ""
    pitch_curve_color: str = "#FFFFFF"
    app_version: str = "1.1.0 Beta 1"

    COLOR_FIELDS = [
        ("bg_color", "背景色"),
        ("note_color", "音名色"),
        ("lyric_color", "歌字色"),
        ("lyric_text_color", "LRC 歌词色"),
        ("other_text_color", "其他文字色"),
        ("pitch_curve_color", "音高线色"),
    ]
    TEXT_FIELDS = [
        ("silent_custom_text", "静默自定义文本"),
        ("end_custom_text", "结束自定义文本"),
        ("pitch_custom_text", "音高占位自定义文本"),
        ("lrc_path", "LRC 文件路径"),
        ("music_path", "伴奏音频路径"),
        ("app_version", "版本号"),
    ]
    ENUM_FIELDS = [
        ("lyric_pos", "歌词位置", LYRIC_POS_OPTIONS),
        ("silent_display", "静默显示", SILENT_DISPLAY_OPTIONS),
        ("end_display", "结束显示", END_DISPLAY_OPTIONS),
        ("pitch_placeholder", "音高占位符", PITCH_PLACEHOLDER_OPTIONS),
    ]

    def to_dict(self) -> dict:
        d: dict[str, Any] = {"fullscreen": self.fullscreen}
        for k, _ in self.COLOR_FIELDS + self.TEXT_FIELDS:
            d[k] = getattr(self, k)
        for k, _, _ in self.ENUM_FIELDS:
            d[k] = getattr(self, k)
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "PlayerStyle":
        s = cls()
        if "fullscreen" in d:
            s.fullscreen = bool(d["fullscreen"])
        for k, _ in cls.COLOR_FIELDS + cls.TEXT_FIELDS:
            if k in d:
                setattr(s, k, str(d[k]))
        for k, _, _ in cls.ENUM_FIELDS:
            if k in d:
                setattr(s, k, str(d[k]))
        return s

    def validate(self) -> list[str]:
        """返回错误消息列表(空 = 合法)。"""
        errs = []
        for k, label in self.COLOR_FIELDS:
            v = getattr(self, k)
            if not _valid_hex(v):
                errs.append(f"{label}({k}) 不是合法 #RRGGBB: {v!r}")
        for k, _, opts in self.ENUM_FIELDS:
            v = getattr(self, k)
            if v not in opts:
                errs.append(f"{k} 必须是 {opts} 之一,当前 {v!r}")
        return errs


# ---------- 完整渲染配置 ----------

@dataclass
class RenderConfig:
    ust: UstInfo = field(default_factory=UstInfo)
    show: ShowConfig = field(default_factory=ShowConfig)
    project: ProjectInfo = field(default_factory=ProjectInfo)
    style: PlayerStyle = field(default_factory=PlayerStyle)
    width: int = 1920
    height: int = 1080
    fps: int = 60
    output_path: str = ""

    def to_dict(self) -> dict:
        return {
            "ust": self.ust.to_dict(),
            "show": self.show.to_dict(),
            "project": self.project.to_dict(),
            "style": self.style.to_dict(),
            "width": self.width,
            "height": self.height,
            "fps": self.fps,
            "output_path": self.output_path,
        }

    @classmethod
    def from_dict(cls, d: dict) -> "RenderConfig":
        c = cls()
        if "ust" in d:
            c.ust = UstInfo.from_dict(d["ust"])
        if "show" in d:
            c.show = ShowConfig.from_dict(d["show"])
        if "project" in d:
            c.project = ProjectInfo.from_dict(d["project"])
        if "style" in d:
            c.style = PlayerStyle.from_dict(d["style"])
        c.width = int(d.get("width", 1920))
        c.height = int(d.get("height", 1080))
        c.fps = int(d.get("fps", 60))
        c.output_path = str(d.get("output_path", ""))
        return c

    @classmethod
    def load(cls, path: str | Path) -> "RenderConfig":
        """从 JSON 文件加载,缺字段用默认值。文件不存在时返回默认配置。"""
        p = Path(path)
        if not p.exists():
            return cls()
        with open(p, "r", encoding="utf-8-sig") as f:
            return cls.from_dict(json.load(f))

    def save(self, path: str | Path) -> None:
        """保存为 JSON(ensure_ascii=False 保留中文)。"""
        p = Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        with open(p, "w", encoding="utf-8") as f:
            json.dump(self.to_dict(), f, ensure_ascii=False, indent=2)

    def validate(self) -> list[str]:
        """整配置校验:样式颜色/枚举 + 数值范围。返回错误列表。"""
        errs = list(self.style.validate())
        if self.width <= 0 or self.height <= 0:
            errs.append(f"分辨率必须为正: {self.width}x{self.height}")
        if not (1 <= self.fps <= 240):
            errs.append(f"fps 应在 1~240,当前 {self.fps}")
        if self.ust.tempo <= 0:
            errs.append(f"tempo(BPM) 必须为正,当前 {self.ust.tempo}")
        return errs

    def export_command(self) -> str:
        """生成外部导出命令(供 TUI 展示/复制)。

        首选 Rust 示例 cli(cli.rs 会读取 output_path 并走完整导出流程);
        若 output_path 为空则提示先配置。
        """
        parts = ["cargo", "run", "--release", "--example", "cli"]
        return shlex.join(parts)

    def summary(self) -> str:
        """一行摘要,用于状态栏。"""
        n = len(self.ust.notes)
        return (
            f"{self.width}x{self.height}@{self.fps}fps | "
            f"{n} 音符 | BPM {self.ust.tempo} | "
            f"曲名 {self.project.song_name or '-'}"
        )


def export_with_ffmpeg(config: RenderConfig, dll_path: str | Path, out_dir: str | Path) -> list[str]:
    """生成实际调用 DLL 导出视频的 Python 脚本(供 TUI 展示或一键生成)。

    返回脚本行列表;脚本用 ctypes 加载 DLL,走 begin/render/end 流程。
    """
    cfg = config.to_dict()
    total_sec = max(1.0, sum(max(n["length"], 1) for n in cfg["ust"]["notes"]) / (cfg["ust"]["tempo"] * 8.0))
    fps = cfg["fps"]
    lines = [
        "import ctypes, json, sys, time",
        f"DLL = r'{dll_path}'",
        f"OUT = r'{out_dir}'",
        "lib = ctypes.CDLL(DLL)",
        "lib.up_create_context.restype = ctypes.c_uint64",
        "lib.up_destroy_context.argtypes = [ctypes.c_uint64]",
        "lib.up_set_config.restype = ctypes.c_int32",
        "lib.up_set_config.argtypes = [ctypes.c_uint64, ctypes.c_char_p]",
        "lib.up_begin_export.restype = ctypes.c_int32",
        "lib.up_begin_export.argtypes = [ctypes.c_uint64]",
        "lib.up_render_frame.restype = ctypes.c_int32",
        "lib.up_render_frame.argtypes = [ctypes.c_uint64, ctypes.c_double]",
        "lib.up_end_export.restype = ctypes.c_int32",
        "lib.up_end_export.argtypes = [ctypes.c_uint64]",
        "lib.up_last_error.restype = ctypes.c_char_p",
        "lib.up_last_error.argtypes = [ctypes.c_uint64]",
        "import os; os.makedirs(OUT, exist_ok=True)",
        f"cfg = {json.dumps(cfg, ensure_ascii=False)}",
        "cfg['output_path'] = os.path.join(OUT, 'out.mp4')",
        "ctx = lib.up_create_context()",
        "assert ctx, 'create context failed'",
        "rc = lib.up_set_config(ctx, json.dumps(cfg, ensure_ascii=False).encode('utf-8'))",
        "if rc != 0:",
        "    print('set_config failed:', lib.up_last_error(ctx).decode()); sys.exit(1)",
        "rc = lib.up_begin_export(ctx)",
        "if rc != 0:",
        "    print('begin_export failed:', lib.up_last_error(ctx).decode()); sys.exit(1)",
        f"total = {total_sec:.3f}",
        "frames = int(total * fps)",
        "t0 = time.time()",
        "for i in range(frames):",
        "    rc = lib.up_render_frame(ctx, i / fps)",
        "    if rc != 0:",
        "        print(f'frame {i} failed:', lib.up_last_error(ctx).decode()); sys.exit(1)",
        "    if i % (fps // 2 or 1) == 0:",
        "        print(f'progress {i}/{frames}')",
        "lib.up_end_export(ctx)",
        "lib.up_destroy_context(ctx)",
        "print(f'done in {time.time()-t0:.1f}s ->', cfg['output_path'])",
    ]
    return lines
