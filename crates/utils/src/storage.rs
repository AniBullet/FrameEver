use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::brand::{EXPORTS_DIR_NAME, RECORDINGS_DIR_NAME, SCREENSHOTS_DIR_NAME, STORAGE_DIR_NAME};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Recording,
    Screenshot,
}

impl ProjectKind {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Recording => RECORDINGS_DIR_NAME,
            Self::Screenshot => SCREENSHOTS_DIR_NAME,
        }
    }
}

pub const STORAGE_PATH_KEY: &str = "storagePath";
pub const PREVIOUS_STORAGE_PATHS_KEY: &str = "previousStoragePaths";
pub const LAST_EXPORT_FOLDER_KEY: &str = "lastExportFolder";

pub fn last_export_folder(general: &serde_json::Map<String, serde_json::Value>) -> Option<PathBuf> {
    general
        .get(LAST_EXPORT_FOLDER_KEY)
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
}

pub fn last_export_folder_entry(export: &Path) -> Option<(&'static str, serde_json::Value)> {
    export.parent().map(|folder| {
        (
            LAST_EXPORT_FOLDER_KEY,
            serde_json::Value::from(folder.to_string_lossy()),
        )
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageSettings {
    pub storage_path: Option<PathBuf>,
    pub previous_storage_paths: Vec<PathBuf>,
}

impl StorageSettings {
    pub fn from_settings_document(document: &serde_json::Value) -> Self {
        document
            .get("general_settings")
            .and_then(serde_json::Value::as_object)
            .map(Self::from_general_settings)
            .unwrap_or_default()
    }

    pub fn from_general_settings(general: &serde_json::Map<String, serde_json::Value>) -> Self {
        let setting = |key: &str| general.get(key);
        Self {
            storage_path: setting(STORAGE_PATH_KEY)
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from),
            previous_storage_paths: setting(PREVIOUS_STORAGE_PATHS_KEY)
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

    pub fn general_settings_entries(&self) -> [(&'static str, serde_json::Value); 2] {
        let path_value = |path: &PathBuf| serde_json::Value::from(path.to_string_lossy());
        [
            (
                STORAGE_PATH_KEY,
                self.storage_path
                    .as_ref()
                    .map(path_value)
                    .unwrap_or_default(),
            ),
            (
                PREVIOUS_STORAGE_PATHS_KEY,
                self.previous_storage_paths.iter().map(path_value).collect(),
            ),
        ]
    }

    pub fn choose_storage_folder(&mut self, folder: Option<PathBuf>) {
        if let Some(old) = self.storage_path.take()
            && Some(&old) != folder.as_ref()
            && !self.previous_storage_paths.contains(&old)
        {
            self.previous_storage_paths.push(old);
        }
        if let Some(new) = &folder {
            self.previous_storage_paths.retain(|path| path != new);
        }
        self.storage_path = folder;
    }

    pub fn remove_previous_storage_folder(&mut self, folder: &Path) {
        self.previous_storage_paths.retain(|path| path != folder);
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
pub const STORAGE_DIR_ENV: &str = "FRAMEEVER_GPUI_STORAGE_DIR";
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

pub fn storage_override() -> Option<PathBuf> {
    env_path(STORAGE_DIR_ENV)
}

pub fn default_storage_dir() -> PathBuf {
    if let Some(test_profile) = env_path(APP_DATA_DIR_ENV) {
        return test_profile.join(STORAGE_DIR_NAME);
    }
    directories::UserDirs::new()
        .and_then(|dirs| dirs.video_dir().map(Path::to_path_buf))
        .unwrap_or_else(app_data_dir)
        .join(STORAGE_DIR_NAME)
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
const EXPORTS_KEY: &str = "exports";
const LEGACY_PROJECT_EXTENSION: &str = "cap";

pub fn project_dir_name(name: &str) -> String {
    format!("{name}.{}", crate::brand::PROJECT_EXTENSION)
}

pub fn unused_project_path(parent: &Path, name: &str) -> std::io::Result<PathBuf> {
    crate::ensure_unique_filename(&project_dir_name(name), parent)
        .map(|dir_name| parent.join(dir_name))
        .map_err(std::io::Error::other)
}

pub fn create_project(parent: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(parent)?;
    let project = unused_project_path(parent, name)?;
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

fn write_project_file(
    project: &Path,
    document: &serde_json::Map<String, serde_json::Value>,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(document).map_err(std::io::Error::other)?;
    std::fs::write(project.join(PROJECT_FILE_NAME), bytes)
}

pub fn stamp_format_version(project: &Path) -> std::io::Result<()> {
    let mut document = read_project_file(project);
    document.insert(FORMAT_VERSION_KEY.into(), FORMAT_VERSION.into());
    write_project_file(project, &document)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecord {
    pub path: PathBuf,
    pub format: String,
    pub codec: Option<String>,
    pub exported_at: u64,
}

fn read_project_file_for_update(
    project: &Path,
) -> std::io::Result<serde_json::Map<String, serde_json::Value>> {
    match std::fs::read(project.join(PROJECT_FILE_NAME)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(std::io::Error::other),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(error) => Err(error),
    }
}

pub fn record_export(project: &Path, record: &ExportRecord) -> std::io::Result<()> {
    let mut document = read_project_file_for_update(project)?;
    let entry = serde_json::to_value(record).map_err(std::io::Error::other)?;
    match document.get_mut(EXPORTS_KEY) {
        Some(serde_json::Value::Array(records)) => records.push(entry),
        _ => {
            document.insert(EXPORTS_KEY.into(), vec![entry].into());
        }
    }
    write_project_file(project, &document)
}

pub fn exports(project: &Path) -> Vec<ExportRecord> {
    let document = read_project_file(project);
    document
        .get(EXPORTS_KEY)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| serde_json::from_value::<ExportRecord>(entry.clone()).ok())
        .filter(|record| record.path.is_file())
        .collect()
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
    default_storage_dir: PathBuf,
    settings: StorageSettings,
    storage_override: Option<PathBuf>,
}

impl Storage {
    pub fn new(default_storage_dir: PathBuf, settings: StorageSettings) -> Self {
        Self {
            default_storage_dir,
            settings,
            storage_override: None,
        }
    }

    pub fn with_storage_override(mut self, dir: Option<PathBuf>) -> Self {
        self.storage_override = dir;
        self
    }

    pub fn from_environment() -> Self {
        Self::new(
            default_storage_dir(),
            StorageSettings::load(&settings_file()),
        )
        .with_storage_override(storage_override())
    }

    pub fn projects(&self, kind: ProjectKind) -> Vec<PathBuf> {
        self.known_dirs(kind)
            .iter()
            .flat_map(|dir| projects_in(dir))
            .collect()
    }

    pub fn storage_dir(&self) -> PathBuf {
        if let Some(dir) = &self.storage_override {
            return dir.clone();
        }
        self.settings
            .storage_path
            .clone()
            .filter(|path| path.is_absolute() && std::fs::create_dir_all(path).is_ok())
            .unwrap_or_else(|| self.default_storage_dir.clone())
    }

    pub fn new_projects_dir(&self, kind: ProjectKind) -> PathBuf {
        self.storage_dir().join(kind.dir_name())
    }

    pub fn recordings_dir(&self) -> PathBuf {
        self.new_projects_dir(ProjectKind::Recording)
    }

    pub fn screenshots_dir(&self) -> PathBuf {
        self.new_projects_dir(ProjectKind::Screenshot)
    }

    pub fn exports_dir(&self) -> PathBuf {
        self.storage_dir().join(EXPORTS_DIR_NAME)
    }

    pub fn export_dialog_folder(&self, last_export_folder: Option<&Path>) -> PathBuf {
        if let Some(folder) = last_export_folder.filter(|folder| folder.is_dir()) {
            return folder.to_path_buf();
        }
        let exports = self.exports_dir();
        if let Err(error) = std::fs::create_dir_all(&exports) {
            tracing::warn!(%error, path = %exports.display(), "Could not create the exports folder");
        }
        exports
    }

    pub fn known_dirs(&self, kind: ProjectKind) -> Vec<PathBuf> {
        let storage_dirs: Vec<PathBuf> = match &self.storage_override {
            Some(dir) => vec![dir.clone()],
            None => self
                .settings
                .storage_path
                .iter()
                .cloned()
                .chain([self.default_storage_dir.clone()])
                .chain(self.settings.previous_storage_paths.iter().cloned())
                .filter(|path| path.is_absolute())
                .collect(),
        };
        dedupe_existing_dirs(
            storage_dirs
                .into_iter()
                .map(|dir| dir.join(kind.dir_name()))
                .collect(),
        )
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

    fn settings_with_folder(storage_path: PathBuf, previous: Vec<PathBuf>) -> StorageSettings {
        StorageSettings {
            storage_path: Some(storage_path),
            previous_storage_paths: previous,
        }
    }

    #[test]
    fn new_projects_go_to_recordings_and_screenshots_in_the_default_storage_folder() {
        let tree = Tree::new();
        let storage = Storage::new(tree.path("Videos/FrameEver"), StorageSettings::default());

        assert_eq!(storage.storage_dir(), tree.path("Videos/FrameEver"));
        assert_eq!(
            storage.recordings_dir(),
            tree.path("Videos/FrameEver/Recordings")
        );
        assert_eq!(
            storage.screenshots_dir(),
            tree.path("Videos/FrameEver/Screenshots")
        );
    }

    #[test]
    fn a_custom_storage_folder_holds_both_recordings_and_screenshots() {
        let tree = Tree::new();
        let storage = Storage::new(
            tree.path("Videos/FrameEver"),
            settings_with_folder(tree.path("D/Captures"), Vec::new()),
        );

        assert_eq!(storage.storage_dir(), tree.path("D/Captures"));
        assert!(tree.path("D/Captures").is_dir());
        assert_eq!(storage.recordings_dir(), tree.path("D/Captures/Recordings"));
        assert_eq!(
            storage.screenshots_dir(),
            tree.path("D/Captures/Screenshots")
        );
    }

    #[test]
    fn an_unavailable_storage_folder_sends_new_projects_to_the_default_until_it_returns() {
        let tree = Tree::new();
        std::fs::write(tree.path("unplugged"), b"").unwrap();
        let default = tree.path("Videos/FrameEver");
        let unplugged = Storage::new(
            default.clone(),
            settings_with_folder(tree.path("unplugged/FrameEver"), Vec::new()),
        );
        let relative = Storage::new(
            default.clone(),
            settings_with_folder("relative".into(), Vec::new()),
        );

        assert_eq!(unplugged.storage_dir(), default);
        assert_eq!(relative.storage_dir(), default);

        std::fs::remove_file(tree.path("unplugged")).unwrap();
        let shot = tree.dir("unplugged/FrameEver/Screenshots/Shot.ever");
        assert_eq!(unplugged.storage_dir(), tree.path("unplugged/FrameEver"));
        assert_eq!(unplugged.projects(ProjectKind::Screenshot), vec![shot]);
    }

    #[test]
    fn known_folders_are_custom_then_default_then_previous_per_kind_without_duplicates_or_missing_ones()
     {
        let tree = Tree::new();
        let default_recordings = tree.dir("Videos/FrameEver/Recordings");
        let default_screenshots = tree.dir("Videos/FrameEver/Screenshots");
        let custom_recordings = tree.dir("custom/Recordings");
        let custom_screenshots = tree.dir("custom/Screenshots");
        let previous_recordings = tree.dir("previous/Recordings");
        let previous_screenshots = tree.dir("previous/Screenshots");
        let storage = Storage::new(
            tree.path("Videos/FrameEver"),
            settings_with_folder(
                tree.path("custom"),
                vec![
                    tree.path("unplugged"),
                    tree.path("previous"),
                    tree.path("custom"),
                    PathBuf::from("relative"),
                ],
            ),
        );

        assert_eq!(
            storage.known_dirs(ProjectKind::Recording),
            vec![custom_recordings, default_recordings, previous_recordings]
        );
        assert_eq!(
            storage.known_dirs(ProjectKind::Screenshot),
            vec![
                custom_screenshots,
                default_screenshots,
                previous_screenshots
            ]
        );
    }

    #[test]
    fn the_storage_override_receives_new_projects_and_is_the_only_folder_scanned() {
        let tree = Tree::new();
        tree.dir("Videos/FrameEver/Recordings");
        tree.dir("custom/Recordings");
        let isolated = tree.dir("isolated/Recordings");
        let storage = Storage::new(
            tree.path("Videos/FrameEver"),
            settings_with_folder(tree.path("custom"), Vec::new()),
        )
        .with_storage_override(Some(tree.path("isolated")));

        assert_eq!(storage.recordings_dir(), isolated);
        assert_eq!(storage.screenshots_dir(), tree.path("isolated/Screenshots"));
        assert_eq!(storage.known_dirs(ProjectKind::Recording), vec![isolated]);
        assert!(storage.known_dirs(ProjectKind::Screenshot).is_empty());
    }

    #[test]
    fn choosing_a_new_storage_folder_keeps_the_old_one_as_a_previous_folder() {
        let mut settings = StorageSettings::default();

        settings.choose_storage_folder(Some("D:\\One".into()));
        assert_eq!(settings, settings_with_folder("D:\\One".into(), Vec::new()));

        settings.choose_storage_folder(Some("E:\\Two".into()));
        settings.choose_storage_folder(Some("E:\\Two".into()));
        assert_eq!(
            settings,
            settings_with_folder("E:\\Two".into(), vec!["D:\\One".into()])
        );

        settings.choose_storage_folder(Some("D:\\One".into()));
        assert_eq!(
            settings,
            settings_with_folder("D:\\One".into(), vec!["E:\\Two".into()])
        );

        settings.choose_storage_folder(None);
        assert_eq!(
            settings,
            StorageSettings {
                storage_path: None,
                previous_storage_paths: vec!["E:\\Two".into(), "D:\\One".into()],
            }
        );
    }

    #[test]
    fn a_previous_folder_can_be_removed_and_is_then_no_longer_scanned() {
        let tree = Tree::new();
        tree.dir("old/Recordings/Old.ever");
        let mut settings = settings_with_folder(tree.path("new"), vec![tree.path("old")]);

        settings.remove_previous_storage_folder(&tree.path("old"));

        assert_eq!(settings, settings_with_folder(tree.path("new"), Vec::new()));
        let storage = Storage::new(tree.path("Videos/FrameEver"), settings);
        assert!(storage.projects(ProjectKind::Recording).is_empty());
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

    fn export_record(path: PathBuf, format: &str, codec: Option<&str>, at: u64) -> ExportRecord {
        ExportRecord {
            path,
            format: format.into(),
            codec: codec.map(Into::into),
            exported_at: at,
        }
    }

    #[test]
    fn an_export_is_recorded_in_the_project_and_read_back() {
        let tree = Tree::new();
        let project = create_project(&tree.path("Recordings"), "Demo").unwrap();
        let video = tree.path("Exports/Demo.mp4");
        tree.dir("Exports");
        std::fs::write(&video, b"video").unwrap();
        let record = export_record(video, "mp4", Some("H264"), 1_793_000_000);

        record_export(&project, &record).unwrap();

        assert_eq!(exports(&project), vec![record]);
        assert_eq!(format_version(&project), Some(1));
    }

    #[test]
    fn an_unreadable_frameever_file_is_left_alone_instead_of_overwritten_by_an_export() {
        let tree = Tree::new();
        let project = tree.dir("Damaged.ever");
        std::fs::write(project.join("frameever.json"), b"{ not json").unwrap();

        let result = record_export(
            &project,
            &export_record(tree.path("Demo.mp4"), "mp4", None, 1),
        );

        assert!(result.is_err());
        assert_eq!(
            std::fs::read(project.join("frameever.json")).unwrap(),
            b"{ not json"
        );
    }

    #[test]
    fn the_save_dialog_opens_in_the_last_export_folder_or_the_exports_folder_the_first_time() {
        let tree = Tree::new();
        let storage = Storage::new(tree.path("Videos/FrameEver"), StorageSettings::default());

        let first_time = storage.export_dialog_folder(None);
        assert_eq!(first_time, tree.path("Videos/FrameEver/Exports"));
        assert!(first_time.is_dir());

        let chosen = tree.dir("D/Shared");
        assert_eq!(storage.export_dialog_folder(Some(&chosen)), chosen);
        assert_eq!(
            storage.export_dialog_folder(Some(&tree.path("unplugged"))),
            first_time
        );

        let general = serde_json::json!({ "lastExportFolder": chosen.to_string_lossy() });
        assert_eq!(
            last_export_folder(general.as_object().unwrap()),
            Some(chosen.clone())
        );
        assert_eq!(last_export_folder(&serde_json::Map::new()), None);
        assert_eq!(
            last_export_folder_entry(&chosen.join("Demo.mp4")),
            Some((LAST_EXPORT_FOLDER_KEY, general["lastExportFolder"].clone()))
        );
        assert_eq!(last_export_folder_entry(Path::new("")), None);
    }

    #[test]
    fn every_export_is_kept_in_order_and_exports_whose_files_are_gone_are_dropped() {
        let tree = Tree::new();
        let project = create_project(&tree.path("Recordings"), "Demo").unwrap();
        tree.dir("Exports");
        let first = export_record(tree.path("Exports/Demo.mp4"), "mp4", Some("Hevc"), 1);
        let moved = export_record(tree.path("Exports/Demo.gif"), "gif", None, 2);
        let third = export_record(tree.path("Elsewhere/Demo.mov"), "mov", None, 3);
        tree.dir("Elsewhere");
        for record in [&first, &moved, &third] {
            std::fs::write(&record.path, b"video").unwrap();
            record_export(&project, record).unwrap();
        }

        std::fs::remove_file(&moved.path).unwrap();

        assert_eq!(exports(&project), vec![first, third]);
        assert!(exports(&tree.dir("Unexported.ever")).is_empty());
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
        let default_project = tree.dir("Videos/FrameEver/Recordings/Default.ever");
        let custom_project = tree.dir("custom/Recordings/Custom.ever");
        let previous_project = tree.dir("previous/Recordings/Previous.ever");
        tree.dir("previous/Recordings/Not a project");
        let screenshot = tree.dir("previous/Screenshots/Shot.ever");
        let storage = Storage::new(
            tree.path("Videos/FrameEver"),
            settings_with_folder(tree.path("custom"), vec![tree.path("previous")]),
        );

        let mut recordings = storage.projects(ProjectKind::Recording);
        recordings.sort();
        let mut expected = vec![default_project, custom_project, previous_project];
        expected.sort();
        assert_eq!(recordings, expected);
        assert_eq!(storage.projects(ProjectKind::Screenshot), vec![screenshot]);
    }

    #[test]
    fn storage_settings_are_read_from_the_settings_document_and_written_back_as_entries() {
        let document = serde_json::json!({
            "general_settings": {
                "storagePath": "D:\\Captures",
                "previousStoragePaths": ["E:\\Old", 7, "F:\\Older"],
                "recordingsPath": "G:\\Upstream",
                "hideDockIcon": true
            }
        });
        let settings = StorageSettings::from_settings_document(&document);

        assert_eq!(
            settings,
            settings_with_folder(
                "D:\\Captures".into(),
                vec!["E:\\Old".into(), "F:\\Older".into()]
            )
        );
        let entries: serde_json::Map<_, _> = settings
            .general_settings_entries()
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
        assert_eq!(StorageSettings::from_general_settings(&entries), settings);
        assert_eq!(
            StorageSettings::from_settings_document(&serde_json::json!([])),
            StorageSettings::default()
        );
    }
}
