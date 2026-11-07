use std::{collections::BTreeSet, path::Path};

use frameever_converter_app::i18n::{Language, chinese_table, resolve, system_language, translate};

fn placeholders(text: &str) -> BTreeSet<String> {
    text.split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name.to_string()))
        .collect()
}

fn used_strings() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = BTreeSet::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "i18n.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for call in ["tr(", "tr_fmt(", "button("] {
            for rest in text.split(call).skip(1) {
                if let Some(literal) = rest.trim_start().strip_prefix('"') {
                    let end = literal.find('"').unwrap();
                    found.insert(literal[..end].to_string());
                }
            }
        }
    }
    found
}

#[test]
fn every_string_the_wizard_shows_has_a_chinese_entry_and_no_entry_is_stale() {
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
fn chinese_entries_keep_their_placeholders() {
    for (english, chinese) in chinese_table() {
        assert_eq!(placeholders(english), placeholders(chinese), "{english}");
        assert!(!chinese.trim().is_empty(), "{english}");
    }
}

#[test]
fn a_key_with_a_context_shows_only_its_text_in_english() {
    assert_eq!(
        translate("{count} · {size}|screenshots", Language::English),
        "{count} · {size}"
    );
    assert_eq!(
        translate("{count} · {size}|screenshots", Language::SimplifiedChinese),
        "{count} 张 · {size}"
    );
}

#[test]
fn the_language_setting_wins_over_the_system_language() {
    assert_eq!(resolve(Some("zh-CN")), Language::SimplifiedChinese);
    assert_eq!(resolve(Some("en")), Language::English);
    assert_eq!(resolve(Some("fr")), system_language());
    assert_eq!(resolve(None), system_language());
}
