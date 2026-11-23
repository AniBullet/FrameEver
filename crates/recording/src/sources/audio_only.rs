use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ffmpeg::util::rational::Rational as FFRational;
use frameever_media_info::{AudioInfo, Pixel, VideoInfo};
use frameever_timestamp::Timestamp;
use futures::{FutureExt, SinkExt, channel::mpsc, future::BoxFuture};

use crate::output_pipeline::{
    AudioFrame, AudioMuxer, Muxer, SetupCtx, TaskPool, VideoFrame, VideoMuxer, VideoSource,
};

pub use frameever_project::{
    AUDIO_ONLY_CANVAS_ENCODERS, AUDIO_ONLY_CANVAS_FPS, AUDIO_ONLY_CANVAS_HEIGHT,
    AUDIO_ONLY_CANVAS_WIDTH,
};

const TICK: Duration = Duration::from_millis(100);

pub struct AudioOnlyClockConfig;

#[derive(Debug, Clone, Copy)]
pub struct ClockTick {
    timestamp: Timestamp,
}

impl VideoFrame for ClockTick {
    fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    fn duplicate(&self) -> Option<Self> {
        Some(*self)
    }
}

pub struct AudioOnlyClock {
    stop: Arc<AtomicBool>,
}

pub fn audio_only_canvas_info() -> VideoInfo {
    VideoInfo {
        pixel_format: Pixel::YUV420P,
        width: AUDIO_ONLY_CANVAS_WIDTH,
        height: AUDIO_ONLY_CANVAS_HEIGHT,
        time_base: FFRational(1, 1_000_000),
        frame_rate: FFRational(AUDIO_ONLY_CANVAS_FPS as i32, 1),
    }
}

impl VideoSource for AudioOnlyClock {
    type Config = AudioOnlyClockConfig;
    type Frame = ClockTick;

    async fn setup(
        _: Self::Config,
        mut video_tx: mpsc::Sender<Self::Frame>,
        ctx: &mut SetupCtx,
    ) -> anyhow::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = ctx.stop_token();
        ctx.tasks().spawn("audio-only-clock", {
            let stop = stop.clone();
            async move {
                let mut ticks = tokio::time::interval(TICK);
                ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => break,
                        tick = ticks.tick() => {
                            if stop.load(Ordering::Relaxed) {
                                break;
                            }
                            let frame = ClockTick {
                                timestamp: Timestamp::Instant(tick.into_std()),
                            };
                            if video_tx.send(frame).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Ok(())
            }
        });
        Ok(Self { stop })
    }

    fn video_info(&self) -> VideoInfo {
        audio_only_canvas_info()
    }

    fn stop(&mut self) -> BoxFuture<'_, anyhow::Result<()>> {
        self.stop.store(true, Ordering::Relaxed);
        async { Ok(()) }.boxed()
    }
}

pub struct AudioOnlyStandinMuxer {
    output_path: PathBuf,
    first: Option<Duration>,
    last: Option<Duration>,
}

impl Muxer for AudioOnlyStandinMuxer {
    type Config = ();

    async fn setup(
        _: Self::Config,
        output_path: PathBuf,
        _: Option<VideoInfo>,
        _: Option<AudioInfo>,
        _: Arc<AtomicBool>,
        _: &mut TaskPool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            output_path,
            first: None,
            last: None,
        })
    }

    fn finish(&mut self, _: Duration) -> anyhow::Result<anyhow::Result<()>> {
        let (Some(first), Some(last)) = (self.first, self.last) else {
            return Ok(Ok(()));
        };
        Ok(standin_for_span(
            &self.output_path,
            last.saturating_sub(first),
        ))
    }
}

impl AudioMuxer for AudioOnlyStandinMuxer {
    fn send_audio_frame(&mut self, _: AudioFrame, _: Duration) -> anyhow::Result<()> {
        Ok(())
    }
}

impl VideoMuxer for AudioOnlyStandinMuxer {
    type VideoFrame = ClockTick;

    fn send_video_frame(&mut self, _: Self::VideoFrame, timestamp: Duration) -> anyhow::Result<()> {
        self.first.get_or_insert(timestamp);
        self.last = Some(timestamp);
        Ok(())
    }
}

pub fn standin_for_span(path: &std::path::Path, duration: Duration) -> anyhow::Result<()> {
    frameever_enc_ffmpeg::standin::write_black_standin(
        path,
        AUDIO_ONLY_CANVAS_WIDTH,
        AUDIO_ONLY_CANVAS_HEIGHT,
        AUDIO_ONLY_CANVAS_FPS,
        duration,
        AUDIO_ONLY_CANVAS_ENCODERS,
    )
    .map_err(|error| anyhow::anyhow!("Could not write the audio-only display track: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn muxer(path: PathBuf) -> AudioOnlyStandinMuxer {
        AudioOnlyStandinMuxer {
            output_path: path,
            first: None,
            last: None,
        }
    }

    #[test]
    fn the_muxer_writes_one_standin_spanning_the_segment() {
        ffmpeg::init().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("display.mp4");
        let mut muxer = muxer(path.clone());
        for ms in (200u64..=3_200).step_by(100) {
            let tick = ClockTick {
                timestamp: Timestamp::Instant(std::time::Instant::now()),
            };
            muxer
                .send_video_frame(tick, Duration::from_millis(ms))
                .unwrap();
        }
        assert!(!path.exists(), "nothing is written while recording");
        muxer.finish(Duration::from_millis(4_100)).unwrap().unwrap();
        let input = ffmpeg::format::input(&path).unwrap();
        let seconds = input.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE);
        assert!((seconds - 3.0).abs() < 0.05, "{seconds}");
    }

    #[test]
    fn a_segment_without_ticks_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("display.mp4");
        muxer(path.clone())
            .finish(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(!path.exists());
    }
}
