use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use ffmpeg::{
    codec::threading,
    format::Pixel,
    software::scaling::{self, flag::Flags},
};
use gpui::RenderImage;

use super::{PlayError, container_start};

const SHOW_EARLY: f64 = 0.002;
const LATE_AFTER: f64 = 0.05;
const LONGEST_HOLD: Duration = Duration::from_millis(100);
const DECODER_THREADS: usize = 4;

pub fn fit_size(source: (u32, u32), bounds: (u32, u32)) -> (u32, u32) {
    let scale = (bounds.0 as f64 / source.0.max(1) as f64)
        .min(bounds.1 as f64 / source.1.max(1) as f64)
        .min(1.);
    (
        ((source.0 as f64 * scale).round() as u32).max(1),
        ((source.1 as f64 * scale).round() as u32).max(1),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pace {
    Wait(Duration),
    Show,
    Skip,
}

pub fn pace(pts: f64, clock: f64, since_shown: Duration) -> Pace {
    let lead = pts - clock;
    if lead > SHOW_EARLY {
        Pace::Wait(Duration::from_secs_f64(lead))
    } else if lead < -LATE_AFTER && since_shown < LONGEST_HOLD {
        Pace::Skip
    } else {
        Pace::Show
    }
}

pub struct DecodedFrame {
    pub local: f64,
    frame: ffmpeg::frame::Video,
}

struct Scaler {
    context: scaling::Context,
    source: (Pixel, u32, u32),
    target: (u32, u32),
}

pub struct VideoReader {
    input: ffmpeg::format::context::Input,
    stream: usize,
    decoder: ffmpeg::decoder::Video,
    time_base: f64,
    start: f64,
    frame_gap: f64,
    scaler: Option<Scaler>,
    draining: bool,
}

impl VideoReader {
    pub fn open(path: &Path) -> Result<Self, PlayError> {
        if !path.is_file() {
            return Err(PlayError::Missing);
        }
        let unreadable = |error: ffmpeg::Error| {
            tracing::warn!(path = %path.display(), "opening the video failed: {error}");
            PlayError::Unreadable
        };
        let input = ffmpeg::format::input(path).map_err(unreadable)?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(PlayError::Unreadable)?;
        let index = stream.index();
        let time_base = f64::from(stream.time_base());
        let rate = f64::from(stream.avg_frame_rate());
        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(unreadable)?;
        context.set_threading(threading::Config {
            kind: threading::Type::Frame,
            count: DECODER_THREADS,
            ..Default::default()
        });
        let decoder = context.decoder().video().map_err(unreadable)?;
        if decoder.width() == 0 || decoder.height() == 0 {
            return Err(PlayError::Unreadable);
        }
        let start = container_start(&input);
        Ok(Self {
            input,
            stream: index,
            decoder,
            time_base,
            start,
            frame_gap: if rate > 0. { 1. / rate } else { 1. / 30. },
            scaler: None,
            draining: false,
        })
    }

    pub fn frame_gap(&self) -> f64 {
        self.frame_gap
    }

    pub fn seek(&mut self, local: f64) {
        let target = ((local + self.start) * 1_000_000.) as i64;
        if let Err(error) = self.input.seek(target, ..target) {
            tracing::debug!("seeking the video failed: {error}");
        }
        self.decoder.flush();
        self.draining = false;
    }

    pub fn next(&mut self) -> Option<DecodedFrame> {
        let mut frame = ffmpeg::frame::Video::empty();
        loop {
            match self.decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    let local = frame
                        .timestamp()
                        .or_else(|| frame.pts())
                        .map_or(0., |pts| pts as f64 * self.time_base - self.start);
                    return Some(DecodedFrame { local, frame });
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::ffi::EAGAIN => {}
                Err(_) => return None,
            }
            if self.draining {
                return None;
            }
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() == self.stream => {
                    if let Err(error) = self.decoder.send_packet(&packet) {
                        tracing::debug!("skipping a video packet: {error}");
                    }
                }
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => {
                    self.draining = true;
                    if self.decoder.send_eof().is_err() {
                        return None;
                    }
                }
                Err(error) => {
                    tracing::debug!("reading the video failed: {error}");
                    self.draining = true;
                    if self.decoder.send_eof().is_err() {
                        return None;
                    }
                }
            }
        }
    }

    pub fn image(
        &mut self,
        decoded: &DecodedFrame,
        bounds: (u32, u32),
    ) -> Result<Arc<RenderImage>, String> {
        let frame = &decoded.frame;
        let source = (frame.format(), frame.width(), frame.height());
        let target = fit_size((source.1, source.2), bounds);
        let stale = self
            .scaler
            .as_ref()
            .is_none_or(|scaler| scaler.source != source || scaler.target != target);
        if stale {
            let context = scaling::Context::get(
                source.0,
                source.1,
                source.2,
                Pixel::BGRA,
                target.0,
                target.1,
                Flags::BILINEAR,
            )
            .map_err(|error| error.to_string())?;
            self.scaler = Some(Scaler {
                context,
                source,
                target,
            });
        }
        let scaler = self.scaler.as_mut().ok_or("no scaler")?;
        let mut bgra = ffmpeg::frame::Video::empty();
        scaler
            .context
            .run(frame, &mut bgra)
            .map_err(|error| error.to_string())?;
        let (width, height) = (bgra.width() as usize, bgra.height() as usize);
        let bytes = frameever_gpui_kernels::frame::copy_rows(
            bgra.data(0),
            width * 4,
            bgra.stride(0),
            height,
            false,
        )
        .ok_or("the frame's rows don't fit")?;
        let image = image::RgbaImage::from_raw(width as u32, height as u32, bytes)
            .ok_or("the frame's size doesn't match")?;
        Ok(Arc::new(RenderImage::new(smallvec::smallvec![
            image::Frame::new(image)
        ])))
    }
}

pub fn wait_for(lead: Duration, now: Instant, deadline: Instant) -> Duration {
    lead.min(deadline.saturating_duration_since(now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_fit_inside_the_box_and_keep_their_shape() {
        assert_eq!(fit_size((1920, 1080), (630, 354)), (629, 354));
        assert_eq!(fit_size((1080, 1920), (630, 354)), (199, 354));
    }

    #[test]
    fn frames_are_never_decoded_above_their_own_size() {
        assert_eq!(fit_size((320, 180), (630, 354)), (320, 180));
    }

    #[test]
    fn a_frame_ahead_of_the_clock_waits() {
        match pace(1.1, 1., Duration::ZERO) {
            Pace::Wait(wait) => assert!((wait.as_secs_f64() - 0.1).abs() < 1e-9),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_frame_on_time_or_just_late_shows() {
        assert_eq!(pace(1., 1., Duration::ZERO), Pace::Show);
        assert_eq!(pace(1., 1.04, Duration::ZERO), Pace::Show);
    }

    #[test]
    fn a_late_frame_is_skipped_while_the_picture_is_fresh() {
        assert_eq!(pace(1., 1.2, Duration::from_millis(16)), Pace::Skip);
    }

    #[test]
    fn a_late_frame_shows_when_the_picture_has_been_held_too_long() {
        assert_eq!(pace(1., 1.2, LONGEST_HOLD), Pace::Show);
    }

    #[test]
    fn a_wait_stops_at_the_next_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(10);
        assert_eq!(
            wait_for(Duration::from_secs(1), now, deadline),
            Duration::from_millis(10)
        );
        assert_eq!(
            wait_for(Duration::from_millis(1), now, deadline),
            Duration::from_millis(1)
        );
    }
}
