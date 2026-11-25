use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    ops::RangeInclusive,
    sync::{Arc, LazyLock},
    time::Instant,
};

pub const BUTTON_SLOTS: usize = 6;
pub const MOUSE_BUTTONS: RangeInclusive<usize> = 1..=5;
pub const BACKLOG_LIMIT: usize = 1024;
const WHEEL_NOTCH: i32 = 120;

pub type ButtonsHeld = [bool; BUTTON_SLOTS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonChange {
    pub button: u8,
    pub down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseChange {
    Button(ButtonChange),
    Wheel { up: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEvent {
    pub at: Instant,
    pub change: MouseChange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MouseTake {
    pub held_before: ButtonsHeld,
    pub events: Vec<MouseEvent>,
    pub continuous: bool,
}

impl MouseTake {
    pub fn held(&self) -> ButtonsHeld {
        let mut held = self.held_before;
        for event in &self.events {
            if let MouseChange::Button(change) = event.change {
                apply(&mut held, change);
            }
        }
        held
    }
}

pub fn apply(held: &mut ButtonsHeld, change: ButtonChange) {
    if let Some(slot) = held.get_mut(usize::from(change.button)) {
        *slot = change.down;
    }
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

pub fn wheel_steps(pending: &mut i32, delta: i16) -> impl Iterator<Item = bool> + use<> {
    if (*pending > 0) != (delta > 0) {
        *pending = 0;
    }
    *pending += i32::from(delta);
    let steps = *pending / WHEEL_NOTCH;
    *pending -= steps * WHEEL_NOTCH;
    std::iter::repeat_n(steps > 0, steps.unsigned_abs() as usize)
}

struct Backlog {
    id: u64,
    held_before: ButtonsHeld,
    held_after: ButtonsHeld,
    events: VecDeque<MouseEvent>,
    continuous: bool,
}

#[derive(Default)]
struct HubState {
    pending_wheel: i32,
    next_id: u64,
    backlogs: Vec<Backlog>,
}

impl HubState {
    fn subscribe(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.backlogs.push(Backlog {
            id,
            held_before: ButtonsHeld::default(),
            held_after: ButtonsHeld::default(),
            events: VecDeque::new(),
            continuous: false,
        });
        id
    }

    fn unsubscribe(&mut self, id: u64) -> bool {
        self.backlogs.retain(|backlog| backlog.id != id);
        !self.backlogs.is_empty()
    }

    fn forget(&mut self) {
        self.pending_wheel = 0;
    }

    fn packet(&mut self, at: Instant, buttons: impl IntoIterator<Item = ButtonChange>, wheel: i16) {
        for change in buttons {
            self.push(MouseEvent {
                at,
                change: MouseChange::Button(change),
            });
        }
        if wheel != 0 {
            for up in wheel_steps(&mut self.pending_wheel, wheel) {
                self.push(MouseEvent {
                    at,
                    change: MouseChange::Wheel { up },
                });
            }
        }
    }

    fn push(&mut self, event: MouseEvent) {
        for backlog in &mut self.backlogs {
            if backlog.events.len() >= BACKLOG_LIMIT
                && let Some(oldest) = backlog.events.pop_front()
            {
                if let MouseChange::Button(change) = oldest.change {
                    apply(&mut backlog.held_before, change);
                }
                backlog.continuous = false;
            }
            if let MouseChange::Button(change) = event.change {
                apply(&mut backlog.held_after, change);
            }
            backlog.events.push_back(event);
        }
    }

    fn take(&mut self, id: u64) -> MouseTake {
        let Some(backlog) = self.backlogs.iter_mut().find(|backlog| backlog.id == id) else {
            return MouseTake {
                held_before: ButtonsHeld::default(),
                events: Vec::new(),
                continuous: false,
            };
        };
        let take = MouseTake {
            held_before: backlog.held_before,
            events: backlog.events.drain(..).collect(),
            continuous: backlog.continuous,
        };
        backlog.held_before = backlog.held_after;
        backlog.continuous = true;
        take
    }
}

#[derive(Clone)]
pub struct MouseSink(Arc<Mutex<HubState>>);

impl MouseSink {
    pub fn packet(&self, at: Instant, buttons: impl IntoIterator<Item = ButtonChange>, wheel: i16) {
        self.0.lock().packet(at, buttons, wheel);
    }
}

pub type StopSource = Box<dyn FnOnce() + Send>;
type StartSource = Box<dyn Fn(MouseSink) -> Option<StopSource> + Send + Sync>;

pub struct MouseHub {
    start: StartSource,
    running: Mutex<Option<StopSource>>,
    state: Arc<Mutex<HubState>>,
}

static DEVICE_HUB: LazyLock<Arc<MouseHub>> = LazyLock::new(|| MouseHub::new(platform::start));

impl MouseHub {
    pub fn new(
        start: impl Fn(MouseSink) -> Option<StopSource> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            start: Box::new(start),
            running: Mutex::new(None),
            state: Arc::default(),
        })
    }

    pub fn subscribe(self: &Arc<Self>) -> Subscription {
        let mut running = self.running.lock();
        let id = self.state.lock().subscribe();
        if running.is_none() {
            *running = (self.start)(MouseSink(self.state.clone()));
        }
        Subscription {
            hub: self.clone(),
            id,
        }
    }

    fn unsubscribe(&self, id: u64) {
        let mut running = self.running.lock();
        if self.state.lock().unsubscribe(id) {
            return;
        }
        if let Some(stop) = running.take() {
            stop();
        }
        self.state.lock().forget();
    }

    #[cfg(test)]
    fn is_running(&self) -> bool {
        self.running.lock().is_some()
    }

    #[cfg(test)]
    pub(crate) fn backlog_lengths(&self) -> Vec<usize> {
        self.state
            .lock()
            .backlogs
            .iter()
            .map(|backlog| backlog.events.len())
            .collect()
    }
}

pub struct Subscription {
    hub: Arc<MouseHub>,
    id: u64,
}

impl Subscription {
    pub fn device() -> Self {
        DEVICE_HUB.subscribe()
    }

    pub fn take(&mut self) -> MouseTake {
        self.hub.state.lock().take(self.id)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.hub.unsubscribe(self.id);
    }
}

#[cfg(windows)]
mod platform {
    use super::{MouseSink, StopSource, button_changes};
    use std::time::Instant;
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::{
                Input::{
                    GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
                    RID_INPUT, RIDEV_INPUTSINK, RIDEV_REMOVE, RIM_TYPEMOUSE,
                    RegisterRawInputDevices,
                },
                WindowsAndMessaging::{
                    CreateWindowExW, DefWindowProcW, DestroyWindow, GetMessageW, HWND_MESSAGE, MSG,
                    PostMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_INPUT,
                },
            },
        },
        core::w,
    };

    const GENERIC_DESKTOP_PAGE: u16 = 0x01;
    const MOUSE_USAGE: u16 = 0x02;
    const WHEEL_FLAG: u16 = 0x0400;

    pub fn start(sink: MouseSink) -> Option<StopSource> {
        let (ready, opened) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("raw-mouse".into())
            .spawn(move || read_packets(sink, ready))
            .inspect_err(|error| tracing::warn!(%error, "Raw mouse thread did not start"))
            .ok()?;
        let Ok(Some(window)) = opened.recv() else {
            thread.join().ok();
            return None;
        };
        Some(Box::new(move || {
            let posted = unsafe {
                PostMessageW(Some(HWND(window as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0))
            };
            match posted {
                Ok(()) => {
                    thread.join().ok();
                }
                Err(error) => tracing::warn!(%error, "Raw mouse thread was not stopped"),
            }
        }))
    }

    fn read_packets(sink: MouseSink, ready: std::sync::mpsc::SyncSender<Option<isize>>) {
        let Some(window) = open() else {
            ready.send(None).ok();
            return;
        };
        ready.send(Some(window.0 as isize)).ok();
        let mut message = MSG::default();
        while unsafe { GetMessageW(&mut message, Some(window), 0, 0) }.0 > 0 {
            if message.message == WM_CLOSE {
                break;
            }
            if message.message == WM_INPUT {
                let at = Instant::now();
                if let Some((flags, data)) = mouse_buttons(HRAWINPUT(message.lParam.0 as _))
                    && flags != 0
                {
                    let wheel = if flags & WHEEL_FLAG != 0 {
                        data as i16
                    } else {
                        0
                    };
                    sink.packet(at, button_changes(flags), wheel);
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
        close(window);
    }

    fn open() -> Option<HWND> {
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
        .inspect_err(|error| tracing::warn!(%error, "Raw mouse window was not created"))
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
        Some(window)
    }

    fn close(window: HWND) {
        let device = RAWINPUTDEVICE {
            usUsagePage: GENERIC_DESKTOP_PAGE,
            usUsage: MOUSE_USAGE,
            dwFlags: RIDEV_REMOVE,
            hwndTarget: HWND::default(),
        };
        unsafe {
            RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32).ok();
            DestroyWindow(window).ok();
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
}

#[cfg(not(windows))]
mod platform {
    use super::{MouseSink, StopSource};

    pub fn start(_sink: MouseSink) -> Option<StopSource> {
        None
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::{ButtonChange, MouseHub, MouseSink, Subscription};
    use parking_lot::Mutex;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Instant,
    };

    pub struct ScriptedHub {
        pub hub: Arc<MouseHub>,
        sink: Arc<Mutex<Option<MouseSink>>>,
        starts: Arc<AtomicUsize>,
        stops: Arc<AtomicUsize>,
    }

    impl ScriptedHub {
        pub fn new() -> Self {
            let sink = Arc::new(Mutex::new(None));
            let starts = Arc::new(AtomicUsize::new(0));
            let stops = Arc::new(AtomicUsize::new(0));
            let hub = MouseHub::new({
                let sink = sink.clone();
                let starts = starts.clone();
                let stops = stops.clone();
                move |started: MouseSink| {
                    starts.fetch_add(1, Ordering::SeqCst);
                    *sink.lock() = Some(started);
                    let sink = sink.clone();
                    let stops = stops.clone();
                    Some(Box::new(move || {
                        *sink.lock() = None;
                        stops.fetch_add(1, Ordering::SeqCst);
                    }) as super::StopSource)
                }
            });
            Self {
                hub,
                sink,
                starts,
                stops,
            }
        }

        pub fn subscribe(&self) -> Subscription {
            self.hub.subscribe()
        }

        pub fn press(&self, at: Instant, button: u8, down: bool) {
            if let Some(sink) = &*self.sink.lock() {
                sink.packet(at, [ButtonChange { button, down }], 0);
            }
        }

        pub fn wheel(&self, at: Instant, delta: i16) {
            if let Some(sink) = &*self.sink.lock() {
                sink.packet(at, [], delta);
            }
        }

        pub fn starts(&self) -> usize {
            self.starts.load(Ordering::SeqCst)
        }

        pub fn stops(&self) -> usize {
            self.stops.load(Ordering::SeqCst)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::ScriptedHub, *};
    use std::time::Duration;

    fn button(at: Instant, button: u8, down: bool) -> MouseEvent {
        MouseEvent {
            at,
            change: MouseChange::Button(ButtonChange { button, down }),
        }
    }

    fn wheel(at: Instant, up: bool) -> MouseEvent {
        MouseEvent {
            at,
            change: MouseChange::Wheel { up },
        }
    }

    fn held(buttons: &[usize]) -> ButtonsHeld {
        let mut held = ButtonsHeld::default();
        for button in buttons {
            held[*button] = true;
        }
        held
    }

    #[test]
    fn every_subscription_sees_every_change_in_order_with_its_time() {
        let hub = ScriptedHub::new();
        let mut first = hub.subscribe();
        let mut second = hub.subscribe();
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);

        hub.press(at(1), 1, true);
        hub.wheel(at(2), 120);
        hub.wheel(at(3), -60);
        hub.wheel(at(4), -60);
        hub.press(at(5), 1, false);
        let expected = vec![
            button(at(1), 1, true),
            wheel(at(2), true),
            wheel(at(4), false),
            button(at(5), 1, false),
        ];

        let taken = first.take();
        assert_eq!(taken.events, expected);
        assert_eq!(taken.held_before, held(&[]));
        assert!(first.take().events.is_empty());
        assert_eq!(second.take().events, expected);
        hub.press(at(6), 3, true);
        assert_eq!(first.take().events, vec![button(at(6), 3, true)]);
        assert_eq!(second.take().events, vec![button(at(6), 3, true)]);
    }

    #[test]
    fn a_subscription_knows_only_what_happened_since_it_was_made() {
        let hub = ScriptedHub::new();
        let mut first = hub.subscribe();
        hub.press(Instant::now(), 2, true);

        let mut late = hub.subscribe();
        let taken = late.take();
        assert_eq!(taken.held(), held(&[]));
        assert!(taken.events.is_empty());
        assert!(!taken.continuous);

        hub.press(Instant::now(), 2, false);
        hub.press(Instant::now(), 3, true);
        let next = late.take();
        assert!(next.continuous);
        assert_eq!(next.held_before, held(&[]));
        assert_eq!(next.held(), held(&[3]));
        assert_eq!(late.take().held_before, held(&[3]));
        assert_eq!(first.take().held(), held(&[3]));
    }

    #[test]
    fn a_release_the_hub_never_saw_does_not_hold_the_button_for_later_subscriptions() {
        let hub = ScriptedHub::new();
        let _always_on = hub.subscribe();
        hub.press(Instant::now(), 1, true);

        let mut next_recording = hub.subscribe();
        assert_eq!(next_recording.take().held(), held(&[]));
        hub.press(Instant::now(), 1, true);
        let taken = next_recording.take();
        assert_eq!(taken.events.len(), 1);
        assert_eq!(taken.held(), held(&[1]));
    }

    #[test]
    fn a_subscription_that_reads_rarely_keeps_only_the_newest_events_and_still_knows_what_is_held()
    {
        let hub = ScriptedHub::new();
        let mut rare = hub.subscribe();
        let mut regular = hub.subscribe();
        let start = Instant::now();
        assert!(rare.take().events.is_empty());
        assert!(regular.take().events.is_empty());

        hub.press(start, 3, true);
        hub.press(start, 4, true);
        hub.press(start, 4, false);
        assert_eq!(regular.take().events.len(), 3);
        for step in 0..BACKLOG_LIMIT {
            hub.wheel(start + Duration::from_millis(step as u64 + 1), 120);
        }

        let taken = rare.take();
        assert_eq!(taken.events.len(), BACKLOG_LIMIT);
        assert!(!taken.continuous);
        assert_eq!(taken.held_before, held(&[3]));
        assert_eq!(taken.held(), held(&[3]));
        assert_eq!(
            taken.events.first(),
            Some(&wheel(start + Duration::from_millis(1), true))
        );
        assert_eq!(
            taken.events.last(),
            Some(&wheel(
                start + Duration::from_millis(BACKLOG_LIMIT as u64),
                true
            ))
        );
        let caught_up = regular.take();
        assert!(caught_up.continuous);
        assert_eq!(caught_up.events.len(), BACKLOG_LIMIT);
        assert!(rare.take().continuous);
    }

    #[test]
    fn the_hub_starts_with_the_first_subscription_stops_after_the_last_and_starts_again() {
        let hub = ScriptedHub::new();
        assert_eq!((hub.starts(), hub.stops()), (0, 0));

        let first = hub.subscribe();
        let second = hub.subscribe();
        assert_eq!((hub.starts(), hub.stops()), (1, 0));
        hub.press(Instant::now(), 1, true);
        drop(first);
        assert_eq!((hub.starts(), hub.stops()), (1, 0));
        drop(second);
        assert_eq!((hub.starts(), hub.stops()), (1, 1));

        let mut again = hub.subscribe();
        assert_eq!((hub.starts(), hub.stops()), (2, 1));
        assert_eq!(again.take().held(), held(&[]));
        hub.press(Instant::now(), 2, true);
        assert_eq!(again.take().held(), held(&[2]));
    }

    #[test]
    fn each_raw_button_flag_is_a_press_or_release_of_its_button() {
        let change = |button, down| ButtonChange { button, down };
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

    #[cfg(windows)]
    #[test]
    fn the_device_hub_registers_reads_and_unregisters_without_a_hook() {
        let mut first = Subscription::device();
        assert!(DEVICE_HUB.is_running());
        assert_eq!(first.take().held_before.len(), BUTTON_SLOTS);
        drop(first);
        let _again = Subscription::device();
        assert!(DEVICE_HUB.is_running());
    }
}
