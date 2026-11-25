use super::{
    builtin::{
        BuiltinPresetId, GamepadPart, GamepadParts, KeyLayout, LayoutKey, LayoutMouse, MOUSE_BLOCK,
        MousePart, MouseParts, PRESET_FULL, PRESET_OPACITY_RANGE, PRESET_SIZE_RANGE,
    },
    keys::key_label,
};

pub const MAKER_COLUMNS: u32 = 18;
pub const MAKER_ROWS: u32 = 5;
pub const MAKER_WIDTHS: [f64; 4] = [1.0, 1.5, 2.0, 4.0];
pub const MAKER_LABEL_LENGTH: usize = 12;
pub const MAKER_NAME_LENGTH: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MakerStart {
    WasdMouse,
    FullKeyboard,
    Gamepad,
    Empty,
}

impl MakerStart {
    pub const ALL: [Self; 4] = [
        Self::WasdMouse,
        Self::FullKeyboard,
        Self::Gamepad,
        Self::Empty,
    ];

    fn layout(self) -> KeyLayout {
        match self {
            Self::WasdMouse => BuiltinPresetId::WasdMouse.layout(),
            Self::FullKeyboard => BuiltinPresetId::FullKeyboard.layout(),
            Self::Gamepad => KeyLayout {
                gamepad: Some(GamepadParts::default()),
                ..Default::default()
            },
            Self::Empty => KeyLayout::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MakerSelection {
    Key(usize),
    Mouse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardMouse {
    pub x: f64,
    pub y: f64,
    pub parts: MouseParts,
}

fn halves(value: f64) -> f64 {
    (value * 2.0).round() / 2.0
}

fn on_the_board((x, y): (f64, f64)) -> (f64, f64) {
    let within = |value: f64, cells: u32, length: f64| {
        if value.is_finite() {
            halves(value).clamp(0.0, (f64::from(cells) - length).max(0.0))
        } else {
            0.0
        }
    };
    (
        within(x, MAKER_COLUMNS, MOUSE_BLOCK.0),
        within(y, MAKER_ROWS, MOUSE_BLOCK.1),
    )
}

fn beside(keys: &[LayoutKey]) -> (f64, f64) {
    let right = keys.iter().map(|key| key.x + key.width).fold(0.0, f64::max);
    let bottom = keys.iter().map(|key| key.y + 1.0).fold(0.0, f64::max);
    let gap = if keys.is_empty() { 0.0 } else { 0.5 };
    on_the_board((right + gap, (bottom - MOUSE_BLOCK.1) / 2.0))
}

#[derive(Clone, Debug, PartialEq)]
pub struct PresetMaker {
    pub name: String,
    pub keys: Vec<LayoutKey>,
    pub mouse: Option<BoardMouse>,
    pub gamepad: Option<GamepadParts>,
    pub selected: Option<MakerSelection>,
    pub capture: bool,
    pub start: Option<MakerStart>,
    pub size: f64,
    pub opacity: f64,
    held: Vec<u16>,
}

fn stored_percent(value: f64) -> Option<f64> {
    (value != PRESET_FULL).then_some(value)
}

fn set_percent(field: &mut f64, value: f64, (low, high): (f64, f64)) -> bool {
    if !value.is_finite() {
        return false;
    }
    let value = value.round().clamp(low, high);
    if *field == value {
        return false;
    }
    *field = value;
    true
}

impl PresetMaker {
    pub fn starting(start: MakerStart, name: impl Into<String>) -> Self {
        Self {
            capture: start == MakerStart::Empty,
            start: Some(start),
            ..Self::editing(name, &start.layout())
        }
    }

    pub fn editing(name: impl Into<String>, layout: &KeyLayout) -> Self {
        let layout = layout.clone().fitted();
        Self {
            name: name.into(),
            mouse: layout.mouse.map(|mouse| {
                let (x, y) = mouse.at.unwrap_or_else(|| beside(&layout.keys));
                BoardMouse {
                    x,
                    y,
                    parts: mouse.parts,
                }
            }),
            size: layout.size_percent(),
            opacity: layout.opacity_percent(),
            keys: layout.keys,
            gamepad: layout.gamepad,
            selected: None,
            capture: false,
            start: None,
            held: Vec::new(),
        }
    }

    pub fn layout(&self) -> KeyLayout {
        if self.gamepad.is_some() {
            return KeyLayout {
                gamepad: self.gamepad,
                size: stored_percent(self.size),
                opacity: stored_percent(self.opacity),
                ..Default::default()
            };
        }
        let corner = |of: fn(&LayoutKey) -> f64, mouse: Option<f64>| {
            self.keys
                .iter()
                .map(of)
                .chain(mouse)
                .fold(f64::MAX, f64::min)
        };
        let left = corner(|key| key.x, self.mouse.map(|mouse| mouse.x));
        let top = corner(|key| key.y, self.mouse.map(|mouse| mouse.y));
        KeyLayout {
            keys: self
                .keys
                .iter()
                .map(|key| LayoutKey {
                    x: key.x - left,
                    y: key.y - top,
                    ..key.clone()
                })
                .collect(),
            mouse: self.mouse.map(|mouse| LayoutMouse {
                at: Some((mouse.x - left, mouse.y - top)),
                parts: mouse.parts,
            }),
            gamepad: None,
            size: stored_percent(self.size),
            opacity: stored_percent(self.opacity),
        }
    }

    pub fn set_size(&mut self, percent: f64) -> bool {
        set_percent(&mut self.size, percent, PRESET_SIZE_RANGE)
    }

    pub fn set_opacity(&mut self, percent: f64) -> bool {
        set_percent(&mut self.opacity, percent, PRESET_OPACITY_RANGE)
    }

    pub fn can_save(&self) -> bool {
        !self.name.trim().is_empty()
            && (self.gamepad.is_some() || self.mouse.is_some() || !self.keys.is_empty())
    }

    pub fn set_gamepad_part(&mut self, part: GamepadPart, shown: bool) -> bool {
        let Some(parts) = &mut self.gamepad else {
            return false;
        };
        if parts.shows(part) == shown {
            return false;
        }
        parts.set(part, shown);
        true
    }

    pub fn toggle_mouse(&mut self) {
        if self.gamepad.is_some() {
            return;
        }
        if self.mouse.take().is_some() {
            if self.selected == Some(MakerSelection::Mouse) {
                self.selected = None;
            }
            return;
        }
        let (x, y) = beside(&self.keys);
        self.mouse = Some(BoardMouse {
            x,
            y,
            parts: MouseParts::default(),
        });
        self.selected = Some(MakerSelection::Mouse);
    }

    pub fn move_mouse(&mut self, x: f64, y: f64) -> bool {
        let Some(mouse) = &mut self.mouse else {
            return false;
        };
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let place = on_the_board((x, y));
        if (mouse.x, mouse.y) == place {
            return false;
        }
        (mouse.x, mouse.y) = place;
        true
    }

    pub fn set_mouse_part(&mut self, part: MousePart, shown: bool) -> bool {
        let Some(mouse) = &mut self.mouse else {
            return false;
        };
        if mouse.parts.shows(part) == shown {
            return false;
        }
        mouse.parts.set(part, shown);
        true
    }

    pub fn selected_key(&self) -> Option<&LayoutKey> {
        match self.selected? {
            MakerSelection::Key(index) => self.keys.get(index),
            MakerSelection::Mouse => None,
        }
    }

    fn selected_key_mut(&mut self) -> Option<&mut LayoutKey> {
        match self.selected? {
            MakerSelection::Key(index) => self.keys.get_mut(index),
            MakerSelection::Mouse => None,
        }
    }

    pub fn held(&self) -> &[u16] {
        &self.held
    }

    pub fn is_held(&self, code: u16) -> bool {
        self.held.contains(&code)
    }

    pub fn free_slot(&self) -> (f64, f64) {
        for row in 0..MAKER_ROWS {
            for column in 0..MAKER_COLUMNS {
                let (x, y) = (f64::from(column), f64::from(row));
                let under_a_key = self
                    .keys
                    .iter()
                    .any(|key| key.y == y && x < key.x + key.width && x + 1.0 > key.x);
                let under_the_mouse = self.mouse.is_some_and(|mouse| {
                    x < mouse.x + MOUSE_BLOCK.0
                        && x + 1.0 > mouse.x
                        && y < mouse.y + MOUSE_BLOCK.1
                        && y + 1.0 > mouse.y
                });
                if !under_a_key && !under_the_mouse {
                    return (x, y);
                }
            }
        }
        (0.0, 0.0)
    }

    pub fn key_down(&mut self, code: u16) -> bool {
        if !self.held.contains(&code) {
            self.held.push(code);
        }
        let adds_keys = self.capture && self.gamepad.is_none();
        if !adds_keys || self.keys.iter().any(|key| key.code == code) {
            return false;
        }
        let (x, y) = self.free_slot();
        self.keys.push(LayoutKey {
            label: key_label(code),
            code,
            x,
            y,
            width: 1.0,
        });
        self.selected = Some(MakerSelection::Key(self.keys.len() - 1));
        true
    }

    pub fn key_up(&mut self, code: u16) {
        self.held.retain(|held| *held != code);
    }

    pub fn release_all(&mut self) {
        self.held.clear();
    }

    pub fn select(&mut self, selection: Option<MakerSelection>) {
        self.selected = selection.filter(|selection| match selection {
            MakerSelection::Key(index) => *index < self.keys.len(),
            MakerSelection::Mouse => self.mouse.is_some(),
        });
    }

    pub fn move_key(&mut self, index: usize, x: f64, y: f64) -> bool {
        let Some(key) = self.keys.get_mut(index) else {
            return false;
        };
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let x = halves(x).clamp(0.0, (f64::from(MAKER_COLUMNS) - key.width).max(0.0));
        let y = y.round().clamp(0.0, f64::from(MAKER_ROWS - 1));
        if (key.x, key.y) == (x, y) {
            return false;
        }
        (key.x, key.y) = (x, y);
        true
    }

    pub fn set_label(&mut self, label: &str) -> bool {
        let Some(key) = self.selected_key_mut() else {
            return false;
        };
        let label: String = label.chars().take(MAKER_LABEL_LENGTH).collect();
        if key.label == label {
            return false;
        }
        key.label = label;
        true
    }

    pub fn set_width(&mut self, width: f64) -> bool {
        let Some(key) = self.selected_key_mut() else {
            return false;
        };
        if !MAKER_WIDTHS.contains(&width) || key.width == width {
            return false;
        }
        key.width = width;
        key.x = key.x.min(f64::from(MAKER_COLUMNS) - width);
        true
    }

    pub fn delete_selected(&mut self) -> bool {
        match self.selected.take() {
            Some(MakerSelection::Key(index)) if index < self.keys.len() => {
                self.keys.remove(index);
                true
            }
            Some(MakerSelection::Mouse) => self.mouse.take().is_some(),
            _ => false,
        }
    }

    pub fn clear(&mut self) -> bool {
        if self.keys.is_empty() {
            return false;
        }
        self.keys.clear();
        if self.selected != Some(MakerSelection::Mouse) {
            self.selected = None;
        }
        true
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.chars().take(MAKER_NAME_LENGTH).collect();
    }
}

impl KeyLayout {
    pub fn fitted(mut self) -> Self {
        if self.gamepad.is_some() {
            self.keys.clear();
            self.mouse = None;
            return self;
        }
        let columns = f64::from(MAKER_COLUMNS);
        self.keys
            .retain(|key| key.x.is_finite() && key.y.is_finite() && key.width.is_finite());
        self.keys
            .truncate((MAKER_COLUMNS * MAKER_ROWS * 2) as usize);
        for key in &mut self.keys {
            key.width = key.width.clamp(0.5, columns);
            key.x = key.x.clamp(0.0, columns - key.width);
            key.y = key.y.round().clamp(0.0, f64::from(MAKER_ROWS - 1));
            if key.label.chars().count() > MAKER_LABEL_LENGTH {
                key.label = key.label.chars().take(MAKER_LABEL_LENGTH).collect();
            }
        }
        if let Some(LayoutMouse { at: Some(at), .. }) = &mut self.mouse {
            *at = on_the_board(*at);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_display::{
        InputDisplayStyle, Preset,
        key_codes::{CONTROL_LEFT, SPACE, UP, letter},
    };

    fn maker() -> PresetMaker {
        PresetMaker::starting(MakerStart::WasdMouse, "Mine")
    }

    fn capturing() -> PresetMaker {
        let mut maker = maker();
        maker.capture = true;
        maker
    }

    #[test]
    fn a_new_preset_starts_from_the_built_in_wasd_and_mouse() {
        let maker = maker();

        assert_eq!(
            maker.layout().keys,
            BuiltinPresetId::WasdMouse.layout().keys
        );
        assert_eq!(
            maker.mouse,
            Some(BoardMouse {
                x: 6.0,
                y: 0.0,
                parts: MouseParts::default()
            })
        );
        assert_eq!(maker.start, Some(MakerStart::WasdMouse));
        assert!(!maker.capture);
        assert_eq!(maker.selected, None);
        assert!(maker.can_save());
    }

    #[test]
    fn a_pressed_key_lands_in_the_first_free_slot_and_is_selected() {
        let mut maker = capturing();

        assert!(maker.key_down(UP));

        let added = maker.selected_key().unwrap();
        assert_eq!((added.x, added.y, added.width), (9.0, 0.0, 1.0));
        assert_eq!((added.code, added.label.as_str()), (UP, "\u{2191}"));
        assert_eq!(maker.keys.len(), 13);
    }

    #[test]
    fn a_key_already_on_the_board_is_not_added_twice() {
        let mut maker = capturing();

        assert!(!maker.key_down(letter('W')));
        assert!(maker.key_down(letter('G')));
        maker.key_up(letter('G'));
        assert!(!maker.key_down(letter('G')));

        assert_eq!(maker.keys.len(), 13);
    }

    #[test]
    fn keys_are_only_added_while_press_keys_to_add_is_on() {
        let mut maker = maker();

        assert!(!maker.key_down(UP));

        assert_eq!(maker.keys.len(), 12);
        assert!(maker.is_held(UP));
    }

    #[test]
    fn held_keys_light_until_they_are_released() {
        let mut maker = maker();
        maker.key_down(letter('W'));
        maker.key_down(letter('W'));
        maker.key_down(SPACE);

        assert!(maker.is_held(letter('W')) && maker.is_held(SPACE));
        assert_eq!(maker.held(), [letter('W'), SPACE]);
        maker.key_up(letter('W'));
        assert!(!maker.is_held(letter('W')) && maker.is_held(SPACE));
        maker.release_all();
        assert!(!maker.is_held(SPACE));
    }

    #[test]
    fn the_first_free_slot_skips_cells_under_wide_keys() {
        let mut maker = capturing();
        maker.clear();
        maker.toggle_mouse();

        assert_eq!(maker.free_slot(), (0.0, 0.0));
        maker.key_down(SPACE);
        maker.set_width(4.0);
        assert_eq!(maker.free_slot(), (4.0, 0.0));
        maker.move_key(0, 0.5, 0.0);
        assert_eq!(maker.free_slot(), (5.0, 0.0));
    }

    #[test]
    fn a_full_board_puts_the_next_key_at_the_start() {
        let mut maker = capturing();
        maker.clear();
        maker.toggle_mouse();
        for code in 0..(MAKER_COLUMNS * MAKER_ROWS) as u16 {
            assert!(maker.key_down(1000 + code));
        }

        assert_eq!(maker.free_slot(), (0.0, 0.0));
        let last = maker.keys.last().unwrap();
        assert_eq!((last.x, last.y), (17.0, 4.0));
    }

    #[test]
    fn a_dragged_key_snaps_to_half_columns_and_rows_and_stays_on_the_board() {
        let mut maker = maker();
        let space = maker.keys.iter().position(|key| key.code == SPACE).unwrap();

        assert!(maker.move_key(space, 3.3, 3.6));
        assert_eq!((maker.keys[space].x, maker.keys[space].y), (3.5, 4.0));
        assert!(maker.move_key(space, 40.0, -9.0));
        assert_eq!((maker.keys[space].x, maker.keys[space].y), (14.0, 0.0));
        assert!(!maker.move_key(space, 14.1, 0.2));
        assert!(!maker.move_key(99, 1.0, 1.0));
        assert!(!maker.move_key(space, f64::NAN, 1.0));
    }

    #[test]
    fn the_selected_key_takes_a_label_and_a_width_and_can_be_deleted() {
        let mut maker = maker();
        let ctrl = maker
            .keys
            .iter()
            .position(|key| key.code == CONTROL_LEFT)
            .unwrap();

        assert!(!maker.set_label("Crouch"));
        maker.select(Some(MakerSelection::Key(ctrl)));
        assert!(maker.set_label("Crouch, a very long label"));
        assert!(maker.set_width(2.0));
        assert!(!maker.set_width(3.0));
        assert_eq!(
            (maker.keys[ctrl].label.as_str(), maker.keys[ctrl].width),
            ("Crouch, a ve", 2.0)
        );

        assert!(maker.delete_selected());
        assert_eq!(maker.selected, None);
        assert!(maker.keys.iter().all(|key| key.code != CONTROL_LEFT));
        assert!(!maker.delete_selected());
    }

    #[test]
    fn a_widened_key_is_pulled_back_onto_the_board() {
        let mut maker = capturing();
        maker.clear();
        maker.key_down(SPACE);
        maker.move_key(0, 17.0, 2.0);

        assert!(maker.set_width(4.0));

        assert_eq!(maker.keys[0].x, 14.0);
    }

    #[test]
    fn a_preset_needs_a_name_and_something_to_show() {
        let mut maker = maker();
        maker.clear();
        assert!(maker.can_save());
        maker.toggle_mouse();
        assert!(!maker.can_save());
        maker.capture = true;
        maker.key_down(SPACE);
        assert!(maker.can_save());
        maker.set_name("   ");
        assert!(!maker.can_save());
        maker.set_name(&"n".repeat(90));
        assert_eq!(maker.name.len(), MAKER_NAME_LENGTH);
    }

    #[test]
    fn the_saved_layout_starts_at_its_first_key_and_draws_in_every_style() {
        let mut maker = capturing();
        maker.clear();
        maker.toggle_mouse();
        maker.key_down(letter('E'));
        maker.move_key(0, 6.0, 3.0);
        maker.key_down(letter('R'));
        maker.move_key(1, 7.5, 4.0);

        let layout = maker.layout();

        assert_eq!(
            layout
                .keys
                .iter()
                .map(|key| (key.x, key.y))
                .collect::<Vec<_>>(),
            vec![(0.0, 0.0), (1.5, 1.0)]
        );
        for style in InputDisplayStyle::ALL {
            let art = layout.art(style);
            let preset = Preset::parse_own(&art.json, (art.atlas_width, art.atlas_height)).unwrap();
            assert_eq!(preset.elements.len(), 2, "{style:?}");
            assert_eq!((preset.width, preset.height), layout.size());
        }
    }

    #[test]
    fn the_maker_sets_the_size_and_opacity_a_preset_is_drawn_at() {
        let mut maker = PresetMaker::starting(MakerStart::WasdMouse, "Mine");
        assert_eq!((maker.size, maker.opacity), (100.0, 100.0));
        assert_eq!(maker.layout().size, None);
        assert_eq!(maker.layout().opacity, None);

        assert!(maker.set_size(142.4));
        assert!(maker.set_opacity(64.6));
        assert_eq!((maker.size, maker.opacity), (142.0, 65.0));
        assert!(!maker.set_size(142.0));
        assert!(maker.set_size(5.0));
        assert!(maker.set_opacity(500.0));
        assert_eq!((maker.size, maker.opacity), (PRESET_SIZE_RANGE.0, 100.0));
        assert!(!maker.set_size(f64::NAN));

        maker.set_size(150.0);
        maker.set_opacity(40.0);
        let layout = maker.layout();
        assert_eq!((layout.size, layout.opacity), (Some(150.0), Some(40.0)));
        let again = PresetMaker::editing("Mine", &layout);
        assert_eq!((again.size, again.opacity), (150.0, 40.0));

        let mut pad = PresetMaker::starting(MakerStart::Gamepad, "Pad");
        pad.set_size(80.0);
        assert_eq!(pad.layout().size, Some(80.0));
        assert!(pad.layout().gamepad.is_some());
    }

    #[test]
    fn a_layout_read_from_disk_is_fitted_to_the_board() {
        let layout = KeyLayout {
            keys: vec![
                LayoutKey {
                    label: "x".repeat(50),
                    code: 1,
                    x: 900.0,
                    y: -3.0,
                    width: 80.0,
                },
                LayoutKey {
                    label: String::new(),
                    code: 2,
                    x: f64::NAN,
                    y: 0.0,
                    width: 1.0,
                },
            ],
            mouse: Some(LayoutMouse {
                at: Some((400.0, f64::NAN)),
                parts: MouseParts::default(),
            }),
            ..Default::default()
        }
        .fitted();

        assert_eq!(layout.keys.len(), 1);
        let key = &layout.keys[0];
        assert_eq!((key.x, key.y, key.width), (0.0, 0.0, 18.0));
        assert_eq!(layout.mouse.unwrap().at, Some((15.5, 0.0)));
        assert_eq!(key.label.len(), MAKER_LABEL_LENGTH);
    }

    #[test]
    fn a_new_preset_starts_from_the_full_keyboard_or_from_nothing() {
        let full = PresetMaker::starting(MakerStart::FullKeyboard, "Mine");
        assert_eq!(
            full.layout().keys,
            BuiltinPresetId::FullKeyboard.layout().keys
        );
        let mouse = full.mouse.unwrap();
        assert_eq!((mouse.x, mouse.y), (15.5, 0.5));
        assert!(!full.capture);
        for key in &full.keys {
            assert!(key.x + key.width <= f64::from(MAKER_COLUMNS), "{key:?}");
        }

        let empty = PresetMaker::starting(MakerStart::Empty, "Mine");
        assert!(empty.keys.is_empty() && empty.mouse.is_none());
        assert!(empty.capture);
        assert!(!empty.can_save());
        assert_eq!(PresetMaker::editing("Mine", &full.layout()).start, None);
    }

    #[test]
    fn the_mouse_is_dragged_in_half_steps_and_stays_on_the_board() {
        let mut maker = maker();

        assert!(maker.move_mouse(8.3, 0.8));
        let mouse = maker.mouse.unwrap();
        assert_eq!((mouse.x, mouse.y), (8.5, 1.0));
        assert!(maker.move_mouse(90.0, 90.0));
        let mouse = maker.mouse.unwrap();
        assert_eq!((mouse.x, mouse.y), (15.5, 1.25));
        assert!(maker.move_mouse(-3.0, -3.0));
        assert!(!maker.move_mouse(0.1, 0.1));
        assert!(!maker.move_mouse(f64::NAN, 0.0));
        maker.toggle_mouse();
        assert!(!maker.move_mouse(3.0, 1.0));
    }

    #[test]
    fn the_mouse_switch_adds_the_block_beside_the_keys_and_selects_it() {
        let mut maker = maker();
        maker.select(Some(MakerSelection::Mouse));
        maker.toggle_mouse();
        assert_eq!((maker.mouse, maker.selected), (None, None));
        maker.select(Some(MakerSelection::Mouse));
        assert_eq!(maker.selected, None);

        maker.toggle_mouse();
        let mouse = maker.mouse.unwrap();
        assert_eq!((mouse.x, mouse.y), (6.0, 0.0));
        assert_eq!(maker.selected, Some(MakerSelection::Mouse));
        assert_eq!(maker.selected_key(), None);
        assert!(!maker.set_label("x") && !maker.set_width(2.0));

        assert!(maker.delete_selected());
        assert_eq!((maker.mouse, maker.selected), (None, None));
    }

    #[test]
    fn the_mouse_shows_only_the_parts_left_on() {
        let mut maker = maker();

        assert!(maker.set_mouse_part(MousePart::Side, false));
        assert!(!maker.set_mouse_part(MousePart::Side, false));
        assert!(maker.set_mouse_part(MousePart::Wheel, false));

        let saved = maker.layout().mouse.unwrap();
        assert_eq!(
            saved.parts,
            MouseParts {
                wheel: false,
                side: false,
                ..Default::default()
            }
        );
        assert_eq!(
            PresetMaker::editing("Mine", &maker.layout()).mouse,
            maker.mouse
        );
    }

    #[test]
    fn the_saved_layout_starts_at_whatever_is_furthest_up_and_left() {
        let mut maker = maker();
        for index in 0..maker.keys.len() {
            let key = maker.keys[index].clone();
            maker.move_key(index, key.x + 4.0, key.y + 2.0);
        }
        maker.move_mouse(1.0, 1.0);

        let layout = maker.layout();

        assert_eq!(layout.mouse.unwrap().at, Some((0.0, 0.0)));
        assert_eq!((layout.keys[0].x, layout.keys[0].y), (3.0, 1.0));
        let again = PresetMaker::editing("Mine", &layout).layout();
        assert_eq!(again, layout);
    }

    #[test]
    fn a_layout_saved_by_1_0_0_opens_with_its_mouse_beside_the_keys() {
        let saved: KeyLayout = serde_json::from_value(serde_json::json!({
            "keys": [{ "label": "W", "code": 87, "x": 0.0, "y": 0.0, "width": 1.0 }],
            "mouse": true
        }))
        .unwrap();

        let maker = PresetMaker::editing("Old", &saved);

        let mouse = maker.mouse.unwrap();
        assert_eq!((mouse.x, mouse.y), (1.5, 0.0));
        assert_eq!(mouse.parts, MouseParts::default());
    }

    #[test]
    fn a_gamepad_preset_starts_as_the_whole_pad_and_holds_nothing_else() {
        let mut maker = PresetMaker::starting(MakerStart::Gamepad, "Pad");

        assert_eq!(maker.gamepad, Some(GamepadParts::default()));
        assert!(maker.keys.is_empty() && maker.mouse.is_none());
        assert!(!maker.capture && maker.can_save());
        maker.capture = true;
        assert!(!maker.key_down(letter('W')));
        maker.toggle_mouse();
        assert!(maker.keys.is_empty() && maker.mouse.is_none());

        assert!(maker.set_gamepad_part(GamepadPart::Triggers, false));
        assert!(!maker.set_gamepad_part(GamepadPart::Triggers, false));
        let layout = maker.layout();
        assert_eq!(
            layout,
            KeyLayout {
                gamepad: Some(GamepadParts {
                    triggers: false,
                    ..Default::default()
                }),
                ..Default::default()
            }
        );
        let reopened = PresetMaker::editing("Pad", &layout);
        assert_eq!(reopened.gamepad, maker.gamepad);
        assert_eq!(reopened.layout(), layout);
        assert!(!self::maker().set_gamepad_part(GamepadPart::Sticks, false));
    }

    #[test]
    fn a_gamepad_layout_that_also_lists_keys_keeps_only_the_gamepad() {
        let layout = KeyLayout {
            gamepad: Some(GamepadParts::default()),
            ..BuiltinPresetId::WasdMouse.layout()
        }
        .fitted();

        assert!(layout.keys.is_empty() && layout.mouse.is_none());
        assert!(layout.gamepad.is_some());
    }
}
