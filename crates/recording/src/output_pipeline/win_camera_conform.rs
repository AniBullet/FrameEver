use super::{NativeCameraFrame, camera_frame_to_ffmpeg};
use anyhow::anyhow;
use frameever_camera_windows::PixelFormat;
use frameever_mediafoundation_utils::IMFMediaBufferExt;
use std::sync::{Arc, Mutex};
use windows::Win32::Media::MediaFoundation::MFCreateMemoryBuffer;

#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameShape {
    pixel_format: PixelFormat,
    width: u32,
    height: u32,
}

impl FrameShape {
    fn of(frame: &NativeCameraFrame) -> Self {
        Self {
            pixel_format: frame.pixel_format,
            width: frame.width,
            height: frame.height,
        }
    }

    fn ffmpeg_pixel(&self) -> Option<ffmpeg::format::Pixel> {
        Some(match self.pixel_format {
            PixelFormat::NV12 => ffmpeg::format::Pixel::NV12,
            PixelFormat::YUYV422 => ffmpeg::format::Pixel::YUYV422,
            PixelFormat::UYVY422 => ffmpeg::format::Pixel::UYVY422,
            PixelFormat::ARGB | PixelFormat::RGB32 => ffmpeg::format::Pixel::BGRA,
            _ => return None,
        })
    }

    fn plane_rows(&self) -> Vec<(usize, usize)> {
        let width = self.width as usize;
        let height = self.height as usize;
        match self.pixel_format {
            PixelFormat::NV12 => vec![(width, height), (width, height / 2)],
            PixelFormat::YUYV422 | PixelFormat::UYVY422 => vec![(width * 2, height)],
            _ => vec![(width * 4, height)],
        }
    }
}

struct Scaler {
    source: (ffmpeg::format::Pixel, u32, u32),
    context: ffmpeg::software::scaling::Context,
}

unsafe impl Send for Scaler {}

#[derive(Default)]
pub struct CameraFrameConformer {
    track: Option<FrameShape>,
    scaler: Option<Scaler>,
    failures: u64,
}

impl CameraFrameConformer {
    pub fn track_writable(&self) -> Option<bool> {
        self.track.map(|track| track.ffmpeg_pixel().is_some())
    }

    pub fn conform(&mut self, frame: NativeCameraFrame) -> Option<NativeCameraFrame> {
        let shape = FrameShape::of(&frame);
        let track = *self.track.get_or_insert(shape);
        if shape == track {
            return Some(frame);
        }
        match self.convert(&frame, track) {
            Ok(frame) => Some(frame),
            Err(error) => {
                self.failures += 1;
                if self.failures == 1 || self.failures.is_multiple_of(300) {
                    tracing::warn!(
                        failures = self.failures,
                        from = ?shape,
                        to = ?track,
                        "Camera frame could not be converted to the track's format: {error:#}"
                    );
                }
                None
            }
        }
    }

    fn convert(
        &mut self,
        frame: &NativeCameraFrame,
        track: FrameShape,
    ) -> anyhow::Result<NativeCameraFrame> {
        let target = track
            .ffmpeg_pixel()
            .ok_or_else(|| anyhow!("unsupported track format {:?}", track.pixel_format))?;
        let source = camera_frame_to_ffmpeg(frame)?;
        let key = (source.format(), source.width(), source.height());
        let scaler = match self.scaler.take() {
            Some(scaler) if scaler.source == key => scaler,
            _ => Scaler {
                source: key,
                context: ffmpeg::software::scaling::Context::get(
                    key.0,
                    key.1,
                    key.2,
                    target,
                    track.width,
                    track.height,
                    ffmpeg::software::scaling::Flags::BILINEAR,
                )?,
            },
        };
        let scaler = self.scaler.insert(scaler);
        let mut scaled = ffmpeg::frame::Video::empty();
        scaler.context.run(&source, &mut scaled)?;

        let rows = track.plane_rows();
        let len = rows
            .iter()
            .map(|(bytes, count)| bytes * count)
            .sum::<usize>();
        let buffer = unsafe { MFCreateMemoryBuffer(len as u32) }?;
        {
            let mut out = buffer.lock_for_write()?;
            let mut offset = 0;
            for (plane, (row_bytes, row_count)) in rows.into_iter().enumerate() {
                let stride = scaled.stride(plane);
                let data = scaled.data(plane);
                for row in 0..row_count {
                    out[offset..offset + row_bytes]
                        .copy_from_slice(&data[row * stride..row * stride + row_bytes]);
                    offset += row_bytes;
                }
            }
        }
        unsafe { buffer.SetCurrentLength(len as u32) }?;

        #[allow(clippy::arc_with_non_send_sync)]
        let buffer = Arc::new(Mutex::new(buffer));
        Ok(NativeCameraFrame {
            buffer,
            pixel_format: track.pixel_format,
            width: track.width,
            height: track.height,
            is_bottom_up: false,
            timestamp: frame.timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::camera_input::tests::frame_from;

    fn nv12(width: u32, height: u32) -> NativeCameraFrame {
        let len = (width * height * 3 / 2) as usize;
        frame_from(PixelFormat::NV12, width, height, &vec![16; len])
    }

    #[test]
    fn frames_from_a_switched_camera_take_the_tracks_size_and_format() {
        ffmpeg::init().ok();
        let mut conformer = CameraFrameConformer::default();
        let first = nv12(64, 36);
        let first_buffer = first.buffer.clone();
        let passed = conformer.conform(first).unwrap();
        assert_eq!(conformer.track_writable(), Some(true));
        assert!(Arc::ptr_eq(&passed.buffer, &first_buffer));

        let white = frame_from(PixelFormat::RGB32, 96, 54, &vec![255; 96 * 54 * 4]);
        let conformed = conformer.conform(white).unwrap();
        assert_eq!(
            FrameShape::of(&conformed),
            FrameShape {
                pixel_format: PixelFormat::NV12,
                width: 64,
                height: 36,
            }
        );
        let decoded = camera_frame_to_ffmpeg(&conformed).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 36));
        let luma = decoded.data(0)[decoded.stride(0) * 18 + 32];
        assert!(luma > 200, "white stays white, got luma {luma}");

        let again = nv12(64, 36);
        let again_buffer = again.buffer.clone();
        assert!(Arc::ptr_eq(
            &conformer.conform(again).unwrap().buffer,
            &again_buffer
        ));
    }

    #[test]
    fn a_track_format_that_cannot_be_written_drops_the_frame() {
        ffmpeg::init().ok();
        let mut conformer = CameraFrameConformer::default();
        assert_eq!(conformer.track_writable(), None);
        let gray = frame_from(PixelFormat::GRAY8, 64, 36, &[16; 64 * 36]);
        assert!(conformer.conform(gray).is_some());
        assert_eq!(conformer.track_writable(), Some(false));
        assert!(conformer.conform(nv12(96, 54)).is_none());
    }

    #[test]
    #[ignore = "measurement: run with --ignored --optimized"]
    fn conforming_a_switched_camera_to_a_1080p30_track_costs() {
        ffmpeg::init().ok();
        let frames = 300;
        let cases = [
            ("YUYV 1280x720", PixelFormat::YUYV422, 1280, 720, 2),
            ("YUYV 1920x1080", PixelFormat::YUYV422, 1920, 1080, 2),
            ("RGB32 1920x1080", PixelFormat::RGB32, 1920, 1080, 4),
        ];
        for (name, format, width, height, bytes_per_pixel) in cases {
            let mut conformer = CameraFrameConformer::default();
            conformer.conform(nv12(1920, 1080)).unwrap();
            let bytes = vec![128; (width * height * bytes_per_pixel) as usize];
            let inputs = (0..frames)
                .map(|_| frame_from(format, width, height, &bytes))
                .collect::<Vec<_>>();
            conformer
                .conform(frame_from(format, width, height, &bytes))
                .unwrap();
            let started = std::time::Instant::now();
            for frame in inputs {
                assert!(conformer.conform(frame).is_some());
            }
            let per_frame = started.elapsed().as_secs_f64() * 1000.0 / frames as f64;
            println!(
                "{name} -> NV12 1920x1080: {per_frame:.2} ms per frame, {:.0}% of one core at 30 fps",
                per_frame * 30.0 / 10.0
            );
        }
    }
}
