use std::{collections::BTreeSet, path::Path};

use frameever_converter_app::i18n::{
    Language, chinese_table, contextual_table, resolve, system_language, translate, translate_in,
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

fn string_literals(rest: &str) -> impl Iterator<Item = String> + '_ {
    let mut rest = rest;
    std::iter::from_fn(move || {
        let literal = rest.trim_start().strip_prefix('"')?;
        let end = literal.find('"')?;
        rest = literal[end + 1..]
            .trim_start()
            .strip_prefix(',')
            .unwrap_or("");
        Some(literal[..end].to_string())
    })
}

fn used_strings() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for text in sources() {
        for call in ["tr(", "tr_fmt(", "button("] {
            for rest in text.split(call).skip(1) {
                found.extend(string_literals(rest).take(1));
            }
        }
    }
    found
}

fn used_contextual_strings() -> BTreeSet<(String, String)> {
    let mut found = BTreeSet::new();
    for text in sources() {
        for rest in text.split("tr_fmt_in(").skip(1) {
            let literals: Vec<_> = string_literals(rest).take(2).collect();
            if let [context, english] = literals.as_slice() {
                found.insert((context.clone(), english.clone()));
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
fn every_string_with_a_context_has_a_chinese_entry_and_no_entry_is_stale() {
    let used = used_contextual_strings();
    let table: BTreeSet<(String, String)> = contextual_table()
        .iter()
        .map(|(context, english, _)| (context.to_string(), english.to_string()))
        .collect();
    assert_eq!(used, table);
    assert_eq!(table.len(), contextual_table().len(), "duplicate entries");
}

#[test]
fn chinese_entries_keep_their_placeholders() {
    let contextual = contextual_table()
        .iter()
        .map(|(_, english, chinese)| (*english, *chinese));
    for (english, chinese) in chinese_table().iter().copied().chain(contextual) {
        assert_eq!(placeholders(english), placeholders(chinese), "{english}");
        assert!(!chinese.trim().is_empty(), "{english}");
    }
}

#[test]
fn the_same_english_reads_differently_in_each_context() {
    assert_eq!(
        translate_in("screenshots", "{count} · {size}", Language::English),
        "{count} · {size}"
    );
    assert_eq!(
        translate_in(
            "screenshots",
            "{count} · {size}",
            Language::SimplifiedChinese
        ),
        "{count} 张 · {size}"
    );
    assert_eq!(
        translate_in(
            "recordings",
            "{count} · {size}",
            Language::SimplifiedChinese
        ),
        "{count} 个 · {size}"
    );
    assert_eq!(translate("Close", Language::SimplifiedChinese), "关闭");
}

#[test]
fn the_language_setting_wins_over_the_system_language() {
    assert_eq!(resolve(Some("zh-CN")), Language::SimplifiedChinese);
    assert_eq!(resolve(Some("en")), Language::English);
    assert_eq!(resolve(Some("fr")), system_language());
    assert_eq!(resolve(None), system_language());
}
