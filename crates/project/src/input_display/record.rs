use std::collections::BTreeSet;

use super::keys::key_code;
use crate::{
    CursorEvents, KeyboardEvents, ProjectConfiguration, RecordingMeta, StudioRecordingMeta,
    TimelineFrameMapping,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Input {
    Key(u16),
    MouseButton(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WheelStep {
    #[default]
    None,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputChange {
    pub time_ms: f64,
    pub input: Input,
    pub down: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputRecord {
    changes: Vec<InputChange>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayState {
    held: BTreeSet<Input>,
    pub wheel: WheelStep,
}

impl DisplayState {
    pub fn apply(&mut self, input: Input, down: bool) {
        if down {
            self.held.insert(input);
        } else {
            self.held.remove(&input);
        }
    }

    pub fn with_wheel(mut self, wheel: WheelStep) -> Self {
        self.wheel = wheel;
        self
    }

    pub fn is_held(&self, input: Input) -> bool {
        self.held.contains(&input)
    }

    pub fn held(&self) -> impl Iterator<Item = Input> + '_ {
        self.held.iter().copied()
    }
}

pub fn take_at(project: &ProjectConfiguration, output_time: f64, source_time: f64) -> usize {
    let mapping = project
        .timeline
        .as_ref()
        .and_then(|timeline| timeline.get_frame_mapping(output_time));
    let source = match mapping {
        Some(
            TimelineFrameMapping::Single { source, .. } | TimelineFrameMapping::Hold { source, .. },
        ) => source,
        Some(TimelineFrameMapping::Transition {
            outgoing, incoming, ..
        }) => {
            if (outgoing.source_time - source_time).abs() < (incoming.source_time - source_time).abs()
            {
                outgoing
            } else {
                incoming
            }
        }
        None => return 0,
    };
    source.segment.recording_clip as usize
}

impl InputRecord {
    pub fn new(mut changes: Vec<InputChange>) -> Self {
        changes.retain(|change| change.time_ms.is_finite());
        changes.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        Self { changes }
    }

    pub fn from_events(keyboard: &KeyboardEvents, cursor: &CursorEvents) -> Self {
        let keys = keyboard.presses.iter().filter_map(|press| {
            Some(InputChange {
                time_ms: press.time_ms,
                input: Input::Key(key_code(&press.key_code)?),
                down: press.down,
            })
        });
        let buttons = cursor.clicks.iter().map(|click| InputChange {
            time_ms: click.time_ms,
            input: Input::MouseButton(click.cursor_num),
            down: click.down,
        });
        Self::new(keys.chain(buttons).collect())
    }

    pub fn for_take(meta: &RecordingMeta, take: usize) -> Self {
        let Some(StudioRecordingMeta::MultipleSegments { inner }) = meta.studio_meta() else {
            return Self::default();
        };
        let Some(segment) = inner.segments.get(take) else {
            return Self::default();
        };
        let clicks = segment
            .cursor
            .as_ref()
            .and_then(|path| CursorEvents::load_clicks_from_file(&meta.path(path)).ok())
            .unwrap_or_default();
        Self::from_events(
            &segment.keyboard_events(meta),
            &CursorEvents {
                clicks,
                moves: Vec::new(),
            },
        )
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn changes(&self) -> &[InputChange] {
        &self.changes
    }

    pub fn state_at(&self, time_ms: f64) -> DisplayState {
        let mut state = DisplayState::default();
        for change in &self.changes {
            if change.time_ms > time_ms {
                break;
            }
            state.apply(change.input, change.down);
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CursorClickEvent, KeyPressEvent, ProjectConfiguration, RecordingMeta};

    fn two_take_project(transitions: serde_json::Value) -> ProjectConfiguration {
        ProjectConfiguration {
            timeline: Some(
                serde_json::from_value(serde_json::json!({
                    "segments": [
                        {"recordingSegment": 0, "start": 0.0, "end": 4.0, "timescale": 1.0},
                        {"recordingSegment": 1, "start": 10.0, "end": 14.0, "timescale": 1.0}
                    ],
                    "zoomSegments": [],
                    "transitions": transitions
                }))
                .unwrap(),
            ),
            ..Default::default()
        }
    }

    #[test]
    fn a_frame_belongs_to_the_take_its_clip_was_cut_from() {
        let project = two_take_project(serde_json::json!([]));

        assert_eq!(take_at(&project, 1.0, 1.0), 0);
        assert_eq!(take_at(&project, 5.0, 11.0), 1);
        assert_eq!(take_at(&ProjectConfiguration::default(), 5.0, 5.0), 0);
        assert_eq!(take_at(&project, 99.0, 0.0), 0);
    }

    #[test]
    fn each_side_of_a_transition_belongs_to_its_own_take() {
        let project = two_take_project(serde_json::json!([
            {"segmentIndex": 1, "type": "cross-fade", "duration": 1.0}
        ]));

        assert_eq!(take_at(&project, 3.5, 3.5), 0);
        assert_eq!(take_at(&project, 3.5, 10.5), 1);
    }

    #[test]
    fn a_takes_record_is_read_from_the_files_the_recording_saved() {
        let directory = tempfile::tempdir().unwrap();
        let take = directory.path().join("content/segments/segment-1");
        std::fs::create_dir_all(&take).unwrap();
        KeyboardEvents {
            presses: vec![key("W", 100.0, true)],
        }
        .write_to_file(&take.join("keyboard.bin"))
        .unwrap();
        std::fs::write(
            take.join("cursor.json"),
            serde_json::to_vec(&serde_json::json!({
                "clicks": [click(2, 150.0, true)],
                "moves": []
            }))
            .unwrap(),
        )
        .unwrap();
        let mut meta: RecordingMeta = serde_json::from_value(serde_json::json!({
            "pretty_name": "two takes",
            "segments": [
                {"display": {"path": "content/segments/segment-0/display.mp4"}},
                {
                    "display": {"path": "content/segments/segment-1/display.mp4"},
                    "cursor": "content/segments/segment-1/cursor.json"
                }
            ]
        }))
        .unwrap();
        meta.project_path = directory.path().to_path_buf();

        assert!(InputRecord::for_take(&meta, 0).is_empty());
        assert_eq!(
            held(&InputRecord::for_take(&meta, 1), 200.0),
            vec![W, Input::MouseButton(2)]
        );
        assert!(InputRecord::for_take(&meta, 7).is_empty());
    }

    const W: Input = Input::Key(0x57);
    const LEFT_SHIFT: Input = Input::Key(0xA010);
    const LEFT_BUTTON: Input = Input::MouseButton(1);

    fn key(code: &str, time_ms: f64, down: bool) -> KeyPressEvent {
        KeyPressEvent {
            key: code.to_lowercase(),
            key_code: code.to_string(),
            time_ms,
            down,
        }
    }

    fn click(button: u8, time_ms: f64, down: bool) -> CursorClickEvent {
        CursorClickEvent {
            active_modifiers: Vec::new(),
            cursor_num: button,
            cursor_id: "0".to_string(),
            time_ms,
            down,
        }
    }

    fn record(presses: Vec<KeyPressEvent>, clicks: Vec<CursorClickEvent>) -> InputRecord {
        InputRecord::from_events(
            &KeyboardEvents { presses },
            &CursorEvents {
                clicks,
                moves: Vec::new(),
            },
        )
    }

    fn held(record: &InputRecord, time_ms: f64) -> Vec<Input> {
        record.state_at(time_ms).held().collect()
    }

    #[test]
    fn a_key_is_held_from_its_press_until_its_release() {
        let record = record(
            vec![key("W", 1000.0, true), key("W", 1500.0, false)],
            Vec::new(),
        );

        assert_eq!(held(&record, 999.0), vec![]);
        assert_eq!(held(&record, 1000.0), vec![W]);
        assert_eq!(held(&record, 1499.0), vec![W]);
        assert_eq!(held(&record, 1500.0), vec![]);
    }

    #[test]
    fn a_chord_holds_every_key_that_is_down() {
        let record = record(
            vec![
                key("LShift", 100.0, true),
                key("W", 200.0, true),
                key("LShift", 300.0, false),
                key("W", 400.0, false),
            ],
            Vec::new(),
        );

        assert_eq!(held(&record, 150.0), vec![LEFT_SHIFT]);
        assert_eq!(held(&record, 250.0), vec![W, LEFT_SHIFT]);
        assert_eq!(held(&record, 350.0), vec![W]);
        assert_eq!(held(&record, 450.0), vec![]);
    }

    #[test]
    fn mouse_buttons_are_held_between_their_down_and_up_clicks() {
        let record = record(
            vec![key("W", 0.0, true)],
            vec![click(1, 50.0, true), click(1, 80.0, false)],
        );

        assert_eq!(held(&record, 60.0), vec![W, LEFT_BUTTON]);
        assert_eq!(held(&record, 80.0), vec![W]);
    }

    #[test]
    fn a_key_that_is_never_released_stays_held() {
        let record = record(vec![key("W", 10.0, true)], Vec::new());

        assert_eq!(held(&record, 3_600_000.0), vec![W]);
    }

    #[test]
    fn events_out_of_order_and_unknown_keys_do_not_disturb_the_state() {
        let record = record(
            vec![
                key("W", 500.0, false),
                key("W", 100.0, true),
                key("VolumeUp", 120.0, true),
                key("W", f64::NAN, false),
            ],
            Vec::new(),
        );

        assert_eq!(held(&record, 200.0), vec![W]);
        assert_eq!(held(&record, 600.0), vec![]);
    }

    #[test]
    fn a_release_without_a_press_holds_nothing() {
        let record = record(vec![key("W", 100.0, false)], Vec::new());

        assert_eq!(held(&record, 200.0), vec![]);
        assert!(!record.is_empty());
    }

    #[test]
    fn a_recording_without_input_gives_an_empty_record() {
        assert!(record(Vec::new(), Vec::new()).is_empty());
    }
}
