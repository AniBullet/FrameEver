use std::{
    collections::HashMap,
    path::Path,
    sync::{
        LazyLock,
        atomic::{AtomicU8, Ordering},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    SimplifiedChinese,
}

impl Language {
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

pub fn language_setting(settings_file: &Path) -> Option<String> {
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(settings_file).ok()?).ok()?;
    document
        .get("general_settings")?
        .get("language")?
        .as_str()
        .map(str::to_string)
}

pub fn resolve(setting: Option<&str>) -> Language {
    setting
        .and_then(Language::from_code)
        .unwrap_or_else(system_language)
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

pub fn ui_font() -> &'static str {
    match current() {
        Language::SimplifiedChinese => "Microsoft YaHei UI",
        Language::English => "Inter",
    }
}

static ZH_CN: &[(&str, &str)] = &[
    (
        "Bring your old recordings into FrameEver",
        "把旧的录制带进 FrameEver",
    ),
    (
        "Found on this computer. Choose one to see what will be converted.",
        "在这台电脑上找到了以下数据。选择一项，查看会转换哪些内容。",
    ),
    (
        "No Cap or Cap-aniBullet 2.x data was found on this computer. Choose the folder that holds your old recordings.",
        "没有在这台电脑上找到 Cap 或 Cap-aniBullet 2.x 的数据。请选择存放旧录制的文件夹。",
    ),
    (
        "Recordings {recordings} · Screenshots {screenshots}",
        "录制文件 {recordings} · 截图 {screenshots}",
    ),
    ("Choose a folder…", "选择文件夹…"),
    ("Close", "关闭"),
    ("Ready to convert", "准备转换"),
    ("Nothing to convert here", "这里没有可转换的内容"),
    ("From {source}", "来自 {source}"),
    ("Recordings", "录制文件"),
    ("Screenshots", "截图"),
    ("Settings", "设置"),
    (
        "Carried over where FrameEver has none yet",
        "只补上 FrameEver 还没有的设置",
    ),
    ("Left unchanged", "保持不变"),
    ("Destination", "目标位置"),
    (
        "Files are moved, not copied. Afterwards only FrameEver opens them.",
        "文件会被移动而不是复制，之后只有 FrameEver 能打开它们。",
    ),
    ("Back", "返回"),
    ("Convert", "转换"),
    ("Converting…", "正在转换…"),
    (
        "Moving {count} items. Keep this window open.",
        "正在移动 {count} 项内容，请不要关闭此窗口。",
    ),
    ("Conversion finished", "转换完成"),
    ("Some items were left behind", "有些内容没有转换"),
    (
        "{converted} of {total} items are now in FrameEver.",
        "{total} 项中的 {converted} 项已进入 FrameEver。",
    ),
    (
        "In use by another program. Close it and run the converter again.",
        "正被其他程序占用。关闭该程序后再运行一次转换。",
    ),
    ("Could not convert: {error}", "无法转换：{error}"),
    (
        "Settings were not carried over: {error}",
        "设置没有带过来：{error}",
    ),
    ("Open storage folder", "打开存储文件夹"),
    ("Convert more", "继续转换"),
];

static ZH_CN_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| ZH_CN.iter().copied().collect());

pub fn translate(english: &'static str, language: Language) -> &'static str {
    match language {
        Language::English => english,
        Language::SimplifiedChinese => ZH_CN_MAP.get(english).copied().unwrap_or(english),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

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
        let table: BTreeSet<String> = ZH_CN.iter().map(|(en, _)| en.to_string()).collect();
        let missing: Vec<_> = used.difference(&table).collect();
        let stale: Vec<_> = table.difference(&used).collect();
        assert!(missing.is_empty(), "missing zh-CN entries: {missing:?}");
        assert!(stale.is_empty(), "unused zh-CN entries: {stale:?}");
        assert_eq!(table.len(), ZH_CN.len(), "duplicate zh-CN entries");
    }

    #[test]
    fn chinese_entries_keep_their_placeholders() {
        for (english, chinese) in ZH_CN {
            assert_eq!(placeholders(english), placeholders(chinese), "{english}");
            assert!(!chinese.trim().is_empty(), "{english}");
        }
    }

    #[test]
    fn the_language_setting_wins_over_the_system_language() {
        assert_eq!(resolve(Some("zh-CN")), Language::SimplifiedChinese);
        assert_eq!(resolve(Some("en")), Language::English);
        assert_eq!(resolve(Some("fr")), system_language());
        assert_eq!(resolve(None), system_language());
    }
}
