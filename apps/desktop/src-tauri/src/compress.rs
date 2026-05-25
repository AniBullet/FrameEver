use crate::FramesRendered;
use cap_enc_ffmpeg::{AudioEncoder, aac::AACEncoder, hevc::HevcEncoder, mp4::HevcMP4File};
use cap_media_info::VideoInfo;
use std::path::PathBuf;
use std::time::Duration;
use tauri::ipc::Channel;
use tracing::{info, warn};

#[tauri::command]
#[specta::specta]
pub async fn compress_video(
    input_path: String,
    crf: u8,
    progress: Channel<FramesRendered>,
) -> Result<String, String> {
    let input_path = PathBuf::from(&input_path);

    if !input_path.exists() {
        return Err(format!("Input file not found: {}", input_path.display()));
    }

    let output_path = compressed_path_for(&input_path);
    let temp_path = input_path.with_extension("_compressing.mp4");

    let input_clone = input_path.clone();
    let temp_clone = temp_path.clone();
    tokio::task::spawn_blocking(move || {
        compress_video_blocking(&input_clone, &temp_clone, crf, Some(&progress))
    })
    .await
    .map_err(|e| e.to_string())??;

    std::fs::rename(&temp_path, &output_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!("Failed to move compressed file: {e}")
    })?;

    Ok(output_path.to_string_lossy().into_owned())
}

pub fn compressed_path_for(original: &std::path::Path) -> PathBuf {
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    original.with_file_name(format!("{stem}_compressed.mp4"))
}

pub fn compress_video_blocking(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    crf: u8,
    progress: Option<&Channel<FramesRendered>>,
) -> Result<String, String> {
    let mut input =
        ffmpeg::format::input(input_path).map_err(|e| format!("Failed to open input: {e}"))?;

    let video_stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or("No video stream found")?;

    let video_stream_index = video_stream.index();
    let video_time_base = video_stream.time_base();
    let total_frames = video_stream.frames() as u32;

    let decoder_params = video_stream.parameters();
    let video_codec = ffmpeg::codec::context::Context::from_parameters(decoder_params.clone())
        .map_err(|e| format!("Codec context: {e}"))?;
    let mut video_decoder = video_codec
        .decoder()
        .video()
        .map_err(|e| format!("Open decoder: {e}"))?;

    let width = video_decoder.width();
    let height = video_decoder.height();
    let fps_rational = video_stream.avg_frame_rate();
    let fps = if fps_rational.1 > 0 {
        (fps_rational.0 as f64 / fps_rational.1 as f64).round() as u32
    } else {
        30
    };

    let audio_stream = input.streams().best(ffmpeg::media::Type::Audio);
    let audio_stream_index = audio_stream.as_ref().map(|s| s.index());
    let mut audio_decoder = audio_stream.and_then(|s| {
        ffmpeg::codec::context::Context::from_parameters(s.parameters())
            .ok()
            .and_then(|ctx| ctx.decoder().audio().ok())
    });
    let has_audio = audio_decoder.is_some();

    let audio_media_info = audio_decoder.as_ref().and_then(|dec| {
        cap_media_info::AudioInfo::new(dec.format(), dec.rate(), dec.channels()).ok()
    });

    info!(
        width = width,
        height = height,
        fps = fps,
        total_frames = total_frames,
        "Compressing video with HEVC CRF {crf}"
    );

    send_progress(progress, 0, total_frames);

    let video_info = VideoInfo::from_raw(cap_media_info::RawVideoFormat::Nv12, width, height, fps);

    let audio_info_for_encoder = audio_media_info;
    let mut muxer = HevcMP4File::init(
        "compress",
        output_path.to_path_buf(),
        |o| {
            HevcEncoder::builder(video_info)
                .with_crf(crf)
                .with_external_conversion()
                .build(o)
        },
        |o| {
            if !has_audio {
                return None;
            }
            let info = audio_info_for_encoder?;
            Some(
                AACEncoder::init(info, o)
                    .map(|v| v.boxed())
                    .map_err(Into::into),
            )
        },
    )
    .map_err(|e| format!("Failed to initialize encoder: {e}"))?;

    let mut scaler = ffmpeg::software::scaling::Context::get(
        video_decoder.format(),
        width,
        height,
        ffmpeg::format::Pixel::NV12,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .map_err(|e| format!("Failed to create scaler: {e}"))?;

    let mut audio_resampler = audio_decoder.as_ref().and_then(|dec| {
        audio_media_info.as_ref().map(|info| {
            ffmpeg::software::resampling::Context::get(
                dec.format(),
                dec.channel_layout(),
                dec.rate(),
                info.sample_format,
                dec.channel_layout(),
                dec.rate(),
            )
            .expect("audio resampler")
        })
    });

    let mut encoded_frames = 0u32;
    let mut nv12_frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, width, height);
    let mut converted_frame: Option<ffmpeg::frame::Video> = None;

    let report_interval = (total_frames / 100).max(1);

    for (stream, packet) in input.packets() {
        if stream.index() == video_stream_index {
            video_decoder
                .send_packet(&packet)
                .map_err(|e| format!("Send packet: {e}"))?;

            let mut decoded_frame = ffmpeg::frame::Video::empty();
            while video_decoder.receive_frame(&mut decoded_frame).is_ok() {
                scaler
                    .run(&decoded_frame, &mut nv12_frame)
                    .map_err(|e| format!("Scale frame: {e}"))?;

                let timestamp = if let Some(pts) = decoded_frame.pts() {
                    if video_time_base.1 > 0 {
                        Duration::from_secs_f64(
                            pts as f64 * video_time_base.0 as f64 / video_time_base.1 as f64,
                        )
                    } else {
                        Duration::from_secs_f64(encoded_frames as f64 / fps as f64)
                    }
                } else {
                    Duration::from_secs_f64(encoded_frames as f64 / fps as f64)
                };

                nv12_frame.set_pts(Some(encoded_frames as i64));

                muxer
                    .queue_video_frame_reusable(&mut nv12_frame, &mut converted_frame, timestamp)
                    .map_err(|e| format!("Queue video frame: {e}"))?;

                encoded_frames += 1;

                if encoded_frames.is_multiple_of(report_interval) {
                    send_progress(progress, encoded_frames.min(total_frames), total_frames);
                }
            }
        } else if Some(stream.index()) == audio_stream_index
            && let Some(ref mut dec) = audio_decoder
        {
            dec.send_packet(&packet).ok();
            let mut decoded_audio = ffmpeg::frame::Audio::empty();
            while dec.receive_frame(&mut decoded_audio).is_ok() {
                if let Some(ref mut resampler) = audio_resampler {
                    let mut resampled = ffmpeg::frame::Audio::empty();
                    if resampler.run(&decoded_audio, &mut resampled).is_ok()
                        && resampled.samples() > 0
                    {
                        resampled.set_pts(decoded_audio.pts());
                        muxer.queue_audio_frame(resampled);
                    }
                } else {
                    muxer.queue_audio_frame(decoded_audio.clone());
                }
            }
        }
    }

    video_decoder.send_eof().ok();
    let mut decoded_frame = ffmpeg::frame::Video::empty();
    while video_decoder.receive_frame(&mut decoded_frame).is_ok() {
        scaler.run(&decoded_frame, &mut nv12_frame).ok();
        let timestamp = Duration::from_secs_f64(encoded_frames as f64 / fps as f64);
        nv12_frame.set_pts(Some(encoded_frames as i64));
        muxer
            .queue_video_frame_reusable(&mut nv12_frame, &mut converted_frame, timestamp)
            .ok();
        encoded_frames += 1;
    }

    if let Some(ref mut dec) = audio_decoder {
        dec.send_eof().ok();
        let mut decoded_audio = ffmpeg::frame::Audio::empty();
        while dec.receive_frame(&mut decoded_audio).is_ok() {
            if let Some(ref mut resampler) = audio_resampler {
                let mut resampled = ffmpeg::frame::Audio::empty();
                if resampler.run(&decoded_audio, &mut resampled).is_ok() && resampled.samples() > 0
                {
                    resampled.set_pts(decoded_audio.pts());
                    muxer.queue_audio_frame(resampled);
                }
            }
        }
    }

    let res = muxer
        .finish()
        .map_err(|e| format!("Failed to finish encoding: {e}"))?;

    if let Err(e) = res.video_finish {
        warn!("Video flush warning: {e}");
    }
    if let Err(e) = res.audio_finish {
        warn!("Audio flush warning: {e}");
    }

    send_progress(progress, total_frames, total_frames);

    info!(encoded_frames = encoded_frames, "Compression complete");

    Ok(output_path.to_string_lossy().into_owned())
}

fn send_progress(
    progress: Option<&Channel<FramesRendered>>,
    rendered_count: u32,
    total_frames: u32,
) {
    if let Some(ch) = progress {
        let _ = ch.send(FramesRendered {
            rendered_count,
            total_frames,
        });
    }
}
