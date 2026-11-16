use serde::{Deserialize, Serialize};

use super::record::{DisplayState, Input, WheelStep};

pub const STATE_GAP: u32 = 3;
pub const MAX_PRESET_ELEMENTS: usize = 512;
pub const MAX_PRESET_JSON_BYTES: usize = 1024 * 1024;
pub const MAX_PRESET_ATLAS_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PRESET_ATLAS_DIMENSION: u32 = 4096;

const MIDDLE_BUTTON: u8 = 3;

const TYPE_TEXTURE: i64 = 0;
const TYPE_KEYBOARD_KEY: i64 = 1;
const TYPE_GAMEPAD_BUTTON: i64 = 2;
const TYPE_MOUSE_BUTTON: i64 = 3;
const TYPE_MOUSE_WHEEL: i64 = 4;
const TYPE_ANALOG_STICK: i64 = 5;
const TYPE_TRIGGER: i64 = 6;
const TYPE_GAMEPAD_ID: i64 = 7;
const TYPE_DPAD: i64 = 8;
const TYPE_MOUSE_MOVEMENT: i64 = 9;

#[derive(Serialize, Deserialize)]
struct PresetFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay_height: Option<u32>,
    elements: Vec<ElementFile>,
}

#[derive(Serialize, Deserialize)]
struct ElementFile {
    #[serde(rename = "type")]
    kind: i64,
    #[serde(default)]
    id: String,
    pos: [u32; 2],
    mapping: [u32; 4],
    #[serde(default)]
    z_level: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteRect {
    pub u: u32,
    pub v: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    Texture,
    KeyboardKey { code: u16 },
    GamepadButton { code: u16 },
    MouseButton { button: u8 },
    MouseWheel,
    AnalogStick,
    Trigger,
    GamepadId,
    DPad,
    MouseMovement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub id: String,
    pub kind: ElementKind,
    pub x: u32,
    pub y: u32,
    pub mapping: SpriteRect,
    pub z_level: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub width: u32,
    pub height: u32,
    pub elements: Vec<Element>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub source: SpriteRect,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresetRefusal {
    JsonTooLarge { bytes: usize },
    JsonUnreadable { detail: String },
    NoElements,
    TooManyElements { count: usize },
    UnknownElementType { element: String, type_id: i64 },
    MissingCode { element: String },
    CodeOutOfRange { element: String, code: u32 },
    EmptyMapping { element: String },
    MappingOutsideAtlas { element: String },
    MissingAtlas,
    AtlasFileTooLarge { bytes: usize },
    AtlasTooLarge { width: u32, height: u32 },
    AtlasUnreadable { detail: String },
}

impl std::fmt::Display for PresetRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonTooLarge { bytes } => write!(
                f,
                "the preset file is {bytes} bytes, more than the {MAX_PRESET_JSON_BYTES} allowed"
            ),
            Self::JsonUnreadable { detail } => {
                write!(f, "the preset file is not valid preset JSON: {detail}")
            }
            Self::NoElements => write!(f, "the preset has no elements"),
            Self::TooManyElements { count } => write!(
                f,
                "the preset has {count} elements, more than the {MAX_PRESET_ELEMENTS} allowed"
            ),
            Self::UnknownElementType { element, type_id } => {
                write!(f, "element \"{element}\" has the unknown type {type_id}")
            }
            Self::MissingCode { element } => {
                write!(f, "button \"{element}\" has no key or button code")
            }
            Self::CodeOutOfRange { element, code } => {
                write!(f, "button \"{element}\" has the code {code}, which is out of range")
            }
            Self::EmptyMapping { element } => {
                write!(f, "element \"{element}\" maps an empty part of the image")
            }
            Self::MappingOutsideAtlas { element } => {
                write!(f, "element \"{element}\" maps a part outside the image")
            }
            Self::MissingAtlas => write!(f, "the preset has no image file"),
            Self::AtlasFileTooLarge { bytes } => write!(
                f,
                "the preset image is {bytes} bytes, more than the {MAX_PRESET_ATLAS_BYTES} allowed"
            ),
            Self::AtlasTooLarge { width, height } => write!(
                f,
                "the preset image is {width} x {height} pixels, more than {MAX_PRESET_ATLAS_DIMENSION} on a side"
            ),
            Self::AtlasUnreadable { detail } => {
                write!(f, "the preset image is not a readable PNG: {detail}")
            }
        }
    }
}

impl std::error::Error for PresetRefusal {}

impl ElementKind {
    fn from_file(element: &ElementFile) -> Result<Self, PresetRefusal> {
        let name = || element.id.clone();
        let code = || element.code.ok_or(PresetRefusal::MissingCode { element: name() });
        let wide_code = || {
            let code = code()?;
            u16::try_from(code).map_err(|_| PresetRefusal::CodeOutOfRange {
                element: name(),
                code,
            })
        };
        Ok(match element.kind {
            TYPE_TEXTURE => Self::Texture,
            TYPE_KEYBOARD_KEY => Self::KeyboardKey { code: wide_code()? },
            TYPE_GAMEPAD_BUTTON => Self::GamepadButton { code: wide_code()? },
            TYPE_MOUSE_BUTTON => {
                let code = code()?;
                Self::MouseButton {
                    button: u8::try_from(code).map_err(|_| PresetRefusal::CodeOutOfRange {
                        element: name(),
                        code,
                    })?,
                }
            }
            TYPE_MOUSE_WHEEL => Self::MouseWheel,
            TYPE_ANALOG_STICK => Self::AnalogStick,
            TYPE_TRIGGER => Self::Trigger,
            TYPE_GAMEPAD_ID => Self::GamepadId,
            TYPE_DPAD => Self::DPad,
            TYPE_MOUSE_MOVEMENT => Self::MouseMovement,
            type_id => {
                return Err(PresetRefusal::UnknownElementType {
                    element: name(),
                    type_id,
                });
            }
        })
    }

    fn type_id(&self) -> i64 {
        match self {
            Self::Texture => TYPE_TEXTURE,
            Self::KeyboardKey { .. } => TYPE_KEYBOARD_KEY,
            Self::GamepadButton { .. } => TYPE_GAMEPAD_BUTTON,
            Self::MouseButton { .. } => TYPE_MOUSE_BUTTON,
            Self::MouseWheel => TYPE_MOUSE_WHEEL,
            Self::AnalogStick => TYPE_ANALOG_STICK,
            Self::Trigger => TYPE_TRIGGER,
            Self::GamepadId => TYPE_GAMEPAD_ID,
            Self::DPad => TYPE_DPAD,
            Self::MouseMovement => TYPE_MOUSE_MOVEMENT,
        }
    }

    fn code(&self) -> Option<u32> {
        match self {
            Self::KeyboardKey { code } | Self::GamepadButton { code } => Some(u32::from(*code)),
            Self::MouseButton { button } => Some(u32::from(*button)),
            _ => None,
        }
    }

    pub(super) fn states(&self) -> (u32, u32) {
        match self {
            Self::KeyboardKey { .. } | Self::GamepadButton { .. } | Self::MouseButton { .. } => {
                (1, 2)
            }
            Self::MouseWheel | Self::GamepadId => (4, 1),
            Self::DPad => (9, 1),
            Self::Texture | Self::AnalogStick | Self::Trigger | Self::MouseMovement => (1, 1),
        }
    }
}

impl Element {
    fn atlas_extent(&self) -> Option<(u32, u32)> {
        let (columns, rows) = self.kind.states();
        let span = |origin: u32, size: u32, count: u32| {
            size.checked_mul(count)?
                .checked_add(STATE_GAP.checked_mul(count - 1)?)?
                .checked_add(origin)
        };
        Some((
            span(self.mapping.u, self.mapping.width, columns)?,
            span(self.mapping.v, self.mapping.height, rows)?,
        ))
    }

    fn state_sprite(&self, column: u32, row: u32) -> SpriteRect {
        SpriteRect {
            u: self.mapping.u + column * (self.mapping.width + STATE_GAP),
            v: self.mapping.v + row * (self.mapping.height + STATE_GAP),
            ..self.mapping
        }
    }

    fn sprite(&self, state: &DisplayState) -> SpriteRect {
        match self.kind {
            ElementKind::KeyboardKey { code } => {
                self.state_sprite(0, u32::from(state.is_held(Input::Key(code))))
            }
            ElementKind::MouseButton { button } => {
                self.state_sprite(0, u32::from(state.is_held(Input::MouseButton(button))))
            }
            ElementKind::MouseWheel => {
                let column = if state.is_held(Input::MouseButton(MIDDLE_BUTTON)) {
                    1
                } else {
                    match state.wheel {
                        WheelStep::None => 0,
                        WheelStep::Up => 2,
                        WheelStep::Down => 3,
                    }
                };
                self.state_sprite(column, 0)
            }
            ElementKind::Texture
            | ElementKind::GamepadButton { .. }
            | ElementKind::AnalogStick
            | ElementKind::Trigger
            | ElementKind::GamepadId
            | ElementKind::DPad
            | ElementKind::MouseMovement => self.mapping,
        }
    }
}

impl Preset {
    pub fn parse(json: &str, atlas_size: (u32, u32)) -> Result<Self, PresetRefusal> {
        if json.len() > MAX_PRESET_JSON_BYTES {
            return Err(PresetRefusal::JsonTooLarge { bytes: json.len() });
        }
        let file: PresetFile =
            serde_json::from_str(json).map_err(|error| PresetRefusal::JsonUnreadable {
                detail: error.to_string(),
            })?;
        if file.elements.is_empty() {
            return Err(PresetRefusal::NoElements);
        }
        if file.elements.len() > MAX_PRESET_ELEMENTS {
            return Err(PresetRefusal::TooManyElements {
                count: file.elements.len(),
            });
        }
        let mut elements = Vec::with_capacity(file.elements.len());
        let mut extent = (0u32, 0u32);
        for (index, mut source) in file.elements.into_iter().enumerate() {
            if source.id.is_empty() {
                source.id = format!("#{}", index + 1);
            }
            let kind = ElementKind::from_file(&source)?;
            let [u, v, width, height] = source.mapping;
            let element = Element {
                id: source.id,
                kind,
                x: source.pos[0],
                y: source.pos[1],
                mapping: SpriteRect {
                    u,
                    v,
                    width,
                    height,
                },
                z_level: source.z_level,
            };
            if width == 0 || height == 0 {
                return Err(PresetRefusal::EmptyMapping {
                    element: element.id,
                });
            }
            let outside = || PresetRefusal::MappingOutsideAtlas {
                element: element.id.clone(),
            };
            let (right, bottom) = element.atlas_extent().ok_or_else(outside)?;
            if right > atlas_size.0 || bottom > atlas_size.1 {
                return Err(outside());
            }
            extent.0 = extent.0.max(element.x.saturating_add(width));
            extent.1 = extent.1.max(element.y.saturating_add(height));
            elements.push(element);
        }
        elements.sort_by_key(|element| element.z_level);
        Ok(Self {
            width: file.overlay_width.unwrap_or(0).max(extent.0),
            height: file.overlay_height.unwrap_or(0).max(extent.1),
            elements,
        })
    }

    pub fn to_json(&self) -> String {
        let file = PresetFile {
            overlay_width: Some(self.width),
            overlay_height: Some(self.height),
            elements: self
                .elements
                .iter()
                .map(|element| ElementFile {
                    kind: element.kind.type_id(),
                    id: element.id.clone(),
                    pos: [element.x, element.y],
                    mapping: [
                        element.mapping.u,
                        element.mapping.v,
                        element.mapping.width,
                        element.mapping.height,
                    ],
                    z_level: element.z_level,
                    code: element.kind.code(),
                })
                .collect(),
        };
        serde_json::to_string_pretty(&file).unwrap_or_default()
    }

    pub fn sprites(&self, state: &DisplayState) -> Vec<Sprite> {
        self.elements
            .iter()
            .map(|element| Sprite {
                source: element.sprite(state),
                x: element.x,
                y: element.y,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATLAS: (u32, u32) = (64, 64);

    fn fixture(elements: &str) -> String {
        format!(r#"{{ "overlay_width": 40, "overlay_height": 30, "elements": [{elements}] }}"#)
    }

    const KEY_W: &str =
        r#"{ "type": 1, "id": "w", "pos": [10, 4], "mapping": [1, 1, 12, 10], "z_level": 1, "code": 87 }"#;
    const BODY: &str =
        r#"{ "type": 0, "id": "body", "pos": [0, 0], "mapping": [20, 1, 30, 20], "z_level": 0 }"#;
    const LEFT_BUTTON: &str =
        r#"{ "type": 3, "id": "left", "pos": [2, 2], "mapping": [1, 30, 8, 9], "z_level": 2, "code": 1 }"#;
    const WHEEL: &str =
        r#"{ "type": 4, "id": "wheel", "pos": [12, 2], "mapping": [1, 55, 4, 6], "z_level": 2 }"#;

    fn refusal(json: &str) -> PresetRefusal {
        Preset::parse(json, ATLAS).unwrap_err()
    }

    fn held(inputs: &[Input]) -> DisplayState {
        let mut state = DisplayState::default();
        for input in inputs {
            state.apply(*input, true);
        }
        state
    }

    #[test]
    fn a_preset_lists_its_elements_in_drawing_order() {
        let preset = Preset::parse(&fixture(&[KEY_W, BODY].join(",")), ATLAS).unwrap();

        assert_eq!((preset.width, preset.height), (40, 30));
        assert_eq!(
            preset
                .elements
                .iter()
                .map(|element| (element.id.as_str(), element.kind))
                .collect::<Vec<_>>(),
            vec![
                ("body", ElementKind::Texture),
                ("w", ElementKind::KeyboardKey { code: 87 })
            ]
        );
    }

    #[test]
    fn a_preset_without_a_stored_size_is_as_large_as_its_elements() {
        let preset =
            Preset::parse(&format!(r#"{{ "elements": [{KEY_W}, {BODY}] }}"#), ATLAS).unwrap();

        assert_eq!((preset.width, preset.height), (30, 20));
    }

    #[test]
    fn a_held_key_draws_the_sprite_below_the_released_one() {
        let preset = Preset::parse(&fixture(KEY_W), ATLAS).unwrap();
        let released = SpriteRect {
            u: 1,
            v: 1,
            width: 12,
            height: 10,
        };

        assert_eq!(
            preset.sprites(&DisplayState::default()),
            vec![Sprite {
                source: released,
                x: 10,
                y: 4
            }]
        );
        assert_eq!(
            preset.sprites(&held(&[Input::Key(87)])),
            vec![Sprite {
                source: SpriteRect { v: 14, ..released },
                x: 10,
                y: 4
            }]
        );
        assert_eq!(
            preset.sprites(&held(&[Input::Key(65), Input::MouseButton(1)])),
            preset.sprites(&DisplayState::default())
        );
    }

    #[test]
    fn mouse_buttons_and_the_wheel_follow_the_mouse_state() {
        let preset = Preset::parse(&fixture(&[LEFT_BUTTON, WHEEL].join(",")), ATLAS).unwrap();
        let sources = |state: &DisplayState| {
            preset
                .sprites(state)
                .into_iter()
                .map(|sprite| (sprite.source.u, sprite.source.v))
                .collect::<Vec<_>>()
        };

        assert_eq!(sources(&DisplayState::default()), vec![(1, 30), (1, 55)]);
        assert_eq!(
            sources(&held(&[Input::MouseButton(1)])),
            vec![(1, 42), (1, 55)]
        );
        assert_eq!(
            sources(&held(&[Input::MouseButton(3)])),
            vec![(1, 30), (8, 55)]
        );
        assert_eq!(
            sources(&DisplayState::default().with_wheel(WheelStep::Up)),
            vec![(1, 30), (15, 55)]
        );
        assert_eq!(
            sources(&DisplayState::default().with_wheel(WheelStep::Down)),
            vec![(1, 30), (22, 55)]
        );
    }

    #[test]
    fn every_documented_element_kind_is_read() {
        let elements = (0..=9)
            .map(|kind| {
                format!(
                    r#"{{ "type": {kind}, "id": "e{kind}", "pos": [0, 0], "mapping": [1, 1, 2, 2], "code": 1 }}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");

        let preset = Preset::parse(&fixture(&elements), ATLAS).unwrap();

        assert_eq!(preset.elements.len(), 10);
        assert_eq!(
            preset.sprites(&DisplayState::default()).len(),
            preset.elements.len()
        );
    }

    #[test]
    fn a_preset_written_back_reads_the_same() {
        let preset =
            Preset::parse(&fixture(&[KEY_W, BODY, LEFT_BUTTON, WHEEL].join(",")), ATLAS).unwrap();

        assert_eq!(Preset::parse(&preset.to_json(), ATLAS), Ok(preset));
    }

    #[test]
    fn truncated_json_is_refused_with_the_parser_reason() {
        let json = fixture(KEY_W);
        let refused = refusal(&json[..json.len() / 2]);

        assert!(matches!(refused, PresetRefusal::JsonUnreadable { .. }));
        assert!(
            refused
                .to_string()
                .starts_with("the preset file is not valid preset JSON: ")
        );
    }

    #[test]
    fn an_unknown_element_type_is_refused_by_name() {
        let refused = refusal(&fixture(
            r#"{ "type": 42, "id": "mystery", "pos": [0, 0], "mapping": [1, 1, 2, 2] }"#,
        ));

        assert_eq!(
            refused,
            PresetRefusal::UnknownElementType {
                element: "mystery".to_string(),
                type_id: 42
            }
        );
        assert_eq!(
            refused.to_string(),
            "element \"mystery\" has the unknown type 42"
        );
    }

    #[test]
    fn a_mapping_whose_pressed_sprite_leaves_the_image_is_refused() {
        let fits = r#"{ "type": 1, "id": "w", "pos": [0, 0], "mapping": [1, 40, 12, 10], "code": 87 }"#;
        let pressed_outside =
            r#"{ "type": 1, "id": "w", "pos": [0, 0], "mapping": [1, 45, 12, 10], "code": 87 }"#;
        let wheel_outside =
            r#"{ "type": 4, "id": "wheel", "pos": [0, 0], "mapping": [40, 1, 4, 6] }"#;

        assert!(Preset::parse(&fixture(fits), ATLAS).is_ok());
        for json in [pressed_outside, wheel_outside] {
            assert!(matches!(
                refusal(&fixture(json)),
                PresetRefusal::MappingOutsideAtlas { .. }
            ));
        }
        assert_eq!(
            refusal(&fixture(
                r#"{ "type": 0, "id": "huge", "pos": [0, 0], "mapping": [4294967295, 1, 4294967295, 2] }"#
            )),
            PresetRefusal::MappingOutsideAtlas {
                element: "huge".to_string()
            }
        );
    }

    #[test]
    fn buttons_without_a_usable_code_and_empty_mappings_are_refused() {
        assert_eq!(
            refusal(&fixture(
                r#"{ "type": 1, "id": "w", "pos": [0, 0], "mapping": [1, 1, 2, 2] }"#
            )),
            PresetRefusal::MissingCode {
                element: "w".to_string()
            }
        );
        assert_eq!(
            refusal(&fixture(
                r#"{ "type": 3, "pos": [0, 0], "mapping": [1, 1, 2, 2], "code": 700 }"#
            )),
            PresetRefusal::CodeOutOfRange {
                element: "#1".to_string(),
                code: 700
            }
        );
        assert_eq!(
            refusal(&fixture(
                r#"{ "type": 0, "id": "flat", "pos": [0, 0], "mapping": [1, 1, 0, 2] }"#
            )),
            PresetRefusal::EmptyMapping {
                element: "flat".to_string()
            }
        );
    }

    #[test]
    fn presets_over_the_limits_are_refused_before_their_elements_are_read() {
        let element = r#"{ "type": 0, "pos": [0, 0], "mapping": [1, 1, 2, 2] }"#;
        let crowded = vec![element; MAX_PRESET_ELEMENTS + 1].join(",");
        let padded = format!("{}{}", fixture(KEY_W), " ".repeat(MAX_PRESET_JSON_BYTES));

        assert_eq!(
            refusal(&fixture(&crowded)),
            PresetRefusal::TooManyElements {
                count: MAX_PRESET_ELEMENTS + 1
            }
        );
        assert_eq!(refusal(&fixture("")), PresetRefusal::NoElements);
        assert_eq!(
            refusal(&padded),
            PresetRefusal::JsonTooLarge {
                bytes: padded.len()
            }
        );
    }
}
