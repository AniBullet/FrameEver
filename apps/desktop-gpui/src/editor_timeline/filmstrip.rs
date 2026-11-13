use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use gpui::RenderImage;

use crate::library::player::video::VideoReader;

pub const FRAME_BOUNDS: (u32, u32) = (160, 90);
const MAX_FRAMES: usize = 180;
const MIN_SPACING: f64 = 2.;

pub fn sample_times(duration: f64) -> Vec<f64> {
    if !duration.is_finite() || duration <= 0. {
        return Vec::new();
    }
    let count = ((duration / MIN_SPACING).ceil() as usize).clamp(1, MAX_FRAMES);
    (0..count)
        .map(|index| duration * (index as f64 + 0.5) / count as f64)
        .collect()
}

pub fn source_time(
    local: f64,
    output_duration: f64,
    holds: &[(f64, f64)],
    source_start: f64,
    source_duration: f64,
) -> f64 {
    let held = |until: f64| -> f64 {
        holds
            .iter()
            .map(|(start, end)| (end.min(until) - start).max(0.))
            .sum()
    };
    let local = local.clamp(0., output_duration);
    let playing = (output_duration - held(output_duration)).max(f64::EPSILON);
    let progressed = (local - held(local)).clamp(0., playing);
    source_start + progressed / playing * source_duration
}

pub struct Filmstrip<T> {
    frames: Vec<(f64, T)>,
    pub aspect: f32,
}

impl<T> std::fmt::Debug for Filmstrip<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Filmstrip")
            .field("frames", &self.frames.len())
            .field("aspect", &self.aspect)
            .finish()
    }
}

impl<T> Filmstrip<T> {
    pub fn new(mut frames: Vec<(f64, T)>, aspect: f32) -> Self {
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self { frames, aspect }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn at(&self, time: f64) -> Option<&T> {
        let after = self.frames.partition_point(|(at, _)| *at < time);
        let candidates = [after.checked_sub(1), Some(after)];
        candidates
            .into_iter()
            .flatten()
            .filter_map(|index| self.frames.get(index))
            .min_by(|a, b| (a.0 - time).abs().total_cmp(&(b.0 - time).abs()))
            .map(|(_, frame)| frame)
    }
}

pub type Frames = Filmstrip<Arc<RenderImage>>;

#[derive(Debug, Default)]
pub struct DecodeStop(Arc<AtomicBool>);

impl DecodeStop {
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.0.clone()
    }
}

impl Drop for DecodeStop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub fn missing(sources: &[(PathBuf, f64)], present: impl Fn(&Path) -> bool) -> Vec<(PathBuf, f64)> {
    sources
        .iter()
        .filter(|(path, _)| !present(path))
        .cloned()
        .collect()
}

pub fn decode(path: &Path, duration: f64, stop: &AtomicBool) -> Option<Frames> {
    let mut reader = VideoReader::open_light(path).ok()?;
    let mut frames: Vec<(f64, Arc<RenderImage>)> = Vec::new();
    let mut aspect = 16. / 9.;
    for time in sample_times(duration) {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        reader.seek(time);
        let Some(decoded) = reader.next() else {
            continue;
        };
        if frames
            .last()
            .is_some_and(|(local, _)| (*local - decoded.local).abs() < 1e-6)
        {
            continue;
        }
        let Ok(image) = reader.image(&decoded, FRAME_BOUNDS) else {
            continue;
        };
        let size = image.size(0);
        if size.height.0 > 0 {
            aspect = size.width.0 as f32 / size.height.0 as f32;
        }
        frames.push((decoded.local, image));
    }
    (!frames.is_empty()).then(|| Filmstrip::new(frames, aspect))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_a_keyframe_apart_until_the_cap() {
        assert_eq!(sample_times(8.), [1., 3., 5., 7.]);
        assert_eq!(sample_times(0.4), [0.2]);
        assert_eq!(sample_times(3600.).len(), MAX_FRAMES);
        assert!(sample_times(0.).is_empty());
        assert!(sample_times(f64::NAN).is_empty());
    }

    #[test]
    fn a_tile_shows_the_frame_nearest_its_time() {
        let strip = Filmstrip::new(vec![(4., "c"), (0., "a"), (2., "b")], 1.);
        assert_eq!(strip.at(-3.), Some(&"a"));
        assert_eq!(strip.at(0.9), Some(&"a"));
        assert_eq!(strip.at(1.1), Some(&"b"));
        assert_eq!(strip.at(3.5), Some(&"c"));
        assert_eq!(strip.at(99.), Some(&"c"));
        assert_eq!(Filmstrip::<&str>::new(Vec::new(), 1.).at(1.), None);
    }

    #[test]
    fn a_tile_maps_through_speed_and_holds_into_the_recording() {
        assert_eq!(source_time(5., 10., &[], 20., 20.), 30.);
        assert_eq!(source_time(-1., 10., &[], 20., 20.), 20.);
        let holds = [(2., 4.)];
        assert_eq!(source_time(3., 10., &holds, 0., 8.), 2.);
        assert_eq!(source_time(10., 10., &holds, 0., 8.), 8.);
        assert_eq!(source_time(6., 10., &holds, 0., 8.), 4.);
    }

    #[test]
    fn decoding_stops_once_its_editor_is_gone() {
        let stop = DecodeStop::default();
        let flag = stop.flag();
        assert!(!flag.load(Ordering::Relaxed));
        drop(stop);
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn only_recordings_without_a_strip_are_decoded() {
        let a = std::path::PathBuf::from("a.mp4");
        let b = std::path::PathBuf::from("b.mp4");
        let sources = vec![(a.clone(), 4.), (b.clone(), 6.)];
        assert_eq!(missing(&sources, |path| path == a.as_path()), vec![(b, 6.)]);
    }

    #[test]
    fn measure_filmstrip_cost() {
        let Some(file) = std::env::var_os("FRAMEEVER_FILMSTRIP_FILE").map(std::path::PathBuf::from)
        else {
            return;
        };
        let duration = std::env::var("FRAMEEVER_FILMSTRIP_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(60.);
        for run in 0..3 {
            let started = std::time::Instant::now();
            let strip = decode(&file, duration, &AtomicBool::new(false)).expect("a filmstrip");
            let elapsed = started.elapsed();
            let bytes: usize = strip
                .frames
                .iter()
                .map(|(_, frame)| frame.as_bytes(0).map_or(0, <[u8]>::len))
                .sum();
            println!(
                "filmstrip {run}: {:.0} ms for {} of {} samples, {:.2} MB",
                elapsed.as_secs_f64() * 1000.,
                strip.len(),
                sample_times(duration).len(),
                bytes as f64 / 1_048_576.
            );
        }
    }
}
