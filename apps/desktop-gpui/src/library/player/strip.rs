use std::{path::PathBuf, sync::Arc};

use gpui::RenderImage;

use super::{
    Hold, PlayError, Segment, probe_duration,
    timeline::Timeline,
    video::{DecodedFrame, VideoReader},
};
use crate::library::view::{Compression, Entry};

pub const STRIP_FRAMES: usize = 20;
pub const LINE_STEPS: f32 = 40.;
const STRIP_BOUNDS: (u32, u32) = (400, 225);
const MAX_STRIPS: usize = 12;

pub fn frame_at(fraction: f32, count: usize) -> usize {
    ((fraction.clamp(0., 1.) * count as f32) as usize).min(count.saturating_sub(1))
}

pub fn line_at(fraction: f32) -> f32 {
    (fraction.clamp(0., 1.) * LINE_STEPS).round() / LINE_STEPS
}

fn sample_points(duration: f64, count: usize) -> Vec<f64> {
    (0..count)
        .map(|index| duration * (index as f64 + 0.5) / count as f64)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripKey {
    path: PathBuf,
    size: u64,
    compression: Compression,
    export: Option<PathBuf>,
}

impl StripKey {
    pub fn of(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            size: entry.size,
            compression: entry.compression,
            export: entry.latest_export().map(|export| export.path.clone()),
        }
    }
}

pub struct Strips<T> {
    held: Vec<(StripKey, Vec<T>)>,
    failed: Vec<StripKey>,
}

impl<T> Default for Strips<T> {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            failed: Vec::new(),
        }
    }
}

impl<T> Strips<T> {
    pub fn get(&self, key: &StripKey) -> Option<&[T]> {
        self.held
            .iter()
            .find(|(held, _)| held == key)
            .map(|(_, frames)| frames.as_slice())
    }

    pub fn needs_building(&self, key: &StripKey) -> bool {
        self.get(key).is_none() && !self.failed.contains(key)
    }

    pub fn fail(&mut self, key: StripKey) {
        self.failed.push(key);
    }

    pub fn insert(&mut self, key: StripKey, frames: Vec<T>) -> Vec<T> {
        let mut retired = Vec::new();
        let same_project = |held: &StripKey| held.path == key.path;
        let mut index = 0;
        while index < self.held.len() {
            if same_project(&self.held[index].0) {
                retired.extend(self.held.remove(index).1);
            } else {
                index += 1;
            }
        }
        self.failed.retain(|failed| !same_project(failed));
        self.held.push((key, frames));
        while self.held.len() > MAX_STRIPS {
            retired.extend(self.held.remove(0).1);
        }
        retired
    }

    pub fn clear(&mut self) -> Vec<T> {
        self.failed.clear();
        self.held.drain(..).flat_map(|(_, frames)| frames).collect()
    }
}

pub enum Built {
    Frames(Vec<Arc<RenderImage>>),
    Stopped,
    Failed,
}

struct Sampler {
    reader: VideoReader,
    segment: usize,
    last: Option<(f64, Arc<RenderImage>)>,
}

impl Sampler {
    fn sample(
        &mut self,
        segments: &[Segment],
        timeline: &Timeline,
        position: f64,
    ) -> Result<Arc<RenderImage>, PlayError> {
        let (segment, local) = timeline.locate(position);
        if segment != self.segment {
            self.reader = VideoReader::open_light(&segments[segment].video)?;
            self.segment = segment;
            self.last = None;
        }
        self.reader.seek(local);
        let decoded: DecodedFrame = self.reader.next().ok_or(PlayError::Unreadable)?;
        if let Some((local, image)) = &self.last
            && *local == decoded.local
        {
            return Ok(image.clone());
        }
        let image = self.reader.image(&decoded, STRIP_BOUNDS).map_err(|error| {
            tracing::warn!("drawing a scrub frame failed: {error}");
            PlayError::Unreadable
        })?;
        self.last = Some((decoded.local, image.clone()));
        Ok(image)
    }
}

pub fn build(segments: &[Segment], hold: &Hold) -> Built {
    let attempt = || -> Result<Option<Vec<Arc<RenderImage>>>, PlayError> {
        let first = segments.first().ok_or(PlayError::Unreadable)?;
        let durations = segments
            .iter()
            .map(|segment| probe_duration(&segment.video))
            .collect::<Result<Vec<_>, _>>()?;
        let timeline = Timeline::new(durations);
        let mut sampler = Sampler {
            reader: VideoReader::open_light(&first.video)?,
            segment: 0,
            last: None,
        };
        let mut frames = Vec::with_capacity(STRIP_FRAMES);
        for position in sample_points(timeline.total(), STRIP_FRAMES) {
            if hold.stopped() {
                return Ok(None);
            }
            frames.push(sampler.sample(segments, &timeline, position)?);
        }
        Ok(Some(frames))
    };
    match attempt() {
        Ok(Some(frames)) => Built::Frames(frames),
        Ok(None) => Built::Stopped,
        Err(error) => {
            tracing::debug!("no scrub strip: {error:?}");
            Built::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(path: &str, size: u64) -> StripKey {
        StripKey {
            path: PathBuf::from(path),
            size,
            compression: Compression::None,
            export: None,
        }
    }

    #[test]
    fn the_pointer_picks_the_frame_under_it() {
        assert_eq!(frame_at(0., 20), 0);
        assert_eq!(frame_at(0.5, 20), 10);
        assert_eq!(frame_at(1., 20), 19);
        assert_eq!(frame_at(1.4, 20), 19);
        assert_eq!(frame_at(-1., 20), 0);
    }

    #[test]
    fn the_line_moves_in_steps_so_a_small_move_redraws_nothing() {
        assert_eq!(line_at(0.5), line_at(0.505));
        assert_eq!(line_at(2.), 1.);
    }

    #[test]
    fn samples_sit_in_the_middle_of_equal_slices() {
        assert_eq!(sample_points(10., 2), [2.5, 7.5]);
    }

    #[test]
    fn a_strip_is_found_only_while_the_project_is_unchanged() {
        let mut strips = Strips::default();
        strips.insert(key("a", 1), vec![1]);
        assert_eq!(strips.get(&key("a", 1)), Some([1].as_slice()));
        assert_eq!(strips.get(&key("a", 2)), None);
        assert!(strips.needs_building(&key("a", 2)));
    }

    #[test]
    fn a_new_strip_retires_the_projects_old_one() {
        let mut strips = Strips::default();
        strips.insert(key("a", 1), vec![1, 2]);
        assert_eq!(strips.insert(key("a", 2), vec![3]), [1, 2]);
        assert_eq!(strips.get(&key("a", 1)), None);
    }

    #[test]
    fn the_oldest_strip_goes_when_too_many_are_held() {
        let mut strips = Strips::default();
        for index in 0..MAX_STRIPS {
            assert!(
                strips
                    .insert(key(&index.to_string(), 0), vec![index])
                    .is_empty()
            );
        }
        assert_eq!(strips.insert(key("last", 0), vec![99]), [0]);
    }

    #[test]
    fn a_failed_strip_is_not_tried_again_until_the_project_changes() {
        let mut strips = Strips::<u8>::default();
        strips.fail(key("a", 1));
        assert!(!strips.needs_building(&key("a", 1)));
        assert!(strips.needs_building(&key("a", 2)));
    }

    #[test]
    fn clearing_hands_back_every_frame() {
        let mut strips = Strips::default();
        strips.insert(key("a", 1), vec![1]);
        strips.insert(key("b", 1), vec![2]);
        assert_eq!(strips.clear(), [1, 2]);
    }
}
