use std::collections::HashMap;

use super::{
    folders::LibraryFile,
    rules,
    view::{Clock, Entry},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    All,
    Pinned,
    Folder(String),
    Smart(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Places {
    pinned: Vec<usize>,
    folders: HashMap<String, Vec<usize>>,
    smart: HashMap<String, Vec<usize>>,
}

impl Places {
    pub fn work_out(file: &LibraryFile, entries: &[Entry], clock: &Clock) -> Self {
        let by_id: HashMap<&str, usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| Some((entry.id.as_deref()?, index)))
            .collect();
        let present = |ids: &[String]| {
            let mut indexes: Vec<usize> = ids
                .iter()
                .filter_map(|id| by_id.get(id.as_str()).copied())
                .collect();
            indexes.sort_unstable();
            indexes.dedup();
            indexes
        };
        Self {
            pinned: present(&file.pins),
            folders: file
                .folders
                .iter()
                .map(|folder| (folder.id.clone(), present(&folder.members)))
                .collect(),
            smart: rules::evaluate_all(&file.smart_folders, entries, clock),
        }
    }

    pub fn members(&self, place: &Place) -> Option<&[usize]> {
        match place {
            Place::All => None,
            Place::Pinned => Some(&self.pinned),
            Place::Folder(id) => Some(self.folders.get(id).map_or(&[], Vec::as_slice)),
            Place::Smart(id) => Some(self.smart.get(id).map_or(&[], Vec::as_slice)),
        }
    }

    pub fn count(&self, place: &Place, total: usize) -> usize {
        self.members(place).map_or(total, <[usize]>::len)
    }

    pub fn exists(&self, place: &Place) -> bool {
        match place {
            Place::All | Place::Pinned => true,
            Place::Folder(id) => self.folders.contains_key(id),
            Place::Smart(id) => self.smart.contains_key(id),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::{FixedOffset, NaiveDate};

    use super::*;
    use crate::library::{
        MediaKind,
        rules::{KindValue, Matching, Rule, SmartFolder},
        view::Compression,
    };

    fn clock() -> Clock {
        Clock {
            today: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            offset: FixedOffset::east_opt(0).unwrap(),
        }
    }

    fn entry(name: &str, kind: MediaKind, id: Option<&str>) -> Entry {
        Entry {
            path: PathBuf::from(format!("{name}.ever")),
            kind,
            name: name.to_string(),
            size: 0,
            duration: None,
            created_millis: 0.,
            exports: Vec::new(),
            compression: Compression::None,
            compressible: false,
            clip_count: 1,
            thumbnail: None,
            thumbnail_version: None,
            screenshot: None,
            opens_editor: true,
            audio_only: false,
            id: id.map(str::to_owned),
            rebuilt: None,
        }
    }

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn folders_and_pins_hold_what_the_scan_found_and_skip_what_is_gone() {
        let entries = vec![
            entry("Demo", MediaKind::Studio, Some("a")),
            entry("Bug", MediaKind::Instant, Some("b")),
            entry("Unkeyed", MediaKind::Studio, None),
        ];
        let mut file = LibraryFile::default();
        let launch = file.new_folder("Launch", &ids(&["gone", "b", "a"]));
        file.toggle_pins(&ids(&["gone", "b"]));
        let places = Places::work_out(&file, &entries, &clock());
        assert_eq!(places.members(&Place::Folder(launch)), Some(&[0, 1][..]));
        assert_eq!(places.members(&Place::Pinned), Some(&[1][..]));
        assert_eq!(places.members(&Place::All), None);
        assert_eq!(places.count(&Place::All, entries.len()), 3);
        assert_eq!(
            places.members(&Place::Folder("deleted".into())),
            Some(&[][..])
        );
        assert!(!places.exists(&Place::Folder("deleted".into())));
    }

    #[test]
    fn a_renamed_project_keeps_its_folders_because_they_follow_its_id() {
        let mut file = LibraryFile::default();
        let launch = file.new_folder("Launch", &ids(&["a"]));
        let before = vec![entry("Demo", MediaKind::Studio, Some("a"))];
        let after = vec![Entry {
            path: PathBuf::from("Elsewhere/Renamed.ever"),
            name: "Renamed".into(),
            ..before[0].clone()
        }];
        let place = Place::Folder(launch);
        assert_eq!(
            Places::work_out(&file, &before, &clock()).count(&place, 1),
            1
        );
        assert_eq!(
            Places::work_out(&file, &after, &clock()).count(&place, 1),
            1
        );
    }

    #[test]
    fn smart_folders_fill_themselves_from_their_rules() {
        let entries = vec![
            entry("Demo", MediaKind::Studio, None),
            entry("Shot", MediaKind::Screenshot, None),
        ];
        let mut file = LibraryFile::default();
        let shots = file.save_smart(SmartFolder {
            id: String::new(),
            name: "Screenshots".into(),
            matching: Matching::All,
            rules: vec![Rule::Kind {
                op: rules::IsOp::Is,
                value: KindValue::Screenshot,
            }],
            extra: Default::default(),
        });
        let places = Places::work_out(&file, &entries, &clock());
        assert_eq!(places.members(&Place::Smart(shots)), Some(&[1][..]));
    }
}
