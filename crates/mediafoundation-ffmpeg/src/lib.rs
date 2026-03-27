#![cfg(windows)]

mod audio;
mod h264;
mod hevc;

pub use audio::AudioExt;
pub use h264::{H264StreamMuxer, MuxerConfig, set_fragmented_mp4_options};
pub use hevc::HevcStreamMuxer;
