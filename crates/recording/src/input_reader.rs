use crate::gamepad::GamepadWatch;
use frameever_project::{
    KeyPressEvent,
    input_display::{GamepadSnapshot, InputEvent, InputEventKind, key_code},
};

const WHEEL_NOTCH: i32 = 120;
const MOUSE_BUTTONS: std::ops::RangeInclusive<usize> = 1..=5;

#[derive(Clone, Default)]
pub struct InputCapture {
    pub record: bool,
    pub target_window: Option<scap_targets::WindowId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputSample {
    pub buttons: Vec<bool>,
    pub wheel_steps: Vec<bool>,
    pub target_in_front: Option<bool>,
    pub gamepad: Option<GamepadSnapshot>,
}

pub fn push_input_changes(
    previous: &InputSample,
    current: &InputSample,
    time_ms: f64,
    events: &mut Vec<InputEvent>,
) {
    let mut push = |kind| events.push(InputEvent { time_ms, kind });
    if let Some(in_front) = current.target_in_front
        && previous.target_in_front != Some(in_front)
    {
        push(InputEventKind::TargetInFront(in_front));
    }
    for button in MOUSE_BUTTONS {
        let down = current.buttons.get(button).copied().unwrap_or(false);
        if down != previous.buttons.get(button).copied().unwrap_or(false) {
            push(InputEventKind::MouseButton {
                button: button as u8,
                down,
            });
        }
    }
    for up in &current.wheel_steps {
        push(InputEventKind::Wheel { up: *up });
    }
    if let Some(gamepad) = current.gamepad
        && previous.gamepad != Some(gamepad)
    {
        push(InputEventKind::Gamepad(gamepad));
    }
}

pub fn push_key_changes(
    previous: &[device_query::Keycode],
    current: &[device_query::Keycode],
    time_ms: f64,
    presses: &mut Vec<KeyPressEvent>,
) {
    let mut push = |key: &device_query::Keycode, down| {
        let (display, code) = crate::cursor::keycode_to_string(key);
        presses.push(KeyPressEvent {
            key: display,
            key_code: code,
            time_ms,
            down,
        });
    };
    for key in current {
        if !previous.contains(key) {
            push(key, true);
        }
    }
    for key in previous {
        if !current.contains(key) {
            push(key, false);
        }
    }
}

pub fn wheel_steps(pending: &mut i32, delta: i16) -> impl Iterator<Item = bool> + use<> {
    if (*pending > 0) != (delta > 0) {
        *pending = 0;
    }
    *pending += i32::from(delta);
    let steps = *pending / WHEEL_NOTCH;
    *pending -= steps * WHEEL_NOTCH;
    std::iter::repeat_n(steps > 0, steps.unsigned_abs() as usize)
}

const BUTTON_SLOTS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonChange {
    pub button: u8,
    pub down: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawMouseInput {
    pub wheel_steps: Vec<bool>,
    pub buttons: Vec<ButtonChange>,
}

pub fn button_changes(flags: u16) -> impl Iterator<Item = ButtonChange> {
    MOUSE_BUTTONS.flat_map(move |button| {
        let down_flag = 1u16 << (2 * (button - 1));
        [(down_flag, true), (down_flag << 1, false)]
            .into_iter()
            .filter(move |(flag, _)| flags & flag != 0)
            .map(move |(_, down)| ButtonChange {
                button: button as u8,
                down,
            })
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldButtons {
    raw: [bool; BUTTON_SLOTS],
    polled: [bool; BUTTON_SLOTS],
}

impl HeldButtons {
    pub fn apply(&mut self, change: ButtonChange) {
        if let Some(held) = self.raw.get_mut(usize::from(change.button)) {
            *held = change.down;
        }
    }

    pub fn merge(&mut self, polled: &[bool]) -> Vec<bool> {
        (0..polled.len().max(BUTTON_SLOTS))
            .map(|index| {
                let polled_down = polled.get(index).copied().unwrap_or(false);
                let (Some(raw), Some(was_polled_down)) =
                    (self.raw.get_mut(index), self.polled.get_mut(index))
                else {
                    return polled_down;
                };
                if *was_polled_down && !polled_down {
                    *raw = false;
                }
                *was_polled_down = polled_down;
                polled_down || *raw
            })
            .collect()
    }
}

enum RawMouseProbe {
    Device(platform::RawMouseReader),
    Scripted(Box<dyn FnMut() -> RawMouseInput>),
}

pub struct MouseInput {
    probe: Option<RawMouseProbe>,
    held: HeldButtons,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MouseRead {
    pub buttons: Vec<bool>,
    pub wheel_steps: Vec<bool>,
}

impl MouseInput {
    pub fn device() -> Self {
        Self {
            probe: platform::RawMouseReader::new().map(RawMouseProbe::Device),
            held: HeldButtons::default(),
        }
    }

    pub fn scripted(probe: impl FnMut() -> RawMouseInput + 'static) -> Self {
        Self {
            probe: Some(RawMouseProbe::Scripted(Box::new(probe))),
            held: HeldButtons::default(),
        }
    }

    pub fn read(&mut self, polled: &[bool]) -> MouseRead {
        let wheel_steps = self.keep_up();
        MouseRead {
            buttons: self.held.merge(polled),
            wheel_steps,
        }
    }

    pub fn keep_up(&mut self) -> Vec<bool> {
        let raw = match &mut self.probe {
            Some(RawMouseProbe::Device(reader)) => reader.drain(),
            Some(RawMouseProbe::Scripted(probe)) => probe(),
            None => RawMouseInput::default(),
        };
        for change in raw.buttons {
            self.held.apply(change);
        }
        raw.wheel_steps
    }
}

enum FocusProbe {
    Window(platform::TargetWindow),
    Scripted(Box<dyn FnMut() -> Option<bool>>),
}

pub struct TargetFocus {
    probe: Option<FocusProbe>,
    in_front: Option<bool>,
}

impl TargetFocus {
    pub fn none() -> Self {
        Self {
            probe: None,
            in_front: None,
        }
    }

    pub fn of(target_window: Option<&scap_targets::WindowId>) -> Self {
        Self {
            probe: target_window
                .and_then(platform::TargetWindow::new)
                .map(FocusProbe::Window),
            in_front: None,
        }
    }

    pub fn scripted(probe: impl FnMut() -> Option<bool> + 'static) -> Self {
        Self {
            probe: Some(FocusProbe::Scripted(Box::new(probe))),
            in_front: None,
        }
    }

    pub fn read(&mut self) -> Option<bool> {
        let read = match &mut self.probe {
            Some(FocusProbe::Window(window)) => window.in_front(),
            Some(FocusProbe::Scripted(probe)) => probe(),
            None => return None,
        };
        self.in_front = read.or(self.in_front);
        self.in_front
    }
}

pub struct InputReader {
    mouse: MouseInput,
    target: TargetFocus,
    gamepad: Option<GamepadWatch>,
}

pub struct InputTake {
    reader: InputReader,
    keys: Vec<device_query::Keycode>,
    sample: Option<InputSample>,
}

impl InputTake {
    pub fn new(reader: InputReader) -> Self {
        Self {
            reader,
            keys: Vec::new(),
            sample: None,
        }
    }

    pub fn tick(
        &mut self,
        time_ms: f64,
        keys: Vec<device_query::Keycode>,
        buttons: &[bool],
        events: &mut Vec<InputEvent>,
        presses: &mut Vec<KeyPressEvent>,
    ) {
        let mut sample = self.reader.sample(buttons);
        match &self.sample {
            Some(previous) => {
                push_input_changes(previous, &sample, time_ms, events);
                push_key_changes(&self.keys, &keys, time_ms, presses);
            }
            None => {
                sample.wheel_steps.clear();
                events.extend(
                    keys.iter()
                        .filter_map(|key| key_code(&crate::cursor::keycode_to_string(key).1))
                        .map(|code| InputEvent {
                            time_ms: 0.0,
                            kind: InputEventKind::HeldKey(code),
                        }),
                );
                push_input_changes(&InputSample::default(), &sample, 0.0, events);
            }
        }
        sample.wheel_steps.clear();
        self.sample = Some(sample);
        self.keys = keys;
    }
}

impl InputReader {
    pub fn new(capture: &InputCapture) -> Option<Self> {
        capture.record.then(|| Self {
            mouse: MouseInput::device(),
            target: TargetFocus::of(capture.target_window.as_ref()),
            gamepad: GamepadWatch::connected(),
        })
    }

    pub fn with_gamepads(mut self, gamepad: GamepadWatch) -> Self {
        self.gamepad = Some(gamepad);
        self
    }

    pub fn with_mouse(mut self, mouse: MouseInput) -> Self {
        self.mouse = mouse;
        self
    }

    pub fn sample(&mut self, buttons: &[bool]) -> InputSample {
        let target_in_front = self.target.read();
        let mouse = self.mouse.read(buttons);
        InputSample {
            buttons: mouse.buttons,
            wheel_steps: mouse.wheel_steps,
            target_in_front,
            gamepad: self.gamepad.as_mut().and_then(GamepadWatch::read),
        }
    }
}

#[cfg(windows)]
mod platform {
    use windows::{
        Win32::{
            Foundation::HWND,
            UI::{
                Input::{
                    GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
                    RID_INPUT, RIDEV_INPUTSINK, RIDEV_REMOVE, RIM_TYPEMOUSE,
                    RegisterRawInputDevices,
                },
                WindowsAndMessaging::{
                    CreateWindowExW, DefWindowProcW, DestroyWindow, GA_ROOTOWNER, GetAncestor,
                    GetForegroundWindow, HWND_MESSAGE, MSG, PM_REMOVE, PeekMessageW,
                    WINDOW_EX_STYLE, WINDOW_STYLE, WM_INPUT,
                },
            },
        },
        core::w,
    };

    const GENERIC_DESKTOP_PAGE: u16 = 0x01;
    const MOUSE_USAGE: u16 = 0x02;
    const WHEEL_FLAG: u16 = 0x0400;
    const MAX_MESSAGES_PER_DRAIN: usize = 8192;

    pub struct RawMouseReader {
        window: HWND,
        pending: i32,
    }

    impl RawMouseReader {
        pub fn new() -> Option<Self> {
            let window = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("Static"),
                    None,
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    None,
                    None,
                )
            }
            .ok()?;
            let device = RAWINPUTDEVICE {
                usUsagePage: GENERIC_DESKTOP_PAGE,
                usUsage: MOUSE_USAGE,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: window,
            };
            let registered = unsafe {
                RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
            };
            if let Err(error) = registered {
                tracing::warn!(%error, "Raw mouse input is not read");
                unsafe { DestroyWindow(window) }.ok();
                return None;
            }
            Some(Self { window, pending: 0 })
        }

        pub fn drain(&mut self) -> super::RawMouseInput {
            let mut input = super::RawMouseInput::default();
            let mut message = MSG::default();
            for _ in 0..MAX_MESSAGES_PER_DRAIN {
                if !unsafe { PeekMessageW(&mut message, Some(self.window), 0, 0, PM_REMOVE) }
                    .as_bool()
                {
                    break;
                }
                if message.message == WM_INPUT
                    && let Some((flags, data)) = mouse_buttons(HRAWINPUT(message.lParam.0 as _))
                {
                    input.buttons.extend(super::button_changes(flags));
                    if flags & WHEEL_FLAG != 0 {
                        input
                            .wheel_steps
                            .extend(super::wheel_steps(&mut self.pending, data as i16));
                    }
                }
                unsafe {
                    DefWindowProcW(
                        message.hwnd,
                        message.message,
                        message.wParam,
                        message.lParam,
                    )
                };
            }
            input
        }
    }

    impl Drop for RawMouseReader {
        fn drop(&mut self) {
            let device = RAWINPUTDEVICE {
                usUsagePage: GENERIC_DESKTOP_PAGE,
                usUsage: MOUSE_USAGE,
                dwFlags: RIDEV_REMOVE,
                hwndTarget: HWND::default(),
            };
            unsafe {
                RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
                    .ok();
                DestroyWindow(self.window).ok();
            }
        }
    }

    fn mouse_buttons(handle: HRAWINPUT) -> Option<(u16, u16)> {
        let mut raw = RAWINPUT::default();
        let mut size = std::mem::size_of::<RAWINPUT>() as u32;
        let read = unsafe {
            GetRawInputData(
                handle,
                RID_INPUT,
                Some(std::ptr::from_mut(&mut raw).cast()),
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if read == u32::MAX || raw.header.dwType != RIM_TYPEMOUSE.0 {
            return None;
        }
        let buttons = unsafe { raw.data.mouse.Anonymous.Anonymous };
        Some((buttons.usButtonFlags, buttons.usButtonData))
    }

    pub struct TargetWindow(HWND);

    impl TargetWindow {
        pub fn new(id: &scap_targets::WindowId) -> Option<Self> {
            scap_targets::Window::from_id(id)
                .map(|window| Self(root_owner(window.raw_handle().inner())))
        }

        pub fn in_front(&self) -> Option<bool> {
            let foreground = unsafe { GetForegroundWindow() };
            (!foreground.is_invalid()).then(|| root_owner(foreground) == self.0)
        }
    }

    fn root_owner(window: HWND) -> HWND {
        let owner = unsafe { GetAncestor(window, GA_ROOTOWNER) };
        if owner.is_invalid() { window } else { owner }
    }
}

#[cfg(not(windows))]
mod platform {
    pub struct RawMouseReader;

    impl RawMouseReader {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn drain(&mut self) -> super::RawMouseInput {
            super::RawMouseInput::default()
        }
    }

    pub struct TargetWindow;

    impl TargetWindow {
        pub fn new(_id: &scap_targets::WindowId) -> Option<Self> {
            None
        }

        pub fn in_front(&self) -> Option<bool> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use device_query::Keycode;

    fn sample(buttons: &[usize], wheel_steps: &[bool], in_front: Option<bool>) -> InputSample {
        let mut pressed = vec![false; 6];
        for button in buttons {
            pressed[*button] = true;
        }
        InputSample {
            buttons: pressed,
            wheel_steps: wheel_steps.to_vec(),
            target_in_front: in_front,
            gamepad: None,
        }
    }

    #[test]
    fn what_is_held_when_a_take_starts_is_recorded_as_pressed_at_its_first_instant() {
        use crate::gamepad::testing::Scripted;
        use frameever_project::{
            KeyboardEvents,
            input_display::{Input, InputEvents, InputRecord, gamepad_buttons, key_codes},
        };
        let pad = GamepadSnapshot::default().with_button(gamepad_buttons::SOUTH);
        let reader = InputReader::new(&InputCapture {
            record: true,
            target_window: None,
        })
        .unwrap()
        .with_gamepads(GamepadWatch::new(
            Scripted::default().then(&[(1, pad)]).then(&[(1, pad)]),
        ));
        let mut take = InputTake::new(reader);
        let mut events = Vec::new();
        let mut presses = Vec::new();

        take.tick(
            40.0,
            vec![Keycode::W, Keycode::LShift],
            &[false, true, false],
            &mut events,
            &mut presses,
        );
        events.retain(|event| !matches!(event.kind, InputEventKind::Wheel { .. }));
        assert!(presses.is_empty());
        assert_eq!(
            events
                .iter()
                .map(|event| (event.time_ms, event.kind))
                .collect::<Vec<_>>(),
            vec![
                (0.0, InputEventKind::HeldKey(0x57)),
                (0.0, InputEventKind::HeldKey(key_codes::SHIFT_LEFT)),
                (
                    0.0,
                    InputEventKind::MouseButton {
                        button: 1,
                        down: true
                    }
                ),
                (0.0, InputEventKind::Gamepad(pad)),
            ]
        );

        take.tick(
            56.0,
            vec![Keycode::LShift],
            &[false, false, false],
            &mut events,
            &mut presses,
        );
        assert_eq!(
            presses
                .iter()
                .map(|press| (press.key_code.as_str(), press.time_ms, press.down))
                .collect::<Vec<_>>(),
            vec![("W", 56.0, false)]
        );
        let record = InputRecord::from_events(&KeyboardEvents { presses }, &InputEvents { events });
        assert_eq!(
            record.state_at(10.0).held().collect::<Vec<_>>(),
            vec![
                Input::Key(0x57),
                Input::Key(key_codes::SHIFT_LEFT),
                Input::MouseButton(1),
                Input::GamepadButton(gamepad_buttons::SOUTH),
            ]
        );
        assert_eq!(
            record.state_at(60.0).held().collect::<Vec<_>>(),
            vec![
                Input::Key(key_codes::SHIFT_LEFT),
                Input::GamepadButton(gamepad_buttons::SOUTH),
            ]
        );
    }

    #[test]
    fn a_gamepad_is_recorded_whenever_what_it_reads_changes() {
        use frameever_project::input_display::{GamepadAxes, GamepadKind, gamepad_buttons};
        let with = |gamepad| InputSample {
            gamepad,
            ..sample(&[], &[], None)
        };
        let idle = GamepadSnapshot {
            kind: GamepadKind::PlayStation,
            ..Default::default()
        };
        let pressed = idle.with_button(gamepad_buttons::SOUTH);
        let pulled = GamepadSnapshot {
            axes: GamepadAxes {
                right_trigger: 90,
                ..Default::default()
            },
            ..pressed
        };

        assert_eq!(changes(&with(None), &with(None)), vec![]);
        assert_eq!(
            changes(&with(None), &with(Some(pressed))),
            vec![InputEventKind::Gamepad(pressed)]
        );
        assert_eq!(changes(&with(Some(pressed)), &with(Some(pressed))), vec![]);
        assert_eq!(
            changes(&with(Some(pressed)), &with(Some(pulled))),
            vec![InputEventKind::Gamepad(pulled)]
        );
        assert_eq!(
            changes(&with(Some(pulled)), &with(Some(idle))),
            vec![InputEventKind::Gamepad(idle)]
        );
        assert_eq!(changes(&with(Some(idle)), &with(None)), vec![]);
    }

    #[test]
    fn the_capture_loop_saves_what_the_gamepads_read_and_the_editor_shows_it() {
        use crate::gamepad::testing::Scripted;
        use frameever_project::{
            KeyboardEvents,
            input_display::{
                GamepadAxes, GamepadKind, Input, InputEvents, InputRecord, gamepad_buttons,
            },
        };
        let idle = GamepadSnapshot {
            kind: GamepadKind::Switch,
            ..Default::default()
        };
        let pushed = GamepadSnapshot {
            axes: GamepadAxes {
                left_stick: [127, 0],
                left_trigger: 255,
                ..Default::default()
            },
            ..idle.with_button(gamepad_buttons::DPAD_UP)
        };
        let mut reader = InputReader::new(&InputCapture {
            record: true,
            target_window: None,
        })
        .unwrap()
        .with_gamepads(GamepadWatch::new(
            Scripted::default()
                .then(&[(3, idle)])
                .then(&[(3, pushed)])
                .then(&[(3, pushed)])
                .then(&[(3, idle)]),
        ));
        let mut events = Vec::new();
        let mut last = InputSample::default();
        for tick in 0..4 {
            let sample = InputSample {
                wheel_steps: Vec::new(),
                ..reader.sample(&[false; 6])
            };
            push_input_changes(&last, &sample, f64::from(tick) * 16.0, &mut events);
            last = sample;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.bin");
        InputEvents { events }.write_to_file(&path).unwrap();
        let saved = InputEvents::load_from_file(&path).unwrap();

        assert_eq!(
            saved
                .events
                .iter()
                .map(|event| (event.time_ms, event.kind))
                .collect::<Vec<_>>(),
            vec![
                (16.0, InputEventKind::Gamepad(pushed)),
                (48.0, InputEventKind::Gamepad(idle)),
            ]
        );
        let record = InputRecord::from_events(&KeyboardEvents::default(), &saved);
        let shown = record.state_at(20.0);
        assert_eq!(
            shown.held().collect::<Vec<_>>(),
            vec![Input::GamepadButton(gamepad_buttons::DPAD_UP)]
        );
        assert_eq!(shown.axes, pushed.axes);
        assert_eq!(shown.controller, Some(GamepadKind::Switch));
        assert_eq!(record.state_at(50.0).axes, GamepadAxes::default());
    }

    fn changes(previous: &InputSample, current: &InputSample) -> Vec<InputEventKind> {
        let mut events = Vec::new();
        push_input_changes(previous, current, 48.0, &mut events);
        assert!(events.iter().all(|event| event.time_ms == 48.0));
        events.into_iter().map(|event| event.kind).collect()
    }

    #[test]
    fn every_mouse_button_including_the_side_buttons_records_its_press_and_release() {
        assert_eq!(
            changes(&sample(&[1, 2], &[], None), &sample(&[2, 4, 5], &[], None)),
            vec![
                InputEventKind::MouseButton {
                    button: 1,
                    down: false
                },
                InputEventKind::MouseButton {
                    button: 4,
                    down: true
                },
                InputEventKind::MouseButton {
                    button: 5,
                    down: true
                },
            ]
        );
    }

    #[test]
    fn wheel_steps_are_recorded_in_the_order_they_were_read() {
        assert_eq!(
            changes(
                &sample(&[], &[false], None),
                &sample(&[], &[true, false], None)
            ),
            vec![
                InputEventKind::Wheel { up: true },
                InputEventKind::Wheel { up: false }
            ]
        );
    }

    #[test]
    fn the_target_window_coming_to_the_front_or_leaving_it_is_recorded_once() {
        let unknown = sample(&[], &[], None);
        let front = sample(&[], &[], Some(true));
        let behind = sample(&[], &[], Some(false));

        assert_eq!(
            changes(&unknown, &front),
            vec![InputEventKind::TargetInFront(true)]
        );
        assert_eq!(changes(&front, &front), vec![]);
        assert_eq!(
            changes(&front, &behind),
            vec![InputEventKind::TargetInFront(false)]
        );
        assert_eq!(changes(&unknown, &unknown), vec![]);
    }

    #[test]
    fn an_unchanged_sample_records_nothing() {
        let held = sample(&[1, 3], &[], Some(true));

        assert_eq!(changes(&held, &held), vec![]);
    }

    #[test]
    fn a_wheel_notch_is_one_step_and_fine_scrolling_adds_up_to_one() {
        let mut pending = 0;

        assert_eq!(wheel_steps(&mut pending, 120).collect::<Vec<_>>(), [true]);
        assert_eq!(
            wheel_steps(&mut pending, -240).collect::<Vec<_>>(),
            [false, false]
        );
        assert_eq!(wheel_steps(&mut pending, 40).count(), 0);
        assert_eq!(wheel_steps(&mut pending, 40).count(), 0);
        assert_eq!(wheel_steps(&mut pending, 40).collect::<Vec<_>>(), [true]);
        assert_eq!(wheel_steps(&mut pending, 100).count(), 0);
        assert_eq!(wheel_steps(&mut pending, -100).count(), 0);
        assert_eq!(wheel_steps(&mut pending, -20).collect::<Vec<_>>(), [false]);
    }

    fn press(key: &str, key_code: &str, down: bool) -> KeyPressEvent {
        KeyPressEvent {
            key: key.to_string(),
            key_code: key_code.to_string(),
            time_ms: 32.0,
            down,
        }
    }

    #[test]
    fn new_keys_go_down_in_held_order_before_released_keys_go_up() {
        let mut presses = vec![];
        push_key_changes(
            &[Keycode::A, Keycode::LShift],
            &[Keycode::LShift, Keycode::Space, Keycode::B],
            32.0,
            &mut presses,
        );

        assert_eq!(
            presses,
            vec![
                press("Space", "Space", true),
                press("b", "B", true),
                press("a", "A", false),
            ]
        );
    }

    #[test]
    fn held_or_absent_keys_record_nothing() {
        let mut presses = vec![];
        push_key_changes(
            &[Keycode::LMeta, Keycode::A],
            &[Keycode::LMeta, Keycode::A],
            32.0,
            &mut presses,
        );
        push_key_changes(&[], &[], 48.0, &mut presses);

        assert!(presses.is_empty());
    }

    #[test]
    fn nothing_is_read_when_input_is_not_recorded() {
        assert!(InputReader::new(&InputCapture::default()).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn the_wheel_source_registers_drains_and_unregisters_without_a_hook() {
        let mut reader = InputReader::new(&InputCapture {
            record: true,
            target_window: None,
        })
        .unwrap();

        assert!(matches!(reader.mouse.probe, Some(RawMouseProbe::Device(_))));
        let sample = reader.sample(&[false; 6]);
        assert_eq!(sample.target_in_front, None);
        drop(reader);
        assert!(platform::RawMouseReader::new().is_some());
    }

    fn change(button: u8, down: bool) -> ButtonChange {
        ButtonChange { button, down }
    }

    #[test]
    fn each_raw_button_flag_is_a_press_or_release_of_its_button() {
        let pairs = [
            (0x0001, change(1, true)),
            (0x0002, change(1, false)),
            (0x0004, change(2, true)),
            (0x0008, change(2, false)),
            (0x0010, change(3, true)),
            (0x0020, change(3, false)),
            (0x0040, change(4, true)),
            (0x0080, change(4, false)),
            (0x0100, change(5, true)),
            (0x0200, change(5, false)),
        ];
        for (flag, expected) in pairs {
            assert_eq!(button_changes(flag).collect::<Vec<_>>(), vec![expected]);
        }
        assert_eq!(button_changes(0x0400).count(), 0);
        assert_eq!(button_changes(0).count(), 0);
        assert_eq!(
            button_changes(0x0004 | 0x0400 | 0x0080).collect::<Vec<_>>(),
            vec![change(2, true), change(4, false)]
        );
    }

    fn held_after(changes: &[ButtonChange]) -> Vec<bool> {
        let mut held = HeldButtons::default();
        for change in changes {
            held.apply(*change);
        }
        held.merge(&[false; 6])
    }

    #[test]
    fn raw_presses_and_releases_fold_into_the_buttons_held() {
        assert_eq!(held_after(&[]), vec![false; 6]);
        assert_eq!(
            held_after(&[change(2, true)]),
            vec![false, false, true, false, false, false]
        );
        assert_eq!(
            held_after(&[change(2, true), change(2, false)]),
            vec![false; 6]
        );
        assert_eq!(
            held_after(&[change(2, true), change(2, true)]),
            vec![false, false, true, false, false, false]
        );
        assert_eq!(
            held_after(&[change(2, true), change(2, true), change(2, false)]),
            vec![false; 6]
        );
        assert_eq!(held_after(&[change(3, false)]), vec![false; 6]);
        assert_eq!(
            held_after(&[1, 2, 3, 4, 5].map(|button| change(button, true))),
            vec![false, true, true, true, true, true]
        );
        assert_eq!(
            held_after(&[
                change(1, true),
                change(5, true),
                change(1, false),
                change(4, true)
            ]),
            vec![false, false, false, false, true, true]
        );
    }

    fn buttons(held: &[usize]) -> Vec<bool> {
        let mut pressed = vec![false; 6];
        for button in held {
            pressed[*button] = true;
        }
        pressed
    }

    #[test]
    fn a_button_is_down_while_either_the_poll_or_raw_input_says_so() {
        let mut held = HeldButtons::default();
        assert_eq!(held.merge(&buttons(&[1])), buttons(&[1]));
        held.apply(change(2, true));
        assert_eq!(held.merge(&buttons(&[1])), buttons(&[1, 2]));
        assert_eq!(held.merge(&buttons(&[])), buttons(&[2]));
        held.apply(change(2, false));
        assert_eq!(held.merge(&buttons(&[])), buttons(&[]));
        assert_eq!(held.merge(&[false, true, false]), buttons(&[1]));
    }

    #[test]
    fn a_release_raw_input_missed_is_taken_from_the_poll_once_the_poll_saw_the_button_down() {
        let mut held = HeldButtons::default();
        held.apply(change(1, true));
        assert_eq!(held.merge(&buttons(&[1])), buttons(&[1]));
        assert_eq!(held.merge(&buttons(&[])), buttons(&[]));
        assert_eq!(held.merge(&buttons(&[])), buttons(&[]));

        held.apply(change(2, true));
        assert_eq!(held.merge(&buttons(&[])), buttons(&[2]));
        assert_eq!(held.merge(&buttons(&[])), buttons(&[2]));
        held.apply(change(2, false));
        assert_eq!(held.merge(&buttons(&[])), buttons(&[]));
    }

    #[test]
    fn a_button_already_held_when_reading_starts_shows_as_the_poll_has_it_and_never_sticks() {
        let mut polled_works = HeldButtons::default();
        assert_eq!(polled_works.merge(&buttons(&[2])), buttons(&[2]));
        polled_works.apply(change(2, false));
        assert_eq!(polled_works.merge(&buttons(&[])), buttons(&[]));

        let mut poll_blind = HeldButtons::default();
        assert_eq!(poll_blind.merge(&buttons(&[])), buttons(&[]));
        poll_blind.apply(change(2, false));
        assert_eq!(poll_blind.merge(&buttons(&[])), buttons(&[]));
        poll_blind.apply(change(2, true));
        assert_eq!(poll_blind.merge(&buttons(&[])), buttons(&[2]));
    }

    fn scripted_mouse(ticks: Vec<Vec<ButtonChange>>) -> MouseInput {
        let mut ticks = ticks.into_iter();
        MouseInput::scripted(move || RawMouseInput {
            wheel_steps: Vec::new(),
            buttons: ticks.next().unwrap_or_default(),
        })
    }

    #[test]
    fn a_hold_only_raw_input_sees_is_recorded_from_its_press_to_its_release() {
        use frameever_project::{
            KeyboardEvents,
            input_display::{Input, InputEvents, InputRecord},
        };
        let reader = InputReader::new(&InputCapture {
            record: true,
            target_window: None,
        })
        .unwrap()
        .with_gamepads(GamepadWatch::new(
            crate::gamepad::testing::Scripted::default(),
        ))
        .with_mouse(scripted_mouse(vec![
            vec![],
            vec![change(2, true)],
            vec![],
            vec![change(2, false)],
            vec![],
        ]));
        let mut take = InputTake::new(reader);
        let mut events = Vec::new();
        let mut presses = Vec::new();
        for time_ms in [0.0, 16.0, 32.0, 48.0, 64.0] {
            take.tick(time_ms, Vec::new(), &[false; 6], &mut events, &mut presses);
        }

        assert_eq!(
            events
                .iter()
                .map(|event| (event.time_ms, event.kind))
                .collect::<Vec<_>>(),
            vec![
                (
                    16.0,
                    InputEventKind::MouseButton {
                        button: 2,
                        down: true
                    }
                ),
                (
                    48.0,
                    InputEventKind::MouseButton {
                        button: 2,
                        down: false
                    }
                ),
            ]
        );
        let record = InputRecord::from_events(&KeyboardEvents::default(), &InputEvents { events });
        assert_eq!(record.state_at(8.0).held().count(), 0);
        assert_eq!(
            record.state_at(40.0).held().collect::<Vec<_>>(),
            vec![Input::MouseButton(2)]
        );
        assert_eq!(record.state_at(56.0).held().count(), 0);
    }
}
