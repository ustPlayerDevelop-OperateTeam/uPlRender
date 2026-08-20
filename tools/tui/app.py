"""ustPlayer Renderer 配置 TUI —— OpenCode 风格。

布局:
  ┌ Header(标题 + 当前文件 + 状态) ─────────────┐
  ├ 左侧导航树        │  右侧编辑区(按导航切换)    │
  ├──────────────────┴──────────────────────────┤
  └ Footer(快捷键提示) ──────────────────────────┘

快捷键:
  q           退出(未保存时提醒)
  s           保存配置到当前文件
  S           另存为(输入路径)
  e           导出(生成导出脚本/命令,展示说明)
  /           聚焦导航树
  Tab         在导航树/编辑区之间切换焦点
  方向键/鼠标  导航与编辑
"""

from __future__ import annotations

import shlex
from pathlib import Path

from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import (
    Button,
    Footer,
    Header,
    Input,
    Label,
    Static,
    Tree,
)

from config import RenderConfig
from widgets import (
    NotesEditor,
    OutputEditor,
    ProjectEditor,
    ShowEditor,
    StyleEditor,
)

DEFAULT_CONFIG = Path(__file__).resolve().parent.parent.parent / "examples" / "frame_config.json"


# ---------- 编辑区容器:同一时刻只显示一个编辑器 ----------

class EditorPane(Vertical):
    """右侧编辑区:根据导航选择切换内容。"""

    def __init__(self, cfg: RenderConfig, **kw):
        super().__init__(**kw)
        self.cfg = cfg
        self._current: str = "notes"

    def compose(self) -> ComposeResult:
        yield NotesEditor(self.cfg, id="editor_notes")
        yield StyleEditor(self.cfg.style, id="editor_style", classes="hidden")
        yield ShowEditor(self.cfg.show, id="editor_show", classes="hidden")
        yield ProjectEditor(self.cfg.project, id="editor_project", classes="hidden")
        yield OutputEditor(self.cfg, id="editor_output", classes="hidden")

    def switch_to(self, name: str) -> None:
        """显示 name 对应的编辑器,隐藏其余。"""
        mapping = {
            "notes": "editor_notes",
            "style": "editor_style",
            "show": "editor_show",
            "project": "editor_project",
            "output": "editor_output",
        }
        target = mapping.get(name)
        if not target:
            return
        for wid in mapping.values():
            w = self.query_one(f"#{wid}")
            w.set_class(w.id != target, "hidden")
        self._current = name
        # 聚焦新显示的面板
        pane = self.query_one(f"#{target}")
        pane.focus()

    def collect_all(self) -> None:
        """把所有编辑器当前值写回 cfg(保存前调用)。"""
        NotesEditor.collect(self.query_one("#editor_notes"), self.cfg)
        StyleEditor.collect(self.query_one("#editor_style"), self.cfg.style)
        ShowEditor.collect(self.query_one("#editor_show"), self.cfg.show)
        ProjectEditor.collect(self.query_one("#editor_project"), self.cfg.project)
        OutputEditor.collect(self.query_one("#editor_output"), self.cfg)


# ---------- 导出说明弹窗 ----------

class ExportScreen(ModalScreen):
    """展示导出方式说明,可一键写入导出脚本。"""

    BINDINGS = [("escape", "dismiss", "关闭")]

    def __init__(self, script: str, cfg: RenderConfig, **kw):
        super().__init__(**kw)
        self._script = script
        self._cfg = cfg

    def compose(self) -> ComposeResult:
        yield Vertical(
            Static("📤 导出", classes="modal-title"),
            Static("方式一:运行 Rust 示例(cli.rs 走完整导出流程)", classes="muted"),
            Static(f"命令: {shlex.join(['cargo', 'run', '--release', '--example', 'cli'])}", classes="code"),
            Static("方式二:生成 ctypes 导出脚本(需先 cargo build --release)", classes="muted"),
            Static(self._script, classes="code prewrap"),
            Horizontal(
                Button("💾 写入 export_script.py", id="btn_write_script"),
                Button("关闭", id="btn_close"),
                classes="btn-row",
            ),
            classes="modal",
        )

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn_write_script":
            repo_root = Path(__file__).resolve().parent.parent.parent
            out = repo_root / "export_script.py"
            out.write_text(self._script, encoding="utf-8")
            self.dismiss(f"已写入 {out}")
        else:
            self.dismiss()


# ---------- 主 App ----------

class RendererTUI(App):
    """OpenCode 风格的渲染配置编辑器。"""

    TITLE = "ustPlayer Renderer TUI"

    CSS = """
    #nav-tree {
        width: 34;
        border-right: solid $accent;
        padding: 0 1;
    }
    #editor-pane {
        height: 1fr;
    }
    .hidden { display: none; }
    .section-title {
        text-style: bold;
        color: $accent;
        margin: 1 0 0 0;
    }
    .field-row { height: 3; }
    .field-label {
        width: 14;
        content-align: right middle;
        color: $text-secondary;
    }
    .field-input { width: 1fr; }
    .btn-row { height: 3; }
    .note-block {
        border: round $surface;
        margin: 1 0;
        padding: 0 1;
    }
    .note-title { text-style: bold; color: $text; }
    .muted { color: $text-muted; }
    .code {
        background: $surface;
        padding: 0 1;
        margin: 1 0;
    }
    .prewrap { height: auto; }
    .modal {
        width: 90;
        height: 70%;
        border: thick $accent;
        background: $surface;
        padding: 1 2;
    }
    .modal-title { text-style: bold; text-align: center; margin-bottom: 1; }
    """

    BINDINGS = [
        Binding("q", "quit", "退出"),
        Binding("s", "save", "保存"),
        Binding("S", "save_as", "另存为"),
        Binding("e", "export", "导出"),
        Binding("/", "focus_tree", "导航"),
        Binding("tab", "focus_next", "下一个"),
        Binding("shift+tab", "focus_previous", "上一个"),
    ]

    def __init__(self, config_path: str | Path | None = None, **kw):
        super().__init__(**kw)
        self.config_path = Path(config_path) if config_path else DEFAULT_CONFIG
        self.cfg = RenderConfig.load(self.config_path)
        self._dirty = False
        self._ui_initialized = False  # 初始化完成前忽略 changed 事件

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        yield Horizontal(
            Tree("配置", id="nav-tree"),
            EditorPane(self.cfg, id="editor-pane"),
        )
        yield Footer()

    def on_mount(self) -> None:
        tree = self.query_one("#nav-tree", Tree)
        tree.root.expand()
        nodes = [
            ("notes", "🎵 UST 数据"),
            ("style", "🎨 样式"),
            ("show", "👁 显示开关"),
            ("project", "🏷 项目信息"),
            ("output", "📤 输出设置"),
        ]
        for key, label in nodes:
            node = tree.root.add(label, data=key)
            if key == "notes":
                node.expand()
        # 默认选中第一项
        tree.focus()
        self._mark_dirty(False)
        # 初始化组件挂载完成后再启用 dirty 跟踪(挂载期会触发 Changed 事件)
        self.call_after_refresh(self._enable_dirty_tracking)

    def _enable_dirty_tracking(self) -> None:
        self._ui_initialized = True
        self._mark_dirty(False)

    # ---------- 事件 ----------

    def on_tree_node_selected(self, event: Tree.NodeSelected) -> None:
        key = event.node.data
        if key:
            self.query_one("#editor-pane", EditorPane).switch_to(key)

    def on_input_changed(self, event: Input.Changed) -> None:
        if self._ui_initialized:
            self._mark_dirty(True)

    def on_checkbox_changed(self, event) -> None:
        if self._ui_initialized:
            self._mark_dirty(True)

    def on_select_changed(self, event) -> None:
        if self._ui_initialized:
            self._mark_dirty(True)

    # ---------- 动作 ----------

    def _mark_dirty(self, dirty: bool) -> None:
        self._dirty = dirty
        # Header 显示 App.title/sub_title,这里更新 sub_title 反映文件与状态
        self.sub_title = (
            f"{self.config_path} {'●' if dirty else '○'} | {self.cfg.summary()}"
        )

    def _collect_and_validate(self) -> list[str]:
        self.query_one("#editor-pane", EditorPane).collect_all()
        return self.cfg.validate()

    def action_save(self) -> None:
        errs = self._collect_and_validate()
        if errs:
            self.notify("; ".join(errs), severity="error", timeout=8)
            return
        self.cfg.save(self.config_path)
        self._mark_dirty(False)
        self.notify(f"已保存 {self.config_path}", severity="information")

    def action_save_as(self) -> None:
        self.query_one("#editor-pane", EditorPane).collect_all()

        def do_save(path: str) -> None:
            p = Path(path.strip())
            if not p.suffix:
                p = p.with_suffix(".json")
            errs = self.cfg.validate()
            if errs:
                self.notify("; ".join(errs), severity="error", timeout=8)
                return
            self.cfg.save(p)
            self.config_path = p
            self._mark_dirty(False)
            self.notify(f"已另存为 {p}", severity="information")

        self._ask_path("另存为 JSON 路径:", do_save)

    def action_export(self) -> None:
        errs = self._collect_and_validate()
        if errs:
            self.notify("; ".join(errs), severity="error", timeout=8)
            return
        from config import export_with_ffmpeg

        repo_root = Path(__file__).resolve().parent.parent.parent
        dll = repo_root / "target" / "release" / "ustplayer_renderer.dll"
        out = repo_root / "export_output"
        script = "\n".join(export_with_ffmpeg(self.cfg, dll, out))
        self.push_screen(ExportScreen(script, self.cfg))

    def action_focus_tree(self) -> None:
        self.query_one("#nav-tree", Tree).focus()

    def action_quit(self) -> None:
        if self._dirty:
            self._confirm_quit()
        else:
            self.exit()

    def _confirm_quit(self) -> None:
        """未保存退出确认(用问答式弹窗)。"""

        class ConfirmScreen(ModalScreen):
            BINDINGS = [("escape", "dismiss", "取消")]

            def compose(self) -> ComposeResult:
                yield Vertical(
                    Static("⚠ 有未保存的修改", classes="modal-title"),
                    Static(f"当前文件: {self.app.config_path}", classes="muted"),
                    Horizontal(
                        Button("💾 保存并退出", id="save_exit", variant="primary"),
                        Button("不保存退出", id="discard_exit", variant="error"),
                        Button("取消", id="cancel"),
                        classes="btn-row",
                    ),
                    classes="modal",
                )

            def on_button_pressed(self, event: Button.Pressed) -> None:
                bid = event.button.id
                if bid == "save_exit":
                    self.app.action_save()
                    self.app.exit()
                elif bid == "discard_exit":
                    self.app.exit()
                else:
                    self.dismiss()

        self.push_screen(ConfirmScreen())

    def _ask_path(self, prompt: str, on_submit) -> None:
        """简单路径输入弹窗。"""

        class PathScreen(ModalScreen):
            BINDINGS = [("escape", "dismiss", "取消")]

            def compose(self) -> ComposeResult:
                yield Vertical(
                    Static(prompt, classes="modal-title"),
                    Input(placeholder="路径", id="path_input"),
                    Horizontal(
                        Button("确定", id="ok", variant="primary"),
                        Button("取消", id="cancel"),
                        classes="btn-row",
                    ),
                    classes="modal",
                )

            def on_button_pressed(self, event: Button.Pressed) -> None:
                if event.button.id == "ok":
                    value = self.query_one("#path_input", Input).value
                    self.dismiss(value)
                else:
                    self.dismiss(None)

            def on_input_submitted(self, event: Input.Submitted) -> None:
                self.dismiss(event.value)

        def _handle(value) -> None:
            if value:
                on_submit(value)

        self.push_screen(PathScreen(), callback=_handle)


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description="ustPlayer Renderer 配置 TUI")
    parser.add_argument("config", nargs="?", default=None,
                        help="配置文件路径(默认 examples/frame_config.json)")
    args = parser.parse_args()
    RendererTUI(args.config).run()


if __name__ == "__main__":
    main()
