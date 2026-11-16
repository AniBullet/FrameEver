use super::{
    InputDisplayStyle,
    keys::key_codes,
    preset::{Element, ElementKind, Preset, STATE_GAP, SpriteRect},
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
const FONT_FAMILY: &str = "Inter, 'Segoe UI', 'Microsoft YaHei UI', Arial, sans-serif";

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutKey {
    pub label: String,
    pub code: u16,
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyLayout {
    pub keys: Vec<LayoutKey>,
    pub mouse: bool,
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
}

impl BuiltinPresetId {
    pub const ALL: [Self; 2] = [Self::WasdMouse, Self::FullKeyboard];

    pub fn id(self) -> &'static str {
        match self {
            Self::WasdMouse => "wasd-mouse",
            Self::FullKeyboard => "full-keyboard",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.id() == id)
    }

    pub fn frame_share(self) -> f64 {
        match self {
            Self::WasdMouse => 0.30,
            Self::FullKeyboard => 0.52,
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
        KeyLayout { keys, mouse: true }
    }

    pub fn art(self, style: InputDisplayStyle) -> PresetArt {
        self.layout().art(style)
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

fn mouse_parts(origin_x: u32, origin_y: u32, palette: &Palette) -> Vec<Part> {
    let stroke = palette.line.stroke();
    let shape = |left: u32, top: u32, body: &str, fill: &Paint| {
        format!(
            r#"<g transform="translate({} {})"><{body} {}{stroke}/></g>"#,
            i64::from(BLEED) - i64::from(left),
            i64::from(BLEED) - i64::from(top),
            fill.fill()
        )
    };
    let button = |id: &str, button: u8, left: u32, top: u32, width: u32, height: u32, body: &str| {
        Part {
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
        }
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
    vec![
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
    ]
}

impl KeyLayout {
    pub fn art(&self, style: InputDisplayStyle) -> PresetArt {
        let palette = Palette::of(style);
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
        let mouse_height = if self.mouse { MOUSE_HEIGHT } else { 0 };
        let content_height = keys_height.max(mouse_height);
        let mut parts: Vec<Part> = self
            .keys
            .iter()
            .enumerate()
            .map(|(index, key)| key_part(index, key, (content_height - keys_height) / 2, &palette))
            .collect();
        let mut width = keys_width.saturating_sub(KEY_GAP as u32) + 2 * BLEED;
        if self.mouse {
            let origin_x = if keys_width == 0 {
                0
            } else {
                keys_width + MOUSE_GAP - MOUSE_SIDE_OVERHANG
            };
            parts.extend(mouse_parts(
                origin_x,
                (content_height - mouse_height) / 2,
                &palette,
            ));
            width = origin_x + MOUSE_SIDE_OVERHANG + MOUSE_WIDTH + 2 * BLEED;
        }
        let height = content_height + 2 * BLEED;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_display::{DisplayState, Input, key_code};

    fn read(art: &PresetArt) -> Preset {
        Preset::parse(&art.json, (art.atlas_width, art.atlas_height)).unwrap()
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
    fn a_layout_without_a_mouse_draws_only_its_keys() {
        let layout = KeyLayout {
            keys: vec![LayoutKey {
                label: "<".to_string(),
                code: 0x2C,
                x: 0.0,
                y: 0.0,
                width: 2.0,
            }],
            mouse: false,
        };
        let art = layout.art(InputDisplayStyle::Keycaps);
        let preset = read(&art);

        assert_eq!((preset.width, preset.height), (78, 38));
        assert_eq!(preset.elements.len(), 1);
        assert!(art.atlas_svg.contains("&lt;"));
    }
}
