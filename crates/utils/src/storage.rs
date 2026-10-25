use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::brand::{RECORDINGS_DIR_NAME, SCREENSHOTS_DIR_NAME};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Recording,
    Screenshot,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageSettings {
    pub recordings_path: Option<PathBuf>,
    pub previous_recordings_paths: Vec<PathBuf>,
}

impl StorageSettings {
    pub fn from_settings_document(document: &serde_json::Value) -> Self {
        document
            .get("general_settings")
            .map(Self::from_general_settings)
            .unwrap_or_default()
    }

    pub fn from_general_settings(general: &serde_json::Value) -> Self {
        let setting = |key: &str| general.get(key);
        Self {
            recordings_path: setting("recordingsPath")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from),
            previous_recordings_paths: setting("previousRecordingsPaths")
                .and_then(serde_json::Value::as_array)
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(PathBuf::from)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn load(settings_file: &Path) -> Self {
        std::fs::read(settings_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .map(|document| Self::from_settings_document(&document))
            .unwrap_or_default()
    }
}

pub const APP_DATA_DIR_ENV: &str = "FRAMEEVER_GPUI_APP_DATA_DIR";
pub const SETTINGS_FILE_ENV: &str = "FRAMEEVER_GPUI_TAURI_STORE";
pub const RECORDINGS_DIR_ENV: &str = "FRAMEEVER_GPUI_RECORDINGS_DIR";
const SETTINGS_FILE_NAME: &str = "store";

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

pub fn app_data_dir() -> PathBuf {
    env_path(APP_DATA_DIR_ENV).unwrap_or_else(|| app_data_root().join(crate::brand::APP_ID))
}

fn app_data_root() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join("Library/Application Support")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

pub fn settings_file() -> PathBuf {
    env_path(SETTINGS_FILE_ENV).unwrap_or_else(|| app_data_dir().join(SETTINGS_FILE_NAME))
}

pub fn recordings_override() -> Option<PathBuf> {
    env_path(RECORDINGS_DIR_ENV)
}

pub fn is_project(path: &Path) -> bool {
    path.is_dir()
        && path
            .extension()
            .is_some_and(|extension| extension == crate::brand::PROJECT_EXTENSION)
}

pub const FORMAT_VERSION: u64 = 1;
const PROJECT_FILE_NAME: &str = "frameever.json";
const FORMAT_VERSION_KEY: &str = "formatVersion";
const LEGACY_PROJECT_EXTENSION: &str = "cap";

pub fn project_dir_name(name: &str) -> String {
    format!("{name}.{}", crate::brand::PROJECT_EXTENSION)
}

pub fn create_project(parent: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(parent)?;
    let dir_name = crate::ensure_unique_filename(&project_dir_name(name), parent)
        .map_err(std::io::Error::other)?;
    let project = parent.join(dir_name);
    std::fs::create_dir(&project)?;
    stamp_format_version(&project)?;
    Ok(project)
}

pub fn discard_if_unused(project: &Path) -> std::io::Result<()> {
    let Ok(entries) = std::fs::read_dir(project) else {
        return Ok(());
    };
    let only_project_file = entries
        .flatten()
        .all(|entry| entry.file_name() == PROJECT_FILE_NAME);
    if only_project_file {
        std::fs::remove_dir_all(project)?;
    }
    Ok(())
}

fn read_project_file(project: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read(project.join(PROJECT_FILE_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn stamp_format_version(project: &Path) -> std::io::Result<()> {
    let mut document = read_project_file(project);
    document.insert(FORMAT_VERSION_KEY.into(), FORMAT_VERSION.into());
    let bytes = serde_json::to_vec_pretty(&document).map_err(std::io::Error::other)?;
    std::fs::write(project.join(PROJECT_FILE_NAME), bytes)
}

pub fn format_version(project: &Path) -> Option<u64> {
    read_project_file(project)
        .get(FORMAT_VERSION_KEY)
        .and_then(serde_json::Value::as_u64)
}

pub fn needs_converter(path: &Path) -> bool {
    let is_older_project = |path: &Path| {
        path.is_dir()
            && path
                .extension()
                .is_some_and(|extension| extension == LEGACY_PROJECT_EXTENSION)
    };
    is_older_project(path) || path.parent().is_some_and(is_older_project)
}

pub fn projects_in(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
        .filter(|path| is_project(path))
        .collect()
}

#[derive(Debug, Clone)]
pub struct Storage {
    app_data_dir: PathBuf,
    settings: StorageSettings,
    recordings_override: Option<PathBuf>,
}

impl Storage {
    pub fn new(app_data_dir: PathBuf, settings: StorageSettings) -> Self {
        Self {
            app_data_dir,
            settings,
            recordings_override: None,
        }
    }

    pub fn with_recordings_override(mut self, dir: Option<PathBuf>) -> Self {
        self.recordings_override = dir;
        self
    }

    pub fn from_environment() -> Self {
        Self::new(app_data_dir(), StorageSettings::load(&settings_file()))
            .with_recordings_override(recordings_override())
    }

    pub fn projects(&self, kind: ProjectKind) -> Vec<PathBuf> {
        self.known_dirs(kind)
            .iter()
            .flat_map(|dir| projects_in(dir))
            .collect()
    }

    pub fn recordings_dir(&self) -> PathBuf {
        if let Some(dir) = &self.recordings_override {
            return dir.clone();
        }
        self.settings
            .recordings_path
            .clone()
            .filter(|path| path.is_absolute() && std::fs::create_dir_all(path).is_ok())
            .unwrap_or_else(|| self.default_dir(ProjectKind::Recording))
    }

    pub fn screenshots_dir(&self) -> PathBuf {
        self.default_dir(ProjectKind::Screenshot)
    }

    pub fn default_dir(&self, kind: ProjectKind) -> PathBuf {
        self.app_data_dir.join(match kind {
            ProjectKind::Recording => RECORDINGS_DIR_NAME,
            ProjectKind::Screenshot => SCREENSHOTS_DIR_NAME,
        })
    }

    pub fn known_dirs(&self, kind: ProjectKind) -> Vec<PathBuf> {
        let candidates = match kind {
            ProjectKind::Recording => {
                if let Some(dir) = &self.recordings_override {
                    vec![dir.clone()]
                } else {
                    self.settings
                        .recordings_path
                        .iter()
                        .cloned()
                        .chain([self.default_dir(kind)])
                        .chain(self.settings.previous_recordings_paths.iter().cloned())
                        .filter(|path| path.is_absolute())
                        .collect()
                }
            }
            ProjectKind::Screenshot => vec![self.default_dir(kind)],
        };
        dedupe_existing_dirs(candidates)
    }
}

fn dedupe_existing_dirs(dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    dirs.into_iter()
        .filter(|dir| dir.is_dir())
        .filter(|dir| seen.insert(dir.canonicalize().unwrap_or_else(|_| dir.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree(tempfile::TempDir);

    impl Tree {
        fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.0.path().join(relative)
        }

        fn dir(&self, relative: &str) -> PathBuf {
            let path = self.path(relative);
            std::fs::create_dir_all(&path).unwrap();
            path
        }
    }

    #[test]
    fn recording_folders_are_custom_then_default_then_previous_without_duplicates_or_missing_ones()
    {
        let tree = Tree::new();
        let app_data = tree.dir("app");
        let default = tree.dir("app/recordings");
        let custom = tree.dir("custom");
        let previous = tree.dir("previous");
        let storage = Storage::new(
            app_data,
            StorageSettings {
                recordings_path: Some(custom.clone()),
                previous_recordings_paths: vec![
                    tree.path("unplugged"),
                    previous.clone(),
                    custom.clone(),
                    PathBuf::from("relative"),
                ],
            },
        );

        assert_eq!(
            storage.known_dirs(ProjectKind::Recording),
            vec![custom, default, previous]
        );
    }

    #[test]
    fn new_recordings_go_to_the_custom_folder_unless_it_cannot_be_created() {
        let tree = Tree::new();
        let app_data = tree.dir("app");
        std::fs::write(tree.path("not-a-drive"), b"").unwrap();
        let storage_with = |custom: PathBuf| {
            Storage::new(
                app_data.clone(),
                StorageSettings {
                    recordings_path: Some(custom),
                    previous_recordings_paths: Vec::new(),
                },
            )
        };

        let custom = tree.path("custom");
        assert_eq!(storage_with(custom.clone()).recordings_dir(), custom);
        assert!(custom.is_dir());

        assert_eq!(
            storage_with(tree.path("not-a-drive/Recordings")).recordings_dir(),
            tree.path("app/recordings")
        );
        assert_eq!(
            storage_with(PathBuf::from("relative")).recordings_dir(),
            tree.path("app/recordings")
        );
    }

    #[test]
    fn the_recordings_override_receives_new_recordings_and_is_the_only_folder_scanned() {
        let tree = Tree::new();
        let app_data = tree.dir("app");
        tree.dir("app/recordings");
        let custom = tree.dir("custom");
        let isolated = tree.dir("isolated");
        let storage = Storage::new(
            app_data,
            StorageSettings {
                recordings_path: Some(custom),
                previous_recordings_paths: Vec::new(),
            },
        )
        .with_recordings_override(Some(isolated.clone()));

        assert_eq!(storage.recordings_dir(), isolated);
        assert_eq!(storage.known_dirs(ProjectKind::Recording), vec![isolated]);
    }

    #[test]
    fn a_new_project_is_an_ever_folder_stamped_with_the_format_version() {
        let tree = Tree::new();
        let recordings = tree.path("Recordings");

        let first = create_project(&recordings, "Display 2026-10-25").unwrap();
        let second = create_project(&recordings, "Display 2026-10-25").unwrap();

        assert_eq!(first, tree.path("Recordings/Display 2026-10-25.ever"));
        assert_eq!(second, tree.path("Recordings/Display 2026-10-25 (1).ever"));
        for project in [&first, &second] {
            assert!(is_project(project));
            assert_eq!(format_version(project), Some(1));
        }
    }

    #[test]
    fn an_unused_new_project_is_discarded_but_one_with_content_is_kept() {
        let tree = Tree::new();
        let unused = create_project(&tree.path("Recordings"), "Unused").unwrap();
        let recorded = create_project(&tree.path("Recordings"), "Recorded").unwrap();
        std::fs::write(recorded.join("recording-meta.json"), b"{}").unwrap();

        discard_if_unused(&unused).unwrap();
        discard_if_unused(&recorded).unwrap();
        discard_if_unused(&tree.path("Recordings/Missing.ever")).unwrap();

        assert!(!unused.exists());
        assert!(recorded.join("recording-meta.json").is_file());
    }

    #[test]
    fn stamping_keeps_other_fields_of_the_frameever_file() {
        let tree = Tree::new();
        let project = tree.dir("Demo.ever");
        std::fs::write(
            project.join("frameever.json"),
            br#"{"formatVersion":0,"exports":[{"path":"D:\\out.mp4"}]}"#,
        )
        .unwrap();

        stamp_format_version(&project).unwrap();

        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(project.join("frameever.json")).unwrap())
                .unwrap();
        assert_eq!(document["formatVersion"], 1);
        assert_eq!(document["exports"][0]["path"], "D:\\out.mp4");
        assert_eq!(format_version(&tree.dir("Unstamped.ever")), None);
    }

    #[test]
    fn only_ever_folders_count_as_projects_and_cap_folders_need_the_converter() {
        let tree = Tree::new();
        let project = tree.dir("Demo.ever");
        let older = tree.dir("Demo.cap");
        let folder = tree.dir("Holiday");
        std::fs::write(tree.path("loose.ever"), b"").unwrap();
        std::fs::write(tree.path("loose.cap"), b"").unwrap();

        assert!(is_project(&project));
        assert!(!is_project(&older));
        assert!(!is_project(&folder));
        assert!(!is_project(&tree.path("loose.ever")));
        assert!(!is_project(&tree.path("missing.ever")));

        assert!(needs_converter(&older));
        assert!(needs_converter(&older.join("recording-meta.json")));
        assert!(!needs_converter(&project));
        assert!(!needs_converter(&folder));
        assert!(!needs_converter(&tree.path("loose.cap")));
    }

    #[test]
    fn projects_are_listed_from_every_known_folder_of_their_kind() {
        let tree = Tree::new();
        let app_data = tree.dir("app");
        let default_project = tree.dir("app/recordings/Default.ever");
        let custom_project = tree.dir("custom/Custom.ever");
        let previous_project = tree.dir("previous/Previous.ever");
        tree.dir("previous/Not a project");
        let screenshot = tree.dir("app/screenshots/Shot.ever");
        let storage = Storage::new(
            app_data,
            StorageSettings {
                recordings_path: Some(tree.path("custom")),
                previous_recordings_paths: vec![tree.path("previous")],
            },
        );

        let mut recordings = storage.projects(ProjectKind::Recording);
        recordings.sort();
        let mut expected = vec![default_project, custom_project, previous_project];
        expected.sort();
        assert_eq!(recordings, expected);
        assert_eq!(storage.projects(ProjectKind::Screenshot), vec![screenshot]);
    }

    #[test]
    fn storage_settings_are_read_from_the_settings_document() {
        let document = serde_json::json!({
            "general_settings": {
                "recordingsPath": "D:\\Recordings",
                "previousRecordingsPaths": ["E:\\Old", 7, "F:\\Older"],
                "hideDockIcon": true
            }
        });

        assert_eq!(
            StorageSettings::from_settings_document(&document),
            StorageSettings {
                recordings_path: Some(PathBuf::from("D:\\Recordings")),
                previous_recordings_paths: vec![
                    PathBuf::from("E:\\Old"),
                    PathBuf::from("F:\\Older")
                ],
            }
        );
        assert_eq!(
            StorageSettings::from_settings_document(&serde_json::json!([])),
            StorageSettings::default()
        );
    }

    #[test]
    fn screenshots_live_in_the_app_data_screenshots_folder() {
        let tree = Tree::new();
        let app_data = tree.dir("app");
        let storage = Storage::new(app_data, StorageSettings::default());

        assert_eq!(storage.screenshots_dir(), tree.path("app/screenshots"));
        assert!(storage.known_dirs(ProjectKind::Screenshot).is_empty());
        let screenshots = tree.dir("app/screenshots");
        assert_eq!(
            storage.known_dirs(ProjectKind::Screenshot),
            vec![screenshots]
        );
    }
}
