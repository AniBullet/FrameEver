mod editor;
mod media;
mod onboarding;
mod settings;
mod shell;
mod system;
mod timeline;

pub static TABLES: &[&[(&str, &str)]] = &[
    onboarding::ENTRIES,
    settings::ENTRIES,
    shell::ENTRIES,
    system::ENTRIES,
    editor::ENTRIES,
    timeline::ENTRIES,
    media::ENTRIES,
];

pub static CONTEXTUAL: &[(&str, &str, &str)] = &[
    ("font weight", "Light", "细"),
    ("strength", "Light", "轻度"),
    ("strength", "Heavy", "重度"),
    ("segment noun", "audio", "音频"),
    ("segment noun", "caption", "字幕"),
    ("segment noun", "image", "图片"),
    ("segment noun", "keyboard", "按键"),
    ("segment noun", "mask", "遮罩"),
    ("segment noun", "style", "样式"),
    ("segment noun", "text", "文本"),
    ("segment noun", "scene", "场景"),
    ("segment noun", "3D", "3D"),
];
