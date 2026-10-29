use std::path::{Path, PathBuf};

use frameever_utils::storage::StorageSettings;
use serde_json::{Map, Value};

use crate::Source;

const GENERAL_SETTINGS: &str = "general_settings";
const AUTOMATIONS: &str = "automations";
const EXCLUDED_WINDOWS: &str = "excludedWindows";
const STUDIO_RECORDING_QUALITY: &str = "studioRecordingQuality";

const CARRIED_KEYS: &[&str] = &[
    "language",
    "autoCompressInstant",
    "autoCompressDeleteOriginal",
    "transcriptionHints",
    "captureKeyboardEvents",
];

const RENAMED_VARIABLES: &[(&str, &str)] = &[
    ("CAP_PROJECT_PATH", "FRAMEEVER_PROJECT_PATH"),
    ("CAP_IMAGE_PATH", "FRAMEEVER_IMAGE_PATH"),
    ("CAP_OUTPUT_PATH", "FRAMEEVER_OUTPUT_PATH"),
    ("CAP_SHARE_LINK", "FRAMEEVER_SHARE_LINK"),
];

pub(crate) fn read_document(path: &Path) -> Option<Map<String, Value>> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

pub(crate) fn general(document: &Map<String, Value>) -> Map<String, Value> {
    document
        .get(GENERAL_SETTINGS)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn path_setting(general: &Map<String, Value>, key: &str) -> Option<PathBuf> {
    general
        .get(key)
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

pub(crate) fn path_list_setting(general: &Map<String, Value>, key: &str) -> Vec<PathBuf> {
    general
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect()
}

pub(crate) fn chosen_storage_folder(
    source: Source,
    old_general: &Map<String, Value>,
) -> Option<PathBuf> {
    match source {
        Source::CapAniBullet2 => path_setting(old_general, "recordingsSavePath"),
        Source::Cap | Source::Folder => None,
    }
}

pub(crate) fn carry_over(
    source: Source,
    old: &Map<String, Value>,
    settings_file: &Path,
) -> Result<(), String> {
    let mut document = match std::fs::read(settings_file) {
        Ok(bytes) => serde_json::from_slice::<Map<String, Value>>(&bytes).map_err(|error| {
            format!(
                "FrameEver settings at {} could not be read: {error}",
                settings_file.display()
            )
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(error) => return Err(error.to_string()),
    };

    let old_general = general(old);
    let mut current = general(&document);
    for (key, value) in carried_settings(source, &old_general) {
        current.entry(key).or_insert(value);
    }
    if let Some(folder) = chosen_storage_folder(source, &old_general) {
        let mut storage = StorageSettings::from_general_settings(&current);
        if storage.storage_path.is_none() {
            storage.choose_storage_folder(Some(folder));
            for (key, value) in storage.general_settings_entries() {
                current.insert(key.into(), value);
            }
        }
    }
    document.insert(GENERAL_SETTINGS.into(), Value::Object(current));
    if let Some(old_automations) = old.get(AUTOMATIONS) {
        merge_automations(&mut document, old_automations);
    }
    write_document(settings_file, &document).map_err(|error| error.to_string())
}

fn carried_settings(source: Source, old: &Map<String, Value>) -> Vec<(String, Value)> {
    let mut carried: Vec<(String, Value)> = CARRIED_KEYS
        .iter()
        .filter_map(|key| old.get(*key).map(|value| (key.to_string(), value.clone())))
        .collect();
    if let Some(Value::Array(rules)) = old.get(EXCLUDED_WINDOWS) {
        carried.push((
            EXCLUDED_WINDOWS.into(),
            rules.iter().cloned().map(rename_window_rule).collect(),
        ));
    }
    let quality = match source {
        Source::CapAniBullet2 => old
            .get("recordingQuality")
            .and_then(Value::as_str)
            .map(|quality| match quality {
                "ultra" => "ultra",
                _ => "balanced",
            })
            .map(Value::from),
        Source::Cap => old.get(STUDIO_RECORDING_QUALITY).cloned(),
        Source::Folder => None,
    };
    if let Some(quality) = quality {
        carried.push((STUDIO_RECORDING_QUALITY.into(), quality));
    }
    carried
}

fn rename_window_rule(mut rule: Value) -> Value {
    if let Some(title) = rule.get_mut("windowTitle")
        && let Some(renamed) = title.as_str().and_then(rename_window_title)
    {
        *title = Value::String(renamed);
    }
    rule
}

fn rename_window_title(title: &str) -> Option<String> {
    let app_name = frameever_utils::brand::APP_NAME;
    if title == "Cap" {
        return Some(app_name.to_string());
    }
    title
        .strip_prefix("Cap ")
        .map(|rest| format!("{app_name} {rest}"))
}

fn merge_automations(document: &mut Map<String, Value>, old: &Value) {
    let Some(old_rules) = old.get("rules").and_then(Value::as_array) else {
        return;
    };
    let automations = document
        .entry(AUTOMATIONS)
        .or_insert_with(|| serde_json::json!({ "version": old.get("version").cloned().unwrap_or(Value::from(1)), "rules": [] }));
    let Some(automations) = automations.as_object_mut() else {
        return;
    };
    let rules = automations
        .entry("rules")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(rules) = rules.as_array_mut() else {
        return;
    };
    for rule in old_rules {
        let id = rule.get("id");
        if rules.iter().any(|existing| existing.get("id") == id) {
            continue;
        }
        rules.push(rename_variables(rule.clone()));
    }
}

fn rename_variables(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(rename_variables_in(&text)),
        Value::Array(items) => items.into_iter().map(rename_variables).collect(),
        Value::Object(entries) => entries
            .into_iter()
            .map(|(key, value)| (rename_variables_in(&key), rename_variables(value)))
            .collect(),
        other => other,
    }
}

fn rename_variables_in(text: &str) -> String {
    RENAMED_VARIABLES
        .iter()
        .fold(text.to_string(), |text, (old, new)| text.replace(old, new))
}

fn write_document(path: &Path, document: &Map<String, Value>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(document).map_err(std::io::Error::other)?;
    let staged = path.with_extension("converter-tmp");
    std::fs::write(&staged, bytes)?;
    std::fs::rename(&staged, path)
}
