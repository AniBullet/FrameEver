use std::path::{Path, PathBuf};

use frameever_installer_app::{
    i18n::{Language, from_ui_language, resolve},
    settings::{language_setting, removable_settings, with_first_language},
};
use serde_json::json;

#[test]
fn a_fresh_install_writes_the_language_as_the_first_ui_language() {
    assert_eq!(
        with_first_language(None, "zh-CN"),
        Some(json!({ "general_settings": { "language": "zh-CN" } }))
    );
    assert_eq!(
        with_first_language(Some(r#"{"general_settings":{"theme":"dark"}}"#), "en"),
        Some(json!({ "general_settings": { "theme": "dark", "language": "en" } }))
    );
    assert_eq!(
        with_first_language(Some(r#"{"other":1}"#), "en"),
        Some(json!({ "other": 1, "general_settings": { "language": "en" } }))
    );
}

#[test]
fn a_language_already_chosen_is_never_replaced() {
    assert_eq!(
        with_first_language(Some(r#"{"general_settings":{"language":"zh-CN"}}"#), "en"),
        None
    );
}

#[test]
fn a_settings_file_that_is_not_json_is_left_alone() {
    assert_eq!(with_first_language(Some("{broken"), "en"), None);
    assert_eq!(with_first_language(Some("[]"), "en"), None);
}

#[test]
fn the_language_comes_from_settings_then_windows() {
    assert_eq!(
        language_setting(r#"{"general_settings":{"language":"zh-CN"}}"#).as_deref(),
        Some("zh-CN")
    );
    assert_eq!(language_setting("{}"), None);
    assert_eq!(
        resolve(Some("zh-CN"), Language::English),
        Language::SimplifiedChinese
    );
    assert_eq!(
        resolve(Some("fr"), Language::SimplifiedChinese),
        Language::SimplifiedChinese
    );
    assert_eq!(resolve(None, Language::English), Language::English);
    assert_eq!(from_ui_language(0x0804), Language::SimplifiedChinese);
    assert_eq!(from_ui_language(0x0404), Language::English);
    assert_eq!(from_ui_language(0x0409), Language::English);
}

#[test]
fn deleting_settings_never_reaches_the_storage_folder() {
    let roaming = Path::new("C:\\Users\\me\\AppData\\Roaming\\io.github.anibullet.frameever");
    let local = Path::new("C:\\Users\\me\\AppData\\Local\\io.github.anibullet.frameever");
    assert_eq!(
        removable_settings(&[roaming, local], &[Path::new("D:\\Videos\\FrameEver")]),
        vec![roaming.to_path_buf(), local.to_path_buf()]
    );
    assert_eq!(
        removable_settings(
            &[roaming, local],
            &[Path::new("D:\\Videos"), &roaming.join("FrameEver")]
        ),
        vec![PathBuf::from(local)]
    );
    assert_eq!(
        removable_settings(&[roaming], &[Path::new("C:\\Users\\me\\AppData")]),
        Vec::<PathBuf>::new()
    );
}
