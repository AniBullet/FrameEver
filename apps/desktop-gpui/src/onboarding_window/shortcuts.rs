use serde_json::{Map, Value};

use crate::{settings_pages, store::Hotkey};

pub(super) const RECORD: &str = "openRecordingPicker";
pub(super) const SCREENSHOT: &str = "screenshotArea";
const ACTIONS: [&str; 2] = [RECORD, SCREENSHOT];

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Note {
    Taken(Hotkey),
    MovedFrom(&'static str),
    Rejected(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Row {
    pub action: &'static str,
    pub binding: Option<Hotkey>,
    pub note: Option<Note>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Leave {
    Continue,
    Back,
    Skip,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Draft {
    pub rows: [Row; 2],
    pub listening: Option<usize>,
}

fn alt_shift(code: &str) -> Hotkey {
    Hotkey {
        code: code.into(),
        meta: false,
        ctrl: false,
        alt: true,
        shift: true,
    }
}

fn suggestion(action: &str) -> Hotkey {
    alt_shift(if action == RECORD { "KeyR" } else { "KeyS" })
}

fn stored(hotkeys: &Map<String, Value>, action: &str) -> Option<Hotkey> {
    hotkeys
        .get(action)
        .and_then(crate::store::hotkey_from_value)
}

impl Draft {
    pub fn new(hotkeys: &Map<String, Value>, suggest: bool) -> Self {
        let row = |action: &'static str| Row {
            action,
            binding: stored(hotkeys, action).or_else(|| suggest.then(|| suggestion(action))),
            note: None,
        };
        Self {
            rows: ACTIONS.map(row),
            listening: None,
        }
    }

    pub fn set(&mut self, index: usize, hotkey: Hotkey, hotkeys: &Map<String, Value>) {
        let mut others = hotkeys.clone();
        for action in ACTIONS {
            others.remove(action);
        }
        let mut moved_from =
            settings_pages::assign_shortcut(&mut others, self.rows[index].action, &hotkey);
        for (other, row) in self.rows.iter_mut().enumerate() {
            if other != index && row.binding.as_ref() == Some(&hotkey) {
                row.binding = None;
                row.note = None;
                moved_from.get_or_insert(row.action);
            }
        }
        let row = &mut self.rows[index];
        row.binding = Some(hotkey);
        row.note = moved_from.map(Note::MovedFrom);
        self.listening = None;
    }

    pub fn reject(&mut self, index: usize, reason: &'static str) {
        self.rows[index].note = Some(Note::Rejected(reason));
    }

    pub fn clear(&mut self, index: usize) {
        let row = &mut self.rows[index];
        row.binding = None;
        row.note = None;
        self.listening = None;
    }

    pub fn leave(&mut self, leave: Leave, hotkeys: &mut Map<String, Value>) -> bool {
        self.listening = None;
        if leave != Leave::Continue {
            return false;
        }
        for row in &mut self.rows {
            match &row.binding {
                Some(binding) => {
                    settings_pages::assign_shortcut(hotkeys, row.action, binding);
                }
                None => {
                    hotkeys.remove(row.action);
                }
            }
            if !matches!(row.note, Some(Note::Taken(_))) {
                row.note = None;
            }
        }
        true
    }

    pub fn mark_taken(&mut self, unavailable: &[&str], hotkeys: &mut Map<String, Value>) -> bool {
        let mut changed = false;
        for row in &mut self.rows {
            if !unavailable.contains(&row.action) {
                continue;
            }
            if let Some(binding) = row.binding.take() {
                row.note = Some(Note::Taken(binding));
                hotkeys.remove(row.action);
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl_shift(code: &str) -> Hotkey {
        Hotkey {
            code: code.into(),
            meta: false,
            ctrl: true,
            alt: false,
            shift: true,
        }
    }

    fn map(entries: &[(&str, Hotkey)]) -> Map<String, Value> {
        entries
            .iter()
            .map(|(action, hotkey)| (action.to_string(), serde_json::to_value(hotkey).unwrap()))
            .collect()
    }

    #[test]
    fn first_run_suggests_alt_shift_r_and_s_and_a_revisit_shows_only_what_is_set() {
        let stored = map(&[(SCREENSHOT, ctrl_shift("Digit4"))]);
        let first = Draft::new(&Map::new(), true);
        assert_eq!(first.rows[0].binding, Some(alt_shift("KeyR")));
        assert_eq!(first.rows[1].binding, Some(alt_shift("KeyS")));
        let again = Draft::new(&stored, false);
        assert_eq!(again.rows[0].binding, None);
        assert_eq!(again.rows[1].binding, Some(ctrl_shift("Digit4")));
    }

    #[test]
    fn only_continue_writes_the_shortcuts() {
        for leave in [Leave::Back, Leave::Skip] {
            let mut hotkeys = map(&[("stopRecording", ctrl_shift("KeyX"))]);
            let before = hotkeys.clone();
            assert!(!Draft::new(&hotkeys, true).leave(leave, &mut hotkeys));
            assert_eq!(hotkeys, before);
        }
        let mut hotkeys = map(&[(SCREENSHOT, ctrl_shift("Digit4"))]);
        let mut draft = Draft::new(&hotkeys, true);
        draft.clear(1);
        assert!(draft.leave(Leave::Continue, &mut hotkeys));
        assert_eq!(stored(&hotkeys, RECORD), Some(alt_shift("KeyR")));
        assert_eq!(stored(&hotkeys, SCREENSHOT), None);
    }

    #[test]
    fn back_keeps_the_edits_for_a_later_continue() {
        let mut hotkeys = Map::new();
        let mut draft = Draft::new(&hotkeys, true);
        draft.set(0, ctrl_shift("KeyR"), &hotkeys);
        assert!(!draft.leave(Leave::Back, &mut hotkeys));
        assert!(hotkeys.is_empty());
        assert!(draft.leave(Leave::Continue, &mut hotkeys));
        assert_eq!(stored(&hotkeys, RECORD), Some(ctrl_shift("KeyR")));
    }

    #[test]
    fn a_key_another_frameever_action_holds_moves_here_with_a_note() {
        let mut hotkeys = map(&[("stopRecording", ctrl_shift("KeyX"))]);
        let mut draft = Draft::new(&hotkeys, true);
        draft.set(0, ctrl_shift("KeyX"), &hotkeys);
        assert_eq!(draft.rows[0].note, Some(Note::MovedFrom("stopRecording")));
        assert!(hotkeys.contains_key("stopRecording"));
        draft.leave(Leave::Continue, &mut hotkeys);
        assert!(!hotkeys.contains_key("stopRecording"));
        assert_eq!(stored(&hotkeys, RECORD), Some(ctrl_shift("KeyX")));
    }

    #[test]
    fn the_same_key_on_the_other_row_leaves_that_row_empty() {
        let hotkeys = Map::new();
        let mut draft = Draft::new(&hotkeys, true);
        draft.set(1, alt_shift("KeyR"), &hotkeys);
        assert_eq!(draft.rows[0].binding, None);
        assert_eq!(draft.rows[1].binding, Some(alt_shift("KeyR")));
        assert_eq!(draft.rows[1].note, Some(Note::MovedFrom(RECORD)));
    }

    #[test]
    fn a_key_another_program_holds_empties_its_row_and_says_so() {
        let mut hotkeys = Map::new();
        let mut draft = Draft::new(&hotkeys, true);
        draft.leave(Leave::Continue, &mut hotkeys);
        assert!(draft.mark_taken(&[RECORD], &mut hotkeys));
        assert_eq!(draft.rows[0].binding, None);
        assert_eq!(draft.rows[0].note, Some(Note::Taken(alt_shift("KeyR"))));
        assert!(!hotkeys.contains_key(RECORD));
        assert_eq!(stored(&hotkeys, SCREENSHOT), Some(alt_shift("KeyS")));
        assert!(!draft.mark_taken(&[RECORD], &mut hotkeys));
    }
}
