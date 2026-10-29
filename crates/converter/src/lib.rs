mod settings;
mod transfer;

use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use frameever_project::{
    InstantRecordingMeta, Platform, ProjectConfiguration, RecordingMeta, RecordingMetaInner,
    SingleSegment, StudioRecordingMeta, VideoMeta,
};
use frameever_utils::storage::{
    self, ProjectKind, Storage, StorageSettings, create_project, discard_if_unused,
    stamp_format_version, unused_project_path,
};
use transfer::{MoveError, keep_dates, move_path};

const CAP_ANIBULLET_2_APP_ID: &str = "so.cap.desktop.anibullet";
const CAP_APP_ID: &str = "so.cap.desktop";
const CAP_ANIBULLET_2_DEFAULT_FOLDER: &str = "Cap Recordings";
const OLD_SETTINGS_FILE: &str = "store";
const OLD_PROJECT_EXTENSION: &str = "cap";
const COMPRESSED_SUFFIX: &str = "_compressed";
const COMPRESSING_SUFFIX: &str = "_compressing";
const INSTANT_OUTPUT: &str = "content/output.mp4";
const INSTANT_COMPRESSED: &str = "content/output_compressed.mp4";
const SCREENSHOT_IMAGE: &str = "original.png";
const SCREENSHOT_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];
const UNKNOWN_INSTANT_FPS: u32 = 30;
const OLD_SCREENSHOTS_FOLDER: &str = "screenshots";
const VIDEOS_DIR_ENV: &str = "FRAMEEVER_CONVERTER_VIDEOS_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    CapAniBullet2,
    Cap,
    Folder,
}

impl Source {
    pub fn carries_settings(self) -> bool {
        self != Source::Folder
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldInstall {
    pub source: Source,
    pub app_data: PathBuf,
    pub videos_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub settings_file: PathBuf,
    pub default_storage_dir: PathBuf,
}

impl Target {
    pub fn from_environment() -> Self {
        Self {
            settings_file: storage::settings_file(),
            default_storage_dir: storage::default_storage_dir(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Project {
        source: PathBuf,
        kind: ProjectKind,
    },
    Video {
        name: String,
        original: Option<PathBuf>,
        compressed: Option<PathBuf>,
        emptied_project: Option<PathBuf>,
    },
    Screenshot {
        source: PathBuf,
    },
}

impl Item {
    pub fn source(&self) -> &Path {
        match self {
            Self::Project { source, .. } | Self::Screenshot { source } => source,
            Self::Video {
                original,
                compressed,
                ..
            } => original
                .as_deref()
                .or(compressed.as_deref())
                .unwrap_or(Path::new("")),
        }
    }

    pub fn kind(&self) -> ProjectKind {
        match self {
            Self::Project { kind, .. } => *kind,
            Self::Video { .. } => ProjectKind::Recording,
            Self::Screenshot { .. } => ProjectKind::Screenshot,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    pub source: PathBuf,
    pub project: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    InUse,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub source: PathBuf,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub converted: Vec<Converted>,
    pub skipped: Vec<Skipped>,
    pub settings_error: Option<String>,
}

pub fn find_installs_on_this_machine() -> Vec<OldInstall> {
    let app_data_root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    let videos_dir = std::env::var_os(VIDEOS_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            directories::UserDirs::new().and_then(|dirs| dirs.video_dir().map(Path::to_path_buf))
        })
        .unwrap_or_default();
    find_installs(&app_data_root, &videos_dir)
}

pub fn find_installs(app_data_root: &Path, videos_dir: &Path) -> Vec<OldInstall> {
    let install = |source, app_id| OldInstall {
        source,
        app_data: app_data_root.join(app_id),
        videos_dir: videos_dir.to_path_buf(),
    };
    let cap_anibullet_2 = install(Source::CapAniBullet2, CAP_ANIBULLET_2_APP_ID);
    let cap = install(Source::Cap, CAP_APP_ID);
    let mut found = Vec::new();
    if cap_anibullet_2.app_data.is_dir() || videos_dir.join(CAP_ANIBULLET_2_DEFAULT_FOLDER).is_dir()
    {
        found.push(cap_anibullet_2);
    }
    if cap.app_data.is_dir() {
        found.push(cap);
    }
    found
}

impl OldInstall {
    pub fn picked_folder(folder: &Path) -> Self {
        Self {
            source: Source::Folder,
            app_data: folder.to_path_buf(),
            videos_dir: folder.to_path_buf(),
        }
    }

    fn settings(&self) -> serde_json::Map<String, serde_json::Value> {
        settings::read_document(&self.app_data.join(OLD_SETTINGS_FILE)).unwrap_or_default()
    }

    pub fn items(&self) -> Vec<Item> {
        let general = settings::general(&self.settings());
        let mut items = Vec::new();
        match self.source {
            Source::CapAniBullet2 => {
                let bases = existing_unique([
                    settings::path_setting(&general, "recordingsSavePath"),
                    Some(self.videos_dir.join(CAP_ANIBULLET_2_DEFAULT_FOLDER)),
                    Some(self.app_data.clone()),
                ]);
                for base in &bases {
                    items.extend(old_projects(
                        &base.join("recordings"),
                        ProjectKind::Recording,
                    ));
                }
                items.extend(old_projects(
                    &self.app_data.join("screenshots"),
                    ProjectKind::Screenshot,
                ));
                for base in &bases {
                    items.extend(loose_videos(&base.join("exports").join("video")));
                    items.extend(loose_screenshots(&base.join("exports").join("screenshot")));
                }
                put_videos_back_into_their_projects(&mut items);
            }
            Source::Cap => {
                let recordings = existing_unique(
                    std::iter::once(settings::path_setting(&general, "recordingsPath"))
                        .chain(
                            settings::path_list_setting(&general, "previousRecordingsPaths")
                                .into_iter()
                                .map(Some),
                        )
                        .chain([Some(self.app_data.join("recordings"))]),
                );
                for dir in &recordings {
                    items.extend(old_projects(dir, ProjectKind::Recording));
                }
                items.extend(old_projects(
                    &self.app_data.join("screenshots"),
                    ProjectKind::Screenshot,
                ));
            }
            Source::Folder => {
                let folder = &self.app_data;
                let is_screenshots_folder = folder
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case(OLD_SCREENSHOTS_FOLDER));
                items.extend(old_projects(
                    folder,
                    if is_screenshots_folder {
                        ProjectKind::Screenshot
                    } else {
                        ProjectKind::Recording
                    },
                ));
                items.extend(old_projects(
                    &folder.join("recordings"),
                    ProjectKind::Recording,
                ));
                items.extend(old_projects(
                    &folder.join(OLD_SCREENSHOTS_FOLDER),
                    ProjectKind::Screenshot,
                ));
                items.extend(loose_videos(&folder.join("exports").join("video")));
                items.extend(loose_screenshots(
                    &folder.join("exports").join("screenshot"),
                ));
                put_videos_back_into_their_projects(&mut items);
            }
        }
        items
    }
}

fn existing_unique(paths: impl IntoIterator<Item = Option<PathBuf>>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .flatten()
        .filter(|path| path.is_absolute() && path.is_dir())
        .filter(|path| seen.insert(path.canonicalize().unwrap_or_else(|_| path.clone())))
        .collect()
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
        .collect();
    paths.sort();
    paths
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|found| found.to_str())
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn old_projects(dir: &Path, kind: ProjectKind) -> Vec<Item> {
    sorted_entries(dir)
        .into_iter()
        .filter(|path| path.is_dir() && has_extension(path, OLD_PROJECT_EXTENSION))
        .map(|source| Item::Project { source, kind })
        .collect()
}

fn loose_videos(dir: &Path) -> Vec<Item> {
    let mut videos: BTreeMap<String, (Option<PathBuf>, Option<PathBuf>)> = BTreeMap::new();
    for path in sorted_entries(dir) {
        if !path.is_file() || !has_extension(&path, "mp4") {
            continue;
        }
        let stem = file_stem(&path);
        if stem.ends_with(COMPRESSING_SUFFIX) {
            continue;
        }
        match stem.strip_suffix(COMPRESSED_SUFFIX) {
            Some(name) => videos.entry(name.to_string()).or_default().1 = Some(path),
            None => videos.entry(stem).or_default().0 = Some(path),
        }
    }
    videos
        .into_iter()
        .map(|(name, (original, compressed))| Item::Video {
            name,
            original,
            compressed,
            emptied_project: None,
        })
        .collect()
}

fn is_instant_without_video(project: &Path) -> bool {
    !project.join(INSTANT_OUTPUT).exists()
        && RecordingMeta::load_for_project(project)
            .is_ok_and(|meta| matches!(meta.inner, RecordingMetaInner::Instant(_)))
}

fn put_videos_back_into_their_projects(items: &mut Vec<Item>) {
    let emptied: Vec<PathBuf> = items
        .iter()
        .filter_map(|item| match item {
            Item::Project {
                source,
                kind: ProjectKind::Recording,
            } if is_instant_without_video(source) => Some(source.clone()),
            _ => None,
        })
        .collect();
    let mut claimed = HashSet::new();
    for item in items.iter_mut() {
        if let Item::Video {
            name,
            emptied_project,
            ..
        } = item
            && let Some(found) = emptied
                .iter()
                .find(|path| file_stem(path) == *name && !claimed.contains(*path))
        {
            claimed.insert(found.clone());
            *emptied_project = Some(found.clone());
        }
    }
    items.retain(|item| !matches!(item, Item::Project { source, .. } if claimed.contains(source)));
}

fn loose_screenshots(dir: &Path) -> Vec<Item> {
    sorted_entries(dir)
        .into_iter()
        .filter(|path| {
            path.is_file()
                && SCREENSHOT_EXTENSIONS
                    .iter()
                    .any(|extension| has_extension(path, extension))
        })
        .map(|source| Item::Screenshot { source })
        .collect()
}

pub fn destination(install: &OldInstall, target: &Target) -> PathBuf {
    let mut storage_settings = StorageSettings::load(&target.settings_file);
    if storage_settings.storage_path.is_none()
        && let Some(folder) =
            settings::chosen_storage_folder(install.source, &settings::general(&install.settings()))
    {
        storage_settings.choose_storage_folder(Some(folder));
    }
    Storage::new(target.default_storage_dir.clone(), storage_settings).storage_dir()
}

pub fn convert(install: &OldInstall, target: &Target) -> Report {
    let items = install.items();
    let mut report = Report {
        settings_error: install
            .source
            .carries_settings()
            .then(|| {
                settings::carry_over(install.source, &install.settings(), &target.settings_file)
                    .err()
            })
            .flatten(),
        ..Report::default()
    };
    let storage = Storage::new(
        target.default_storage_dir.clone(),
        StorageSettings::load(&target.settings_file),
    );
    for item in items {
        let source = item.source().to_path_buf();
        match convert_item(&storage, &item) {
            Ok(project) => report.converted.push(Converted { source, project }),
            Err(reason) => report.skipped.push(Skipped { source, reason }),
        }
    }
    report
}

impl From<MoveError> for SkipReason {
    fn from(error: MoveError) -> Self {
        match error {
            MoveError::InUse => Self::InUse,
            MoveError::Other(error) => Self::Failed(error.to_string()),
        }
    }
}

fn failure(error: impl std::fmt::Display) -> SkipReason {
    SkipReason::Failed(error.to_string())
}

fn convert_item(storage: &Storage, item: &Item) -> Result<PathBuf, SkipReason> {
    let parent = storage.new_projects_dir(item.kind());
    std::fs::create_dir_all(&parent).map_err(failure)?;
    match item {
        Item::Project { source, .. } => {
            let project = unused_project_path(&parent, &file_stem(source)).map_err(failure)?;
            move_path(source, &project)?;
            stamp_format_version(&project).map_err(failure)?;
            Ok(project)
        }
        Item::Video {
            name,
            original,
            compressed,
            emptied_project: emptied,
        } => {
            let files: Vec<(&Path, &str)> = match (original, compressed) {
                (Some(original), compressed) => {
                    std::iter::once((original.as_path(), INSTANT_OUTPUT))
                        .chain(compressed.as_deref().map(|path| (path, INSTANT_COMPRESSED)))
                        .collect()
                }
                (None, Some(compressed)) => vec![(compressed.as_path(), INSTANT_OUTPUT)],
                (None, None) => Vec::new(),
            };
            if let Some(emptied) = emptied {
                let project = unused_project_path(&parent, name).map_err(failure)?;
                move_path(emptied, &project)?;
                if let Err(reason) = move_into_project(&project, &files) {
                    move_path(&project, emptied).ok();
                    return Err(reason);
                }
                stamp_format_version(&project).map_err(failure)?;
                return Ok(project);
            }
            let dates = std::fs::metadata(item.source()).map_err(failure)?;
            let project = create_project(&parent, name).map_err(failure)?;
            if let Err(reason) = move_into_project(&project, &files) {
                discard_new_project(&project);
                return Err(reason);
            }
            instant_meta(&project, name)
                .save_for_project()
                .map_err(failure)?;
            keep_dates(&project, &dates).map_err(failure)?;
            Ok(project)
        }
        Item::Screenshot { source } => {
            let name = file_stem(source);
            let dates = std::fs::metadata(source).map_err(failure)?;
            let project = create_project(&parent, &name).map_err(failure)?;
            let placed = if has_extension(source, "png") {
                move_into_project(&project, &[(source.as_path(), SCREENSHOT_IMAGE)])
            } else {
                convert_image_into_project(source, &project, &dates)
            };
            if let Err(reason) = placed {
                discard_new_project(&project);
                return Err(reason);
            }
            screenshot_meta(&project, &name)
                .save_for_project()
                .map_err(failure)?;
            screenshot_configuration()
                .write(&project)
                .map_err(failure)?;
            keep_dates(&project, &dates).map_err(failure)?;
            Ok(project)
        }
    }
}

fn move_into_project(project: &Path, files: &[(&Path, &str)]) -> Result<(), SkipReason> {
    let mut moved: Vec<(&Path, PathBuf)> = Vec::new();
    for (source, relative) in files {
        let destination = project.join(relative);
        let result = destination
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(MoveError::from)
            .and_then(|()| move_path(source, &destination));
        if let Err(error) = result {
            for (source, destination) in moved.iter().rev() {
                move_path(destination, source).ok();
            }
            return Err(error.into());
        }
        moved.push((source, destination));
    }
    Ok(())
}

fn convert_image_into_project(
    source: &Path,
    project: &Path,
    dates: &std::fs::Metadata,
) -> Result<(), SkipReason> {
    let bytes = std::fs::read(source).map_err(MoveError::from)?;
    let png = project.join(SCREENSHOT_IMAGE);
    image::load_from_memory(&bytes)
        .and_then(|decoded| decoded.save_with_format(&png, image::ImageFormat::Png))
        .map_err(failure)?;
    keep_dates(&png, dates).map_err(failure)?;
    if let Err(error) = std::fs::remove_file(source) {
        std::fs::remove_file(&png).ok();
        return Err(MoveError::from(error).into());
    }
    Ok(())
}

fn discard_new_project(project: &Path) {
    if let Some(content) = project.join(INSTANT_OUTPUT).parent() {
        std::fs::remove_dir(content).ok();
    }
    discard_if_unused(project).ok();
}

fn instant_meta(project: &Path, name: &str) -> RecordingMeta {
    RecordingMeta {
        platform: Some(Platform::default()),
        project_path: project.to_path_buf(),
        pretty_name: name.to_string(),
        sharing: None,
        inner: RecordingMetaInner::Instant(InstantRecordingMeta::Complete {
            fps: UNKNOWN_INSTANT_FPS,
            sample_rate: None,
        }),
        upload: None,
    }
}

fn screenshot_meta(project: &Path, name: &str) -> RecordingMeta {
    RecordingMeta {
        platform: Some(Platform::default()),
        project_path: project.to_path_buf(),
        pretty_name: name.to_string(),
        sharing: None,
        inner: RecordingMetaInner::Studio(Box::new(StudioRecordingMeta::SingleSegment {
            segment: SingleSegment {
                display: VideoMeta {
                    path: SCREENSHOT_IMAGE.into(),
                    fps: 0,
                    start_time: Some(0.0),
                    device_id: None,
                },
                camera: None,
                audio: None,
                cursor: None,
            },
        })),
        upload: None,
    }
}

fn screenshot_configuration() -> ProjectConfiguration {
    let mut config = ProjectConfiguration::default();
    config.background.source = frameever_project::BackgroundSource::Color {
        value: [255, 255, 255],
        alpha: 0,
    };
    config.background.shadow = 0.0;
    config
}
