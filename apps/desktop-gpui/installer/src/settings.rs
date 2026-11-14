use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

const GENERAL: &str = "general_settings";
const LANGUAGE: &str = "language";

pub fn language_setting(document: &str) -> Option<String> {
    let document: Value = serde_json::from_str(document).ok()?;
    document
        .get(GENERAL)?
        .get(LANGUAGE)?
        .as_str()
        .map(str::to_string)
}

pub fn with_first_language(document: Option<&str>, code: &str) -> Option<Value> {
    let mut document = match document {
        Some(text) => serde_json::from_str::<Value>(text).ok()?,
        None => Value::Object(Map::new()),
    };
    let root = document.as_object_mut()?;
    let general = root
        .entry(GENERAL)
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()?;
    if general.get(LANGUAGE).is_some_and(|value| !value.is_null()) {
        return None;
    }
    general.insert(LANGUAGE.to_string(), Value::String(code.to_string()));
    Some(document)
}

fn normalized(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\").to_lowercase();
    text.trim_end_matches('\\').to_string()
}

fn contains(outer: &Path, inner: &Path) -> bool {
    let outer = normalized(outer);
    let inner = normalized(inner);
    inner == outer
        || inner.starts_with(&format!("{outer}\\"))
        || outer.starts_with(&format!("{inner}\\"))
}

pub fn removable_settings(roots: &[&Path], protected: &[&Path]) -> Vec<PathBuf> {
    roots
        .iter()
        .filter(|root| !protected.iter().any(|kept| contains(root, kept)))
        .map(|root| root.to_path_buf())
        .collect()
}
