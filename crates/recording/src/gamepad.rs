use frameever_project::input_display::{GamepadKind, GamepadSnapshot};

pub const MAX_GAMEPADS: usize = 4;

pub type GamepadId = u32;

pub trait GamepadPoll {
    fn poll(&mut self) -> Vec<(GamepadId, GamepadSnapshot)>;
}

pub struct GamepadWatch {
    source: Box<dyn GamepadPoll>,
    shown: Option<GamepadId>,
    controller: Option<GamepadKind>,
}

impl GamepadWatch {
    pub fn new(source: impl GamepadPoll + 'static) -> Self {
        Self {
            source: Box::new(source),
            shown: None,
            controller: None,
        }
    }

    pub fn connected() -> Option<Self> {
        platform::Gamepads::open().map(Self::new)
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

#[cfg(any(windows, target_os = "macos"))]
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

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::{GamepadId, GamepadPoll};
    use frameever_project::input_display::GamepadSnapshot;

    pub struct Gamepads;

    impl Gamepads {
        pub fn open() -> Option<Self> {
            None
        }
    }

    impl GamepadPoll for Gamepads {
        fn poll(&mut self) -> Vec<(GamepadId, GamepadSnapshot)> {
            Vec::new()
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
    fn only_the_first_four_gamepads_are_considered() {
        let mut gamepads: Vec<_> = (0..4).map(|id| (id, idle(GamepadKind::Xbox))).collect();
        gamepads.push((9, pressing(GamepadKind::Xbox, gamepad_buttons::SOUTH)));
        let mut watch = GamepadWatch::new(Scripted::default().then(&gamepads));

        assert_eq!(watch.read(), None);
    }

    #[cfg(any(windows, target_os = "macos"))]
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
