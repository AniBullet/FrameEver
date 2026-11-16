use std::path::Path;

use serde::{Deserialize, Serialize};

use super::gamepad::GamepadSnapshot;

pub const INPUT_EVENTS_FILE_NAME: &str = "input.bin";
const INPUT_EVENTS_MAGIC: &[u8; 6] = b"FEIR01";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum InputEventKind {
    MouseButton { button: u8, down: bool },
    Wheel { up: bool },
    TargetInFront(bool),
    Gamepad(GamepadSnapshot),
    HeldKey(u16),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct InputEvent {
    pub time_ms: f64,
    pub kind: InputEventKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct InputEvents {
    pub events: Vec<InputEvent>,
}

impl InputEvents {
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("Failed to open input events file: {e}"))?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let payload = bytes
            .strip_prefix(INPUT_EVENTS_MAGIC)
            .ok_or_else(|| "Not an input events file".to_string())?;
        bincode::serde::decode_from_slice::<Self, _>(payload, bincode::config::standard())
            .map(|(events, _)| events)
            .map_err(|e| format!("Failed to parse input events: {e}"))
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = INPUT_EVENTS_MAGIC.to_vec();
        let payload = bincode::serde::encode_to_vec(self, bincode::config::standard())
            .map_err(|e| format!("Failed to serialize input events: {e}"))?;
        bytes.extend(payload);
        Ok(bytes)
    }

    pub fn write_to_file(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_bytes()?)
            .map_err(|e| format!("Failed to write input events file: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_display::{GamepadAxes, GamepadKind};

    fn every_kind() -> InputEvents {
        InputEvents {
            events: vec![
                InputEvent {
                    time_ms: 0.0,
                    kind: InputEventKind::TargetInFront(true),
                },
                InputEvent {
                    time_ms: 16.0,
                    kind: InputEventKind::MouseButton {
                        button: 4,
                        down: true,
                    },
                },
                InputEvent {
                    time_ms: 32.5,
                    kind: InputEventKind::Wheel { up: false },
                },
                InputEvent {
                    time_ms: 48.0,
                    kind: InputEventKind::Gamepad(GamepadSnapshot {
                        kind: GamepadKind::PlayStation,
                        buttons: 0b100_0000_0000_0101,
                        axes: GamepadAxes {
                            left_stick: [-127, 64],
                            right_stick: [1, -2],
                            left_trigger: 255,
                            right_trigger: 7,
                        },
                    }),
                },
                InputEvent {
                    time_ms: 0.0,
                    kind: InputEventKind::HeldKey(0xA010),
                },
            ],
        }
    }

    #[test]
    fn every_event_kind_round_trips_through_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(INPUT_EVENTS_FILE_NAME);

        every_kind().write_to_file(&path).unwrap();

        assert_eq!(InputEvents::load_from_file(&path).unwrap(), every_kind());
    }

    #[test]
    fn the_file_keeps_its_byte_layout() {
        let events = InputEvents {
            events: vec![InputEvent {
                time_ms: 1.0,
                kind: InputEventKind::Wheel { up: true },
            }],
        };

        assert_eq!(
            events.to_bytes().unwrap(),
            [b"FEIR01".as_slice(), &[1], &1.0f64.to_le_bytes(), &[1, 1]].concat()
        );
    }

    #[test]
    fn a_keyboard_file_or_a_cut_off_file_is_refused() {
        assert!(InputEvents::from_bytes(b"CPKB01\x00").is_err());
        let mut bytes = every_kind().to_bytes().unwrap();
        bytes.truncate(bytes.len() - 3);
        assert!(InputEvents::from_bytes(&bytes).is_err());
    }
}
