mod direct3d;
pub use direct3d::*;

mod cpal;
pub use cpal::*;

pub trait AsFFmpeg {
    fn as_ffmpeg(&self) -> Result<ffmpeg::frame::Video, AsFFmpegError>;
}
