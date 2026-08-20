use serde::Deserialize;

/// 单个音符（对应 Python NoteInfo）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct NoteInfo {
    pub index: String,        // 段编号，如 "0000"
    pub length: i32,          // 长度（tick）
    pub lyric: String,        // 歌词字，可能为 "R"（休止）或 "-"（延音）
    pub note_num: i32,        // MIDI 音高
    pub phoneme: String,      // 音素，可为空
    pub pitch_bend: Vec<i32>, // 音高弯曲数据（≥2 个点才绘制）
}

/// UST 解析结果（对应 Python UstInfo）
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UstInfo {
    pub version: String,
    pub tempo: f64, // BPM，默认 120.0
    pub tracks: u32,
    pub notes: Vec<NoteInfo>,
}

impl Default for UstInfo {
    fn default() -> Self {
        Self {
            version: String::new(),
            tempo: 120.0,
            tracks: 1,
            notes: Vec::new(),
        }
    }
}

/// 显示开关（对应 Python ShowConfig）
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ShowConfig {
    pub bpm: bool,         // 显示 BPM
    pub play_time: bool,   // 显示播放时间
    pub song_name: bool,   // 显示曲名
    pub song_author: bool, // 显示 MIDI 作者
    pub ust_author: bool,  // 显示调音师
    pub lyric: bool,       // 显示 LRC 歌词
    pub curve_show: bool,  // 显示音高线（默认 false）
}

impl Default for ShowConfig {
    fn default() -> Self {
        Self {
            bpm: true,
            play_time: true,
            song_name: true,
            song_author: true,
            ust_author: true,
            lyric: true,
            curve_show: false,
        }
    }
}

/// 项目信息（对应 Python ProjectInfo）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProjectInfo {
    pub project_name: String,
    pub song_name: String,
    pub song_author: String,
    pub ust_author: String,
}

/// 播放器样式（对应 Python PlayerStyle）
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PlayerStyle {
    pub bg_color: String,         // "#000000" 默认
    pub note_color: String,       // "#6c6c6c" 默认（音名色）
    pub lyric_color: String,      // "#FFFFFF"（歌字色）
    pub lyric_text_color: String, // "#FFFFFF"（LRC 歌词色）
    pub other_text_color: String, // "#FFFFFF"（其他文字）
    pub lyric_pos: String,        // "top" | "bottom"
    pub fullscreen: bool,         // 是否全屏
    pub lrc_path: String,         // LRC 歌词文件路径
    pub music_path: String,       // 伴奏音频路径
    pub silent_display: String,   // "r" | "dash" | "custom" | "none"
    pub silent_custom_text: String,
    pub end_display: String, // "end" | "dash" | "custom" | "none"
    pub end_custom_text: String,
    pub pitch_placeholder: String, // "none" | "dash" | "custom"
    pub pitch_custom_text: String,
    pub pitch_curve_color: String, // "#FFFFFF"（音高线颜色）
    pub app_version: String,       // 版权行版本号，默认 "1.1.0 Beta 1"
}

impl Default for PlayerStyle {
    fn default() -> Self {
        Self {
            bg_color: "#000000".into(),
            note_color: "#6c6c6c".into(),
            lyric_color: "#FFFFFF".into(),
            lyric_text_color: "#FFFFFF".into(),
            other_text_color: "#FFFFFF".into(),
            lyric_pos: "top".into(),
            fullscreen: true,
            lrc_path: String::new(),
            music_path: String::new(),
            silent_display: "r".into(),
            silent_custom_text: String::new(),
            end_display: "end".into(),
            end_custom_text: String::new(),
            pitch_placeholder: "none".into(),
            pitch_custom_text: String::new(),
            pitch_curve_color: "#FFFFFF".into(),
            app_version: "1.1.0 Beta 1".into(),
        }
    }
}

/// 完整启动参数（对应 Python PlayerLaunchParams）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RenderConfig {
    pub ust: UstInfo,
    pub show: ShowConfig,
    pub project: ProjectInfo,
    pub style: PlayerStyle,
    pub width: u32,  // 输出画面宽
    pub height: u32, // 输出画面高
    pub fps: u32,    // 帧率，默认 60
    pub output_path: String,
}
