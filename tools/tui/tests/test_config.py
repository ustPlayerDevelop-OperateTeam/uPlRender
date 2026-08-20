"""config.py 单元测试:配置模型加载/校验/保存/导出脚本。"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from config import (
    END_DISPLAY_OPTIONS,
    LYRIC_POS_OPTIONS,
    PITCH_PLACEHOLDER_OPTIONS,
    SILENT_DISPLAY_OPTIONS,
    NoteInfo,
    PlayerStyle,
    RenderConfig,
    export_with_ffmpeg,
)


def _sample_dict() -> dict:
    return {
        "ust": {
            "tempo": 120.0,
            "tracks": 1,
            "notes": [{"index": "0000", "length": 480, "lyric": "A", "note_num": 69, "phoneme": "a", "pitch_bend": [0, 5, -3]}],
        },
        "show": {"bpm": True, "play_time": True, "lyric": True, "curve_show": False},
        "project": {"song_name": "测试曲", "song_author": "作者", "ust_author": "调音师"},
        "style": {"bg_color": "#000000", "note_color": "#6c6c6c", "lyric_pos": "top"},
        "width": 1920,
        "height": 1080,
        "fps": 60,
        "output_path": "",
    }


class TestRenderConfigLoad:
    def test_from_dict_defaults(self):
        cfg = RenderConfig.from_dict({})
        assert cfg.width == 1920
        assert cfg.height == 1080
        assert cfg.fps == 60
        assert cfg.ust.tempo == 120.0
        assert cfg.ust.notes == []
        assert cfg.style.bg_color == "#000000"
        assert cfg.style.silent_display == "r"

    def test_from_dict_full(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        assert cfg.width == 1920
        assert len(cfg.ust.notes) == 1
        n = cfg.ust.notes[0]
        assert n.lyric == "A"
        assert n.note_num == 69
        assert n.pitch_bend == [0, 5, -3]
        assert cfg.project.song_name == "测试曲"
        assert cfg.style.lyric_pos == "top"

    def test_roundtrip_to_dict(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        d = cfg.to_dict()
        cfg2 = RenderConfig.from_dict(d)
        assert cfg2.to_dict() == cfg.to_dict()

    def test_load_missing_file_returns_default(self, tmp_path):
        cfg = RenderConfig.load(tmp_path / "nope.json")
        assert cfg.width == 1920

    def test_save_and_load(self, tmp_path):
        cfg = RenderConfig.from_dict(_sample_dict())
        p = tmp_path / "out.json"
        cfg.save(p)
        cfg2 = RenderConfig.load(p)
        assert cfg2.to_dict() == cfg.to_dict()
        # 中文保留(ensure_ascii=False)
        raw = p.read_text(encoding="utf-8")
        assert "测试曲" in raw

    def test_load_utf8_bom(self, tmp_path):
        p = tmp_path / "bom.json"
        p.write_bytes(b"\xef\xbb\xbf" + json.dumps(_sample_dict(), ensure_ascii=False).encode("utf-8"))
        cfg = RenderConfig.load(p)
        assert cfg.width == 1920


class TestValidation:
    def test_valid_config_no_errors(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        assert cfg.validate() == []

    def test_invalid_color(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        cfg.style.bg_color = "#GGGGGG"
        errs = cfg.validate()
        assert any("bg_color" in e for e in errs)

    def test_invalid_enum(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        cfg.style.silent_display = "bogus"
        assert any("silent_display" in e for e in cfg.validate())

    def test_zero_size(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        cfg.width = 0
        assert any("分辨率" in e for e in cfg.validate())

    def test_bad_fps(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        cfg.fps = 999
        assert any("fps" in e for e in cfg.validate())

    def test_bad_tempo(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        cfg.ust.tempo = -5
        assert any("tempo" in e for e in cfg.validate())


class TestExport:
    def test_export_script_contains_flow(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        lines = export_with_ffmpeg(cfg, "dll_path", "out_dir")
        joined = "\n".join(lines)
        assert "up_create_context" in joined
        assert "up_set_config" in joined
        assert "up_begin_export" in joined
        assert "up_render_frame" in joined
        assert "up_end_export" in joined
        assert "up_destroy_context" in joined

    def test_export_command(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        cmd = cfg.export_command()
        assert "cargo" in cmd and "cli" in cmd

    def test_summary(self):
        cfg = RenderConfig.from_dict(_sample_dict())
        s = cfg.summary()
        assert "1920x1080@60fps" in s
        assert "1 音符" in s


class TestNoteInfo:
    def test_from_dict_parses_pitch_bend(self):
        n = NoteInfo.from_dict({"pitch_bend": [0, 1, -2, 3]})
        assert n.pitch_bend == [0, 1, -2, 3]

    def test_from_dict_invalid_pitch_bend(self):
        n = NoteInfo.from_dict({"pitch_bend": ["a", "b"]})
        # 非法元素被跳过
        assert n.pitch_bend == []

    def test_pitch_bend_not_list(self):
        n = NoteInfo.from_dict({"pitch_bend": "not-a-list"})
        assert n.pitch_bend == []


class TestEnums:
    def test_enum_values(self):
        assert SILENT_DISPLAY_OPTIONS == ["r", "dash", "custom", "none"]
        assert END_DISPLAY_OPTIONS == ["end", "dash", "custom", "none"]
        assert PITCH_PLACEHOLDER_OPTIONS == ["none", "dash", "custom"]
        assert LYRIC_POS_OPTIONS == ["top", "bottom"]
