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
    ("export format", "Tracks", "分轨"),
    ("transcript", "Cut", "剪掉"),
    ("window menu", "Size", "尺寸"),
    ("strength", "Light", "轻度"),
    ("eq band", "Low", "低频"),
    ("eq band", "High", "高频"),
    ("voice eq preset", "Clear", "清晰"),
    ("voice eq scope", "Mixed", "不一致"),
    ("segment noun", "audio", "音频"),
    ("segment noun", "shape", "形状"),
    ("arrowhead", "Open", "空心"),
    ("segment noun", "caption", "字幕"),
    ("segment noun", "image", "图片"),
    ("segment noun", "keyboard", "按键"),
    ("segment noun", "mask", "遮罩"),
    ("segment noun", "style", "样式"),
    ("segment noun", "text", "文本"),
    ("segment noun", "scene", "场景"),
    ("segment noun", "3D", "3D"),
    ("library menu", "View", "视图"),
    ("text align", "Left", "靠左"),
    ("screenshot tool", "Spotlight", "聚光灯"),
    ("magnifier", "Zoom", "放大"),
    ("palette hint", "Show in library", "在资源库中显示"),
    ("result card", "Show in library", "在资源库中显示"),
    ("input display style", "Outline", "线框"),
    ("gif size", "Size", "尺寸"),
    ("gif size", "Large", "大"),
    ("gif size", "Medium", "中"),
    ("gif size", "Small", "小"),
    ("gif quality", "Low", "低"),
    ("gif quality", "Medium", "中"),
    ("gif quality", "High", "高"),
    ("clip transition", "Transition", "转场"),
];
