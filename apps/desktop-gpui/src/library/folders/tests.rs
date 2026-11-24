use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::library::rules::{Matching, Rule, TextOp};

fn scratch() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.json");
    (dir, path)
}

fn smart(id: &str) -> SmartFolder {
    SmartFolder {
        id: id.into(),
        name: format!("Folder {id}"),
        matching: Matching::All,
        rules: vec![Rule::Title {
            op: TextOp::Contains,
            value: "standup".into(),
        }],
        extra: Default::default(),
    }
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn missing_file_is_an_empty_library() {
    let (_dir, path) = scratch();
    let loaded = load(&path);
    assert_eq!(loaded.outcome, LoadOutcome::Fresh);
    assert!(loaded.file.folders.is_empty());
    assert!(loaded.file.smart_folders.is_empty());
}

#[test]
fn edits_round_trip_and_keep_a_backup() {
    let (dir, path) = scratch();
    update(&path, |f| f.smart_folders.push(smart("a"))).unwrap();
    update(&path, |f| f.smart_folders.push(smart("b"))).unwrap();
    let loaded = load(&path);
    assert_eq!(loaded.outcome, LoadOutcome::Loaded);
    assert_eq!(loaded.file.smart_folders.len(), 2);
    let backup: LibraryFile =
        serde_json::from_slice(&std::fs::read(dir.path().join("library.json.bak")).unwrap())
            .unwrap();
    assert_eq!(backup.smart_folders.len(), 1);
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .all(|e| { !e.unwrap().file_name().to_string_lossy().ends_with(".tmp") })
    );
}

#[test]
fn two_writers_never_undo_each_other() {
    let (_dir, path) = scratch();
    update(&path, |f| f.smart_folders.push(smart("from-window-1"))).unwrap();
    update(&path, |f| f.smart_folders.push(smart("from-window-2"))).unwrap();
    let names: Vec<_> = load(&path)
        .file
        .smart_folders
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(names, ["from-window-1", "from-window-2"]);
}

#[test]
fn a_corrupt_file_is_moved_aside_never_overwritten() {
    let (_dir, path) = scratch();
    std::fs::write(&path, b"{ \"folders\": [ trunc").unwrap();
    let loaded = load(&path);
    let LoadOutcome::MovedAside(aside) = loaded.outcome else {
        panic!(
            "expected the file to be moved aside, got {:?}",
            loaded.outcome
        );
    };
    assert_eq!(std::fs::read(&aside).unwrap(), b"{ \"folders\": [ trunc");
    assert!(!path.exists());
    update(&path, |f| {
        f.new_folder("New", &[]);
    })
    .unwrap();
    assert!(aside.exists());
}

#[test]
fn one_broken_folder_does_not_cost_the_others() {
    let (_dir, path) = scratch();
    std::fs::write(
        &path,
        r#"{"version":1,
        "smartFolders":[
            {"id":"good","name":"Good","rules":[{"field":"title","op":"contains","value":"x"}]},
            {"name":"no id"},
            42,
            {"id":"good","name":"Duplicate id"}
        ],
        "folders":[
            {"id":"f1","name":"Launch","members":["a", 7, "b"]},
            {"id":"f2","name":"Junk","members":"junk"},
            "nope"
        ],
        "pins":["a", null, "c"]}"#,
    )
    .unwrap();
    let loaded = load(&path);
    assert_eq!(loaded.outcome, LoadOutcome::Loaded);
    assert_eq!(loaded.file.smart_folders.len(), 1);
    assert_eq!(loaded.file.smart_folders[0].name, "Good");
    assert_eq!(loaded.file.folders.len(), 2);
    assert_eq!(loaded.file.folders[0].members, ids(&["a", "b"]));
    assert!(loaded.file.folders[1].members.is_empty());
    assert_eq!(loaded.file.pins, ids(&["a", "c"]));
}

#[test]
fn a_newer_file_is_read_but_never_written() {
    let (_dir, path) = scratch();
    let newer = r#"{"version":9,"smartFolders":[{"id":"a","name":"A","rules":[
        {"field":"sentiment","is":"happy"},
        {"field":"title","op":"contains","value":"x"}
    ],"colour":"teal"}],"cloudSync":{"on":true}}"#;
    std::fs::write(&path, newer).unwrap();
    let loaded = load(&path);
    assert_eq!(loaded.outcome, LoadOutcome::ReadOnly);
    assert_eq!(loaded.file.smart_folders[0].rules.len(), 2);
    assert!(matches!(
        loaded.file.smart_folders[0].rules[0],
        Rule::Unknown(_)
    ));
    assert_eq!(
        update(&path, |f| f.smart_folders.clear()),
        Err(UpdateError::ReadOnly)
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
}

#[test]
fn unknown_rules_and_fields_survive_a_round_trip() {
    let (_dir, path) = scratch();
    std::fs::write(
        &path,
        r#"{"version":1,"smartFolders":[{"id":"a","name":"A","rules":[
            {"field":"sentiment","is":"happy"}],"colour":"teal"}],
            "folders":[{"id":"f1","name":"F","members":["x"],"icon":"star"}],
            "cloudSync":{"on":true}}"#,
    )
    .unwrap();
    update(&path, |f| f.smart_folders[0].name = "Renamed".into()).unwrap();
    let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["cloudSync"]["on"], true);
    assert_eq!(json["smartFolders"][0]["colour"], "teal");
    assert_eq!(json["smartFolders"][0]["rules"][0]["field"], "sentiment");
    assert_eq!(json["smartFolders"][0]["name"], "Renamed");
    assert_eq!(json["folders"][0]["icon"], "star");
}

#[test]
fn oversized_input_is_clipped_not_rejected() {
    let (_dir, path) = scratch();
    update(&path, |f| {
        let mut big = smart("big");
        big.name = "n".repeat(10_000);
        big.rules = (0..500)
            .map(|_| Rule::Title {
                op: TextOp::Contains,
                value: "v".repeat(10_000),
            })
            .collect();
        f.smart_folders.push(big);
        let id = f.new_folder(&"m".repeat(10_000), &[]);
        f.rename_folder(&id, &"r".repeat(10_000));
    })
    .unwrap();
    let file = load(&path).file;
    assert_eq!(file.smart_folders[0].name.chars().count(), MAX_NAME_CHARS);
    assert_eq!(file.smart_folders[0].rules.len(), MAX_RULES);
    assert_eq!(file.folders[0].name.chars().count(), MAX_NAME_CHARS);
}

#[test]
fn memberships_a_scan_did_not_see_are_kept() {
    let (_dir, path) = scratch();
    update(&path, |f| {
        let id = f.new_folder("Launch", &ids(&["on-unplugged-drive", "here"]));
        f.toggle_pins(&ids(&["on-unplugged-drive"]));
        assert_eq!(id, "f1");
    })
    .unwrap();
    update(&path, |f| f.rename_folder("f1", "Launch week")).unwrap();
    let file = load(&path).file;
    assert_eq!(
        file.folders[0].members,
        ids(&["on-unplugged-drive", "here"])
    );
    assert_eq!(file.pins, ids(&["on-unplugged-drive"]));
}

#[test]
fn an_item_can_sit_in_several_folders_and_leaving_one_keeps_the_others() {
    let mut file = LibraryFile::default();
    let launch = file.new_folder("Launch", &ids(&["a"]));
    let support = file.new_folder("Support", &[]);
    file.toggle_members(&support, &ids(&["a", "b"]));
    assert_eq!(file.holding("a"), vec![launch.as_str(), support.as_str()]);
    file.remove_members(&launch, &ids(&["a"]));
    assert_eq!(file.holding("a"), vec![support.as_str()]);
    file.toggle_members(&support, &ids(&["a", "b"]));
    assert!(file.holding("a").is_empty());
}

#[test]
fn toggling_a_mixed_selection_adds_the_missing_ones_first() {
    let mut file = LibraryFile::default();
    let launch = file.new_folder("Launch", &ids(&["a"]));
    file.toggle_members(&launch, &ids(&["a", "b"]));
    assert_eq!(file.folders[0].members, ids(&["a", "b"]));
    file.toggle_pins(&ids(&["a"]));
    file.toggle_pins(&ids(&["a", "b"]));
    assert_eq!(file.pins, ids(&["a", "b"]));
    file.toggle_pins(&ids(&["a", "b"]));
    assert!(file.pins.is_empty());
}

#[test]
fn deleting_a_folder_never_touches_pins_or_other_folders() {
    let mut file = LibraryFile::default();
    let launch = file.new_folder("Launch", &ids(&["a"]));
    let support = file.new_folder("Support", &ids(&["a"]));
    file.toggle_pins(&ids(&["a"]));
    let rules = file.save_smart(SmartFolder {
        id: String::new(),
        ..smart("")
    });
    file.delete(&launch);
    file.delete(&rules);
    assert_eq!(file.holding("a"), vec![support.as_str()]);
    assert_eq!(file.pins, ids(&["a"]));
    assert!(file.smart_folders.is_empty());
}

#[test]
fn new_ids_never_reuse_one_still_in_the_file() {
    let mut file = LibraryFile::default();
    let first = file.new_folder("A", &[]);
    let second = file.new_folder("B", &[]);
    file.delete(&first);
    let third = file.new_folder("C", &[]);
    assert_ne!(third, second);
    let smart_id = file.save_smart(SmartFolder {
        id: String::new(),
        ..smart("")
    });
    assert!(file.folders.iter().all(|folder| folder.id != smart_id));
    let edited = file.save_smart(SmartFolder {
        name: "Edited".into(),
        ..file.smart_folders[0].clone()
    });
    assert_eq!(edited, smart_id);
    assert_eq!(file.smart_folders.len(), 1);
    assert_eq!(file.smart_folders[0].name, "Edited");
}

#[test]
fn an_id_freed_by_a_delete_is_never_handed_out_again() {
    let mut file = LibraryFile::default();
    file.new_folder("A", &[]);
    file.new_folder("B", &[]);
    let third = file.new_folder("C", &[]);
    file.delete(&third);
    let next = file.new_folder("D", &[]);
    assert_ne!(next, third);
    let smart_id = file.save_smart(SmartFolder {
        id: String::new(),
        ..smart("")
    });
    file.delete(&smart_id);
    let next_smart = file.save_smart(SmartFolder {
        id: String::new(),
        ..smart("")
    });
    assert_ne!(next_smart, smart_id);
}

#[test]
fn undoing_a_delete_after_a_new_folder_keeps_both() {
    let (_dir, path) = scratch();
    std::fs::write(
        &path,
        br#"{"version":1,"folders":[{"id":"f1","name":"A","members":[]},{"id":"f2","name":"B","members":[]},{"id":"f3","name":"Old","members":["x"]}],"pins":[],"smartFolders":[]}"#,
    )
    .unwrap();
    let mut removed = None;
    update(&path, |file| {
        removed = file.folders.iter().find(|f| f.id == "f3").cloned();
        file.delete("f3");
    })
    .unwrap();
    let mut created = String::new();
    update(&path, |file| created = file.new_folder("New", &ids(&["y"]))).unwrap();
    let restored = update(&path, |file| file.folders.push(removed.take().unwrap())).unwrap();
    assert_ne!(created, "f3");
    let names: Vec<&str> = restored.folders.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["A", "B", "New", "Old"]);
    assert_eq!(restored.folders[2].members, ids(&["y"]));
    let reloaded = load(&path).file;
    assert_eq!(reloaded.folders.len(), 4);
    let again = update(&path, |file| {
        file.new_folder("Later", &[]);
    })
    .unwrap();
    let mut seen = HashSet::new();
    assert!(again.folders.iter().all(|f| seen.insert(f.id.clone())));
}
