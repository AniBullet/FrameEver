use std::{
    collections::HashSet,
    io::Write as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::rules::{Rule, SmartFolder};

pub const VERSION: u32 = 1;
pub const MAX_FOLDERS: usize = 200;
pub const MAX_RULES: usize = 50;
pub const MAX_NAME_CHARS: usize = 120;
pub const MAX_VALUE_CHARS: usize = 400;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    #[serde(default, deserialize_with = "lenient_ids")]
    pub members: Vec<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default, deserialize_with = "lenient_list")]
    pub folders: Vec<Folder>,
    #[serde(default, deserialize_with = "lenient_ids")]
    pub pins: Vec<String>,
    #[serde(default, deserialize_with = "lenient_list")]
    pub smart_folders: Vec<SmartFolder>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl Default for LibraryFile {
    fn default() -> Self {
        Self {
            version: VERSION,
            folders: Vec::new(),
            pins: Vec::new(),
            smart_folders: Vec::new(),
            extra: Default::default(),
        }
    }
}

fn default_version() -> u32 {
    VERSION
}

fn lenient_list<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    let values = Vec::<Value>::deserialize(deserializer).unwrap_or_default();
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

fn lenient_ids<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let values = Vec::<Value>::deserialize(deserializer).unwrap_or_default();
    Ok(values
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect())
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoadOutcome {
    Fresh,
    Loaded,
    MovedAside(PathBuf),
    ReadOnly,
}

#[derive(Debug)]
pub struct Loaded {
    pub file: LibraryFile,
    pub outcome: LoadOutcome,
}

#[derive(Debug, PartialEq)]
pub enum UpdateError {
    ReadOnly,
    Io(String),
}

fn sanitize(file: &mut LibraryFile) {
    let mut seen = HashSet::new();
    file.folders
        .retain(|folder| !folder.id.is_empty() && seen.insert(folder.id.clone()));
    file.smart_folders
        .retain(|folder| !folder.id.is_empty() && seen.insert(folder.id.clone()));
    file.folders.truncate(MAX_FOLDERS);
    file.smart_folders.truncate(MAX_FOLDERS);
    for folder in &mut file.folders {
        folder.name = clip(&folder.name, MAX_NAME_CHARS);
        dedupe(&mut folder.members);
    }
    for folder in &mut file.smart_folders {
        folder.name = clip(&folder.name, MAX_NAME_CHARS);
        folder.rules.truncate(MAX_RULES);
        for rule in &mut folder.rules {
            if let Rule::Title { value, .. } = rule {
                *value = clip(value, MAX_VALUE_CHARS);
            }
        }
    }
    dedupe(&mut file.pins);
}

fn dedupe(ids: &mut Vec<String>) {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

pub fn load(path: &Path) -> Loaded {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                file: LibraryFile::default(),
                outcome: LoadOutcome::Fresh,
            };
        }
        Err(error) => {
            tracing::warn!(%error, "the library file could not be read");
            return Loaded {
                file: LibraryFile::default(),
                outcome: LoadOutcome::ReadOnly,
            };
        }
    };
    match serde_json::from_slice::<LibraryFile>(&bytes) {
        Ok(mut file) => {
            sanitize(&mut file);
            let outcome = if file.version > VERSION {
                LoadOutcome::ReadOnly
            } else {
                LoadOutcome::Loaded
            };
            Loaded { file, outcome }
        }
        Err(error) => {
            tracing::warn!(%error, "the library file did not parse; moving it aside");
            let aside = path.with_file_name(format!(
                "library.unreadable-{}.json",
                chrono::Utc::now().format("%Y%m%dT%H%M%S%.3f")
            ));
            match std::fs::rename(path, &aside) {
                Ok(()) => Loaded {
                    file: LibraryFile::default(),
                    outcome: LoadOutcome::MovedAside(aside),
                },
                Err(_) => Loaded {
                    file: LibraryFile::default(),
                    outcome: LoadOutcome::ReadOnly,
                },
            }
        }
    }
}

pub fn update(
    path: &Path,
    edit: impl FnOnce(&mut LibraryFile),
) -> Result<LibraryFile, UpdateError> {
    static WRITER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turn = WRITER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let loaded = load(path);
    if loaded.outcome == LoadOutcome::ReadOnly {
        return Err(UpdateError::ReadOnly);
    }
    let mut file = loaded.file;
    edit(&mut file);
    file.version = VERSION;
    sanitize(&mut file);
    write_atomic(path, &file).map_err(|error| UpdateError::Io(error.to_string()))?;
    Ok(file)
}

fn write_atomic(path: &Path, file: &LibraryFile) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("the library file has no folder"))?;
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(file).map_err(std::io::Error::other)?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let staged = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    {
        let mut out = std::fs::File::create(&staged)?;
        out.write_all(&json)?;
        out.sync_all()?;
    }
    if path.exists()
        && let Err(error) = std::fs::copy(path, dir.join(format!("{name}.bak")))
    {
        tracing::warn!(%error, "could not keep a backup of the library file");
    }
    std::fs::rename(&staged, path).inspect_err(|_| {
        if let Err(error) = std::fs::remove_file(&staged) {
            tracing::debug!(%error, "no staged library file to remove");
        }
    })
}

impl LibraryFile {
    fn unused_id(&self, prefix: &str) -> String {
        let highest = self
            .folders
            .iter()
            .map(|folder| folder.id.as_str())
            .chain(self.smart_folders.iter().map(|folder| folder.id.as_str()))
            .filter_map(|id| id.strip_prefix(prefix)?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("{prefix}{}", highest + 1)
    }

    fn folder_mut(&mut self, id: &str) -> Option<&mut Folder> {
        self.folders.iter_mut().find(|folder| folder.id == id)
    }

    pub fn new_folder(&mut self, name: &str, members: &[String]) -> String {
        let id = self.unused_id("f");
        self.folders.push(Folder {
            id: id.clone(),
            name: clip(name, MAX_NAME_CHARS),
            members: members.to_vec(),
            extra: Default::default(),
        });
        id
    }

    pub fn rename_folder(&mut self, id: &str, name: &str) {
        let name = clip(name.trim(), MAX_NAME_CHARS);
        if name.is_empty() {
            return;
        }
        if let Some(folder) = self.folder_mut(id) {
            folder.name = name;
        } else if let Some(folder) = self.smart_folders.iter_mut().find(|f| f.id == id) {
            folder.name = name;
        }
    }

    pub fn delete(&mut self, id: &str) {
        self.folders.retain(|folder| folder.id != id);
        self.smart_folders.retain(|folder| folder.id != id);
    }

    pub fn toggle_members(&mut self, id: &str, items: &[String]) {
        if let Some(folder) = self.folder_mut(id) {
            toggle(&mut folder.members, items);
        }
    }

    pub fn remove_members(&mut self, id: &str, items: &[String]) {
        if let Some(folder) = self.folder_mut(id) {
            folder.members.retain(|member| !items.contains(member));
        }
    }

    pub fn toggle_pins(&mut self, items: &[String]) {
        toggle(&mut self.pins, items);
    }

    pub fn save_smart(&mut self, mut folder: SmartFolder) -> String {
        folder.name = clip(folder.name.trim(), MAX_NAME_CHARS);
        if let Some(held) = self
            .smart_folders
            .iter_mut()
            .find(|held| !folder.id.is_empty() && held.id == folder.id)
        {
            *held = folder;
            return held.id.clone();
        }
        folder.id = self.unused_id("s");
        let id = folder.id.clone();
        self.smart_folders.push(folder);
        id
    }

    pub fn holding(&self, item: &str) -> Vec<&str> {
        self.folders
            .iter()
            .filter(|folder| folder.members.iter().any(|member| member == item))
            .map(|folder| folder.id.as_str())
            .collect()
    }

    pub fn is_pinned(&self, item: &str) -> bool {
        self.pins.iter().any(|pin| pin == item)
    }
}

fn toggle(held: &mut Vec<String>, items: &[String]) {
    if items.iter().all(|item| held.contains(item)) {
        held.retain(|member| !items.contains(member));
    } else {
        for item in items {
            if !held.contains(item) {
                held.push(item.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests;
