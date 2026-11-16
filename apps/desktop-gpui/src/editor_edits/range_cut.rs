use frameever_project::{TimelineConfiguration, TimelineSegment};

use super::{
    ProjectHistory, clip_timeline_duration, effective_to_output, effective_to_output_end,
    ripple_delete_output_tracks,
};
use crate::editor_clips::{ripple_keyboard_track, ripple_track};
use crate::editor_timeline;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeCut {
    Applied,
    Empty,
    Transition,
    Hold,
    LastClip,
}

pub const MIN_CLIP_PIECE: f64 = 1.0 / 60.0;

impl ProjectHistory {
    pub fn removed_takes(&self, timeline: &TimelineConfiguration) -> Vec<TimelineSegment> {
        let mut seen: std::collections::HashSet<u32> = timeline
            .segments
            .iter()
            .map(|segment| segment.recording_clip)
            .collect();
        let mut found = Vec::new();
        let current = self.entries.len() - self.count;
        for entry in self.entries[..current].iter().rev() {
            let Some(past) = entry.timeline.as_ref() else {
                continue;
            };
            for segment in &past.segments {
                if seen.insert(segment.recording_clip) {
                    found.push(segment.clone());
                }
            }
        }
        found
    }
}

pub fn cut_clip_range(
    timeline: &mut TimelineConfiguration,
    index: usize,
    local_start: f64,
    local_end: f64,
) -> RangeCut {
    let Some(segment) = timeline.segments.get(index) else {
        return RangeCut::Empty;
    };
    let duration = segment.duration();
    if !(local_start.is_finite() && local_end.is_finite() && duration.is_finite()) {
        return RangeCut::Empty;
    }
    let mut local_start = local_start.clamp(0.0, duration);
    let mut local_end = local_end.clamp(0.0, duration);
    if local_end - local_start < 1e-4 {
        return RangeCut::Empty;
    }
    if local_start < MIN_CLIP_PIECE {
        local_start = 0.0;
    }
    if duration - local_end < MIN_CLIP_PIECE {
        local_end = duration;
    }
    let incoming = timeline
        .effective_transition(index)
        .map_or(0.0, |transition| transition.duration);
    let outgoing = timeline
        .effective_transition(index + 1)
        .map_or(0.0, |transition| transition.duration);
    if (incoming > 0.0 && local_start < incoming * 2.0)
        || (outgoing > 0.0 && duration - local_end < outgoing * 2.0)
    {
        return RangeCut::Transition;
    }
    let whole = local_start == 0.0 && local_end == duration;
    if whole && timeline.segments.len() < 2 {
        return RangeCut::LastClip;
    }

    let clip_start = editor_timeline::clip_timeline_offsets(timeline)
        .get(index)
        .copied()
        .unwrap_or(0.0);
    let holds = timeline.hold_windows();
    let output_start = effective_to_output_end(&holds, clip_start + local_start);
    let output_end = effective_to_output(&holds, clip_start + local_end).max(output_start);
    if holds
        .iter()
        .any(|(start, end)| *start < output_end - 1e-9 && *end > output_start + 1e-9)
    {
        return RangeCut::Hold;
    }

    let duration_before = clip_timeline_duration(timeline);
    let segment = &mut timeline.segments[index];
    let source_start = segment.start + local_start * segment.timescale;
    let source_end = segment.start + local_end * segment.timescale;
    if whole {
        timeline.segments.remove(index);
        timeline.transitions.retain(|transition| {
            let at = transition.segment_index as usize;
            at != index && at != index + 1
        });
        for transition in timeline.transitions.iter_mut() {
            if transition.segment_index as usize > index {
                transition.segment_index -= 1;
            }
        }
    } else if local_start == 0.0 {
        segment.start = source_end;
    } else if local_end == duration {
        segment.end = source_start;
    } else {
        let mut tail = segment.clone();
        tail.start = source_end;
        segment.end = source_start;
        timeline.segments.insert(index + 1, tail);
        for transition in timeline.transitions.iter_mut() {
            if transition.segment_index as usize > index {
                transition.segment_index += 1;
            }
        }
    }
    let shift = (duration_before - clip_timeline_duration(timeline)).max(0.0);
    ripple_delete_output_tracks(timeline, output_start, output_end, shift);
    RangeCut::Applied
}

pub fn ripple_insert_output_tracks(
    timeline: &mut TimelineConfiguration,
    boundary: f64,
    shift: f64,
) {
    ripple_track(&mut timeline.style_segments, boundary, shift);
    ripple_track(&mut timeline.image_segments, boundary, shift);
    ripple_track(&mut timeline.zoom_segments, boundary, shift);
    ripple_track(&mut timeline.scene_segments, boundary, shift);
    ripple_track(&mut timeline.text_segments, boundary, shift);
    ripple_track(&mut timeline.caption_segments, boundary, shift);
    ripple_keyboard_track(&mut timeline.keyboard_segments, boundary, shift);
    ripple_track(&mut timeline.audio_segments, boundary, shift);
    for segment in &mut timeline.mask_segments {
        if segment.start >= boundary {
            segment.start += shift;
            segment.end += shift;
        } else if segment.end > boundary {
            let local = boundary - segment.start;
            for time in segment
                .keyframes
                .position
                .iter_mut()
                .map(|key| &mut key.time)
                .chain(segment.keyframes.size.iter_mut().map(|key| &mut key.time))
                .chain(
                    segment
                        .keyframes
                        .intensity
                        .iter_mut()
                        .map(|key| &mut key.time),
                )
            {
                if *time >= local {
                    *time += shift;
                }
            }
            segment.end += shift;
        }
    }
    for segment in &mut timeline.camera3d_segments {
        if segment.start >= boundary {
            segment.start += shift;
            segment.end += shift;
        } else if segment.end > boundary {
            let local = boundary - segment.start;
            for track in segment.tracks.all_tracks_mut() {
                for keyframe in track.iter_mut() {
                    if keyframe.time >= local {
                        keyframe.time += shift;
                    }
                }
            }
            segment.end += shift;
        }
    }
}

#[cfg(test)]
mod tests {
    use frameever_project::{ClipTransition, ClipTransitionType, ProjectConfiguration};

    use super::*;

    fn project(json: serde_json::Value) -> ProjectConfiguration {
        serde_json::from_value(json).expect("fixture parses")
    }

    fn two_recordings() -> ProjectConfiguration {
        project(serde_json::json!({
            "timeline": {
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 10.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 5.0 }
                ],
                "zoomSegments": [{ "start": 12.0, "end": 14.0, "amount": 1.5, "mode": "auto" }],
                "textSegments": [{ "start": 3.0, "end": 8.0, "content": "hi" }]
            }
        }))
    }

    fn bounds(timeline: &TimelineConfiguration) -> Vec<(u32, f64, f64)> {
        timeline
            .segments
            .iter()
            .map(|segment| (segment.recording_clip, segment.start, segment.end))
            .collect()
    }

    #[test]
    fn cutting_inside_a_clip_splits_it_and_ripples_every_track() {
        let mut project = two_recordings();
        let timeline = project.timeline.as_mut().unwrap();
        assert_eq!(cut_clip_range(timeline, 0, 4.0, 6.0), RangeCut::Applied);
        assert_eq!(
            bounds(timeline),
            [(0, 0.0, 4.0), (0, 6.0, 10.0), (1, 0.0, 5.0)]
        );
        assert_eq!(
            (
                timeline.zoom_segments[0].start,
                timeline.zoom_segments[0].end
            ),
            (10.0, 12.0)
        );
        assert_eq!(
            (
                timeline.text_segments[0].start,
                timeline.text_segments[0].end
            ),
            (3.0, 6.0)
        );
    }

    #[test]
    fn a_whole_take_after_a_recording_boundary_goes_and_the_last_clip_stays() {
        let mut project = two_recordings();
        let timeline = project.timeline.as_mut().unwrap();
        assert_eq!(cut_clip_range(timeline, 1, 0.0, 5.0), RangeCut::Applied);
        assert_eq!(bounds(timeline), [(0, 0.0, 10.0)]);
        assert_eq!(cut_clip_range(timeline, 0, 0.0, 10.0), RangeCut::LastClip);
    }

    #[test]
    fn a_cut_next_to_a_transition_is_refused() {
        let mut project = two_recordings();
        let timeline = project.timeline.as_mut().unwrap();
        timeline.transitions.push(ClipTransition {
            segment_index: 1,
            kind: ClipTransitionType::CrossFade,
            duration: 0.5,
        });
        let before = bounds(timeline);
        assert_eq!(cut_clip_range(timeline, 0, 9.5, 9.8), RangeCut::Transition);
        assert_eq!(cut_clip_range(timeline, 1, 0.2, 0.8), RangeCut::Transition);
        assert_eq!(bounds(timeline), before);
        assert_eq!(cut_clip_range(timeline, 0, 2.0, 3.0), RangeCut::Applied);
        assert_eq!(timeline.transitions[0].segment_index, 2);
    }

    #[test]
    fn slivers_at_clip_edges_are_absorbed() {
        let mut project = two_recordings();
        let timeline = project.timeline.as_mut().unwrap();
        assert_eq!(cut_clip_range(timeline, 0, 0.005, 2.0), RangeCut::Applied);
        assert_eq!(bounds(timeline)[0], (0, 2.0, 10.0));
    }

    #[test]
    fn ripple_insert_moves_keyframes_after_the_boundary_with_the_material() {
        let mut project = project(serde_json::json!({
            "timeline": {
                "segments": [{ "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 20.0 }],
                "zoomSegments": [{ "start": 8.0, "end": 9.0, "amount": 1.5, "mode": "auto" }],
                "maskSegments": [{
                    "start": 2.0, "end": 6.0, "maskType": "sensitive",
                    "center": { "x": 0.5, "y": 0.5 }, "size": { "x": 0.2, "y": 0.2 },
                    "keyframes": { "position": [
                        { "time": 1.0, "x": 0.1, "y": 0.1 },
                        { "time": 3.0, "x": 0.9, "y": 0.9 }
                    ] }
                }]
            }
        }));
        let timeline = project.timeline.as_mut().unwrap();
        ripple_insert_output_tracks(timeline, 4.0, 2.0);
        let mask = &timeline.mask_segments[0];
        assert_eq!((mask.start, mask.end), (2.0, 8.0));
        let times: Vec<f64> = mask.keyframes.position.iter().map(|key| key.time).collect();
        assert_eq!(times, [1.0, 5.0]);
        assert_eq!(timeline.zoom_segments[0].start, 10.0);
    }

    #[test]
    fn removed_takes_come_from_the_undo_history() {
        let mut project = two_recordings();
        let mut history = ProjectHistory::new(project.clone());
        let timeline = project.timeline.as_mut().unwrap();
        timeline.segments[1].volume = Some(0.5);
        history.record(&project);
        let timeline = project.timeline.as_mut().unwrap();
        assert_eq!(cut_clip_range(timeline, 1, 0.0, 5.0), RangeCut::Applied);
        history.record(&project);
        let removed = history.removed_takes(project.timeline.as_ref().unwrap());
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].recording_clip, 1);
        assert_eq!(removed[0].volume, Some(0.5));
    }

    #[test]
    fn every_transition_kind_made_by_a_drag_refuses_a_cut_next_to_it_and_keeps_its_kind() {
        for kind in ClipTransitionType::ALL {
            let mut project = two_recordings();
            let base = project.timeline.clone().unwrap();
            let timeline = project.timeline.as_mut().unwrap();
            *timeline = crate::editor_clips::apply_dragged_transition(&base, 1, kind, Some(0.5));
            let before = bounds(timeline);
            assert_eq!(
                cut_clip_range(timeline, 0, 9.5, 9.8),
                RangeCut::Transition,
                "{kind:?}"
            );
            assert_eq!(
                cut_clip_range(timeline, 1, 0.2, 0.8),
                RangeCut::Transition,
                "{kind:?}"
            );
            assert_eq!(bounds(timeline), before);
            assert_eq!(cut_clip_range(timeline, 0, 2.0, 3.0), RangeCut::Applied);
            let moved = timeline.effective_transition(2).unwrap();
            assert_eq!((moved.kind, moved.duration), (kind, 0.5));
            let reopened: TimelineConfiguration =
                serde_json::from_value(serde_json::to_value(&*timeline).unwrap()).unwrap();
            assert_eq!(reopened.effective_transition(2).unwrap().kind, kind);
        }
    }
}
