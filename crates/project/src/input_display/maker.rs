use super::{
    builtin::{BuiltinPresetId, KeyLayout, LayoutKey},
    keys::key_label,
};

pub const MAKER_COLUMNS: u32 = 14;
pub const MAKER_ROWS: u32 = 5;
pub const MAKER_WIDTHS: [f64; 4] = [1.0, 1.5, 2.0, 4.0];
pub const MAKER_LABEL_LENGTH: usize = 12;
pub const MAKER_NAME_LENGTH: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub struct PresetMaker {
    pub name: String,
    pub keys: Vec<LayoutKey>,
    pub mouse: bool,
    pub selected: Option<usize>,
    pub capture: bool,
    held: Vec<u16>,
}

impl PresetMaker {
    pub fn starting_from(preset: BuiltinPresetId, name: impl Into<String>) -> Self {
        Self::editing(name, &preset.layout())
    }

    pub fn editing(name: impl Into<String>, layout: &KeyLayout) -> Self {
        let layout = layout.clone().fitted();
        Self {
            name: name.into(),
            keys: layout.keys,
            mouse: layout.mouse,
            selected: None,
            capture: false,
            held: Vec::new(),
        }
    }

    pub fn layout(&self) -> KeyLayout {
        let left = self.keys.iter().map(|key| key.x).fold(f64::MAX, f64::min);
        let top = self.keys.iter().map(|key| key.y).fold(f64::MAX, f64::min);
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
            mouse: self.mouse,
        }
    }

    pub fn can_save(&self) -> bool {
        !self.name.trim().is_empty() && (self.mouse || !self.keys.is_empty())
    }

    pub fn selected_key(&self) -> Option<&LayoutKey> {
        self.keys.get(self.selected?)
    }

    pub fn is_held(&self, code: u16) -> bool {
        self.held.contains(&code)
    }

    pub fn free_slot(&self) -> (f64, f64) {
        for row in 0..MAKER_ROWS {
            for column in 0..MAKER_COLUMNS {
                let (x, y) = (f64::from(column), f64::from(row));
                let taken = self
                    .keys
                    .iter()
                    .any(|key| key.y == y && x < key.x + key.width && x + 1.0 > key.x);
                if !taken {
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
        if !self.capture || self.keys.iter().any(|key| key.code == code) {
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
        self.selected = Some(self.keys.len() - 1);
        true
    }

    pub fn key_up(&mut self, code: u16) {
        self.held.retain(|held| *held != code);
    }

    pub fn release_all(&mut self) {
        self.held.clear();
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|index| *index < self.keys.len());
    }

    pub fn move_key(&mut self, index: usize, x: f64, y: f64) -> bool {
        let Some(key) = self.keys.get_mut(index) else {
            return false;
        };
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let x =
            ((x * 2.0).round() / 2.0).clamp(0.0, (f64::from(MAKER_COLUMNS) - key.width).max(0.0));
        let y = y.round().clamp(0.0, f64::from(MAKER_ROWS - 1));
        if (key.x, key.y) == (x, y) {
            return false;
        }
        (key.x, key.y) = (x, y);
        true
    }

    pub fn set_label(&mut self, label: &str) -> bool {
        let Some(key) = self.selected.and_then(|index| self.keys.get_mut(index)) else {
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
        let Some(key) = self.selected.and_then(|index| self.keys.get_mut(index)) else {
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
        let Some(index) = self.selected.filter(|index| *index < self.keys.len()) else {
            return false;
        };
        self.keys.remove(index);
        self.selected = None;
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.keys.is_empty() {
            return false;
        }
        self.keys.clear();
        self.selected = None;
        true
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.chars().take(MAKER_NAME_LENGTH).collect();
    }
}

impl KeyLayout {
    pub fn fitted(mut self) -> Self {
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
        PresetMaker::starting_from(BuiltinPresetId::WasdMouse, "Mine")
    }

    fn capturing() -> PresetMaker {
        let mut maker = maker();
        maker.capture = true;
        maker
    }

    #[test]
    fn a_new_preset_starts_from_the_built_in_wasd_and_mouse() {
        let maker = maker();

        assert_eq!(maker.layout(), BuiltinPresetId::WasdMouse.layout());
        assert!(maker.mouse);
        assert!(!maker.capture);
        assert_eq!(maker.selected, None);
        assert!(maker.can_save());
    }

    #[test]
    fn a_pressed_key_lands_in_the_first_free_slot_and_is_selected() {
        let mut maker = capturing();

        assert!(maker.key_down(UP));

        let added = maker.selected_key().unwrap();
        assert_eq!((added.x, added.y, added.width), (6.0, 0.0, 1.0));
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
        maker.key_up(letter('W'));
        assert!(!maker.is_held(letter('W')) && maker.is_held(SPACE));
        maker.release_all();
        assert!(!maker.is_held(SPACE));
    }

    #[test]
    fn the_first_free_slot_skips_cells_under_wide_keys() {
        let mut maker = capturing();
        maker.clear();

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
        for code in 0..(MAKER_COLUMNS * MAKER_ROWS) as u16 {
            assert!(maker.key_down(1000 + code));
        }

        assert_eq!(maker.free_slot(), (0.0, 0.0));
        let last = maker.keys.last().unwrap();
        assert_eq!((last.x, last.y), (13.0, 4.0));
    }

    #[test]
    fn a_dragged_key_snaps_to_half_columns_and_rows_and_stays_on_the_board() {
        let mut maker = maker();
        let space = maker.keys.iter().position(|key| key.code == SPACE).unwrap();

        assert!(maker.move_key(space, 3.3, 3.6));
        assert_eq!((maker.keys[space].x, maker.keys[space].y), (3.5, 4.0));
        assert!(maker.move_key(space, 40.0, -9.0));
        assert_eq!((maker.keys[space].x, maker.keys[space].y), (10.0, 0.0));
        assert!(!maker.move_key(space, 10.1, 0.2));
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
        maker.select(Some(ctrl));
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
        maker.move_key(0, 13.0, 2.0);

        assert!(maker.set_width(4.0));

        assert_eq!(maker.keys[0].x, 10.0);
    }

    #[test]
    fn a_preset_needs_a_name_and_something_to_show() {
        let mut maker = maker();
        maker.clear();
        assert!(maker.can_save());
        maker.mouse = false;
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
        maker.mouse = false;
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
            mouse: false,
        }
        .fitted();

        assert_eq!(layout.keys.len(), 1);
        let key = &layout.keys[0];
        assert_eq!((key.x, key.y, key.width), (0.0, 0.0, 14.0));
        assert_eq!(key.label.len(), MAKER_LABEL_LENGTH);
    }
}
