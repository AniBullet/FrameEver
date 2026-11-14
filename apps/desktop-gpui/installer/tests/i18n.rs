use std::{collections::BTreeSet, path::Path};

use frameever_installer_app::{
    i18n::{Language, chinese_table, translate},
    instance_control::{Control, Reply},
};

fn placeholders(text: &str) -> BTreeSet<String> {
    text.split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name.to_string()))
        .collect()
}

fn sources() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| !path.file_name().is_some_and(|name| name == "i18n.rs"))
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect()
}

fn first_literal(rest: &str) -> Option<String> {
    let literal = rest.trim_start().strip_prefix('"')?;
    let end = literal.find('"')?;
    Some(literal[..end].to_string())
}

fn used_strings() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for text in sources() {
        for call in ["tr(", "tr_fmt(", "button("] {
            for rest in text.split(call).skip(1) {
                found.extend(first_literal(rest));
            }
        }
    }
    found
}

#[test]
fn every_string_the_installer_shows_has_a_chinese_entry_and_no_entry_is_stale() {
    let used = used_strings();
    let table: BTreeSet<String> = chinese_table()
        .iter()
        .map(|(en, _)| en.to_string())
        .collect();
    let missing: Vec<_> = used.difference(&table).collect();
    let stale: Vec<_> = table.difference(&used).collect();
    assert!(missing.is_empty(), "missing zh-CN entries: {missing:?}");
    assert!(stale.is_empty(), "unused zh-CN entries: {stale:?}");
    assert_eq!(
        table.len(),
        chinese_table().len(),
        "duplicate zh-CN entries"
    );
}

#[test]
fn chinese_entries_keep_their_placeholders_and_call_the_app_by_its_chinese_name() {
    for (english, chinese) in chinese_table() {
        assert_eq!(placeholders(english), placeholders(chinese), "{english}");
        assert!(!chinese.trim().is_empty(), "{english}");
        assert!(!chinese.contains("FrameEver"), "{english}");
    }
    assert_eq!(translate("Close", Language::SimplifiedChinese), "关闭");
    assert_eq!(translate("Close", Language::English), "Close");
}

#[test]
fn the_running_app_and_the_installer_speak_the_same_control_protocol() {
    for control in [Control::Busy, Control::Quit] {
        assert_eq!(Control::from_size(control.size()), Some(control));
        assert!(control.size() > 1024 * 1024);
    }
    assert_eq!(Control::from_size(0), None);
    for reply in [Reply::Idle, Reply::Busy, Reply::Quitting, Reply::Refused] {
        assert_eq!(Reply::from_byte(reply.byte()), Some(reply));
        assert_ne!(reply.byte(), 1);
    }
    assert_eq!(Reply::to_quit(true, true), Reply::Refused);
    assert_eq!(Reply::to_quit(false, false), Reply::Refused);
    assert_eq!(Reply::to_quit(false, true), Reply::Quitting);
}
