use std::collections::BTreeSet;

use super::{
    builtin::BuiltinPresetId,
    events::{InputEventKind, InputEvents},
    gamepad::{GamepadAxes, GamepadKind, GamepadSnapshot},
    keys::key_code,
};
use crate::{
    CursorClickEvent, KeyboardEvents, ProjectConfiguration, RecordingMeta, StudioRecordingMeta,
    TimelineFrameMapping,
};

pub const WHEEL_FLASH_MS: f64 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Input {
    Key(u16),
    MouseButton(u8),
    GamepadButton(u16),
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
    gamepad: Vec<(f64, GamepadSnapshot)>,
    wheel_steps: Vec<(f64, WheelStep)>,
    focus_changes: Vec<(f64, bool)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayState {
    held: BTreeSet<Input>,
    pub wheel: WheelStep,
    pub axes: GamepadAxes,
    pub controller: Option<GamepadKind>,
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

    pub fn show_gamepad(&mut self, snapshot: &GamepadSnapshot) {
        for button in snapshot.pressed() {
            self.held.insert(Input::GamepadButton(button));
        }
        self.axes = snapshot.axes;
        self.controller = Some(snapshot.kind);
    }

    pub fn is_held(&self, input: Input) -> bool {
        self.held.contains(&input)
    }

    pub fn held(&self) -> impl Iterator<Item = Input> + '_ {
        self.held.iter().copied()
    }
}

#[derive(Clone, Debug, Default)]
pub struct WindowRule {
    shown: BTreeSet<Input>,
    polled: BTreeSet<Input>,
}

impl WindowRule {
    pub fn change(&mut self, input: Input, down: bool, target_in_front: bool) {
        if !down {
            self.shown.remove(&input);
        } else if target_in_front {
            self.shown.insert(input);
        }
    }

    pub fn follow(&mut self, polled: &DisplayState, target_in_front: bool) -> DisplayState {
        let follows_presses = |input: &Input| !matches!(input, Input::GamepadButton(_));
        let now: BTreeSet<Input> = polled.held().filter(follows_presses).collect();
        for input in self.polled.difference(&now).copied().collect::<Vec<_>>() {
            self.change(input, false, target_in_front);
        }
        for input in now.difference(&self.polled).copied().collect::<Vec<_>>() {
            self.change(input, true, target_in_front);
        }
        self.polled = now;
        if !target_in_front {
            return DisplayState {
                controller: polled.controller,
                ..Default::default()
            };
        }
        let mut held = self.shown.clone();
        held.extend(polled.held().filter(|input| !follows_presses(input)));
        DisplayState {
            held,
            ..polled.clone()
        }
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
            if (outgoing.source_time - source_time).abs()
                < (incoming.source_time - source_time).abs()
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

pub fn preset_for_recording<'a>(
    meta: &RecordingMeta,
    preset: &'a str,
    made_for_a_gamepad: bool,
) -> &'a str {
    let takes = match meta.studio_meta() {
        Some(StudioRecordingMeta::MultipleSegments { inner }) => inner.segments.len(),
        _ => 0,
    };
    let records: Vec<InputRecord> = (0..takes)
        .map(|take| InputRecord::for_take(meta, take))
        .collect();
    let shows_gamepad = made_for_a_gamepad
        || BuiltinPresetId::from_id(preset).is_some_and(|preset| preset.controller().is_some());
    if records.iter().any(InputRecord::used_gamepad) {
        if shows_gamepad {
            preset
        } else {
            BuiltinPresetId::Gamepad.id()
        }
    } else if shows_gamepad && records.iter().any(InputRecord::used_keys_or_mouse) {
        BuiltinPresetId::WasdMouse.id()
    } else {
        preset
    }
}

impl InputRecord {
    pub fn used_gamepad(&self) -> bool {
        self.controller().is_some()
    }

    pub fn used_keys_or_mouse(&self) -> bool {
        !self.changes.is_empty() || !self.wheel_steps.is_empty()
    }

    pub fn new(mut changes: Vec<InputChange>) -> Self {
        changes.retain(|change| change.time_ms.is_finite());
        changes.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        Self {
            changes,
            ..Default::default()
        }
    }

    pub fn with_gamepad(mut self, mut snapshots: Vec<(f64, GamepadSnapshot)>) -> Self {
        snapshots.retain(|(time_ms, _)| time_ms.is_finite());
        snapshots.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.gamepad = snapshots;
        self
    }

    pub fn controller(&self) -> Option<GamepadKind> {
        self.gamepad
            .iter()
            .find(|(_, snapshot)| !snapshot.is_idle())
            .map(|(_, snapshot)| snapshot.kind)
    }

    pub fn from_events(keyboard: &KeyboardEvents, input: &InputEvents) -> Self {
        let mut changes = keyboard
            .presses
            .iter()
            .filter_map(|press| {
                Some(InputChange {
                    time_ms: press.time_ms,
                    input: Input::Key(key_code(&press.key_code)?),
                    down: press.down,
                })
            })
            .collect::<Vec<_>>();
        let mut wheel_steps = Vec::new();
        let mut focus_changes = Vec::new();
        let mut gamepad = Vec::new();
        for event in input
            .events
            .iter()
            .filter(|event| event.time_ms.is_finite())
        {
            match event.kind {
                InputEventKind::MouseButton { button, down } => changes.push(InputChange {
                    time_ms: event.time_ms,
                    input: Input::MouseButton(button),
                    down,
                }),
                InputEventKind::Wheel { up } => wheel_steps.push((
                    event.time_ms,
                    if up { WheelStep::Up } else { WheelStep::Down },
                )),
                InputEventKind::TargetInFront(in_front) => {
                    focus_changes.push((event.time_ms, in_front));
                }
                InputEventKind::Gamepad(snapshot) => gamepad.push((event.time_ms, snapshot)),
                InputEventKind::HeldKey(code) => changes.push(InputChange {
                    time_ms: event.time_ms,
                    input: Input::Key(code),
                    down: true,
                }),
            }
        }
        wheel_steps.sort_by(|a, b| a.0.total_cmp(&b.0));
        focus_changes.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self {
            wheel_steps,
            focus_changes,
            ..Self::new(changes)
        }
        .with_gamepad(gamepad)
    }

    pub fn for_take(meta: &RecordingMeta, take: usize) -> Self {
        let Some(StudioRecordingMeta::MultipleSegments { inner }) = meta.studio_meta() else {
            return Self::default();
        };
        let Some(segment) = inner.segments.get(take) else {
            return Self::default();
        };
        let record = Self::from_events(&segment.keyboard_events(meta), &segment.input_events(meta));
        let saved_before_input_events = segment.has_keyboard_events(meta)
            && !segment
                .input_events_path(meta)
                .is_some_and(|path| path.exists());
        if saved_before_input_events {
            record.with_clicks(&segment.cursor_events(meta).clicks)
        } else {
            record
        }
    }

    fn with_clicks(mut self, clicks: &[CursorClickEvent]) -> Self {
        self.changes.extend(
            clicks
                .iter()
                .filter(|click| click.time_ms.is_finite())
                .map(|click| InputChange {
                    time_ms: click.time_ms,
                    input: Input::MouseButton(click.cursor_num),
                    down: click.down,
                }),
        );
        self.changes.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.wheel_steps.is_empty() && self.gamepad.is_empty()
    }

    pub fn gamepad(&self) -> &[(f64, GamepadSnapshot)] {
        &self.gamepad
    }

    pub fn changes(&self) -> &[InputChange] {
        &self.changes
    }

    fn target_in_front_at(&self, time_ms: f64) -> bool {
        let earlier = self
            .focus_changes
            .partition_point(|(changed_at, _)| *changed_at <= time_ms);
        earlier
            .checked_sub(1)
            .and_then(|index| self.focus_changes.get(index))
            .is_none_or(|(_, in_front)| *in_front)
    }

    pub fn state_at(&self, time_ms: f64) -> DisplayState {
        let mut state = DisplayState {
            controller: self.controller(),
            ..Default::default()
        };
        if !self.target_in_front_at(time_ms) {
            return state;
        }
        let mut rule = WindowRule::default();
        for change in &self.changes {
            if change.time_ms > time_ms {
                break;
            }
            rule.change(
                change.input,
                change.down,
                self.target_in_front_at(change.time_ms),
            );
        }
        state.held = rule.shown;
        let earlier = self
            .wheel_steps
            .partition_point(|(stepped_at, _)| *stepped_at <= time_ms);
        if let Some((stepped_at, step)) = earlier
            .checked_sub(1)
            .and_then(|index| self.wheel_steps.get(index))
            && time_ms - stepped_at < WHEEL_FLASH_MS
            && self.target_in_front_at(*stepped_at)
        {
            state.wheel = *step;
        }
        let earlier = self
            .gamepad
            .partition_point(|(changed_at, _)| *changed_at <= time_ms);
        if let Some((_, snapshot)) = earlier
            .checked_sub(1)
            .and_then(|index| self.gamepad.get(index))
        {
            state.show_gamepad(snapshot);
        }
        state.controller = self.controller();
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KeyPressEvent, ProjectConfiguration, RecordingMeta,
        input_display::{
            GamepadAxes, GamepadKind, GamepadSnapshot, INPUT_EVENTS_FILE_NAME, InputEvent,
        },
    };

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
        InputEvents {
            events: vec![click(2, 150.0, true)],
        }
        .write_to_file(&take.join(INPUT_EVENTS_FILE_NAME))
        .unwrap();
        std::fs::write(
            take.join("cursor.json"),
            serde_json::to_vec(&serde_json::json!({
                "clicks": [{
                    "active_modifiers": [],
                    "cursor_num": 1,
                    "cursor_id": "0",
                    "time_ms": 10.0,
                    "down": true
                }],
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

    fn take_with_clicks_only_in_the_cursor_file(
        keyboard: bool,
    ) -> (tempfile::TempDir, RecordingMeta) {
        let directory = tempfile::tempdir().unwrap();
        let take = directory.path().join("content/segments/segment-0");
        std::fs::create_dir_all(&take).unwrap();
        if keyboard {
            KeyboardEvents {
                presses: vec![key("W", 100.0, true)],
            }
            .write_to_file(&take.join("keyboard.bin"))
            .unwrap();
        }
        let click = |button: u8, time_ms: f64, down: bool| {
            serde_json::json!({
                "active_modifiers": [],
                "cursor_num": button,
                "cursor_id": "0",
                "time_ms": time_ms,
                "down": down
            })
        };
        std::fs::write(
            take.join("cursor.json"),
            serde_json::to_vec(&serde_json::json!({
                "clicks": [click(1, 120.0, true), click(1, 300.0, false), click(2, 250.0, true)],
                "moves": []
            }))
            .unwrap(),
        )
        .unwrap();
        let mut meta: RecordingMeta = serde_json::from_value(serde_json::json!({
            "pretty_name": "older",
            "segments": [{
                "display": {"path": "content/segments/segment-0/display.mp4"},
                "cursor": "content/segments/segment-0/cursor.json"
            }]
        }))
        .unwrap();
        meta.project_path = directory.path().to_path_buf();
        (directory, meta)
    }

    #[test]
    fn a_recording_from_before_the_input_file_shows_its_saved_clicks() {
        let (_directory, meta) = take_with_clicks_only_in_the_cursor_file(true);
        let record = InputRecord::for_take(&meta, 0);

        assert_eq!(held(&record, 200.0), vec![W, LEFT_BUTTON]);
        assert_eq!(held(&record, 400.0), vec![W, Input::MouseButton(2)]);
    }

    #[test]
    fn clicks_alone_are_not_an_input_record() {
        let (_directory, meta) = take_with_clicks_only_in_the_cursor_file(false);

        assert!(InputRecord::for_take(&meta, 0).is_empty());
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

    fn click(button: u8, time_ms: f64, down: bool) -> InputEvent {
        InputEvent {
            time_ms,
            kind: InputEventKind::MouseButton { button, down },
        }
    }

    fn wheel(time_ms: f64, up: bool) -> InputEvent {
        InputEvent {
            time_ms,
            kind: InputEventKind::Wheel { up },
        }
    }

    fn in_front(time_ms: f64, in_front: bool) -> InputEvent {
        InputEvent {
            time_ms,
            kind: InputEventKind::TargetInFront(in_front),
        }
    }

    fn record(presses: Vec<KeyPressEvent>, events: Vec<InputEvent>) -> InputRecord {
        InputRecord::from_events(&KeyboardEvents { presses }, &InputEvents { events })
    }

    #[test]
    fn a_wheel_step_flashes_briefly_in_its_direction() {
        let record = record(Vec::new(), vec![wheel(1000.0, true), wheel(2000.0, false)]);

        assert_eq!(record.state_at(999.0).wheel, WheelStep::None);
        assert_eq!(record.state_at(1000.0).wheel, WheelStep::Up);
        assert_eq!(record.state_at(1119.0).wheel, WheelStep::Up);
        assert_eq!(record.state_at(1120.0).wheel, WheelStep::None);
        assert_eq!(record.state_at(2050.0).wheel, WheelStep::Down);
        assert!(!record.is_empty());
    }

    #[test]
    fn side_buttons_are_held_like_the_other_buttons() {
        let record = record(
            Vec::new(),
            vec![
                click(4, 10.0, true),
                click(5, 20.0, true),
                click(4, 30.0, false),
            ],
        );

        assert_eq!(
            held(&record, 25.0),
            vec![Input::MouseButton(4), Input::MouseButton(5)]
        );
        assert_eq!(held(&record, 35.0), vec![Input::MouseButton(5)]);
    }

    #[test]
    fn input_pressed_while_another_window_was_in_front_is_not_shown() {
        let record = record(
            vec![
                key("W", 100.0, true),
                key("W", 200.0, false),
                key("A", 1100.0, true),
                key("A", 1200.0, false),
                key("W", 2100.0, true),
            ],
            vec![
                in_front(0.0, true),
                in_front(1000.0, false),
                click(1, 1150.0, true),
                wheel(1150.0, true),
                click(1, 1180.0, false),
                in_front(2000.0, true),
            ],
        );

        assert_eq!(held(&record, 150.0), vec![W]);
        assert_eq!(held(&record, 1150.0), vec![]);
        assert_eq!(record.state_at(1160.0).wheel, WheelStep::None);
        assert_eq!(held(&record, 2150.0), vec![W]);
    }

    #[test]
    fn a_key_held_across_a_window_switch_follows_the_window() {
        let record = record(
            vec![
                key("W", 100.0, true),
                key("A", 1500.0, true),
                key("LShift", 2500.0, true),
            ],
            vec![in_front(1000.0, false), in_front(2000.0, true)],
        );

        assert_eq!(held(&record, 500.0), vec![W]);
        assert_eq!(held(&record, 1600.0), vec![]);
        assert_eq!(held(&record, 2100.0), vec![W]);
        assert_eq!(held(&record, 2600.0), vec![W, LEFT_SHIFT]);
    }

    #[test]
    fn live_input_follows_the_recorded_window_like_a_saved_record() {
        use crate::input_display::gamepad_buttons::SOUTH;
        let a = Input::Key(0x41);
        let polled = |inputs: &[Input]| {
            let mut state = DisplayState::default();
            for input in inputs {
                state.apply(*input, true);
            }
            state
        };
        let shown = |state: DisplayState| state.held().collect::<Vec<_>>();
        let mut rule = WindowRule::default();

        assert_eq!(shown(rule.follow(&polled(&[W]), true)), vec![W]);
        assert_eq!(shown(rule.follow(&polled(&[W, a]), false)), vec![]);
        assert_eq!(shown(rule.follow(&polled(&[W, a]), true)), vec![W]);
        assert_eq!(shown(rule.follow(&polled(&[W]), true)), vec![W]);
        assert_eq!(shown(rule.follow(&polled(&[W, a]), true)), vec![a, W]);
        assert_eq!(shown(rule.follow(&polled(&[a]), false)), vec![]);
        assert_eq!(shown(rule.follow(&polled(&[a, W]), false)), vec![]);
        assert_eq!(shown(rule.follow(&polled(&[a, W]), true)), vec![a]);

        let mut playing = polled(&[]);
        playing.show_gamepad(&GamepadSnapshot {
            axes: GamepadAxes {
                right_trigger: 200,
                ..Default::default()
            },
            ..pad(GamepadKind::PlayStation).with_button(SOUTH)
        });
        let behind = rule.follow(&playing, false);
        assert_eq!(shown(behind.clone()), vec![]);
        assert_eq!(behind.axes, GamepadAxes::default());
        assert_eq!(behind.controller, Some(GamepadKind::PlayStation));
        assert_eq!(rule.follow(&playing, true), playing);
    }

    #[test]
    fn a_recording_without_focus_changes_shows_everything() {
        let record = record(vec![key("W", 100.0, true)], vec![click(1, 100.0, true)]);

        assert_eq!(held(&record, 150.0), vec![W, LEFT_BUTTON]);
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
    fn a_key_already_held_when_the_take_started_shows_until_its_release() {
        let record = record(
            vec![key("W", 300.0, false)],
            vec![InputEvent {
                time_ms: 0.0,
                kind: InputEventKind::HeldKey(0x57),
            }],
        );

        assert_eq!(held(&record, 0.0), vec![W]);
        assert_eq!(held(&record, 299.0), vec![W]);
        assert_eq!(held(&record, 300.0), vec![]);
        assert!(record.used_keys_or_mouse());
    }

    #[test]
    fn a_release_without_a_press_holds_nothing() {
        let record = record(vec![key("W", 100.0, false)], Vec::new());

        assert_eq!(held(&record, 200.0), vec![]);
        assert!(!record.is_empty());
    }

    fn pad(kind: GamepadKind) -> GamepadSnapshot {
        GamepadSnapshot {
            kind,
            ..Default::default()
        }
    }

    #[test]
    fn gamepad_buttons_sticks_and_triggers_show_as_last_read() {
        use crate::input_display::gamepad_buttons::{DPAD_LEFT, SOUTH};
        let pushed = GamepadAxes {
            left_stick: [-127, 40],
            right_trigger: 200,
            ..Default::default()
        };
        let record = InputRecord::new(Vec::new()).with_gamepad(vec![
            (
                300.0,
                GamepadSnapshot {
                    axes: pushed,
                    ..pad(GamepadKind::Xbox).with_button(SOUTH)
                },
            ),
            (100.0, pad(GamepadKind::Xbox).with_button(DPAD_LEFT)),
            (500.0, pad(GamepadKind::Xbox)),
            (f64::NAN, pad(GamepadKind::Xbox).with_button(SOUTH)),
        ]);

        assert_eq!(held(&record, 50.0), vec![]);
        assert_eq!(held(&record, 100.0), vec![Input::GamepadButton(DPAD_LEFT)]);
        assert_eq!(record.state_at(100.0).axes, GamepadAxes::default());
        assert_eq!(held(&record, 499.0), vec![Input::GamepadButton(SOUTH)]);
        assert_eq!(record.state_at(499.0).axes, pushed);
        assert_eq!(held(&record, 500.0), vec![]);
        assert_eq!(record.state_at(500.0).axes, GamepadAxes::default());
        assert!(!record.is_empty());
    }

    #[test]
    fn a_saved_gamepad_shows_only_while_the_recorded_window_is_in_front() {
        use crate::input_display::gamepad_buttons::SOUTH;
        let pressed = InputEvent {
            time_ms: 100.0,
            kind: InputEventKind::Gamepad(pad(GamepadKind::Switch).with_button(SOUTH)),
        };
        let record = record(
            Vec::new(),
            vec![in_front(200.0, false), pressed, in_front(300.0, true)],
        );

        assert_eq!(held(&record, 150.0), vec![Input::GamepadButton(SOUTH)]);
        assert_eq!(held(&record, 250.0), vec![]);
        assert_eq!(record.state_at(250.0).axes, GamepadAxes::default());
        assert_eq!(record.state_at(250.0).controller, Some(GamepadKind::Switch));
        assert_eq!(held(&record, 350.0), vec![Input::GamepadButton(SOUTH)]);
        assert_eq!(record.controller(), Some(GamepadKind::Switch));
    }

    fn recording(takes: &[Vec<InputEvent>]) -> (tempfile::TempDir, RecordingMeta) {
        let directory = tempfile::tempdir().unwrap();
        let mut segments = Vec::new();
        for (index, events) in takes.iter().enumerate() {
            let folder = format!("content/segments/segment-{index}");
            let take = directory.path().join(&folder);
            std::fs::create_dir_all(&take).unwrap();
            InputEvents {
                events: events.clone(),
            }
            .write_to_file(&take.join(INPUT_EVENTS_FILE_NAME))
            .unwrap();
            segments
                .push(serde_json::json!({"display": {"path": format!("{folder}/display.mp4")}}));
        }
        let mut meta: RecordingMeta = serde_json::from_value(serde_json::json!({
            "pretty_name": "devices",
            "segments": segments
        }))
        .unwrap();
        meta.project_path = directory.path().to_path_buf();
        (directory, meta)
    }

    #[test]
    fn a_recording_that_used_a_gamepad_gets_the_gamepad_preset() {
        use crate::input_display::gamepad_buttons::SOUTH;
        let pressed = InputEvent {
            time_ms: 100.0,
            kind: InputEventKind::Gamepad(pad(GamepadKind::PlayStation).with_button(SOUTH)),
        };
        let resting = InputEvent {
            time_ms: 100.0,
            kind: InputEventKind::Gamepad(pad(GamepadKind::Xbox)),
        };
        let (_played, with_gamepad) = recording(&[vec![click(1, 10.0, true)], vec![pressed]]);
        let (_typed, with_mouse) = recording(&[vec![click(1, 10.0, true), resting]]);
        let (_silent, without_input) = recording(&[Vec::new()]);

        assert_eq!(
            preset_for_recording(&with_gamepad, "wasd-mouse", false),
            "gamepad"
        );
        assert_eq!(
            preset_for_recording(&with_gamepad, "my-preset", false),
            "gamepad"
        );
        assert_eq!(
            preset_for_recording(&with_gamepad, "gamepad", false),
            "gamepad"
        );
        assert_eq!(
            preset_for_recording(&with_mouse, "gamepad", false),
            "wasd-mouse"
        );
        assert_eq!(
            preset_for_recording(&with_mouse, "full-keyboard", false),
            "full-keyboard"
        );
        assert_eq!(
            preset_for_recording(&without_input, "gamepad", false),
            "gamepad"
        );
        assert_eq!(
            preset_for_recording(&with_gamepad, "user:p-pad", true),
            "user:p-pad"
        );
        assert_eq!(
            preset_for_recording(&with_mouse, "user:p-pad", true),
            "wasd-mouse"
        );
        assert_eq!(
            preset_for_recording(&without_input, "user:p-pad", true),
            "user:p-pad"
        );
        assert_eq!(
            preset_for_recording(&without_input, "my-preset", false),
            "my-preset"
        );
    }

    #[test]
    fn a_record_names_the_controller_that_first_produced_input() {
        use crate::input_display::gamepad_buttons::NORTH;
        let played = InputRecord::new(Vec::new()).with_gamepad(vec![
            (0.0, pad(GamepadKind::Switch)),
            (200.0, pad(GamepadKind::PlayStation).with_button(NORTH)),
            (400.0, pad(GamepadKind::Xbox).with_button(NORTH)),
        ]);
        let untouched =
            InputRecord::new(Vec::new()).with_gamepad(vec![(0.0, pad(GamepadKind::Switch))]);

        assert_eq!(played.controller(), Some(GamepadKind::PlayStation));
        assert_eq!(
            played.state_at(10.0).controller,
            Some(GamepadKind::PlayStation)
        );
        assert_eq!(untouched.controller(), None);
        assert_eq!(untouched.state_at(10.0).controller, None);
        assert_eq!(
            record(vec![key("W", 0.0, true)], Vec::new()).controller(),
            None
        );
    }

    #[test]
    fn a_recording_without_input_gives_an_empty_record() {
        assert!(record(Vec::new(), Vec::new()).is_empty());
    }
}
