//! Caption transcription -- the gpui port of the Tauri binary's `captions.rs`
//! model management and transcription pipeline (Whisper + Parakeet), driving
//! the editor's Captions tab.
//!
//! Three pieces, all mirrored from the shipping app so the two binaries share
//! one model library on disk:
//!
//! * **models** -- the catalogue lives in `editor_tabs.rs` (`CAPTION_MODELS`);
//!   the files live in `{appLocalDataDir}/transcription_models/`, the exact
//!   directory the Tauri app resolves (`captions.ts:25,529-535`), so a model
//!   either app downloads is downloaded for both.
//! * **downloads** -- `download_whisper_model` / `download_parakeet_model`
//!   from `captions.rs:2062-2573`, streamed through reqwest with the same URL
//!   tables, part sizes and staging-directory finalisation. Status lives in a
//!   process-global [`Hub`], the stand-in for Tauri's `MODEL_DOWNLOADS` map +
//!   `DownloadProgress` events: the tab polls it instead of listening.
//! * **transcribe** -- `transcribe_audio` (`captions.rs:1315-1439`): mix the
//!   bundle's per-segment mic/system audio to 16kHz mono WAV via ffmpeg, then
//!   run whisper-rs or parakeet-rs with word timestamps and chunk the words
//!   into caption segments. `apply_caption_result` is
//!   `applyCaptionResultToProject` (`captions.ts:469-527`) plus the
//!   `deriveCaptionTrackSegments` projection (`captions.ts:323-366`) that the
//!   Tauri app runs in TypeScript.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use frameever_editor::{
    TranscriptionAudioSource, TranscriptionAudioTake, append_transcription_audio,
};
use frameever_project::{
    CaptionSegment, CaptionSettings, CaptionTrackSegment, CaptionWord, CaptionsData,
    ProjectConfiguration, RecordingMeta, StudioRecordingMeta, TimelineConfiguration,
    TimelineSegment, caption_text_from_words,
    filler_words::{self, filler_language},
    is_cjk, is_cjk_punctuation,
};
use frameever_rendering::Video;

use crate::i18n::t;
use ffmpeg::{
    ChannelLayout, codec as avcodec,
    format::{self as avformat},
    software::resampling,
};
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
use parakeet_rs::{ParakeetTDT, TimestampMode, Transcriber};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

use crate::{editor_timeline::clip_timeline_offsets, store};

pub(crate) mod fetch;
mod verbatim;

// ---------------------------------------------------------------------------
// Model catalogue & shared paths
// ---------------------------------------------------------------------------

/// Every model the catalogue can name, for the disk scan.
const MODEL_NAMES: [&str; 4] = ["best", "best-max", "small", "medium"];

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const PARAKEET_UNSUPPORTED_MESSAGE: &str = "Parakeet transcription is not available on Intel macOS";

/// The section of the shared tauri-plugin-store file this app parks its own
/// keys in; the Tauri app ignores sections it does not know. The two key names
/// mirror the `localStorage` keys `CaptionsTab.tsx` persists under.
pub const GPUI_STORE_SECTION: &str = "gpui";
pub const SELECTED_MODEL_KEY: &str = "selectedTranscriptionModel";
pub const SELECTED_LANGUAGE_KEY: &str = "selectedTranscriptionLanguage";

pub fn supports_parakeet() -> bool {
    parakeet_built() && onnx_runtime_loads()
}

pub fn parakeet_built() -> bool {
    cfg!(not(all(target_os = "macos", target_arch = "x86_64")))
}

fn onnx_runtime_loads() -> bool {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        static LOADS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *LOADS.get_or_init(|| {
            let path = frameever_camera_effects::onnx_runtime_library_path();
            let needs = path.as_deref().map(|path| {
                let mut head = Vec::new();
                File::open(path)
                    .and_then(|file| file.take(64 * 1024).read_to_end(&mut head))
                    .map_err(|_| ())
                    .and_then(|_| macos_needed_by_dylib(&head))
            });
            let running = crate::permissions::macos::product_version()
                .as_deref()
                .and_then(parse_macos_version);
            let loads = match needs {
                Some(Ok(Some(needs))) => running.is_none_or(|running| running >= needs),
                Some(Ok(None) | Err(())) | None => true,
            };
            if loads {
                tracing::info!(?needs, ?running, "ONNX Runtime runs on this macOS");
            } else {
                tracing::warn!(
                    ?path,
                    ?needs,
                    ?running,
                    "ONNX Runtime cannot load on this macOS: Whisper transcribes, without Parakeet or speaker labels"
                );
            }
            loads
        })
    }
    #[cfg(not(all(target_os = "macos", not(test))))]
    {
        true
    }
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_version(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version
        .trim()
        .split('.')
        .map(|part| part.parse::<u32>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

#[cfg(any(target_os = "macos", test))]
fn macos_needed_by_dylib(bytes: &[u8]) -> Result<Option<(u32, u32, u32)>, ()> {
    const MH_MAGIC_64: u32 = 0xfeed_facf;
    const FAT_MAGIC: u32 = 0xcafe_babe;
    const LC_VERSION_MIN_MACOSX: u32 = 0x24;
    const LC_BUILD_VERSION: u32 = 0x32;
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    match word(0) {
        Some(MH_MAGIC_64) => {}
        Some(magic) if magic.swap_bytes() == FAT_MAGIC => return Ok(None),
        _ => return Err(()),
    }
    let commands = word(16).ok_or(())?;
    let unpack = |v: u32| (v >> 16, (v >> 8) & 0xff, v & 0xff);
    let mut at = 32;
    for _ in 0..commands {
        let (Some(cmd), Some(size)) = (word(at), word(at + 4)) else {
            break;
        };
        match cmd {
            LC_BUILD_VERSION => return Ok(word(at + 12).map(unpack)),
            LC_VERSION_MIN_MACOSX => return Ok(word(at + 8).map(unpack)),
            _ => {}
        }
        if size < 8 {
            break;
        }
        at += size as usize;
    }
    Ok(None)
}

/// `PARAKEET_DIR_MODELS` (`captions.ts:26`).
pub fn is_parakeet_model(model: &str) -> bool {
    matches!(model, "best" | "best-max")
}

const PARAKEET_LANGUAGES: [&str; 25] = [
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

const WHISPER_MODELS_LARGEST_FIRST: [&str; 2] = ["medium", "small"];

const PARAKEET_UNSUPPORTED_LANGUAGE: &str = "Parakeet cannot transcribe language";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptionEngine {
    model: String,
    language: String,
}

fn caption_engine(
    model: &str,
    language: &str,
    downloaded: impl Fn(&str) -> bool,
    detect_language: impl FnOnce(&str) -> Option<String>,
) -> Result<CaptionEngine, String> {
    let engine = |model: &str, language: &str| CaptionEngine {
        model: model.to_string(),
        language: language.to_string(),
    };
    if !is_parakeet_model(model) || PARAKEET_LANGUAGES.contains(&language) {
        return Ok(engine(model, language));
    }
    let whisper = WHISPER_MODELS_LARGEST_FIRST
        .into_iter()
        .find(|whisper| downloaded(whisper));
    if language == "auto" {
        return Ok(whisper
            .and_then(|whisper| {
                detect_language(whisper)
                    .filter(|detected| !PARAKEET_LANGUAGES.contains(&detected.as_str()))
                    .map(|detected| engine(whisper, &detected))
            })
            .unwrap_or_else(|| engine(model, language)));
    }
    whisper
        .map(|whisper| engine(whisper, language))
        .ok_or_else(|| format!("{PARAKEET_UNSUPPORTED_LANGUAGE} {language}"))
}

pub fn models_dir() -> PathBuf {
    frameever_utils::storage::caption_models_dir()
}

/// `getModelPath` (`captions.ts:529-535`): Parakeet models are directories
/// named `parakeet-{model}`, Whisper models are single `{model}.bin` files.
pub fn model_path(model: &str) -> PathBuf {
    if is_parakeet_model(model) {
        models_dir().join(format!("parakeet-{model}"))
    } else {
        models_dir().join(format!("{model}.bin"))
    }
}

/// `check_model_exists` / `check_parakeet_model_exists`
/// (`captions.rs:2223-2226,2579-2597`).
fn model_files_present(model: &str) -> bool {
    let path = model_path(model);
    if !is_parakeet_model(model) {
        return path.exists();
    }
    supports_parakeet() && parakeet_model_installed(&path)
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn parakeet_model_installed(dir: &Path) -> bool {
    parakeet_model_files_for_dir(dir)
        .iter()
        .all(|(filename, _)| dir.join(filename).is_file())
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn parakeet_model_installed(_dir: &Path) -> bool {
    false
}

fn parakeet_model_outdated(dir: &Path) -> bool {
    dir.join("encoder-model.int8.onnx").is_file() && !parakeet_model_installed(dir)
}

// ---------------------------------------------------------------------------
// Shared UI state -- the stand-in for Tauri's MODEL_DOWNLOADS + events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Downloading,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ModelDownload {
    pub model: String,
    pub state: DownloadState,
    pub progress: f64,
    pub message: String,
}

#[derive(Default)]
struct Hub {
    /// One download at a time -- the tab disables its button while one runs,
    /// so Tauri's per-path map collapses to a single slot here.
    download: Option<ModelDownload>,
    /// `None` until the first disk scan.
    downloaded: Option<HashSet<String>>,
    deleting: Option<String>,
    generating: HashSet<PathBuf>,
    generation_errors: HashMap<PathBuf, String>,
    no_speech: HashSet<PathBuf>,
    recommended_outdated: bool,
}

impl Hub {
    fn downloading(&self, model: &str) -> bool {
        self.download.as_ref().is_some_and(|download| {
            download.model == model && download.state == DownloadState::Downloading
        })
    }

    fn work_in_flight(&self) -> bool {
        self.download
            .as_ref()
            .is_some_and(|download| download.state == DownloadState::Downloading)
            || !self.generating.is_empty()
            || self.deleting.is_some()
    }
}

static HUB: LazyLock<Mutex<Hub>> = LazyLock::new(|| Mutex::new(Hub::default()));

fn hub() -> std::sync::MutexGuard<'static, Hub> {
    HUB.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Everything `render_captions_tab` reads, snapshotted under one lock.
pub struct CaptionsUiSnapshot {
    pub download: Option<ModelDownload>,
    pub downloaded: HashSet<String>,
    pub deleting: Option<String>,
    pub generating: bool,
    pub generation_error: Option<String>,
    pub no_speech: bool,
    pub recommended_outdated: bool,
}

pub fn ui_snapshot(project_path: &Path) -> CaptionsUiSnapshot {
    let hub = hub();
    CaptionsUiSnapshot {
        download: hub.download.clone(),
        downloaded: hub.downloaded.clone().unwrap_or_default(),
        deleting: hub.deleting.clone(),
        generating: hub.generating.contains(project_path),
        generation_error: hub.generation_errors.get(project_path).cloned(),
        no_speech: hub.no_speech.contains(project_path),
        recommended_outdated: hub.recommended_outdated,
    }
}

pub fn download_active() -> bool {
    hub()
        .download
        .as_ref()
        .is_some_and(|download| download.state == DownloadState::Downloading)
}

pub fn work_in_flight() -> bool {
    hub().work_in_flight()
}

/// Rescan the catalogue against the disk -- `refreshDownloadedModels`
/// (`CaptionsTab.tsx:481-492`), minus the async round-trips.
pub fn refresh_downloaded_models() {
    let downloaded: HashSet<String> = MODEL_NAMES
        .iter()
        .filter(|model| model_files_present(model))
        .map(|model| (*model).to_string())
        .collect();
    let recommended_outdated = supports_parakeet() && parakeet_model_outdated(&model_path("best"));
    let mut hub = hub();
    hub.downloaded = Some(downloaded);
    hub.recommended_outdated = recommended_outdated;
}

pub fn begin_download(model: &str) -> bool {
    let mut hub = hub();
    if hub
        .download
        .as_ref()
        .is_some_and(|download| download.state == DownloadState::Downloading)
    {
        return false;
    }
    hub.download = Some(ModelDownload {
        model: model.to_string(),
        state: DownloadState::Downloading,
        progress: 0.0,
        message: t!("Preparing model download").to_string(),
    });
    true
}

fn set_download_progress(model: &str, progress: f64, message: String) {
    let mut hub = hub();
    if let Some(download) = hub.download.as_mut()
        && download.model == model
        && download.state == DownloadState::Downloading
    {
        download.progress = progress.clamp(0.0, 100.0);
        download.message = message;
    }
}

/// The whole download, run on the tokio runtime; final status lands in the
/// hub the way Tauri's spawned task lands it in `MODEL_DOWNLOADS`
/// (`captions.rs:2079-2100`).
pub async fn run_model_download(model: String) {
    let result = if is_parakeet_model(&model) {
        download_parakeet_model(&model).await
    } else {
        download_whisper_model(&model).await
    };

    let status = match &result {
        Ok(()) => ModelDownload {
            model: model.clone(),
            state: DownloadState::Completed,
            progress: 100.0,
            message: t!("Download complete").to_string(),
        },
        Err(error) => {
            tracing::error!(model, "caption model download failed: {error}");
            ModelDownload {
                model: model.clone(),
                state: DownloadState::Failed,
                progress: 0.0,
                message: t!("Download failed: {error}", error = error),
            }
        }
    };
    hub().download = Some(status);
    refresh_downloaded_models();
}

pub fn begin_delete(model: &str) -> bool {
    let mut hub = hub();
    if hub.deleting.is_some() {
        return false;
    }
    hub.deleting = Some(model.to_string());
    true
}

/// `delete_whisper_model` / `delete_parakeet_model`
/// (`captions.rs:2231-2245,2613-2628`), run on a background thread.
pub fn run_model_delete(model: &str) {
    if let Err(error) = delete_model_files(model) {
        tracing::warn!(model, "failed to delete caption model: {error}");
    }
    {
        let mut hub = hub();
        if hub.deleting.as_deref() == Some(model) {
            hub.deleting = None;
        }
        // A finished download record for this model no longer matches disk.
        if hub
            .download
            .as_ref()
            .is_some_and(|d| d.model == model && d.state != DownloadState::Downloading)
        {
            hub.download = None;
        }
    }
    refresh_downloaded_models();
}

fn delete_model_files(model: &str) -> Result<(), String> {
    let path = model_path(model);
    if !path.exists() {
        return Err(format!("Model file not found: {}", path.display()));
    }
    if is_parakeet_model(model) {
        invalidate_parakeet_cache_for_dir(&path);
        std::fs::remove_dir_all(&path).map_err(|e| format!("Failed to delete model directory: {e}"))
    } else {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete model file: {e}"))?;
        invalidate_whisper_cache_for_path(&path);
        Ok(())
    }
}

pub fn begin_generation(project_path: &Path) -> bool {
    let mut hub = hub();
    if hub.generating.contains(project_path) {
        return false;
    }
    hub.generation_errors.remove(project_path);
    hub.no_speech.remove(project_path);
    hub.generating.insert(project_path.to_path_buf());
    true
}

pub fn finish_generation(project_path: &Path, error: Option<String>) {
    let mut hub = hub();
    hub.generating.remove(project_path);
    hub.no_speech.remove(project_path);
    match error {
        Some(message) => {
            hub.generation_errors
                .insert(project_path.to_path_buf(), message);
        }
        None => {
            hub.generation_errors.remove(project_path);
        }
    }
}

pub fn finish_generation_without_speech(project_path: &Path) {
    let mut hub = hub();
    hub.generating.remove(project_path);
    hub.generation_errors.remove(project_path);
    hub.no_speech.insert(project_path.to_path_buf());
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptionError {
    NoSpeech,
    Failed(String),
}

impl std::fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSpeech => f.write_str(t!("No speech detected in the audio")),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

fn heard_segments(
    result: Result<Vec<CaptionSegment>, String>,
) -> Result<Vec<CaptionSegment>, TranscriptionError> {
    let segments = result.map_err(TranscriptionError::Failed)?;
    if segments.is_empty() {
        return Err(TranscriptionError::NoSpeech);
    }
    Ok(segments)
}

// ---------------------------------------------------------------------------
// Downloads
// ---------------------------------------------------------------------------

/// `download_whisper_model_to_path` (`captions.rs:2105-2196`), restricted to
/// the two Whisper entries the catalogue offers.
async fn download_whisper_model(model: &str) -> Result<(), String> {
    let model_parts: &[&str] = match model {
        "small" => &[
            "https://github.com/CapSoftware/transcription-models/releases/download/whisper-v1/ggml-small.bin",
        ],
        "medium" => &[
            "https://github.com/CapSoftware/transcription-models/releases/download/whisper-v1/ggml-medium.bin.part0",
            "https://github.com/CapSoftware/transcription-models/releases/download/whisper-v1/ggml-medium.bin.part1",
        ],
        _ => return Err(format!("Unknown Whisper model: {model}")),
    };

    let path = model_path(model);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directories: {e}"))?;
    }

    let total_size = total_content_length(model_parts).await;
    let have = whisper_resumable_bytes(&path, model_parts.len(), total_size);
    if let Some(dir) = path.parent()
        && let Some(missing) = fetch::missing_space(dir, total_size.saturating_sub(have))
    {
        return Err(no_space_message(missing));
    }

    let result = write_whisper_parts(&fetch::network, model, model_parts, &path, total_size).await;
    if result.is_err() && model_parts.len() > 1 {
        let _ = std::fs::remove_file(whisper_staging_path(&path));
    }
    result
}

fn no_space_message(missing: u64) -> String {
    t!(
        "Not enough free space. Free up {size} and try again",
        size = fetch::size_label(missing)
    )
    .to_string()
}

fn whisper_staging_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".downloading");
    PathBuf::from(name)
}

fn whisper_resumable_bytes(path: &Path, parts: usize, total_size: u64) -> u64 {
    let len = std::fs::metadata(whisper_staging_path(path)).map_or(0, |m| m.len());
    match (parts, total_size) {
        (1, 0) => len,
        (1, total) if len <= total => len,
        _ => 0,
    }
}

async fn write_whisper_parts(
    open: &fetch::Opener,
    model: &str,
    model_parts: &[&str],
    path: &Path,
    total_size: u64,
) -> Result<(), String> {
    let staging = whisper_staging_path(path);
    let have = whisper_resumable_bytes(path, model_parts.len(), total_size);
    let (mut file, on_disk) =
        fetch::reopen(&staging).map_err(|e| format!("Failed to create file: {e}"))?;
    if on_disk != have {
        fetch::restart(&mut file).map_err(|e| format!("Failed to create file: {e}"))?;
    }

    let mut downloaded: u64 = have;
    let part_count = model_parts.len() as f64;

    for (idx, url) in model_parts.iter().enumerate() {
        let mut downloaded_part: u64 = 0;
        let part_size = if model_parts.len() == 1 {
            total_size
        } else {
            0
        };
        let from = if idx == 0 { have } else { 0 };
        let base = downloaded.saturating_sub(from);
        let mut on_progress = |event: fetch::Progress<'_>| {
            match event {
                fetch::Progress::Bytes(chunk) => {
                    downloaded = downloaded.saturating_add(chunk.len() as u64);
                    downloaded_part = downloaded_part.saturating_add(chunk.len() as u64);
                }
                fetch::Progress::Rewound(bytes) => {
                    downloaded = downloaded.saturating_sub(bytes);
                    downloaded_part = downloaded_part.saturating_sub(bytes);
                }
            }
            let progress = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else if part_size > 0 {
                ((idx as f64 + downloaded_part as f64 / part_size as f64) / part_count) * 100.0
            } else {
                (idx as f64 / part_count) * 100.0
            };

            set_download_progress(
                model,
                progress,
                t!(
                    "Downloading model: {progress}%",
                    progress = format!("{progress:.0}")
                ),
            );
        };
        fetch::part(
            open,
            fetch::Part {
                url,
                base,
                have: from,
            },
            &mut file,
            fetch::Patience::default(),
            &mut on_progress,
        )
        .await
        .map_err(|e| format!("Failed to download model: {e}"))?;
    }

    file.flush()
        .map_err(|e| format!("Failed to flush file: {e}"))?;
    drop(file);
    if total_size > 0 && std::fs::metadata(&staging).map_or(0, |m| m.len()) != total_size {
        let _ = std::fs::remove_file(&staging);
        return Err("Downloaded model file did not match the expected size".to_string());
    }
    std::fs::rename(&staging, path).map_err(|e| format!("Failed to install model: {e}"))
}

/// `total_content_length` (`captions.rs:2198-2218`).
async fn total_content_length(urls: &[&str]) -> u64 {
    let mut total: u64 = 0;
    for url in urls {
        let Ok(resp) = crate::http::get(url, Duration::from_secs(30)).await else {
            return 0;
        };
        if !(200..300).contains(&resp.status()) {
            return 0;
        }
        match resp.content_length() {
            Some(size) => total = total.saturating_add(size),
            None => return 0,
        }
    }
    total
}

/// `PARAKEET_TDT_INT8_MODEL_FILES` (`captions.rs:2249-2268`).
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
const PARAKEET_TDT_INT8_MODEL_FILES: &[(&str, &[&str])] = &[
    (
        "encoder-model.onnx",
        &[
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v2/encoder-model.q8.onnx",
        ],
    ),
    (
        "decoder_joint-model.int8.onnx",
        &[
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v1/decoder_joint-model.int8.onnx",
        ],
    ),
    (
        "vocab.txt",
        &[
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v1/vocab.txt",
        ],
    ),
];

/// `PARAKEET_TDT_FULL_MODEL_FILES` (`captions.rs:2270-2296`).
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
const PARAKEET_TDT_FULL_MODEL_FILES: &[(&str, &[&str])] = &[
    (
        "encoder-model.onnx",
        &[
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v1/encoder-model.onnx",
        ],
    ),
    (
        "encoder-model.onnx.data",
        &[
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v1/encoder-model.onnx.data.part0",
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v1/encoder-model.onnx.data.part1",
        ],
    ),
    (
        "decoder_joint-model.onnx",
        &[
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v1/decoder_joint-model.onnx",
        ],
    ),
    (
        "vocab.txt",
        &[
            "https://github.com/CapSoftware/transcription-models/releases/download/parakeet-tdt-v1/vocab.txt",
        ],
    ),
];

/// `PARAKEET_MODEL_CLEANUP_FILES` (`captions.rs:2298-2306`).
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
const PARAKEET_MODEL_CLEANUP_FILES: &[&str] = &[
    "encoder-model.onnx",
    "encoder-model.onnx.data",
    "decoder_joint-model.onnx",
    "encoder-model.int8.onnx",
    "decoder_joint-model.int8.onnx",
    "nemo128.onnx",
    "vocab.txt",
];

/// `PARAKEET_KNOWN_PART_SIZES` (`captions.rs:2308-2316`).
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
const PARAKEET_KNOWN_PART_SIZES: &[(&str, u64)] = &[
    ("encoder-model.q8.onnx", 895_802_223),
    ("decoder_joint-model.int8.onnx", 18_202_004),
    ("encoder-model.onnx", 41_770_866),
    ("encoder-model.onnx.data.part0", 1_300_000_000),
    ("encoder-model.onnx.data.part1", 1_135_420_160),
    ("decoder_joint-model.onnx", 72_520_893),
    ("vocab.txt", 93_939),
];

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn parakeet_known_part_size(url: &str) -> Option<u64> {
    PARAKEET_KNOWN_PART_SIZES
        .iter()
        .find_map(|(name, size)| url.ends_with(name).then_some(*size))
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn parakeet_model_files_for_dir(
    output_dir: &Path,
) -> &'static [(&'static str, &'static [&'static str])] {
    match output_dir.file_name().and_then(|name| name.to_str()) {
        Some("parakeet-best-max") => PARAKEET_TDT_FULL_MODEL_FILES,
        _ => PARAKEET_TDT_INT8_MODEL_FILES,
    }
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn parakeet_staging_dir(validated_dir: &Path) -> PathBuf {
    validated_dir.with_file_name(format!(
        "{}.downloading",
        validated_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("model")
    ))
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
async fn parakeet_model_file_sizes(
    model_files: &'static [(&'static str, &'static [&'static str])],
) -> Result<Vec<(&'static str, u64)>, String> {
    let mut sizes = Vec::with_capacity(model_files.len());
    for (filename, urls) in model_files {
        let mut file_size = 0_u64;
        for url in *urls {
            let resp = crate::http::get(url, Duration::from_secs(30))
                .await
                .map_err(|e| format!("Failed to get size for {filename}: {e}"))?;

            if !(200..300).contains(&resp.status()) {
                return Err(format!(
                    "Failed to get size for {filename}: HTTP {}",
                    resp.status()
                ));
            }

            let part_size = resp
                .content_length()
                .filter(|size| *size > 0)
                .or_else(|| parakeet_known_part_size(url))
                .unwrap_or(0);
            file_size = file_size.saturating_add(part_size);
        }
        sizes.push((*filename, file_size));
    }
    Ok(sizes)
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn parakeet_model_files_match(dir: &Path, expected_files: &[(&str, u64)]) -> bool {
    expected_files.iter().all(|(filename, expected_size)| {
        let Ok(metadata) = std::fs::metadata(dir.join(filename)) else {
            return false;
        };
        metadata.is_file() && (*expected_size == 0 || metadata.len() == *expected_size)
    })
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn finalize_parakeet_model_download(
    validated_dir: &Path,
    staging_dir: &Path,
    model_files: &'static [(&'static str, &'static [&'static str])],
) -> Result<(), String> {
    std::fs::create_dir_all(validated_dir)
        .map_err(|e| format!("Failed to create model directory: {e}"))?;

    for filename in PARAKEET_MODEL_CLEANUP_FILES {
        let file_path = validated_dir.join(filename);
        if file_path.exists() {
            let _ = std::fs::remove_file(&file_path);
        }
    }

    for (filename, _) in model_files {
        let src = staging_dir.join(filename);
        let dst = validated_dir.join(filename);
        std::fs::rename(&src, &dst)
            .map_err(|e| format!("Failed to move {filename} to final location: {e}"))?;
    }

    let _ = std::fs::remove_dir_all(staging_dir);
    Ok(())
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn resumable_parakeet_bytes(
    staging_dir: &Path,
    model_files: &[(&str, &[&str])],
    expected_file_sizes: &[(&str, u64)],
) -> Vec<u64> {
    model_files
        .iter()
        .zip(expected_file_sizes)
        .map(|((filename, urls), (_, expected))| {
            let len = std::fs::metadata(staging_dir.join(filename)).map_or(0, |m| m.len());
            let sizes_known =
                urls.len() == 1 || urls.iter().all(|u| parakeet_known_part_size(u).is_some());
            match *expected {
                0 if sizes_known => len,
                0 => 0,
                expected if len <= expected && sizes_known => len,
                _ => 0,
            }
        })
        .collect()
}

/// `download_parakeet_model_to_dir` (`captions.rs:2455-2565`): staged into a
/// sibling `.downloading` directory, size-verified, then renamed into place.
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
async fn download_parakeet_model(model: &str) -> Result<(), String> {
    let validated_dir = model_path(model);
    std::fs::create_dir_all(&validated_dir)
        .map_err(|e| format!("Failed to create model directory: {e}"))?;

    let model_files = parakeet_model_files_for_dir(&validated_dir);
    let expected_file_sizes = parakeet_model_file_sizes(model_files).await?;

    let staging_dir = parakeet_staging_dir(&validated_dir);
    if parakeet_model_files_match(&staging_dir, &expected_file_sizes) {
        tracing::info!("Finalizing previously completed Parakeet model download");
        finalize_parakeet_model_download(&validated_dir, &staging_dir, model_files)?;
        invalidate_parakeet_cache_for_dir(&validated_dir);
        split_after_download(validated_dir);
        return Ok(());
    }

    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create staging directory: {e}"))?;

    let total_size = expected_file_sizes
        .iter()
        .fold(0_u64, |acc, (_, size)| acc.saturating_add(*size));
    let resumable = resumable_parakeet_bytes(&staging_dir, model_files, &expected_file_sizes);
    if let Some(missing) = fetch::missing_space(
        &staging_dir,
        total_size.saturating_sub(resumable.iter().sum()),
    ) {
        return Err(no_space_message(missing));
    }

    let mut downloaded_total: u64 = resumable.iter().sum();

    let download_result: Result<(), String> = async {
        for (idx, (filename, urls)) in model_files.iter().enumerate() {
            let file_path = staging_dir.join(filename);
            let expected = expected_file_sizes[idx].1;
            if expected > 0 && resumable[idx] == expected {
                continue;
            }
            let (mut file, written) = fetch::reopen(&file_path)
                .map_err(|e| format!("Failed to create {filename}: {e}"))?;
            if written != resumable[idx] {
                fetch::restart(&mut file)
                    .map_err(|e| format!("Failed to create {filename}: {e}"))?;
            }
            let mut base = 0_u64;
            for url in *urls {
                let part_size = parakeet_known_part_size(url);
                let on_disk = resumable[idx].saturating_sub(base);
                if part_size.is_some_and(|size| on_disk >= size) {
                    base += part_size.unwrap_or(0);
                    continue;
                }
                tracing::info!(have = on_disk, "Downloading {filename} part from {url}");
                let mut on_progress = |event: fetch::Progress<'_>| {
                    match event {
                        fetch::Progress::Bytes(chunk) => {
                            downloaded_total = downloaded_total.saturating_add(chunk.len() as u64);
                        }
                        fetch::Progress::Rewound(bytes) => {
                            downloaded_total = downloaded_total.saturating_sub(bytes);
                        }
                    }
                    let progress = if total_size > 0 {
                        (downloaded_total as f64 / total_size as f64) * 100.0
                    } else {
                        ((idx as f64 + 0.5) / model_files.len() as f64) * 100.0
                    };

                    set_download_progress(
                        model,
                        progress,
                        t!(
                            "Downloading {filename}: {progress}%",
                            filename = filename,
                            progress = format!("{progress:.0}")
                        ),
                    );
                };
                let written = fetch::part(
                    &fetch::network,
                    fetch::Part {
                        url,
                        base,
                        have: on_disk,
                    },
                    &mut file,
                    fetch::Patience::default(),
                    &mut on_progress,
                )
                .await
                .map_err(|e| format!("Failed to download {filename}: {e}"))?;
                base += written;
            }

            file.flush()
                .map_err(|e| format!("Failed to flush {filename}: {e}"))?;

            tracing::info!("Finished downloading {filename}");
        }
        Ok(())
    }
    .await;

    if let Err(e) = &download_result {
        tracing::warn!("Download failed, keeping what arrived for the next try: {e}");
        return Err(e.clone());
    }

    if !parakeet_model_files_match(&staging_dir, &expected_file_sizes) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(t!("Downloaded model files did not match expected sizes").to_string());
    }

    finalize_parakeet_model_download(&validated_dir, &staging_dir, model_files)?;
    invalidate_parakeet_cache_for_dir(&validated_dir);
    split_after_download(validated_dir);
    Ok(())
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn split_parakeet_encoder(dir: &Path) {
    if cfg!(test) && std::env::var_os("FRAMEEVER_PARAKEET_SPLIT_IN_TESTS").is_none() {
        return;
    }
    let free_bytes = |dir: &Path| frameever_utils::disk_space::free_bytes_for_path(dir).ok();
    if let Err(error) = crate::parakeet_split::ensure_split(dir, free_bytes) {
        tracing::warn!(%error, "Parakeet encoder kept as downloaded");
    }
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn split_after_download(dir: PathBuf) {
    if let Err(error) = std::thread::Builder::new()
        .name("frameever-parakeet-split".into())
        .spawn(move || split_parakeet_encoder(&dir))
    {
        tracing::warn!(%error, "could not start the Parakeet encoder split");
    }
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
async fn download_parakeet_model(_model: &str) -> Result<(), String> {
    Err(PARAKEET_UNSUPPORTED_MESSAGE.to_string())
}

// ---------------------------------------------------------------------------
// Engine contexts
// ---------------------------------------------------------------------------

/// `TRANSCRIPTION_LOCK` (`captions.rs:58`): one engine run at a time,
/// process-wide.
static TRANSCRIPTION_LOCK: Mutex<()> = Mutex::new(());

fn lock_transcription_worker_slot() -> std::sync::MutexGuard<'static, ()> {
    TRANSCRIPTION_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

struct CachedWhisperContext {
    model_path: String,
    context: Arc<WhisperContext>,
}

static WHISPER_CONTEXT: LazyLock<Mutex<Option<CachedWhisperContext>>> =
    LazyLock::new(|| Mutex::new(None));

fn invalidate_whisper_cache_for_path(model_path: &Path) {
    let removed = {
        let mut guard = WHISPER_CONTEXT
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if guard
            .as_ref()
            .is_some_and(|cached| Path::new(&cached.model_path) == model_path)
        {
            guard.take()
        } else {
            None
        }
    };
    drop(removed);
}

/// `get_whisper_context_blocking` (`captions.rs:691-707`), keyed by model path
/// so switching small -> medium reloads instead of reusing the stale context.
fn get_whisper_context(model_path: &str) -> Result<Arc<WhisperContext>, String> {
    frameever_utils::local_captions::ensure_whisper_cpu_support()?;
    let mut guard = WHISPER_CONTEXT
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    if let Some(cached) = guard.as_ref()
        && cached.model_path == model_path
    {
        tracing::info!("Reusing cached Whisper context");
        return Ok(cached.context.clone());
    }

    tracing::info!("Initializing Whisper context with model: {model_path}");
    let context = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| format!("Failed to load Whisper model: {e}"))?;

    let context = Arc::new(context);
    *guard = Some(CachedWhisperContext {
        model_path: model_path.to_string(),
        context: context.clone(),
    });
    Ok(context)
}

/// `release_whisper_context_after_transcription` (`captions.rs:709-716`):
/// Apple-silicon Macs drop the context after every run to give the memory
/// back; elsewhere it is kept for the next run.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn release_whisper_context_after_transcription() {
    *WHISPER_CONTEXT
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn release_whisper_context_after_transcription() {}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
struct CachedParakeetContext {
    model_dir: String,
    model: Arc<Mutex<ParakeetTDT>>,
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
static PARAKEET_CONTEXT: LazyLock<Mutex<Option<CachedParakeetContext>>> =
    LazyLock::new(|| Mutex::new(None));

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn invalidate_parakeet_cache_for_dir(model_dir: &Path) {
    let mut guard = PARAKEET_CONTEXT
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if guard
        .as_ref()
        .is_some_and(|cached| cached.model_dir.as_str() == model_dir.to_string_lossy().as_ref())
    {
        tracing::info!(
            "Invalidating cached Parakeet context for {}",
            model_dir.display()
        );
        *guard = None;
    }
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn invalidate_parakeet_cache_for_dir(_model_dir: &Path) {}

/// The loaded Parakeet model and its ONNX Runtime arena hold ~1.8 GB for
/// `best`; keep them only long enough for a quick regenerate.
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
const PARAKEET_IDLE_RELEASE: Duration = Duration::from_secs(60);

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
static PARAKEET_LAST_USE: AtomicU64 = AtomicU64::new(0);

/// Call with `slot` held, so a run in flight always bumps the generation
/// before the release thread can take the slot. The thread returns whether it
/// dropped a cached value.
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn schedule_idle_release<T: Send + 'static>(
    slot: &'static Mutex<()>,
    cache: &'static Mutex<Option<T>>,
    last_use: &'static AtomicU64,
    delay: Duration,
) -> std::io::Result<std::thread::JoinHandle<bool>> {
    let generation = last_use.fetch_add(1, Ordering::AcqRel) + 1;
    std::thread::Builder::new()
        .name("parakeet-release".into())
        .spawn(move || {
            std::thread::sleep(delay);
            let _slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
            if last_use.load(Ordering::Acquire) != generation {
                return false;
            }
            let released = cache.lock().unwrap_or_else(PoisonError::into_inner).take();
            let was_cached = released.is_some();
            drop(released);
            if was_cached {
                tracing::info!("Released idle Parakeet TDT model");
            }
            was_cached
        })
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn schedule_parakeet_release() {
    let spawned = schedule_idle_release(
        &TRANSCRIPTION_LOCK,
        &PARAKEET_CONTEXT,
        &PARAKEET_LAST_USE,
        PARAKEET_IDLE_RELEASE,
    );
    if let Err(error) = spawned {
        tracing::warn!("Failed to schedule Parakeet release: {error}");
    }
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn schedule_parakeet_release() {}

// ---------------------------------------------------------------------------
// Audio extraction
// ---------------------------------------------------------------------------

/// `frameever_audio::AudioData::SAMPLE_RATE` -- the rate every source is decoded to
/// before mixing, matching the Tauri pipeline's intermediate format.
const DECODE_SAMPLE_RATE: u32 = 48_000;
/// `WHISPER_SAMPLE_RATE` (`captions.rs:66`).
const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// `AudioData::from_file` (`crates/audio/src/audio_data.rs:22-95`) transcribed
/// against the same ffmpeg fork: any input decoded to packed f32 at 48kHz,
/// mono stays mono, everything else downmixes to stereo.
fn decode_audio_file(path: &Path) -> Result<(Vec<f32>, usize), String> {
    let mut input_ctx = avformat::input(&path).map_err(|e| format!("Input Open / {e}"))?;
    let input_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or_else(|| "No Stream".to_string())?;

    let decoder_ctx = avcodec::Context::from_parameters(input_stream.parameters())
        .map_err(|e| format!("Decoder Parameters / {e}"))?;
    let mut decoder = decoder_ctx
        .decoder()
        .audio()
        .map_err(|e| format!("Set Parameters / {e}"))?;

    let source_channels = decoder.channels().max(1);
    if decoder.channel_layout().is_empty() {
        decoder.set_channel_layout(ChannelLayout::default(i32::from(source_channels)));
    }
    decoder.set_packet_time_base(input_stream.time_base());

    let target_channels: u16 = if source_channels <= 1 { 1 } else { 2 };
    let target_channel_layout = ChannelLayout::default(i32::from(target_channels));
    let mut options = ffmpeg::Dictionary::new();
    options.set("filter_size", "128");
    options.set("cutoff", "0.97");

    let mut resampler = resampling::Context::get_with(
        decoder.format(),
        decoder.channel_layout(),
        decoder.rate(),
        avformat::Sample::F32(avformat::sample::Type::Packed),
        target_channel_layout,
        DECODE_SAMPLE_RATE,
        options,
    )
    .map_err(|e| format!("Resampler / {e}"))?;

    let index = input_stream.index();

    let mut decoded_frame = ffmpeg::frame::Audio::empty();
    let mut samples: Vec<f32> = Vec::new();

    for (stream, packet) in input_ctx.packets() {
        if stream.index() != index {
            continue;
        }

        decoder
            .send_packet(&packet)
            .map_err(|e| format!("Send Packet / {e}"))?;

        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            run_decode_resampler(&mut resampler, &decoded_frame, &mut samples)?;
        }
    }

    decoder.send_eof().map_err(|e| format!("Send EOF / {e}"))?;

    while decoder.receive_frame(&mut decoded_frame).is_ok() {
        run_decode_resampler(&mut resampler, &decoded_frame, &mut samples)?;
    }

    flush_decode_resampler(&mut resampler, &mut samples)?;

    Ok((samples, usize::from(target_channels)))
}

fn run_decode_resampler(
    resampler: &mut resampling::Context,
    decoded_frame: &ffmpeg::frame::Audio,
    samples: &mut Vec<f32>,
) -> Result<(), String> {
    let target = *resampler.output();
    let capacity = decode_resample_capacity(resampler, decoded_frame.samples());
    let mut resampled_frame =
        ffmpeg::frame::Audio::new(target.format, capacity, target.channel_layout);

    resampler
        .run(decoded_frame, &mut resampled_frame)
        .map_err(|e| format!("Run Resampler / {e}"))?;

    append_resampled_frame(samples, &resampled_frame)
}

fn flush_decode_resampler(
    resampler: &mut resampling::Context,
    samples: &mut Vec<f32>,
) -> Result<(), String> {
    for _ in 0..64 {
        let Some(delay) = resampler.delay() else {
            break;
        };
        let target = *resampler.output();
        let capacity = delay
            .output
            .max(1)
            .saturating_add(16)
            .min(i64::from(i32::MAX)) as usize;
        let mut resampled_frame =
            ffmpeg::frame::Audio::new(target.format, capacity, target.channel_layout);
        let remaining = resampler
            .flush(&mut resampled_frame)
            .map_err(|e| format!("Flush Resampler / {e}"))?;

        let output_samples = resampled_frame.samples();
        append_resampled_frame(samples, &resampled_frame)?;

        if remaining.is_none() || output_samples == 0 {
            break;
        }
    }
    Ok(())
}

fn decode_resample_capacity(resampler: &resampling::Context, input_samples: usize) -> usize {
    let src_rate = u64::from(resampler.input().rate.max(1));
    let dst_rate = u64::from(resampler.output().rate.max(1));
    let pending_output_samples = resampler
        .delay()
        .map(|d| d.output.max(0) as u64)
        .unwrap_or(0);
    let resampled_from_input = (input_samples as u64)
        .saturating_mul(dst_rate)
        .div_ceil(src_rate);

    pending_output_samples
        .saturating_add(resampled_from_input)
        .saturating_add(16)
        .min(i32::MAX as u64) as usize
}

fn append_resampled_frame(
    samples: &mut Vec<f32>,
    frame: &ffmpeg::frame::Audio,
) -> Result<(), String> {
    if frame.samples() == 0 {
        return Ok(());
    }

    let byte_len = frame
        .samples()
        .saturating_mul(frame.channels() as usize)
        .saturating_mul(std::mem::size_of::<f32>());
    let data = frame
        .data(0)
        .get(..byte_len)
        .ok_or_else(|| "Resampled frame data shorter than expected".to_string())?;

    // SAFETY: ffmpeg's frame buffers are allocated with av_malloc alignment,
    // and the slice length is a whole number of f32s by construction.
    samples
        .extend(unsafe { std::slice::from_raw_parts(data.as_ptr() as *const f32, byte_len / 4) });
    Ok(())
}

/// `normalize_audio_for_transcription` (`captions.rs:885-915`).
fn normalize_audio_for_transcription(samples: &mut [f32]) -> f32 {
    if samples.is_empty() {
        return 1.0;
    }

    let peak = samples
        .iter()
        .fold(0.0_f32, |max, sample| max.max(sample.abs()));
    if peak <= f32::EPSILON {
        return 1.0;
    }

    let rms =
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt();
    if rms <= f32::EPSILON {
        return 1.0;
    }

    let target_rms = 0.08_f32;
    let desired_gain = (target_rms / rms).clamp(1.0, 8.0);
    let peak_limited_gain = 0.98 / peak;
    let gain = desired_gain.min(peak_limited_gain);

    if (gain - 1.0).abs() > 0.01 {
        for sample in samples {
            *sample = (*sample * gain).clamp(-0.98, 0.98);
        }
    }

    gain
}

/// The recording-directory arm of `extract_audio_from_video`
/// (`captions.rs:250-517`): per segment, decode system audio then mic
/// (`captions.rs:281-283`), downmix each to mono, average them together,
/// concatenate the segments, normalize, and write a 16kHz mono s16le WAV.
fn extract_project_audio(project_path: &Path, output_path: &Path) -> Result<(), String> {
    ffmpeg::init().map_err(|e| format!("Failed to initialise ffmpeg: {e}"))?;

    let meta = RecordingMeta::load_for_project(project_path)
        .map_err(|e| format!("Failed to read recording metadata: {e}"))?;
    let Some(studio) = meta.studio_meta() else {
        return Err(t!("Only studio recordings can be transcribed").to_string());
    };

    studio.ensure_ordinary_media_access(&meta.project_path)?;
    let mut final_samples = Vec::new();
    let mut any_audio = false;
    match studio {
        StudioRecordingMeta::SingleSegment { segment } => {
            let display_duration = Video::new(
                segment.display.path.to_path(&meta.project_path),
                segment.display.start_time.unwrap_or_default(),
            )
            .map_err(|error| format!("Failed to read display video: {error}"))?
            .duration;
            let mut sources = Vec::new();
            if let Some(audio) = &segment.audio {
                let path = meta.path(&audio.path);
                if path.exists() {
                    match decode_audio_file(&path) {
                        Ok((samples, channels)) => {
                            any_audio = true;
                            sources.push(TranscriptionAudioSource {
                                samples,
                                channels,
                                offset_secs: 0.0,
                            });
                        }
                        Err(error) => tracing::warn!(
                            path = %path.display(),
                            "Failed to process audio source: {error}"
                        ),
                    }
                }
            }
            let take = TranscriptionAudioTake {
                display_duration_secs: display_duration,
                sources,
            };
            append_transcription_audio(&mut final_samples, &take, DECODE_SAMPLE_RATE)
                .map_err(|error| format!("Failed to assemble transcription audio: {error}"))?;
        }
        StudioRecordingMeta::MultipleSegments { inner } => {
            for (segment_idx, segment) in inner.segments.iter().enumerate() {
                let display_duration = Video::new(
                    segment.display.path.to_path(&meta.project_path),
                    segment.display.start_time.unwrap_or_default(),
                )
                .map_err(|error| {
                    format!("Failed to read display video for segment {segment_idx}: {error}")
                })?
                .duration;
                let offsets = segment.calculate_audio_offsets();
                let mut sources = Vec::new();
                let mut push_source = |audio: &frameever_project::AudioMeta, offset_secs: f64| {
                    let path = meta.path(&audio.path);
                    if !path.exists() {
                        return;
                    }
                    match decode_audio_file(&path) {
                        Ok((samples, channels)) => {
                            any_audio = true;
                            sources.push(TranscriptionAudioSource {
                                samples,
                                channels,
                                offset_secs,
                            });
                        }
                        Err(error) => tracing::warn!(
                            path = %path.display(),
                            "Failed to process audio source: {error}"
                        ),
                    }
                };

                if let Some(system_audio) = &segment.system_audio {
                    push_source(system_audio, f64::from(offsets.system_audio));
                }
                if let Some(mic) = &segment.mic {
                    push_source(mic, f64::from(offsets.mic));
                }
                let take = TranscriptionAudioTake {
                    display_duration_secs: display_duration,
                    sources,
                };
                append_transcription_audio(&mut final_samples, &take, DECODE_SAMPLE_RATE)
                    .map_err(|error| format!("Failed to assemble transcription audio: {error}"))?;
            }
        }
    }

    if !any_audio {
        return Err(t!("No audio sources found in the recording metadata").to_string());
    }

    if final_samples.is_empty() {
        return Err(t!("Failed to process any audio sources").to_string());
    }

    let gain = normalize_audio_for_transcription(&mut final_samples);
    if (gain - 1.0).abs() > 0.01 {
        tracing::info!("Applied transcription audio gain: {gain:.2}x");
    }

    write_wav_16k_mono(&final_samples, output_path)
}

/// The WAV encode half of the recording-directory arm
/// (`captions.rs:372-515`): 48kHz f32 mono in, 16kHz s16le mono WAV out.
fn write_wav_16k_mono(mixed_samples: &[f32], output_path: &Path) -> Result<(), String> {
    let mut output =
        avformat::output(&output_path).map_err(|e| format!("Failed to create output file: {e}"))?;

    let codec = avcodec::encoder::find_by_name("pcm_s16le")
        .ok_or_else(|| "PCM encoder not found".to_string())?;

    let mut encoder = avcodec::Context::new()
        .encoder()
        .audio()
        .map_err(|e| format!("Failed to create encoder: {e}"))?;

    encoder.set_rate(WHISPER_SAMPLE_RATE as i32);
    let channel_layout = ChannelLayout::MONO;
    encoder.set_channel_layout(channel_layout);
    encoder.set_format(avformat::Sample::I16(avformat::sample::Type::Packed));

    let mut encoder = encoder
        .open_as(codec)
        .map_err(|e| format!("Failed to open encoder: {e}"))?;

    let mut stream = output
        .add_stream(codec)
        .map_err(|e| format!("Failed to add stream: {e}"))?;
    stream.set_parameters(&encoder);

    output
        .write_header()
        .map_err(|e| format!("Failed to write header: {e}"))?;

    let mut resampler = resampling::Context::get(
        avformat::Sample::F32(avformat::sample::Type::Packed),
        channel_layout,
        DECODE_SAMPLE_RATE,
        avformat::Sample::I16(avformat::sample::Type::Packed),
        channel_layout,
        WHISPER_SAMPLE_RATE,
    )
    .map_err(|e| format!("Failed to create resampler: {e}"))?;

    let frame_size = encoder.frame_size() as usize;
    let frame_size = if frame_size == 0 { 1024 } else { frame_size };

    for chunk in mixed_samples.chunks(frame_size) {
        let mut input_frame = ffmpeg::frame::Audio::new(
            avformat::Sample::F32(avformat::sample::Type::Packed),
            chunk.len(),
            channel_layout,
        );
        input_frame.set_rate(DECODE_SAMPLE_RATE);

        // SAFETY: reinterpreting the f32 chunk as its little-endian bytes for
        // the frame copy, exactly as the Tauri encoder does.
        let bytes = unsafe {
            std::slice::from_raw_parts(chunk.as_ptr() as *const u8, std::mem::size_of_val(chunk))
        };
        input_frame.data_mut(0)[0..bytes.len()].copy_from_slice(bytes);

        let mut output_frame = ffmpeg::frame::Audio::new(
            avformat::Sample::I16(avformat::sample::Type::Packed),
            frame_size,
            ChannelLayout::MONO,
        );
        output_frame.set_rate(WHISPER_SAMPLE_RATE);

        if let Err(error) = resampler.run(&input_frame, &mut output_frame) {
            tracing::error!("Failed to resample chunk: {error}");
            continue;
        }

        if let Err(error) = encoder.send_frame(&output_frame) {
            tracing::error!("Failed to send frame to encoder: {error}");
            continue;
        }

        loop {
            let mut packet = ffmpeg::Packet::empty();
            if encoder.receive_packet(&mut packet).is_err() {
                break;
            }
            if let Err(error) = packet.write_interleaved(&mut output) {
                tracing::error!("Failed to write packet: {error}");
            }
        }
    }

    encoder
        .send_eof()
        .map_err(|e| format!("Failed to send EOF: {e}"))?;

    loop {
        let mut packet = ffmpeg::Packet::empty();
        if encoder.receive_packet(&mut packet).is_err() {
            break;
        }
        packet
            .write_interleaved(&mut output)
            .map_err(|e| format!("Failed to write final packet: {e}"))?;
    }

    output
        .write_trailer()
        .map_err(|e| format!("Failed to write trailer: {e}"))
}

// ---------------------------------------------------------------------------
// Whisper
// ---------------------------------------------------------------------------

const ENGLISH_VERBATIM_PROMPT: &str = "So, um, I think, uh, we should start.";
const CHINESE_VERBATIM_PROMPT: &str = "那我觉得，嗯，我们先，呃，开始吧。";

/// `build_initial_prompt` (`captions.rs:1181-1200`).
fn build_initial_prompt(language: &str, transcription_hints: &[String]) -> Option<String> {
    let mut normalized = Vec::new();

    for hint in transcription_hints {
        let value = hint.replace('\0', "").trim().to_string();
        if value.is_empty() || normalized.contains(&value) {
            continue;
        }
        normalized.push(value);
    }

    if language == "zh" {
        let mut prompt = "以下是普通话的句子，使用简体中文和标点符号。".to_string();
        if !normalized.is_empty() {
            prompt.push_str(&format!("专有名词：{}。", normalized.join("、")));
        }
        prompt.push_str(CHINESE_VERBATIM_PROMPT);
        return Some(prompt);
    }
    let hints = (!normalized.is_empty()).then(|| {
        format!(
            "Preferred spellings, names, and capitalization for this transcript: {}",
            normalized.join("; ")
        )
    });
    match (hints, language == "en") {
        (Some(hints), true) => Some(format!("{hints}. {ENGLISH_VERBATIM_PROMPT}")),
        (None, true) => Some(ENGLISH_VERBATIM_PROMPT.to_string()),
        (hints, false) => hints,
    }
}

/// `general_settings.transcriptionHints`, with the Rust store's own default
/// list when the key is absent (`general_settings.rs:286-293`).
fn transcription_hints() -> Vec<String> {
    let section = store::store_section(store::GENERAL_SETTINGS);
    match section.get("transcriptionHints").and_then(|v| v.as_array()) {
        Some(values) => values
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::to_string)
            .collect(),
        None => vec![
            "FrameEver".to_string(),
            "TypeScript".to_string(),
            "My Brand Name".to_string(),
            "mywebsite.com".to_string(),
        ],
    }
}

/// `is_special_token` (`captions.rs:718-735`).
fn whisper_threads(available: Option<usize>) -> i32 {
    available.unwrap_or(4).clamp(1, 4) as i32
}

fn is_special_token(token_text: &str) -> bool {
    let trimmed = token_text.trim();
    if trimmed.is_empty() {
        return true;
    }
    trimmed.contains('[')
        || trimmed.contains(']')
        || trimmed.contains("_TT_")
        || trimmed.contains("_BEG_")
        || trimmed.contains("<|")
}

fn full_width_punctuation(value: char) -> char {
    match value {
        ',' => '，',
        '.' => '。',
        '!' => '！',
        '?' => '？',
        ';' => '；',
        ':' => '：',
        other => other,
    }
}

const MIN_WHISPER_WORD_SECONDS: f32 = 0.1;

fn whisper_words<'a>(tokens: impl IntoIterator<Item = (&'a [u8], f32, f32)>) -> Vec<CaptionWord> {
    let mut words: Vec<CaptionWord> = Vec::new();
    let mut pending = Vec::new();
    let mut pending_start = 0.0;
    let mut joins_previous = false;

    for (bytes, start, end) in tokens {
        if pending.is_empty() {
            pending_start = start;
        }
        pending.extend_from_slice(bytes);
        let text = match std::str::from_utf8(&pending) {
            Ok(text) => text.to_string(),
            Err(error) if error.error_len().is_none() => continue,
            Err(_) => String::from_utf8_lossy(&pending).into_owned(),
        };
        pending.clear();

        let piece = text.trim();
        if piece.is_empty() {
            joins_previous = false;
            continue;
        }
        if is_special_token(piece) {
            continue;
        }

        let has_cjk = piece.chars().any(is_cjk);
        match words.last_mut() {
            Some(word) if joins_previous && !has_cjk && !text.starts_with(char::is_whitespace) => {
                word.text.push_str(piece);
                word.end = end;
            }
            _ => words.push(CaptionWord {
                text: piece.to_string(),
                start: pending_start,
                end,
            }),
        }
        joins_previous = !piece.ends_with(|c| is_cjk(c) || is_cjk_punctuation(c));
    }

    let next_starts: Vec<Option<f32>> = words
        .iter()
        .skip(1)
        .map(|word| Some(word.start))
        .chain([None])
        .collect();
    for (word, next_start) in words.iter_mut().zip(next_starts) {
        if word.end <= word.start {
            word.end = next_start
                .filter(|next_start| *next_start > word.start)
                .unwrap_or(word.start + MIN_WHISPER_WORD_SECONDS);
        }
    }

    words
}

fn read_whisper_audio(audio_path: &Path) -> Result<Vec<f32>, String> {
    let mut audio_data = Vec::new();
    File::open(audio_path)
        .map_err(|e| format!("Failed to open audio file: {e} at path: {audio_path:?}"))?
        .read_to_end(&mut audio_data)
        .map_err(|e| format!("Failed to read audio file: {e}"))?;

    let mut audio_data_f32: Vec<f32> = audio_data
        .chunks_exact(2)
        .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32768.0)
        .collect();

    let gain = normalize_audio_for_transcription(&mut audio_data_f32);
    if (gain - 1.0).abs() > 0.01 {
        tracing::info!("Applied Whisper input gain: {gain:.2}x");
    }
    Ok(audio_data_f32)
}

const LANGUAGE_DETECTION_SPEECH_LEVEL: f32 = 0.05;
const LANGUAGE_DETECTION_SECONDS: usize = 30;

fn detect_whisper_language(state: &mut WhisperState, samples: &[f32]) -> Option<String> {
    let speech_start = samples
        .iter()
        .position(|sample| sample.abs() > LANGUAGE_DETECTION_SPEECH_LEVEL)?;
    let window_end = samples
        .len()
        .min(speech_start + LANGUAGE_DETECTION_SECONDS * WHISPER_SAMPLE_RATE as usize);
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let detected = state
        .pcm_to_mel(&samples[speech_start..window_end], threads)
        .and_then(|()| state.lang_detect(0, threads))
        .map_err(|error| tracing::warn!("Whisper language detection failed: {error}"))
        .ok()
        .and_then(|(id, _)| whisper_rs::get_lang_str(id))?;
    tracing::info!("Whisper detected spoken language: {detected}");
    Some(detected.to_string())
}

fn detect_spoken_language(audio_path: &Path, whisper_model: &str) -> Option<String> {
    let context = get_whisper_context(model_path(whisper_model).to_string_lossy().as_ref())
        .map_err(|error| tracing::warn!("Failed to load Whisper for language detection: {error}"))
        .ok()?;
    let samples = read_whisper_audio(audio_path)
        .map_err(|error| tracing::warn!("{error}"))
        .ok()?;
    let mut state = context
        .create_state()
        .map_err(|error| tracing::warn!("Failed to create Whisper state: {error}"))
        .ok()?;
    detect_whisper_language(&mut state, &samples)
}

/// `process_with_whisper` (`captions.rs:917-1179`).
fn process_with_whisper(
    audio_path: &Path,
    context: Arc<WhisperContext>,
    language: &str,
    transcription_hints: &[String],
) -> Result<HeardWords, String> {
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: 1.0,
    });
    params.set_n_threads(whisper_threads(
        std::thread::available_parallelism().ok().map(|n| n.get()),
    ));

    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_token_timestamps(true);
    params.set_max_len(i32::MAX);

    let audio_data_f32 = read_whisper_audio(audio_path)?;

    let mut state = context
        .create_state()
        .map_err(|e| format!("Failed to create Whisper state: {e}"))?;

    let language = if language == "auto" {
        detect_whisper_language(&mut state, &audio_data_f32).unwrap_or_else(|| "auto".to_string())
    } else {
        language.to_string()
    };
    params.set_language(Some(&language));

    let initial_prompt = build_initial_prompt(&language, transcription_hints);
    if let Some(initial_prompt) = &initial_prompt {
        params.set_initial_prompt(initial_prompt);
    }

    tracing::info!(
        "Whisper input: {} samples ({:.2}s at {}Hz), language: {language}",
        audio_data_f32.len(),
        audio_data_f32.len() as f32 / WHISPER_SAMPLE_RATE as f32,
        WHISPER_SAMPLE_RATE
    );

    state
        .full(params, &audio_data_f32[..])
        .map_err(|e| format!("Failed to run Whisper transcription: {e}"))?;

    let num_segments = state.full_n_segments();

    let mut groups = Vec::new();

    for i in 0..num_segments {
        let segment = state
            .get_segment(i)
            .ok_or_else(|| format!("Whisper segment {i} is out of range"))?;

        let tokens = (0..segment.n_tokens())
            .filter_map(|t| segment.get_token(t))
            .filter_map(|token| {
                let data = token.token_data();
                let bytes = token.to_bytes().ok()?;
                Some((bytes, data.t0 as f32 / 100.0, data.t1 as f32 / 100.0))
            });
        let words = normalize_caption_words(whisper_words(tokens));
        if !words.is_empty() {
            groups.push(words);
        }
    }

    tracing::info!(
        "Whisper produced {} segments ({} words)",
        groups.len(),
        groups.iter().map(Vec::len).sum::<usize>()
    );

    Ok(HeardWords {
        groups,
        samples: audio_data_f32,
    })
}

// ---------------------------------------------------------------------------
// Parakeet
// ---------------------------------------------------------------------------

/// Parakeet's encoder fails with an ONNX broadcast error past 400 s of audio
/// (5,000 frames of 80 ms), and below that its time and memory per second of
/// audio grow with the input's length. Minute-long windows, cut at the quietest
/// 100 ms near each boundary so words stay whole, keep cost linear in the
/// recording's length and lift the 400 s cap.
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
const PARAKEET_CHUNK_SECONDS: usize = 60;
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
const PARAKEET_CUT_SEARCH_SECONDS: usize = 5;

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn parakeet_chunk_ranges(samples: &[f32], sample_rate: usize) -> Vec<std::ops::Range<usize>> {
    let target = PARAKEET_CHUNK_SECONDS * sample_rate;
    let search = PARAKEET_CUT_SEARCH_SECONDS * sample_rate;
    let window = sample_rate / 10;
    let energy = |at: usize| -> f32 { samples[at..at + window].iter().map(|s| s * s).sum() };
    let mut ranges = Vec::new();
    let mut start = 0;
    while samples.len() - start > target {
        let end = start + target;
        let quietest = (end - search..=end - window)
            .step_by(sample_rate / 100)
            .min_by(|a, b| energy(*a).total_cmp(&energy(*b)))
            .unwrap_or(end - window);
        let cut = quietest + window / 2;
        ranges.push(start..cut);
        start = cut;
    }
    ranges.push(start..samples.len());
    ranges
}

/// `process_with_parakeet` (`captions.rs:1202-1302`).
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn process_with_parakeet(audio_path: &Path, model_dir: &str) -> Result<HeardWords, String> {
    let cached_model = {
        let guard = PARAKEET_CONTEXT
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        guard.as_ref().and_then(|cached| {
            if cached.model_dir == model_dir {
                Some(Arc::clone(&cached.model))
            } else {
                None
            }
        })
    };

    let model_arc = if let Some(model) = cached_model {
        tracing::info!("Reusing cached Parakeet TDT model");
        model
    } else {
        tracing::info!("Loading Parakeet TDT model from: {model_dir}");
        frameever_camera_effects::initialize_onnx_runtime()
            .map_err(|error| format!("{error:#}"))?;
        split_parakeet_encoder(Path::new(model_dir));
        let model = ParakeetTDT::from_pretrained(model_dir, None).map_err(|e| format!("{e}"))?;
        let loaded_model = Arc::new(Mutex::new(model));

        let mut guard = PARAKEET_CONTEXT
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = guard
            .as_ref()
            .filter(|cached| cached.model_dir == model_dir)
        {
            Arc::clone(&cached.model)
        } else {
            *guard = Some(CachedParakeetContext {
                model_dir: model_dir.to_string(),
                model: Arc::clone(&loaded_model),
            });
            loaded_model
        }
    };

    let mut reader = hound::WavReader::open(audio_path)
        .map_err(|e| format!("Failed to open audio file: {e} at path: {audio_path:?}"))?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != WHISPER_SAMPLE_RATE {
        return Err(format!(
            "Expected 16 kHz mono audio, got {} Hz with {} channels",
            spec.sample_rate, spec.channels
        ));
    }
    let sample_rate = spec.sample_rate;
    let samples = reader
        .samples::<i16>()
        .map(|sample| sample.map(|sample| f32::from(sample) / 32768.0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read audio file: {e}"))?;
    if samples.is_empty() {
        return Ok(HeardWords {
            groups: Vec::new(),
            samples,
        });
    }

    let mut words = Vec::new();
    {
        let mut parakeet = model_arc
            .lock()
            .map_err(|e| format!("Failed to lock Parakeet model: {e}"))?;
        for range in parakeet_chunk_ranges(&samples, sample_rate as usize) {
            let offset = range.start as f32 / sample_rate as f32;
            let result = parakeet
                .transcribe_samples(
                    samples[range].to_vec(),
                    sample_rate,
                    1,
                    Some(TimestampMode::Tokens),
                )
                .map_err(|e| format!("Parakeet transcription failed: {e}"))?;
            words.extend(verbatim::words_from_subword_tokens(
                result.tokens.into_iter().map(|token| CaptionWord {
                    text: token.text,
                    start: token.start + offset,
                    end: token.end + offset,
                }),
            ));
        }
    }

    tracing::info!("Parakeet produced {} words", words.len());

    Ok(HeardWords {
        groups: vec![normalize_caption_words(words)],
        samples,
    })
}

#[cfg(test)]
fn caption_segments_from_tokens(
    tokens: impl IntoIterator<Item = CaptionWord>,
) -> Result<Vec<CaptionSegment>, String> {
    let words = normalize_caption_words(
        tokens
            .into_iter()
            .filter(|token| !token.text.trim().is_empty())
            .collect(),
    );
    if words.is_empty() {
        return Ok(Vec::new());
    }
    Ok(caption_segments_from_groups(vec![words]))
}

struct HeardWords {
    groups: Vec<Vec<CaptionWord>>,
    samples: Vec<f32>,
}

fn caption_segments_from_groups(groups: Vec<Vec<CaptionWord>>) -> Vec<CaptionSegment> {
    let groups: Vec<Vec<CaptionWord>> = groups
        .into_iter()
        .filter(|group| !group.is_empty())
        .collect();
    let single = groups.len() == 1;
    let skips_hesitations = filler_language(&groups.concat()).is_some();
    let mut segments = Vec::new();
    for (group_index, words) in groups.iter().enumerate() {
        let hesitations = if skips_hesitations {
            filler_words::hesitations(words)
        } else {
            vec![false; words.len()]
        };
        for (chunk_index, chunk) in caption_word_chunks_counting(words, &hesitations)
            .into_iter()
            .enumerate()
        {
            segments.push(CaptionSegment {
                id: if single {
                    format!("segment-{chunk_index}")
                } else {
                    format!("segment-{group_index}-{chunk_index}")
                },
                start: chunk.first().map_or(0.0, |word| word.start),
                end: chunk.last().map_or(0.0, |word| word.end),
                text: caption_text_from_words(chunk),
                words: chunk.to_vec(),
                recording_clip: None,
            });
        }
    }
    segments
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn process_with_parakeet(_audio_path: &Path, _model_dir: &str) -> Result<HeardWords, String> {
    Err(PARAKEET_UNSUPPORTED_MESSAGE.to_string())
}

// ---------------------------------------------------------------------------
// Transcribe entry point
// ---------------------------------------------------------------------------

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `transcribe_audio` (`captions.rs:1315-1439`), minus the Tauri command
/// plumbing. Blocking -- run it on the tokio blocking pool.
pub fn transcribe_blocking(
    project_path: &Path,
    model: &str,
    language: &str,
) -> Result<Vec<CaptionSegment>, TranscriptionError> {
    if !project_path.exists() {
        return Err(TranscriptionError::Failed(format!(
            "Video file not found at path: {}",
            project_path.display()
        )));
    }
    let usable = |model: &str| model_files_present(model) && !hub().downloading(model);

    transcribe_project_with(project_path, |audio_path| {
        let _guard = lock_transcription_worker_slot();
        let engine = caption_engine(model, language, usable, |whisper| {
            detect_spoken_language(audio_path, whisper)
        })?;
        if engine.model != model || engine.language != language {
            tracing::info!(
                "Captions for {language} with {model} run on {} with {}",
                engine.model,
                engine.language
            );
        }
        let (model, language) = (engine.model.as_str(), engine.language.as_str());
        let model_file = model_path(model);
        if !model_files_present(model) {
            return Err(format!(
                "Model file not found at path: {}",
                model_file.display()
            ));
        }
        if is_parakeet_model(model) {
            for whisper in WHISPER_MODELS_LARGEST_FIRST {
                invalidate_whisper_cache_for_path(&model_path(whisper));
            }
            tracing::info!("Using Parakeet TDT engine");
            let result = process_with_parakeet(audio_path, model_file.to_string_lossy().as_ref());
            schedule_parakeet_release();
            result
        } else {
            tracing::info!("Using Whisper engine");
            let hints = transcription_hints();
            let context = get_whisper_context(model_file.to_string_lossy().as_ref())?;
            let result = process_with_whisper(audio_path, context, language, &hints);
            release_whisper_context_after_transcription();
            result
        }
        .map(|heard| {
            caption_segments_from_groups(verbatim::finish_word_timings(
                heard.groups,
                &heard.samples,
                WHISPER_SAMPLE_RATE,
            ))
        })
    })
}

/// Extracts the project's transcription audio to a temporary 16 kHz mono WAV
/// and runs `engine` on it.
fn transcribe_project_with(
    project_path: &Path,
    engine: impl FnOnce(&Path) -> Result<Vec<CaptionSegment>, String>,
) -> Result<Vec<CaptionSegment>, TranscriptionError> {
    let temp_dir = std::env::temp_dir().join(format!(
        "cap-gpui-transcription-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        TranscriptionError::Failed(format!("Failed to create temporary directory: {e}"))
    })?;
    let audio_path = temp_dir.join("audio.wav");

    let result = (|| {
        extract_audio_from_video(project_path, &audio_path)?;
        engine(&audio_path)
    })();

    let _ = std::fs::remove_dir_all(&temp_dir);

    heard_segments(result)
}

/// `resolve_audio_extraction_source` (`captions.rs:221-243`), directory arm
/// only -- the gpui editor always opens `.cap` bundles, so a plain media file
/// is refused rather than decoded.
fn extract_audio_from_video(video_path: &Path, output_path: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(video_path)
        .map_err(|e| format!("Failed to read video path metadata: {e}"))?;
    if !metadata.is_dir() {
        return Err("Video path is not a recording directory".to_string());
    }
    if !video_path.join("recording-meta.json").is_file() {
        return Err("Recording directory is missing recording-meta.json".to_string());
    }
    extract_project_audio(video_path, output_path)
}

// ---------------------------------------------------------------------------
// Caption words & chunking
// ---------------------------------------------------------------------------

/// `MAX_CAPTION_WORD_DURATION` (`captions.rs:75`): Whisper/Parakeet sometimes
/// stretch a trailing word's end across a following silence, which leaves the
/// rendered caption stuck on screen and duplicates the word across timeline
/// cuts once projected.
pub const MAX_CAPTION_WORD_DURATION: f32 = 2.5;
/// `captions.rs:67-69`.
const TARGET_CAPTION_WORDS_PER_SEGMENT: usize = 6;
const MAX_CAPTION_WORDS_PER_SEGMENT: usize = 8;
const MIN_FINAL_CAPTION_WORDS: usize = 3;

/// `caption_char_attaches_to_previous` (`captions.rs:745-767`).
fn caption_char_attaches_to_previous(value: char) -> bool {
    matches!(
        value,
        ',' | '.'
            | '!'
            | '?'
            | ';'
            | ':'
            | '%'
            | ')'
            | ']'
            | '}'
            | '\''
            | '’'
            | '、'
            | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '，'
    )
}

fn caption_token_attaches_to_previous(text: &str) -> bool {
    text.trim()
        .chars()
        .next()
        .is_some_and(caption_char_attaches_to_previous)
}

/// `caption_boundary_word_is_weak` (`captions.rs:769-801`).
fn caption_boundary_word_is_weak(word: &CaptionWord) -> bool {
    let normalized = word
        .text
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();

    normalized.chars().count() <= 1
        || matches!(
            normalized.as_str(),
            "an" | "as"
                | "at"
                | "be"
                | "by"
                | "do"
                | "he"
                | "if"
                | "in"
                | "is"
                | "it"
                | "me"
                | "my"
                | "of"
                | "on"
                | "or"
                | "so"
                | "to"
                | "up"
                | "we"
        )
}

/// `normalize_caption_words` (`captions.rs:803-831`).
fn normalize_caption_words(words: Vec<CaptionWord>) -> Vec<CaptionWord> {
    let mut normalized: Vec<CaptionWord> = Vec::with_capacity(words.len());

    for word in words {
        let text = word.text.trim();
        if text.is_empty() {
            continue;
        }

        if caption_token_attaches_to_previous(text)
            && let Some(previous) = normalized.last_mut()
        {
            if previous.text.ends_with(is_cjk) {
                previous
                    .text
                    .extend(text.chars().map(full_width_punctuation));
            } else {
                previous.text.push_str(text);
            }
            previous.end = word.end;
        } else {
            normalized.push(CaptionWord {
                text: text.to_string(),
                start: word.start,
                end: word.end,
            });
        }
    }

    for word in &mut normalized {
        word.end = word.end.min(word.start + MAX_CAPTION_WORD_DURATION);
    }

    normalized
}

#[cfg(test)]
fn caption_word_chunks(words: &[CaptionWord]) -> Vec<&[CaptionWord]> {
    caption_word_chunks_counting(words, &vec![false; words.len()])
}

fn caption_word_chunks_counting<'a>(
    words: &'a [CaptionWord],
    uncounted: &[bool],
) -> Vec<&'a [CaptionWord]> {
    let mut counted: Vec<usize> = (0..words.len())
        .filter(|&index| !uncounted[index])
        .collect();
    if counted.is_empty() {
        counted = (0..words.len()).collect();
    }
    let spans = chunk_spans(counted.len(), |index| {
        caption_boundary_word_is_weak(&words[counted[index]])
    });
    let last = spans.len().saturating_sub(1);
    let mut start = 0;
    spans
        .into_iter()
        .enumerate()
        .map(|(chunk, range)| {
            let end = if chunk == last {
                words.len()
            } else {
                counted[range.end - 1] + 1
            };
            let slice = &words[start..end];
            start = end;
            slice
        })
        .collect()
}

/// `caption_word_chunks` (`captions.rs:851-883`).
fn chunk_spans(len: usize, weak: impl Fn(usize) -> bool) -> Vec<std::ops::Range<usize>> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < len {
        let remaining = len - start;
        if remaining <= TARGET_CAPTION_WORDS_PER_SEGMENT {
            chunks.push(start..len);
            break;
        }

        let mut end = (start + TARGET_CAPTION_WORDS_PER_SEGMENT).min(len);
        while end < len && weak(end - 1) && end - start < MAX_CAPTION_WORDS_PER_SEGMENT {
            end += 1;
        }

        let remaining_after = len - end;
        if remaining_after > 0 && remaining_after < MIN_FINAL_CAPTION_WORDS && weak(end) {
            end = len;
        }

        chunks.push(start..end);
        start = end;
    }

    chunks
}

// ---------------------------------------------------------------------------
// Track derivation -- deriveCaptionTrackSegments, in Rust
// ---------------------------------------------------------------------------

pub fn source_caption_id(track_id: &str) -> &str {
    frameever_project::source_caption_id(track_id)
}

struct SourceToEditedMapping {
    source_start: f64,
    source_end: f64,
    edited_start: f64,
    timescale: f64,
}

/// `buildSourceToEditedMappings` (`captions.ts:98-128`), with the recording
/// offsets built from `editorInstance.recordings.segments[].display.duration`
/// -- here the pre-flight's `clip_display_durations`.
fn build_source_to_edited_mappings(
    timeline: &TimelineConfiguration,
    recording_durations: &[f64],
) -> Vec<SourceToEditedMapping> {
    let mut recording_offsets = Vec::with_capacity(recording_durations.len());
    let mut cumulative = 0.0;
    for duration in recording_durations {
        recording_offsets.push(cumulative);
        cumulative += duration;
    }

    let edited_offsets = clip_timeline_offsets(timeline);

    timeline
        .segments
        .iter()
        .zip(edited_offsets)
        .map(|(segment, edited_start)| {
            let recording_offset = recording_offsets
                .get(segment.recording_clip as usize)
                .copied()
                .unwrap_or(0.0);
            SourceToEditedMapping {
                source_start: recording_offset + segment.start,
                source_end: recording_offset + segment.end,
                edited_start,
                timescale: segment.timescale,
            }
        })
        .collect()
}

pub fn derive_caption_track_segments(
    source_segments: &[CaptionSegment],
    timeline: &TimelineConfiguration,
    recording_durations: &[f64],
) -> Vec<CaptionTrackSegment> {
    frameever_project::derive_caption_track_segments(source_segments, timeline, recording_durations)
}

/// `heldTimeBefore` (`timeline-holds.ts:45-50`): how much hold-extended
/// output time has passed before `time`.
fn held_time_before(holds: &[(f64, f64)], time: f64) -> f64 {
    holds
        .iter()
        .map(|(start, end)| (time.min(*end) - start).max(0.))
        .sum()
}

/// `mapEditedTimeToSource` (`captions.ts:428-467`) in its `"outgoing"`
/// preference -- the mode the caption override panel uses. The `"incoming"`
/// arm serves only the legacy non-source-timed inversion, which this build
/// never produces.
pub fn map_edited_time_to_source(
    edited_time: f64,
    timeline: &TimelineConfiguration,
    recording_durations: &[f64],
    source_range: Option<(f64, f64)>,
    recording_clip: Option<u32>,
) -> Option<(f64, u32)> {
    map_edited_time_to_piece(
        edited_time,
        timeline,
        recording_durations,
        source_range,
        recording_clip,
    )
    .map(|(time, piece)| (time, timeline.segments[piece].recording_clip))
}

fn media_end_before_next_clip(clip: u32, recording_durations: &[f64]) -> Option<f64> {
    let clip = clip as usize;
    (clip + 1 < recording_durations.len()).then(|| recording_durations[..=clip].iter().sum::<f64>())
}

fn map_edited_time_to_piece(
    edited_time: f64,
    timeline: &TimelineConfiguration,
    recording_durations: &[f64],
    source_range: Option<(f64, f64)>,
    recording_clip: Option<u32>,
) -> Option<(f64, usize)> {
    let edited_time = edited_time - held_time_before(&timeline.hold_windows(), edited_time);
    let media_end = |clip: u32| media_end_before_next_clip(clip, recording_durations);
    let mut best: Option<(u8, (f64, usize))> = None;
    for (piece, (mapping, segment)) in
        build_source_to_edited_mappings(timeline, recording_durations)
            .into_iter()
            .zip(&timeline.segments)
            .enumerate()
    {
        let edited_end =
            mapping.edited_start + (mapping.source_end - mapping.source_start) / mapping.timescale;
        if edited_time >= mapping.edited_start && edited_time <= edited_end {
            let hit = (
                mapping.source_start + (edited_time - mapping.edited_start) * mapping.timescale,
                piece,
            );
            let tied = recording_clip == Some(segment.recording_clip);
            let reach_end = match media_end(segment.recording_clip) {
                Some(end) if !tied => mapping.source_end.min(end),
                _ => mapping.source_end,
            };
            let overlaps = source_range.is_some_and(|(range_start, range_end)| {
                range_start < reach_end && range_end > mapping.source_start
            });
            let rank = match (tied, overlaps) {
                (true, true) => 0,
                (true, false) => 1,
                (false, true) => 2,
                (false, false) => 3,
            };
            if best.is_none_or(|(best_rank, _)| rank < best_rank) {
                best = Some((rank, hit));
            }
        }
    }
    best.map(|(_, hit)| hit)
}

/// `syncCaptionWordsWithText` (`captions.ts:593-632`): keep word timings when
/// only spellings changed, otherwise spread the tokens evenly.
pub fn sync_caption_words_with_text(
    text: &str,
    existing: &[CaptionWord],
    start: f64,
    end: f64,
) -> Vec<CaptionWord> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    if existing.len() == tokens.len() {
        return existing
            .iter()
            .zip(&tokens)
            .map(|(word, token)| CaptionWord {
                text: (*token).to_string(),
                start: word.start,
                end: word.end,
            })
            .collect();
    }
    let duration = (end - start).max(0.);
    let step = duration / tokens.len() as f64;
    tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            let source = existing.get(index);
            let word_start = start + step * index as f64;
            let word_end = if index == tokens.len() - 1 {
                end
            } else {
                start + step * (index + 1) as f64
            };
            CaptionWord {
                text: (*token).to_string(),
                start: source.map_or(word_start as f32, |word| word.start),
                end: source.map_or(word_end as f32, |word| word.end),
            }
        })
        .collect()
}

fn effective_to_output(holds: &[(f64, f64)], effective: f64, at_end: bool) -> f64 {
    let mut output = effective;
    for (start, end) in holds {
        if output > *start || (!at_end && output == *start) {
            output += end - start;
        } else {
            break;
        }
    }
    output
}

pub const MIN_TIED_CAPTION_SECS: f64 = 1e-3;

fn f32_at_or_after(time: f64) -> f32 {
    let rounded = time as f32;
    if f64::from(rounded) < time {
        rounded.next_up()
    } else {
        rounded
    }
}

fn f32_at_or_before(time: f64) -> f32 {
    let rounded = time as f32;
    if f64::from(rounded) > time {
        rounded.next_down()
    } else {
        rounded
    }
}

fn tied_caption_piece(
    timeline: &TimelineConfiguration,
    recording_durations: &[f64],
    caption: &CaptionSegment,
    item_id: &str,
) -> Option<((f64, f64), usize)> {
    let clip = caption.recording_clip?;
    let nth = item_id
        .rsplit_once("::edl")
        .and_then(|(_, index)| index.parse::<usize>().ok())
        .unwrap_or(0);
    let mappings = build_source_to_edited_mappings(timeline, recording_durations);
    let (piece, mapping) = mappings
        .iter()
        .enumerate()
        .filter(|(piece, mapping)| {
            let overlaps = |start: f64, end: f64| {
                start.max(mapping.source_start) < end.min(mapping.source_end)
            };
            timeline.segments[*piece].recording_clip == clip
                && if caption.words.is_empty() {
                    overlaps(f64::from(caption.start), f64::from(caption.end))
                } else {
                    caption.words.iter().any(|word| {
                        let end = word.end.min(word.start + MAX_CAPTION_WORD_DURATION);
                        overlaps(f64::from(word.start), f64::from(end))
                    })
                }
        })
        .nth(nth)?;
    let holds = timeline.hold_windows();
    let edited_end =
        mapping.edited_start + (mapping.source_end - mapping.source_start) / mapping.timescale;
    Some((
        (
            effective_to_output(&holds, mapping.edited_start, false),
            effective_to_output(&holds, edited_end, true),
        ),
        piece,
    ))
}

pub fn tied_caption_window(
    project: &ProjectConfiguration,
    index: usize,
    recording_durations: &[f64],
) -> Option<(f64, f64)> {
    let timeline = project.timeline.as_ref()?;
    let item = timeline.caption_segments.get(index)?;
    let source_id = source_caption_id(&item.id);
    let caption = project
        .captions
        .as_ref()?
        .segments
        .iter()
        .find(|segment| segment.id == source_id)?;
    tied_caption_piece(timeline, recording_durations, caption, &item.id).map(|(window, _)| window)
}

/// `updateSelectedCaption`'s source write-through (`CaptionsTab.tsx:257-315`):
/// after a track segment's start/end/text was edited in output time, route
/// the timing and content back onto the source-time caption master so the
/// edit survives future clip changes. Style overrides stay on the track and
/// are carried across by source id when re-derived.
pub fn write_caption_edit_to_source(
    project: &mut ProjectConfiguration,
    index: usize,
    recording_durations: &[f64],
) {
    let Some(timeline) = project.timeline.as_ref() else {
        return;
    };
    let Some(track_segment) = timeline.caption_segments.get(index) else {
        return;
    };
    let track_id = track_segment.id.clone();
    let source_id = source_caption_id(&track_id).to_string();
    let (track_start, track_end, track_text) = (
        track_segment.start,
        track_segment.end,
        track_segment.text.clone(),
    );

    let Some(captions) = project.captions.as_mut() else {
        return;
    };
    let timeline = project
        .timeline
        .as_ref()
        .expect("timeline presence was just checked");
    let Some(source) = captions
        .segments
        .iter_mut()
        .find(|segment| segment.id == source_id)
    else {
        return;
    };

    if source.recording_clip.is_some() {
        if let Some(((window_start, window_end), piece)) =
            tied_caption_piece(timeline, recording_durations, source, &track_id)
        {
            let (start, end) = (
                track_start.clamp(window_start, window_end),
                track_end.clamp(window_start, window_end),
            );
            let holds = timeline.hold_windows();
            let mapping = &build_source_to_edited_mappings(timeline, recording_durations)[piece];
            let to_source = |time: f64| {
                let effective = time - held_time_before(&holds, time);
                mapping.source_start + (effective - mapping.edited_start) * mapping.timescale
            };
            let (start, end) = (
                f32_at_or_after(to_source(start)),
                f32_at_or_before(to_source(end)),
            );
            if f64::from(end - start) / mapping.timescale > MIN_TIED_CAPTION_SECS {
                (source.start, source.end) = (start, end);
            }
        }
    } else {
        let source_range = Some((f64::from(source.start), f64::from(source.end)));
        let map = |time| {
            map_edited_time_to_piece(time, timeline, recording_durations, source_range, None)
        };
        let (start, end) = (map(track_start), map(track_end));
        let clip_of = |piece: usize| timeline.segments[piece].recording_clip;
        let end = match (start, end) {
            (Some((start, start_piece)), Some((end, end_piece)))
                if clip_of(start_piece) != clip_of(end_piece) && end <= start =>
            {
                build_source_to_edited_mappings(timeline, recording_durations)
                    .get(start_piece)
                    .map(|mapping| (mapping.source_end, start_piece))
            }
            _ => end,
        };
        if let Some((start, _)) = start {
            source.start = start as f32;
        }
        if let Some((end, piece)) = end {
            source.end = end as f32;
            source.recording_clip = frameever_project::held_tail_clip(
                clip_of(piece),
                f64::from(source.end),
                recording_durations,
            );
        }
    }
    source.text = track_text;
    if !is_manual_caption(source) {
        source.words = sync_caption_words_with_text(
            &source.text,
            &source.words,
            f64::from(source.start),
            f64::from(source.end),
        );
    }
}

pub fn is_manual_caption(segment: &CaptionSegment) -> bool {
    segment.words.is_empty() && segment.id.starts_with(MANUAL_CAPTION_PREFIX)
}

pub const MANUAL_CAPTION_PREFIX: &str = "caption-";

/// `applyCaptionResultToProject` (`captions.ts:469-527`): set the captions
/// block (enabled, preserving existing style settings), mark it source-timed,
/// and re-derive the rendered track through the current edit list.
pub fn apply_caption_result(
    project: &mut ProjectConfiguration,
    segments: Vec<CaptionSegment>,
    recording_durations: &[f64],
    recording_duration: f64,
) {
    let words: Vec<CaptionWord> = segments
        .iter()
        .flat_map(|segment| segment.words.iter().cloned())
        .collect();
    let settings = CaptionSettings {
        enabled: true,
        filler_language: filler_language(&words).map(str::to_string),
        ..project
            .captions
            .as_ref()
            .map(|captions| captions.settings.clone())
            .unwrap_or_default()
    };

    if project.timeline.is_none() {
        project.timeline = Some(TimelineConfiguration {
            segments: vec![TimelineSegment {
                recording_clip: 0,
                timescale: 1.0,
                start: 0.0,
                end: recording_duration,
                name: None,
                speed_audio_mode: None,
                hide_cursor: None,
                voice_eq: None,
                volume: None,
            }],
            transitions: Vec::new(),
            zoom_segments: Vec::new(),
            scene_segments: Vec::new(),
            mask_segments: Vec::new(),
            text_segments: Vec::new(),
            caption_segments: Vec::new(),
            keyboard_segments: Vec::new(),
            audio_segments: Vec::new(),
            camera3d_segments: Vec::new(),
            style_segments: Vec::new(),
            image_segments: Vec::new(),
            shape_segments: Vec::new(),
        });
    }
    let timeline = project
        .timeline
        .as_mut()
        .expect("timeline was just created");

    let mut segments = segments;
    if let Some(previous) = project
        .captions
        .as_ref()
        .filter(|captions| captions.source_timed)
    {
        segments.extend(
            previous
                .segments
                .iter()
                .filter(|segment| is_manual_caption(segment))
                .cloned(),
        );
        segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    }

    timeline.caption_segments =
        derive_caption_track_segments(&segments, timeline, recording_durations);

    project.captions = Some(CaptionsData {
        segments,
        settings,
        source_timed: true,
    });
}

/// `getCaptionGenerationErrorMessage` (`captions.ts:634-662`).
pub fn caption_generation_error_message(message: &str) -> String {
    if message.contains("No audio stream found") {
        return t!("No audio found in the video file").to_string();
    }
    if message.contains("Model file not found") {
        return t!("Caption model not found. Please download it first").to_string();
    }
    if message.contains("Failed to load Whisper model") {
        return t!("Failed to load the caption model. Try downloading it again").to_string();
    }
    if message.contains(PARAKEET_UNSUPPORTED_LANGUAGE) {
        return t!("The Recommended and High Accuracy models only transcribe English and 24 other European languages. Download the Small or Medium model to caption this language").to_string();
    }
    if message.contains("Parakeet transcription is not available on Intel macOS") {
        return t!("Parakeet models are not available on Intel Macs. Use a Whisper model instead")
            .to_string();
    }
    message.to_string()
}

#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod verify_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn dylib(commands: &[(u32, Vec<u32>)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for word in [
            0xfeed_facf,
            0x0100_000c,
            0,
            6,
            commands.len() as u32,
            0,
            0,
            0,
        ] {
            bytes.extend_from_slice(&u32::to_le_bytes(word));
        }
        for (cmd, body) in commands {
            bytes.extend_from_slice(&cmd.to_le_bytes());
            bytes.extend_from_slice(&((8 + 4 * body.len()) as u32).to_le_bytes());
            for word in body {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
        }
        bytes
    }

    #[test]
    fn the_onnx_runtime_dylib_names_the_macos_it_needs() {
        let ort_1_30 = dylib(&[
            (0x1b, vec![0; 4]),
            (0x32, vec![1, 0x000e_0000, 0x001a_0200, 0]),
        ]);
        assert_eq!(macos_needed_by_dylib(&ort_1_30), Ok(Some((14, 0, 0))));
        let older = dylib(&[(0x24, vec![0x000a_0f00, 0x000e_0000])]);
        assert_eq!(macos_needed_by_dylib(&older), Ok(Some((10, 15, 0))));
        assert_eq!(
            macos_needed_by_dylib(&dylib(&[(0x1b, vec![0; 4])])),
            Ok(None)
        );
        assert_eq!(
            macos_needed_by_dylib(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2]),
            Ok(None)
        );
        assert_eq!(macos_needed_by_dylib(b""), Err(()));
        assert_eq!(macos_needed_by_dylib(b"not a library at all"), Err(()));
    }

    #[test]
    fn macos_versions_compare_as_numbers() {
        assert_eq!(parse_macos_version("13.6.1"), Some((13, 6, 1)));
        assert_eq!(parse_macos_version("14.0"), Some((14, 0, 0)));
        assert_eq!(parse_macos_version("26"), Some((26, 0, 0)));
        assert_eq!(parse_macos_version("x"), None);
        assert!(parse_macos_version("13.6.1") < Some((14, 0, 0)));
        assert!(parse_macos_version("14.2") >= Some((14, 0, 0)));
        assert!(parse_macos_version("26.1") >= Some((14, 0, 0)));
    }

    #[test]
    fn hearing_nothing_is_no_speech_and_an_engine_error_is_a_failure_whatever_it_says() {
        assert_eq!(
            heard_segments(Ok(Vec::new())).err(),
            Some(TranscriptionError::NoSpeech)
        );
        let worded_like_no_speech = t!("No speech detected in the audio").to_string();
        assert_eq!(
            heard_segments(Err(worded_like_no_speech.clone())).err(),
            Some(TranscriptionError::Failed(worded_like_no_speech))
        );
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    fn install(root: &Path, dir: &str, files: &[&str]) -> PathBuf {
        let dir = root.join(dir);
        std::fs::create_dir_all(&dir).unwrap();
        for file in files {
            std::fs::write(dir.join(file), b"x").unwrap();
        }
        dir
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    fn recommended_parakeet_needs_the_float_activation_encoder() {
        let root = tempfile::tempdir().unwrap();
        let dynamic_int8 = install(
            root.path(),
            "old/parakeet-best",
            &[
                "encoder-model.int8.onnx",
                "decoder_joint-model.int8.onnx",
                "vocab.txt",
            ],
        );
        assert!(!parakeet_model_installed(&dynamic_int8));

        let recommended = install(
            root.path(),
            "new/parakeet-best",
            &[
                "encoder-model.onnx",
                "decoder_joint-model.int8.onnx",
                "vocab.txt",
            ],
        );
        assert!(parakeet_model_installed(&recommended));
        let encoder_urls = PARAKEET_TDT_INT8_MODEL_FILES
            .iter()
            .find(|(file, _)| *file == "encoder-model.onnx")
            .map(|(_, urls)| *urls)
            .unwrap();
        assert!(
            encoder_urls
                .iter()
                .all(|url| parakeet_known_part_size(url).is_some() && !url.contains(".int8."))
        );

        let full = install(
            root.path(),
            "full/parakeet-best-max",
            &[
                "encoder-model.onnx",
                "encoder-model.onnx.data",
                "decoder_joint-model.onnx",
                "vocab.txt",
            ],
        );
        assert!(parakeet_model_installed(&full));
        std::fs::remove_file(full.join("encoder-model.onnx.data")).unwrap();
        assert!(!parakeet_model_installed(&full));
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    fn update_note_shows_only_for_the_old_recommended_download() {
        let root = tempfile::tempdir().unwrap();
        let old = install(
            root.path(),
            "old/parakeet-best",
            &[
                "encoder-model.int8.onnx",
                "decoder_joint-model.int8.onnx",
                "vocab.txt",
            ],
        );
        let current = install(
            root.path(),
            "current/parakeet-best",
            &[
                "encoder-model.onnx",
                "decoder_joint-model.int8.onnx",
                "vocab.txt",
            ],
        );

        assert!(parakeet_model_outdated(&old));
        assert!(!parakeet_model_outdated(&current));
        assert!(!parakeet_model_outdated(
            &root.path().join("fresh/parakeet-best")
        ));
        assert!(!parakeet_model_outdated(&install(
            root.path(),
            "empty/parakeet-best",
            &[]
        )));
    }

    #[test]
    fn hearing_no_speech_is_an_empty_state_not_an_error() {
        let path = PathBuf::from("/tmp/frameever-no-speech-state-test.ever");
        assert!(begin_generation(&path));
        finish_generation_without_speech(&path);
        let snapshot = ui_snapshot(&path);
        assert!(snapshot.no_speech && !snapshot.generating);
        assert_eq!(snapshot.generation_error, None);

        assert!(begin_generation(&path));
        assert!(!ui_snapshot(&path).no_speech);
        finish_generation(&path, Some("boom".to_string()));
        let snapshot = ui_snapshot(&path);
        assert!(!snapshot.no_speech);
        assert_eq!(snapshot.generation_error.as_deref(), Some("boom"));
        finish_generation(&path, None);
    }

    fn engine(model: &str, language: &str) -> CaptionEngine {
        CaptionEngine {
            model: model.to_string(),
            language: language.to_string(),
        }
    }

    #[test]
    fn chinese_moves_from_parakeet_to_a_downloaded_whisper_model() {
        let no_detection =
            |_: &str| -> Option<String> { panic!("no detection for a set language") };
        assert_eq!(
            caption_engine("best", "zh", |_| true, no_detection),
            Ok(engine("medium", "zh"))
        );
        assert_eq!(
            caption_engine("best-max", "zh", |model| model == "small", no_detection),
            Ok(engine("small", "zh"))
        );
        let error = caption_engine("best", "zh", |_| false, no_detection).unwrap_err();
        assert_eq!(
            caption_generation_error_message(&error),
            "The Recommended and High Accuracy models only transcribe English and 24 other European languages. Download the Small or Medium model to caption this language"
        );
    }

    #[test]
    fn auto_detected_chinese_moves_from_parakeet_to_whisper() {
        assert_eq!(
            caption_engine(
                "best",
                "auto",
                |model| model == "small",
                |whisper| {
                    assert_eq!(whisper, "small");
                    Some("zh".to_string())
                }
            ),
            Ok(engine("small", "zh"))
        );
        assert_eq!(
            caption_engine("best", "auto", |_| true, |_| Some("de".to_string())),
            Ok(engine("best", "auto"))
        );
        assert_eq!(
            caption_engine("best", "auto", |_| true, |_| None),
            Ok(engine("best", "auto"))
        );
        assert_eq!(
            caption_engine(
                "best",
                "auto",
                |_| false,
                |_| -> Option<String> { panic!("no Whisper model to detect with") }
            ),
            Ok(engine("best", "auto"))
        );
    }

    #[test]
    fn european_languages_and_whisper_selections_keep_their_model() {
        let no_detection =
            |_: &str| -> Option<String> { panic!("no detection for a set language") };
        for language in ["en", "de", "ru", "uk", "pl"] {
            assert_eq!(
                caption_engine("best-max", language, |_| true, no_detection),
                Ok(engine("best-max", language))
            );
        }
        assert_eq!(
            caption_engine("small", "zh", |_| false, no_detection),
            Ok(engine("small", "zh"))
        );
        assert_eq!(
            caption_engine("medium", "auto", |_| true, no_detection),
            Ok(engine("medium", "auto"))
        );
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    fn parakeet_chunks_cut_in_the_quiet_gap_and_cover_every_sample() {
        let rate = 16_000;
        let mut samples: Vec<f32> = (0..rate * 150).map(|i| (i as f32 * 0.05).sin()).collect();
        samples[rate * 57..rate * 57 + rate / 2].fill(0.0);

        let ranges = parakeet_chunk_ranges(&samples, rate);

        assert_eq!(ranges.len(), 3);
        assert!((rate * 57..rate * 57 + rate / 2).contains(&ranges[0].end));
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges.last().unwrap().end, samples.len());
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        assert!(
            ranges
                .iter()
                .all(|r| r.len() <= rate * PARAKEET_CHUNK_SECONDS)
        );
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    fn parakeet_keeps_a_one_minute_recording_in_one_chunk() {
        let samples = vec![0.5; 16_000 * PARAKEET_CHUNK_SECONDS];

        assert_eq!(
            parakeet_chunk_ranges(&samples, 16_000),
            vec![0..samples.len()]
        );
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    #[ignore = "needs FRAMEEVER_PARAKEET_BENCH_MODEL and FRAMEEVER_PARAKEET_BENCH_WAV"]
    fn benchmark_parakeet_transcription() {
        let model = std::env::var("FRAMEEVER_PARAKEET_BENCH_MODEL")
            .expect("FRAMEEVER_PARAKEET_BENCH_MODEL");
        let wav = PathBuf::from(
            std::env::var("FRAMEEVER_PARAKEET_BENCH_WAV").expect("FRAMEEVER_PARAKEET_BENCH_WAV"),
        );
        let started = std::time::Instant::now();
        let result = process_with_parakeet(&wav, &model);
        let seconds = started.elapsed().as_secs_f64();
        match result {
            Ok(heard) => {
                let words: Vec<&CaptionWord> = heard.groups.iter().flatten().collect();
                println!("parakeet_seconds={seconds:.2} words={}", words.len());
                if let Ok(path) = std::env::var("FRAMEEVER_PARAKEET_BENCH_WORDS") {
                    let dump: String = words
                        .iter()
                        .map(|w| format!("{}\t{:.2}\t{:.2}\n", w.text, w.start, w.end))
                        .collect();
                    std::fs::write(path, dump).expect("write words");
                }
            }
            Err(error) => panic!("parakeet_seconds={seconds:.2} error={error}"),
        }
    }

    #[test]
    #[ignore = "needs FRAMEEVER_CAPTIONS_PROJECT, FRAMEEVER_CAPTIONS_MODEL and FRAMEEVER_CAPTIONS_LANGUAGE"]
    fn caption_a_real_project() {
        let project_path = PathBuf::from(
            std::env::var("FRAMEEVER_CAPTIONS_PROJECT").expect("FRAMEEVER_CAPTIONS_PROJECT"),
        );
        let model = std::env::var("FRAMEEVER_CAPTIONS_MODEL").expect("FRAMEEVER_CAPTIONS_MODEL");
        let language =
            std::env::var("FRAMEEVER_CAPTIONS_LANGUAGE").expect("FRAMEEVER_CAPTIONS_LANGUAGE");
        let started = std::time::Instant::now();
        let segments =
            transcribe_blocking(&project_path, &model, &language).unwrap_or_else(|error| {
                panic!("{}", caption_generation_error_message(&error.to_string()))
            });
        let mut project = ProjectConfiguration::load(&project_path).expect("project-config.json");
        let source_end = segments
            .last()
            .map_or(0.0, |segment| f64::from(segment.end));
        apply_caption_result(&mut project, segments, &[source_end], source_end);
        let track = &project.timeline.as_ref().unwrap().caption_segments;
        println!(
            "seconds={:.2} source_captions={} timeline_captions={}",
            started.elapsed().as_secs_f64(),
            project.captions.as_ref().unwrap().segments.len(),
            track.len()
        );
        for caption in track.iter().take(12) {
            println!("{:.2}-{:.2} {}", caption.start, caption.end, caption.text);
        }
    }

    fn word(text: &str, index: usize) -> CaptionWord {
        CaptionWord {
            text: text.to_string(),
            start: index as f32,
            end: index as f32 + 0.5,
        }
    }

    fn timeline(json: serde_json::Value) -> TimelineConfiguration {
        serde_json::from_value(json).expect("fixture parses")
    }

    fn caption(id: &str, start: f32, end: f32, words: &[(&str, f32, f32)]) -> CaptionSegment {
        let words: Vec<CaptionWord> = words
            .iter()
            .map(|(text, start, end)| CaptionWord {
                text: (*text).to_string(),
                start: *start,
                end: *end,
            })
            .collect();
        CaptionSegment {
            id: id.to_string(),
            start,
            end,
            text: caption_text_from_words(&words),
            words,
            recording_clip: None,
        }
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    fn idle_release_drops_cached_value_after_delay() {
        static SLOT: Mutex<()> = Mutex::new(());
        static CACHE: Mutex<Option<u32>> = Mutex::new(Some(7));
        static LAST_USE: AtomicU64 = AtomicU64::new(0);

        let handle = schedule_idle_release(&SLOT, &CACHE, &LAST_USE, Duration::from_millis(10))
            .expect("spawns");

        assert!(handle.join().expect("joins"));
        assert!(CACHE.lock().unwrap().is_none());
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    fn idle_release_is_cancelled_by_a_newer_run() {
        static SLOT: Mutex<()> = Mutex::new(());
        static CACHE: Mutex<Option<u32>> = Mutex::new(Some(7));
        static LAST_USE: AtomicU64 = AtomicU64::new(0);

        let slot = SLOT.lock().unwrap();
        let older =
            schedule_idle_release(&SLOT, &CACHE, &LAST_USE, Duration::ZERO).expect("spawns");
        LAST_USE.fetch_add(1, Ordering::AcqRel);
        drop(slot);

        assert!(!older.join().expect("joins"));
        assert_eq!(*CACHE.lock().unwrap(), Some(7));
    }

    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[test]
    fn idle_release_waits_for_a_run_in_flight() {
        static SLOT: Mutex<()> = Mutex::new(());
        static CACHE: Mutex<Option<u32>> = Mutex::new(Some(7));
        static LAST_USE: AtomicU64 = AtomicU64::new(0);

        let run = SLOT.lock().unwrap();
        let handle =
            schedule_idle_release(&SLOT, &CACHE, &LAST_USE, Duration::ZERO).expect("spawns");
        std::thread::sleep(Duration::from_millis(50));
        assert!(!handle.is_finished());
        assert_eq!(*CACHE.lock().unwrap(), Some(7));
        drop(run);

        assert!(handle.join().expect("joins"));
        assert!(CACHE.lock().unwrap().is_none());
    }

    // -- captions.rs's own unit tests, carried over --------------------------

    fn timed_words(words: &[CaptionWord]) -> Vec<(&str, f32, f32)> {
        words
            .iter()
            .map(|word| (word.text.as_str(), word.start, word.end))
            .collect()
    }

    #[test]
    fn whisper_words_rebuild_characters_split_across_tokens() {
        let jia = "家".as_bytes();
        let zhong = "众".as_bytes();
        let tokens: Vec<(&[u8], f32, f32)> = vec![
            ("[_BEG_]".as_bytes(), 0.0, 0.0),
            (" 大".as_bytes(), 0.0, 0.3),
            (&jia[..2], 0.3, 0.4),
            (&jia[2..], 0.4, 0.5),
            ("好".as_bytes(), 0.5, 0.7),
            ("，".as_bytes(), 0.7, 0.75),
            ("观".as_bytes(), 0.75, 0.9),
            (&zhong[..1], 0.9, 1.0),
            (&zhong[1..], 1.0, 1.2),
            ("。".as_bytes(), 1.2, 1.3),
        ];
        assert_eq!(
            timed_words(&normalize_caption_words(whisper_words(tokens))),
            vec![
                ("大", 0.0, 0.3),
                ("家", 0.3, 0.5),
                ("好，", 0.5, 0.75),
                ("观", 0.75, 0.9),
                ("众。", 0.9, 1.3),
            ]
        );
    }

    #[test]
    fn whisper_words_without_duration_last_until_the_next_word() {
        let tokens: Vec<(&[u8], f32, f32)> = vec![
            ("我".as_bytes(), 3.0, 3.2),
            ("来".as_bytes(), 3.2, 3.2),
            ("演".as_bytes(), 3.25, 3.25),
            ("示".as_bytes(), 3.4, 3.6),
            ("吧".as_bytes(), 3.7, 3.7),
        ];
        assert_eq!(
            timed_words(&whisper_words(tokens)),
            vec![
                ("我", 3.0, 3.2),
                ("来", 3.2, 3.25),
                ("演", 3.25, 3.4),
                ("示", 3.4, 3.6),
                ("吧", 3.7, 3.8),
            ]
        );
    }

    #[test]
    fn whisper_words_join_latin_tokens_until_a_space() {
        let tokens: Vec<(&[u8], f32, f32)> = vec![
            (" Hello".as_bytes(), 0.0, 0.4),
            (",".as_bytes(), 0.4, 0.5),
            (" Frame".as_bytes(), 0.6, 0.8),
            ("Ever".as_bytes(), 0.8, 1.0),
            ("<|endoftext|>".as_bytes(), 1.0, 1.0),
        ];
        assert_eq!(
            timed_words(&whisper_words(tokens)),
            vec![("Hello,", 0.0, 0.5), ("FrameEver", 0.6, 1.0)]
        );
    }

    #[test]
    fn whisper_words_start_a_latin_word_after_full_width_punctuation() {
        let tokens: Vec<(&[u8], f32, f32)> = vec![
            ("好".as_bytes(), 0.0, 0.2),
            ("，".as_bytes(), 0.2, 0.3),
            ("OK".as_bytes(), 0.3, 0.5),
        ];
        assert_eq!(
            timed_words(&whisper_words(tokens)),
            vec![("好", 0.0, 0.2), ("，", 0.2, 0.3), ("OK", 0.3, 0.5)]
        );
    }

    #[test]
    fn chinese_whisper_prompt_asks_for_simplified_chinese() {
        let hints = [
            "帧刻".to_string(),
            "FrameEver".to_string(),
            " 帧刻 ".to_string(),
        ];
        assert_eq!(
            build_initial_prompt("zh", &hints).as_deref(),
            Some(
                "以下是普通话的句子，使用简体中文和标点符号。专有名词：帧刻、FrameEver。那我觉得，嗯，我们先，呃，开始吧。"
            )
        );
        assert_eq!(
            build_initial_prompt("zh", &[]).as_deref(),
            Some("以下是普通话的句子，使用简体中文和标点符号。那我觉得，嗯，我们先，呃，开始吧。")
        );
    }

    #[test]
    fn english_whisper_prompt_keeps_filler_words_after_the_hints() {
        let hints = ["帧刻".to_string(), "FrameEver".to_string()];
        assert_eq!(
            build_initial_prompt("en", &hints).as_deref(),
            Some(
                "Preferred spellings, names, and capitalization for this transcript: 帧刻; FrameEver. So, um, I think, uh, we should start."
            )
        );
        assert_eq!(
            build_initial_prompt("en", &[]).as_deref(),
            Some("So, um, I think, uh, we should start.")
        );
        assert_eq!(
            build_initial_prompt("de", &hints).as_deref(),
            Some(
                "Preferred spellings, names, and capitalization for this transcript: 帧刻; FrameEver"
            )
        );
        assert_eq!(build_initial_prompt("auto", &[]), None);
    }

    fn spoken(texts: &[&str]) -> Vec<CaptionWord> {
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| word(text, index))
            .collect()
    }

    fn segment_texts(segments: &[CaptionSegment]) -> Vec<Vec<&str>> {
        segments
            .iter()
            .map(|segment| {
                segment
                    .words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn english_captions_do_not_count_hesitations() {
        let verbatim = spoken(&[
            "So,",
            "um,",
            "I",
            "think",
            "we",
            "should,",
            "uh,",
            "start",
            "with",
            "the",
            "dashboard",
            "today",
            "please",
            "now",
        ]);
        let segments = caption_segments_from_groups(vec![verbatim]);
        assert_eq!(
            segment_texts(&segments),
            vec![
                vec!["So,", "um,", "I", "think", "we", "should,", "uh,", "start"],
                vec!["with", "the", "dashboard", "today", "please", "now"],
            ]
        );
        assert_eq!(segments[0].id, "segment-0");
        assert_eq!(segments[1].id, "segment-1");
    }

    #[test]
    fn chinese_captions_do_not_count_hesitations() {
        let verbatim = spoken(&[
            "嗯，", "我们", "觉得", "呃", "应该", "先做", "这个", "功能", "然后", "再来", "发布",
            "一下",
        ]);
        let segments = caption_segments_from_groups(vec![verbatim]);
        assert_eq!(
            segment_texts(&segments),
            vec![
                vec!["嗯，", "我们", "觉得", "呃", "应该", "先做", "这个", "功能"],
                vec!["然后", "再来", "发布", "一下"],
            ]
        );
    }

    #[test]
    fn captions_in_other_languages_count_every_word() {
        let german = spoken(&[
            "Er", "kommt", "um", "acht", "und", "bleibt", "bis", "zehn", "Uhr", "heute",
        ]);
        let segments = caption_segments_from_groups(vec![german]);
        assert_eq!(segments[0].words.len(), 6);
    }

    #[test]
    fn caption_groups_never_share_a_caption() {
        let segments = caption_segments_from_groups(vec![
            spoken(&["Hello", "there"]),
            Vec::new(),
            spoken(&["we", "ship", "today"]),
        ]);
        assert_eq!(
            segment_texts(&segments),
            vec![vec!["Hello", "there"], vec!["we", "ship", "today"]]
        );
        assert_eq!(segments[0].id, "segment-0-0");
        assert_eq!(segments[1].id, "segment-1-0");
    }

    #[test]
    fn applied_captions_remember_the_transcript_language() {
        let caption = |texts: &[&str]| CaptionSegment {
            id: "segment-0".into(),
            start: 0.0,
            end: texts.len() as f32 * 0.5,
            text: texts.join(" "),
            words: spoken(texts),
            recording_clip: None,
        };
        for (texts, expected) in [
            (
                &["so", "I", "think", "we", "ship", "the", "app"][..],
                Some("en"),
            ),
            (&["嗯，", "我们", "开始"][..], Some("zh")),
            (&["Er", "kommt", "um", "acht"][..], None),
        ] {
            let mut project = ProjectConfiguration::default();
            apply_caption_result(&mut project, vec![caption(texts)], &[5.0], 5.0);
            assert_eq!(
                project
                    .captions
                    .unwrap()
                    .settings
                    .filler_language
                    .as_deref(),
                expected
            );
        }
    }

    #[test]
    fn ascii_punctuation_after_chinese_becomes_full_width() {
        let words = ["好", ",", "吗", "?", "OK", ","]
            .iter()
            .enumerate()
            .map(|(index, text)| word(text, index))
            .collect();
        let texts: Vec<String> = normalize_caption_words(words)
            .into_iter()
            .map(|word| word.text)
            .collect();
        assert_eq!(texts, ["好，", "吗？", "OK,"]);
    }

    #[test]
    fn normalize_caption_words_attaches_punctuation() {
        let words = normalize_caption_words(vec![
            word("test", 0),
            word(",", 1),
            word("test", 2),
            word(".", 3),
        ]);

        assert_eq!(caption_text_from_words(&words), "test, test.");
        assert_eq!(words.len(), 2);
    }

    #[test]
    fn normalize_caption_words_clamps_inflated_trailing_word() {
        let words = normalize_caption_words(vec![CaptionWord {
            text: "seconds.".to_string(),
            start: 53.92,
            end: 70.16,
        }]);

        assert_eq!(words.len(), 1);
        assert!((words[0].end - (53.92 + MAX_CAPTION_WORD_DURATION)).abs() < 1e-4);
    }

    #[test]
    fn caption_word_chunks_do_not_end_on_short_connector_when_more_words_follow() {
        for connector in ["I", "à", "à,", "a\u{300}", "日"] {
            let words = [
                "This", "is", "where", "we", "record", connector, "want", "clean", "captions",
            ]
            .iter()
            .enumerate()
            .map(|(index, text)| word(text, index))
            .collect::<Vec<_>>();

            let chunks = caption_word_chunks(&words);

            assert_eq!(
                caption_text_from_words(chunks[0]),
                format!("This is where we record {connector} want")
            );
            assert_eq!(caption_text_from_words(chunks[1]), "clean captions");
        }
    }

    #[test]
    fn caption_word_chunks_merge_short_tail_starting_with_connector() {
        for connector in ["I", "à", "à,", "a\u{300}", "日"] {
            let words = [
                "These", "captions", "should", "stay", "grouped", "together", connector, "Paris",
            ]
            .iter()
            .enumerate()
            .map(|(index, text)| word(text, index))
            .collect::<Vec<_>>();

            let chunks = caption_word_chunks(&words);

            assert_eq!(chunks.len(), 1, "connector {connector}");
            assert_eq!(
                caption_text_from_words(chunks[0]),
                format!("These captions should stay grouped together {connector} Paris")
            );
        }
    }

    // -- Model paths ----------------------------------------------------------

    #[test]
    fn whisper_uses_its_own_default_counting_only_the_cores_it_may_use() {
        assert_eq!(whisper_threads(Some(1)), 1);
        assert_eq!(whisper_threads(Some(2)), 2);
        assert_eq!(whisper_threads(Some(3)), 3);
        assert_eq!(whisper_threads(Some(4)), 4);
        assert_eq!(whisper_threads(Some(16)), 4);
        assert_eq!(whisper_threads(None), 4);
    }

    #[test]
    fn model_paths_sit_in_the_caption_models_folder() {
        let base = models_dir();
        assert_eq!(base, frameever_utils::storage::caption_models_dir());
        assert_eq!(model_path("best"), base.join("parakeet-best"));
        assert_eq!(model_path("best-max"), base.join("parakeet-best-max"));
        assert_eq!(model_path("small"), base.join("small.bin"));
        assert_eq!(model_path("medium"), base.join("medium.bin"));
    }

    #[test]
    fn downloads_generation_and_deletion_block_application_handoffs() {
        let mut state = Hub::default();
        assert!(!state.work_in_flight());

        state.download = Some(ModelDownload {
            model: "small".to_string(),
            state: DownloadState::Downloading,
            progress: 25.0,
            message: "Downloading".to_string(),
        });
        assert!(state.work_in_flight());

        state.download.as_mut().unwrap().state = DownloadState::Completed;
        assert!(!state.work_in_flight());

        state.generating.insert(PathBuf::from("/tmp/recording.cap"));
        assert!(state.work_in_flight());
        state.generating.clear();

        state.deleting = Some("small".to_string());
        assert!(state.work_in_flight());
    }

    #[test]
    fn source_caption_id_strips_the_edl_suffix() {
        assert_eq!(source_caption_id("segment-0-1"), "segment-0-1");
        assert_eq!(source_caption_id("segment-0-1::edl3"), "segment-0-1");
    }

    // -- captions.ts's derivation vitest cases, carried over ------------------

    #[test]
    fn derive_splits_caption_words_across_retained_timeline_ranges() {
        let timeline = timeline(serde_json::json!({
            "segments": [
                { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 1.0 },
                { "recordingSegment": 0, "timescale": 1.0, "start": 2.0, "end": 3.0 }
            ],
            "zoomSegments": []
        }));
        let source = vec![caption(
            "caption",
            0.4,
            2.3,
            &[("hello", 0.4, 0.6), ("world", 2.1, 2.3)],
        )];

        let track = derive_caption_track_segments(&source, &timeline, &[4.0]);

        assert_eq!(track.len(), 2);
        assert_eq!(track[0].id, "caption::edl0");
        assert_eq!(track[0].text, "hello");
        assert!((track[0].start - 0.4).abs() < 1e-6);
        assert!((track[0].end - 0.6).abs() < 1e-6);
        assert_eq!(track[1].id, "caption::edl1");
        assert_eq!(track[1].text, "world");
        assert!((track[1].start - 1.1).abs() < 1e-6);
        assert!((track[1].end - 1.3).abs() < 1e-6);
    }

    #[test]
    fn derive_keeps_chinese_captions_free_of_spaces_across_cuts() {
        let timeline = timeline(serde_json::json!({
            "segments": [
                { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 1.0 },
                { "recordingSegment": 0, "timescale": 1.0, "start": 2.0, "end": 3.0 }
            ],
            "zoomSegments": []
        }));
        let source = vec![caption(
            "caption",
            0.2,
            2.6,
            &[
                ("大", 0.2, 0.4),
                ("家", 0.4, 0.6),
                ("好，", 0.6, 0.8),
                ("欢", 2.1, 2.3),
                ("迎", 2.3, 2.6),
            ],
        )];

        let track = derive_caption_track_segments(&source, &timeline, &[4.0]);

        let texts: Vec<&str> = track.iter().map(|segment| segment.text.as_str()).collect();
        assert_eq!(texts, ["大家好，", "欢迎"]);
    }

    #[test]
    fn derive_clamps_an_inflated_trailing_word_across_cuts() {
        let timeline = timeline(serde_json::json!({
            "segments": [
                { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 1.5 },
                { "recordingSegment": 0, "timescale": 1.0, "start": 10.0, "end": 12.0 }
            ],
            "zoomSegments": []
        }));
        let source = vec![caption(
            "caption",
            0.4,
            16.4,
            &[("a", 0.4, 0.5), ("few", 0.5, 0.8), ("seconds.", 0.8, 16.4)],
        )];

        let track = derive_caption_track_segments(&source, &timeline, &[20.0]);

        assert_eq!(track.len(), 1);
        assert_eq!(track[0].id, "caption");
        assert_eq!(track[0].text, "a few seconds.");
        assert_eq!(track[0].words[2].text, "seconds.");
        assert!((track[0].words[2].end - 1.5).abs() < 1e-4);
    }

    #[test]
    fn derive_projects_captions_into_hold_extended_output_time() {
        let timeline = timeline(serde_json::json!({
            "segments": [
                { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 10.0 }
            ],
            "zoomSegments": [],
            "textSegments": [
                { "start": 2.0, "end": 5.0, "layout": "fullscreen" }
            ]
        }));

        let after = derive_caption_track_segments(
            &[caption("caption", 4.0, 5.0, &[("later", 4.0, 5.0)])],
            &timeline,
            &[10.0],
        );
        assert_eq!(after.len(), 1);
        assert!((after[0].start - 7.0).abs() < 1e-6);
        assert!((after[0].end - 8.0).abs() < 1e-6);

        let before = derive_caption_track_segments(
            &[caption("caption", 1.0, 2.0, &[("before", 1.0, 2.0)])],
            &timeline,
            &[10.0],
        );
        assert!((before[0].start - 1.0).abs() < 1e-6);
        assert!((before[0].end - 2.0).abs() < 1e-6);

        let across = derive_caption_track_segments(
            &[caption(
                "caption",
                1.0,
                3.0,
                &[("across", 1.0, 2.0), ("it", 2.0, 3.0)],
            )],
            &timeline,
            &[10.0],
        );
        assert_eq!(across.len(), 1);
        assert!((across[0].start - 1.0).abs() < 1e-6);
        assert!((across[0].end - 6.0).abs() < 1e-6);
        assert!((across[0].words[0].end - 2.0).abs() < 1e-4);
        assert!((across[0].words[1].start - 5.0).abs() < 1e-4);
    }

    #[test]
    fn derive_follows_clip_reordering_and_carries_overrides_by_source_id() {
        let mut timeline = timeline(serde_json::json!({
            "segments": [
                { "recordingSegment": 0, "timescale": 1.0, "start": 5.0, "end": 8.0 },
                { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 3.0 }
            ],
            "zoomSegments": []
        }));
        let source = vec![
            caption("capA", 1.0, 2.0, &[("a", 1.0, 2.0)]),
            caption("capB", 6.0, 7.0, &[("b", 6.0, 7.0)]),
        ];

        let track = derive_caption_track_segments(&source, &timeline, &[10.0]);
        assert_eq!(
            track.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["capB", "capA"]
        );
        assert!((track[0].start - 1.0).abs() < 1e-6);
        assert!((track[1].start - 4.0).abs() < 1e-6);

        timeline.caption_segments = track;
        timeline.caption_segments[1].font_size_override = Some(42);

        let rederived = derive_caption_track_segments(&source, &timeline, &[10.0]);
        assert_eq!(
            rederived
                .iter()
                .find(|s| s.id == "capA")
                .and_then(|s| s.font_size_override),
            Some(42)
        );
    }

    #[test]
    fn derive_drops_captions_whose_content_was_cut_out() {
        let timeline = timeline(serde_json::json!({
            "segments": [
                { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 3.0 }
            ],
            "zoomSegments": []
        }));
        let source = vec![
            caption("capA", 1.0, 2.0, &[("a", 1.0, 2.0)]),
            caption("capB", 6.0, 7.0, &[("b", 6.0, 7.0)]),
        ];

        let track = derive_caption_track_segments(&source, &timeline, &[10.0]);
        assert_eq!(
            track.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["capA"]
        );
    }

    // -- applyCaptionResultToProject ------------------------------------------

    #[test]
    fn a_new_transcript_keeps_captions_typed_in_by_hand() {
        let mut project = ProjectConfiguration {
            timeline: Some(timeline(serde_json::json!({
                "segments": [{ "timescale": 1.0, "start": 0.0, "end": 10.0 }],
                "zoomSegments": []
            }))),
            captions: Some(CaptionsData {
                segments: vec![
                    caption("segment-0", 1.0, 2.0, &[("old", 1.0, 2.0)]),
                    caption("caption-1-a", 5.0, 7.0, &[]),
                    caption("segment-legacy", 8.0, 9.0, &[]),
                ],
                source_timed: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        project.captions.as_mut().unwrap().segments[1].text = "Chapter two".into();
        project.captions.as_mut().unwrap().segments[2].text = "legacy".into();

        apply_caption_result(
            &mut project,
            vec![
                caption("segment-0", 0.5, 1.5, &[("hello", 0.5, 1.5)]),
                caption("segment-1", 6.0, 6.5, &[("world", 6.0, 6.5)]),
            ],
            &[10.0],
            10.0,
        );

        let ids: Vec<&str> = project
            .captions
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .map(|segment| segment.id.as_str())
            .collect();
        assert_eq!(ids, ["segment-0", "caption-1-a", "segment-1"]);
        let track: Vec<&str> = project
            .timeline
            .as_ref()
            .unwrap()
            .caption_segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        assert_eq!(track, ["hello", "Chapter two", "world"]);
    }

    #[test]
    fn editing_a_caption_typed_in_by_hand_keeps_it_out_of_the_transcript() {
        let mut project = ProjectConfiguration {
            timeline: Some(timeline(serde_json::json!({
                "segments": [{ "timescale": 1.0, "start": 0.0, "end": 10.0 }],
                "zoomSegments": []
            }))),
            captions: Some(CaptionsData {
                segments: vec![caption("caption-1-a", 5.0, 7.0, &[])],
                source_timed: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let durations = [10.0];
        frameever_project::synchronize_captions(&mut project, &durations);
        let track = &mut project.timeline.as_mut().unwrap().caption_segments[0];
        track.text = "Step one: open settings".into();
        track.end = 8.0;

        write_caption_edit_to_source(&mut project, 0, &durations);

        let source = &project.captions.as_ref().unwrap().segments[0];
        assert_eq!(source.text, "Step one: open settings");
        assert_eq!((source.start, source.end), (5.0, 8.0));
        assert!(source.words.is_empty());
        assert!(
            crate::transcript::model::flatten_words(project.captions.as_ref().unwrap()).is_empty()
        );
    }

    #[test]
    fn retyping_a_cleared_transcribed_caption_brings_its_words_back() {
        let mut project = ProjectConfiguration {
            timeline: Some(timeline(serde_json::json!({
                "segments": [{ "timescale": 1.0, "start": 0.0, "end": 10.0 }],
                "zoomSegments": []
            }))),
            captions: Some(CaptionsData {
                segments: vec![caption(
                    "segment-0",
                    1.0,
                    3.0,
                    &[("hello", 1.0, 2.0), ("world", 2.0, 3.0)],
                )],
                source_timed: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let durations = [10.0];
        frameever_project::synchronize_captions(&mut project, &durations);

        project.timeline.as_mut().unwrap().caption_segments[0].text = String::new();
        write_caption_edit_to_source(&mut project, 0, &durations);
        assert!(
            project.captions.as_ref().unwrap().segments[0]
                .words
                .is_empty()
        );

        project.timeline.as_mut().unwrap().caption_segments[0].text = "fresh take".into();
        write_caption_edit_to_source(&mut project, 0, &durations);

        let words: Vec<String> =
            crate::transcript::model::flatten_words(project.captions.as_ref().unwrap())
                .into_iter()
                .map(|word| word.text)
                .collect();
        assert_eq!(words, ["fresh", "take"]);
    }

    #[test]
    fn apply_caption_result_enables_captions_and_derives_the_track() {
        let mut project: ProjectConfiguration = serde_json::from_value(serde_json::json!({
            "timeline": {
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                ],
                "zoomSegments": []
            }
        }))
        .unwrap();
        project.captions = Some(CaptionsData {
            segments: Vec::new(),
            settings: CaptionSettings {
                enabled: false,
                size: 72,
                ..CaptionSettings::default()
            },
            source_timed: false,
        });

        apply_caption_result(
            &mut project,
            vec![caption("segment-0-0", 1.0, 2.0, &[("hello", 1.0, 2.0)])],
            &[10.0],
            10.0,
        );

        let captions = project.captions.as_ref().unwrap();
        assert!(captions.settings.enabled);
        // Existing style settings survive the regenerate.
        assert_eq!(captions.settings.size, 72);
        assert!(captions.source_timed);
        assert_eq!(captions.segments.len(), 1);

        let track = &project.timeline.as_ref().unwrap().caption_segments;
        assert_eq!(track.len(), 1);
        assert_eq!(track[0].id, "segment-0-0");
        assert_eq!(track[0].text, "hello");
    }

    #[test]
    fn apply_caption_result_synthesises_a_timeline_when_missing() {
        let mut project = ProjectConfiguration::default();

        apply_caption_result(
            &mut project,
            vec![caption("segment-0-0", 1.0, 2.0, &[("hello", 1.0, 2.0)])],
            &[10.0],
            10.0,
        );

        let timeline = project.timeline.as_ref().unwrap();
        assert_eq!(timeline.segments.len(), 1);
        assert!((timeline.segments[0].end - 10.0).abs() < 1e-6);
        assert_eq!(timeline.caption_segments.len(), 1);
    }

    #[test]
    fn generation_error_messages_match_the_web_mapping() {
        assert_eq!(
            caption_generation_error_message("No audio stream found in x"),
            "No audio found in the video file"
        );
        assert_eq!(
            caption_generation_error_message("Model file not found at path: /x"),
            "Caption model not found. Please download it first"
        );
        assert_eq!(caption_generation_error_message("boom"), "boom");
    }

    #[test]
    fn a_caption_moved_onto_a_held_tail_stays_on_its_clip() {
        let mut project = ProjectConfiguration {
            timeline: Some(timeline(serde_json::json!({
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 4.0, "end": 14.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                ],
                "zoomSegments": []
            }))),
            captions: Some(CaptionsData {
                segments: vec![caption("caption-1-a", 6.0, 7.0, &[])],
                source_timed: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let durations = [10.0, 10.0];
        let track = |project: &mut ProjectConfiguration| {
            frameever_project::synchronize_captions(project, &durations);
            project
                .timeline
                .as_ref()
                .unwrap()
                .caption_segments
                .iter()
                .map(|segment| (segment.start, segment.end))
                .collect::<Vec<_>>()
        };
        assert_eq!(track(&mut project), [(2.0, 3.0)]);
        let moved = |project: &mut ProjectConfiguration, start: f64, end: f64| {
            let item = &mut project.timeline.as_mut().unwrap().caption_segments[0];
            (item.start, item.end) = (start, end);
            write_caption_edit_to_source(project, 0, &durations);
            project.captions.as_ref().unwrap().segments[0].recording_clip
        };
        assert_eq!(moved(&mut project, 8.0, 9.0), Some(0));
        assert_eq!(track(&mut project), [(8.0, 9.0)]);
        assert_eq!(moved(&mut project, 1.0, 2.0), Some(0));
        assert_eq!(track(&mut project), [(1.0, 2.0)]);
    }

    #[test]
    fn retyping_a_caption_inside_a_crossfade_keeps_its_clip() {
        let mut tied = caption("caption-tail", 13.2, 13.8, &[]);
        tied.recording_clip = Some(0);
        tied.text = "typed".into();
        let mut project = ProjectConfiguration {
            timeline: Some(timeline(serde_json::json!({
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 4.0, "end": 14.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                ],
                "transitions": [{ "segmentIndex": 1, "type": "cross-fade", "duration": 1.0 }],
                "zoomSegments": []
            }))),
            captions: Some(CaptionsData {
                segments: vec![tied, caption("next", 10.2, 10.8, &[("next", 10.2, 10.8)])],
                source_timed: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let durations = [10.0, 10.0];
        let track = |project: &mut ProjectConfiguration| {
            frameever_project::synchronize_captions(project, &durations);
            project
                .timeline
                .as_ref()
                .unwrap()
                .caption_segments
                .iter()
                .map(|segment| {
                    let near = |time: f64| (time * 1000.0).round() / 1000.0;
                    (segment.text.clone(), near(segment.start), near(segment.end))
                })
                .collect::<Vec<_>>()
        };
        let mut before = track(&mut project);
        before.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            before,
            [
                ("next".to_string(), 9.2, 9.8),
                ("typed".to_string(), 9.2, 9.8)
            ]
        );
        let retype = |project: &mut ProjectConfiguration, id: &str, text: &str| {
            let timeline = project.timeline.as_mut().unwrap();
            let index = timeline
                .caption_segments
                .iter()
                .position(|item| source_caption_id(&item.id) == id)
                .unwrap();
            timeline.caption_segments[index].text = text.to_string();
            write_caption_edit_to_source(project, index, &durations);
            let source = project
                .captions
                .as_ref()
                .unwrap()
                .segments
                .iter()
                .find(|segment| segment.id == id)
                .unwrap();
            let near = |time: f32| (f64::from(time) * 1000.0).round() / 1000.0;
            (near(source.start), near(source.end), source.recording_clip)
        };
        assert_eq!(
            retype(&mut project, "caption-tail", "tail"),
            (13.2, 13.8, Some(0))
        );
        assert_eq!(retype(&mut project, "next", "after"), (10.2, 10.8, None));
        let mut after = track(&mut project);
        after.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            after,
            [
                ("after".to_string(), 9.2, 9.8),
                ("tail".to_string(), 9.2, 9.8)
            ]
        );
    }

    fn resize_caption(
        project: &mut ProjectConfiguration,
        index: usize,
        start: f64,
        end: f64,
        durations: &[f64],
    ) -> Vec<(String, f64, f64)> {
        let item = &mut project.timeline.as_mut().unwrap().caption_segments[index];
        (item.start, item.end) = (start, end);
        write_caption_edit_to_source(project, index, durations);
        frameever_project::synchronize_captions(project, durations);
        let near = |time: f64| (time * 1000.0).round() / 1000.0;
        project
            .timeline
            .as_ref()
            .unwrap()
            .caption_segments
            .iter()
            .map(|segment| (segment.text.clone(), near(segment.start), near(segment.end)))
            .collect()
    }

    fn held_tail_project(
        timeline_json: serde_json::Value,
        captions: Vec<CaptionSegment>,
    ) -> ProjectConfiguration {
        ProjectConfiguration {
            timeline: Some(timeline(timeline_json)),
            captions: Some(CaptionsData {
                segments: captions,
                source_timed: true,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn a_tied_caption_resized_past_its_clip_stops_at_the_edge() {
        let held = |clip: u32, start: f64, end: f64| serde_json::json!({ "recordingSegment": clip, "timescale": 1.0, "start": start, "end": end });
        let cases = [
            (
                vec![held(0, 4.0, 14.0), held(1, 0.0, 10.0)],
                0,
                (12.0, 13.0),
                (8.0, 11.0),
                (8.0, 10.0),
                (12.0, 14.0),
            ),
            (
                vec![held(0, 0.0, 10.0), held(1, 4.0, 14.0), held(2, 0.0, 10.0)],
                1,
                (22.0, 23.0),
                (9.0, 19.0),
                (10.0, 19.0),
                (14.0, 23.0),
            ),
            (
                vec![held(0, 4.0, 14.0), held(1, 4.0, 14.0), held(2, 0.0, 10.0)],
                0,
                (12.0, 13.0),
                (8.0, 21.0),
                (8.0, 10.0),
                (12.0, 14.0),
            ),
            (
                vec![held(0, 0.0, 10.0), held(1, 4.0, 14.0), held(2, 4.0, 14.0)],
                2,
                (32.0, 33.0),
                (9.0, 29.0),
                (20.0, 29.0),
                (24.0, 33.0),
            ),
        ];
        let durations = [10.0, 10.0, 10.0];
        for (segments, clip, (start, end), (drag_start, drag_end), shown, saved) in cases {
            let durations = &durations[..segments.len().max(2)];
            let mut tied = caption("caption-tail", start, end, &[]);
            tied.recording_clip = Some(clip);
            tied.text = "typed".into();
            let mut project = held_tail_project(
                serde_json::json!({ "segments": segments, "zoomSegments": [] }),
                vec![tied],
            );
            frameever_project::synchronize_captions(&mut project, durations);
            let track = resize_caption(&mut project, 0, drag_start, drag_end, durations);
            assert_eq!(track, [("typed".to_string(), shown.0, shown.1)], "{clip}");
            let sources: Vec<_> = project
                .captions
                .as_ref()
                .unwrap()
                .segments
                .iter()
                .map(|segment| (segment.start, segment.end, segment.recording_clip))
                .collect();
            assert_eq!(sources, [(saved.0, saved.1, Some(clip))], "{clip}");
        }
    }

    #[test]
    fn a_tied_transcribed_caption_resized_into_the_next_clip_keeps_its_words() {
        let durations = [10.0, 10.0];
        let mut tied = caption("tail", 12.0, 13.0, &[("held", 12.2, 12.8)]);
        tied.recording_clip = Some(0);
        let mut project = held_tail_project(
            serde_json::json!({
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 4.0, "end": 14.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                ],
                "zoomSegments": []
            }),
            vec![tied],
        );
        frameever_project::synchronize_captions(&mut project, &durations);
        let track = resize_caption(&mut project, 0, 8.0, 11.0, &durations);
        assert_eq!(track, [("held".to_string(), 8.2, 8.8)]);
        let source = &project.captions.as_ref().unwrap().segments;
        assert_eq!(source.len(), 1);
        assert_eq!(source[0].recording_clip, Some(0));
    }

    #[test]
    fn an_untied_caption_resized_across_clips_is_unchanged() {
        let durations = [10.0, 10.0];
        let mut project = held_tail_project(
            serde_json::json!({
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 2.0, "end": 8.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                ],
                "zoomSegments": []
            }),
            vec![
                caption("caption-typed", 6.0, 7.0, &[]),
                caption("spoken", 13.0, 14.0, &[("spoken", 13.0, 14.0)]),
            ],
        );
        project.captions.as_mut().unwrap().segments[0].text = "typed".into();
        frameever_project::synchronize_captions(&mut project, &durations);
        assert_eq!(
            resize_caption(&mut project, 0, 4.0, 7.0, &durations),
            [
                ("typed".to_string(), 4.0, 6.0),
                ("typed".to_string(), 6.0, 7.0),
                ("spoken".to_string(), 9.0, 10.0)
            ]
        );
        assert_eq!(
            resize_caption(&mut project, 2, 5.0, 10.0, &durations),
            [
                ("typed".to_string(), 4.0, 6.0),
                ("typed".to_string(), 6.0, 7.0),
                ("spoken".to_string(), 9.0, 10.0)
            ]
        );
        let sources = serde_json::to_value(&project.captions.as_ref().unwrap().segments).unwrap();
        assert_eq!(
            sources,
            serde_json::json!([
                { "id": "caption-typed", "start": 6.0, "end": 11.0, "text": "typed", "words": [] },
                { "id": "spoken", "start": 7.0, "end": 14.0, "text": "spoken",
                  "words": [{ "text": "spoken", "start": 13.0, "end": 14.0 }] }
            ])
        );
    }

    #[test]
    fn a_caption_dragged_out_of_a_held_tail_is_not_saved_reversed() {
        for (tie, start, end, saved, shown) in [
            (None, 8.0, 9.0, (13.5, 14.0, Some(0)), (9.5, 10.0)),
            (Some(2), 25.0, 26.0, (25.0, 26.0, Some(2)), (25.0, 26.0)),
        ] {
            let mut moved = caption("caption-moved", start, end, &[]);
            moved.recording_clip = tie;
            moved.text = "typed".into();
            let mut project = ProjectConfiguration {
                timeline: Some(timeline(serde_json::json!({
                    "segments": [
                        { "recordingSegment": 0, "timescale": 1.0, "start": 4.0, "end": 14.0 },
                        { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 },
                        { "recordingSegment": 2, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                    ],
                    "zoomSegments": []
                }))),
                captions: Some(CaptionsData {
                    segments: vec![moved],
                    source_timed: true,
                    ..Default::default()
                }),
                ..Default::default()
            };
            let durations = [10.0, 10.0, 10.0];
            let track = |project: &mut ProjectConfiguration| {
                frameever_project::synchronize_captions(project, &durations);
                project
                    .timeline
                    .as_ref()
                    .unwrap()
                    .caption_segments
                    .iter()
                    .map(|segment| (segment.start, segment.end))
                    .collect::<Vec<_>>()
            };
            assert_eq!(track(&mut project).len(), 1, "{tie:?}");
            let item = &mut project.timeline.as_mut().unwrap().caption_segments[0];
            (item.start, item.end) = (9.5, 10.5);
            write_caption_edit_to_source(&mut project, 0, &durations);
            let saved_now = |project: &ProjectConfiguration| -> Vec<(f32, f32, Option<u32>)> {
                project
                    .captions
                    .as_ref()
                    .unwrap()
                    .segments
                    .iter()
                    .map(|segment| (segment.start, segment.end, segment.recording_clip))
                    .collect()
            };
            assert_eq!(saved_now(&project), [saved], "{tie:?}");
            assert_eq!(track(&mut project), [shown], "{tie:?}");
        }
    }

    #[test]
    fn a_tied_caption_resized_over_a_repeated_clip_stays_inside_the_drag() {
        let durations = [10.0, 10.0];
        let mut tied = caption("caption-tail", 12.0, 13.0, &[]);
        tied.recording_clip = Some(0);
        tied.text = "typed".into();
        let mut project = held_tail_project(
            serde_json::json!({
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 4.0, "end": 14.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                ],
                "zoomSegments": []
            }),
            vec![tied],
        );
        frameever_project::synchronize_captions(&mut project, &durations);
        let track = resize_caption(&mut project, 0, 8.0, 22.0, &durations);
        assert_eq!(track, [("typed".to_string(), 8.0, 10.0)]);
        let sources: Vec<_> = project
            .captions
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .map(|segment| (segment.start, segment.end, segment.recording_clip))
            .collect();
        assert_eq!(sources, [(12.0, 14.0, Some(0))]);
    }

    #[test]
    fn a_tied_caption_resized_across_its_own_split_stops_at_the_split() {
        let durations = [10.0];
        let mut tied = caption("caption-split", 1.0, 3.0, &[]);
        tied.recording_clip = Some(0);
        tied.text = "typed".into();
        let mut project = held_tail_project(
            serde_json::json!({
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 3.7 },
                    { "recordingSegment": 0, "timescale": 1.0, "start": 3.7, "end": 10.0 }
                ],
                "zoomSegments": []
            }),
            vec![tied],
        );
        frameever_project::synchronize_captions(&mut project, &durations);
        assert_eq!(
            tied_caption_window(&project, 0, &durations),
            Some((0.0, 3.7))
        );
        let track = resize_caption(&mut project, 0, 1.0, 6.0, &durations);
        assert_eq!(track, [("typed".to_string(), 1.0, 3.7)]);
        let sources: Vec<_> = project
            .captions
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .map(|segment| {
                (
                    segment.id.as_str(),
                    segment.start,
                    segment.end,
                    segment.recording_clip,
                )
            })
            .collect();
        assert_eq!(
            sources,
            [("caption-split", 1.0, 3.7_f32.next_down(), Some(0))]
        );
    }

    #[test]
    fn a_tied_transcribed_caption_resized_across_its_split_keeps_its_words_on_its_clip() {
        let durations = [10.0, 10.0];
        let mut tied = caption("spoken", 9.0, 11.0, &[("held", 9.0, 11.0)]);
        tied.recording_clip = Some(0);
        let mut project = held_tail_project(
            serde_json::json!({
                "segments": [
                    { "recordingSegment": 0, "timescale": 1.0, "start": 4.0, "end": 10.0 },
                    { "recordingSegment": 0, "timescale": 1.0, "start": 10.0, "end": 14.0 },
                    { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 10.0 }
                ],
                "zoomSegments": []
            }),
            vec![tied],
        );
        frameever_project::synchronize_captions(&mut project, &durations);
        let shown = [
            ("held".to_string(), 5.0, 6.0),
            ("held".to_string(), 6.0, 7.0),
        ];
        let near = |time: f64| (time * 1000.0).round() / 1000.0;
        let track: Vec<_> = project
            .timeline
            .as_ref()
            .unwrap()
            .caption_segments
            .iter()
            .map(|segment| (segment.text.clone(), near(segment.start), near(segment.end)))
            .collect();
        assert_eq!(track, shown);
        assert_eq!(resize_caption(&mut project, 0, 5.0, 6.5, &durations), shown);
        let source = &project.captions.as_ref().unwrap().segments;
        assert_eq!(source.len(), 1);
        assert_eq!(
            (source[0].start, source[0].end, source[0].recording_clip),
            (9.0, 10.0, Some(0))
        );
    }

    struct Rng(u64);

    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn unit(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }

        fn between(&mut self, low: f64, high: f64) -> f64 {
            low + (high - low) * self.unit()
        }

        fn below(&mut self, count: usize) -> usize {
            (self.next_u64() % count as u64) as usize
        }

        fn chance(&mut self, probability: f64) -> bool {
            self.unit() < probability
        }
    }

    struct OraclePiece {
        recording: usize,
        source_start: f64,
        source_end: f64,
        output_start: f64,
        output_end: f64,
        timescale: f64,
    }

    impl OraclePiece {
        fn source_at(&self, output: f64) -> f64 {
            self.source_start + (output - self.output_start) * self.timescale
        }
    }

    fn oracle_pieces(timeline: &TimelineConfiguration, durations: &[f64]) -> Vec<OraclePiece> {
        let mut output = 0.0;
        timeline
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                output -= timeline
                    .effective_transition(index)
                    .map_or(0.0, |transition| transition.duration);
                let recording = segment.recording_clip as usize;
                let offset: f64 = durations[..recording].iter().sum();
                let piece = OraclePiece {
                    recording,
                    source_start: offset + segment.start,
                    source_end: offset + segment.end,
                    output_start: output,
                    output_end: output + segment.duration(),
                    timescale: segment.timescale,
                };
                output = piece.output_end;
                piece
            })
            .collect()
    }

    #[derive(Clone, Copy, Debug)]
    enum CaptionDrag {
        Start,
        End,
        Move,
    }

    struct DragCase {
        durations: Vec<f64>,
        project: ProjectConfiguration,
        id: String,
        tie: u32,
        typed: bool,
        index: usize,
        before: (f64, f64),
        dragged: (f64, f64),
        drag: Option<CaptionDrag>,
    }

    fn random_drag_case(rng: &mut Rng) -> Option<DragCase> {
        let grid = rng.below(3);
        let snap = |time: f64| match grid {
            0 => (time * 8.0).round() / 8.0,
            1 => (time * 10.0).round() / 10.0,
            _ => time,
        };
        let recordings = 1 + rng.below(4);
        let durations: Vec<f64> = (0..recordings)
            .map(|_| snap(rng.between(2.0, 12.0)))
            .collect();
        let mut segments: Vec<(usize, f64, f64, f64)> = Vec::new();
        for _ in 0..1 + rng.below(5) {
            if !segments.is_empty() && rng.chance(0.25) {
                let copy = segments[rng.below(segments.len())];
                segments.push(copy);
                continue;
            }
            let recording = rng.below(recordings);
            let duration = durations[recording];
            let start = snap(rng.between(0.0, duration - 1.0));
            let end = if rng.chance(0.4) {
                snap(duration + rng.between(0.5, 6.0))
            } else {
                snap(rng.between(start + 0.5, duration))
            };
            if end <= start {
                return None;
            }
            let timescale = if rng.chance(0.15) {
                [0.5, 2.0][rng.below(2)]
            } else {
                1.0
            };
            segments.push((recording, start, end, timescale));
        }
        if rng.chance(0.5) {
            segments.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        }
        if rng.chance(0.4) {
            let index = rng.below(segments.len());
            let (recording, start, end, timescale) = segments[index];
            let split = snap(rng.between(start + 0.1, end - 0.1));
            if start < split && split < end {
                segments[index].2 = split;
                segments.insert(index + 1, (recording, split, end, timescale));
            }
        }
        let mut transitions = Vec::new();
        for index in 1..segments.len() {
            if rng.chance(0.25) {
                transitions.push(serde_json::json!({
                    "segmentIndex": index,
                    "type": "cross-fade",
                    "duration": snap(rng.between(0.25, 1.5))
                }));
            }
        }

        let (recording, piece_start, piece_end, _) = segments[rng.below(segments.len())];
        let offset: f64 = durations[..recording].iter().sum();
        let start = snap(rng.between(piece_start, piece_end - 0.25));
        let end = snap((start + rng.between(0.2, 2.0)).min(piece_end));
        if end <= start {
            return None;
        }
        let (start, end) = ((offset + start) as f32, (offset + end) as f32);
        let typed = rng.chance(0.7);
        let mut tied = if typed {
            let mut segment = caption("caption-tied", start, end, &[]);
            segment.text = "typed".into();
            segment
        } else {
            let count = 1 + rng.below(3);
            let step = (end - start) / count as f32;
            let words: Vec<_> = (0..count)
                .map(|index| {
                    let word_start = start + step * index as f32;
                    ("spoken", word_start + step * 0.1, word_start + step * 0.9)
                })
                .collect();
            caption("spoken", start, end, &words)
        };
        tied.recording_clip = Some(recording as u32);
        let id = tied.id.clone();

        let mut project = held_tail_project(
            serde_json::json!({
                "segments": segments
                    .iter()
                    .map(|(recording, start, end, timescale)| serde_json::json!({
                        "recordingSegment": recording,
                        "timescale": timescale,
                        "start": start,
                        "end": end
                    }))
                    .collect::<Vec<_>>(),
                "transitions": transitions,
                "zoomSegments": []
            }),
            vec![tied],
        );
        frameever_project::synchronize_captions(&mut project, &durations);
        let timeline = project.timeline.as_ref()?;
        let shown: Vec<usize> = timeline
            .caption_segments
            .iter()
            .enumerate()
            .filter(|(_, item)| source_caption_id(&item.id) == id && item.end - item.start > 0.01)
            .map(|(index, _)| index)
            .collect();
        if shown.is_empty() {
            return None;
        }
        let index = shown[rng.below(shown.len())];
        let item = &timeline.caption_segments[index];
        let before = (item.start, item.end);
        let total = timeline.duration();
        let (drag, dragged) = match rng.below(4) {
            0 => (
                CaptionDrag::Start,
                (snap(rng.between(0.0, before.1 - 0.05)), before.1),
            ),
            1 => (
                CaptionDrag::End,
                (before.0, snap(rng.between(before.0 + 0.05, total))),
            ),
            2 => {
                let shift = snap(rng.between(-before.0, total - before.1));
                (CaptionDrag::Move, (before.0 + shift, before.1 + shift))
            }
            _ => {
                let start = snap(rng.between(0.0, total - 0.1));
                (
                    CaptionDrag::Move,
                    (start, snap(rng.between(start + 0.05, total))),
                )
            }
        };
        if dragged.0 >= dragged.1 {
            return None;
        }
        Some(DragCase {
            durations,
            project,
            id,
            tie: recording as u32,
            typed,
            index,
            before,
            dragged,
            drag: rng.chance(0.75).then_some(drag),
        })
    }

    fn dragged_item(case: &DragCase, project: &ProjectConfiguration) -> (f64, f64) {
        let (Some(drag), Some(window)) = (
            case.drag,
            tied_caption_window(project, case.index, &case.durations),
        ) else {
            return case.dragged;
        };
        let free = crate::editor_edits::DragBounds {
            min: -1e9,
            max: 1e9,
        };
        let (before, dragged) = (case.before, case.dragged);
        match drag {
            CaptionDrag::Start => (
                crate::editor_edits::window_trim_start_bounds(window, free).clamp(dragged.0),
                before.1,
            ),
            CaptionDrag::End => (
                before.0,
                crate::editor_edits::window_trim_end_bounds(window, free).clamp(dragged.1),
            ),
            CaptionDrag::Move => {
                let shift = crate::editor_edits::window_move_bounds(window, before, free)
                    .clamp(dragged.0 - before.0);
                (before.0 + shift, before.1 + shift)
            }
        }
    }

    fn check_tied_drag(case: &DragCase) -> Result<(), String> {
        const TOLERANCE: f64 = 2e-3;
        let durations = &case.durations[..];
        let mut project = case.project.clone();
        let timeline = project.timeline.clone().unwrap();
        let pieces = oracle_pieces(&timeline, durations);
        let source = project.captions.as_ref().unwrap().segments[0].clone();
        let own: Vec<&OraclePiece> = pieces
            .iter()
            .filter(|piece| piece.recording == case.tie as usize)
            .collect();
        let near = |a: f64, b: f64| (a - b).abs() <= TOLERANCE;
        let spans: Vec<(f64, f64)> = if source.words.is_empty() {
            vec![(f64::from(source.start), f64::from(source.end))]
        } else {
            source
                .words
                .iter()
                .map(|word| {
                    let end = word.end.min(word.start + MAX_CAPTION_WORD_DURATION);
                    (f64::from(word.start), f64::from(end))
                })
                .collect()
        };
        let (before, dragged) = (case.before, case.dragged);
        let drawn: Vec<&OraclePiece> = own
            .iter()
            .copied()
            .filter(|piece| {
                let output =
                    |time: f64| piece.output_start + (time - piece.source_start) / piece.timescale;
                let shown: Vec<(f64, f64)> = spans
                    .iter()
                    .map(|&(start, end)| (start.max(piece.source_start), end.min(piece.source_end)))
                    .filter(|(start, end)| start < end)
                    .collect();
                match (shown.first(), shown.last()) {
                    (Some(first), Some(last)) => {
                        near(output(first.0), before.0) && near(output(last.1), before.1)
                    }
                    _ => false,
                }
            })
            .collect();
        let clamp_to =
            |piece: &OraclePiece, time: f64| time.clamp(piece.output_start, piece.output_end);
        let expect = |piece: &OraclePiece| {
            Some(match case.drag {
                Some(CaptionDrag::Start) => (clamp_to(piece, dragged.0), before.1),
                Some(CaptionDrag::End) => (before.0, clamp_to(piece, dragged.1)),
                Some(CaptionDrag::Move) => {
                    let shift = (dragged.0 - before.0)
                        .max(piece.output_start - before.0)
                        .min(piece.output_end - before.1);
                    (before.0 + shift, before.1 + shift)
                }
                None => (clamp_to(piece, dragged.0), clamp_to(piece, dragged.1)),
            })
            .filter(|(start, end)| end - start > MIN_TIED_CAPTION_SECS + 1e-6)
        };

        let item = dragged_item(case, &project);
        let edited = &mut project.timeline.as_mut().unwrap().caption_segments[case.index];
        (edited.start, edited.end) = item;
        write_caption_edit_to_source(&mut project, case.index, durations);
        frameever_project::synchronize_captions(&mut project, durations);

        let part_prefix = format!("{}-", case.id);
        let saved: Vec<&CaptionSegment> = project
            .captions
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .filter(|segment| segment.id == case.id || segment.id.starts_with(&part_prefix))
            .collect();
        let [caption] = saved[..] else {
            return Err(format!("1: saved as {} captions", saved.len()));
        };
        if caption.recording_clip != Some(case.tie) {
            return Err(format!(
                "1: tie {:?}, was {}",
                caption.recording_clip, case.tie
            ));
        }
        if caption.start >= caption.end || caption.start.is_nan() || caption.end.is_nan() {
            return Err(format!("1: saved reversed {caption:?}"));
        }

        let (saved_start, saved_end) = (f64::from(caption.start), f64::from(caption.end));
        let unchanged = (caption.start, caption.end) == (source.start, source.end);
        let matches = |piece: &OraclePiece| match expect(piece) {
            Some((start, end)) => {
                near(saved_start, piece.source_at(start)) && near(saved_end, piece.source_at(end))
            }
            None => unchanged,
        };
        let expected = match drawn.iter().copied().find(|piece| matches(piece)) {
            Some(piece) => expect(piece),
            None if drawn.is_empty() && unchanged => None,
            None => {
                let wanted: Vec<_> = drawn.iter().map(|piece| expect(piece)).collect();
                return Err(format!(
                    "2: saved {saved_start:.4}..{saved_end:.4}, drag clamped to {wanted:?}"
                ));
            }
        };

        let shown: Vec<(f64, f64)> = project
            .timeline
            .as_ref()
            .unwrap()
            .caption_segments
            .iter()
            .filter(|item| source_caption_id(&item.id) == case.id)
            .map(|item| (item.start, item.end))
            .collect();
        for &(start, end) in &shown {
            let on_own = own.iter().any(|piece| {
                piece.output_start - TOLERANCE <= start && end <= piece.output_end + TOLERANCE
            });
            if !on_own {
                return Err(format!(
                    "3: shown at {start:.4}..{end:.4} off its recording"
                ));
            }
        }
        if case.typed
            && let Some((start, end)) = expected
            && !shown
                .iter()
                .any(|&(shown_start, shown_end)| near(shown_start, start) && near(shown_end, end))
        {
            return Err(format!(
                "3: not shown at the clamped drag {start:.4}..{end:.4}: {shown:?}"
            ));
        }

        let word_recordings = |word: &CaptionWord, tie: Option<u32>| {
            let (start, end) = (f64::from(word.start), f64::from(word.end));
            pieces
                .iter()
                .filter(|piece| {
                    let reach_end = match tie {
                        Some(tie) if tie as usize == piece.recording => piece.source_end,
                        Some(_) => return false,
                        None if piece.recording + 1 < durations.len() => {
                            let media_end: f64 = durations[..=piece.recording].iter().sum();
                            piece.source_end.min(media_end)
                        }
                        None => piece.source_end,
                    };
                    start.max(piece.source_start) < end.min(reach_end) - 1e-6
                })
                .map(|piece| piece.recording)
                .collect::<std::collections::BTreeSet<_>>()
        };
        for word in &caption.words {
            let Some(before) = source.words.iter().find(|before| {
                (before.text.as_str(), before.start, before.end)
                    == (word.text.as_str(), word.start, word.end)
            }) else {
                continue;
            };
            let (was, now) = (
                word_recordings(before, source.recording_clip),
                word_recordings(word, caption.recording_clip),
            );
            if !was.is_empty() && was != now {
                return Err(format!("4: word {word:?} moved from {was:?} to {now:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn tied_caption_drags_stay_on_their_clip_piece() {
        let hex = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| u64::from_str_radix(value.trim_start_matches("0x"), 16).unwrap())
        };
        let cases = hex("CAP_TIED_DRAG_CASES").map_or(20_000, |cases| cases as usize);
        let mut rng = Rng(hex("CAP_TIED_DRAG_BASE").unwrap_or(0x4308));
        let (mut checked, mut failures) = (0, Vec::new());
        while checked < cases {
            let seed = rng.next_u64();
            let Some(case) = random_drag_case(&mut Rng(seed)) else {
                continue;
            };
            checked += 1;
            if let Err(failure) = check_tied_drag(&case) {
                failures.push(format!("seed {seed:#x}: {failure}"));
            }
        }
        let mut kinds = std::collections::BTreeMap::<String, usize>::new();
        for failure in &failures {
            let kind = failure.split(": ").nth(1).unwrap_or_default();
            *kinds.entry(kind.to_string()).or_default() += 1;
        }
        println!(
            "tied drag property: {checked} cases, {} failures {kinds:?}",
            failures.len()
        );
        if let Some(seed) = hex("CAP_TIED_DRAG_SEED") {
            let case = random_drag_case(&mut Rng(seed)).unwrap();
            let timeline = case.project.timeline.as_ref().unwrap();
            println!(
                "durations {:?}\nsegments {}\ntransitions {}\ncaptions {}\nitem {} {:?} drag {:?} to {:?}\n{:?}",
                case.durations,
                serde_json::to_string(&timeline.segments).unwrap(),
                serde_json::to_string(&timeline.transitions).unwrap(),
                serde_json::to_string(&case.project.captions.as_ref().unwrap().segments).unwrap(),
                case.index,
                case.before,
                case.drag,
                case.dragged,
                check_tied_drag(&case)
            );
        }
        assert!(
            failures.is_empty(),
            "{} of {checked} cases fail:\n{}",
            failures.len(),
            kinds
                .keys()
                .flat_map(|kind| failures
                    .iter()
                    .filter(|f| f.contains(kind.as_str()))
                    .take(3))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
