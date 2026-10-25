use ffmpeg::{format, frame};
use frameever_media_info::RawVideoFormat;
use std::{path::PathBuf, time::Duration};
use tracing::*;

use crate::{
    PacketSink,
    audio::AudioEncoder,
    h264,
    video::{
        h264::{H264Encoder, H264EncoderError},
        hevc::{HevcEncoder, HevcEncoderError},
    },
};

pub enum Mp4VideoEncoder {
    H264(H264Encoder),
    Hevc(HevcEncoder),
}

#[derive(thiserror::Error, Debug)]
pub enum VideoInitError {
    #[error("{0}")]
    H264(#[from] H264EncoderError),
    #[error("{0}")]
    Hevc(#[from] HevcEncoderError),
}

pub struct MP4File {
    #[allow(unused)]
    tag: &'static str,
    output: format::context::Output,
    video: Mp4VideoEncoder,
    audio: Option<Box<dyn AudioEncoder + Send>>,
    is_finished: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum InitError {
    #[error("{0:?}")]
    Ffmpeg(ffmpeg::Error),
    #[error("Video/{0}")]
    VideoInit(VideoInitError),
    #[error("Audio/{0}")]
    AudioInit(Box<dyn std::error::Error>),
}

#[derive(thiserror::Error, Debug)]
pub enum FinishError {
    #[error("Already finished")]
    AlreadyFinished,
    #[error("{0}")]
    WriteTrailerFailed(ffmpeg::Error),
}

pub struct FinishResult {
    pub video_finish: Result<(), ffmpeg::Error>,
    pub audio_finish: Result<(), ffmpeg::Error>,
}

impl MP4File {
    pub fn init(
        tag: &'static str,
        output: PathBuf,
        faststart: bool,
        video: impl FnOnce(&mut format::context::Output) -> Result<H264Encoder, H264EncoderError>,
        audio: impl FnOnce(
            &mut format::context::Output,
        )
            -> Option<Result<Box<dyn AudioEncoder + Send>, Box<dyn std::error::Error>>>,
    ) -> Result<Self, InitError> {
        Self::init_with(
            tag,
            output,
            faststart,
            |o| {
                video(o)
                    .map(Mp4VideoEncoder::H264)
                    .map_err(VideoInitError::from)
            },
            audio,
        )
    }

    pub fn init_hevc(
        tag: &'static str,
        output: PathBuf,
        faststart: bool,
        video: impl FnOnce(&mut format::context::Output) -> Result<HevcEncoder, HevcEncoderError>,
        audio: impl FnOnce(
            &mut format::context::Output,
        )
            -> Option<Result<Box<dyn AudioEncoder + Send>, Box<dyn std::error::Error>>>,
    ) -> Result<Self, InitError> {
        Self::init_with(
            tag,
            output,
            faststart,
            |o| {
                video(o)
                    .map(Mp4VideoEncoder::Hevc)
                    .map_err(VideoInitError::from)
            },
            audio,
        )
    }

    fn init_with(
        tag: &'static str,
        mut output: PathBuf,
        faststart: bool,
        video: impl FnOnce(&mut format::context::Output) -> Result<Mp4VideoEncoder, VideoInitError>,
        audio: impl FnOnce(
            &mut format::context::Output,
        )
            -> Option<Result<Box<dyn AudioEncoder + Send>, Box<dyn std::error::Error>>>,
    ) -> Result<Self, InitError> {
        output.set_extension("mp4");

        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut output = format::output(&output).map_err(InitError::Ffmpeg)?;

        trace!("Preparing encoders for mp4 file");

        let video = video(&mut output).map_err(InitError::VideoInit)?;
        let audio = audio(&mut output)
            .transpose()
            .map_err(InitError::AudioInit)?;

        info!("Prepared encoders for mp4 file");

        if faststart {
            let mut opts = ffmpeg::Dictionary::new();
            opts.set("movflags", "+faststart");
            output
                .write_header_with(opts)
                .map(|_| ())
                .map_err(InitError::Ffmpeg)?;
        } else {
            output.write_header().map_err(InitError::Ffmpeg)?;
        }

        Ok(Self {
            tag,
            output,
            video,
            audio,
            is_finished: false,
        })
    }

    pub fn video_format() -> RawVideoFormat {
        RawVideoFormat::Yuv420p
    }

    pub fn video_encoder(&self) -> Option<&H264Encoder> {
        match &self.video {
            Mp4VideoEncoder::H264(encoder) => Some(encoder),
            Mp4VideoEncoder::Hevc(_) => None,
        }
    }

    pub fn set_video_packet_stats(&mut self, stats: std::sync::Arc<crate::EncodedPacketStats>) {
        match &mut self.video {
            Mp4VideoEncoder::H264(encoder) => encoder.set_packet_stats(stats),
            Mp4VideoEncoder::Hevc(encoder) => encoder.set_packet_stats(stats),
        }
    }

    pub fn queue_video_frame(
        &mut self,
        frame: frame::Video,
        timestamp: Duration,
    ) -> Result<(), h264::QueueFrameError> {
        if self.is_finished {
            return Ok(());
        }

        match &mut self.video {
            Mp4VideoEncoder::H264(encoder) => {
                encoder.queue_frame(frame, timestamp, &mut self.output)
            }
            Mp4VideoEncoder::Hevc(encoder) => encoder
                .queue_frame(frame, timestamp, &mut self.output)
                .map_err(Into::into),
        }
    }

    pub fn queue_video_frame_reusable(
        &mut self,
        frame: &mut frame::Video,
        converted_frame: &mut Option<frame::Video>,
        timestamp: Duration,
    ) -> Result<(), h264::QueueFrameError> {
        if self.is_finished {
            return Ok(());
        }

        match &mut self.video {
            Mp4VideoEncoder::H264(encoder) => {
                encoder.queue_frame_reusable(frame, converted_frame, timestamp, &mut self.output)
            }
            Mp4VideoEncoder::Hevc(encoder) => encoder
                .queue_frame_reusable(frame, converted_frame, timestamp, &mut self.output)
                .map_err(Into::into),
        }
    }

    pub fn queue_audio_frame(&mut self, frame: frame::Audio) {
        if self.is_finished {
            return;
        }

        let Some(audio) = &mut self.audio else {
            return;
        };

        audio.send_frame(frame, &mut self.output);
    }

    pub fn try_queue_audio_frame(&mut self, frame: frame::Audio) -> Result<(), ffmpeg::Error> {
        if self.is_finished {
            return Err(ffmpeg::Error::Eof);
        }

        let Some(audio) = &mut self.audio else {
            return Err(ffmpeg::Error::StreamNotFound);
        };

        audio.try_send_frame(frame, &mut self.output)
    }

    pub fn stream_time_base(&self, stream_index: usize) -> ffmpeg::Rational {
        self.output.stream_time_base(stream_index)
    }

    pub fn write_packets(
        &mut self,
        packets: impl IntoIterator<Item = ffmpeg::Packet>,
    ) -> Result<(), ffmpeg::Error> {
        if self.is_finished {
            return Err(ffmpeg::Error::Eof);
        }

        for packet in packets {
            self.output.write_packet(packet)?;
        }
        Ok(())
    }

    /// Hands back an audio encoder that ran off this file's thread so
    /// [`Self::finish`] flushes it after the video encoder, as it would
    /// have if the file had owned it throughout.
    pub fn attach_audio(&mut self, audio: Box<dyn AudioEncoder + Send>) {
        self.audio = Some(audio);
    }

    pub fn finish(&mut self) -> Result<FinishResult, FinishError> {
        if self.is_finished {
            return Err(FinishError::AlreadyFinished);
        }

        self.is_finished = true;

        tracing::info!("MP4Encoder: Finishing encoding");

        let video_finish = match &mut self.video {
            Mp4VideoEncoder::H264(encoder) => encoder.flush(&mut self.output),
            Mp4VideoEncoder::Hevc(encoder) => encoder.flush(&mut self.output),
        }
        .inspect_err(|e| {
            error!("Failed to finish video encoder: {e:#}");
        });

        let audio_finish = self
            .audio
            .as_mut()
            .map(|enc| {
                tracing::info!("MP4Encoder: Flushing audio encoder");
                enc.flush(&mut self.output).inspect_err(|e| {
                    error!("Failed to finish audio encoder: {e:#}");
                })
            })
            .unwrap_or(Ok(()));

        tracing::info!("MP4Encoder: Writing trailer");
        self.output
            .write_trailer()
            .map_err(FinishError::WriteTrailerFailed)?;

        Ok(FinishResult {
            video_finish,
            audio_finish,
        })
    }

    pub fn video(&self) -> Option<&H264Encoder> {
        self.video_encoder()
    }

    pub fn video_mut(&mut self) -> Option<&mut H264Encoder> {
        match &mut self.video {
            Mp4VideoEncoder::H264(encoder) => Some(encoder),
            Mp4VideoEncoder::Hevc(_) => None,
        }
    }
}

impl Drop for MP4File {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

pub struct MP4Input {
    pub video: frame::Video,
    pub audio: Option<frame::Audio>,
}

unsafe impl Send for H264Encoder {}
unsafe impl Send for HevcEncoder {}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_media_info::VideoInfo;

    fn video_info() -> VideoInfo {
        VideoInfo {
            pixel_format: frameever_media_info::Pixel::NV12,
            width: 320,
            height: 240,
            time_base: ffmpeg::Rational(1, 1_000_000),
            frame_rate: ffmpeg::Rational(30, 1),
        }
    }

    fn gray_frame() -> frame::Video {
        let mut frame = frame::Video::new(ffmpeg::format::Pixel::NV12, 320, 240);
        for plane in 0..frame.planes() {
            frame.data_mut(plane).fill(128);
        }
        frame
    }

    #[test]
    fn av1_files_hold_an_av1_stream_with_every_frame() {
        ffmpeg::init().ok();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("av1.mp4");
        let mut file = MP4File::init_hevc(
            "av1-test",
            path.clone(),
            false,
            |o| HevcEncoder::builder(video_info()).with_av1().build(o),
            |_| None,
        )
        .unwrap();
        let mut converted = None;
        for index in 0..30_u64 {
            let mut frame = gray_frame();
            file.queue_video_frame_reusable(
                &mut frame,
                &mut converted,
                Duration::from_nanos(index * 1_000_000_000 / 30),
            )
            .unwrap();
        }
        file.finish().unwrap().video_finish.unwrap();

        let input = ffmpeg::format::input(&path).unwrap();
        let stream = input.streams().best(ffmpeg::media::Type::Video).unwrap();
        assert_eq!(stream.parameters().id(), ffmpeg::codec::Id::AV1);
        assert_eq!(stream.frames(), 30);
    }

    #[test]
    fn hevc_files_hold_an_hevc_stream_with_every_frame() {
        ffmpeg::init().ok();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hevc.mp4");
        let mut file = MP4File::init_hevc(
            "hevc-test",
            path.clone(),
            false,
            |o| {
                HevcEncoder::builder(video_info())
                    .with_external_conversion()
                    .build(o)
            },
            |_| None,
        )
        .unwrap();
        assert!(file.video_encoder().is_none());

        let mut converted = None;
        for index in 0..30_u64 {
            let mut frame = gray_frame();
            file.queue_video_frame_reusable(
                &mut frame,
                &mut converted,
                Duration::from_nanos(index * 1_000_000_000 / 30),
            )
            .unwrap();
        }
        let result = file.finish().unwrap();
        result.video_finish.unwrap();

        let input = ffmpeg::format::input(&path).unwrap();
        let stream = input.streams().best(ffmpeg::media::Type::Video).unwrap();
        assert_eq!(stream.parameters().id(), ffmpeg::codec::Id::HEVC);
        let tag = unsafe { (*stream.parameters().as_ptr()).codec_tag };
        assert_eq!(&tag.to_le_bytes(), b"hvc1");
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .unwrap()
            .decoder()
            .video()
            .unwrap();
        assert_eq!((decoder.width(), decoder.height()), (320, 240));
    }
}
