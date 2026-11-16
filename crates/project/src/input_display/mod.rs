mod builtin;
mod clear;
mod events;
mod gamepad;
mod keys;
mod placement;
mod preset;
mod record;

pub use builtin::{BuiltinPresetId, KeyLayout, LayoutKey, PresetArt, preset_for_controller};
pub use clear::{clear_input_record, has_input_record, input_record_generation};
pub use events::{INPUT_EVENTS_FILE_NAME, InputEvent, InputEventKind, InputEvents};
pub use gamepad::{
    GamepadAxes, GamepadKind, GamepadSnapshot, STICK_FULL, TRIGGER_FULL, gamepad_buttons,
};
pub use keys::{key_code, key_codes};
pub use placement::{
    DisplayRect, INPUT_DISPLAY_SNAP_DISTANCE, INPUT_DISPLAY_SNAP_STOPS, display_rect,
    position_for_origin, size_for_width, snap_position,
};
pub use preset::{
    Element, ElementKind, FillDirection, MAX_PRESET_ATLAS_BYTES, MAX_PRESET_ATLAS_DIMENSION,
    MAX_PRESET_ELEMENTS, MAX_PRESET_JSON_BYTES, Preset, PresetRefusal, STATE_GAP, Sprite,
    SpriteRect,
};
pub use record::{
    DisplayState, Input, InputChange, InputRecord, WHEEL_FLASH_MS, WheelStep, preset_for_recording,
    take_at,
};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::XY;

pub const INPUT_DISPLAY_MARGIN: f64 = 0.03;
pub const INPUT_DISPLAY_DEFAULT_SIZE: f64 = 60.0;
pub const INPUT_DISPLAY_SIZE_RANGE: (f64, f64) = (30.0, 120.0);
pub const INPUT_DISPLAY_OPACITY_RANGE: (f64, f64) = (20.0, 100.0);
pub const INPUT_DISPLAY_IMPORTED_FRAME_SHARE: f64 = 0.3;

#[derive(Type, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum InputDisplayStyle {
    #[default]
    Keycaps,
    Outline,
    Ghost,
}

impl InputDisplayStyle {
    pub const ALL: [Self; 3] = [Self::Keycaps, Self::Outline, Self::Ghost];
}

#[derive(Type, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct InputDisplay {
    pub enabled: bool,
    pub preset: String,
    pub style: InputDisplayStyle,
    pub position: XY<f64>,
    pub size: f64,
    pub opacity: f64,
}

impl Default for InputDisplay {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: BuiltinPresetId::WasdMouse.id().to_string(),
            style: InputDisplayStyle::default(),
            position: XY::new(0.0, 1.0),
            size: INPUT_DISPLAY_DEFAULT_SIZE,
            opacity: INPUT_DISPLAY_OPACITY_RANGE.1,
        }
    }
}

impl InputDisplay {
    pub fn opacity_fraction(&self) -> f64 {
        if self.opacity.is_finite() {
            (self.opacity / 100.0).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

impl crate::ProjectConfiguration {
    pub fn shown_input_display(&self) -> Option<&InputDisplay> {
        self.input_display.as_ref().filter(|block| block.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectConfiguration;

    #[test]
    fn a_project_without_the_block_writes_none_and_shows_nothing() {
        let project = ProjectConfiguration::default();
        let json = serde_json::to_value(&project).unwrap();

        assert!(json.get("inputDisplay").is_none());
        assert!(project.shown_input_display().is_none());
        assert!(
            serde_json::from_str::<ProjectConfiguration>("{}")
                .unwrap()
                .input_display
                .is_none()
        );
    }

    #[test]
    fn the_block_round_trips_through_the_project_file() {
        let block = InputDisplay {
            enabled: true,
            preset: BuiltinPresetId::FullKeyboard.id().to_string(),
            style: InputDisplayStyle::Ghost,
            position: XY::new(0.5, 0.25),
            size: 90.0,
            opacity: 40.0,
        };
        let project = ProjectConfiguration {
            input_display: Some(block.clone()),
            ..Default::default()
        };

        let json = serde_json::to_value(&project).unwrap();
        let read: ProjectConfiguration = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(
            json["inputDisplay"],
            serde_json::json!({
                "enabled": true,
                "preset": "full-keyboard",
                "style": "ghost",
                "position": { "x": 0.5, "y": 0.25 },
                "size": 90.0,
                "opacity": 40.0
            })
        );
        assert_eq!(read.shown_input_display(), Some(&block));
    }

    #[test]
    fn a_block_that_is_switched_off_keeps_its_setup_and_shows_nothing() {
        let project: ProjectConfiguration = serde_json::from_value(serde_json::json!({
            "inputDisplay": { "preset": "full-keyboard" }
        }))
        .unwrap();

        assert!(project.shown_input_display().is_none());
        assert_eq!(
            project.input_display,
            Some(InputDisplay {
                preset: "full-keyboard".to_string(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn applying_an_editor_preset_keeps_the_projects_block() {
        let block = InputDisplay {
            enabled: true,
            ..Default::default()
        };
        let project = ProjectConfiguration {
            input_display: Some(block.clone()),
            ..Default::default()
        };
        let preset = ProjectConfiguration {
            input_display: Some(InputDisplay {
                preset: "someone-elses".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(
            project.with_preset(preset).input_display,
            Some(block.clone())
        );
        assert_eq!(
            ProjectConfiguration::default()
                .with_preset(project)
                .input_display,
            None
        );
    }
}
