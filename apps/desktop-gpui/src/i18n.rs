use std::{
    collections::HashMap,
    fmt::Display,
    sync::{
        LazyLock,
        atomic::{AtomicU8, Ordering},
    },
};

mod zh_cn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    SimplifiedChinese,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::English, Language::SimplifiedChinese];

    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::SimplifiedChinese => "zh-CN",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let code = code.trim().to_ascii_lowercase().replace('_', "-");
        if code == "en" || code.starts_with("en-") {
            Some(Language::English)
        } else if code == "zh" || code.starts_with("zh-") {
            Some(Language::SimplifiedChinese)
        } else {
            None
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::SimplifiedChinese => "简体中文",
        }
    }

    fn index(self) -> u8 {
        match self {
            Language::English => 0,
            Language::SimplifiedChinese => 1,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::SimplifiedChinese,
        _ => Language::English,
    }
}

pub fn set(language: Language) {
    CURRENT.store(language.index(), Ordering::Relaxed);
}

pub fn resolve(setting: Option<&str>) -> Language {
    setting
        .and_then(Language::from_code)
        .unwrap_or_else(system_language)
}

pub fn init(setting: Option<&str>) {
    set(resolve(setting));
}

#[cfg(windows)]
pub fn system_language() -> Language {
    const LANG_CHINESE: u16 = 0x04;
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    if id & 0x3ff == LANG_CHINESE {
        Language::SimplifiedChinese
    } else {
        Language::English
    }
}

#[cfg(not(windows))]
pub fn system_language() -> Language {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .and_then(|value| Language::from_code(value.split(['.', '@']).next().unwrap_or("")))
        .unwrap_or(Language::English)
}

fn entries() -> impl Iterator<Item = (&'static str, &'static str)> {
    zh_cn::TABLES.iter().flat_map(|table| table.iter().copied())
}

static ZH_CN: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| entries().collect());

pub fn translate(english: &'static str, language: Language) -> &'static str {
    match language {
        Language::English => english,
        Language::SimplifiedChinese => ZH_CN.get(english).copied().unwrap_or(english),
    }
}

pub fn tr(english: &'static str) -> &'static str {
    translate(english, current())
}

static ZH_CN_CONTEXTUAL: LazyLock<HashMap<(&'static str, &'static str), &'static str>> =
    LazyLock::new(|| {
        zh_cn::CONTEXTUAL
            .iter()
            .map(|(context, english, chinese)| ((*context, *english), *chinese))
            .collect()
    });

pub fn tr_ctx(context: &'static str, english: &'static str) -> &'static str {
    match current() {
        Language::English => english,
        Language::SimplifiedChinese => ZH_CN_CONTEXTUAL
            .get(&(context, english))
            .copied()
            .unwrap_or(english),
    }
}

pub fn tr_fmt(template: &'static str, args: &[(&str, &dyn Display)]) -> String {
    fill(tr(template), args)
}

fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(key, _)| *key == name) {
                    Some((_, value)) => out.push_str(&value.to_string()),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

macro_rules! t {
    ($text:literal) => {
        $crate::i18n::tr($text)
    };
    ($text:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tr_fmt(
            $text,
            &[$((stringify!($name), &$value as &dyn std::fmt::Display)),+],
        )
    };
}
pub(crate) use t;

macro_rules! n {
    ($text:literal) => {
        $text
    };
}
pub(crate) use n;

macro_rules! tc {
    ($context:literal, $text:literal) => {
        $crate::i18n::tr_ctx($context, $text)
    };
}
pub(crate) use tc;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else { break };
            found.insert(after[..close].to_string());
            rest = &after[close + 1..];
        }
        found
    }

    fn unescape(raw: &str) -> String {
        let mut out = String::new();
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('\'') => out.push('\''),
                Some('\n') | Some('\r') => {
                    while chars.peek().is_some_and(|c| c.is_whitespace()) {
                        chars.next();
                    }
                }
                Some('u') => {
                    let code: String = chars
                        .by_ref()
                        .skip_while(|c| *c == '{')
                        .take_while(|c| *c != '}')
                        .collect();
                    if let Some(c) = u32::from_str_radix(&code, 16).ok().and_then(char::from_u32) {
                        out.push(c);
                    }
                }
                Some(other) => out.push(other),
                None => {}
            }
        }
        out
    }

    fn sources() -> Vec<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut texts = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "rs")
                    || path.ends_with("i18n.rs")
                    || path.parent().is_some_and(|parent| parent.ends_with("i18n"))
                {
                    continue;
                }
                texts.push(std::fs::read_to_string(&path).unwrap());
            }
        }
        texts
    }

    fn leading_literal(text: &str) -> Option<(String, &str)> {
        let body = text.trim_start().strip_prefix('"')?;
        let bytes = body.as_bytes();
        let mut end = 0;
        while end < bytes.len() {
            match bytes[end] {
                b'\\' => end += 2,
                b'"' => break,
                _ => end += 1,
            }
        }
        Some((unescape(&body[..end]), &body[(end + 1).min(body.len())..]))
    }

    fn macro_calls<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
        let marker = format!("{name}!(");
        let mut calls = Vec::new();
        let mut rest = text;
        while let Some(at) = rest.find(&marker) {
            let before = rest[..at].chars().last();
            rest = &rest[at + marker.len()..];
            if !before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                calls.push(rest);
            }
        }
        calls
    }

    fn source_keys() -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        for text in sources() {
            for name in ["t", "n"] {
                for call in macro_calls(&text, name) {
                    if let Some((key, _)) = leading_literal(call) {
                        keys.insert(key);
                    }
                }
            }
        }
        keys
    }

    fn contextual_keys() -> BTreeSet<(String, String)> {
        let mut keys = BTreeSet::new();
        for text in sources() {
            for call in macro_calls(&text, "tc") {
                let Some((context, rest)) = leading_literal(call) else {
                    continue;
                };
                let Some(rest) = rest.trim_start().strip_prefix(',') else {
                    continue;
                };
                if let Some((english, _)) = leading_literal(rest) {
                    keys.insert((context, english));
                }
            }
        }
        keys
    }

    #[test]
    fn every_translated_string_has_a_chinese_entry_and_no_entry_is_stale() {
        let used = source_keys();
        let table: BTreeSet<String> = entries().map(|(en, _)| en.to_string()).collect();
        let missing: Vec<_> = used.difference(&table).collect();
        let stale: Vec<_> = table.difference(&used).collect();
        assert!(
            missing.is_empty(),
            "t!() strings without a zh-CN entry: {missing:#?}"
        );
        assert!(stale.is_empty(), "zh-CN entries no t!() uses: {stale:#?}");

        let used = contextual_keys();
        let table: BTreeSet<(String, String)> = zh_cn::CONTEXTUAL
            .iter()
            .map(|(context, english, _)| (context.to_string(), english.to_string()))
            .collect();
        let missing: Vec<_> = used.difference(&table).collect();
        let stale: Vec<_> = table.difference(&used).collect();
        assert!(
            missing.is_empty(),
            "tc!() strings without an entry: {missing:#?}"
        );
        assert!(
            stale.is_empty(),
            "contextual entries no tc!() uses: {stale:#?}"
        );
    }

    #[test]
    fn chinese_entries_are_unique_complete_and_keep_their_placeholders() {
        let mut seen = HashMap::new();
        for (english, chinese) in entries() {
            if let Some(previous) = seen.insert(english, chinese) {
                assert_eq!(
                    previous, chinese,
                    "conflicting translations for {english:?}"
                );
            }
            assert!(
                !chinese.trim().is_empty(),
                "empty translation for {english:?}"
            );
            assert_eq!(
                placeholders(english),
                placeholders(chinese),
                "placeholders differ for {english:?}"
            );
        }
    }

    #[test]
    fn language_codes_round_trip_and_unknown_codes_fall_back() {
        for language in Language::ALL {
            assert_eq!(Language::from_code(language.code()), Some(language));
        }
        assert_eq!(
            Language::from_code("zh-Hans"),
            Some(Language::SimplifiedChinese)
        );
        assert_eq!(Language::from_code("en_US"), Some(Language::English));
        assert_eq!(Language::from_code("ja"), None);
        assert_eq!(resolve(Some("zh-CN")), Language::SimplifiedChinese);
        assert_eq!(resolve(Some("en")), Language::English);
    }

    #[test]
    fn templates_fill_named_values_and_keep_unknown_ones() {
        let version = "1.0.0";
        assert_eq!(
            fill("Version {version} of {app}", &[("version", &version)]),
            "Version 1.0.0 of {app}"
        );
        assert_eq!(fill("no braces", &[]), "no braces");
        assert_eq!(fill("open { only", &[]), "open { only");
        assert_eq!(translate("Get started", Language::English), "Get started");
    }
}
