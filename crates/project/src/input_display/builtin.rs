use serde::{Deserialize, Serialize};

use super::{
    InputDisplayStyle,
    gamepad::{GamepadKind, gamepad_buttons},
    keys::key_codes,
    preset::{Element, ElementKind, FillDirection, Preset, STATE_GAP, SpriteRect},
};

const UNIT: f64 = 40.0;
const KEY_GAP: f64 = 4.0;
const KEY_HEIGHT: u32 = 36;
const KEY_RADIUS: u32 = 6;
const BLEED: u32 = 1;
const MOUSE_GAP: u32 = 24;
const MOUSE_WIDTH: u32 = 96;
const MOUSE_HEIGHT: u32 = 150;
const MOUSE_SIDE_OVERHANG: u32 = 4;
const ATLAS_ROW_WIDTH: u32 = 1024;
const GAMEPAD_WIDTH: u32 = 420;
const GAMEPAD_HEIGHT: u32 = 262;
const STICK_TRAVEL: u16 = 9;
const FONT_FAMILY: &str = "Inter, 'Segoe UI', 'Microsoft YaHei UI', Arial, sans-serif";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutKey {
    pub label: String,
    pub code: u16,
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

pub const MOUSE_BLOCK: (f64, f64) = (
    (MOUSE_WIDTH + MOUSE_SIDE_OVERHANG) as f64 / UNIT,
    MOUSE_HEIGHT as f64 / UNIT,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MousePart {
    Left,
    Right,
    Wheel,
    Side,
}

impl MousePart {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Wheel, Self::Side];
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseParts {
    pub left: bool,
    pub right: bool,
    pub wheel: bool,
    pub side: bool,
}

impl Default for MouseParts {
    fn default() -> Self {
        Self {
            left: true,
            right: true,
            wheel: true,
            side: true,
        }
    }
}

impl MouseParts {
    pub fn shows(&self, part: MousePart) -> bool {
        match part {
            MousePart::Left => self.left,
            MousePart::Right => self.right,
            MousePart::Wheel => self.wheel,
            MousePart::Side => self.side,
        }
    }

    pub fn set(&mut self, part: MousePart, shown: bool) {
        *match part {
            MousePart::Left => &mut self.left,
            MousePart::Right => &mut self.right,
            MousePart::Wheel => &mut self.wheel,
            MousePart::Side => &mut self.side,
        } = shown;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutMouse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<(f64, f64)>,
    #[serde(default, flatten)]
    pub parts: MouseParts,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MouseInFile {
    Whole(bool),
    Block(LayoutMouse),
}

fn mouse_in_file<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<LayoutMouse>, D::Error> {
    Ok(match Option::<MouseInFile>::deserialize(deserializer)? {
        Some(MouseInFile::Whole(true)) => Some(LayoutMouse::default()),
        Some(MouseInFile::Block(mouse)) => Some(mouse),
        Some(MouseInFile::Whole(false)) | None => None,
    })
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyLayout {
    #[serde(default)]
    pub keys: Vec<LayoutKey>,
    #[serde(default, deserialize_with = "mouse_in_file")]
    pub mouse: Option<LayoutMouse>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetArt {
    pub json: String,
    pub atlas_svg: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinPresetId {
    WasdMouse,
    FullKeyboard,
    Gamepad,
    GamepadPlayStation,
    GamepadSwitch,
}

impl BuiltinPresetId {
    pub const ALL: [Self; 3] = [Self::WasdMouse, Self::FullKeyboard, Self::Gamepad];
    const DRAWN: [Self; 5] = [
        Self::WasdMouse,
        Self::FullKeyboard,
        Self::Gamepad,
        Self::GamepadPlayStation,
        Self::GamepadSwitch,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::WasdMouse => "wasd-mouse",
            Self::FullKeyboard => "full-keyboard",
            Self::Gamepad => "gamepad",
            Self::GamepadPlayStation => "gamepad-playstation",
            Self::GamepadSwitch => "gamepad-switch",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::DRAWN.into_iter().find(|preset| preset.id() == id)
    }

    pub fn controller(self) -> Option<GamepadKind> {
        match self {
            Self::WasdMouse | Self::FullKeyboard => None,
            Self::Gamepad => Some(GamepadKind::Xbox),
            Self::GamepadPlayStation => Some(GamepadKind::PlayStation),
            Self::GamepadSwitch => Some(GamepadKind::Switch),
        }
    }

    pub fn for_controller(self, controller: Option<GamepadKind>) -> Self {
        match (self.controller(), controller) {
            (Some(_), Some(GamepadKind::Xbox)) => Self::Gamepad,
            (Some(_), Some(GamepadKind::PlayStation)) => Self::GamepadPlayStation,
            (Some(_), Some(GamepadKind::Switch)) => Self::GamepadSwitch,
            _ => self,
        }
    }

    pub fn frame_share(self) -> f64 {
        match self {
            Self::WasdMouse => 0.30,
            Self::FullKeyboard => 0.52,
            Self::Gamepad | Self::GamepadPlayStation | Self::GamepadSwitch => 0.24,
        }
    }

    pub fn size(self) -> (u32, u32) {
        match self.controller() {
            Some(_) => (GAMEPAD_WIDTH + 2 * BLEED, GAMEPAD_HEIGHT + 2 * BLEED),
            None => self.layout().size(),
        }
    }

    pub fn layout(self) -> KeyLayout {
        use key_codes::*;
        let letters = |text: &'static str| {
            text.char_indices()
                .map(move |(index, character)| (&text[index..index + 1], letter(character), 1.0))
        };
        let digits = |text: &'static str| {
            text.char_indices()
                .map(move |(index, character)| (&text[index..index + 1], digit(character), 1.0))
        };
        let rows: Vec<Vec<(&'static str, u16, f64)>> = match self {
            Self::WasdMouse => vec![
                [("Tab", TAB, 1.5)]
                    .into_iter()
                    .chain(letters("QWER"))
                    .collect(),
                [("Shift", SHIFT_LEFT, 1.5)]
                    .into_iter()
                    .chain(letters("ASDF"))
                    .collect(),
                vec![("Ctrl", CONTROL_LEFT, 1.5), ("Space", SPACE, 4.0)],
            ],
            Self::FullKeyboard => vec![
                [("`", BACKQUOTE, 1.0)]
                    .into_iter()
                    .chain(digits("1234567890"))
                    .chain([
                        ("-", MINUS, 1.0),
                        ("=", EQUALS, 1.0),
                        ("Back", BACKSPACE, 2.0),
                    ])
                    .collect(),
                [("Tab", TAB, 1.5)]
                    .into_iter()
                    .chain(letters("QWERTYUIOP"))
                    .chain([
                        ("[", OPEN_BRACKET, 1.0),
                        ("]", CLOSE_BRACKET, 1.0),
                        ("\\", BACK_SLASH, 1.5),
                    ])
                    .collect(),
                [("Caps", CAPS_LOCK, 1.75)]
                    .into_iter()
                    .chain(letters("ASDFGHJKL"))
                    .chain([
                        (";", SEMICOLON, 1.0),
                        ("'", QUOTE, 1.0),
                        ("Enter", ENTER, 2.25),
                    ])
                    .collect(),
                [("Shift", SHIFT_LEFT, 2.25)]
                    .into_iter()
                    .chain(letters("ZXCVBNM"))
                    .chain([
                        (",", COMMA, 1.0),
                        (".", PERIOD, 1.0),
                        ("/", SLASH, 1.0),
                        ("Shift", SHIFT_RIGHT, 2.75),
                    ])
                    .collect(),
                vec![
                    ("Ctrl", CONTROL_LEFT, 1.25),
                    ("Win", META_LEFT, 1.25),
                    ("Alt", ALT_LEFT, 1.25),
                    ("", SPACE, 6.25),
                    ("Alt", ALT_RIGHT, 1.25),
                    ("Win", META_RIGHT, 1.25),
                    ("Menu", CONTEXT_MENU, 1.25),
                    ("Ctrl", CONTROL_RIGHT, 1.25),
                ],
            ],
            Self::Gamepad | Self::GamepadPlayStation | Self::GamepadSwitch => {
                return KeyLayout::default();
            }
        };
        let mut keys = Vec::new();
        for (row, entries) in rows.into_iter().enumerate() {
            let mut x = 0.0;
            for (label, code, width) in entries {
                keys.push(LayoutKey {
                    label: label.to_string(),
                    code,
                    x,
                    y: row as f64,
                    width,
                });
                x += width;
            }
        }
        KeyLayout {
            keys,
            mouse: Some(LayoutMouse::default()),
        }
    }

    pub fn art(self, style: InputDisplayStyle) -> PresetArt {
        match self.controller() {
            Some(controller) => {
                let (width, height) = self.size();
                pack(
                    gamepad_parts(controller, &Palette::of(style)),
                    width,
                    height,
                )
            }
            None => self.layout().art(style),
        }
    }
}

pub fn preset_for_controller(id: &str, controller: Option<GamepadKind>) -> &str {
    match BuiltinPresetId::from_id(id) {
        Some(preset) => preset.for_controller(controller).id(),
        None => id,
    }
}

struct Paint {
    color: &'static str,
    opacity: f32,
}

impl Paint {
    const fn solid(color: &'static str) -> Self {
        Self {
            color,
            opacity: 1.0,
        }
    }

    const NONE: Self = Self {
        color: "#000000",
        opacity: 0.0,
    };

    fn fill(&self) -> String {
        format!(r#"fill="{}" fill-opacity="{}""#, self.color, self.opacity)
    }

    fn stroke(&self) -> String {
        if self.opacity <= 0.0 {
            String::new()
        } else {
            format!(
                r#" stroke="{}" stroke-opacity="{}" stroke-width="1.5""#,
                self.color, self.opacity
            )
        }
    }
}

struct Palette {
    rest: Paint,
    rest_ink: Paint,
    line: Paint,
    on: Paint,
    on_ink: Paint,
    body: Paint,
    well: Paint,
}

impl Palette {
    fn of(style: InputDisplayStyle) -> Self {
        let on = Paint::solid("#ffffff");
        let on_ink = Paint::solid("#111111");
        match style {
            InputDisplayStyle::Keycaps => Self {
                rest: Paint::solid("#181818"),
                rest_ink: Paint::solid("#c4c4c4"),
                line: Paint::NONE,
                on,
                on_ink,
                body: Paint::solid("#3d3d3d"),
                well: Paint::solid("#5a5a5a"),
            },
            InputDisplayStyle::Outline => Self {
                rest: Paint {
                    color: "#000000",
                    opacity: 0.32,
                },
                rest_ink: Paint::solid("#ffffff"),
                line: Paint::solid("#ffffff"),
                on,
                on_ink,
                body: Paint {
                    color: "#000000",
                    opacity: 0.32,
                },
                well: Paint::solid("#ffffff"),
            },
            InputDisplayStyle::Ghost => Self {
                rest: Paint {
                    color: "#ffffff",
                    opacity: 0.16,
                },
                rest_ink: Paint::NONE,
                line: Paint::NONE,
                on,
                on_ink,
                body: Paint {
                    color: "#000000",
                    opacity: 0.28,
                },
                well: Paint {
                    color: "#ffffff",
                    opacity: 0.3,
                },
            },
        }
    }
}

struct Part {
    id: String,
    kind: ElementKind,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    z_level: i32,
    states: Vec<String>,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn key_part(index: usize, key: &LayoutKey, origin_y: u32, palette: &Palette) -> Part {
    let x = (key.x * UNIT).round() as u32;
    let y = origin_y + (key.y * UNIT).round() as u32;
    let width = ((key.width * UNIT - KEY_GAP).round() as u32).max(KEY_RADIUS * 2);
    let font_size = if key.label.chars().count() > 1 {
        10.0
    } else {
        13.0
    };
    let state = |fill: &Paint, ink: &Paint| {
        let mut svg = format!(
            r#"<rect x="{BLEED}" y="{BLEED}" width="{width}" height="{KEY_HEIGHT}" rx="{KEY_RADIUS}" {}{}/>"#,
            fill.fill(),
            palette.line.stroke()
        );
        if !key.label.is_empty() && ink.opacity > 0.0 {
            svg.push_str(&format!(
                r#"<text x="{}" y="{}" font-family="{FONT_FAMILY}" font-size="{font_size}" font-weight="500" text-anchor="middle" {}>{}</text>"#,
                f64::from(BLEED) + f64::from(width) / 2.0,
                f64::from(BLEED) + f64::from(KEY_HEIGHT) / 2.0 + font_size * 0.36,
                ink.fill(),
                escape(&key.label)
            ));
        }
        svg
    };
    Part {
        id: format!("key-{index}-{:04x}", key.code),
        kind: ElementKind::KeyboardKey { code: key.code },
        x,
        y,
        width: width + 2 * BLEED,
        height: KEY_HEIGHT + 2 * BLEED,
        z_level: 1,
        states: vec![
            state(&palette.rest, &palette.rest_ink),
            state(&palette.on, &palette.on_ink),
        ],
    }
}

fn mouse_parts(origin_x: u32, origin_y: u32, shown: MouseParts, palette: &Palette) -> Vec<Part> {
    let stroke = palette.line.stroke();
    let shape = |left: u32, top: u32, body: &str, fill: &Paint| {
        format!(
            r#"<g transform="translate({} {})"><{body} {}{stroke}/></g>"#,
            i64::from(BLEED) - i64::from(left),
            i64::from(BLEED) - i64::from(top),
            fill.fill()
        )
    };
    let button =
        |id: &str, button: u8, left: u32, top: u32, width: u32, height: u32, body: &str| Part {
            id: id.to_string(),
            kind: ElementKind::MouseButton { button },
            x: origin_x + left,
            y: origin_y + top,
            width: width + 2 * BLEED,
            height: height + 2 * BLEED,
            z_level: 1,
            states: vec![
                shape(left, top, body, &palette.rest),
                shape(left, top, body, &palette.on),
            ],
        };
    let side = MOUSE_SIDE_OVERHANG;
    let wheel_up = r#"path d="M45 33 V25 a7 7 0 0 1 14 0 V33 Z""#;
    let wheel_down = r#"path d="M45 35 V43 a7 7 0 0 0 14 0 V35 Z""#;
    let wheel = |up: &Paint, down: &Paint| {
        format!(
            "{}{}",
            shape(45, 18, wheel_up, up),
            shape(45, 18, wheel_down, down)
        )
    };
    let mut parts = vec![
        Part {
            id: "mouse-body".to_string(),
            kind: ElementKind::Texture,
            x: origin_x + side,
            y: origin_y,
            width: MOUSE_WIDTH + 2 * BLEED,
            height: MOUSE_HEIGHT + 2 * BLEED,
            z_level: 0,
            states: vec![format!(
                r#"<rect x="{BLEED}" y="{BLEED}" width="{MOUSE_WIDTH}" height="{MOUSE_HEIGHT}" rx="44" {}{stroke}/>"#,
                palette.body.fill()
            )],
        },
        button(
            "mouse-left",
            1,
            8,
            4,
            41,
            62,
            r#"path d="M8 66 V46 A42 42 0 0 1 49 4 V66 Z""#,
        ),
        button(
            "mouse-right",
            2,
            55,
            4,
            41,
            62,
            r#"path d="M96 66 V46 A42 42 0 0 0 55 4 V66 Z""#,
        ),
        Part {
            id: "mouse-wheel".to_string(),
            kind: ElementKind::MouseWheel,
            x: origin_x + 45,
            y: origin_y + 18,
            width: 14 + 2 * BLEED,
            height: 32 + 2 * BLEED,
            z_level: 2,
            states: vec![
                wheel(&palette.rest, &palette.rest),
                wheel(&palette.on, &palette.on),
                wheel(&palette.on, &palette.rest),
                wheel(&palette.rest, &palette.on),
            ],
        },
        button(
            "mouse-side-1",
            4,
            0,
            74,
            7,
            18,
            r#"rect x="0" y="74" width="7" height="18" rx="3""#,
        ),
        button(
            "mouse-side-2",
            5,
            0,
            96,
            7,
            18,
            r#"rect x="0" y="96" width="7" height="18" rx="3""#,
        ),
    ];
    parts.retain(|part| match part.id.as_str() {
        "mouse-left" => shown.left,
        "mouse-right" => shown.right,
        "mouse-wheel" => shown.wheel,
        "mouse-side-1" | "mouse-side-2" => shown.side,
        _ => true,
    });
    parts
}

struct Bounds {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

impl Bounds {
    fn round(cx: u32, cy: u32, radius: u32) -> Self {
        Self {
            left: cx - radius,
            top: cy - radius,
            width: radius * 2,
            height: radius * 2,
        }
    }

    fn centre(&self) -> (f64, f64) {
        (
            f64::from(self.left) + f64::from(self.width) / 2.0,
            f64::from(self.top) + f64::from(self.height) / 2.0,
        )
    }

    fn part(self, id: &str, kind: ElementKind, z_level: i32, states: Vec<String>) -> Part {
        let shift = format!(
            r#"<g transform="translate({} {})">"#,
            i64::from(BLEED) - i64::from(self.left),
            i64::from(BLEED) - i64::from(self.top)
        );
        Part {
            id: id.to_string(),
            kind,
            x: self.left,
            y: self.top,
            width: self.width + 2 * BLEED,
            height: self.height + 2 * BLEED,
            z_level,
            states: states
                .into_iter()
                .map(|state| format!("{shift}{state}</g>"))
                .collect(),
        }
    }
}

fn label(text: &str, (cx, cy): (f64, f64), ink: &Paint) -> String {
    if text.is_empty() || ink.opacity <= 0.0 {
        return String::new();
    }
    let glyph = |shape: &str| {
        format!(
            r#"<g transform="translate({cx} {cy})" fill="none" stroke="{}" stroke-opacity="{}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">{shape}</g>"#,
            ink.color, ink.opacity
        )
    };
    match text {
        "cross" => glyph(r#"<path d="M-4 -4 L4 4 M4 -4 L-4 4"/>"#),
        "circle" => glyph(r#"<circle r="5"/>"#),
        "square" => glyph(r#"<rect x="-4.5" y="-4.5" width="9" height="9"/>"#),
        "triangle" => glyph(r#"<path d="M0 -5.5 L5.2 4 L-5.2 4 Z"/>"#),
        _ => {
            let font_size = if text.chars().count() > 1 { 10.0 } else { 13.0 };
            format!(
                r#"<text x="{cx}" y="{}" font-family="{FONT_FAMILY}" font-size="{font_size}" font-weight="500" text-anchor="middle" {}>{}</text>"#,
                cy + font_size * 0.36,
                ink.fill(),
                escape(text)
            )
        }
    }
}

struct ControllerNames {
    face: [&'static str; 4],
    shoulders: [&'static str; 2],
    triggers: [&'static str; 2],
    sticks_side_by_side: bool,
}

impl ControllerNames {
    fn of(controller: GamepadKind) -> Self {
        match controller {
            GamepadKind::Xbox => Self {
                face: ["A", "B", "X", "Y"],
                shoulders: ["LB", "RB"],
                triggers: ["LT", "RT"],
                sticks_side_by_side: false,
            },
            GamepadKind::PlayStation => Self {
                face: ["cross", "circle", "square", "triangle"],
                shoulders: ["L1", "R1"],
                triggers: ["L2", "R2"],
                sticks_side_by_side: true,
            },
            GamepadKind::Switch => Self {
                face: ["B", "A", "Y", "X"],
                shoulders: ["L", "R"],
                triggers: ["ZL", "ZR"],
                sticks_side_by_side: false,
            },
        }
    }
}

fn gamepad_parts(controller: GamepadKind, palette: &Palette) -> Vec<Part> {
    use gamepad_buttons::*;
    let names = ControllerNames::of(controller);
    let stroke = palette.line.stroke();
    let two_states = |shape: &dyn Fn(&Paint) -> String, text: &str, centre: (f64, f64)| {
        vec![
            format!(
                "{}{}",
                shape(&palette.rest),
                label(text, centre, &palette.rest_ink)
            ),
            format!(
                "{}{}",
                shape(&palette.on),
                label(text, centre, &palette.on_ink)
            ),
        ]
    };
    let pill = |id: &str, kind: ElementKind, bounds: Bounds, radius: u32, text: &str| {
        let shape = |fill: &Paint| {
            format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{radius}" {}{stroke}/>"#,
                bounds.left,
                bounds.top,
                bounds.width,
                bounds.height,
                fill.fill()
            )
        };
        let states = two_states(&shape, text, bounds.centre());
        bounds.part(id, kind, 1, states)
    };
    let round =
        |id: &str, kind: ElementKind, z_level: i32, cx: u32, cy: u32, radius: u32, text: &str| {
            let shape = |fill: &Paint| {
                format!(
                    r#"<circle cx="{cx}" cy="{cy}" r="{radius}" {}{stroke}/>"#,
                    fill.fill()
                )
            };
            let bounds = Bounds::round(cx, cy, radius);
            let states = two_states(&shape, text, bounds.centre());
            bounds.part(id, kind, z_level, states)
        };
    let button = |code: u16| ElementKind::GamepadButton { code };
    let well = |id: &str, cx: u32, cy: u32| {
        Bounds::round(cx, cy, 26).part(
            id,
            ElementKind::Texture,
            1,
            vec![format!(
                r#"<circle cx="{cx}" cy="{cy}" r="25" fill="none"{}/>"#,
                palette.well.stroke()
            )],
        )
    };
    let stick = |id: &str, right: bool, cx: u32, cy: u32| {
        round(
            id,
            ElementKind::AnalogStick {
                right,
                radius: STICK_TRAVEL,
            },
            2,
            cx,
            cy,
            15,
            "",
        )
    };
    let trigger = |id: &str, right: bool, left: u32, text: &str| {
        pill(
            id,
            ElementKind::Trigger {
                right,
                fill: FillDirection::Right,
            },
            Bounds {
                left,
                top: 4,
                width: 46,
                height: 20,
            },
            6,
            text,
        )
    };
    let shoulder = |id: &str, code: u16, left: u32, text: &str| {
        pill(
            id,
            button(code),
            Bounds {
                left,
                top: 30,
                width: 74,
                height: 18,
            },
            9,
            text,
        )
    };
    let ((left_x, left_y), (cross_x, cross_y)) = if names.sticks_side_by_side {
        ((165, 152), (120, 108))
    } else {
        ((120, 108), (165, 152))
    };
    let arm = |id: &str, code: u16, left: u32, top: u32, width: u32, height: u32| {
        pill(
            id,
            button(code),
            Bounds {
                left,
                top,
                width,
                height,
            },
            3,
            "",
        )
    };
    let body = Bounds {
        left: 20,
        top: 44,
        width: 380,
        height: 208,
    };
    vec![
        trigger("trigger-left", false, 90, names.triggers[0]),
        trigger("trigger-right", true, 284, names.triggers[1]),
        shoulder("shoulder-left", LEFT_SHOULDER, 76, names.shoulders[0]),
        shoulder("shoulder-right", RIGHT_SHOULDER, 270, names.shoulders[1]),
        body.part(
            "gamepad-body",
            ElementKind::Texture,
            0,
            vec![format!(
                r#"<path d="M90 62 Q210 44 330 62 Q372 70 388 152 Q400 217 372 238 Q345 252 318 207 Q300 180 270 180 L150 180 Q120 180 102 207 Q75 252 48 238 Q20 217 32 152 Q48 70 90 62 Z" {}{stroke}/>"#,
                palette.body.fill()
            )],
        ),
        well("well-left", left_x, left_y),
        well("well-right", 255, 152),
        stick("stick-left", false, left_x, left_y),
        stick("stick-right", true, 255, 152),
        arm("dpad-up", DPAD_UP, cross_x - 7, cross_y - 23, 14, 16),
        arm("dpad-down", DPAD_DOWN, cross_x - 7, cross_y + 7, 14, 16),
        arm("dpad-left", DPAD_LEFT, cross_x - 23, cross_y - 7, 16, 14),
        arm("dpad-right", DPAD_RIGHT, cross_x + 7, cross_y - 7, 16, 14),
        round("face-south", button(SOUTH), 1, 300, 130, 11, names.face[0]),
        round("face-east", button(EAST), 1, 322, 108, 11, names.face[1]),
        round("face-west", button(WEST), 1, 278, 108, 11, names.face[2]),
        round("face-north", button(NORTH), 1, 300, 86, 11, names.face[3]),
        round("back", button(BACK), 1, 185, 108, 7, ""),
        round("start", button(START), 1, 235, 108, 7, ""),
        round("guide", button(GUIDE), 1, 210, 84, 9, ""),
    ]
}

fn pack(parts: Vec<Part>, width: u32, height: u32) -> PresetArt {
    let mut svg = String::new();
    let mut elements = Vec::with_capacity(parts.len());
    let (mut cursor_x, mut cursor_y, mut row_height, mut atlas_width) = (1u32, 1u32, 0u32, 0u32);
    for part in parts {
        let (columns, rows) = part.kind.states();
        let cell_width = part.width * columns + STATE_GAP * (columns - 1);
        let cell_height = part.height * rows + STATE_GAP * (rows - 1);
        if cursor_x > 1 && cursor_x + cell_width + 1 > ATLAS_ROW_WIDTH {
            cursor_x = 1;
            cursor_y += row_height + STATE_GAP;
            row_height = 0;
        }
        for (index, state) in part.states.iter().enumerate() {
            let index = index as u32;
            let (column, row) = if columns > 1 { (index, 0) } else { (0, index) };
            svg.push_str(&format!(
                r#"<g transform="translate({} {})">{state}</g>"#,
                cursor_x + column * (part.width + STATE_GAP),
                cursor_y + row * (part.height + STATE_GAP)
            ));
        }
        elements.push(Element {
            id: part.id,
            kind: part.kind,
            x: part.x,
            y: part.y,
            mapping: SpriteRect {
                u: cursor_x,
                v: cursor_y,
                width: part.width,
                height: part.height,
            },
            z_level: part.z_level,
        });
        cursor_x += cell_width + STATE_GAP;
        row_height = row_height.max(cell_height);
        atlas_width = atlas_width.max(cursor_x - STATE_GAP + 1);
    }
    let atlas_width = atlas_width.max(2);
    let atlas_height = cursor_y + row_height + 1;
    PresetArt {
        json: Preset {
            width,
            height,
            elements,
        }
        .to_json(),
        atlas_svg: format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{atlas_width}" height="{atlas_height}" viewBox="0 0 {atlas_width} {atlas_height}">{svg}</svg>"#
        ),
        atlas_width,
        atlas_height,
    }
}

struct LayoutExtent {
    keys_top: u32,
    mouse: Option<(u32, u32)>,
    width: u32,
    height: u32,
}

impl KeyLayout {
    fn extent(&self) -> LayoutExtent {
        let keys_width = self
            .keys
            .iter()
            .map(|key| ((key.x + key.width) * UNIT).round() as u32)
            .max()
            .unwrap_or(0);
        let keys_height = self
            .keys
            .iter()
            .map(|key| ((key.y + 1.0) * UNIT - KEY_GAP).round() as u32)
            .max()
            .unwrap_or(0);
        let (keys_top, mouse) = match self.mouse.map(|mouse| mouse.at) {
            None => (0, None),
            Some(None) => {
                let content_height = keys_height.max(MOUSE_HEIGHT);
                let beside = if keys_width == 0 {
                    0
                } else {
                    keys_width + MOUSE_GAP - MOUSE_SIDE_OVERHANG
                };
                (
                    (content_height - keys_height) / 2,
                    Some((beside, (content_height - MOUSE_HEIGHT) / 2)),
                )
            }
            Some(Some((x, y))) => {
                let pixels = |units: f64| {
                    if units.is_finite() {
                        (units.max(0.0) * UNIT).round() as u32
                    } else {
                        0
                    }
                };
                (0, Some((pixels(x), pixels(y))))
            }
        };
        let block = MOUSE_SIDE_OVERHANG + MOUSE_WIDTH;
        LayoutExtent {
            keys_top,
            mouse,
            width: keys_width
                .saturating_sub(KEY_GAP as u32)
                .max(mouse.map_or(0, |(x, _)| x + block))
                + 2 * BLEED,
            height: (keys_top + keys_height).max(mouse.map_or(0, |(_, y)| y + MOUSE_HEIGHT))
                + 2 * BLEED,
        }
    }

    pub fn frame_share(&self) -> f64 {
        let reference = BuiltinPresetId::WasdMouse;
        let share = reference.frame_share() * f64::from(self.size().0)
            / f64::from(reference.layout().size().0.max(1));
        share.clamp(0.06, 0.6)
    }

    pub fn size(&self) -> (u32, u32) {
        let extent = self.extent();
        (extent.width, extent.height)
    }

    pub fn art(&self, style: InputDisplayStyle) -> PresetArt {
        let palette = Palette::of(style);
        let LayoutExtent {
            keys_top,
            mouse,
            width,
            height,
        } = self.extent();
        let mut parts: Vec<Part> = self
            .keys
            .iter()
            .enumerate()
            .map(|(index, key)| key_part(index, key, keys_top, &palette))
            .collect();
        if let (Some((x, y)), Some(shown)) = (mouse, self.mouse) {
            parts.extend(mouse_parts(x, y, shown.parts, &palette));
        }

        pack(parts, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_display::{DisplayState, GamepadAxes, GamepadSnapshot, Input, key_code};

    fn read(art: &PresetArt) -> Preset {
        Preset::parse_own(&art.json, (art.atlas_width, art.atlas_height)).unwrap()
    }

    fn key_count(preset: &Preset) -> usize {
        preset
            .elements
            .iter()
            .filter(|element| matches!(element.kind, ElementKind::KeyboardKey { .. }))
            .count()
    }

    #[test]
    fn the_reader_loads_every_built_in_preset_in_every_style() {
        for id in BuiltinPresetId::ALL {
            for style in InputDisplayStyle::ALL {
                let art = id.art(style);
                let preset = read(&art);

                assert!(art.atlas_svg.starts_with("<svg "), "{id:?} {style:?}");
                assert!(preset.width > 0 && preset.height > 0);
                assert_eq!(BuiltinPresetId::from_id(id.id()), Some(id));
            }
        }
    }

    #[test]
    fn the_built_in_presets_show_their_keys_and_a_five_button_mouse() {
        let wasd = read(&BuiltinPresetId::WasdMouse.art(InputDisplayStyle::Keycaps));
        let full = read(&BuiltinPresetId::FullKeyboard.art(InputDisplayStyle::Keycaps));

        assert_eq!((wasd.width, wasd.height), (342, 152));
        assert_eq!((full.width, full.height), (722, 198));
        assert_eq!(key_count(&wasd), 12);
        assert_eq!(key_count(&full), 61);
        for preset in [&wasd, &full] {
            let mut buttons: Vec<u8> = preset
                .elements
                .iter()
                .filter_map(|element| match element.kind {
                    ElementKind::MouseButton { button } => Some(button),
                    _ => None,
                })
                .collect();
            buttons.sort();
            assert_eq!(buttons, vec![1, 2, 4, 5]);
            assert_eq!(
                preset
                    .elements
                    .iter()
                    .filter(|element| element.kind == ElementKind::MouseWheel)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn pressing_w_changes_only_the_w_key() {
        let preset = read(&BuiltinPresetId::WasdMouse.art(InputDisplayStyle::Outline));
        let mut pressed = DisplayState::default();
        pressed.apply(Input::Key(key_code("W").unwrap()), true);

        let changed = preset
            .sprites(&DisplayState::default())
            .into_iter()
            .zip(preset.sprites(&pressed))
            .filter(|(released, pressed)| released != pressed)
            .count();

        assert_eq!(changed, 1);
    }

    #[test]
    fn every_key_the_recorder_can_name_on_the_full_keyboard_has_its_code() {
        let recorded = ('A'..='Z')
            .map(|letter| letter.to_string())
            .chain((0..=9).map(|digit| format!("Key{digit}")))
            .chain(
                [
                    "Grave",
                    "Minus",
                    "Equal",
                    "Backspace",
                    "Tab",
                    "LeftBracket",
                    "RightBracket",
                    "BackSlash",
                    "CapsLock",
                    "Semicolon",
                    "Apostrophe",
                    "Enter",
                    "LShift",
                    "RShift",
                    "Comma",
                    "Dot",
                    "Slash",
                    "LControl",
                    "RControl",
                    "Meta",
                    "RMeta",
                    "LAlt",
                    "RAlt",
                    "Space",
                ]
                .map(str::to_string),
            )
            .filter_map(|name| key_code(&name))
            .collect::<std::collections::BTreeSet<_>>();

        let unnamed: Vec<String> = BuiltinPresetId::FullKeyboard
            .layout()
            .keys
            .into_iter()
            .filter(|key| !recorded.contains(&key.code))
            .map(|key| key.label)
            .collect();

        assert_eq!(unnamed, vec!["Menu".to_string()]);
    }

    #[test]
    fn atlas_sprites_keep_the_gaps_the_format_asks_for() {
        for id in BuiltinPresetId::ALL {
            let art = id.art(InputDisplayStyle::Keycaps);
            let preset = read(&art);
            let mut rects = Vec::new();
            for element in &preset.elements {
                let (columns, rows) = element.kind.states();
                for column in 0..columns {
                    for row in 0..rows {
                        rects.push((
                            element.mapping.u + column * (element.mapping.width + STATE_GAP),
                            element.mapping.v + row * (element.mapping.height + STATE_GAP),
                            element.mapping.width,
                            element.mapping.height,
                        ));
                    }
                }
            }
            for (index, a) in rects.iter().enumerate() {
                assert!(a.0 >= 1 && a.1 >= 1);
                assert!(a.0 + a.2 < art.atlas_width && a.1 + a.3 < art.atlas_height);
                for b in &rects[index + 1..] {
                    let apart_x = a.0 + a.2 + STATE_GAP <= b.0 || b.0 + b.2 + STATE_GAP <= a.0;
                    let apart_y = a.1 + a.3 + STATE_GAP <= b.1 || b.1 + b.3 + STATE_GAP <= a.1;
                    assert!(apart_x || apart_y, "{a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn a_made_layout_draws_its_keys_as_large_as_the_built_in_ones() {
        let wasd = BuiltinPresetId::WasdMouse.layout();
        let single = KeyLayout {
            keys: vec![wasd.keys[1].clone()],
            mouse: None,
        };

        assert_eq!(wasd.frame_share(), BuiltinPresetId::WasdMouse.frame_share());
        let per_pixel = |layout: &KeyLayout| layout.frame_share() / f64::from(layout.size().0);
        assert!((per_pixel(&single) - per_pixel(&wasd)).abs() < 1e-3);
        assert_eq!(BuiltinPresetId::FullKeyboard.layout().frame_share(), 0.6);
    }

    #[test]
    fn a_layout_round_trips_through_its_file() {
        let layout = BuiltinPresetId::WasdMouse.layout();
        let json = serde_json::to_string(&layout).unwrap();

        assert_eq!(serde_json::from_str::<KeyLayout>(&json).unwrap(), layout);
        assert_eq!(
            serde_json::from_str::<KeyLayout>("{}").unwrap(),
            KeyLayout::default()
        );
    }

    #[test]
    fn the_built_in_wasd_picture_keeps_its_size_and_places() {
        let layout = BuiltinPresetId::WasdMouse.layout();
        let preset = read(&layout.art(InputDisplayStyle::Keycaps));
        let place = |id: &str| {
            let element = preset
                .elements
                .iter()
                .find(|element| element.id.starts_with(id))
                .unwrap();
            (element.x, element.y)
        };

        assert_eq!(layout.size(), (342, 152));
        assert_eq!(place("mouse-body"), (244, 0));
        assert_eq!(place("mouse-side-1"), (240, 74));
        assert_eq!(place("key-0-"), (0, 17));
        assert_eq!(preset.elements.len(), 12 + 6);
    }

    #[test]
    fn a_layout_saved_by_1_0_0_reads_as_the_whole_mouse_beside_the_keys() {
        let built_in = BuiltinPresetId::WasdMouse.layout();
        let mut saved = serde_json::to_value(&built_in).unwrap();
        saved["mouse"] = serde_json::json!(true);
        let read_back: KeyLayout = serde_json::from_value(saved.clone()).unwrap();

        assert_eq!(read_back, built_in);
        assert_eq!(
            read_back.art(InputDisplayStyle::Keycaps),
            built_in.art(InputDisplayStyle::Keycaps)
        );
        saved["mouse"] = serde_json::json!(false);
        assert_eq!(
            serde_json::from_value::<KeyLayout>(saved).unwrap().mouse,
            None
        );
    }

    fn placed(at: (f64, f64), parts: MouseParts) -> KeyLayout {
        KeyLayout {
            mouse: Some(LayoutMouse {
                at: Some(at),
                parts,
            }),
            ..BuiltinPresetId::WasdMouse.layout()
        }
    }

    #[test]
    fn a_placed_mouse_is_drawn_where_it_was_put() {
        let layout = placed((7.0, 0.5), MouseParts::default());
        let preset = read(&layout.art(InputDisplayStyle::Keycaps));
        let body = preset
            .elements
            .iter()
            .find(|element| element.id == "mouse-body")
            .unwrap();
        let key = preset
            .elements
            .iter()
            .find(|element| matches!(element.kind, ElementKind::KeyboardKey { .. }))
            .unwrap();

        assert_eq!((body.x, body.y), (7 * 40 + MOUSE_SIDE_OVERHANG, 20));
        assert_eq!(key.y, 0);
        assert_eq!(layout.size(), (7 * 40 + 100 + 2, 20 + 150 + 2));
        assert_eq!(layout.size(), (preset.width, preset.height));

        let left_of_the_keys = KeyLayout {
            keys: layout
                .keys
                .iter()
                .cloned()
                .map(|key| LayoutKey {
                    x: key.x + 3.0,
                    ..key
                })
                .collect(),
            ..placed((0.0, 0.0), MouseParts::default())
        };
        assert_eq!(
            left_of_the_keys.size(),
            ((3.0 * 40.0 + 216.0) as u32 + 2, 152)
        );
        let round_trip: KeyLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        assert_eq!(round_trip, layout);
    }

    #[test]
    fn a_mouse_draws_only_the_parts_chosen_for_it() {
        let ids = |parts: MouseParts| -> Vec<String> {
            read(&placed((7.0, 0.0), parts).art(InputDisplayStyle::Outline))
                .elements
                .into_iter()
                .map(|element| element.id)
                .filter(|id| id.starts_with("mouse-"))
                .collect()
        };

        assert_eq!(ids(MouseParts::default()).len(), 6);
        assert_eq!(
            ids(MouseParts {
                wheel: false,
                side: false,
                ..Default::default()
            }),
            ["mouse-body", "mouse-left", "mouse-right"]
        );
        let mut only_the_wheel = MouseParts {
            left: false,
            right: false,
            side: false,
            wheel: false,
        };
        only_the_wheel.set(MousePart::Wheel, true);
        assert!(only_the_wheel.shows(MousePart::Wheel));
        assert_eq!(ids(only_the_wheel), ["mouse-body", "mouse-wheel"]);
        assert_eq!(MOUSE_BLOCK, (2.5, 3.75));
    }

    #[test]
    fn a_layout_knows_its_size_without_being_drawn() {
        let mut layouts: Vec<KeyLayout> = BuiltinPresetId::ALL
            .into_iter()
            .filter(|preset| preset.controller().is_none())
            .map(BuiltinPresetId::layout)
            .collect();
        layouts.push(KeyLayout {
            mouse: None,
            ..BuiltinPresetId::WasdMouse.layout()
        });
        layouts.push(KeyLayout {
            keys: Vec::new(),
            mouse: Some(LayoutMouse::default()),
        });
        for layout in layouts {
            let preset = read(&layout.art(InputDisplayStyle::Outline));

            assert_eq!(layout.size(), (preset.width, preset.height));
        }
    }

    fn gamepad(controller: GamepadKind) -> Preset {
        read(
            &BuiltinPresetId::Gamepad
                .for_controller(Some(controller))
                .art(InputDisplayStyle::Keycaps),
        )
    }

    fn element<'a>(preset: &'a Preset, id: &str) -> &'a Element {
        preset
            .elements
            .iter()
            .find(|element| element.id == id)
            .unwrap()
    }

    #[test]
    fn the_gamepad_preset_shows_every_button_both_sticks_and_both_triggers() {
        for controller in GamepadKind::ALL {
            for style in InputDisplayStyle::ALL {
                let id = BuiltinPresetId::Gamepad.for_controller(Some(controller));
                let preset = read(&id.art(style));
                let mut buttons: Vec<u16> = preset
                    .elements
                    .iter()
                    .filter_map(|element| match element.kind {
                        ElementKind::GamepadButton { code } => Some(code),
                        _ => None,
                    })
                    .collect();
                buttons.sort();
                let sides = |wanted: fn(&ElementKind) -> Option<bool>| {
                    let mut sides: Vec<bool> = preset
                        .elements
                        .iter()
                        .filter_map(|element| wanted(&element.kind))
                        .collect();
                    sides.sort();
                    sides
                };

                assert_eq!((preset.width, preset.height), (422, 264));
                assert_eq!(id.size(), (preset.width, preset.height));
                assert_eq!(
                    buttons,
                    vec![0, 1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14],
                    "{controller:?}"
                );
                assert_eq!(
                    sides(|kind| match kind {
                        ElementKind::AnalogStick { right, radius: 9 } => Some(*right),
                        _ => None,
                    }),
                    vec![false, true]
                );
                assert_eq!(
                    sides(|kind| match kind {
                        ElementKind::Trigger {
                            right,
                            fill: FillDirection::Right,
                        } => Some(*right),
                        _ => None,
                    }),
                    vec![false, true]
                );
            }
        }
    }

    #[test]
    fn each_controller_gets_its_own_button_names_and_stick_layout() {
        let art = |controller| {
            BuiltinPresetId::Gamepad
                .for_controller(Some(controller))
                .art(InputDisplayStyle::Keycaps)
                .atlas_svg
        };
        let stick = |controller| {
            let preset = gamepad(controller);
            let left = element(&preset, "stick-left");
            let cross = element(&preset, "dpad-up");
            ((left.x, left.y), (cross.x, cross.y))
        };

        for (controller, shown, hidden) in [
            (GamepadKind::Xbox, [">LB<", ">RT<", ">A<"], [">L1<", ">ZL<"]),
            (
                GamepadKind::PlayStation,
                [">L1<", ">R2<", "<circle r=\"5\"/>"],
                [">LB<", ">A<"],
            ),
            (
                GamepadKind::Switch,
                [">ZL<", ">R<", ">A<"],
                [">LB<", ">L1<"],
            ),
        ] {
            let svg = art(controller);
            for text in shown {
                assert!(svg.contains(text), "{controller:?} lacks {text}");
            }
            for text in hidden {
                assert!(!svg.contains(text), "{controller:?} shows {text}");
            }
        }
        assert_eq!(stick(GamepadKind::Xbox), ((105, 93), (158, 129)));
        assert_eq!(stick(GamepadKind::Switch), stick(GamepadKind::Xbox));
        assert_eq!(stick(GamepadKind::PlayStation), ((150, 137), (113, 85)));
    }

    #[test]
    fn the_bottom_face_button_is_a_on_xbox_and_b_on_switch() {
        let south_label = |controller| {
            let art = BuiltinPresetId::Gamepad
                .for_controller(Some(controller))
                .art(InputDisplayStyle::Keycaps);
            let preset = read(&art);
            let south = element(&preset, "face-south").mapping;
            let cell = format!(r#"<g transform="translate({} {})">"#, south.u, south.v);
            let after = art.atlas_svg.split(&cell).nth(1).unwrap().to_string();
            let text = after.split("</text>").next().unwrap();
            text.rsplit('>').next().unwrap().to_string()
        };

        assert_eq!(south_label(GamepadKind::Xbox), "A");
        assert_eq!(south_label(GamepadKind::Switch), "B");
    }

    #[test]
    fn the_gamepad_preset_follows_the_controller_and_other_presets_do_not() {
        use GamepadKind::*;
        assert_eq!(BuiltinPresetId::ALL[2], BuiltinPresetId::Gamepad);
        assert_eq!(preset_for_controller("gamepad", None), "gamepad");
        assert_eq!(preset_for_controller("gamepad", Some(Xbox)), "gamepad");
        assert_eq!(
            preset_for_controller("gamepad", Some(PlayStation)),
            "gamepad-playstation"
        );
        assert_eq!(
            preset_for_controller("gamepad-playstation", Some(Switch)),
            "gamepad-switch"
        );
        assert_eq!(
            preset_for_controller("wasd-mouse", Some(PlayStation)),
            "wasd-mouse"
        );
        assert_eq!(preset_for_controller("imported", Some(Switch)), "imported");
        assert_eq!(
            BuiltinPresetId::from_id("gamepad-switch").and_then(BuiltinPresetId::controller),
            Some(Switch)
        );
    }

    #[test]
    fn a_pushed_stick_and_a_pulled_trigger_change_only_their_own_parts() {
        let preset = gamepad(GamepadKind::Xbox);
        let mut state = DisplayState::default();
        state.show_gamepad(&GamepadSnapshot {
            axes: GamepadAxes {
                left_stick: [127, -127],
                right_trigger: 255,
                ..Default::default()
            },
            ..Default::default()
        });
        let resting = preset.sprites(&DisplayState::default());
        let moved = preset.sprites(&state);
        let stick = element(&preset, "stick-left");

        assert_eq!(moved.len(), resting.len() + 1);
        assert!(moved.iter().any(|sprite| {
            sprite.source == stick.mapping && (sprite.x, sprite.y) == (stick.x + 9, stick.y - 9)
        }));
        assert_eq!(
            moved
                .iter()
                .filter(|sprite| !resting.contains(sprite))
                .count(),
            2
        );
    }

    #[test]
    fn a_layout_without_a_mouse_draws_only_its_keys() {
        let layout = KeyLayout {
            keys: vec![LayoutKey {
                label: "<".to_string(),
                code: 0x2C,
                x: 0.0,
                y: 0.0,
                width: 2.0,
            }],
            mouse: None,
        };
        let art = layout.art(InputDisplayStyle::Keycaps);
        let preset = read(&art);

        assert_eq!((preset.width, preset.height), (78, 38));
        assert_eq!(preset.elements.len(), 1);
        assert!(art.atlas_svg.contains("&lt;"));
    }
}
