//! # ustplayer_renderer
//!
//! Rust 视频渲染动态库，为 ustPlayer 提供像素级还原的渲染与视频导出能力。
//! 通过 C ABI 与宿主通信，零 panic 跨越 FFI。

pub mod encode;
pub mod fonts;
pub mod lrc;
pub mod model;
pub mod render;
pub mod text;
pub mod timing;

pub mod ffi;
