use crate::i18n::t;
use frameever_project::{
    CaptionSegment, CaptionWord, Cursors, MultipleSegment, ProjectConfiguration, RecordingMeta,
    RecordingMetaInner, StudioRecordingMeta, StudioRecordingStatus,
};
use frameever_rendering::ProjectRecordingsMeta;
use frameever_utils::storage::{self, ChapterMarker};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

#[cfg(test)]
mod tests;

const JOURNAL: &str = storage::UNUSED_MEDIA_JOURNAL;
const BACKUPS: &str = "backups";
const STAGING: &str = "content/removed-media";
const SEGMENTS: &str = "content/segments/";
const META: &str = "recording-meta.json";
const CONFIG: &str = "project-config.json";
const NEXT_META: &str = "next-recording-meta.json";
const NEXT_CONFIG: &str = "next-project-config.json";
const PROJECT_FILE: &str = storage::PROJECT_FILE_NAME;
const CHAPTER_MARKERS: &str = "chapter-markers.json";
const NEXT_CHAPTER_MARKERS: &str = "next-chapter-markers.json";
const DIAGNOSTICS: &str = "recording-diagnostics.json";
const CAPTION_TOLERANCE: f64 = 1e-3;
const SETTLE_WAIT: Duration = Duration::from_secs(30);
const MEDIA_RELEASE_WAIT: Duration = Duration::from_secs(20);
const COMMIT_RELEASE_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SegmentProbe {
    pub duration: f64,
    pub display: (u32, u32),
    pub camera: Option<(u32, u32)>,
    pub mic_channels: Option<u16>,
}

pub(crate) struct Request {
    probes: Vec<SegmentProbe>,
}

impl Request {
    pub(crate) fn new(
        config: &ProjectConfiguration,
        recordings: &ProjectRecordingsMeta,
    ) -> Option<Self> {
        let timeline = config.timeline.as_ref()?;
        let count = recordings.segments.len();
        if count < 2 || timeline.segments.is_empty() {
            return None;
        }
        let mut used = vec![false; count];
        for clip in &timeline.segments {
            if let Some(used) = used.get_mut(clip.recording_clip as usize) {
                *used = true;
            }
        }
        if used.iter().all(|used| *used) {
            return None;
        }
        Some(Self {
            probes: recordings
                .segments
                .iter()
                .map(|segment| SegmentProbe {
                    duration: segment.display.duration,
                    display: (segment.display.width, segment.display.height),
                    camera: segment
                        .camera
                        .as_ref()
                        .map(|camera| (camera.width, camera.height)),
                    mic_channels: segment.mic.as_ref().map(|mic| mic.channels),
                })
                .collect(),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_probes(probes: Vec<SegmentProbe>) -> Self {
        Self { probes }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Skip {
    NotStudio,
    SingleSegment,
    Unfinished,
    TrackFailures,
    ProbeMismatch,
    NoTimeline,
    ClipOutOfRange,
    NothingUnused,
    LegacyKeyboard,
    LegacyCaptions,
    NothingRemovable,
    EditChanged,
    ChapterMarkerOnCut,
    Invalid,
    TrashInProgress,
    ClipAlignment,
    MediaInUse,
}

#[derive(Debug)]
pub(crate) struct Plan {
    pub removed: Vec<usize>,
    pub meta: RecordingMeta,
    pub config: ProjectConfiguration,
    pub chapter_markers: Option<ChapterMarkerChange>,
    pub folders: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct ChapterMarkerChange {
    pub before: Vec<ChapterMarker>,
    pub after: Vec<ChapterMarker>,
}

fn segment_paths(segment: &MultipleSegment) -> Vec<String> {
    let mut paths = vec![segment.display.path.as_str()];
    paths.extend(segment.camera.as_ref().map(|camera| camera.path.as_str()));
    paths.extend(segment.mic.as_ref().map(|mic| mic.path.as_str()));
    paths.extend(
        segment
            .system_audio
            .as_ref()
            .map(|audio| audio.path.as_str()),
    );
    paths.extend(segment.cursor.as_ref().map(|cursor| cursor.as_str()));
    paths.extend(segment.keyboard.as_ref().map(|keyboard| keyboard.as_str()));
    paths
        .into_iter()
        .map(|path| path.replace('\\', "/"))
        .collect()
}

fn segment_folder(segment: &MultipleSegment) -> Option<String> {
    let mut folder: Option<&str> = None;
    let paths = segment_paths(segment);
    for path in &paths {
        let name = path.strip_prefix(SEGMENTS)?.split('/').next()?;
        if name.is_empty()
            || name == "."
            || name == ".."
            || path.len() <= SEGMENTS.len() + name.len() + 1
        {
            return None;
        }
        match folder {
            Some(existing) if existing != name => return None,
            _ => folder = Some(name),
        }
    }
    folder.map(|name| format!("{SEGMENTS}{name}"))
}

fn config_mentions(raw_config: &str, folder: &str) -> bool {
    let name = &folder[SEGMENTS.len()..];
    raw_config.contains(&format!("segments/{name}"))
        || raw_config.contains(&format!("segments\\\\{name}"))
}

fn same_first_segment(
    old: (&MultipleSegment, &SegmentProbe),
    new: (&MultipleSegment, &SegmentProbe),
) -> bool {
    old.1.display == new.1.display
        && old.1.camera == new.1.camera
        && old.1.mic_channels == new.1.mic_channels
        && old.0.display_notch == new.0.display_notch
        && old.0.mic.is_some() == new.0.mic.is_some()
        && old.0.system_audio.is_some() == new.0.system_audio.is_some()
}

fn has_track_failures(diagnostics: Option<&[u8]>) -> bool {
    let Some(raw) = diagnostics else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(raw) else {
        return true;
    };
    value
        .get("segments")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|segments| {
            segments.iter().any(|segment| {
                segment
                    .get("trackFailures")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(|failures| !failures.is_empty())
            })
        })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cut {
    start: f64,
    end: f64,
}

fn cuts(durations: &[f64], removed: &[usize]) -> Vec<Cut> {
    let mut offset = 0.0;
    let mut cuts: Vec<Cut> = Vec::new();
    for (index, duration) in durations.iter().enumerate() {
        if removed.contains(&index) {
            match cuts.last_mut() {
                Some(last) if last.end == offset => last.end = offset + duration,
                _ => cuts.push(Cut {
                    start: offset,
                    end: offset + duration,
                }),
            }
        }
        offset += duration;
    }
    cuts
}

fn collapse(time: f64, cuts: &[Cut]) -> f64 {
    time - cuts
        .iter()
        .map(|cut| (time.min(cut.end) - cut.start).max(0.0))
        .sum::<f64>()
}

fn inside_cut(start: f64, end: f64, cuts: &[Cut]) -> bool {
    cuts.iter().any(|cut| start >= cut.start && end <= cut.end)
}

fn remap_captions(
    segments: &[CaptionSegment],
    durations: &[f64],
    removed: &[usize],
) -> Vec<CaptionSegment> {
    let cuts = cuts(durations, removed);
    segments
        .iter()
        .filter_map(|caption| {
            if let Some(clip) = caption.recording_clip {
                let shift: f64 = durations
                    .iter()
                    .take(clip as usize)
                    .enumerate()
                    .filter(|(index, _)| removed.contains(index))
                    .map(|(_, duration)| duration)
                    .sum();
                let at = |time: f32| (f64::from(time) - shift) as f32;
                return Some(CaptionSegment {
                    id: caption.id.clone(),
                    start: at(caption.start),
                    end: at(caption.end),
                    text: caption.text.clone(),
                    words: caption
                        .words
                        .iter()
                        .map(|word| CaptionWord {
                            text: word.text.clone(),
                            start: at(word.start),
                            end: at(word.end),
                        })
                        .collect(),
                    recording_clip: Some(remap_index(clip, removed)?),
                });
            }
            if inside_cut(f64::from(caption.start), f64::from(caption.end), &cuts) {
                return None;
            }
            let at = |time: f32| collapse(f64::from(time), &cuts) as f32;
            let words: Vec<CaptionWord> = caption
                .words
                .iter()
                .filter(|word| !inside_cut(f64::from(word.start), f64::from(word.end), &cuts))
                .map(|word| CaptionWord {
                    text: word.text.clone(),
                    start: at(word.start),
                    end: at(word.end),
                })
                .collect();
            if !caption.words.is_empty() && words.is_empty() {
                return None;
            }
            let text = if words.len() == caption.words.len() {
                caption.text.clone()
            } else {
                frameever_project::caption_text_from_words(&words)
            };
            Some(CaptionSegment {
                id: caption.id.clone(),
                start: at(caption.start),
                end: at(caption.end),
                text,
                words,
                recording_clip: None,
            })
        })
        .collect()
}

fn remap_chapter_markers(
    markers: &[ChapterMarker],
    durations: &[f64],
    removed: &[usize],
) -> Option<Vec<ChapterMarker>> {
    let cuts = cuts(durations, removed);
    let total: f64 = durations.iter().sum();
    let mut moved = Vec::new();
    for marker in markers {
        let at = marker.recorded_at;
        let mut inside = false;
        for cut in &cuts {
            let ends_recording = close(cut.end, total, CAPTION_TOLERANCE);
            if close(at, cut.start, CAPTION_TOLERANCE)
                || close(at, cut.end, CAPTION_TOLERANCE)
                || (ends_recording && at > cut.end)
            {
                return None;
            }
            inside |= at > cut.start && at < cut.end;
        }
        if !inside {
            moved.push(ChapterMarker {
                recorded_at: collapse(at, &cuts),
                name: marker.name.clone(),
            });
        }
    }
    Some(moved)
}

fn remap_index(index: u32, removed: &[usize]) -> Option<u32> {
    if removed.contains(&(index as usize)) {
        return None;
    }
    let before = removed
        .iter()
        .filter(|&&removed| removed < index as usize)
        .count();
    Some(index - before as u32)
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

fn same_result(
    old: &ProjectConfiguration,
    old_durations: &[f64],
    new: &ProjectConfiguration,
    new_durations: &[f64],
) -> bool {
    let (Some(old_timeline), Some(new_timeline)) = (&old.timeline, &new.timeline) else {
        return false;
    };
    if old_timeline.duration() != new_timeline.duration() {
        return false;
    }

    let captions = |config: &ProjectConfiguration, durations: &[f64]| {
        let mut config = config.clone();
        frameever_project::synchronize_captions(&mut config, durations);
        config
            .timeline
            .map(|timeline| timeline.caption_segments)
            .unwrap_or_default()
    };
    let (old_captions, new_captions) = (captions(old, old_durations), captions(new, new_durations));
    old_captions.len() == new_captions.len()
        && old_captions.iter().zip(&new_captions).all(|(a, b)| {
            a.id == b.id
                && a.text == b.text
                && close(a.start, b.start, CAPTION_TOLERANCE)
                && close(a.end, b.end, CAPTION_TOLERANCE)
                && a.words.len() == b.words.len()
                && a.words.iter().zip(&b.words).all(|(a, b)| {
                    a.text == b.text
                        && close(f64::from(a.start), f64::from(b.start), CAPTION_TOLERANCE)
                        && close(f64::from(a.end), f64::from(b.end), CAPTION_TOLERANCE)
                })
        })
}

pub(crate) struct Snapshot {
    pub meta_bytes: Vec<u8>,
    pub config_bytes: Vec<u8>,
    pub meta: RecordingMeta,
    pub config: ProjectConfiguration,
    pub project_file: Option<Vec<u8>>,
    pub diagnostics: Option<Vec<u8>>,
    pub captions_file: bool,
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(raw) => Ok(Some(raw)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

impl Snapshot {
    pub(crate) fn read(project: &Path) -> Result<Self, String> {
        let meta_bytes = std::fs::read(project.join(META)).map_err(|error| error.to_string())?;
        let meta = RecordingMeta::load_for_project(project).map_err(|error| error.to_string())?;
        let config = ProjectConfiguration::load(project).map_err(|error| error.to_string())?;
        let config_bytes =
            std::fs::read(project.join(CONFIG)).map_err(|error| error.to_string())?;
        Ok(Self {
            meta_bytes,
            config_bytes,
            meta,
            config,
            project_file: read_optional(&project.join(PROJECT_FILE))?,
            diagnostics: read_optional(&project.join(DIAGNOSTICS))?,
            captions_file: project.join("captions.json").exists(),
        })
    }
}

pub(crate) fn plan(snapshot: &Snapshot, probes: &[SegmentProbe]) -> Result<Plan, Skip> {
    let meta = &snapshot.meta;
    let mut config = std::borrow::Cow::Borrowed(&snapshot.config);
    let RecordingMetaInner::Studio(studio) = &meta.inner else {
        return Err(Skip::NotStudio);
    };
    let StudioRecordingMeta::MultipleSegments { inner } = studio.as_ref() else {
        return Err(Skip::SingleSegment);
    };
    if config.clip_alignment_version > frameever_project::CLIP_ALIGNMENT_VERSION {
        return Err(Skip::ClipAlignment);
    }
    if config.clip_alignment_version < frameever_project::CLIP_ALIGNMENT_VERSION {
        let mut migrated = snapshot.config.clone();
        let durations = || probes.iter().map(|probe| probe.duration).collect();
        if frameever_project::migrate_late_audio_alignment(&mut migrated, studio, durations) {
            return Err(Skip::ClipAlignment);
        }
        config = std::borrow::Cow::Owned(migrated);
    }
    let config = config.as_ref();
    if !matches!(inner.status, None | Some(StudioRecordingStatus::Complete)) {
        return Err(Skip::Unfinished);
    }
    if has_track_failures(snapshot.diagnostics.as_deref()) {
        return Err(Skip::TrackFailures);
    }
    let count = inner.segments.len();
    if probes.len() != count || probes.iter().any(|probe| !probe.duration.is_finite()) {
        return Err(Skip::ProbeMismatch);
    }
    let timeline = config
        .timeline
        .as_ref()
        .filter(|timeline| !timeline.segments.is_empty())
        .ok_or(Skip::NoTimeline)?;
    let mut used = vec![false; count];
    for clip in &timeline.segments {
        *used
            .get_mut(clip.recording_clip as usize)
            .ok_or(Skip::ClipOutOfRange)? = true;
    }
    if used.iter().all(|used| *used) {
        return Err(Skip::NothingUnused);
    }
    if timeline
        .keyboard_segments
        .iter()
        .any(|segment| !segment.id.starts_with("kb-edit-"))
    {
        return Err(Skip::LegacyKeyboard);
    }
    if config.captions.is_none() && snapshot.captions_file {
        return Err(Skip::LegacyCaptions);
    }

    let raw_config = String::from_utf8_lossy(&snapshot.config_bytes);
    let cursor_images: Vec<String> = match &inner.cursors {
        Cursors::Correct(map) => map
            .values()
            .map(|cursor| cursor.image_path.as_str().replace('\\', "/"))
            .collect(),
        Cursors::Old(map) => map.values().map(|path| path.replace('\\', "/")).collect(),
    };

    let mut kept = used.clone();
    let (removed, folders) = loop {
        let kept_paths: Vec<String> = inner
            .segments
            .iter()
            .enumerate()
            .filter(|(index, _)| kept[*index])
            .flat_map(|(_, segment)| segment_paths(segment))
            .chain(cursor_images.iter().cloned())
            .collect();
        let mut removed = Vec::new();
        let mut folders: Vec<String> = Vec::new();
        let mut changed = false;
        for (index, segment) in inner.segments.iter().enumerate() {
            if kept[index] {
                continue;
            }
            let removable = segment_folder(segment).filter(|folder| {
                let prefix = format!("{folder}/");
                !kept_paths
                    .iter()
                    .any(|path| path.starts_with(&prefix) || path == folder)
                    && !config_mentions(&raw_config, folder)
            });
            match removable {
                Some(folder) => {
                    removed.push(index);
                    if !folders.contains(&folder) {
                        folders.push(folder);
                    }
                }
                None => {
                    kept[index] = true;
                    changed = true;
                }
            }
        }
        if !changed
            && !kept[0]
            && let Some(first) = kept.iter().position(|kept| *kept)
            && !same_first_segment(
                (&inner.segments[0], &probes[0]),
                (&inner.segments[first], &probes[first]),
            )
        {
            kept[0] = true;
            changed = true;
        }
        if !changed {
            break (removed, folders);
        }
    };
    if removed.is_empty() {
        return Err(Skip::NothingRemovable);
    }

    let mut new_meta = meta.clone();
    if let RecordingMetaInner::Studio(studio) = &mut new_meta.inner
        && let StudioRecordingMeta::MultipleSegments { inner } = studio.as_mut()
    {
        let mut index = 0;
        inner.segments.retain(|_| {
            let keep = !removed.contains(&index);
            index += 1;
            keep
        });
    }

    let mut new_config = config.clone();
    if let Some(timeline) = new_config.timeline.as_mut() {
        for clip in &mut timeline.segments {
            clip.recording_clip =
                remap_index(clip.recording_clip, &removed).ok_or(Skip::Invalid)?;
        }
    }
    new_config
        .clips
        .retain_mut(|clip| match remap_index(clip.index, &removed) {
            Some(index) => {
                clip.index = index;
                true
            }
            None => false,
        });
    if new_config.clips.is_empty() && !config.clips.is_empty() {
        new_config
            .clips
            .push(frameever_project::ClipConfiguration::default());
    }
    let old_durations: Vec<f64> = probes.iter().map(|probe| probe.duration).collect();
    let new_durations: Vec<f64> = old_durations
        .iter()
        .enumerate()
        .filter(|(index, _)| !removed.contains(index))
        .map(|(_, duration)| *duration)
        .collect();
    if let Some(captions) = new_config.captions.as_mut()
        && captions.source_timed
    {
        captions.segments = remap_captions(&captions.segments, &old_durations, &removed);
    }

    if new_config.validate().is_err() {
        return Err(Skip::Invalid);
    }
    if !same_result(config, &old_durations, &new_config, &new_durations) {
        return Err(Skip::EditChanged);
    }

    let markers = snapshot
        .project_file
        .as_deref()
        .map(storage::chapter_markers_in)
        .unwrap_or_default();
    let moved = remap_chapter_markers(&markers, &old_durations, &removed)
        .ok_or(Skip::ChapterMarkerOnCut)?;
    let chapter_markers = (moved != markers).then_some(ChapterMarkerChange {
        before: markers,
        after: moved,
    });

    Ok(Plan {
        removed,
        meta: new_meta,
        config: new_config,
        chapter_markers,
        folders,
    })
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Journal {
    backup: String,
    staging: String,
    folders: Vec<String>,
}

fn safe_relative(path: &str, root: &str) -> bool {
    path.starts_with(root)
        && Path::new(path)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

impl Journal {
    fn is_safe(&self) -> bool {
        safe_relative(&self.backup, BACKUPS)
            && safe_relative(&self.staging, STAGING)
            && self
                .folders
                .iter()
                .all(|folder| safe_relative(folder, SEGMENTS) && folder_name(folder).is_some())
    }
}

fn folder_name(folder: &str) -> Option<&str> {
    folder.rsplit('/').next().filter(|name| !name.is_empty())
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn is_plain_dir(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_dir() && !metadata.file_type().is_symlink()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn no_links_below(project: &Path, relative: &str) -> io::Result<bool> {
    let mut path = project.to_path_buf();
    for component in Path::new(relative).components() {
        path.push(component);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(false),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

fn ensure_no_links<'a>(project: &Path, paths: impl IntoIterator<Item = &'a str>) -> io::Result<()> {
    for relative in paths {
        if !no_links_below(project, relative)? {
            return Err(io::Error::other(format!(
                "{relative} goes through a link; the cleanup leaves it alone"
            )));
        }
    }
    Ok(())
}

fn exists_unfollowed(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

static STAGING_HOLDS: Active = Active {
    projects: Mutex::new(Vec::new()),
    released: Condvar::new(),
};

struct StagingGuard(PathBuf);

fn staging_lock(project: &Path) -> StagingGuard {
    let key = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let mut projects = STAGING_HOLDS
        .projects
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    while projects.contains(&key) {
        projects = STAGING_HOLDS
            .released
            .wait(projects)
            .unwrap_or_else(PoisonError::into_inner);
    }
    projects.push(key.clone());
    StagingGuard(key)
}

fn try_staging_lock(project: &Path) -> Option<StagingGuard> {
    let key = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let mut projects = STAGING_HOLDS
        .projects
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if projects.contains(&key) {
        return None;
    }
    projects.push(key.clone());
    Some(StagingGuard(key))
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        STAGING_HOLDS
            .projects
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|held| held != &self.0);
        STAGING_HOLDS.released.notify_all();
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn replace_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("invalid file name"))?;
    let temporary = path.with_file_name(format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if let Err(error) =
        write_new(&temporary, bytes).and_then(|()| std::fs::rename(&temporary, path))
    {
        std::fs::remove_file(&temporary).ok();
        return Err(error);
    }
    Ok(())
}

fn unique_dir(parent: &Path, base: &str) -> io::Result<(PathBuf, String)> {
    std::fs::create_dir_all(parent)?;
    for attempt in 1..1000 {
        let name = if attempt == 1 {
            base.to_string()
        } else {
            format!("{base} {attempt}")
        };
        let path = parent.join(&name);
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok((path, name)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("no free backup name"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Step {
    Backup,
    Journal,
    Config,
    Meta,
    Stage,
    Done,
}

impl Step {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 6] = [
        Self::Backup,
        Self::Journal,
        Self::Config,
        Self::Meta,
        Self::Stage,
        Self::Done,
    ];
}

pub(crate) fn commit(
    project: &Path,
    snapshot: &Snapshot,
    plan: &Plan,
    stamp: &str,
    label: &str,
    last: Step,
) -> io::Result<PathBuf> {
    if std::fs::read(project.join(META))? != snapshot.meta_bytes
        || std::fs::read(project.join(CONFIG))? != snapshot.config_bytes
        || read_optional(&project.join(PROJECT_FILE)).map_err(io::Error::other)?
            != snapshot.project_file
    {
        return Err(io::Error::other("project changed before cleanup"));
    }
    ensure_no_links(
        project,
        [STAGING, BACKUPS]
            .into_iter()
            .chain(plan.folders.iter().map(String::as_str)),
    )?;
    for folder in &plan.folders {
        if !is_plain_dir(&project.join(folder))? {
            return Err(io::Error::other("segment folder missing before cleanup"));
        }
    }

    let (backup, backup_name) = unique_dir(&project.join(BACKUPS), stamp)?;
    write_new(&backup.join(META), &snapshot.meta_bytes)?;
    write_new(&backup.join(CONFIG), &snapshot.config_bytes)?;
    write_new(
        &backup.join(NEXT_META),
        serde_json::to_string_pretty(&plan.meta)
            .map_err(io::Error::other)?
            .as_bytes(),
    )?;
    write_new(
        &backup.join(NEXT_CONFIG),
        serde_json::to_string_pretty(&plan.config)
            .map_err(io::Error::other)?
            .as_bytes(),
    )?;
    if let Some(change) = &plan.chapter_markers {
        write_new(
            &backup.join(CHAPTER_MARKERS),
            &serde_json::to_vec_pretty(&change.before).map_err(io::Error::other)?,
        )?;
        write_new(
            &backup.join(NEXT_CHAPTER_MARKERS),
            &serde_json::to_vec_pretty(&change.after).map_err(io::Error::other)?,
        )?;
    }
    sync_dir(&backup)?;
    sync_dir(&project.join(BACKUPS))?;
    if last == Step::Backup {
        return Ok(PathBuf::new());
    }

    let (staging, staging_name) = unique_dir(&project.join(STAGING), stamp)?;
    sync_dir(&project.join(STAGING))?;
    sync_dir(&project.join("content"))?;
    let journal = Journal {
        backup: format!("{BACKUPS}/{backup_name}"),
        staging: format!("{STAGING}/{staging_name}"),
        folders: plan.folders.clone(),
    };
    let journaled = replace_file(
        &project.join(JOURNAL),
        &serde_json::to_vec_pretty(&journal).map_err(io::Error::other)?,
    )
    .and_then(|()| sync_dir(project));
    if let Err(error) = journaled {
        std::fs::remove_dir(&staging).ok();
        std::fs::remove_dir(project.join(STAGING)).ok();
        return Err(error);
    }
    if last == Step::Journal {
        return Ok(staging);
    }

    let result = (|| {
        plan.config.write(project)?;
        if last == Step::Config {
            return Ok(());
        }
        plan.meta
            .save_for_project_with_retry()
            .map_err(|error| error.either(io::Error::other, |error| error))?;
        if let Some(change) = &plan.chapter_markers {
            write_chapter_markers(project, &change.after)?;
        }
        if last == Step::Meta {
            return Ok(());
        }
        let target = staging.join(label);
        std::fs::create_dir(&target)?;
        for folder in &plan.folders {
            let name = folder_name(folder).ok_or_else(|| io::Error::other("invalid folder"))?;
            std::fs::rename(project.join(folder), target.join(name))?;
        }
        sync_dir(&target)?;
        sync_dir(&staging)?;
        sync_dir(&project.join("content/segments"))?;
        if last == Step::Stage {
            return Ok(());
        }
        std::fs::remove_file(project.join(JOURNAL))?;
        sync_dir(project)
    })();
    if let Err(error) = result {
        if let Err(rollback_error) = roll_back(project) {
            tracing::error!(
                path = %project.display(),
                %rollback_error,
                "Unused media cleanup could not roll back; it will retry when the project opens"
            );
        }
        return Err(error);
    }
    Ok(staging)
}

pub(crate) fn roll_back(project: &Path) -> io::Result<bool> {
    let raw = match std::fs::read(project.join(JOURNAL)) {
        Ok(raw) => raw,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let journal: Journal = serde_json::from_slice(&raw).map_err(io::Error::other)?;
    if !journal.is_safe() {
        return Err(io::Error::other("unexpected paths in the cleanup journal"));
    }
    ensure_no_links(
        project,
        [journal.staging.as_str(), journal.backup.as_str()]
            .into_iter()
            .chain(journal.folders.iter().map(String::as_str)),
    )?;
    let staging = project.join(&journal.staging);
    let mut labels = Vec::new();
    match std::fs::read_dir(&staging) {
        Ok(entries) => {
            if !is_plain_dir(&staging)? {
                return Err(io::Error::other("cleanup staging is not a plain folder"));
            }
            for label in entries {
                let label = label?.path();
                if !is_plain_dir(&label)? {
                    return Err(io::Error::other(
                        "cleanup staging holds an unexpected entry",
                    ));
                }
                labels.push(label);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut restorable = true;
    for folder in &journal.folders {
        let name = folder_name(folder).ok_or_else(|| io::Error::other("invalid folder"))?;
        let mut found = is_plain_dir(&project.join(folder))?;
        for label in &labels {
            found |= is_plain_dir(&label.join(name))?;
        }
        restorable &= found;
    }
    let backup = project.join(&journal.backup);
    if !restorable {
        let config = std::fs::read(backup.join(NEXT_CONFIG))?;
        let meta = std::fs::read(backup.join(NEXT_META))?;
        replace_file(&project.join(CONFIG), &config)?;
        replace_file(&project.join(META), &meta)?;
        restore_chapter_markers(project, &backup.join(NEXT_CHAPTER_MARKERS))?;
        std::fs::remove_file(project.join(JOURNAL))?;
        sync_dir(project)?;
        tracing::warn!(path = %project.display(), "Finished an interrupted unused media cleanup whose media had gone");
        return Ok(true);
    }
    for label in &labels {
        for folder in &journal.folders {
            let Some(name) = folder_name(folder) else {
                continue;
            };
            let moved = label.join(name);
            let original = project.join(folder);
            if is_plain_dir(&moved)? && !exists_unfollowed(&original)? {
                std::fs::rename(&moved, &original)?;
            }
        }
        std::fs::remove_dir(label).ok();
    }
    for folder in &journal.folders {
        if !is_plain_dir(&project.join(folder))? {
            return Err(io::Error::other(format!(
                "{folder} could not be put back; the cleanup journal is kept"
            )));
        }
    }
    let mut parents: Vec<PathBuf> = journal
        .folders
        .iter()
        .filter_map(|folder| project.join(folder).parent().map(Path::to_path_buf))
        .collect();
    parents.dedup();
    for parent in &parents {
        sync_dir(parent)?;
    }
    if is_plain_dir(&staging)? {
        sync_dir(&staging)?;
    }
    replace_file(&project.join(CONFIG), &std::fs::read(backup.join(CONFIG))?)?;
    replace_file(&project.join(META), &std::fs::read(backup.join(META))?)?;
    restore_chapter_markers(project, &backup.join(CHAPTER_MARKERS))?;
    std::fs::remove_dir(&staging).ok();
    std::fs::remove_dir(project.join(STAGING)).ok();
    std::fs::remove_file(project.join(JOURNAL))?;
    sync_dir(project)?;
    tracing::warn!(path = %project.display(), "Rolled back an interrupted unused media cleanup");
    Ok(true)
}

fn write_chapter_markers(project: &Path, markers: &[ChapterMarker]) -> io::Result<()> {
    let path = project.join(PROJECT_FILE);
    let current = std::fs::read(&path)?;
    replace_file(&path, &storage::with_chapter_markers(&current, markers)?)
}

fn restore_chapter_markers(project: &Path, backup: &Path) -> io::Result<()> {
    match std::fs::read(backup) {
        Ok(raw) => write_chapter_markers(
            project,
            &serde_json::from_slice::<Vec<ChapterMarker>>(&raw).map_err(io::Error::other)?,
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

struct Active {
    projects: Mutex<Vec<PathBuf>>,
    released: Condvar,
}

static ACTIVE: Active = Active {
    projects: Mutex::new(Vec::new()),
    released: Condvar::new(),
};

fn active() -> MutexGuard<'static, Vec<PathBuf>> {
    ACTIVE
        .projects
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
static SETTLE_WAITING: Active = Active {
    projects: Mutex::new(Vec::new()),
    released: Condvar::new(),
};

pub(crate) struct Claim(PathBuf);

impl Claim {
    pub(crate) fn take(project: &Path) -> Option<Self> {
        let mut projects = active();
        if projects.iter().any(|held| held == project) {
            return None;
        }
        projects.push(project.to_path_buf());
        Some(Self(project.to_path_buf()))
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        active().retain(|held| held != &self.0);
        ACTIVE.released.notify_all();
    }
}

pub(crate) fn settle(project: &Path) -> Result<(), String> {
    let key = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let started = std::time::Instant::now();
    let deadline = started + SETTLE_WAIT;
    let mut projects = active();
    let waited = projects.contains(&key);
    while projects.contains(&key) {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err("the unused clip media cleanup is still running".into());
        }
        #[cfg(test)]
        {
            let mut waiting = SETTLE_WAITING
                .projects
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            waiting.push(key.clone());
            SETTLE_WAITING.released.notify_all();
        }
        projects = ACTIVE
            .released
            .wait_timeout(projects, left)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
    if waited {
        tracing::info!(path = %key.display(), waited_ms = started.elapsed().as_millis() as u64, "Waited for an unused clip media cleanup before reading the project");
    }
    if !exists_unfollowed(&key.join(JOURNAL)).map_err(|error| error.to_string())? {
        return Ok(());
    }
    projects.push(key.clone());
    drop(projects);
    let _claim = Claim(key.clone());
    let _staging = staging_lock(&key);
    roll_back(&key).map(drop).map_err(|error| {
        tracing::error!(path = %key.display(), %error, "Could not roll back an interrupted unused media cleanup");
        format!("an interrupted unused clip media cleanup could not be undone: {error}")
    })
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return std::fs::symlink_metadata(path)
            .map(|meta| meta.len())
            .unwrap_or(0);
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => directory_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|meta| meta.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

#[cfg(target_os = "macos")]
fn trash_context() -> trash::TrashContext {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut context = trash::TrashContext::default();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context
}

#[cfg(not(target_os = "macos"))]
fn trash_context() -> trash::TrashContext {
    trash::TrashContext::default()
}

#[cfg(windows)]
pub(crate) fn has_recycle_bin(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW};
    const DRIVE_FIXED: u32 = 3;
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return false;
    };
    let drive = match prefix.kind() {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
        _ => return false,
    };
    let rest: PathBuf = path
        .components()
        .skip_while(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
        .collect();
    let plain = PathBuf::from(format!("{}:\\", drive as char)).join(rest);
    let wide: Vec<u16> = plain.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut root = vec![0u16; 32_768];
    let found = unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) };
    if found == 0 {
        root = format!("{}:\\", drive as char)
            .encode_utf16()
            .chain(Some(0))
            .collect();
    }
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_FIXED }
}

pub(crate) fn move_to_trash(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    if !has_recycle_bin(path) {
        return Err("this drive has no Recycle Bin, so the media stays in the project".into());
    }
    trash_context()
        .delete(path)
        .map_err(|error| error.to_string())
}

pub(crate) fn trash_staged(project: &Path, trash: &dyn Fn(&Path) -> Result<(), String>) -> bool {
    let _staging = staging_lock(project);
    let staging = project.join(STAGING);
    if !matches!(is_plain_dir(&project.join("content")), Ok(true))
        || !matches!(is_plain_dir(&staging), Ok(true))
        || exists_unfollowed(&project.join(JOURNAL)).unwrap_or(true)
    {
        return !exists_unfollowed(&staging).unwrap_or(true);
    }
    let Ok(root) = staging.canonicalize() else {
        return false;
    };
    let Ok(stamps) = std::fs::read_dir(&staging) else {
        return false;
    };
    let mut done = true;
    for stamp in stamps {
        let Ok(stamp) = stamp.map(|stamp| stamp.path()) else {
            done = false;
            continue;
        };
        if !matches!(is_plain_dir(&stamp), Ok(true)) {
            tracing::warn!(path = %stamp.display(), "Left an unexpected entry in the removed clip media folder");
            done = false;
            continue;
        }
        let Ok(labels) = std::fs::read_dir(&stamp) else {
            done = false;
            continue;
        };
        for label in labels {
            let Ok(label) = label.map(|label| label.path()) else {
                done = false;
                continue;
            };
            let inside = matches!(is_plain_dir(&label), Ok(true))
                && label
                    .canonicalize()
                    .is_ok_and(|resolved| resolved.starts_with(&root));
            if !inside {
                tracing::warn!(path = %label.display(), "Left an unexpected entry in the removed clip media folder");
                done = false;
                continue;
            }
            if let Err(error) = trash(&label) {
                tracing::warn!(path = %label.display(), %error, "Could not move unused clip media to the trash");
                done = false;
            }
        }
        std::fs::remove_dir(&stamp).ok();
    }
    std::fs::remove_dir(&staging).ok();
    done
}

pub(crate) fn trash_leftovers(project: PathBuf) {
    if !exists_unfollowed(&project.join(STAGING)).unwrap_or(false) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("unused-media".into())
        .spawn(move || {
            trash_staged(&project, &move_to_trash);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "Could not start trashing unused clip media");
    }
}

fn now_stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H.%M.%S").to_string()
}

fn trash_label(pretty_name: &str) -> String {
    let name: String = sanitize_filename::sanitize(pretty_name.trim())
        .chars()
        .take(80)
        .collect();
    let name = if name.trim().is_empty() {
        "FrameEver".to_string()
    } else {
        name
    };
    sanitize_filename::sanitize(t!("{name} (removed clips)", name = name))
}

#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    Skipped(Skip),
    Cleaned { removed: Vec<usize>, bytes: u64 },
}

#[cfg(windows)]
fn folder_in_use(folder: &Path) -> io::Result<bool> {
    use std::os::windows::fs::OpenOptionsExt;
    const DELETE: u32 = 0x0001_0000;
    const SHARE_ALL: u32 = 0x7;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let mut pending = vec![folder.to_path_buf()];
    while let Some(path) = pending.pop() {
        if std::fs::symlink_metadata(&path)?.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        }
        let opened = std::fs::OpenOptions::new()
            .access_mode(DELETE)
            .share_mode(SHARE_ALL)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(&path);
        match opened {
            Ok(_) => {}
            Err(error) if matches!(error.raw_os_error(), Some(5 | 32 | 33)) => return Ok(true),
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

#[cfg(not(windows))]
fn folder_in_use(_folder: &Path) -> io::Result<bool> {
    Ok(false)
}

fn wait_for_media(
    project: &Path,
    folders: &[String],
    max_wait: Duration,
    in_use: &dyn Fn(&Path) -> io::Result<bool>,
) -> io::Result<bool> {
    let deadline = std::time::Instant::now() + max_wait;
    loop {
        let mut busy = false;
        for folder in folders {
            if in_use(&project.join(folder))? {
                busy = true;
                break;
            }
        }
        if !busy {
            return Ok(true);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub(crate) fn wait_until_free(project: &Path, request: &Request) -> bool {
    let Ok(snapshot) = Snapshot::read(project) else {
        return true;
    };
    let Ok(plan) = plan(&snapshot, &request.probes) else {
        return true;
    };
    wait_for_media(project, &plan.folders, MEDIA_RELEASE_WAIT, &folder_in_use).unwrap_or(true)
}

pub(crate) fn run(project: &Path, request: &Request) -> Result<Outcome, String> {
    run_waiting(project, request, COMMIT_RELEASE_WAIT, &folder_in_use)
}

pub(crate) fn run_waiting(
    project: &Path,
    request: &Request,
    max_wait: Duration,
    in_use: &dyn Fn(&Path) -> io::Result<bool>,
) -> Result<Outcome, String> {
    let Some(_staging) = try_staging_lock(project) else {
        return Ok(Outcome::Skipped(Skip::TrashInProgress));
    };
    roll_back(project).map_err(|error| error.to_string())?;
    let snapshot = Snapshot::read(project)?;
    let plan = match plan(&snapshot, &request.probes) {
        Ok(plan) => plan,
        Err(skip) => return Ok(Outcome::Skipped(skip)),
    };
    let stamp = now_stamp();
    let label = trash_label(&snapshot.meta.pretty_name);
    if !wait_for_media(project, &plan.folders, max_wait, in_use)
        .map_err(|error| error.to_string())?
    {
        return Ok(Outcome::Skipped(Skip::MediaInUse));
    }
    let staged = commit(project, &snapshot, &plan, &stamp, &label, Step::Done)
        .map_err(|error| error.to_string())?;
    Ok(Outcome::Cleaned {
        removed: plan.removed,
        bytes: directory_size(&staged),
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Busy {
    pub open_in_editor: bool,
    pub deleting: bool,
    pub recording: bool,
    pub exporting: bool,
}

impl Busy {
    pub(crate) fn reason(self) -> Option<&'static str> {
        if self.open_in_editor {
            Some("open in another editor")
        } else if self.deleting {
            Some("being deleted")
        } else if self.recording {
            Some("recording or finalizing")
        } else if self.exporting {
            Some("export in progress")
        } else {
            None
        }
    }
}
