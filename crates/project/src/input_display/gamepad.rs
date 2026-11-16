use serde::{Deserialize, Serialize};

pub mod gamepad_buttons {
    pub const SOUTH: u16 = 0;
    pub const EAST: u16 = 1;
    pub const WEST: u16 = 2;
    pub const NORTH: u16 = 3;
    pub const BACK: u16 = 4;
    pub const GUIDE: u16 = 5;
    pub const START: u16 = 6;
    pub const LEFT_STICK: u16 = 7;
    pub const RIGHT_STICK: u16 = 8;
    pub const LEFT_SHOULDER: u16 = 9;
    pub const RIGHT_SHOULDER: u16 = 10;
    pub const DPAD_UP: u16 = 11;
    pub const DPAD_DOWN: u16 = 12;
    pub const DPAD_LEFT: u16 = 13;
    pub const DPAD_RIGHT: u16 = 14;
    pub const COUNT: u16 = 15;
}

pub const STICK_FULL: i8 = 127;
pub const TRIGGER_FULL: u8 = 255;

const RAW_FULL: f64 = 32767.0;
const STICK_DEAD_ZONE: f64 = 0.12;
const TRIGGER_DEAD_ZONE: f64 = 0.03;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GamepadKind {
    #[default]
    Xbox,
    PlayStation,
    Switch,
}

impl GamepadKind {
    pub const ALL: [Self; 3] = [Self::Xbox, Self::PlayStation, Self::Switch];
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamepadAxes {
    pub left_stick: [i8; 2],
    pub right_stick: [i8; 2],
    pub left_trigger: u8,
    pub right_trigger: u8,
}

impl GamepadAxes {
    pub fn from_raw(left_stick: [i16; 2], right_stick: [i16; 2], triggers: [i16; 2]) -> Self {
        Self {
            left_stick: stick_from_raw(left_stick),
            right_stick: stick_from_raw(right_stick),
            left_trigger: trigger_from_raw(triggers[0]),
            right_trigger: trigger_from_raw(triggers[1]),
        }
    }
}

fn stick_from_raw([x, y]: [i16; 2]) -> [i8; 2] {
    let (x, y) = (f64::from(x) / RAW_FULL, f64::from(y) / RAW_FULL);
    let pushed = x.hypot(y);
    if pushed < STICK_DEAD_ZONE {
        return [0, 0];
    }
    let live = ((pushed - STICK_DEAD_ZONE) / (1.0 - STICK_DEAD_ZONE)).min(1.0) / pushed;
    let step = |value: f64| {
        (value * live * f64::from(STICK_FULL))
            .round()
            .clamp(-f64::from(STICK_FULL), f64::from(STICK_FULL)) as i8
    };
    [step(x), step(y)]
}

fn trigger_from_raw(value: i16) -> u8 {
    let pulled = f64::from(value.max(0)) / RAW_FULL;
    if pulled < TRIGGER_DEAD_ZONE {
        0
    } else {
        (pulled * f64::from(TRIGGER_FULL)).round() as u8
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamepadSnapshot {
    pub kind: GamepadKind,
    pub buttons: u16,
    pub axes: GamepadAxes,
}

impl GamepadSnapshot {
    pub fn with_button(mut self, button: u16) -> Self {
        if button < gamepad_buttons::COUNT {
            self.buttons |= 1 << button;
        }
        self
    }

    pub fn is_pressed(&self, button: u16) -> bool {
        button < gamepad_buttons::COUNT && self.buttons & (1 << button) != 0
    }

    pub fn pressed(&self) -> impl Iterator<Item = u16> + use<> {
        let buttons = self.buttons;
        (0..gamepad_buttons::COUNT).filter(move |button| buttons & (1 << button) != 0)
    }

    pub fn is_idle(&self) -> bool {
        self.buttons == 0 && self.axes == GamepadAxes::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resting_stick_reads_as_centred_and_a_pushed_one_keeps_its_direction() {
        assert_eq!(stick_from_raw([900, -1200]), [0, 0]);
        assert_eq!(stick_from_raw([32767, 0]), [127, 0]);
        assert_eq!(stick_from_raw([0, -32768]), [0, -127]);
        assert_eq!(stick_from_raw([-32768, 32767]), [-90, 90]);
        let [x, y] = stick_from_raw([16384, 0]);
        assert_eq!((x, y), (55, 0));
    }

    #[test]
    fn a_trigger_reads_from_empty_to_full() {
        let axes = GamepadAxes::from_raw([0, 0], [0, 0], [300, 32767]);

        assert_eq!((axes.left_trigger, axes.right_trigger), (0, 255));
        assert_eq!(trigger_from_raw(16384), 128);
        assert_eq!(trigger_from_raw(-5), 0);
    }

    #[test]
    fn a_snapshot_names_its_pressed_buttons() {
        let snapshot = GamepadSnapshot::default()
            .with_button(gamepad_buttons::SOUTH)
            .with_button(gamepad_buttons::DPAD_RIGHT)
            .with_button(99);

        assert_eq!(snapshot.pressed().collect::<Vec<_>>(), vec![0, 14]);
        assert!(snapshot.is_pressed(gamepad_buttons::SOUTH));
        assert!(!snapshot.is_pressed(gamepad_buttons::NORTH));
        assert!(!snapshot.is_idle());
        assert!(GamepadSnapshot::default().is_idle());
    }
}
