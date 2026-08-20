use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

/// 视频帧编码器抽象。
pub trait VideoEncoder {
    /// 送入一帧 RGBA 原始数据。
    fn push_frame(&mut self, data: &[u8], w: u32, h: u32) -> Result<(), Box<dyn Error>>;
    /// 完成编码，写出文件尾/索引。
    fn finish(&mut self) -> Result<(), Box<dyn Error>>;
}

/// 空编码器：不做任何实际写入，仅用于验证流程。
#[derive(Default)]
pub struct NullEncoder;

impl NullEncoder {
    pub fn new() -> Self {
        Self
    }
}

impl VideoEncoder for NullEncoder {
    fn push_frame(&mut self, _data: &[u8], _w: u32, _h: u32) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

/// 通过外部 `ffmpeg` 将原始 RGBA 帧编码为 H.264 MP4。
pub struct FfmpegEncoder {
    _output_path: String,
    width: u32,
    height: u32,
    _fps: u32,
    child: Option<std::process::Child>,
}

impl FfmpegEncoder {
    pub fn new(
        output_path: &str,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self, Box<dyn Error>> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{}x{}", width, height),
            "-r",
            &fps.to_string(),
            "-i",
            "-",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-y",
            output_path,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        // stderr 继承父进程:ffmpeg 会输出实时编码进度(frame=/fps=/time=),
        // 让宿主能看到编码正在进行,而不是无声等待。
        .stderr(Stdio::inherit());

        let child = cmd.spawn().map_err(|e| -> Box<dyn Error> {
            if e.kind() == std::io::ErrorKind::NotFound {
                "ffmpeg executable not found in PATH".into()
            } else {
                e.into()
            }
        })?;

        Ok(Self {
            _output_path: output_path.to_string(),
            width,
            height,
            _fps: fps,
            child: Some(child),
        })
    }
}

impl VideoEncoder for FfmpegEncoder {
    fn push_frame(&mut self, data: &[u8], w: u32, h: u32) -> Result<(), Box<dyn Error>> {
        let expected = (self.width * self.height * 4) as usize;
        if data.len() != expected {
            return Err(format!(
                "frame size mismatch: expected {} bytes for {}x{}, got {}",
                expected,
                self.width,
                self.height,
                data.len()
            )
            .into());
        }
        if w != self.width || h != self.height {
            return Err(format!(
                "frame dimension mismatch: expected {}x{}, got {}x{}",
                self.width, self.height, w, h
            )
            .into());
        }
        let stdin = self
            .child
            .as_mut()
            .and_then(|c| c.stdin.as_mut())
            .ok_or("ffmpeg stdin not available")?;
        stdin.write_all(data).map_err(|e| e.into())
    }

    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take());
            let status = child.wait()?;
            if !status.success() {
                return Err(format!("ffmpeg exited with {:?}", status.code()).into());
            }
        }
        Ok(())
    }
}

impl Drop for FfmpegEncoder {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_encoder() {
        let mut enc = NullEncoder::new();
        enc.push_frame(&[], 1920, 1080).unwrap();
        enc.finish().unwrap();
    }
}
