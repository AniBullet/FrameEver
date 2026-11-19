use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use frameever_project::input_display::{GamepadKind, GamepadSnapshot};

static RECORDING_WATCHES: AtomicUsize = AtomicUsize::new(0);

struct RecordingWatch;

impl RecordingWatch {
    fn open() -> Self {
        RECORDING_WATCHES.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for RecordingWatch {
    fn drop(&mut self) {
        RECORDING_WATCHES.fetch_sub(1, Ordering::SeqCst);
    }
}

pub const MAX_GAMEPADS: usize = 4;

pub type GamepadId = u32;

pub trait GamepadPoll {
    fn poll(&mut self) -> Vec<(GamepadId, GamepadSnapshot)>;
}

pub struct GamepadWatch {
    source: Box<dyn GamepadPoll>,
    shown: Option<GamepadId>,
    controller: Option<GamepadKind>,
    _recording: Option<RecordingWatch>,
}

impl GamepadWatch {
    pub fn new(source: impl GamepadPoll + 'static) -> Self {
        Self {
            source: Box::new(source),
            shown: None,
            controller: None,
            _recording: None,
        }
    }

    pub fn connected() -> Option<Self> {
        let recording = RecordingWatch::open();
        platform::Gamepads::open().map(|gamepads| Self {
            _recording: Some(recording),
            ..Self::new(gamepads)
        })
    }

    pub fn controller(&self) -> Option<GamepadKind> {
        self.controller
    }

    pub fn read(&mut self) -> Option<GamepadSnapshot> {
        let shown = self.shown_now();
        self.controller = shown.map(|snapshot| snapshot.kind).or(self.controller);
        shown
    }

    fn shown_now(&mut self) -> Option<GamepadSnapshot> {
        let gamepads = self.source.poll();
        self.controller = gamepads.first().map(|(_, snapshot)| snapshot.kind);
        let of = |id: GamepadId| {
            gamepads
                .iter()
                .find(|(connected, _)| *connected == id)
                .map(|(_, snapshot)| *snapshot)
        };
        if let Some(snapshot) = self.shown.and_then(of) {
            return Some(snapshot);
        }
        let (id, snapshot) = gamepads
            .iter()
            .take(MAX_GAMEPADS)
            .find(|(_, snapshot)| !snapshot.is_idle())?;
        self.shown = Some(*id);
        Some(*snapshot)
    }
}

struct Presence<Open> {
    open: Open,
    watch: Option<GamepadWatch>,
    reported: Option<Option<GamepadKind>>,
}

impl<Open: FnMut() -> Option<GamepadWatch>> Presence<Open> {
    fn new(open: Open) -> Self {
        Self {
            open,
            watch: None,
            reported: None,
        }
    }

    fn look(&mut self, recording: bool) -> Option<Option<GamepadKind>> {
        if recording {
            self.watch = None;
            return None;
        }
        if self.watch.is_none() {
            self.watch = (self.open)();
        }
        let watch = self.watch.as_mut()?;
        watch.read();
        let connected = watch.controller();
        if self.reported == Some(connected) {
            return None;
        }
        self.reported = Some(connected);
        Some(connected)
    }
}

pub struct ConnectedGamepad {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ConnectedGamepad {
    const INTERVAL: Duration = Duration::from_millis(250);

    pub fn watch(report: impl FnMut(Option<GamepadKind>) + Send + 'static) -> Self {
        Self::watching(
            || platform::Gamepads::open().map(GamepadWatch::new),
            Self::INTERVAL,
            report,
        )
    }

    fn watching(
        open: impl FnMut() -> Option<GamepadWatch> + Send + 'static,
        interval: Duration,
        mut report: impl FnMut(Option<GamepadKind>) + Send + 'static,
    ) -> Self {
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = thread::Builder::new()
            .name("connected-gamepad".to_string())
            .spawn(move || {
                let mut presence = Presence::new(open);
                loop {
                    let recording = RECORDING_WATCHES.load(Ordering::SeqCst) > 0;
                    if let Some(connected) = presence.look(recording) {
                        report(connected);
                    }
                    if stopped.recv_timeout(interval) != Err(mpsc::RecvTimeoutError::Timeout) {
                        return;
                    }
                }
            })
            .ok();
        Self {
            stop: Some(stop),
            thread,
        }
    }
}

impl Drop for ConnectedGamepad {
    fn drop(&mut self) {
        self.stop = None;
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

mod platform {
    use super::{GamepadId, GamepadPoll, MAX_GAMEPADS};
    use frameever_project::input_display::{
        GamepadAxes, GamepadKind, GamepadSnapshot, gamepad_buttons,
    };
    use sdl3_sys::everything::*;

    pub struct Gamepads {
        open: Vec<(SDL_JoystickID, *mut SDL_Gamepad)>,
    }

    pub fn kind_of(reported: SDL_GamepadType) -> GamepadKind {
        match reported {
            SDL_GamepadType::PS3 | SDL_GamepadType::PS4 | SDL_GamepadType::PS5 => {
                GamepadKind::PlayStation
            }
            SDL_GamepadType::NINTENDO_SWITCH_PRO
            | SDL_GamepadType::NINTENDO_SWITCH_JOYCON_LEFT
            | SDL_GamepadType::NINTENDO_SWITCH_JOYCON_RIGHT
            | SDL_GamepadType::NINTENDO_SWITCH_JOYCON_PAIR => GamepadKind::Switch,
            _ => GamepadKind::Xbox,
        }
    }

    impl Gamepads {
        pub fn open() -> Option<Self> {
            let started = unsafe {
                SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
                SDL_InitSubSystem(SDL_INIT_GAMEPAD)
            };
            if !started {
                tracing::warn!("Gamepads are not read: the gamepad reader did not start");
                return None;
            }
            unsafe {
                SDL_SetGamepadEventsEnabled(false);
                SDL_SetJoystickEventsEnabled(false);
            }
            Some(Self { open: Vec::new() })
        }

        fn connected(&self) -> Vec<SDL_JoystickID> {
            let mut count = 0;
            let ids = unsafe { SDL_GetGamepads(&mut count) };
            if ids.is_null() {
                return Vec::new();
            }
            let connected =
                unsafe { std::slice::from_raw_parts(ids, usize::try_from(count).unwrap_or(0)) }
                    .iter()
                    .copied()
                    .take(MAX_GAMEPADS)
                    .collect();
            unsafe { SDL_free(ids.cast()) };
            connected
        }

        fn follow(&mut self, connected: &[SDL_JoystickID]) {
            self.open.retain(|(id, gamepad)| {
                let stays = connected.contains(id);
                if !stays {
                    unsafe { SDL_CloseGamepad(*gamepad) };
                }
                stays
            });
            for id in connected {
                if self.open.iter().any(|(open, _)| open == id) {
                    continue;
                }
                let gamepad = unsafe { SDL_OpenGamepad(*id) };
                if !gamepad.is_null() {
                    self.open.push((*id, gamepad));
                }
            }
        }
    }

    fn snapshot(gamepad: *mut SDL_Gamepad) -> GamepadSnapshot {
        let axis = |axis: SDL_GamepadAxis| unsafe { SDL_GetGamepadAxis(gamepad, axis) };
        let held = (0..gamepad_buttons::COUNT)
            .filter(|button| unsafe {
                SDL_GetGamepadButton(gamepad, SDL_GamepadButton(i32::from(*button)))
            })
            .fold(GamepadSnapshot::default(), GamepadSnapshot::with_button);
        GamepadSnapshot {
            kind: kind_of(unsafe { SDL_GetGamepadType(gamepad) }),
            axes: GamepadAxes::from_raw(
                [axis(SDL_GamepadAxis::LEFTX), axis(SDL_GamepadAxis::LEFTY)],
                [axis(SDL_GamepadAxis::RIGHTX), axis(SDL_GamepadAxis::RIGHTY)],
                [
                    axis(SDL_GamepadAxis::LEFT_TRIGGER),
                    axis(SDL_GamepadAxis::RIGHT_TRIGGER),
                ],
            ),
            ..held
        }
    }

    impl GamepadPoll for Gamepads {
        fn poll(&mut self) -> Vec<(GamepadId, GamepadSnapshot)> {
            unsafe { SDL_UpdateGamepads() };
            let connected = self.connected();
            self.follow(&connected);
            self.open
                .iter()
                .map(|(id, gamepad)| (id.0, snapshot(*gamepad)))
                .collect()
        }
    }

    impl Drop for Gamepads {
        fn drop(&mut self) {
            for (_, gamepad) in self.open.drain(..) {
                unsafe { SDL_CloseGamepad(gamepad) };
            }
            unsafe { SDL_QuitSubSystem(SDL_INIT_GAMEPAD) };
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    #[derive(Clone, Default)]
    pub(crate) struct Scripted(Arc<Mutex<VecDeque<Vec<(GamepadId, GamepadSnapshot)>>>>);

    impl Scripted {
        pub(crate) fn then(self, gamepads: &[(GamepadId, GamepadSnapshot)]) -> Self {
            self.0.lock().unwrap().push_back(gamepads.to_vec());
            self
        }
    }

    impl GamepadPoll for Scripted {
        fn poll(&mut self) -> Vec<(GamepadId, GamepadSnapshot)> {
            let mut script = self.0.lock().unwrap();
            if script.len() > 1 {
                script.pop_front().unwrap_or_default()
            } else {
                script.front().cloned().unwrap_or_default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::Scripted, *};
    use frameever_project::input_display::{GamepadAxes, GamepadKind, gamepad_buttons};

    fn idle(kind: GamepadKind) -> GamepadSnapshot {
        GamepadSnapshot {
            kind,
            ..Default::default()
        }
    }

    fn pressing(kind: GamepadKind, button: u16) -> GamepadSnapshot {
        idle(kind).with_button(button)
    }

    #[test]
    fn nothing_is_shown_until_a_gamepad_produces_input() {
        let mut watch = GamepadWatch::new(
            Scripted::default()
                .then(&[])
                .then(&[(7, idle(GamepadKind::Xbox))])
                .then(&[(7, pressing(GamepadKind::Xbox, gamepad_buttons::SOUTH))])
                .then(&[(7, idle(GamepadKind::Xbox))]),
        );

        assert_eq!(watch.read(), None);
        assert_eq!(watch.read(), None);
        assert_eq!(
            watch.read(),
            Some(pressing(GamepadKind::Xbox, gamepad_buttons::SOUTH))
        );
        assert_eq!(watch.read(), Some(idle(GamepadKind::Xbox)));
    }

    #[test]
    fn the_first_gamepad_that_produced_input_keeps_being_the_one_shown() {
        let pushed = GamepadSnapshot {
            axes: GamepadAxes {
                left_stick: [90, 0],
                ..Default::default()
            },
            ..idle(GamepadKind::PlayStation)
        };
        let mut watch = GamepadWatch::new(
            Scripted::default()
                .then(&[(1, idle(GamepadKind::Xbox)), (2, pushed)])
                .then(&[
                    (1, pressing(GamepadKind::Xbox, gamepad_buttons::NORTH)),
                    (2, idle(GamepadKind::PlayStation)),
                ]),
        );

        assert_eq!(watch.read(), Some(pushed));
        assert_eq!(watch.read(), Some(idle(GamepadKind::PlayStation)));
    }

    #[test]
    fn when_the_shown_gamepad_is_unplugged_the_next_one_to_produce_input_takes_over() {
        let switch = pressing(GamepadKind::Switch, gamepad_buttons::EAST);
        let mut watch = GamepadWatch::new(
            Scripted::default()
                .then(&[(1, pressing(GamepadKind::Xbox, gamepad_buttons::SOUTH))])
                .then(&[(2, idle(GamepadKind::Switch))])
                .then(&[(2, switch)]),
        );

        assert!(watch.read().is_some());
        assert_eq!(watch.read(), None);
        assert_eq!(watch.read(), Some(switch));
    }

    #[test]
    fn the_controller_is_known_before_it_produces_input_and_follows_the_one_shown() {
        let mut watch = GamepadWatch::new(
            Scripted::default()
                .then(&[])
                .then(&[
                    (1, idle(GamepadKind::Switch)),
                    (2, idle(GamepadKind::PlayStation)),
                ])
                .then(&[
                    (1, idle(GamepadKind::Switch)),
                    (2, pressing(GamepadKind::PlayStation, gamepad_buttons::WEST)),
                ]),
        );

        assert_eq!(watch.controller(), None);
        assert_eq!(watch.read(), None);
        assert_eq!(watch.controller(), None);
        assert_eq!(watch.read(), None);
        assert_eq!(watch.controller(), Some(GamepadKind::Switch));
        assert!(watch.read().is_some());
        assert_eq!(watch.controller(), Some(GamepadKind::PlayStation));
    }

    #[test]
    fn the_connected_gamepad_is_reported_when_it_changes_and_not_read_during_a_recording() {
        let opened = std::sync::Arc::new(AtomicUsize::new(0));
        let script = Scripted::default()
            .then(&[])
            .then(&[])
            .then(&[(1, idle(GamepadKind::Xbox))])
            .then(&[(1, idle(GamepadKind::Xbox))])
            .then(&[
                (1, idle(GamepadKind::Xbox)),
                (
                    2,
                    pressing(GamepadKind::PlayStation, gamepad_buttons::SOUTH),
                ),
            ])
            .then(&[(1, idle(GamepadKind::Xbox))])
            .then(&[])
            .then(&[(3, idle(GamepadKind::Switch))]);
        let mut presence = Presence::new({
            let opened = opened.clone();
            move || {
                opened.fetch_add(1, Ordering::SeqCst);
                Some(GamepadWatch::new(script.clone()))
            }
        });

        assert_eq!(presence.look(false), Some(None));
        assert_eq!(presence.look(false), None);
        assert_eq!(presence.look(false), Some(Some(GamepadKind::Xbox)));
        assert_eq!(presence.look(false), None);
        assert_eq!(presence.look(false), Some(Some(GamepadKind::PlayStation)));
        assert_eq!(presence.look(false), Some(Some(GamepadKind::Xbox)));
        assert_eq!(presence.look(false), Some(None));
        assert_eq!(opened.load(Ordering::SeqCst), 1);

        assert_eq!(presence.look(true), None);
        assert!(presence.watch.is_none());
        assert_eq!(presence.look(true), None);
        assert_eq!(opened.load(Ordering::SeqCst), 1);
        assert_eq!(presence.look(false), Some(Some(GamepadKind::Switch)));
        assert_eq!(opened.load(Ordering::SeqCst), 2);

        let mut unavailable = Presence::new(|| None);
        assert_eq!(unavailable.look(false), None);
    }

    #[test]
    fn the_connected_gamepad_is_watched_on_its_own_thread_until_it_is_dropped() {
        let (seen, reports) = mpsc::channel();
        let script = Scripted::default()
            .then(&[(1, idle(GamepadKind::Switch))])
            .then(&[]);
        let watching = ConnectedGamepad::watching(
            move || Some(GamepadWatch::new(script.clone())),
            Duration::from_millis(1),
            move |connected| {
                seen.send(connected).ok();
            },
        );

        let wait = Duration::from_secs(5);
        assert_eq!(reports.recv_timeout(wait), Ok(Some(GamepadKind::Switch)));
        assert_eq!(reports.recv_timeout(wait), Ok(None));
        drop(watching);
        assert_eq!(
            reports.recv_timeout(wait),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn only_the_first_four_gamepads_are_considered() {
        let mut gamepads: Vec<_> = (0..4).map(|id| (id, idle(GamepadKind::Xbox))).collect();
        gamepads.push((9, pressing(GamepadKind::Xbox, gamepad_buttons::SOUTH)));
        let mut watch = GamepadWatch::new(Scripted::default().then(&gamepads));

        assert_eq!(watch.read(), None);
    }

    #[test]
    fn controller_families_are_told_apart_by_what_the_reader_reports() {
        use sdl3_sys::everything::SDL_GamepadType;

        let kind = super::platform::kind_of;
        assert_eq!(kind(SDL_GamepadType::XBOXONE), GamepadKind::Xbox);
        assert_eq!(kind(SDL_GamepadType::XBOX360), GamepadKind::Xbox);
        assert_eq!(kind(SDL_GamepadType::STANDARD), GamepadKind::Xbox);
        assert_eq!(kind(SDL_GamepadType::PS4), GamepadKind::PlayStation);
        assert_eq!(kind(SDL_GamepadType::PS5), GamepadKind::PlayStation);
        assert_eq!(
            kind(SDL_GamepadType::NINTENDO_SWITCH_PRO),
            GamepadKind::Switch
        );
        assert_eq!(
            kind(SDL_GamepadType::NINTENDO_SWITCH_JOYCON_PAIR),
            GamepadKind::Switch
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_device_reader_starts_polls_and_stops_on_a_background_thread_twice() {
        for _ in 0..2 {
            std::thread::spawn(|| {
                let mut watch = GamepadWatch::connected().expect("the gamepad reader starts");
                for _ in 0..5 {
                    let read = watch.read();
                    assert!(read.is_none_or(|snapshot| snapshot.pressed().count() <= 15));
                    std::thread::sleep(std::time::Duration::from_millis(8));
                }
            })
            .join()
            .unwrap();
        }
    }
}
