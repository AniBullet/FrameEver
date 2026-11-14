use std::{
    collections::HashMap,
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
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::SimplifiedChinese => "zh-CN",
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

pub fn from_ui_language(id: u16) -> Language {
    const ZH_CN: u16 = 0x0804;
    if id == ZH_CN {
        Language::SimplifiedChinese
    } else {
        Language::English
    }
}

pub fn from_setting(code: &str) -> Option<Language> {
    let code = code.trim().to_ascii_lowercase().replace('_', "-");
    if code == "en" || code.starts_with("en-") {
        Some(Language::English)
    } else if code == "zh" || code.starts_with("zh-") {
        Some(Language::SimplifiedChinese)
    } else {
        None
    }
}

pub fn resolve(setting: Option<&str>, system: Language) -> Language {
    setting.and_then(from_setting).unwrap_or(system)
}

#[cfg(windows)]
pub fn system_language() -> Language {
    from_ui_language(unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() })
}

#[cfg(not(windows))]
pub fn system_language() -> Language {
    Language::English
}

pub fn ui_font() -> &'static str {
    match current() {
        Language::SimplifiedChinese => "HarmonyOS Sans SC",
        Language::English => "Inter",
    }
}

static ZH_CN: &[(&str, &str)] = &[
    ("FrameEver", "帧刻"),
    (
        "Record, edit and export, all on your own computer.",
        "录屏、剪辑、导出，全部留在你自己的电脑上。",
    ),
    ("Install FrameEver", "安装帧刻"),
    ("Update FrameEver", "更新帧刻"),
    ("Uninstall FrameEver", "卸载帧刻"),
    ("Install", "安装"),
    ("Options", "选项"),
    ("Install location", "安装位置"),
    ("Change…", "更改…"),
    (
        "Needs {needed} · {free} free",
        "需要 {needed} · 可用 {free}",
    ),
    ("Needs {needed}", "需要 {needed}"),
    ("Desktop shortcut", "桌面快捷方式"),
    ("Start menu shortcut", "开始菜单快捷方式"),
    ("Add frameever to PATH", "把 frameever 加入 PATH"),
    ("Run the CLI from any terminal.", "在任意终端里运行 CLI。"),
    ("Installing…", "正在安装…"),
    ("Opening FrameEver…", "正在打开帧刻…"),
    ("Close FrameEver to continue", "关闭帧刻后继续"),
    ("Close the other program to continue", "关闭其他程序后继续"),
    (
        "FrameEver is open, so its files can't be replaced.",
        "帧刻正在运行，文件无法替换。",
    ),
    (
        "FrameEver is open, so its files can't be replaced. Quit it from the tray, then try again.",
        "帧刻正在运行，文件无法替换。从托盘退出后再试一次。",
    ),
    (
        "{apps} is using FrameEver's files. Close it, then try again.",
        "{apps} 正在使用帧刻的文件。关闭后再试一次。",
    ),
    ("Cancel", "取消"),
    ("Close FrameEver", "关闭帧刻"),
    ("Finish your recording first", "先结束正在进行的录制"),
    (
        "FrameEver is recording. The installer won't close it. Stop the recording, then try again.",
        "帧刻正在录制，安装器不会关闭它。停止录制后再试一次。",
    ),
    ("Try again", "再试一次"),
    ("Couldn't install FrameEver", "无法安装帧刻"),
    (
        "Drive {drive} has {free} free and FrameEver needs {needed}. Free up space or choose another drive.",
        "{drive} 盘还有 {free}，帧刻需要 {needed}。清理空间，或换一个磁盘。",
    ),
    ("Change location", "更改位置"),
    (
        "Setup stopped before it finished. Try again.",
        "安装中途停止了。再试一次。",
    ),
    ("Details", "详情"),
    ("Export logs", "导出日志"),
    ("Couldn't open FrameEver", "无法打开帧刻"),
    (
        "FrameEver is installed but didn't start. Try again.",
        "帧刻已经装好，但没能启动。再试一次。",
    ),
    ("Update to {version}", "更新到 {version}"),
    (
        "Version {version} is installed. Location, shortcuts and settings stay as they are.",
        "已安装 {version}。位置、快捷方式和设置都保持不变。",
    ),
    (
        "An earlier version is installed. Location, shortcuts and settings stay as they are.",
        "已安装较早的版本。位置、快捷方式和设置都保持不变。",
    ),
    ("Uninstall instead", "改为卸载"),
    ("Update", "更新"),
    ("FrameEver {version} is installed", "已安装帧刻 {version}"),
    (
        "Reinstall it to repair missing files. Settings stay.",
        "重新安装可以修复缺失的文件，设置保留。",
    ),
    ("Uninstall", "卸载"),
    ("Reinstall", "重新安装"),
    ("A newer version is installed", "已安装更新的版本"),
    (
        "FrameEver {installed} is on this computer. This installer is {version}.",
        "这台电脑上是帧刻 {installed}，这个安装程序是 {version}。",
    ),
    ("Install {version}", "安装 {version}"),
    ("Keep {installed}", "保留 {installed}"),
    ("Updating FrameEver…", "正在更新帧刻…"),
    ("Couldn't update FrameEver", "无法更新帧刻"),
    (
        "Try again, or get the installer from GitHub Releases.",
        "再试一次，或从 GitHub Releases 下载安装程序。",
    ),
    ("Open Releases", "打开 Releases"),
    ("Uninstall FrameEver?", "卸载帧刻？"),
    (
        "This removes the app, its shortcuts and its PATH entry.",
        "这会移除应用、快捷方式和 PATH 项。",
    ),
    (
        "Recordings, screenshots and exports stay in {folder}.",
        "录制、截图和导出保留在 {folder}。",
    ),
    ("Also delete settings", "同时删除设置"),
    (
        "Settings and logs in the app data folder.",
        "应用数据文件夹里的设置和日志。",
    ),
    ("Uninstalling FrameEver", "正在卸载帧刻"),
    (
        "Your recordings stay where they are.",
        "你的录制保留在原处。",
    ),
    ("Uninstalling…", "正在卸载…"),
    ("Couldn't uninstall FrameEver", "无法卸载帧刻"),
    (
        "Uninstall stopped before it finished. Try again.",
        "卸载中途停止了。再试一次。",
    ),
    ("FrameEver is uninstalled", "帧刻已卸载"),
    (
        "Your recordings, screenshots and exports are still in {folder}.",
        "你的录制、截图和导出还在 {folder}。",
    ),
    ("Open folder", "打开文件夹"),
    ("Close", "关闭"),
    (
        "Can't be closed while files are being copied",
        "复制文件时不能关闭",
    ),
];

static ZH_CN_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| ZH_CN.iter().copied().collect());

pub fn chinese_table() -> &'static [(&'static str, &'static str)] {
    ZH_CN
}

pub fn translate(english: &'static str, language: Language) -> &'static str {
    match language {
        Language::English => english,
        Language::SimplifiedChinese => ZH_CN_MAP.get(english).copied().unwrap_or(english),
    }
}

pub fn tr(english: &'static str) -> &'static str {
    translate(english, current())
}
