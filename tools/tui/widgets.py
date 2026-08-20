"""可复用的编辑区组件:样式编辑 / 显示开关 / 项目信息 / 输出设置 / 音符表。

每个组件是一个 Textual Composite,负责展示并写回 config 模型。
"""

from __future__ import annotations

from textual.app import ComposeResult
from textual.containers import Horizontal, Vertical, VerticalScroll
from textual.widgets import (
    Button,
    Checkbox,
    Input,
    Label,
    Select,
    Static,
)

from config import (
    LYRIC_POS_OPTIONS,
    PITCH_PLACEHOLDER_OPTIONS,
    SILENT_DISPLAY_OPTIONS,
    END_DISPLAY_OPTIONS,
    NoteInfo,
    PlayerStyle,
    ProjectInfo,
    RenderConfig,
    ShowConfig,
)


def _select_options(opts: list[str]) -> list[tuple[str, str]]:
    return [(o, o) for o in opts]


def _safe_select_value(value: str, opts: list[str], default: str) -> str:
    """Select 的 value 必须是选项之一,否则 Textual 抛 SelectError。"""
    return value if value in opts else default


# ---------- 通用:一对 Label + Input 的字段行 ----------

class FieldRow(Horizontal):
    """一行表单字段:左侧标签(固定宽),右侧输入框。"""

    def __init__(self, label: str, value: str, placeholder: str = "", **kw):
        super().__init__(**kw)
        self._label = label
        self._value = value
        self._placeholder = placeholder

    def compose(self) -> ComposeResult:
        yield Label(self._label, classes="field-label")
        yield Input(self._value, placeholder=self._placeholder, classes="field-input")

    @property
    def input(self) -> Input:
        return self.query_one(Input)


# ---------- 样式编辑 ----------

class StyleEditor(VerticalScroll):
    """样式:颜色字段 + 枚举选择 + 自定义文本。"""

    def __init__(self, style: PlayerStyle, **kw):
        super().__init__(**kw)
        self.style = style

    def compose(self) -> ComposeResult:
        yield Static("🎨 颜色", classes="section-title")
        for key, label in PlayerStyle.COLOR_FIELDS:
            yield FieldRow(label, getattr(self.style, key), placeholder="#RRGGBB")

        yield Static("⚙ 选项", classes="section-title")
        yield Horizontal(
            Label("歌词位置", classes="field-label"),
            Select(_select_options(LYRIC_POS_OPTIONS),
                   value=_safe_select_value(self.style.lyric_pos, LYRIC_POS_OPTIONS, "top"),
                   id="sel_lyric_pos"),
            classes="field-row",
        )
        yield Horizontal(
            Label("静默显示", classes="field-label"),
            Select(_select_options(SILENT_DISPLAY_OPTIONS),
                   value=_safe_select_value(self.style.silent_display, SILENT_DISPLAY_OPTIONS, "r"),
                   id="sel_silent"),
            classes="field-row",
        )
        yield Horizontal(
            Label("结束显示", classes="field-label"),
            Select(_select_options(END_DISPLAY_OPTIONS),
                   value=_safe_select_value(self.style.end_display, END_DISPLAY_OPTIONS, "end"),
                   id="sel_end"),
            classes="field-row",
        )
        yield Horizontal(
            Label("音高占位", classes="field-label"),
            Select(_select_options(PITCH_PLACEHOLDER_OPTIONS),
                   value=_safe_select_value(self.style.pitch_placeholder, PITCH_PLACEHOLDER_OPTIONS, "none"),
                   id="sel_pitch"),
            classes="field-row",
        )

        yield Static("📝 自定义文本", classes="section-title")
        for key, label in PlayerStyle.TEXT_FIELDS:
            yield FieldRow(label, getattr(self.style, key), placeholder="")

        yield Checkbox("全屏", self.style.fullscreen, id="chk_fullscreen")

    def collect(self, style: PlayerStyle) -> None:
        """把界面上的值写回 style 对象。"""
        # 字段行顺序与 compose 一致:先颜色字段,后文本字段
        rows = list(self.query(FieldRow))
        idx = 0
        for key, _ in PlayerStyle.COLOR_FIELDS:
            setattr(style, key, rows[idx].input.value.strip())
            idx += 1
        for key, _ in PlayerStyle.TEXT_FIELDS:
            setattr(style, key, rows[idx].input.value)
            idx += 1
        style.lyric_pos = str(self.query_one("#sel_lyric_pos", Select).value)
        style.silent_display = str(self.query_one("#sel_silent", Select).value)
        style.end_display = str(self.query_one("#sel_end", Select).value)
        style.pitch_placeholder = str(self.query_one("#sel_pitch", Select).value)
        style.fullscreen = self.query_one("#chk_fullscreen", Checkbox).value


# ---------- 显示开关 ----------

class ShowEditor(VerticalScroll):
    """显示开关:一组 Checkbox。"""

    def __init__(self, show: ShowConfig, **kw):
        super().__init__(**kw)
        self.show = show

    def compose(self) -> ComposeResult:
        yield Static("👁 显示开关", classes="section-title")
        for key, label in ShowConfig.FIELDS:
            yield Checkbox(label, getattr(self.show, key), id=f"chk_{key}")

    def collect(self, show: ShowConfig) -> None:
        for key, _ in ShowConfig.FIELDS:
            setattr(show, key, self.query_one(f"#chk_{key}", Checkbox).value)


# ---------- 项目信息 ----------

class ProjectEditor(VerticalScroll):
    """项目信息:文本字段。"""

    def __init__(self, project: ProjectInfo, **kw):
        super().__init__(**kw)
        self.project = project

    def compose(self) -> ComposeResult:
        yield Static("🏷 项目信息", classes="section-title")
        for key, label in ProjectInfo.FIELDS:
            yield FieldRow(label, getattr(self.project, key))

    def collect(self, project: ProjectInfo) -> None:
        rows = list(self.query(FieldRow))
        for i, (key, _) in enumerate(ProjectInfo.FIELDS):
            setattr(project, key, rows[i].input.value)


# ---------- 输出设置 ----------

class OutputEditor(VerticalScroll):
    """输出设置:分辨率 / fps / 输出路径。"""

    def __init__(self, cfg: RenderConfig, **kw):
        super().__init__(**kw)
        self.cfg = cfg

    def compose(self) -> ComposeResult:
        yield Static("📤 输出设置", classes="section-title")
        yield FieldRow("宽度", str(self.cfg.width), placeholder="如 1920")
        yield FieldRow("高度", str(self.cfg.height), placeholder="如 1080")
        yield FieldRow("fps", str(self.cfg.fps), placeholder="如 60")
        yield FieldRow("输出路径", self.cfg.output_path, placeholder="如 out.mp4")

    def collect(self, cfg: RenderConfig) -> None:
        rows = list(self.query(FieldRow))
        try:
            cfg.width = max(1, int(rows[0].input.value.strip() or "1920"))
            cfg.height = max(1, int(rows[1].input.value.strip() or "1080"))
            cfg.fps = max(1, min(240, int(rows[2].input.value.strip() or "60")))
        except ValueError:
            pass  # 非法输入保持原值
        cfg.output_path = rows[3].input.value.strip()


# ---------- 音符表 ----------

class NotesEditor(VerticalScroll):
    """音符列表:tempo/tracks 输入 + 每音符一个折叠行(可编辑)。"""

    def __init__(self, cfg: RenderConfig, **kw):
        super().__init__(**kw)
        self.cfg = cfg

    def compose(self) -> ComposeResult:
        yield Static("🎵 UST 数据", classes="section-title")
        yield FieldRow("BPM (tempo)", str(self.cfg.ust.tempo))
        yield FieldRow("音轨数 (tracks)", str(self.cfg.ust.tracks))
        yield Static(f"音符数: {len(self.cfg.ust.notes)}", id="note_count", classes="muted")
        yield Horizontal(
            Button("➕ 添加音符", id="btn_add_note"),
            Button("🗑 删除最后", id="btn_del_note"),
            classes="btn-row",
        )
        for i, note in enumerate(self.cfg.ust.notes):
            yield self._note_block(i, note)

    def _note_block(self, i: int, note: NoteInfo):
        return Vertical(
            Static(f"#{i}  {note.lyric or '-'}  MIDI {note.note_num}  len {note.length}",
                   id=f"note_title_{i}", classes="note-title"),
            Horizontal(
                Label("歌词", classes="field-label"),
                Input(note.lyric, id=f"note_lyric_{i}"),
                Label("音高", classes="field-label"),
                Input(str(note.note_num), id=f"note_num_{i}"),
                classes="field-row",
            ),
            Horizontal(
                Label("长度", classes="field-label"),
                Input(str(note.length), id=f"note_len_{i}"),
                Label("音素", classes="field-label"),
                Input(note.phoneme, id=f"note_phoneme_{i}"),
                classes="field-row",
            ),
            Horizontal(
                Label("音高弯曲", classes="field-label"),
                Input(
                    " ".join(str(x) for x in note.pitch_bend),
                    id=f"note_pb_{i}",
                    placeholder="空格分隔,如 0 10 -5",
                ),
                classes="field-row",
            ),
            classes="note-block",
        )

    def on_button_pressed(self, event: Button.Pressed) -> None:
        """处理添加/删除音符:先收集当前输入,再重建界面。"""
        if event.button.id == "btn_add_note":
            self.collect(self.cfg)
            self.cfg.ust.notes.append(NoteInfo(index=f"{len(self.cfg.ust.notes):04d}"))
            self._refresh()
            self.notify(f"已添加音符 #{len(self.cfg.ust.notes) - 1}", severity="information")
        elif event.button.id == "btn_del_note":
            self.collect(self.cfg)
            if self.cfg.ust.notes:
                self.cfg.ust.notes.pop()
                self._refresh()
                self.notify(f"已删除,剩余 {len(self.cfg.ust.notes)} 个音符")
            else:
                self.notify("没有可删除的音符", severity="warning")

    def _refresh(self) -> None:
        """重建整个编辑器界面(音符结构变化后)。"""
        self.recompose()

    def collect(self, cfg: RenderConfig) -> None:
        rows = list(self.query(FieldRow))
        if len(rows) >= 2:
            try:
                cfg.ust.tempo = float(rows[0].input.value.strip() or "120")
                cfg.ust.tracks = max(1, int(rows[1].input.value.strip() or "1"))
            except ValueError:
                pass
        notes: list[NoteInfo] = []
        i = 0
        while True:
            matches = self.query(f"Input#note_lyric_{i}")
            if not matches:
                break
            lyric = matches.first().value
            try:
                num = int(self.query_one(f"#note_num_{i}", Input).value.strip() or "60")
            except ValueError:
                num = 60
            try:
                length = int(self.query_one(f"#note_len_{i}", Input).value.strip() or "480")
            except ValueError:
                length = 480
            phoneme = self.query_one(f"#note_phoneme_{i}", Input).value
            pb_raw = self.query_one(f"#note_pb_{i}", Input).value.strip()
            pb = []
            for tok in pb_raw.replace(",", " ").split():
                try:
                    pb.append(int(tok))
                except ValueError:
                    pass
            notes.append(NoteInfo(index=f"{i:04d}", length=length, lyric=lyric,
                                  note_num=num, phoneme=phoneme, pitch_bend=pb))
            i += 1
        cfg.ust.notes = notes
