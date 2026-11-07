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
    ("Choose", "选择"),
    ("Review", "确认"),
    ("Convert", "转换"),
    ("Done", "完成"),
    (
        "Bring your recordings into FrameEver",
        "把录制带进 FrameEver",
    ),
    (
        "Found on this PC. Choose one to see what comes over.",
        "在这台电脑上找到了这些。选一项，看看会带上什么。",
    ),
    (
        "No Cap or Cap-aniBullet 2.x data on this PC. Choose the folder that holds your recordings.",
        "这台电脑上没有 Cap 或 Cap-aniBullet 2.x 的数据。选择存放录制的文件夹。",
    ),
    (
        "{recordings} recordings · {screenshots} screenshots · {size}",
        "{recordings} 个录制 · {screenshots} 张截图 · {size}",
    ),
    ("Converted", "已转换"),
    ("Choose a folder…", "选择文件夹…"),
    ("Choose another…", "换一个文件夹…"),
    (
        "There's nothing to convert in this folder.",
        "这个文件夹里没有可转换的内容。",
    ),
    ("Close", "关闭"),
    ("Back", "返回"),
    ("Choose what comes over", "选择要带上的内容"),
    ("From {source}", "来自 {source}"),
    ("Recordings", "录制"),
    (
        ".cap projects and loose MP4 exports",
        ".cap 项目和零散的 MP4 导出",
    ),
    ("{count} · {size}|recordings", "{count} 个 · {size}"),
    ("Screenshots", "截图"),
    (
        "PNG and JPEG, kept with their dates",
        "PNG 和 JPEG，保留原来的日期",
    ),
    ("{count} · {size}|screenshots", "{count} 张 · {size}"),
    ("Settings and automations", "设置和自动化"),
    (
        "Only what FrameEver doesn't have yet",
        "只补上 FrameEver 还没有的",
    ),
    ("{count} rules", "{count} 条规则"),
    ("Destination", "转换到"),
    ("Change…", "更改…"),
    (
        "Files are moved, not copied. Afterwards only FrameEver opens them.",
        "文件会被移动而不是复制，之后只有 FrameEver 能打开它们。",
    ),
    (
        "Files are moved to another drive: copied, checked, then the originals are deleted. Afterwards only FrameEver opens them.",
        "文件会移到另一个磁盘：先复制、再校验，然后删除原文件。之后只有 FrameEver 能打开它们。",
    ),
    ("Convert {count} items", "转换 {count} 项"),
    (
        "Not enough space on {drive}. This needs {needed} and {free} is free. Change the folder or convert less.",
        "{drive} 盘空间不够。需要 {needed}，只剩 {free}。换个文件夹，或者少转换一些。",
    ),
    (
        "{app} is running. Close it first, or the projects it has open stay behind.",
        "{app} 正在运行。先关闭它，否则它打开着的项目会留在原处。",
    ),
    (
        "FrameEver is running. Close it first, or it may overwrite the automations brought over.",
        "FrameEver 正在运行。先关闭它，否则它可能会覆盖带过来的自动化。",
    ),
    ("Check again", "再次检查"),
    ("Converting…", "正在转换…"),
    ("Keep this window open.", "不要关闭这个窗口。"),
    (
        "Moving {at} of {total}: {name}",
        "正在移动第 {at} 项，共 {total} 项：{name}",
    ),
    ("Finishing this item…", "正在完成当前一项…"),
    ("{size} left", "还剩 {size}"),
    (
        "Items not moved yet stay where they are if you stop.",
        "停止后，还没移动的内容留在原处。",
    ),
    ("Stop", "停止"),
    ("Conversion finished", "转换完成"),
    (
        "{total} items are now in FrameEver.",
        "{total} 项内容已进入 FrameEver。",
    ),
    ("Conversion stopped", "转换已停止"),
    (
        "{converted} of {total} items are now in FrameEver. The rest are where they were.",
        "{total} 项中的 {converted} 项已进入 FrameEver，其余的还在原处。",
    ),
    ("Some items were left behind", "有些内容没有转换"),
    (
        "{converted} of {total} items are now in FrameEver.",
        "{total} 项中的 {converted} 项已进入 FrameEver。",
    ),
    (
        "In use by another program. Close it and try again.",
        "正被其他程序占用。关闭该程序后再试一次。",
    ),
    (
        "Couldn't move this recording. It's still where it was.",
        "无法移动这个录制。它还在原来的位置。",
    ),
    (
        "Couldn't move this screenshot. It's still where it was.",
        "无法移动这张截图。它还在原来的位置。",
    ),
    (
        "Couldn't carry over your settings. Your recordings aren't affected.",
        "无法带上你的设置。录制不受影响。",
    ),
    ("Details", "详情"),
    ("Show folder", "显示文件夹"),
    ("Open FrameEver", "打开 FrameEver"),
    ("Convert another", "转换另一处"),
    ("Convert the rest", "转换剩下的"),
    ("Try again", "再试一次"),
];

static ZH_CN_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| ZH_CN.iter().copied().collect());

pub fn chinese_table() -> &'static [(&'static str, &'static str)] {
    ZH_CN
}

pub fn translate(key: &'static str, language: Language) -> &'static str {
    let english = key.split('|').next().unwrap_or(key);
    match language {
        Language::English => english,
        Language::SimplifiedChinese => ZH_CN_MAP.get(key).copied().unwrap_or(english),
    }
}
