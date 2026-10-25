use std::{path::Path, time::Duration};

use frameever_enc_ffmpeg::{
    AudioEncoder,
    aac::AACEncoder,
    hevc::{HevcEncoder, HevcPreset},
    mp4::MP4File,
};
use frameever_media_info::{AudioInfo, VideoInfo};

pub const COMPRESSION_BPP: f32 = 0.06;

pub fn compress_to_hevc(
    input_path: &Path,
    output_path: &Path,
    mut progress: impl FnMut(u64, u64) -> bool,
) -> Result<(), String> {
    let mut input = ffmpeg::format::input(input_path)
        .map_err(|error| format!("Could not open the video: {error}"))?;

    let video_stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| "The file has no video".to_string())?;
    let video_index = video_stream.index();
    let video_time_base = video_stream.time_base();
    let duration = input.duration().max(0) as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE);
    let frame_rate = video_stream.avg_frame_rate();
    let fps = if frame_rate.1 > 0 && frame_rate.0 > 0 {
        (f64::from(frame_rate.0) / f64::from(frame_rate.1)).round() as u32
    } else {
        30
    };
    let total_frames = match video_stream.frames() {
        frames if frames > 0 => frames as u64,
        _ => (duration * f64::from(fps)).round().max(1.) as u64,
    };

    let mut video_decoder =
        ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
            .and_then(|context| context.decoder().video())
            .map_err(|error| format!("Could not decode the video: {error}"))?;

    let audio_stream = input.streams().best(ffmpeg::media::Type::Audio);
    let audio_index = audio_stream.as_ref().map(|stream| stream.index());
    let mut audio_decoder = audio_stream.and_then(|stream| {
        ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .and_then(|context| context.decoder().audio())
            .ok()
    });
    let audio_info = audio_decoder.as_ref().and_then(|decoder| {
        AudioInfo::new(decoder.format(), decoder.rate(), decoder.channels()).ok()
    });

    let video_info = VideoInfo::from_raw_ffmpeg(
        video_decoder.format(),
        video_decoder.width(),
        video_decoder.height(),
        fps,
    );

    let mut file = MP4File::init_hevc(
        "compress",
        output_path.to_path_buf(),
        true,
        |output| {
            HevcEncoder::builder(video_info)
                .with_bpp(COMPRESSION_BPP)
                .with_preset(HevcPreset::Medium)
                .build(output)
        },
        |output| {
            let info = audio_info?;
            Some(
                AACEncoder::init(info, output)
                    .map(|encoder| encoder.boxed())
                    .map_err(Into::into),
            )
        },
    )
    .map_err(|error| format!("Could not start the encoder: {error}"))?;

    let seconds = |pts: Option<i64>| {
        pts.filter(|_| video_time_base.1 > 0).map(|pts| {
            Duration::from_secs_f64(
                (pts as f64 * f64::from(video_time_base.0) / f64::from(video_time_base.1)).max(0.),
            )
        })
    };

    let mut encoded = 0_u64;
    let mut decoded = ffmpeg::frame::Video::empty();
    let mut converted = None;
    let mut last = Duration::ZERO;
    let mut encode_ready = |decoder: &mut ffmpeg::decoder::Video,
                            file: &mut MP4File,
                            encoded: &mut u64|
     -> Result<(), String> {
        while decoder.receive_frame(&mut decoded).is_ok() {
            let fallback = Duration::from_secs_f64(*encoded as f64 / f64::from(fps));
            let timestamp = seconds(decoded.pts()).unwrap_or(fallback).max(last);
            last = timestamp;
            file.queue_video_frame_reusable(&mut decoded, &mut converted, timestamp)
                .map_err(|error| format!("Could not encode a frame: {error}"))?;
            *encoded += 1;
        }
        Ok(())
    };

    if !progress(0, total_frames) {
        return Err(CANCELLED.to_string());
    }

    let mut decoded_audio = ffmpeg::frame::Audio::empty();
    for (stream, packet) in input.packets() {
        if stream.index() == video_index {
            video_decoder
                .send_packet(&packet)
                .map_err(|error| format!("Could not decode the video: {error}"))?;
            encode_ready(&mut video_decoder, &mut file, &mut encoded)?;
            if !progress(encoded.min(total_frames), total_frames) {
                return Err(CANCELLED.to_string());
            }
        } else if Some(stream.index()) == audio_index
            && let Some(decoder) = audio_decoder.as_mut()
            && decoder.send_packet(&packet).is_ok()
        {
            while decoder.receive_frame(&mut decoded_audio).is_ok() {
                file.queue_audio_frame(decoded_audio.clone());
            }
        }
    }

    video_decoder.send_eof().ok();
    encode_ready(&mut video_decoder, &mut file, &mut encoded)?;
    if let Some(decoder) = audio_decoder.as_mut() {
        decoder.send_eof().ok();
        while decoder.receive_frame(&mut decoded_audio).is_ok() {
            file.queue_audio_frame(decoded_audio.clone());
        }
    }

    let finished = file
        .finish()
        .map_err(|error| format!("Could not finish the file: {error}"))?;
    finished
        .video_finish
        .map_err(|error| format!("Could not finish the video: {error}"))?;
    if encoded == 0 {
        return Err("The video has no frames".to_string());
    }
    progress(total_frames, total_frames);
    Ok(())
}

pub const CANCELLED: &str = "Compression cancelled";

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_enc_ffmpeg::h264::H264Encoder;

    fn write_h264(path: &Path, frames: u64) {
        let info = VideoInfo {
            pixel_format: frameever_media_info::Pixel::NV12,
            width: 320,
            height: 240,
            time_base: ffmpeg::Rational(1, 1_000_000),
            frame_rate: ffmpeg::Rational(30, 1),
        };
        let mut file = MP4File::init(
            "source",
            path.to_path_buf(),
            false,
            |output| H264Encoder::builder(info).build(output),
            |_| None,
        )
        .unwrap();
        for index in 0..frames {
            let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, 320, 240);
            for plane in 0..frame.planes() {
                frame.data_mut(plane).fill((index * 7 % 255) as u8);
            }
            file.queue_video_frame(frame, Duration::from_nanos(index * 1_000_000_000 / 30))
                .unwrap();
        }
        file.finish().unwrap();
    }

    #[test]
    fn compression_writes_an_hevc_copy_with_every_frame_and_reports_progress() {
        ffmpeg::init().ok();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("output.mp4");
        let target = dir.path().join("output_compressed.mp4");
        write_h264(&source, 45);

        let mut reports = Vec::new();
        compress_to_hevc(&source, &target, |done, total| {
            reports.push((done, total));
            true
        })
        .unwrap();

        let input = ffmpeg::format::input(&target).unwrap();
        let stream = input.streams().best(ffmpeg::media::Type::Video).unwrap();
        assert_eq!(stream.parameters().id(), ffmpeg::codec::Id::HEVC);
        assert_eq!(stream.frames(), 45);
        assert_eq!(reports.first(), Some(&(0, 45)));
        assert_eq!(reports.last(), Some(&(45, 45)));
    }

    #[test]
    fn compression_stops_when_asked() {
        ffmpeg::init().ok();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("output.mp4");
        write_h264(&source, 10);
        let error =
            compress_to_hevc(&source, &dir.path().join("out.mp4"), |_, _| false).unwrap_err();
        assert_eq!(error, CANCELLED);
    }
}
