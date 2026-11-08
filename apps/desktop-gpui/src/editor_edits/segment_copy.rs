use frameever_project::{
    AudioTrackSegment, Camera3DSegment, CaptionTrackSegment, ImageSegment, KeyboardTrackSegment,
    MaskSegment, SceneSegment, StyleSegment, TextSegment, TimelineConfiguration, ZoomSegment,
};

use super::{Selection, TrackSegmentOps, sort_track, split_id};
use crate::editor_timeline::TrackKind;

const TOUCH_TOLERANCE: f64 = 1e-9;

#[derive(Debug, Clone)]
pub struct SegmentClipboard {
    track: TrackKind,
    timeline: TimelineConfiguration,
}

pub trait PastedSegment: TrackSegmentOps {
    fn pasted(&self, shift: f64) -> Self {
        let mut copy = self.clone();
        copy.set_start(self.start() + shift);
        copy.set_end(self.end() + shift);
        copy
    }
}

impl PastedSegment for ZoomSegment {}
impl PastedSegment for SceneSegment {}
impl PastedSegment for StyleSegment {}
impl PastedSegment for ImageSegment {}
impl PastedSegment for TextSegment {}
impl PastedSegment for MaskSegment {}
impl PastedSegment for AudioTrackSegment {}

impl PastedSegment for Camera3DSegment {
    fn pasted(&self, shift: f64) -> Self {
        let mut copy = self.clone();
        copy.start += shift;
        copy.end += shift;
        copy
    }
}

impl PastedSegment for CaptionTrackSegment {
    fn pasted(&self, shift: f64) -> Self {
        let mut copy = self.clone();
        copy.start += shift;
        copy.end += shift;
        for word in &mut copy.words {
            word.start += shift as f32;
            word.end += shift as f32;
        }
        copy.id = split_id("cap");
        copy
    }
}

impl PastedSegment for KeyboardTrackSegment {
    fn pasted(&self, shift: f64) -> Self {
        let mut copy = self.clone();
        copy.start += shift;
        copy.end += shift;
        copy.id = split_id("kb-edit");
        copy
    }
}

macro_rules! with_track_pair {
    ($kind:expr, $target:expr, $source:expr, |$segments:ident, $copies:ident| $body:expr) => {
        match $kind {
            TrackKind::Zoom => {
                let ($segments, $copies) = (&mut $target.zoom_segments, &$source.zoom_segments);
                $body
            }
            TrackKind::Scene => {
                let ($segments, $copies) = (&mut $target.scene_segments, &$source.scene_segments);
                $body
            }
            TrackKind::ThreeD => {
                let ($segments, $copies) =
                    (&mut $target.camera3d_segments, &$source.camera3d_segments);
                $body
            }
            TrackKind::Style => {
                let ($segments, $copies) = (&mut $target.style_segments, &$source.style_segments);
                $body
            }
            TrackKind::Image => {
                let ($segments, $copies) = (&mut $target.image_segments, &$source.image_segments);
                $body
            }
            TrackKind::Text => {
                let ($segments, $copies) = (&mut $target.text_segments, &$source.text_segments);
                $body
            }
            TrackKind::Mask => {
                let ($segments, $copies) = (&mut $target.mask_segments, &$source.mask_segments);
                $body
            }
            TrackKind::Audio => {
                let ($segments, $copies) = (&mut $target.audio_segments, &$source.audio_segments);
                $body
            }
            TrackKind::Caption => {
                let ($segments, $copies) =
                    (&mut $target.caption_segments, &$source.caption_segments);
                $body
            }
            TrackKind::Keyboard => {
                let ($segments, $copies) =
                    (&mut $target.keyboard_segments, &$source.keyboard_segments);
                $body
            }
            TrackKind::Clip => Default::default(),
        }
    };
}

pub fn copy_segments(
    timeline: &TimelineConfiguration,
    selection: &Selection,
) -> Option<SegmentClipboard> {
    let mut copied = timeline.clone();
    let copied_any = with_track!(copied, selection.track, |segments| {
        keep_indices(segments, &selection.indices)
    });
    copied_any.then_some(SegmentClipboard {
        track: selection.track,
        timeline: copied,
    })
}

pub fn paste_segments(
    timeline: &mut TimelineConfiguration,
    clipboard: &SegmentClipboard,
    at: f64,
    total: f64,
) -> Option<Selection> {
    let indices = with_track_pair!(
        clipboard.track,
        timeline,
        clipboard.timeline,
        |segments, copies| paste_into(segments, copies, at, total)
    )?;
    Some(Selection {
        track: clipboard.track,
        indices,
    })
}

pub fn duplicate_segments(
    timeline: &mut TimelineConfiguration,
    selection: &Selection,
    total: f64,
) -> Option<Selection> {
    let clipboard = copy_segments(timeline, selection)?;
    let end = with_track!(clipboard.timeline.clone(), clipboard.track, |segments| {
        segments
            .iter()
            .map(|segment| segment.end())
            .max_by(f64::total_cmp)
    })?;
    paste_segments(timeline, &clipboard, end, total)
}

fn keep_indices<T>(segments: &mut Vec<T>, indices: &[usize]) -> bool {
    let mut position = 0;
    segments.retain(|_| {
        let keep = indices.contains(&position);
        position += 1;
        keep
    });
    !segments.is_empty()
}

fn paste_into<T: PastedSegment>(
    segments: &mut Vec<T>,
    copies: &[T],
    at: f64,
    total: f64,
) -> Option<Vec<usize>> {
    let first = copies.iter().map(T::start).min_by(f64::total_cmp)?;
    let last = copies.iter().map(T::end).max_by(f64::total_cmp)?;
    let latest = total - (last - first);
    if latest < 0.0 {
        return None;
    }
    let earliest = at.clamp(0.0, latest);
    let mut candidates = vec![earliest];
    for segment in segments.iter() {
        for copy in copies {
            let start = segment.end() - (copy.start() - first);
            if start > earliest && start <= latest {
                candidates.push(start);
            }
        }
    }
    candidates.sort_by(f64::total_cmp);
    let start = candidates
        .into_iter()
        .find(|start| fits(segments, copies, start - first))?;
    let shift = start - first;
    let pasted: Vec<T> = copies.iter().map(|copy| copy.pasted(shift)).collect();
    let keys: Vec<(u32, f64, f64)> = pasted
        .iter()
        .map(|copy| (copy.lane(), copy.start(), copy.end()))
        .collect();
    segments.extend(pasted);
    sort_track(segments);
    Some(
        keys.into_iter()
            .filter_map(|(lane, start, end)| {
                segments.iter().position(|segment| {
                    segment.lane() == lane && segment.start() == start && segment.end() == end
                })
            })
            .collect(),
    )
}

fn fits<T: TrackSegmentOps>(segments: &[T], copies: &[T], shift: f64) -> bool {
    copies.iter().all(|copy| {
        let (start, end) = (copy.start() + shift, copy.end() + shift);
        segments.iter().all(|segment| {
            segment.lane() != copy.lane()
                || segment.end() <= start + TOUCH_TOLERANCE
                || end <= segment.start() + TOUCH_TOLERANCE
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline(json: serde_json::Value) -> TimelineConfiguration {
        serde_json::from_value(json).expect("fixture parses")
    }

    fn zoom_timeline() -> TimelineConfiguration {
        timeline(serde_json::json!({
            "segments": [{ "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 30.0 }],
            "zoomSegments": [
                { "start": 2.0, "end": 5.0, "amount": 1.5, "mode": "auto" },
                { "start": 20.0, "end": 24.0, "amount": 2.0, "mode": "auto" }
            ]
        }))
    }

    fn spans<T: super::super::TrackSegmentOps>(segments: &[T]) -> Vec<(f64, f64)> {
        segments.iter().map(|s| (s.start(), s.end())).collect()
    }

    #[test]
    fn paste_lands_on_the_same_track_at_the_playhead_with_its_settings() {
        let mut timeline = zoom_timeline();
        let clipboard = copy_segments(&timeline, &Selection::single(TrackKind::Zoom, 1)).unwrap();
        let pasted = paste_segments(&mut timeline, &clipboard, 10.0, 30.0).unwrap();
        assert_eq!(
            spans(&timeline.zoom_segments),
            vec![(2.0, 5.0), (10.0, 14.0), (20.0, 24.0)]
        );
        assert_eq!(pasted, Selection::single(TrackKind::Zoom, 1));
        assert_eq!(timeline.zoom_segments[1].amount, 2.0);
    }

    #[test]
    fn paste_moves_past_segments_it_would_overlap() {
        let mut timeline = zoom_timeline();
        let clipboard = copy_segments(&timeline, &Selection::single(TrackKind::Zoom, 0)).unwrap();
        let pasted = paste_segments(&mut timeline, &clipboard, 21.0, 30.0).unwrap();
        assert_eq!(
            spans(&timeline.zoom_segments),
            vec![(2.0, 5.0), (20.0, 24.0), (24.0, 27.0)]
        );
        assert_eq!(pasted.indices, vec![2]);
        assert!(paste_segments(&mut timeline, &clipboard, 26.0, 29.0).is_none());
        assert_eq!(timeline.zoom_segments.len(), 3);
    }

    #[test]
    fn paste_keeps_lanes_apart_and_stays_inside_the_timeline() {
        let mut timeline = timeline(serde_json::json!({
            "segments": [{ "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 30.0 }],
            "zoomSegments": [],
            "textSegments": [
                { "start": 1.0, "end": 3.0, "track": 0, "content": "a" },
                { "start": 10.0, "end": 14.0, "track": 1, "content": "b" }
            ]
        }));
        let clipboard = copy_segments(&timeline, &Selection::single(TrackKind::Text, 0)).unwrap();
        paste_segments(&mut timeline, &clipboard, 11.0, 30.0).unwrap();
        paste_segments(&mut timeline, &clipboard, 29.0, 30.0).unwrap();
        let placed: Vec<(u32, f64, f64, String)> = timeline
            .text_segments
            .iter()
            .map(|s| (s.track, s.start, s.end, s.content.clone()))
            .collect();
        assert_eq!(
            placed,
            vec![
                (0, 1.0, 3.0, "a".into()),
                (0, 11.0, 13.0, "a".into()),
                (0, 28.0, 30.0, "a".into()),
                (1, 10.0, 14.0, "b".into()),
            ]
        );
    }

    #[test]
    fn a_multi_selection_keeps_its_relative_timing() {
        let mut timeline = zoom_timeline();
        let selection = Selection {
            track: TrackKind::Zoom,
            indices: vec![1, 0],
        };
        let clipboard = copy_segments(&timeline, &selection).unwrap();
        timeline.zoom_segments.clear();
        let pasted = paste_segments(&mut timeline, &clipboard, 3.0, 30.0).unwrap();
        assert_eq!(
            spans(&timeline.zoom_segments),
            vec![(3.0, 6.0), (21.0, 25.0)]
        );
        assert_eq!(pasted.indices, vec![0, 1]);
    }

    #[test]
    fn duplicate_places_the_copy_straight_after_the_original() {
        let mut timeline = zoom_timeline();
        let duplicated =
            duplicate_segments(&mut timeline, &Selection::single(TrackKind::Zoom, 0), 30.0)
                .unwrap();
        assert_eq!(
            spans(&timeline.zoom_segments),
            vec![(2.0, 5.0), (5.0, 8.0), (20.0, 24.0)]
        );
        assert_eq!(duplicated, Selection::single(TrackKind::Zoom, 1));
        let both = Selection {
            track: TrackKind::Zoom,
            indices: vec![0, 1],
        };
        duplicate_segments(&mut timeline, &both, 30.0).unwrap();
        assert_eq!(
            spans(&timeline.zoom_segments),
            vec![
                (2.0, 5.0),
                (5.0, 8.0),
                (8.0, 11.0),
                (11.0, 14.0),
                (20.0, 24.0)
            ]
        );
    }

    #[test]
    fn clips_are_not_copied() {
        let timeline = zoom_timeline();
        assert!(copy_segments(&timeline, &Selection::single(TrackKind::Clip, 0)).is_none());
    }

    #[test]
    fn keyboard_copies_get_their_own_id() {
        let mut timeline = timeline(serde_json::json!({
            "segments": [{ "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 30.0 }],
            "zoomSegments": [],
            "keyboardSegments": [{ "id": "kb-1", "start": 1.0, "end": 2.0, "displayText": "Ctrl+S" }]
        }));
        duplicate_segments(
            &mut timeline,
            &Selection::single(TrackKind::Keyboard, 0),
            30.0,
        )
        .unwrap();
        let copy = &timeline.keyboard_segments[1];
        assert_eq!((copy.start, copy.end), (2.0, 3.0));
        assert_eq!(copy.display_text, "Ctrl+S");
        assert_ne!(copy.id, "kb-1");
    }

    #[test]
    fn one_paste_is_one_undo_step() {
        let mut project = frameever_project::ProjectConfiguration {
            timeline: Some(zoom_timeline()),
            ..Default::default()
        };
        let before = serde_json::to_value(&project).unwrap();
        let mut history = super::super::ProjectHistory::new(project.clone());
        let selection = Selection {
            track: TrackKind::Zoom,
            indices: vec![0, 1],
        };
        let clipboard = copy_segments(project.timeline.as_ref().unwrap(), &selection).unwrap();
        paste_segments(project.timeline.as_mut().unwrap(), &clipboard, 0.0, 60.0).unwrap();
        history.record(&project);
        assert_eq!(project.timeline.as_ref().unwrap().zoom_segments.len(), 4);
        assert_eq!(
            serde_json::to_value(history.undo().unwrap()).unwrap(),
            before
        );
        assert!(!history.can_undo());
    }
}
