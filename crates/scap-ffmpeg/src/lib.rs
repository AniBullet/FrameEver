#[cfg(target_os = "macos")]
mod screencapturekit;
#[cfg(target_os = "macos")]
pub use screencapturekit::*;

#[cfg(windows)]
mod direct3d;
#[cfg(windows)]
pub use direct3d::*;

mod cpal;
pub use cpal::*;

pub trait AsFFmpeg {
    fn as_ffmpeg(&self) -> Result<ffmpeg::frame::Video, AsFFmpegError>;
    fn as_ffmpeg_into(&self, dest: &mut ffmpeg::frame::Video) -> Result<(), AsFFmpegError> {
        let frame = self.as_ffmpeg()?;
        let src = frame.data(0);
        let dst = dest.data_mut(0);
        let copy_len = src.len().min(dst.len());
        dst[..copy_len].copy_from_slice(&src[..copy_len]);
        Ok(())
    }
}
