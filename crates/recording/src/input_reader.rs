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
    wheel: Option<platform::WheelReader>,
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
            wheel: platform::WheelReader::new(),
            target: TargetFocus::of(capture.target_window.as_ref()),
            gamepad: GamepadWatch::connected(),
        })
    }

    pub fn with_gamepads(mut self, gamepad: GamepadWatch) -> Self {
        self.gamepad = Some(gamepad);
        self
    }

    pub fn sample(&mut self, buttons: &[bool]) -> InputSample {
        let target_in_front = self.target.read();
        InputSample {
            buttons: buttons.to_vec(),
            wheel_steps: self
                .wheel
                .as_mut()
                .map(platform::WheelReader::drain)
                .unwrap_or_default(),
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

    pub struct WheelReader {
        window: HWND,
        pending: i32,
    }

    impl WheelReader {
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
                tracing::warn!(%error, "Wheel steps are not recorded");
                unsafe { DestroyWindow(window) }.ok();
                return None;
            }
            Some(Self { window, pending: 0 })
        }

        pub fn drain(&mut self) -> Vec<bool> {
            let mut steps = Vec::new();
            let mut message = MSG::default();
            for _ in 0..MAX_MESSAGES_PER_DRAIN {
                if !unsafe { PeekMessageW(&mut message, Some(self.window), 0, 0, PM_REMOVE) }
                    .as_bool()
                {
                    break;
                }
                if message.message == WM_INPUT
                    && let Some(delta) = wheel_delta(HRAWINPUT(message.lParam.0 as _))
                {
                    steps.extend(super::wheel_steps(&mut self.pending, delta));
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
            steps
        }
    }

    impl Drop for WheelReader {
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

    fn wheel_delta(handle: HRAWINPUT) -> Option<i16> {
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
        (buttons.usButtonFlags & WHEEL_FLAG != 0).then_some(buttons.usButtonData as i16)
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
    pub struct WheelReader;

    impl WheelReader {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn drain(&mut self) -> Vec<bool> {
            Vec::new()
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

        assert!(reader.wheel.is_some());
        let sample = reader.sample(&[false; 6]);
        assert_eq!(sample.target_in_front, None);
        drop(reader);
        assert!(platform::WheelReader::new().is_some());
    }
}
