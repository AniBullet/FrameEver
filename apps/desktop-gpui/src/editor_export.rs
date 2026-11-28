//! Editor export page -- `routes/editor/ExportPage.tsx`, 1:1 layout.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use frameever_export::audio::AudioExportFormat;
use frameever_export::estimates::{ExportEstimates, estimate_export};
use frameever_export::gif::{GifExportSettings, GifQuality, GifQualityPreset};
use frameever_export::mov::MovExportSettings;
use frameever_export::mp4::{ExportCodec, ExportCompression, Mp4ExportSettings};
use frameever_export::preview::{
    ExportPreviewSettings, render_preview_with_config, render_preview_with_editor,
};
use frameever_export::settings::ExportSettings;
use frameever_export::tracks::{self, TrackKind as ExportTrack, TrackProgress};
use frameever_export::{ExportRange, ExporterBase, make_cursor_only_project};
use frameever_project::{BackgroundSource, XY};
use frameever_utils::export_resources::{
    DiskBudget, ExportResources, estimated_working_bytes, is_resource_stop,
};
use gpui::{
    Context, FontWeight, Hsla, InteractiveElement, IntoElement, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, RenderImage, SharedString, StatefulInteractiveElement, Styled,
    Window, div, img, prelude::FluentBuilder, px, svg,
};

use crate::editor_edits::Selection;
use crate::editor_timeline::{Segment, SegmentDetail, TrackKind};
use crate::editor_window::EditorWindow;
use crate::i18n::{n, t, tc, tr};
use crate::store::{self, ExportPrefs};
use crate::ui;
use crate::{library, platform};

const SIDEBAR_WIDTH: f32 = 400.;
const HEADER_HEIGHT: f32 = 44.;
const STAGE_PADDING_X: f32 = 24.;
/// Stage top padding, caption row, preview padding, stats card and bottom
/// padding: everything on the stage that is not the preview itself.
const STAGE_VERTICAL_CHROME: f32 = 16. + 22. + 14. + 18. + STATS_HEIGHT + 20.;
const STATS_HEIGHT: f32 = 44.;
const SEGMENT_HEIGHT: f32 = 26.;
const RANGE_NAME_LENGTH: usize = 15;
const COMPRESSION_PRESETS: [(ExportCompression, &str); 4] = [
    (ExportCompression::Potato, n!("Potato")),
    (ExportCompression::Web, n!("Web")),
    (ExportCompression::Social, n!("Social")),
    (ExportCompression::Maximum, n!("Maximum")),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDestination {
    File,
    Clipboard,
}

impl ExportDestination {
    const ALL: &'static [Self] = &[Self::File, Self::Clipboard];

    fn label(self) -> &'static str {
        match self {
            Self::File => t!("File"),
            Self::Clipboard => t!("Clipboard"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::File => "icons/folder.svg",
            Self::Clipboard => "icons/copy.svg",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Clipboard => "clipboard",
        }
    }

    fn from_slug(slug: &str) -> Self {
        match slug {
            "clipboard" => Self::Clipboard,
            _ => Self::File,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormatKind {
    Mp4,
    Gif,
    Tracks,
    Audio,
}

impl ExportFormatKind {
    const ALL: [Self; 4] = [Self::Mp4, Self::Gif, Self::Tracks, Self::Audio];

    fn label(self) -> &'static str {
        match self {
            Self::Mp4 => "MP4",
            Self::Gif => "GIF",
            Self::Tracks => tc!("export format", "Tracks"),
            Self::Audio => t!("Audio"),
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Mp4 => "Mp4",
            Self::Gif => "Gif",
            Self::Tracks => "Tracks",
            Self::Audio => "Audio",
        }
    }

    fn from_slug(slug: &str) -> Self {
        match slug {
            "Gif" => Self::Gif,
            "Tracks" => Self::Tracks,
            "Audio" => Self::Audio,
            _ => Self::Mp4,
        }
    }

    fn renders_video(self) -> bool {
        matches!(self, Self::Mp4 | Self::Gif)
    }
}

enum FileJob {
    Tracks {
        selection: Vec<ExportTrack>,
        fps: u32,
        force_ffmpeg_decoder: bool,
    },
    Audio(AudioExportFormat),
}

impl FileJob {
    fn record_format(&self) -> &'static str {
        match self {
            Self::Tracks { .. } => library::TRACKS_EXPORT_FORMAT,
            Self::Audio(format) => format.extension(),
        }
    }
}

fn audio_format_from_slug(slug: Option<&str>) -> AudioExportFormat {
    slug.and_then(AudioExportFormat::from_extension)
        .unwrap_or(AudioExportFormat::Mp3)
}

fn audio_format_label(format: AudioExportFormat) -> &'static str {
    match format {
        AudioExportFormat::Mp3 => "MP3",
        AudioExportFormat::Wav => "WAV",
    }
}

fn audio_format_note(format: AudioExportFormat) -> &'static str {
    match format {
        AudioExportFormat::Mp3 => t!("Small and plays everywhere. 192 kb/s stereo."),
        AudioExportFormat::Wav => t!("Lossless 16-bit, 48 kHz. Best for more editing."),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackOption {
    pub kind: ExportTrack,
    pub size: Option<(u32, u32)>,
}

fn track_options(
    recordings: &frameever_rendering::ProjectRecordingsMeta,
    project: &frameever_project::ProjectConfiguration,
) -> Vec<TrackOption> {
    let used = |index: usize| {
        project.timeline.as_ref().is_none_or(|timeline| {
            timeline
                .segments
                .iter()
                .any(|segment| segment.recording_clip as usize == index)
        })
    };
    tracks::available_tracks(recordings, project)
        .into_iter()
        .map(|kind| {
            let size = recordings
                .segments
                .iter()
                .enumerate()
                .filter(|(index, _)| used(*index))
                .find_map(|(_, segment)| match kind {
                    ExportTrack::Screen => Some((segment.display.width, segment.display.height)),
                    ExportTrack::Camera => segment
                        .camera
                        .as_ref()
                        .map(|camera| (camera.width, camera.height)),
                    ExportTrack::Microphone | ExportTrack::SystemAudio => None,
                });
            TrackOption { kind, size }
        })
        .collect()
}

fn track_label(kind: ExportTrack) -> &'static str {
    match kind {
        ExportTrack::Screen => t!("Screen"),
        ExportTrack::Camera => t!("Camera"),
        ExportTrack::Microphone => t!("Microphone"),
        ExportTrack::SystemAudio => t!("System audio"),
    }
}

fn track_icon(kind: ExportTrack) -> &'static str {
    match kind {
        ExportTrack::Screen => "icons/monitor.svg",
        ExportTrack::Camera => "icons/webcam.svg",
        ExportTrack::Microphone => "icons/mic.svg",
        ExportTrack::SystemAudio => "icons/volume-2.svg",
    }
}

fn track_toggle_id(kind: ExportTrack) -> &'static str {
    match kind {
        ExportTrack::Screen => "export-track-screen",
        ExportTrack::Camera => "export-track-camera",
        ExportTrack::Microphone => "export-track-microphone",
        ExportTrack::SystemAudio => "export-track-system-audio",
    }
}

fn track_detail(option: &TrackOption) -> String {
    match option.size {
        Some((width, height)) => format!("MP4 · {width}×{height}"),
        None => "WAV · 48 kHz".into(),
    }
}

fn selected_tracks(options: &[TrackOption], excluded: &[ExportTrack]) -> Vec<ExportTrack> {
    let selected = options
        .iter()
        .map(|option| option.kind)
        .filter(|kind| !excluded.contains(kind))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        options.iter().map(|option| option.kind).collect()
    } else {
        selected
    }
}

fn estimated_tracks_bytes(
    options: &[TrackOption],
    selected: &[ExportTrack],
    fps: u32,
    duration: f64,
) -> u64 {
    let bpp = ExportCompression::Maximum.bits_per_pixel() as f64;
    options
        .iter()
        .filter(|option| selected.contains(&option.kind))
        .map(|option| match option.size {
            Some((width, height)) => {
                width as f64 * height as f64 * fps as f64 * bpp / 8.0 * duration
            }
            None => 48_000.0 * 2.0 * 3.0 * duration,
        })
        .sum::<f64>()
        .max(0.0) as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExportResolution {
    P480,
    P720,
    P1080,
    P4k,
}

impl ExportResolution {
    fn label(self) -> &'static str {
        match self {
            Self::P480 => "480p",
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P4k => "4K",
        }
    }

    fn size(self) -> (u32, u32) {
        match self {
            Self::P480 => (854, 480),
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::P4k => (3840, 2160),
        }
    }

    fn from_slug(slug: &str) -> Self {
        match slug {
            "480p" => Self::P480,
            "1080p" => Self::P1080,
            "4K" | "4k" => Self::P4k,
            _ => Self::P720,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPhase {
    Idle,
    ChoosingFile,
    Starting,
    Rendering,
    Copying,
    Done,
    Failed,
}

impl ExportPhase {
    pub(crate) fn is_busy(self) -> bool {
        matches!(
            self,
            Self::ChoosingFile | Self::Starting | Self::Rendering | Self::Copying
        )
    }

    fn shows_progress(self) -> bool {
        !matches!(self, Self::Idle | Self::ChoosingFile)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExportClip {
    pub name: SharedString,
    pub range: ExportRange,
}

impl ExportClip {
    fn from_segment(segment: &Segment) -> Option<Self> {
        let SegmentDetail::Clip { name, .. } = &segment.detail else {
            return None;
        };
        Some(Self {
            name: name.clone(),
            range: ExportRange {
                start: segment.start,
                end: segment.end,
            },
        })
    }

    fn label(&self) -> String {
        let name = if self.name.chars().count() > RANGE_NAME_LENGTH {
            let shortened: String = self.name.chars().take(RANGE_NAME_LENGTH).collect();
            format!("{shortened}…")
        } else {
            self.name.to_string()
        };
        format!(
            "{name} · {}",
            format_duration(self.range.end - self.range.start)
        )
    }
}

pub(crate) fn selected_clip(selection: Option<&Selection>) -> Option<usize> {
    match selection {
        Some(Selection {
            track: TrackKind::Clip,
            indices,
        }) if indices.len() == 1 => Some(indices[0]),
        _ => None,
    }
}

fn preview_time_in_range(time: f64, range: Option<ExportRange>) -> f64 {
    match range {
        Some(range) if !(range.start..range.end).contains(&time) => range.start,
        _ => time,
    }
}

pub struct PreviewStats {
    pub width: u32,
    pub height: u32,
    pub total_frames: u32,
    pub estimated_size_mb: f64,
}

pub struct ExportUi {
    pub destination: ExportDestination,
    pub format: ExportFormatKind,
    pub resolution: ExportResolution,
    pub fps: u32,
    pub compression: ExportCompression,
    pub codec: ExportCodec,
    pub gif_quality: GifQualityPreset,
    pub optimize_filesize: bool,
    pub cursor_only: bool,
    pub custom_bpp: Option<f32>,
    pub force_ffmpeg: bool,
    pub advanced_open: bool,
    clip: Option<ExportClip>,
    clip_only: bool,
    pub preview: Option<Arc<RenderImage>>,
    pub preview_stats: Option<PreviewStats>,
    pub preview_error: Option<String>,
    pub preview_task: Option<gpui::Task<()>>,
    preview_request: Arc<()>,
    estimate: Option<ExportEstimates>,
    estimate_loading: bool,
    estimate_cancel: Arc<AtomicBool>,
    pub phase: ExportPhase,
    details_open: bool,
    finished_at: Option<std::time::Instant>,
    close_requested: bool,
    pub rendered: u32,
    pub total_frames: u32,
    pub output_path: Option<PathBuf>,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub export_task: Option<gpui::Task<()>>,
    bpp_track: ui::SliderTrack,
    bpp_dragging: bool,
    resolution_from_source: bool,
    fps_from_source: bool,
    source: ExportSource,
    pub track_options: Vec<TrackOption>,
    pub tracks: Vec<ExportTrack>,
    excluded_tracks: Vec<ExportTrack>,
    pub audio_format: AudioExportFormat,
    picture: ExportPicture,
    sound: bool,
    saved_format: ExportFormatKind,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ExportSource {
    pub size: Option<XY<u32>>,
    pub fps: Option<u32>,
}

impl Drop for ExportUi {
    fn drop(&mut self) {
        self.estimate_cancel.store(true, Ordering::Release);
    }
}

const BPP_MIN: f32 = 0.02;
const BPP_MAX: f32 = 0.5;
const BPP_STEP: f32 = 0.01;

impl ExportUi {
    pub fn load() -> Self {
        Self::from_preferences(store::load().export.unwrap_or_else(first_export_prefs))
    }

    pub(crate) fn video_settings(&self) -> VideoSettings {
        VideoSettings {
            format: self.format,
            cursor_only: self.cursor_only,
            fps: self.fps,
            size: self.resolution.size(),
            compression: self.compression,
            codec: self.codec,
            custom_bpp: self.custom_bpp,
            gif_quality: self.gif_quality,
            optimize: self.optimize_filesize,
            force_ffmpeg: self.force_ffmpeg,
        }
    }

    fn from_preferences(prefs: ExportPrefs) -> Self {
        Self {
            destination: ExportDestination::from_slug(&prefs.export_to),
            format: ExportFormatKind::from_slug(&prefs.format),
            resolution: ExportResolution::from_slug(&prefs.resolution),
            fps: prefs.fps,
            compression: match prefs.compression.as_str() {
                "Social" => ExportCompression::Social,
                "Web" => ExportCompression::Web,
                "Potato" => ExportCompression::Potato,
                _ => ExportCompression::Maximum,
            },
            codec: match prefs.codec.as_deref() {
                Some("Hevc") => ExportCodec::Hevc,
                Some("Av1") => ExportCodec::Av1,
                _ => ExportCodec::H264,
            },
            gif_quality: GifQualityPreset::from_slug(prefs.gif_quality.as_deref()),
            optimize_filesize: prefs.optimize_filesize,
            cursor_only: prefs.cursor_only,
            custom_bpp: prefs.custom_bpp,
            force_ffmpeg: prefs.force_ffmpeg_decoder,
            advanced_open: prefs.advanced_open,
            clip: None,
            clip_only: false,
            preview: None,
            preview_stats: None,
            preview_error: None,
            preview_task: None,
            preview_request: Arc::new(()),
            estimate: None,
            estimate_loading: false,
            estimate_cancel: Arc::new(AtomicBool::new(false)),
            phase: ExportPhase::Idle,
            details_open: false,
            finished_at: None,
            close_requested: false,
            rendered: 0,
            total_frames: 0,
            output_path: None,
            error: None,
            cancel: Arc::new(AtomicBool::new(false)),
            export_task: None,
            bpp_track: ui::SliderTrack::default(),
            bpp_dragging: false,
            resolution_from_source: prefs.resolution_from_source,
            fps_from_source: prefs.fps_from_source,
            source: ExportSource::default(),
            track_options: Vec::new(),
            tracks: Vec::new(),
            excluded_tracks: prefs
                .excluded_tracks
                .iter()
                .flatten()
                .filter_map(|slug| ExportTrack::from_slug(slug))
                .collect(),
            audio_format: audio_format_from_slug(prefs.audio_format.as_deref()),
            picture: ExportPicture::Recorded,
            sound: false,
            saved_format: match ExportFormatKind::from_slug(&prefs.format) {
                ExportFormatKind::Audio => ExportFormatKind::Mp4,
                format => format,
            },
        }
    }

    fn has_audio(&self) -> bool {
        self.sound
    }

    fn renders_video(&self) -> bool {
        self.format.renders_video()
    }

    fn set_picture(&mut self, picture: ExportPicture) {
        self.picture = picture;
        match picture {
            ExportPicture::None => {
                self.format = ExportFormatKind::Audio;
                self.cursor_only = false;
                self.clip_only = false;
            }
            ExportPicture::Visuals => {
                self.format = match self.saved_format {
                    ExportFormatKind::Tracks | ExportFormatKind::Audio => ExportFormatKind::Mp4,
                    format => format,
                };
                self.cursor_only = false;
            }
            ExportPicture::Recorded => {
                if self.format == ExportFormatKind::Audio && !self.has_audio() {
                    self.format = ExportFormatKind::Mp4;
                }
            }
        }
    }

    fn set_track_options(&mut self, options: Vec<TrackOption>) {
        self.tracks = selected_tracks(&options, &self.excluded_tracks);
        self.track_options = options;
        if self.format == ExportFormatKind::Tracks && self.track_options.is_empty() {
            self.format = ExportFormatKind::Mp4;
        }
    }

    pub(crate) fn use_tracks(&mut self, slugs: Option<&str>) {
        if self.track_options.is_empty() {
            return;
        }
        self.format = ExportFormatKind::Tracks;
        self.cursor_only = false;
        if let Some(slugs) = slugs {
            let mut tracks = slugs
                .split(',')
                .filter_map(|slug| ExportTrack::from_slug(slug.trim()))
                .filter(|kind| self.track_options.iter().any(|option| option.kind == *kind))
                .collect::<Vec<_>>();
            tracks.sort();
            tracks.dedup();
            self.tracks = tracks;
        }
    }

    fn toggle_track(&mut self, kind: ExportTrack) {
        if let Some(index) = self.tracks.iter().position(|track| *track == kind) {
            self.tracks.remove(index);
            if !self.excluded_tracks.contains(&kind) {
                self.excluded_tracks.push(kind);
            }
        } else if self.track_options.iter().any(|option| option.kind == kind) {
            self.tracks.push(kind);
            self.tracks.sort();
            self.excluded_tracks.retain(|excluded| *excluded != kind);
        }
    }

    fn is_tracks(&self) -> bool {
        self.format == ExportFormatKind::Tracks
    }

    fn saves_to_file(&self) -> bool {
        !self.renders_video() || self.destination == ExportDestination::File
    }

    fn can_start(&self) -> bool {
        !self.phase.is_busy() && !(self.is_tracks() && self.tracks.is_empty())
    }

    fn choose_resolution(&mut self, resolution: ExportResolution) {
        self.resolution = resolution;
        self.resolution_from_source = false;
    }

    fn choose_fps(&mut self, fps: u32) {
        self.fps = fps;
        self.fps_from_source = false;
    }

    fn update_preview(&mut self, request: &Arc<()>, update: impl FnOnce(&mut Self)) -> bool {
        if !Arc::ptr_eq(&self.preview_request, request) {
            return false;
        }
        update(self);
        true
    }

    fn persist(&mut self) {
        if self.format != ExportFormatKind::Audio {
            self.saved_format = self.format;
        }
        let prefs = self.preferences();
        store::update(|state| state.export = Some(prefs));
    }

    fn preferences(&self) -> ExportPrefs {
        ExportPrefs {
            format: if self.format == ExportFormatKind::Audio {
                self.saved_format.slug().to_string()
            } else {
                self.format.slug().to_string()
            },
            fps: self.fps,
            export_to: self.destination.slug().to_string(),
            resolution: self.resolution.label().to_string(),
            compression: format!("{:?}", self.compression),
            codec: Some(format!("{:?}", self.codec)),
            optimize_filesize: self.optimize_filesize,
            cursor_only: self.cursor_only,
            custom_bpp: self.custom_bpp,
            force_ffmpeg_decoder: self.force_ffmpeg,
            advanced_open: self.advanced_open,
            organization_id: None,
            gif_quality: Some(self.gif_quality.slug().to_string()),
            resolution_from_source: self.resolution_from_source,
            fps_from_source: self.fps_from_source,
            audio_format: Some(self.audio_format.extension().to_string()),
            excluded_tracks: (!self.excluded_tracks.is_empty()).then(|| {
                self.excluded_tracks
                    .iter()
                    .map(|kind| kind.slug().to_string())
                    .collect()
            }),
        }
    }

    fn offer_clip(&mut self, clip: Option<ExportClip>, clip_only: bool) {
        self.clip_only = clip_only && clip.is_some();
        self.clip = clip;
        if self.clip_only && !self.renders_video() && self.picture == ExportPicture::Recorded {
            self.format = ExportFormatKind::Mp4;
        }
    }

    fn range(&self) -> Option<ExportRange> {
        self.clip
            .as_ref()
            .filter(|_| self.clip_only && self.renders_video())
            .map(|clip| clip.range)
    }

    fn range_labels(&self) -> Option<[String; 2]> {
        self.clip
            .as_ref()
            .filter(|_| self.renders_video())
            .map(|clip| [t!("Whole video").to_string(), clip.label()])
    }

    fn frames_to_export(&self, timeline_frames: u32) -> u32 {
        self.range().map_or(timeline_frames, |range| {
            range.frame_count(self.fps, timeline_frames)
        })
    }

    fn bpp(&self) -> f32 {
        self.custom_bpp
            .unwrap_or_else(|| self.compression.bits_per_pixel())
    }

    fn estimate_settings(&self) -> ExportSettings {
        let (width, height) = self.resolution.size();
        let resolution_base = XY::new(width, height);
        if self.cursor_only {
            ExportSettings::Mov(MovExportSettings {
                fps: self.fps,
                resolution_base,
                cursor_only: true,
            })
        } else if self.format == ExportFormatKind::Gif {
            ExportSettings::Gif(GifExportSettings {
                fps: self.fps,
                resolution_base,
                quality: Some(self.gif_quality.quality()),
            })
        } else {
            ExportSettings::Mp4(Mp4ExportSettings {
                codec: self.codec,
                fps: self.fps,
                resolution_base,
                compression: self.compression,
                custom_bpp: self.custom_bpp,
                force_ffmpeg_decoder: self.force_ffmpeg,
                optimize_filesize: self.optimize_filesize,
            })
        }
    }

    fn gif_preview_quality(&self) -> Option<GifQuality> {
        (self.format == ExportFormatKind::Gif && !self.cursor_only)
            .then(|| self.gif_quality.quality())
    }

    fn is_custom_bpp(&self) -> bool {
        self.custom_bpp
            .is_some_and(|bpp| (bpp - self.compression.bits_per_pixel()).abs() > 0.001)
    }

    fn clipboard_retry_path(&self) -> Option<&Path> {
        if self.phase == ExportPhase::Failed && !self.saves_to_file() {
            self.output_path.as_deref()
        } else {
            None
        }
    }

    fn copy_completed_export(
        &mut self,
        path: PathBuf,
        copy: impl FnOnce(&Path) -> Result<(), String>,
    ) {
        self.output_path = Some(path.clone());
        self.error = None;
        if self.close_requested || self.cancel.load(Ordering::Relaxed) {
            self.phase = ExportPhase::Idle;
            return;
        }

        self.phase = ExportPhase::Copying;
        match copy(&path) {
            Ok(()) => {
                self.phase = ExportPhase::Done;
                self.finished_at = Some(std::time::Instant::now());
            }
            Err(error) => {
                self.phase = ExportPhase::Failed;
                self.error = Some(error);
            }
        }
    }
}

fn first_export_prefs() -> ExportPrefs {
    ExportPrefs {
        format: "Mp4".into(),
        fps: 60,
        export_to: "file".into(),
        resolution: "1080p".into(),
        compression: "Maximum".into(),
        codec: None,
        optimize_filesize: false,
        cursor_only: false,
        custom_bpp: None,
        force_ffmpeg_decoder: false,
        advanced_open: false,
        organization_id: None,
        gif_quality: None,
        resolution_from_source: true,
        fps_from_source: true,
        excluded_tracks: None,
        audio_format: None,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct VideoSettings {
    pub format: ExportFormatKind,
    pub cursor_only: bool,
    pub fps: u32,
    pub size: (u32, u32),
    pub compression: ExportCompression,
    pub codec: ExportCodec,
    pub custom_bpp: Option<f32>,
    pub gif_quality: GifQualityPreset,
    pub optimize: bool,
    pub force_ffmpeg: bool,
}

impl VideoSettings {
    pub(crate) fn extension(&self) -> &'static str {
        if self.cursor_only {
            "mov"
        } else if self.format == ExportFormatKind::Gif {
            "gif"
        } else {
            "mp4"
        }
    }

    pub(crate) fn recorded_codec(&self) -> Option<String> {
        (self.extension() == "mp4").then(|| format!("{:?}", self.codec))
    }
}

pub(crate) fn last_video_settings(
    prefs: Option<ExportPrefs>,
    source: ExportSource,
    transparent: bool,
) -> VideoSettings {
    let mut ui = ExportUi::from_preferences(prefs.unwrap_or_else(first_export_prefs));
    ui.format = match ui.format {
        _ if transparent => ExportFormatKind::Gif,
        ExportFormatKind::Tracks | ExportFormatKind::Audio => ExportFormatKind::Mp4,
        format => format,
    };
    ui.cursor_only = false;
    ui.source = source;
    normalize_export_options(&mut ui, transparent);
    ui.video_settings()
}

pub(crate) fn recording_source(
    summary: &crate::editor_window::ProjectSummary,
    project: &frameever_project::ProjectConfiguration,
) -> ExportSource {
    let segments = &summary.recordings.segments;
    let size = segments.first().map(|segment| {
        let options = frameever_rendering::RenderOptions {
            camera_size: segment
                .camera
                .as_ref()
                .map(|camera| XY::new(camera.width, camera.height)),
            screen_size: XY::new(segment.display.width, segment.display.height),
            preserve_screen_alpha: false,
        };
        let (width, height) =
            frameever_rendering::ProjectUniforms::get_base_size(&options, project);
        XY::new(width, height)
    });
    let fps = highest_recorded_fps(
        segments.iter().enumerate().map(|(index, segment)| {
            let recorded = summary.recorded_fps.get(index);
            (
                capture_fps(segment.display.fps, recorded.map(|(display, _)| *display)),
                segment.camera.as_ref().map(|camera| {
                    capture_fps(camera.fps, recorded.and_then(|(_, camera)| *camera))
                }),
            )
        }),
        !project.camera.hide,
    );
    ExportSource { size, fps }
}

pub(crate) fn has_transparent_background(
    project: &frameever_project::ProjectConfiguration,
) -> bool {
    matches!(
        project.background.source,
        BackgroundSource::Color { alpha, .. } if alpha < 255
    )
}

fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

fn format_estimate_range(range: [f64; 2], time: bool) -> String {
    if range[1] < 1.0 {
        return if time { t!("< 1s") } else { "< 1 MB" }.into();
    }
    let (scale, unit) = if time {
        if range[1] >= 3600.0 {
            (3600.0, t!("hr"))
        } else if range[1] >= 60.0 {
            (60.0, t!("min"))
        } else {
            (1.0, t!("s"))
        }
    } else if range[1] >= 1024.0 {
        (1024.0, "GB")
    } else {
        (1.0, "MB")
    };
    let precision = if scale == 1.0 { 1.0 } else { 10.0 };
    let lower = ((range[0] / scale * precision).round() / precision).max(1.0 / precision);
    let upper = ((range[1] / scale * precision).round() / precision).max(lower);
    if lower == upper {
        format!("~{lower} {unit}")
    } else {
        format!("~{lower}–{upper} {unit}")
    }
}

fn decode_jpeg_bytes(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let rgba = frameever_gpui_kernels::codec::decode_jpeg_to_bgra(bytes)?;
    Some(Arc::new(RenderImage::new(smallvec::smallvec![
        image::Frame::new(rgba)
    ])))
}

impl EditorWindow {
    pub(crate) fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let clip = selected_clip(self.selection.as_ref()).and_then(|index| self.export_clip(index));
        self.open_export_offering(clip, false, window, cx);
    }

    pub(crate) fn open_clip_export(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let clip = self.export_clip(index);
        self.open_export_offering(clip, true, window, cx);
    }

    fn export_clip(&self, index: usize) -> Option<ExportClip> {
        self.timeline
            .clips
            .get(index)
            .and_then(ExportClip::from_segment)
    }

    fn open_export_offering(
        &mut self,
        clip: Option<ExportClip>,
        clip_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.playing {
            self.toggle_play_from_crop(cx);
        }
        self.set_preview_comparing(false, cx);
        self.set_selection(None, cx);
        let mut ui = ExportUi::load();
        ui.offer_clip(clip, clip_only);
        ui.source = self.export_source();
        ui.set_track_options(
            self.summary()
                .map(|summary| track_options(&summary.recordings, &self.project))
                .unwrap_or_default(),
        );
        ui.sound = self.summary().is_some_and(|summary| {
            frameever_export::audio::has_audio(&summary.recordings, &self.project)
        });
        ui.set_picture(match (self.is_audio_only(), self.audio_visuals_enabled()) {
            (false, _) => ExportPicture::Recorded,
            (true, false) => ExportPicture::None,
            (true, true) => ExportPicture::Visuals,
        });
        if has_transparent_background(&self.project) && ui.renders_video() {
            ui.format = ExportFormatKind::Gif;
        }
        self.normalize_export_options(&mut ui);
        self.export = Some(ui);
        self.refresh_export_preview(window, cx);
        cx.notify();
    }

    pub(crate) fn close_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ui) = self.export.as_mut() {
            if ui.phase.is_busy() {
                ui.close_requested = true;
                ui.cancel.store(true, Ordering::Relaxed);
                cx.notify();
                return;
            }
            if let Some(image) = ui.preview.take() {
                let _ = window.drop_image(image);
            }
            ui.persist();
        }
        self.export = None;
        cx.notify();
    }

    fn finish_requested_export_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .export
            .as_ref()
            .is_some_and(|ui| ui.close_requested && !ui.phase.is_busy())
        {
            self.close_export(window, cx);
        }
    }

    fn normalize_export_options(&self, ui: &mut ExportUi) {
        normalize_export_options(ui, has_transparent_background(&self.project));
    }

    fn export_source(&self) -> ExportSource {
        self.summary()
            .map(|summary| recording_source(summary, &self.project))
            .unwrap_or_default()
    }

    fn normalize_loaded_export_options(&mut self) {
        let transparent = has_transparent_background(&self.project);
        if let Some(ui) = self.export.as_mut() {
            normalize_export_options(ui, transparent);
        }
    }

    fn export_fps_options(&self, ui: &ExportUi) -> &'static [u32] {
        export_fps_options(ui.format, ui.cursor_only)
    }

    fn export_resolutions(&self, ui: &ExportUi) -> Vec<ExportResolution> {
        export_resolutions(
            ui.format,
            ui.cursor_only,
            has_transparent_background(&self.project),
        )
        .to_vec()
    }

    fn refresh_export_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.project_path.clone();
        let time = self.preview_or_playhead();
        let instance = self.instance.clone();
        let Some(ui) = self.export.as_mut() else {
            return;
        };
        let project = self.project.clone();
        ui.estimate_cancel.store(true, Ordering::Release);
        ui.estimate_cancel = Arc::new(AtomicBool::new(false));
        ui.estimate = None;
        let estimate_wanted = ui.renders_video();
        ui.estimate_loading = estimate_wanted;
        ui.preview_stats = None;
        let estimate_cancel = ui.estimate_cancel.clone();
        let estimate_settings = ui.estimate_settings();
        let range = ui.range();
        let time = preview_time_in_range(time, range);
        let estimate_instance = instance.clone();
        let estimate_project = project.clone();
        let (width, height) = ui.resolution.size();
        let settings = ExportPreviewSettings {
            fps: ui.fps,
            resolution_base: XY::new(width, height),
            compression_bpp: ui.bpp(),
            cursor_only: ui.cursor_only,
            gif_quality: ui.gif_preview_quality(),
        };
        // Match Windows editor playback: a fresh Media Foundation preview seek can return black.
        let force = cfg!(target_os = "windows") || ui.force_ffmpeg;
        ui.preview_error = None;
        let request = Arc::new(());
        ui.preview_request = request.clone();
        ui.preview_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let started = std::time::Instant::now();
            let result = gpui_tokio::Tokio::spawn(cx, async move {
                match instance {
                    Some(instance) => {
                        render_preview_with_editor(&instance, project, time, settings).await
                    }
                    None => render_preview_with_config(path, project, time, settings, force).await,
                }
            })
            .await
            .ok();
            tracing::debug!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "export preview rendered"
            );
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(ui) = this.export.as_mut() else {
                    return;
                };
                let updated = ui.update_preview(&request, |ui| match result {
                    Some(Ok(preview)) => {
                        let bytes = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            preview.jpeg_base64.as_bytes(),
                        )
                        .ok();
                        if let Some(image) = bytes.as_deref().and_then(decode_jpeg_bytes)
                            && let Some(old) = ui.preview.replace(image)
                        {
                            let _ = window.drop_image(old);
                        }
                        ui.preview_stats = Some(PreviewStats {
                            width: preview.actual_width,
                            height: preview.actual_height,
                            total_frames: preview.total_frames,
                            estimated_size_mb: preview.estimated_size_mb,
                        });
                        ui.preview_error = None;
                    }
                    Some(Err(error)) => {
                        ui.preview_error = Some(error.to_string());
                    }
                    None => {
                        ui.preview_error = Some(t!("Preview unavailable").into());
                    }
                });
                if !updated {
                    return;
                }
                cx.notify();
                window.refresh();
            });
            let result = if let Some(instance) = estimate_instance.filter(|_| estimate_wanted) {
                let (estimate_tx, estimate_rx) = flume::unbounded();
                let estimate_task = gpui_tokio::Tokio::spawn(cx, async move {
                    estimate_export(
                        instance,
                        estimate_project,
                        estimate_settings,
                        range,
                        estimate_cancel,
                        move |estimate| {
                            let _ = estimate_tx.send(estimate);
                        },
                    )
                    .await
                });
                while let Ok(estimate) = estimate_rx.recv_async().await {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(ui) = this.export.as_mut()
                            && !ui.estimate_cancel.load(Ordering::Acquire)
                            && ui.update_preview(&request, |ui| ui.estimate = Some(estimate))
                        {
                            cx.notify();
                        }
                    });
                }
                estimate_task.await.ok().and_then(Result::ok)
            } else {
                None
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(ui) = this.export.as_mut()
                    && ui.update_preview(&request, |ui| {
                        if let Some(estimate) = result {
                            ui.estimate = Some(estimate);
                        }
                        ui.estimate_loading = false;
                    })
                {
                    cx.notify();
                }
            });
        }));
    }

    pub(crate) fn start_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pretty_name = self
            .summary()
            .map(|summary| summary.pretty_name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| frameever_utils::brand::DEFAULT_RECORDING_NAME.into());
        let Some(ui) = self.export.as_mut() else {
            return;
        };
        ui.estimate_cancel.store(true, Ordering::Release);
        if ui.phase.is_busy() || ui.close_requested {
            return;
        }
        if !ui.renders_video() {
            self.start_file_export(pretty_name, window, cx);
            return;
        }
        let estimate = ui
            .estimate
            .as_ref()
            .map(|estimate| estimate.size_range_mb[1])
            .or_else(|| {
                ui.preview_stats
                    .as_ref()
                    .map(|stats| stats.estimated_size_mb)
            })
            .map(estimated_working_bytes)
            .unwrap_or(0);
        let destination = ui.destination;
        let settings = ui.video_settings();
        let range = ui.range();
        let project_path = self.project_path.clone();
        let project = self.project.clone();
        ui.cancel = Arc::new(AtomicBool::new(false));
        let cancel = ui.cancel.clone();
        ui.phase = if destination == ExportDestination::File {
            ExportPhase::ChoosingFile
        } else {
            ExportPhase::Starting
        };
        ui.error = None;
        ui.rendered = 0;
        ui.total_frames = 0;
        ui.output_path = None;
        cx.notify();

        let ext = settings.extension();
        let recorded_codec = settings.recorded_codec();
        let record_project = project_path.clone();

        ui.export_task = Some(cx.spawn_in(window, async move |this, cx| {
            let save_path = if destination == ExportDestination::File {
                let default = format!("{pretty_name}.{ext}");
                let chosen: Result<Option<PathBuf>, String> = match std::env::var_os("FRAMEEVER_GPUI_AUTO_EXPORT") {
                    Some(path) => Ok(Some(PathBuf::from(path))),
                    None => {
                        let folder = library::export_dialog_folder();
                        #[cfg(target_os = "macos")]
                        {
                            platform::try_save_file_panel(Some(&folder), &default, &[ext])
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            Ok(platform::save_file_panel_in(&folder, &default, &[ext]))
                        }
                    }
                };
                if matches!(chosen, Ok(None)) {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(ui) = this.export.as_mut()
                            && Arc::ptr_eq(&ui.cancel, &cancel)
                        {
                            ui.phase = ExportPhase::Idle;
                        }
                        cx.notify();
                    });
                    let _ = this.update_in(cx, |this, window, cx| {
                        this.finish_requested_export_close(window, cx)
                    });
                    return;
                }
                match chosen {
                    Ok(path) => path,
                    Err(error) => {
                        tracing::warn!(error, "Save dialog unavailable; keeping the export in its project output folder");
                        None
                    }
                }
            } else {
                None
            };

            let output = save_path.clone().unwrap_or_else(|| project_path.join("output"));
            let resources = ExportResources::new(vec![DiskBudget {
                path: output,
                description: "your export drive",
                estimated_bytes: estimate,
            }]);
            if !confirm_export_resources(&this, cx, &resources, &cancel).await {
                let _ = this.update_in(cx, |this, window, cx| this.finish_requested_export_close(window, cx));
                return;
            }

            let started = this.update_in(cx, |this, _, cx| {
                let Some(ui) = this.export.as_mut() else {
                    return false;
                };
                if !Arc::ptr_eq(&ui.cancel, &cancel) {
                    return false;
                }
                if cancel.load(Ordering::Relaxed) {
                    ui.phase = ExportPhase::Idle;
                    cx.notify();
                    return false;
                }
                ui.phase = ExportPhase::Starting;
                cx.notify();
                true
            });
            if !started.unwrap_or(false) {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.finish_requested_export_close(window, cx)
                });
                return;
            }

            let notify_file_save = save_path.is_some();
            let (progress_tx, progress_rx) = flume::unbounded::<(u32, u32)>();
            let export_cancel = cancel.clone();
            let export = gpui_tokio::Tokio::spawn(cx, async move {
                let stopped = export_cancel.clone();
                resources.supervise(render_video(
                    project_path,
                    project,
                    settings,
                    range,
                    save_path.clone(),
                    progress_tx,
                    export_cancel,
                ), || stopped.store(true, Ordering::Release)).await
            });

            loop {
                while let Ok((rendered, total)) = progress_rx.try_recv() {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(ui) = this.export.as_mut() {
                            ui.rendered = rendered;
                            ui.total_frames = total;
                            ui.phase = ExportPhase::Rendering;
                        }
                        cx.notify();
                    });
                }
                if progress_rx.is_disconnected() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
            }

            match export.await {
                Ok(Ok(path)) => {
                    tracing::info!(path = %path.display(), "editor export completed");
                    if destination == ExportDestination::Clipboard {
                        let _ = this.update(cx, |this, cx| {
                            if let Some(ui) = this.export.as_mut() {
                                ui.copy_completed_export(path, |path| {
                                    let result = platform::copy_file_to_clipboard(path, cx);
                                    crate::app_sounds::play_notification();
                                    result
                                });
                            }
                            cx.notify();
                        });
                    } else {
                        library::record_export(
                            &record_project,
                            &library::export_record(path.clone(), ext, recorded_codec),
                            notify_file_save,
                        );
                        if notify_file_save {
                            crate::app_sounds::play_notification();
                        }
                        let _ = this.update(cx, |this, cx| {
                            if let Some(ui) = this.export.as_mut() {
                                ui.phase = ExportPhase::Done;
                                ui.finished_at = Some(std::time::Instant::now());
                                ui.output_path = Some(path);
                            }
                            cx.notify();
                        });
                    }
                }
                Ok(Err(error)) => {
                    tracing::error!(error, "editor export failed");
                    let cancelled = !is_resource_stop(&error) && (error == "Export cancelled" || cancel.load(Ordering::Relaxed));
                    let _ = this.update(cx, |this, cx| {
                        if let Some(ui) = this.export.as_mut() {
                            if cancelled {
                                ui.phase = ExportPhase::Idle;
                            } else {
                                ui.phase = ExportPhase::Failed;
                                ui.error = Some(error);
                            }
                        }
                        cx.notify();
                    });
                }
                Err(_) => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(ui) = this.export.as_mut() {
                            ui.phase = ExportPhase::Failed;
                            ui.error = Some(t!("Export task failed").into());
                        }
                        cx.notify();
                    });
                }
            }
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish_requested_export_close(window, cx)
            });
        }));
    }

    fn start_file_export(
        &mut self,
        pretty_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let duration = self.total_duration();
        let project_path = self.project_path.clone();
        let project = self.project.clone();
        let Some(ui) = self.export.as_mut() else {
            return;
        };
        let job = match ui.format {
            ExportFormatKind::Audio => FileJob::Audio(ui.audio_format),
            _ if ui.tracks.is_empty() => return,
            _ => FileJob::Tracks {
                selection: ui.tracks.clone(),
                fps: ui.fps,
                force_ffmpeg_decoder: ui.force_ffmpeg,
            },
        };
        let estimate = match &job {
            FileJob::Audio(format) => format.estimated_bytes(duration),
            FileJob::Tracks { selection, fps, .. } => {
                estimated_tracks_bytes(&ui.track_options, selection, *fps, duration)
            }
        };
        let (suggested, extensions, fallback) = match &job {
            FileJob::Audio(format) => (
                pretty_name.clone(),
                vec![format.extension()],
                project_path
                    .join("output")
                    .join(format!("result.{}", format.extension())),
            ),
            FileJob::Tracks { .. } => (
                t!("{name} tracks", name = pretty_name),
                Vec::new(),
                project_path.join("output").join("tracks"),
            ),
        };
        let record_format = job.record_format();
        let record_project = project_path.clone();
        ui.cancel = Arc::new(AtomicBool::new(false));
        let cancel = ui.cancel.clone();
        ui.phase = ExportPhase::ChoosingFile;
        ui.error = None;
        ui.rendered = 0;
        ui.total_frames = 0;
        ui.output_path = None;
        cx.notify();

        ui.export_task = Some(cx.spawn_in(window, async move |this, cx| {
            let chosen: Result<Option<PathBuf>, String> =
                match std::env::var_os("FRAMEEVER_GPUI_AUTO_EXPORT") {
                    Some(path) => Ok(Some(PathBuf::from(path))),
                    None => {
                        let parent = library::export_dialog_folder();
                        #[cfg(target_os = "macos")]
                        {
                            platform::try_save_file_panel(Some(&parent), &suggested, &extensions)
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            Ok(platform::save_file_panel_in(
                                &parent,
                                &suggested,
                                &extensions,
                            ))
                        }
                    }
                };
            let (folder, remember_folder) = match chosen {
                Ok(Some(path)) => (path, true),
                Ok(None) => {
                    let _ = this.update_in(cx, |this, window, cx| {
                        if let Some(ui) = this.export.as_mut()
                            && Arc::ptr_eq(&ui.cancel, &cancel)
                        {
                            ui.phase = ExportPhase::Idle;
                        }
                        cx.notify();
                        this.finish_requested_export_close(window, cx);
                    });
                    return;
                }
                Err(error) => {
                    tracing::warn!(
                        error,
                        "Save dialog unavailable; keeping the export in the project output folder"
                    );
                    (fallback, false)
                }
            };

            let resources = ExportResources::new(vec![DiskBudget {
                path: folder.clone(),
                description: "your export drive",
                estimated_bytes: estimate,
            }]);
            if !confirm_export_resources(&this, cx, &resources, &cancel).await {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.finish_requested_export_close(window, cx)
                });
                return;
            }
            let started = this.update(cx, |this, cx| {
                let Some(ui) = this.export.as_mut() else {
                    return false;
                };
                if !Arc::ptr_eq(&ui.cancel, &cancel) {
                    return false;
                }
                if cancel.load(Ordering::Relaxed) {
                    ui.phase = ExportPhase::Idle;
                    cx.notify();
                    return false;
                }
                ui.phase = ExportPhase::Starting;
                cx.notify();
                true
            });
            if !started.unwrap_or(false) {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.finish_requested_export_close(window, cx)
                });
                return;
            }

            let (progress_tx, progress_rx) = flume::unbounded::<TrackProgress>();
            let export_cancel = cancel.clone();
            let export_folder = folder.clone();
            let started_at = std::time::Instant::now();
            let export = gpui_tokio::Tokio::spawn(cx, async move {
                let stopped = export_cancel.clone();
                resources
                    .supervise(
                        async move {
                            frameever_export::validate_output_path(&export_folder).map_err(
                                |_| t!("Export output must be a local file path").to_string(),
                            )?;
                            let on_progress = move |progress| {
                                let _ = progress_tx.send(progress);
                            };
                            match job {
                                FileJob::Tracks {
                                    selection,
                                    fps,
                                    force_ffmpeg_decoder,
                                } => tracks::export_tracks(
                                    project_path,
                                    Some(project),
                                    tracks::TrackExportSettings {
                                        tracks: selection,
                                        fps,
                                        force_ffmpeg_decoder,
                                    },
                                    export_folder.clone(),
                                    export_cancel,
                                    on_progress,
                                )
                                .await
                                .map(|_| export_folder),
                                FileJob::Audio(format) => {
                                    frameever_export::audio::export_audio(
                                        project_path,
                                        Some(project),
                                        format,
                                        export_folder,
                                        export_cancel,
                                        on_progress,
                                    )
                                    .await
                                }
                            }
                            .map_err(|error| error.to_string())
                        },
                        || stopped.store(true, Ordering::Release),
                    )
                    .await
            });

            loop {
                if let Some(progress) = progress_rx.try_iter().last() {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(ui) = this.export.as_mut() {
                            ui.rendered = progress.done.min(u64::from(u32::MAX)) as u32;
                            ui.total_frames = progress.total.min(u64::from(u32::MAX)) as u32;
                            ui.phase = ExportPhase::Rendering;
                        }
                        cx.notify();
                    });
                }
                if progress_rx.is_disconnected() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
            }

            let result = export.await;
            let _ = this.update(cx, |this, cx| {
                let Some(ui) = this.export.as_mut() else {
                    return;
                };
                match result {
                    Ok(Ok(saved)) => {
                        tracing::info!(
                            path = %saved.display(),
                            elapsed_ms = started_at.elapsed().as_millis() as u64,
                            "editor file export completed"
                        );
                        library::record_export(
                            &record_project,
                            &library::export_record(saved.clone(), record_format, None),
                            remember_folder,
                        );
                        crate::app_sounds::play_notification();
                        ui.phase = ExportPhase::Done;
                        ui.finished_at = Some(std::time::Instant::now());
                        ui.output_path = Some(saved);
                    }
                    Ok(Err(error)) => {
                        let cancelled = !is_resource_stop(&error)
                            && (error == "Export cancelled" || cancel.load(Ordering::Relaxed));
                        if cancelled {
                            ui.phase = ExportPhase::Idle;
                        } else {
                            tracing::error!(error, "editor file export failed");
                            ui.phase = ExportPhase::Failed;
                            ui.error = Some(error);
                        }
                    }
                    Err(error) => {
                        tracing::error!(%error, "editor file export task ended without a result");
                        ui.phase = ExportPhase::Failed;
                        ui.error = Some(t!("Export task failed").into());
                    }
                }
                cx.notify();
            });
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish_requested_export_close(window, cx)
            });
        }));
    }

    fn retry_clipboard_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.export.as_mut() else {
            return;
        };
        let Some(path) = ui.clipboard_retry_path().map(Path::to_path_buf) else {
            return;
        };
        ui.cancel = Arc::new(AtomicBool::new(false));
        ui.copy_completed_export(path, |path| {
            let result = platform::copy_file_to_clipboard(path, cx);
            crate::app_sounds::play_notification();
            result
        });
        cx.notify();
        self.finish_requested_export_close(window, cx);
    }

    pub(crate) fn render_export_page(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        let Some(ui) = self.export.as_ref() else {
            return div().into_any_element();
        };

        let header = div()
            .relative()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .pl(px(if cfg!(target_os = "macos") { 92. } else { 12. }))
            .pr(px(12.))
            .border_b_1()
            .border_color(Hsla::from(editor.line))
            .when(cfg!(target_os = "windows"), |header| {
                header.window_control_area(gpui::WindowControlArea::Drag)
            })
            .when(!cfg!(target_os = "windows"), |header| {
                header.on_mouse_down(gpui::MouseButton::Left, |event, window, _| {
                    if event.click_count == 2 {
                        window.titlebar_double_click();
                    } else {
                        window.start_window_move();
                    }
                })
            })
            .child(
                div().flex_1().flex().flex_row().items_center().child(
                    div()
                        .id("export-back")
                        .role(gpui::Role::Button)
                        .aria_label(t!("Back to editor"))
                        .tab_index(0)
                        .when(cfg!(target_os = "windows"), |button| button.occlude())
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .h(px(crate::theme::CONTROL_HEIGHT))
                        .pl(px(8.))
                        .pr(px(10.))
                        .rounded(px(8.))
                        .tab_index(0)
                        .cursor_pointer()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Hsla::from(editor.text_2))
                        .hover(move |style| {
                            style
                                .bg(Hsla::from(editor.ctl))
                                .text_color(Hsla::from(editor.text_1))
                        })
                        .child(
                            svg()
                                .path("icons/move-left.svg")
                                .size(px(14.))
                                .text_color(Hsla::from(editor.text_2)),
                        )
                        .child(t!("Back to editor"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_export(window, cx);
                        })),
                ),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(Hsla::from(editor.text_1))
                    .child(t!("Export")),
            )
            .child(div().flex_1());
        #[cfg(target_os = "windows")]
        let header = header.child(div().absolute().right_0().top_0().h_full().child(
            ui::windows_caption_controls(
                theme,
                window.is_window_active(),
                window.is_maximized(),
                true,
                true,
            ),
        ));

        let viewport = window.viewport_size();
        let preview_box = preview_fit(
            f32::from(viewport.width) - SIDEBAR_WIDTH - STAGE_PADDING_X * 2.,
            f32::from(viewport.height) - HEADER_HEIGHT - STAGE_VERTICAL_CHROME,
            ui.preview.as_ref().map(|image| {
                let size = image.size(0);
                (size.width.0 as f32, size.height.0 as f32)
            }),
        );

        div()
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .child(header)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_export_preview_pane(ui, preview_box))
                    .child(if ui.phase.shows_progress() {
                        div()
                            .w(px(SIDEBAR_WIDTH))
                            .h_full()
                            .flex_none()
                            .border_l_1()
                            .border_color(theme.border)
                            .child(self.render_export_overlay(ui, cx))
                            .into_any_element()
                    } else {
                        self.render_export_sidebar(ui, cx).into_any_element()
                    })
                    .when(ui.phase == ExportPhase::ChoosingFile, |this| {
                        this.child(div().absolute().inset_0().occlude())
                    }),
            )
            .when(ui.bpp_dragging, |this| {
                this.child(ui::Slider::drag_layer(
                    "export-bpp-drag",
                    cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                        this.export_bpp_drag_to(event.position.x, cx);
                    }),
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.export_bpp_mouse_up(window, cx);
                    }),
                ))
            })
            .into_any_element()
    }

    fn render_export_preview_pane(
        &self,
        ui: &ExportUi,
        preview_box: (f32, f32),
    ) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        let stats = ui.preview_stats.as_ref();
        let tracks_mode = !ui.renders_video();
        let tracks_size = if ui.format == ExportFormatKind::Audio {
            ui.audio_format.estimated_bytes(self.total_duration())
        } else {
            estimated_tracks_bytes(&ui.track_options, &ui.tracks, ui.fps, self.total_duration())
        } as f64
            / (1024.0 * 1024.0);
        let duration = stats
            .map(|stats| {
                if ui.fps == 0 {
                    0.0
                } else {
                    ui.frames_to_export(stats.total_frames) as f64 / ui.fps as f64
                }
            })
            .unwrap_or(0.0);
        let (box_w, box_h) = preview_box;

        let preview = match ui.preview.clone() {
            Some(image) => {
                use gpui::StyledImage as _;
                div()
                    .w(px(box_w))
                    .h(px(box_h))
                    .rounded(px(10.))
                    .overflow_hidden()
                    .child(img(image).object_fit(gpui::ObjectFit::Contain).size_full())
                    .into_any_element()
            }
            None => div()
                .w(px(box_w))
                .h(px(box_h))
                .rounded(px(10.))
                .bg(Hsla::from(editor.ctl))
                .flex()
                .items_center()
                .justify_center()
                .px(px(24.))
                .text_size(px(12.))
                .text_color(Hsla::from(editor.text_2))
                .text_center()
                .child(
                    ui.preview_error
                        .clone()
                        .unwrap_or_else(|| t!("Generating preview…").into()),
                )
                .into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .bg(Hsla::from(editor.stage))
            .pt(px(16.))
            .px(px(STAGE_PADDING_X))
            .pb(px(20.))
            .child(
                div()
                    .h(px(22.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Hsla::from(editor.text_2))
                            .child(t!("Preview")),
                    )
                    .child(
                        div()
                            .id("export-preview-info")
                            .flex()
                            .items_center()
                            .tooltip(move |_window, cx| {
                                crate::ui::Tooltip::new(
                                    &theme,
                                    "This is a rendered frame from your video. Adjust the \
                                     settings to see the quality of the final export.",
                                )
                                .view(cx)
                            })
                            .child(
                                svg()
                                    .path("icons/info.svg")
                                    .size(px(14.))
                                    .text_color(Hsla::from(editor.text_3)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .items_center()
                    .justify_center()
                    .pt(px(14.))
                    .pb(px(18.))
                    .child(preview),
            )
            .child(
                div().flex().flex_row().justify_center().child(
                    div()
                        .h(px(STATS_HEIGHT))
                        .min_w(px(520.))
                        .flex()
                        .flex_row()
                        .rounded(px(10.))
                        .bg(Hsla::from(editor.card))
                        .child(export_stat(
                            &theme,
                            t!("Duration"),
                            stats.map(|_| format_duration(duration)),
                            false,
                        ))
                        .child(if ui.format == ExportFormatKind::Audio {
                            export_stat(
                                &theme,
                                t!("Output"),
                                Some(audio_format_label(ui.audio_format).to_string()),
                                true,
                            )
                        } else if tracks_mode {
                            export_stat(
                                &theme,
                                t!("Files"),
                                Some(ui.tracks.len().to_string()),
                                true,
                            )
                        } else {
                            export_stat(
                                &theme,
                                t!("Output"),
                                stats.map(|stats| {
                                    format!("{}×{} · {} fps", stats.width, stats.height, ui.fps)
                                }),
                                true,
                            )
                        })
                        .child(export_stat(
                            &theme,
                            if ui.estimate_loading && ui.estimate.is_some() {
                                t!("Refining size…")
                            } else {
                                t!("Estimated size")
                            },
                            Some(if tracks_mode {
                                if ui.is_tracks() && ui.tracks.is_empty() {
                                    t!("Unavailable").into()
                                } else {
                                    format_estimate_range([tracks_size, tracks_size], false)
                                }
                            } else {
                                ui.estimate
                                    .as_ref()
                                    .map(|estimate| {
                                        format_estimate_range(estimate.size_range_mb, false)
                                    })
                                    .unwrap_or_else(|| {
                                        if ui.estimate_loading {
                                            t!("Calculating…").into()
                                        } else {
                                            t!("Unavailable").into()
                                        }
                                    })
                            }),
                            true,
                        ))
                        .when(!tracks_mode, |card| {
                            card.child(export_stat(
                                &theme,
                                if ui.estimate_loading && ui.estimate.is_some() {
                                    t!("Refining time…")
                                } else {
                                    t!("Export time")
                                },
                                Some(
                                    ui.estimate
                                        .as_ref()
                                        .map(|estimate| {
                                            format_estimate_range(estimate.time_range_seconds, true)
                                        })
                                        .unwrap_or_else(|| {
                                            if ui.estimate_loading {
                                                t!("Calculating…").into()
                                            } else {
                                                t!("Unavailable").into()
                                            }
                                        }),
                                ),
                                true,
                            ))
                        }),
                ),
            )
    }

    fn render_export_sidebar(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        let transparent = has_transparent_background(&self.project);
        let tracks_mode = ui.is_tracks();
        let video = ui.renders_video();
        let resolutions = self.export_resolutions(ui);
        let fps_options = self.export_fps_options(ui);

        div()
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(Hsla::from(editor.card))
            .border_l_1()
            .border_color(Hsla::from(editor.line))
            .child(
                div()
                    .id("export-settings")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(16.))
                    .gap(px(20.))
                    .children(
                        ui.range_labels()
                            .map(|labels| self.export_range_field(ui, labels, cx)),
                    )
                    .when(video, |el| el.child(self.export_destination_field(ui, cx)))
                    .child(self.export_format_field(ui, transparent, cx))
                    .when(tracks_mode, |el| el.child(self.export_tracks_field(ui, cx)))
                    .when(ui.format == ExportFormatKind::Audio, |el| {
                        el.child(self.export_audio_type_field(ui, cx))
                    })
                    .when(
                        ui.format == ExportFormatKind::Mp4 && !ui.cursor_only,
                        |el| el.child(self.export_codec_field(ui, cx)),
                    )
                    .when(video, |el| {
                        el.child(self.export_resolution_field(ui, &resolutions, cx))
                    })
                    .when(ui.format != ExportFormatKind::Audio, |el| {
                        el.child(self.export_fps_field(ui, fps_options, cx))
                    })
                    .when(
                        ui.format == ExportFormatKind::Mp4 && !ui.cursor_only,
                        |this| this.child(self.export_quality_field(ui, cx)),
                    )
                    .when(
                        ui.format == ExportFormatKind::Gif && !ui.cursor_only,
                        |this| this.child(self.export_gif_quality_field(ui, cx)),
                    )
                    .when(video, |el| {
                        el.child(div().h(px(1.)).flex_none().bg(Hsla::from(editor.line)))
                            .child(self.export_advanced(ui, cx))
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .px(px(16.))
                    .pt(px(12.))
                    .pb(px(16.))
                    .border_t_1()
                    .border_color(Hsla::from(editor.line))
                    .when(tracks_mode && ui.tracks.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(11.))
                                .line_height(px(14.))
                                .text_color(Hsla::from(editor.text_3))
                                .child(t!("Choose at least one track.")),
                        )
                    })
                    .child(self.export_cta(ui, cx)),
            )
    }

    fn export_cta(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        let (label, icon, primary) = match ui.destination {
            _ if ui.is_tracks() => (t!("Export tracks"), Some("icons/folder.svg"), true),
            _ if ui.format == ExportFormatKind::Audio => {
                (t!("Export audio"), Some("icons/folder.svg"), true)
            }
            ExportDestination::File => (t!("Export to File"), Some("icons/folder.svg"), true),
            ExportDestination::Clipboard => {
                (t!("Export to Clipboard"), Some("icons/copy.svg"), true)
            }
        };
        let busy = !ui.can_start();
        let (bg, hover_bg, text) = if primary {
            (
                Hsla::from(editor.accent),
                Hsla::from(editor.accent_hover),
                gpui::white(),
            )
        } else {
            (
                Hsla::from(editor.ctl),
                Hsla::from(editor.ctl_hover),
                Hsla::from(editor.text_1),
            )
        };

        div()
            .id("export-cta")
            .role(gpui::Role::Button)
            .aria_label(label)
            .tab_index(0)
            .w_full()
            .h(px(40.))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .rounded(px(10.))
            .bg(bg)
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(text)
            .when(busy, |this| this.opacity(0.5))
            .when(!busy, |this| {
                this.tab_index(0)
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover_bg))
                    .on_click(cx.listener(|this, _, window, cx| this.start_export(window, cx)))
            })
            .children(icon.map(|icon| svg().path(icon).size(px(16.)).text_color(text)))
            .child(label)
    }

    fn export_section(&self, name: &'static str, icon: &'static str) -> ui::Field {
        ui::Field::section(&self.theme, name)
            .icon(icon)
            .icon_size(px(14.))
            .gap(px(8.))
    }

    fn export_range_field(
        &self,
        ui: &ExportUi,
        labels: [String; 2],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        self.export_section(t!("Range"), "icons/scissors.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-range",
                    labels
                        .into_iter()
                        .enumerate()
                        .map(|(index, label)| {
                            ui::SegmentOption::new(label, ui.clip_only == (index == 1))
                        })
                        .collect(),
                )
                .stretch()
                .item_height(px(SEGMENT_HEIGHT))
                .on_select(cx.listener(|this, index: &usize, window, cx| {
                    if let Some(ui) = this.export.as_mut() {
                        ui.clip_only = *index == 1;
                    }
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                })),
            )
    }

    fn export_destination_field(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        self.export_section(t!("Destination"), "icons/upload.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-destination",
                    ExportDestination::ALL
                        .iter()
                        .map(|dest| {
                            let mut option =
                                ui::SegmentOption::new(dest.label(), ui.destination == *dest);
                            option.icon = Some(dest.icon().into());
                            option
                        })
                        .collect(),
                )
                .stretch()
                .item_height(px(30.))
                .on_select(cx.listener(|this, index: &usize, window, cx| {
                    let Some(dest) = ui::option_at(ExportDestination::ALL, *index) else {
                        return;
                    };
                    if let Some(ui) = this.export.as_mut() {
                        ui.destination = dest;
                        ui.persist();
                    }
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                })),
            )
    }

    fn export_format_field(
        &self,
        ui: &ExportUi,
        transparent: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let options = ExportFormatKind::ALL;
        let availability = FormatAvailability {
            transparent,
            cursor_only: ui.cursor_only,
            no_tracks: ui.track_options.is_empty(),
            no_audio: !ui.has_audio(),
            picture: ui.picture,
        };
        let disabled = move |format: ExportFormatKind| availability.disabled(format);
        self.export_section(t!("Format"), "icons/video.svg").child(
            ui::SegmentedControl::editor(
                &theme,
                "export-format",
                options
                    .iter()
                    .map(|format| {
                        ui::SegmentOption::new(format.label(), ui.format == *format)
                            .disabled(disabled(*format))
                    })
                    .collect(),
            )
            .stretch()
            .item_height(px(SEGMENT_HEIGHT))
            .on_select(cx.listener(move |this, index: &usize, window, cx| {
                let Some(format) = options.get(*index).copied() else {
                    return;
                };
                if disabled(format) {
                    return;
                }
                if let Some(ui) = this.export.as_mut() {
                    ui.format = format;
                    if !format.renders_video() {
                        ui.cursor_only = false;
                    }
                }
                this.normalize_loaded_export_options();
                if let Some(ui) = this.export.as_mut() {
                    ui.persist();
                }
                this.refresh_export_preview(window, cx);
                cx.notify();
            })),
        )
    }

    fn export_audio_type_field(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        let options = AudioExportFormat::ALL;
        self.export_section(t!("File type"), "icons/volume-2.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-audio-type",
                    options
                        .iter()
                        .map(|format| {
                            ui::SegmentOption::new(
                                audio_format_label(*format),
                                ui.audio_format == *format,
                            )
                        })
                        .collect(),
                )
                .stretch()
                .item_height(px(SEGMENT_HEIGHT))
                .on_select(cx.listener(move |this, index: &usize, _, cx| {
                    let Some(format) = options.get(*index).copied() else {
                        return;
                    };
                    if let Some(ui) = this.export.as_mut() {
                        ui.audio_format = format;
                        ui.persist();
                    }
                    cx.notify();
                })),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .text_color(Hsla::from(editor.text_3))
                    .child(audio_format_note(ui.audio_format)),
            )
    }

    fn export_tracks_field(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.theme.editor;
        let mut field = self.export_section(t!("Tracks"), "icons/layers.svg");
        for option in &ui.track_options {
            field =
                field.child(self.export_track_row(option, ui.tracks.contains(&option.kind), cx));
        }
        field.child(
            div()
                .text_size(px(11.))
                .line_height(px(14.))
                .text_color(Hsla::from(editor.text_3))
                .child(t!(
                    "Each track follows your cuts and speed, without background or effects."
                )),
        )
    }

    fn export_track_row(
        &self,
        option: &TrackOption,
        checked: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let editor = self.theme.editor;
        let kind = option.kind;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .min_h(px(34.))
            .child(
                svg()
                    .path(track_icon(kind))
                    .size(px(14.))
                    .flex_none()
                    .text_color(Hsla::from(if checked {
                        editor.text_1
                    } else {
                        editor.text_3
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(Hsla::from(editor.text_1))
                    .truncate()
                    .child(track_label(kind)),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(Hsla::from(editor.text_3))
                    .child(track_detail(option)),
            )
            .child(
                ui::Toggle::plain(&self.theme, track_toggle_id(kind), checked).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if let Some(ui) = this.export.as_mut() {
                            ui.toggle_track(kind);
                            ui.persist();
                        }
                        cx.notify();
                    }),
                ),
            )
    }

    fn export_codec_field(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let options = [ExportCodec::H264, ExportCodec::Hevc, ExportCodec::Av1];
        let description = match ui.codec {
            ExportCodec::H264 => t!("Plays everywhere, including older devices and browsers."),
            ExportCodec::Hevc => {
                t!("About 40% smaller at the same quality. Needs a recent player or device.")
            }
            ExportCodec::Av1 => t!(
                "About half the size at the same quality. Fast on recent graphics cards, slower otherwise; older devices may not play it."
            ),
        };
        self.export_section(t!("Video codec"), "icons/film.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-codec",
                    options
                        .iter()
                        .map(|codec| {
                            ui::SegmentOption::new(
                                match codec {
                                    ExportCodec::H264 => t!("H.264 · Compatible"),
                                    ExportCodec::Hevc => t!("HEVC · Smaller"),
                                    ExportCodec::Av1 => t!("AV1 · Smallest"),
                                },
                                ui.codec == *codec,
                            )
                        })
                        .collect(),
                )
                .stretch()
                .item_height(px(SEGMENT_HEIGHT))
                .on_select(cx.listener(move |this, index: &usize, window, cx| {
                    let Some(codec) = options.get(*index).copied() else {
                        return;
                    };
                    if let Some(ui) = this.export.as_mut() {
                        ui.codec = codec;
                        ui.persist();
                    }
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                })),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(Hsla::from(theme.muted_foreground))
                    .child(description),
            )
    }

    fn export_resolution_field(
        &self,
        ui: &ExportUi,
        resolutions: &[ExportResolution],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let resolutions = resolutions.to_vec();
        self.export_section(t!("Resolution"), "icons/monitor.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-resolution",
                    resolutions
                        .iter()
                        .map(|res| ui::SegmentOption::new(res.label(), ui.resolution == *res))
                        .collect(),
                )
                .stretch()
                .item_height(px(SEGMENT_HEIGHT))
                .on_select(cx.listener(move |this, index: &usize, window, cx| {
                    let Some(resolution) = resolutions.get(*index).copied() else {
                        return;
                    };
                    if let Some(ui) = this.export.as_mut() {
                        ui.choose_resolution(resolution);
                        ui.persist();
                    }
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                })),
            )
    }

    fn export_fps_field(
        &self,
        ui: &ExportUi,
        options: &[u32],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let options = options.to_vec();
        self.export_section(t!("Frame rate"), "icons/gauge.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-fps",
                    options
                        .iter()
                        .map(|fps| ui::SegmentOption::new(format!("{fps} FPS"), ui.fps == *fps))
                        .collect(),
                )
                .stretch()
                .item_height(px(SEGMENT_HEIGHT))
                .on_select(cx.listener(move |this, index: &usize, window, cx| {
                    let Some(fps) = options.get(*index).copied() else {
                        return;
                    };
                    if let Some(ui) = this.export.as_mut() {
                        ui.choose_fps(fps);
                        ui.persist();
                    }
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                })),
            )
    }

    fn export_quality_field(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        self.export_section(t!("Quality"), "icons/gem.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-quality",
                    COMPRESSION_PRESETS
                        .iter()
                        .map(|(value, label)| {
                            ui::SegmentOption::new(
                                tr(label),
                                !ui.is_custom_bpp() && matches_compression(ui.compression, *value),
                            )
                        })
                        .collect(),
                )
                .stretch()
                .item_height(px(SEGMENT_HEIGHT))
                .on_select(cx.listener(move |this, index: &usize, window, cx| {
                    let Some((compression, _)) = COMPRESSION_PRESETS.get(*index).copied() else {
                        return;
                    };
                    if let Some(ui) = this.export.as_mut() {
                        ui.compression = compression;
                        ui.custom_bpp = None;
                        ui.persist();
                    }
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .px(px(2.))
                    .text_size(px(10.5))
                    .text_color(Hsla::from(editor.text_3))
                    .child(t!("Smaller file"))
                    .child(t!("Larger file")),
            )
            .child(self.export_optimize_row(ui, cx))
            .child(self.export_enhance_row(cx))
    }

    fn export_gif_quality_field(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        self.export_section(t!("Quality"), "icons/gem.svg")
            .child(
                ui::SegmentedControl::editor(
                    &theme,
                    "export-gif-quality",
                    GifQualityPreset::ALL
                        .iter()
                        .map(|preset| {
                            ui::SegmentOption::new(
                                crate::library::gif::quality_label(*preset),
                                ui.gif_quality == *preset,
                            )
                        })
                        .collect(),
                )
                .stretch()
                .item_height(px(SEGMENT_HEIGHT))
                .on_select(cx.listener(move |this, index: &usize, window, cx| {
                    let Some(preset) = GifQualityPreset::ALL.get(*index).copied() else {
                        return;
                    };
                    if let Some(ui) = this.export.as_mut() {
                        ui.gif_quality = preset;
                        ui.persist();
                    }
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .px(px(2.))
                    .text_size(px(10.5))
                    .text_color(Hsla::from(editor.text_3))
                    .child(t!("Smaller file"))
                    .child(t!("Larger file")),
            )
            .child(self.export_enhance_row(cx))
    }

    fn export_enhance_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id("export-enhance-row")
            .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
            .tooltip(move |_window, cx| {
                ui::Tooltip::new(
                    &theme,
                    t!("Sharpens zoomed-in and scaled-down screen content (slower)"),
                )
                .view(cx)
            })
            .child(self.export_toggle_row(
                "export-enhance",
                t!("Enhance quality"),
                None,
                self.project.enhance_quality,
                cx.listener(|this, _, window, cx| {
                    this.toggle_enhance_quality(window, cx);
                    this.refresh_export_preview(window, cx);
                    cx.notify();
                }),
            ))
    }

    fn export_toggle_row(
        &self,
        id: &'static str,
        title: &'static str,
        description: Option<&'static str>,
        checked: bool,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> impl IntoElement {
        let editor = self.theme.editor;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .min_h(px(34.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Hsla::from(editor.text_1))
                            .child(title),
                    )
                    .children(description.map(|description| {
                        div()
                            .text_size(px(11.))
                            .line_height(px(14.))
                            .text_color(Hsla::from(editor.text_3))
                            .child(description)
                    })),
            )
            .child(ui::Toggle::plain(&self.theme, id, checked).on_click(on_click))
    }

    fn export_optimize_row(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        self.export_toggle_row(
            "export-optimize",
            t!("Optimize file size"),
            Some(t!(
                "Re-encodes with software for much smaller files (slower)"
            )),
            ui.optimize_filesize,
            cx.listener(|this, _, window, cx| {
                if let Some(ui) = this.export.as_mut() {
                    ui.optimize_filesize = !ui.optimize_filesize;
                    ui.persist();
                }
                this.refresh_export_preview(window, cx);
                cx.notify();
            }),
        )
    }

    fn export_advanced(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        let show_bpp = ui.format == ExportFormatKind::Mp4 && !ui.cursor_only;
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .id("export-advanced-toggle").tab_index(0)
                    .role(gpui::Role::Button)
                    .aria_label(t!("Advanced"))
                    .aria_expanded(ui.advanced_open)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .h(px(30.))
                    .mx(px(-6.))
                    .px(px(6.))
                    .rounded(px(8.))
                    .tab_index(0).cursor_pointer()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(Hsla::from(editor.text_2))
                    .hover(move |style| {
                        style
                            .bg(Hsla::from(editor.ctl))
                            .text_color(Hsla::from(editor.text_1))
                    })
                    .on_click(cx.listener(|this, _, _window, cx| {
                        if let Some(ui) = this.export.as_mut() {
                            ui.advanced_open = !ui.advanced_open;
                            ui.persist();
                        }
                        cx.notify();
                    }))
                    .child(
                        svg()
                            .path("icons/sliders-horizontal.svg")
                            .size(px(14.))
                            .text_color(Hsla::from(editor.text_2)),
                    )
                    .child(t!("Advanced"))
                    .child(
                        svg()
                            .path(if ui.advanced_open {
                                "icons/chevron-up.svg"
                            } else {
                                "icons/chevron-down.svg"
                            })
                            .size(px(14.))
                            .ml_auto()
                            .text_color(Hsla::from(editor.text_3)),
                    ),
            )
            .when(ui.advanced_open, |this| {
                this.child(self.export_toggle_row(
                    "export-cursor-only",
                    t!("Export cursor only"),
                    Some(t!("Keeps the same cursor motion and clicks on a transparent background")),
                    ui.cursor_only,
                    cx.listener(|this, _, window, cx| {
                        if let Some(ui) = this.export.as_mut() {
                            ui.cursor_only = !ui.cursor_only;
                        }
                        this.normalize_loaded_export_options();
                        if let Some(ui) = this.export.as_mut() {
                            ui.persist();
                        }
                        this.refresh_export_preview(window, cx);
                        cx.notify();
                    }),
                ))
                .when(ui.cursor_only, |this| {
                    this.child(
                        div()
                            .mt(px(4.))
                            .p(px(12.))
                            .rounded(px(10.))
                            .bg(Hsla::from(editor.card_2))
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(8.))
                            .child(
                                svg()
                                    .path("icons/triangle-alert.svg")
                                    .size(px(14.))
                                    .flex_shrink_0()
                                    .mt(px(1.))
                                    .text_color(Hsla::from(editor.text_2)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .line_height(px(15.))
                                    .text_color(Hsla::from(editor.text_2))
                                    .child(
                                        t!("Exports as a transparent MOV. Files are large and best for compositing or editing."),
                                    ),
                            ),
                    )
                })
                .when(show_bpp, |this| {
                    let fraction = ((ui.bpp() - BPP_MIN) / (BPP_MAX - BPP_MIN)).clamp(0., 1.);
                    this.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .h(px(34.))
                            .gap(px(10.))
                            .child(
                                div()
                                    .flex_none()
                                    .min_w(px(96.))
                                    .text_size(px(13.))
                                    .text_color(Hsla::from(editor.text_1))
                                    .child(t!("Bits per pixel")),
                            )
                            .child(
                                div()
                                    .id("export-bpp-row").tab_index(0)
                                    .focus_visible(|style| style.bg(theme.selection))
                                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                                        let Some(ui) = this.export.as_ref() else { return; };
                                        if let Some(value) = ui::keyboard_slider_value(event.keystroke.key.as_str(), ui.bpp(), BPP_MIN, BPP_MAX, BPP_STEP) {
                                            window.prevent_default(); cx.stop_propagation();
                                            this.set_export_bpp(value, cx);
                                            if let Some(ui) = this.export.as_mut() { ui.persist(); }
                                            this.refresh_export_preview(window, cx);
                                        }
                                    }))
                                    .flex_1()
                                    .min_w_0()
                                    .px(px(4.))
                                    .h(px(32.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .child(
                                        ui::Slider::new(
                                            "export-bpp",
                                            fraction,
                                            ui.bpp_track.clone(),
                                        )
                                        .flex()
                                        .row_height(px(28.))
                                        .track(px(3.), Hsla::from(editor.ctl_active))
                                        .fill(Hsla::from(editor.accent))
                                        .thumb(
                                            px(14.),
                                            Hsla::from(editor.thumb),
                                            Some(gpui::hsla(0., 0., 0., 0.12)),
                                        )
                                        .on_drag_start(cx.listener(
                                            |this, event: &MouseDownEvent, _window, cx| {
                                                this.export_bpp_mouse_down(event, cx);
                                            },
                                        )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .min_w(px(36.))
                                    .text_right()
                                    .text_size(px(11.))
                                    .text_color(Hsla::from(editor.text_3))
                                    .child(format!("{:.2}", ui.bpp())),
                            ),
                    )
                    .when(ui.is_custom_bpp(), |this| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(Hsla::from(editor.text_3))
                                .child(t!("Using a custom bitrate")),
                        )
                    })
                })
                .when(cfg!(target_os = "macos") && show_bpp, |this| {
                    this.child(self.export_toggle_row(
                        "export-force-ffmpeg",
                        t!("Force FFmpeg decoder"),
                        Some(t!("Skip hardware decoder (auto-fallback enabled)")),
                        ui.force_ffmpeg,
                        cx.listener(|this, _, window, cx| {
                            if let Some(ui) = this.export.as_mut() {
                                ui.force_ffmpeg = !ui.force_ffmpeg;
                                ui.persist();
                            }
                            this.refresh_export_preview(window, cx);
                            cx.notify();
                        }),
                    ))
                })
            })
    }

    fn export_bpp_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if let Some(ui) = self.export.as_mut() {
            ui.bpp_dragging = true;
        }
        self.export_bpp_drag_to(event.position.x, cx);
        cx.notify();
    }

    fn export_bpp_drag_to(&mut self, x: gpui::Pixels, cx: &mut Context<Self>) {
        let Some(ui) = self.export.as_mut() else {
            return;
        };
        let Some(bounds) = ui.bpp_track.get() else {
            return;
        };
        let Some(fraction) = ui::fraction_from_x(x, bounds) else {
            return;
        };
        let value = ui::snap_to_step(
            ui::value_from_fraction(fraction, BPP_MIN, BPP_MAX),
            BPP_MIN,
            BPP_MAX,
            BPP_STEP,
        );
        self.set_export_bpp(value, cx);
    }

    fn set_export_bpp(&mut self, value: f32, cx: &mut Context<Self>) {
        let Some(ui) = self.export.as_mut() else {
            return;
        };
        if (value - ui.bpp()).abs() < f32::EPSILON {
            return;
        }
        ui.custom_bpp = Some(value);
        if let Some((preset, _)) = COMPRESSION_PRESETS
            .iter()
            .find(|(preset, _)| (preset.bits_per_pixel() - value).abs() < 0.001)
        {
            ui.compression = *preset;
        }
        cx.notify();
    }

    fn export_bpp_mouse_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.export.as_mut() else {
            return;
        };
        if !ui.bpp_dragging {
            return;
        }
        ui.bpp_dragging = false;
        ui.persist();
        self.refresh_export_preview(window, cx);
        cx.notify();
    }

    fn render_export_overlay(&self, ui: &ExportUi, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let editor = theme.editor;
        let fraction = if ui.total_frames == 0 {
            None
        } else {
            Some(ui.rendered as f32 / ui.total_frames as f32)
        };
        let heading = match ui.phase {
            ExportPhase::Starting => t!("Preparing export"),
            ExportPhase::Rendering if ui.is_tracks() => t!("Exporting tracks"),
            ExportPhase::Rendering if ui.format == ExportFormatKind::Audio => {
                t!("Exporting audio")
            }
            ExportPhase::Rendering if ui.cursor_only => t!("Rendering cursor track"),
            ExportPhase::Rendering if ui.format == ExportFormatKind::Gif => t!("Rendering GIF"),
            ExportPhase::Rendering => t!("Rendering video"),
            ExportPhase::Copying if ui.destination == ExportDestination::Clipboard => {
                t!("Copying to clipboard")
            }
            ExportPhase::Copying => t!("Saving to file"),
            ExportPhase::Done if !ui.saves_to_file() => t!("Copied to clipboard"),
            ExportPhase::Done => t!("Export complete"),
            ExportPhase::Failed if ui.clipboard_retry_path().is_some() => {
                t!("Export complete; clipboard copy failed")
            }
            ExportPhase::Failed => t!("Couldn't finish the export"),
            ExportPhase::Idle | ExportPhase::ChoosingFile => "",
        };

        div()
            .id("export-state")
            .size_full()
            .overflow_y_scroll()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .p(px(24.))
            .bg(theme.card)
            .text_color(Hsla::from(editor.text_1))
            .when(ui.phase == ExportPhase::Done, |this| {
                this.child(div().w_full().h(px(4.)).bg(theme.primary))
                    .child(crate::doodle::doodle_once(
                        &crate::onboarding_doodles::ALL_SET,
                        crate::doodle::Palette::new(&theme),
                        ui.finished_at.unwrap_or_else(std::time::Instant::now),
                        cx.reduce_motion(),
                        72.,
                    ))
            })
            .when(ui.phase.is_busy(), |this| this.child({
                let ring = ui::CircularProgress::new(
                    px(80.),
                    px(6.),
                    Hsla::from(editor.ctl_active),
                    Hsla::from(editor.accent),
                )
                .label(Hsla::from(editor.text_1), px(14.));
                if let Some(fraction) = fraction {
                    ring.progress(fraction)
                } else {
                    ring.indeterminate()
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(heading),
                    ),
            )
            .when(ui.phase == ExportPhase::Rendering && ui.total_frames > 0 && ui.renders_video(), |this| {
                this.child(
                    div()
                        .text_size(px(12.))
                        .text_color(Hsla::from(editor.text_2))
                        .child(t!(
                            "{rendered} / {total} frames",
                            rendered = ui.rendered,
                            total = ui.total_frames
                        )),
                )
            })
            .when(ui.phase == ExportPhase::Done, |this| {
                this.children(ui.output_path.as_ref().map(|path| {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    let text = match std::fs::metadata(path) {
                        _ if ui.is_tracks() => {
                            t!("{name} · {count} files", name = name, count = ui.tracks.len())
                        }
                        Ok(meta) => format!("{name} · {:.1} MB", meta.len() as f64 / 1_000_000.),
                        Err(_) => name.into_owned(),
                    };
                    div().text_size(px(12.)).child(text)
                }))
            })
            .when(ui.phase == ExportPhase::Failed, |this| {
                this.child(div().text_size(px(13.)).child(if ui.clipboard_retry_path().is_some() {
                    t!("Your exported file is kept. Retry copying or open the file.")
                } else {
                    t!("Your recording is kept. Check the export location and free disk space, then try again. See Details for the cause.")
                }))
                .child(ui::Button::plain(&theme, "export-details", ui::ButtonVariant::Gray, ui::ButtonSize::Sm)
                    .label(t!("Details"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(ui) = this.export.as_mut() { ui.details_open = !ui.details_open; }
                        cx.notify();
                    })))
                .when(ui.details_open, |this| this.child(
                    div().w_full().text_size(px(12.)).text_color(theme.muted_foreground)
                        .child(ui.error.clone().unwrap_or_default())
                ))
                .child(ui::Button::plain(&theme, "export-logs", ui::ButtonVariant::Gray, ui::ButtonSize::Sm)
                    .label(t!("Export logs"))
                    .on_click(cx.listener(|_, _, window, cx| {
                        cx.spawn_in(window, async move |_, cx| {
                            let Some(path) = platform::save_file_panel_async("frameever-export-logs.txt", &["txt"], cx).await else { return; };
                            let result = cx.background_executor().spawn(async move {
                                crate::diagnostics::export_logs(&path).map(|()| path)
                            }).await;
                            match result {
                                Ok(path) => { cx.update(|_, cx| library::reveal_path(&path, cx)).ok(); }
                                Err(error) => { tracing::error!(error, "could not export logs"); platform::alert_dialog(t!("Couldn't export logs"), t!("Choose a writable folder and try again.")); }
                            }
                        }).detach();
                    })))
                .child(ui::Button::plain(&theme, "export-back-options", ui::ButtonVariant::Gray, ui::ButtonSize::Md)
                    .label(t!("Back to export"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(ui) = this.export.as_mut() { ui.phase = ExportPhase::Idle; ui.details_open = false; }
                        cx.notify();
                    })))
            })
            .when(ui.phase.is_busy(), |this| {
                this.child(
                    ui::Button::plain(
                        &theme,
                        "export-cancel",
                        ui::ButtonVariant::Gray,
                        ui::ButtonSize::Md,
                    )
                    .label(t!("Cancel"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let Some(ui) = this.export.as_mut() else {
                            return;
                        };
                        let cancel = ui.cancel.clone();
                        cx.spawn_in(window, async move |this, cx| {
                            let confirmed = platform::confirm_dialog(
                                t!("Cancel export?"),
                                t!("The frames rendered so far are discarded."),
                                t!("Cancel export"),
                                t!("Keep exporting"),
                                true,
                            );
                            let _ = this.update(cx, |this, cx| {
                                if let Some(ui) = this.export.as_mut() {
                                    cancel_matching_export(&ui.cancel, &cancel, confirmed);
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
                )
                .child(
                    div()
                        .max_w(px(320.))
                        .text_size(px(12.))
                        .text_color(Hsla::from(editor.text_2))
                        .text_center()
                        .child(
                            t!("Use Instant Mode for your next recording if you want a ready-to-play file the moment you stop."),
                        ),
                )
            })
            .when(ui.phase == ExportPhase::Done || ui.phase == ExportPhase::Failed, |this| {
                this.when(ui.clipboard_retry_path().is_some(), |this| {
                    this.child(
                        ui::Button::plain(
                            &theme,
                            "export-retry-copy",
                            ui::ButtonVariant::Gray,
                            ui::ButtonSize::Md,
                        )
                        .icon("icons/copy.svg")
                        .label(t!("Retry Copy"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.retry_clipboard_copy(window, cx);
                        })),
                    )
                })
                .when(
                    (ui.phase == ExportPhase::Done && ui.saves_to_file())
                        || ui.clipboard_retry_path().is_some(),
                    |this| {
                        let path = ui.output_path.clone();
                        this.child(
                            ui::Button::plain(
                                &theme,
                                "export-open-file",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Md,
                            )
                            .label(if ui.is_tracks() {
                                t!("Open folder")
                            } else {
                                t!("Open file")
                            })
                            .on_click(move |_, _, cx| {
                                if let Some(path) = &path {
                                    library::open_with_system(path, cx);
                                }
                            }),
                        )
                    },
                )
                .child(
                    ui::Button::plain(
                        &theme,
                        "export-back-done",
                        ui::ButtonVariant::Gray,
                        ui::ButtonSize::Md,
                    )
                    .label(t!("Back to editor"))
                    .on_click(cx.listener(|this, _, window, cx| this.close_export(window, cx))),
                )
            })
    }
}

/// The largest box of the preview's aspect that fits the stage, so the frame's
/// rounded corners and shadow hug the picture instead of a letterboxed pane.
fn preview_fit(avail_w: f32, avail_h: f32, image: Option<(f32, f32)>) -> (f32, f32) {
    let avail_w = avail_w.max(120.);
    let avail_h = avail_h.max(68.);
    let (iw, ih) = match image {
        Some((w, h)) if w > 0. && h > 0. => (w, h),
        _ => (16., 9.),
    };
    let scale = (avail_w / iw).min(avail_h / ih);
    ((iw * scale).floor(), (ih * scale).floor())
}

async fn confirm_export_resources(
    this: &gpui::WeakEntity<EditorWindow>,
    cx: &mut gpui::AsyncWindowContext,
    resources: &ExportResources,
    cancel: &Arc<AtomicBool>,
) -> bool {
    let check = {
        let resources = resources.clone();
        gpui_tokio::Tokio::spawn(cx, async move { resources.check().await }).await
    };
    let result = async {
        let warning = check.map_err(|error| error.to_string())??;
        if let Some(message) = warning {
            let confirmed = {
                let current = this
                    .update(cx, |this, _| {
                        this.export.as_ref().is_some_and(|ui| {
                            Arc::ptr_eq(&ui.cancel, cancel)
                                && !ui.close_requested
                                && !cancel.load(Ordering::Acquire)
                        })
                    })
                    .unwrap_or(false);
                if !current {
                    return Ok(false);
                }
                platform::confirm_dialog(
                    t!("Check export resources"),
                    &message,
                    t!("Proceed anyway"),
                    t!("Go back"),
                    false,
                )
            };
            if !confirmed {
                return Ok(false);
            }
            let resources = resources.clone();
            let _ = gpui_tokio::Tokio::spawn(cx, async move { resources.check().await })
                .await
                .map_err(|error| error.to_string())??;
        }
        Ok::<bool, String>(true)
    }
    .await;
    this.update(cx, |this, cx| {
        let Some(ui) = this.export.as_mut() else {
            return false;
        };
        if !Arc::ptr_eq(&ui.cancel, cancel) {
            return false;
        }
        let accepted = result.as_ref().is_ok_and(|accepted| *accepted)
            && !ui.close_requested
            && !cancel.load(Ordering::Acquire);
        if !accepted {
            match result {
                Err(error) if !ui.close_requested && !cancel.load(Ordering::Acquire) => {
                    tracing::error!(error, "export stopped before it started");
                    ui.phase = ExportPhase::Failed;
                    ui.error = Some(error);
                }
                _ => ui.phase = ExportPhase::Idle,
            }
            cx.notify();
        }
        accepted
    })
    .unwrap_or(false)
}

fn cancel_matching_export(current: &Arc<AtomicBool>, requested: &Arc<AtomicBool>, confirmed: bool) {
    if confirmed && Arc::ptr_eq(current, requested) {
        current.store(true, Ordering::Relaxed);
    }
}

fn export_stat(
    theme: &crate::theme::Theme,
    label: &'static str,
    value: Option<String>,
    divided: bool,
) -> impl IntoElement {
    let editor = theme.editor;
    div()
        .flex()
        .flex_1()
        .flex_col()
        .justify_center()
        .px(px(16.))
        .gap(px(1.))
        .when(divided, |this| {
            this.border_l_1().border_color(Hsla::from(editor.line))
        })
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Hsla::from(editor.text_3))
                .child(label),
        )
        .child(match value {
            Some(value) => div()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Hsla::from(editor.text_1))
                .whitespace_nowrap()
                .child(value)
                .into_any_element(),
            None => div()
                .my(px(3.))
                .h(px(12.))
                .w(px(56.))
                .rounded(px(4.))
                .bg(Hsla::from(editor.ctl_active))
                .into_any_element(),
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportPicture {
    Recorded,
    None,
    Visuals,
}

#[derive(Debug, Clone, Copy)]
struct FormatAvailability {
    transparent: bool,
    cursor_only: bool,
    no_tracks: bool,
    no_audio: bool,
    picture: ExportPicture,
}

impl FormatAvailability {
    fn disabled(self, format: ExportFormatKind) -> bool {
        match (format, self.picture) {
            (ExportFormatKind::Audio, _) => self.no_audio,
            (_, ExportPicture::None) | (ExportFormatKind::Tracks, ExportPicture::Visuals) => true,
            (ExportFormatKind::Mp4, _) => self.transparent || self.cursor_only,
            (ExportFormatKind::Gif, _) => self.cursor_only,
            (ExportFormatKind::Tracks, _) => self.no_tracks,
        }
    }
}

fn export_fps_options(format: ExportFormatKind, cursor_only: bool) -> &'static [u32] {
    if format == ExportFormatKind::Gif && !cursor_only {
        &[5, 10, 15, 20, 25, 30]
    } else {
        &[15, 30, 60]
    }
}

fn export_resolutions(
    format: ExportFormatKind,
    cursor_only: bool,
    transparent: bool,
) -> &'static [ExportResolution] {
    if cursor_only {
        &[ExportResolution::P720, ExportResolution::P1080]
    } else if format == ExportFormatKind::Gif {
        &[
            ExportResolution::P480,
            ExportResolution::P720,
            ExportResolution::P1080,
        ]
    } else if transparent {
        &[ExportResolution::P720, ExportResolution::P1080]
    } else {
        &[
            ExportResolution::P720,
            ExportResolution::P1080,
            ExportResolution::P4k,
        ]
    }
}

fn normalize_export_options(ui: &mut ExportUi, transparent: bool) {
    let fps_options = export_fps_options(ui.format, ui.cursor_only);
    if ui.fps_from_source {
        ui.fps = source_fps(ui.source.fps, fps_options);
    }
    let resolutions = export_resolutions(ui.format, ui.cursor_only, transparent);
    if ui.resolution_from_source {
        let largest = if ui.format == ExportFormatKind::Gif && !ui.cursor_only {
            ExportResolution::P720
        } else {
            ExportResolution::P1080
        };
        ui.resolution = source_resolution(ui.source.size, resolutions, largest);
    }
    if !fps_options.contains(&ui.fps) {
        ui.fps = if ui.format == ExportFormatKind::Gif {
            15
        } else {
            30
        };
    }
    if !resolutions.contains(&ui.resolution) {
        ui.resolution = if ui.resolution == ExportResolution::P4k {
            ExportResolution::P1080
        } else {
            ExportResolution::P720
        };
    }
}

fn capture_fps(probed: u32, recorded: Option<u32>) -> u32 {
    probed.max(recorded.unwrap_or(0))
}

fn highest_recorded_fps(
    segments: impl IntoIterator<Item = (u32, Option<u32>)>,
    camera_visible: bool,
) -> Option<u32> {
    segments
        .into_iter()
        .flat_map(|(display, camera)| [Some(display), camera.filter(|_| camera_visible)])
        .flatten()
        .max()
        .filter(|fps| *fps > 0)
}

fn source_resolution(
    source: Option<XY<u32>>,
    options: &[ExportResolution],
    largest: ExportResolution,
) -> ExportResolution {
    let allowed = || options.iter().copied().filter(|res| *res <= largest);
    let Some(smallest) = allowed().min() else {
        return largest;
    };
    let Some(source) = source.filter(|size| size.x > 0 && size.y > 0) else {
        return allowed().max().unwrap_or(smallest);
    };
    allowed()
        .filter(|res| {
            let (width, height) = res.size();
            let scale = (width as f64 / source.x as f64).min(height as f64 / source.y as f64);
            scale <= 1.0
        })
        .max()
        .unwrap_or(smallest)
}

fn source_fps(source: Option<u32>, options: &[u32]) -> u32 {
    let highest = options.iter().copied().max().unwrap_or(30);
    let Some(source) = source else {
        return highest;
    };
    options
        .iter()
        .copied()
        .filter(|fps| u64::from(*fps) * 10 >= u64::from(source) * 9)
        .min()
        .unwrap_or(highest)
}

fn matches_compression(current: ExportCompression, expected: ExportCompression) -> bool {
    std::mem::discriminant(&current) == std::mem::discriminant(&expected)
}

pub(crate) async fn render_video(
    project_path: PathBuf,
    project: frameever_project::ProjectConfiguration,
    settings: VideoSettings,
    range: Option<ExportRange>,
    save_path: Option<PathBuf>,
    progress_tx: flume::Sender<(u32, u32)>,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    let VideoSettings {
        format,
        cursor_only,
        fps,
        size: (width, height),
        compression,
        codec,
        custom_bpp,
        gif_quality,
        optimize,
        force_ffmpeg: force,
    } = settings;
    let gif_quality = gif_quality.quality();
    if cancel.load(Ordering::Relaxed) {
        return Err("Export cancelled".into());
    }
    frameever_export::validate_output_path(&project_path)
        .map_err(|_| t!("Export output must be a local file path").to_string())?;
    if let Some(path) = &save_path {
        frameever_export::validate_output_path(path)
            .map_err(|_| t!("Export output must be a local file path").to_string())?;
    }
    let mut builder = ExporterBase::builder(project_path)
        .with_force_ffmpeg_decoder(force)
        .with_range(range);
    if cursor_only {
        builder = builder.with_config(make_cursor_only_project(project));
    } else {
        builder = builder.with_config(project);
    }
    if let Some(path) = save_path.clone() {
        builder = builder.with_output_path(path);
    }

    enum PreparedBase {
        Mp4(frameever_export::Mp4ExporterBase),
        Other(ExporterBase),
    }
    let (base, total) = if !cursor_only && format != ExportFormatKind::Gif {
        let base = builder
            .build_for_mp4(cancel.clone())
            .await
            .map_err(|error| error.to_string())?;
        let total = base.total_frames(fps);
        (PreparedBase::Mp4(base), total)
    } else {
        let base = builder
            .build_video_only()
            .await
            .map_err(|error| error.to_string())?;
        let total = base.total_frames(fps);
        (PreparedBase::Other(base), total)
    };
    let _ = progress_tx.send((0, total));

    let progress = {
        let progress_tx = progress_tx.clone();
        let cancel = cancel.clone();
        move |frame_index: u32| {
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            progress_tx
                .send(((frame_index + 1).min(total), total))
                .is_ok()
        }
    };

    let resolution = XY::new(width, height);
    match base {
        PreparedBase::Other(base) if cursor_only => {
            MovExportSettings {
                fps,
                resolution_base: resolution,
                cursor_only: true,
            }
            .export(base, progress)
            .await
        }
        PreparedBase::Other(base) => {
            GifExportSettings {
                fps,
                resolution_base: resolution,
                quality: Some(gif_quality),
            }
            .export(base, progress)
            .await
        }
        PreparedBase::Mp4(base) => {
            Mp4ExportSettings {
                codec,
                fps,
                resolution_base: resolution,
                compression,
                custom_bpp,
                force_ffmpeg_decoder: force,
                optimize_filesize: optimize,
            }
            .export_prepared(base, progress)
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExportClip, ExportDestination, ExportFormatKind, ExportPhase, ExportPicture,
        ExportResolution, ExportSource, ExportTrack, ExportUi, FormatAvailability, TrackOption,
        cancel_matching_export, capture_fps, estimated_tracks_bytes, export_fps_options,
        export_resolutions, highest_recorded_fps, last_video_settings, normalize_export_options,
        preview_time_in_range, selected_clip, selected_tracks,
    };
    use crate::editor_edits::Selection;
    use crate::editor_timeline::TrackKind;
    use crate::store::ExportPrefs;
    use frameever_export::ExportRange;
    use frameever_export::audio::AudioExportFormat;
    use frameever_export::gif::{GifExportSettings, GifQualityPreset};
    use frameever_export::mp4::{ExportCodec, ExportCompression};
    use frameever_export::settings::ExportSettings;
    use frameever_project::XY;
    use std::path::PathBuf;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn export_output_paths_refuse_protocols_and_network_shares() {
        for path in [
            "rtmp://example.com/live",
            "http://example.com/output.mp4",
            "https://example.com/output.mp4",
            "udp:example.com:1234",
            "concat:one.mp4|two.mp4",
            "file:output.mp4",
            "pipe:1",
            r"\\server\share\output.mp4",
            "//server/share/output.mp4",
            r"/\server\share\output.mp4",
            r"\/server/share/output.mp4",
            r"\\?\UNC\server\share\output.mp4",
            r"\\.\pipe\output.mp4",
        ] {
            assert!(
                frameever_export::validate_output_path(std::path::Path::new(path)).is_err(),
                "accepted non-local export output: {path}"
            );
        }
    }

    #[test]
    fn export_output_paths_keep_local_relative_and_drive_paths() {
        for path in [
            "output.mp4",
            "exports/output.mp4",
            "./http:output.mp4",
            "/tmp/output.mp4",
            r"C:\Exports\output.mp4",
            "D:/Exports/output.mp4",
            "C:output.mp4",
            r"\\?\C:\Exports\output.mp4",
            "//?/C:/Exports/output.mp4",
            r"\\?\C:\",
            "//?/C:/",
        ] {
            assert!(
                frameever_export::validate_output_path(std::path::Path::new(path)).is_ok(),
                "rejected local export output: {path}"
            );
        }
    }

    fn clipboard_export() -> ExportUi {
        ExportUi::from_preferences(prefs())
    }

    fn prefs() -> ExportPrefs {
        ExportPrefs {
            format: "Mp4".into(),
            fps: 30,
            export_to: "clipboard".into(),
            resolution: "720p".into(),
            compression: "Maximum".into(),
            codec: None,
            optimize_filesize: false,
            cursor_only: false,
            custom_bpp: None,
            force_ffmpeg_decoder: false,
            advanced_open: false,
            organization_id: None,
            gif_quality: None,
            resolution_from_source: false,
            fps_from_source: false,
            excluded_tracks: None,
            audio_format: None,
        }
    }

    fn options(kinds: &[ExportTrack]) -> Vec<TrackOption> {
        kinds
            .iter()
            .map(|kind| TrackOption {
                kind: *kind,
                size: kind.is_video().then_some((1920, 1080)),
            })
            .collect()
    }

    #[test]
    fn the_card_export_renders_with_the_last_video_settings() {
        let settings = last_video_settings(
            Some(ExportPrefs {
                codec: Some("Hevc".into()),
                compression: "Social".into(),
                optimize_filesize: true,
                force_ffmpeg_decoder: true,
                ..prefs()
            }),
            ExportSource::default(),
            false,
        );
        assert_eq!(settings.format, ExportFormatKind::Mp4);
        assert_eq!(settings.codec, ExportCodec::Hevc);
        assert!(matches!(settings.compression, ExportCompression::Social));
        assert_eq!((settings.fps, settings.size), (30, (1280, 720)));
        assert!(settings.optimize && settings.force_ffmpeg);
        assert_eq!(settings.extension(), "mp4");
        assert_eq!(settings.recorded_codec().as_deref(), Some("Hevc"));
    }

    #[test]
    fn the_card_export_keeps_a_gif_and_saves_everything_else_as_one_video_file() {
        let gif = last_video_settings(
            Some(ExportPrefs {
                format: "Gif".into(),
                fps: 15,
                gif_quality: Some("Low".into()),
                ..prefs()
            }),
            ExportSource::default(),
            false,
        );
        assert_eq!(gif.format, ExportFormatKind::Gif);
        assert_eq!(gif.gif_quality, GifQualityPreset::Low);
        assert_eq!(gif.extension(), "gif");
        assert_eq!(gif.recorded_codec(), None);

        let tracks = last_video_settings(
            Some(ExportPrefs {
                format: "Tracks".into(),
                cursor_only: true,
                ..prefs()
            }),
            ExportSource::default(),
            false,
        );
        assert_eq!(tracks.format, ExportFormatKind::Mp4);
        assert!(!tracks.cursor_only);
        assert_eq!(tracks.extension(), "mp4");
    }

    #[test]
    fn the_card_export_follows_the_recording_when_the_last_export_did() {
        let source = ExportSource {
            size: Some(XY::new(1280, 720)),
            fps: Some(30),
        };
        let followed = last_video_settings(
            Some(ExportPrefs {
                fps: 60,
                resolution: "1080p".into(),
                resolution_from_source: true,
                fps_from_source: true,
                ..prefs()
            }),
            source,
            false,
        );
        assert_eq!((followed.fps, followed.size), (30, (1280, 720)));

        let transparent = last_video_settings(Some(prefs()), source, true);
        assert_eq!(transparent.format, ExportFormatKind::Gif);
        assert_eq!(transparent.extension(), "gif");

        let first_export = last_video_settings(None, source, false);
        assert_eq!(first_export.format, ExportFormatKind::Mp4);
        assert_eq!((first_export.fps, first_export.size), (30, (1280, 720)));
    }

    #[test]
    fn every_available_track_starts_on_unless_turned_off_before() {
        let all = options(&ExportTrack::ALL);
        assert_eq!(selected_tracks(&all, &[]), ExportTrack::ALL);
        assert_eq!(
            selected_tracks(&all, &[ExportTrack::Camera, ExportTrack::SystemAudio]),
            [ExportTrack::Screen, ExportTrack::Microphone]
        );
        let screen_only = options(&[ExportTrack::Screen]);
        assert_eq!(
            selected_tracks(&screen_only, &[ExportTrack::Screen]),
            [ExportTrack::Screen]
        );
        assert!(selected_tracks(&[], &[]).is_empty());
    }

    #[test]
    fn turning_tracks_off_is_remembered_and_turning_them_on_forgets_it() {
        let mut ui = clipboard_export();
        ui.set_track_options(options(&[ExportTrack::Screen, ExportTrack::Microphone]));
        ui.toggle_track(ExportTrack::Microphone);
        assert_eq!(ui.tracks, [ExportTrack::Screen]);
        ui.toggle_track(ExportTrack::Camera);
        assert_eq!(ui.tracks, [ExportTrack::Screen]);
        assert_eq!(
            ui.preferences().excluded_tracks,
            Some(vec!["microphone".to_string()])
        );

        let mut reopened = ExportUi::from_preferences(ExportPrefs {
            excluded_tracks: Some(vec!["microphone".into(), "unknown".into()]),
            ..prefs()
        });
        reopened.set_track_options(options(&ExportTrack::ALL));
        assert_eq!(
            reopened.tracks,
            [
                ExportTrack::Screen,
                ExportTrack::Camera,
                ExportTrack::SystemAudio
            ]
        );
        reopened.toggle_track(ExportTrack::Microphone);
        assert_eq!(reopened.tracks, ExportTrack::ALL);
        assert_eq!(reopened.preferences().excluded_tracks, None);
    }

    #[test]
    fn a_tracks_export_needs_recorded_tracks_and_at_least_one_chosen() {
        let mut ui = ExportUi::from_preferences(ExportPrefs {
            format: "Tracks".into(),
            ..prefs()
        });
        ui.set_track_options(Vec::new());
        assert_eq!(ui.format, ExportFormatKind::Mp4);
        ui.use_tracks(None);
        assert_eq!(ui.format, ExportFormatKind::Mp4);
        assert!(format_disabled(
            ExportFormatKind::Tracks,
            false,
            false,
            true
        ));

        ui.set_track_options(options(&ExportTrack::ALL));
        ui.cursor_only = true;
        ui.use_tracks(Some("mic, screen,cursor"));
        assert_eq!(ui.format, ExportFormatKind::Tracks);
        assert!(!ui.cursor_only);
        assert_eq!(ui.tracks, [ExportTrack::Screen, ExportTrack::Microphone]);
        assert!(ui.can_start());

        ui.toggle_track(ExportTrack::Screen);
        ui.toggle_track(ExportTrack::Microphone);
        assert!(!ui.can_start());
        assert_eq!(
            ExportFormatKind::from_slug(ExportFormatKind::Tracks.slug()),
            ExportFormatKind::Tracks
        );
    }

    #[test]
    fn tracks_go_to_a_folder_whatever_destination_was_remembered() {
        let mut ui = clipboard_export();
        ui.offer_clip(Some(intro()), true);
        assert!(!ui.saves_to_file());
        assert!(ui.range().is_some());

        ui.set_track_options(options(&ExportTrack::ALL));
        ui.use_tracks(None);
        assert!(ui.saves_to_file());
        assert!(ui.range().is_none());
        assert!(ui.range_labels().is_none());
        assert_eq!(ui.destination, ExportDestination::Clipboard);

        ui.phase = ExportPhase::Failed;
        ui.output_path = Some(PathBuf::from("tracks"));
        assert_eq!(ui.clipboard_retry_path(), None);
    }

    #[test]
    fn exporting_one_clip_leaves_a_remembered_tracks_format() {
        let mut ui = ExportUi::from_preferences(ExportPrefs {
            format: "Tracks".into(),
            ..prefs()
        });
        ui.offer_clip(Some(intro()), true);
        ui.set_track_options(options(&ExportTrack::ALL));
        assert_eq!(ui.format, ExportFormatKind::Mp4);
        assert!(ui.range().is_some());

        let mut whole = ExportUi::from_preferences(ExportPrefs {
            format: "Tracks".into(),
            ..prefs()
        });
        whole.offer_clip(Some(intro()), false);
        whole.set_track_options(options(&ExportTrack::ALL));
        assert_eq!(whole.format, ExportFormatKind::Tracks);
    }

    #[test]
    fn track_size_estimates_count_only_selected_tracks() {
        let all = options(&ExportTrack::ALL);
        assert_eq!(estimated_tracks_bytes(&all, &[], 30, 60.0), 0);
        let audio = estimated_tracks_bytes(&all, &[ExportTrack::Microphone], 30, 60.0);
        assert_eq!(audio, 48_000 * 2 * 3 * 60);
        let screen = estimated_tracks_bytes(&all, &[ExportTrack::Screen], 30, 60.0);
        assert!(screen > audio);
        assert_eq!(
            estimated_tracks_bytes(
                &all,
                &[ExportTrack::Screen, ExportTrack::Microphone],
                30,
                60.0
            ),
            screen + audio
        );
    }

    fn format_disabled(
        format: ExportFormatKind,
        transparent: bool,
        cursor_only: bool,
        no_tracks: bool,
    ) -> bool {
        FormatAvailability {
            transparent,
            cursor_only,
            no_tracks,
            no_audio: false,
            picture: ExportPicture::Recorded,
        }
        .disabled(format)
    }

    #[test]
    fn audio_is_offered_for_any_project_with_sound_and_alone_for_audio_only_ones() {
        let video = FormatAvailability {
            transparent: false,
            cursor_only: false,
            no_tracks: false,
            no_audio: false,
            picture: ExportPicture::Recorded,
        };
        assert!(!video.disabled(ExportFormatKind::Audio));
        assert!(!video.disabled(ExportFormatKind::Mp4));
        let silent = FormatAvailability {
            no_audio: true,
            ..video
        };
        assert!(silent.disabled(ExportFormatKind::Audio));
        let audio_only = FormatAvailability {
            picture: ExportPicture::None,
            ..video
        };
        for format in [
            ExportFormatKind::Mp4,
            ExportFormatKind::Gif,
            ExportFormatKind::Tracks,
        ] {
            assert!(audio_only.disabled(format), "{format:?}");
        }
        assert!(!audio_only.disabled(ExportFormatKind::Audio));
        let visuals = FormatAvailability {
            picture: ExportPicture::Visuals,
            ..video
        };
        assert!(!visuals.disabled(ExportFormatKind::Mp4));
        assert!(!visuals.disabled(ExportFormatKind::Gif));
        assert!(!visuals.disabled(ExportFormatKind::Audio));
        assert!(visuals.disabled(ExportFormatKind::Tracks));
    }

    #[test]
    fn an_audio_only_project_with_visuals_opens_export_on_its_last_video_format() {
        for (saved, expected) in [
            ("Gif", ExportFormatKind::Gif),
            ("Tracks", ExportFormatKind::Mp4),
            ("Audio", ExportFormatKind::Mp4),
        ] {
            let mut ui = ExportUi::from_preferences(ExportPrefs {
                format: saved.into(),
                ..prefs()
            });
            ui.sound = true;
            ui.cursor_only = true;
            ui.clip_only = true;
            ui.set_picture(ExportPicture::Visuals);
            assert_eq!(ui.format, expected, "{saved}");
            assert!(!ui.cursor_only);
            assert!(ui.clip_only, "a clip export stays on the clip");
        }
    }

    #[test]
    fn an_audio_only_project_opens_export_on_audio_and_keeps_the_video_format_for_later() {
        let mut ui = ExportUi::from_preferences(ExportPrefs {
            format: "Gif".into(),
            audio_format: Some("wav".into()),
            ..prefs()
        });
        ui.set_track_options(options(&[ExportTrack::Microphone]));
        ui.sound = true;
        ui.cursor_only = true;
        ui.set_picture(ExportPicture::None);
        assert_eq!(ui.format, ExportFormatKind::Audio);
        assert!(!ui.cursor_only);
        assert!(ui.saves_to_file());
        assert!(ui.range_labels().is_none());
        assert_eq!(ui.audio_format, AudioExportFormat::Wav);
        let reopened = ExportUi::from_preferences(ui.preferences());
        assert_eq!(reopened.format, ExportFormatKind::Gif);
        assert_eq!(reopened.audio_format, AudioExportFormat::Wav);
        let video = last_video_settings(Some(ui.preferences()), ExportSource::default(), false);
        assert_eq!(video.format, ExportFormatKind::Gif);
    }

    #[test]
    fn a_video_project_without_sound_falls_back_from_a_remembered_audio_export() {
        let mut ui = ExportUi::from_preferences(ExportPrefs {
            format: "Audio".into(),
            ..prefs()
        });
        ui.set_track_options(options(&[ExportTrack::Screen]));
        ui.set_picture(ExportPicture::Recorded);
        assert_eq!(ui.format, ExportFormatKind::Mp4);
    }

    #[test]
    fn a_transparent_or_cursor_only_export_still_locks_the_video_formats() {
        assert!(format_disabled(ExportFormatKind::Mp4, true, false, false));
        assert!(!format_disabled(ExportFormatKind::Gif, true, false, false));
        assert!(format_disabled(ExportFormatKind::Mp4, false, true, false));
        assert!(format_disabled(ExportFormatKind::Gif, false, true, false));
        assert!(!format_disabled(
            ExportFormatKind::Tracks,
            false,
            true,
            false
        ));
    }

    #[test]
    fn gif_exports_offer_smaller_options_that_other_formats_do_not() {
        assert_eq!(
            export_fps_options(ExportFormatKind::Gif, false),
            [5, 10, 15, 20, 25, 30]
        );
        assert_eq!(
            export_fps_options(ExportFormatKind::Mp4, false),
            [15, 30, 60]
        );
        assert_eq!(
            export_fps_options(ExportFormatKind::Gif, true),
            [15, 30, 60]
        );

        for transparent in [false, true] {
            assert_eq!(
                export_resolutions(ExportFormatKind::Gif, false, transparent),
                [
                    ExportResolution::P480,
                    ExportResolution::P720,
                    ExportResolution::P1080
                ]
            );
            for format in [ExportFormatKind::Gif, ExportFormatKind::Mp4] {
                assert_eq!(
                    export_resolutions(format, true, transparent),
                    [ExportResolution::P720, ExportResolution::P1080]
                );
            }
        }
        assert_eq!(
            export_resolutions(ExportFormatKind::Mp4, false, false),
            [
                ExportResolution::P720,
                ExportResolution::P1080,
                ExportResolution::P4k
            ]
        );
    }

    #[test]
    fn gif_only_options_fall_back_when_the_format_changes() {
        let mut ui = clipboard_export();
        ui.format = ExportFormatKind::Gif;
        ui.resolution = ExportResolution::P480;
        ui.fps = 5;
        normalize_export_options(&mut ui, false);
        assert_eq!((ui.resolution, ui.fps), (ExportResolution::P480, 5));

        ui.format = ExportFormatKind::Mp4;
        normalize_export_options(&mut ui, false);
        assert_eq!((ui.resolution, ui.fps), (ExportResolution::P720, 30));

        ui.format = ExportFormatKind::Gif;
        ui.resolution = ExportResolution::P480;
        ui.fps = 5;
        ui.cursor_only = true;
        normalize_export_options(&mut ui, false);
        assert_eq!((ui.resolution, ui.fps), (ExportResolution::P720, 15));

        ui.cursor_only = false;
        ui.resolution = ExportResolution::P4k;
        ui.fps = 60;
        normalize_export_options(&mut ui, false);
        assert_eq!((ui.resolution, ui.fps), (ExportResolution::P1080, 15));
    }

    fn new_export(width: u32, height: u32, fps: u32) -> ExportUi {
        let mut ui = ExportUi::from_preferences(ExportPrefs {
            export_to: "file".into(),
            resolution: "1080p".into(),
            fps: 60,
            resolution_from_source: true,
            fps_from_source: true,
            ..clipboard_export().preferences()
        });
        ui.source = ExportSource {
            size: Some(XY::new(width, height)),
            fps: Some(fps),
        };
        normalize_export_options(&mut ui, false);
        ui
    }

    fn chosen(ui: &ExportUi) -> (ExportResolution, u32) {
        (ui.resolution, ui.fps)
    }

    #[test]
    fn new_exports_default_to_1080p60_without_upscaling_the_source() {
        let p1080 = ExportResolution::P1080;
        let p720 = ExportResolution::P720;
        for (width, height, fps, expected) in [
            (3840, 2160, 60, (p1080, 60)),
            (4148, 2332, 60, (p1080, 60)),
            (3024, 1964, 60, (p1080, 60)),
            (1920, 1080, 60, (p1080, 60)),
            (2074, 1166, 60, (p1080, 60)),
            (1080, 1920, 60, (p1080, 60)),
            (1920, 1080, 30, (p1080, 30)),
            (1918, 1078, 60, (p720, 60)),
            (1382, 778, 60, (p720, 60)),
            (1280, 720, 60, (p720, 60)),
            (1280, 720, 30, (p720, 30)),
            (800, 600, 30, (p720, 30)),
        ] {
            assert_eq!(
                chosen(&new_export(width, height, fps)),
                expected,
                "{width}x{height} at {fps} fps"
            );
        }
    }

    #[test]
    fn new_exports_keep_every_source_frame_without_inventing_frames() {
        for (source, expected) in [
            (120, 60),
            (60, 60),
            (58, 60),
            (50, 60),
            (34, 60),
            (31, 30),
            (30, 30),
            (29, 30),
            (24, 30),
            (15, 15),
        ] {
            assert_eq!(new_export(1920, 1080, source).fps, expected, "{source} fps");
        }

        let mut unknown = new_export(1920, 1080, 60);
        unknown.source = ExportSource::default();
        normalize_export_options(&mut unknown, false);
        assert_eq!(chosen(&unknown), (ExportResolution::P1080, 60));
    }

    #[test]
    fn a_visible_faster_camera_sets_the_frame_rate() {
        let segments = [(30, Some(60)), (30, None)];
        assert_eq!(highest_recorded_fps(segments, true), Some(60));
        assert_eq!(highest_recorded_fps(segments, false), Some(30));
        assert_eq!(highest_recorded_fps([(0, None)], true), None);
    }

    #[test]
    fn a_still_screen_exports_at_the_rate_it_was_captured_at() {
        assert_eq!(capture_fps(5, Some(60)), 60);
        assert_eq!(new_export(1920, 1200, capture_fps(5, Some(60))).fps, 60);
        assert_eq!(capture_fps(60, Some(30)), 60);
        assert_eq!(capture_fps(24, Some(0)), 24);
        assert_eq!(capture_fps(48, None), 48);
    }

    #[test]
    fn new_gif_exports_keep_their_smaller_defaults() {
        let mut ui = new_export(3840, 2160, 60);
        ui.format = ExportFormatKind::Gif;
        normalize_export_options(&mut ui, false);
        assert_eq!(chosen(&ui), (ExportResolution::P720, 30));

        ui.format = ExportFormatKind::Mp4;
        normalize_export_options(&mut ui, false);
        assert_eq!(chosen(&ui), (ExportResolution::P1080, 60));
    }

    #[test]
    fn a_remembered_choice_wins_over_the_source() {
        let saved_by_1_0_0: ExportPrefs = serde_json::from_str(
            r#"{"format":"Mp4","fps":30,"export_to":"file","resolution":"720p","compression":"Maximum","codec":"H264","optimize_filesize":false,"cursor_only":false,"custom_bpp":null,"force_ffmpeg_decoder":false,"advanced_open":false,"organization_id":null,"gif_quality":"high"}"#,
        )
        .unwrap();
        assert!(!saved_by_1_0_0.resolution_from_source);
        assert!(!saved_by_1_0_0.fps_from_source);
        let mut ui = ExportUi::from_preferences(saved_by_1_0_0);
        ui.source = ExportSource {
            size: Some(XY::new(3840, 2160)),
            fps: Some(60),
        };
        normalize_export_options(&mut ui, false);
        assert_eq!(chosen(&ui), (ExportResolution::P720, 30));

        let mut ui = new_export(1280, 720, 30);
        ui.choose_resolution(ExportResolution::P4k);
        ui.choose_fps(60);
        let mut reopened = ExportUi::from_preferences(ui.preferences());
        reopened.source = ExportSource {
            size: Some(XY::new(1280, 720)),
            fps: Some(30),
        };
        normalize_export_options(&mut reopened, false);
        assert_eq!(chosen(&reopened), (ExportResolution::P4k, 60));
    }

    #[test]
    fn untouched_defaults_follow_each_project() {
        let first = new_export(3840, 2160, 60);
        let mut second = ExportUi::from_preferences(first.preferences());
        second.source = ExportSource {
            size: Some(XY::new(1280, 720)),
            fps: Some(30),
        };
        normalize_export_options(&mut second, false);
        assert_eq!(chosen(&second), (ExportResolution::P720, 30));

        let mut picked_fps_only = new_export(3840, 2160, 60);
        picked_fps_only.choose_fps(30);
        let mut third = ExportUi::from_preferences(picked_fps_only.preferences());
        third.source = ExportSource {
            size: Some(XY::new(1280, 720)),
            fps: Some(60),
        };
        normalize_export_options(&mut third, false);
        assert_eq!(chosen(&third), (ExportResolution::P720, 30));
    }

    #[test]
    fn gif_quality_defaults_to_high_and_is_remembered() {
        let ui = clipboard_export();
        assert_eq!(ui.gif_quality, GifQualityPreset::High);
        assert_eq!(
            GifQualityPreset::from_slug(Some("Sharp")),
            GifQualityPreset::High
        );

        let mut qualities = Vec::new();
        for preset in GifQualityPreset::ALL {
            assert_eq!(GifQualityPreset::from_slug(Some(preset.slug())), preset);
            qualities.push(preset.quality().quality.unwrap());
        }
        assert_eq!(qualities, [50, 70, 90]);
    }

    #[test]
    fn the_chosen_gif_quality_reaches_the_estimate_and_only_a_gif_preview() {
        let mut ui = clipboard_export();
        assert!(ui.gif_preview_quality().is_none());
        ui.format = ExportFormatKind::Gif;
        ui.gif_quality = GifQualityPreset::Low;
        assert_eq!(ui.gif_preview_quality().and_then(|q| q.quality), Some(50));
        assert!(matches!(
            ui.estimate_settings(),
            ExportSettings::Gif(GifExportSettings {
                quality: Some(quality),
                ..
            }) if quality.quality == Some(50)
        ));
        ui.cursor_only = true;
        assert!(ui.gif_preview_quality().is_none());
    }

    fn intro() -> ExportClip {
        ExportClip {
            name: "Intro".into(),
            range: ExportRange {
                start: 2.0,
                end: 5.0,
            },
        }
    }

    #[test]
    fn the_range_is_exported_only_when_the_clip_was_chosen() {
        let mut ui = clipboard_export();
        assert!(ui.range_labels().is_none());
        assert_eq!(ui.range(), None);

        ui.offer_clip(Some(intro()), false);
        assert_eq!(
            ui.range_labels(),
            Some(["Whole video".to_string(), "Intro · 0:03".to_string()])
        );
        assert_eq!(ui.range(), None);

        ui.offer_clip(Some(intro()), true);
        assert_eq!(
            ui.range(),
            Some(ExportRange {
                start: 2.0,
                end: 5.0
            })
        );

        ui.clip_only = false;
        assert_eq!(ui.range(), None);

        ui.offer_clip(None, true);
        assert!(ui.range_labels().is_none());
        assert_eq!(ui.range(), None);
    }

    #[test]
    fn the_duration_describes_the_chosen_range() {
        let mut ui = clipboard_export();
        ui.fps = 30;
        assert_eq!(ui.frames_to_export(300), 300);
        ui.offer_clip(Some(intro()), false);
        assert_eq!(ui.frames_to_export(300), 300);
        ui.offer_clip(Some(intro()), true);
        assert_eq!(ui.frames_to_export(300), 90);
    }

    #[test]
    fn a_long_clip_name_is_shortened_in_the_range_choice() {
        let mut ui = clipboard_export();
        ui.offer_clip(
            Some(ExportClip {
                name: "A walkthrough of the settings page".into(),
                range: ExportRange {
                    start: 0.0,
                    end: 75.0,
                },
            }),
            true,
        );
        assert_eq!(ui.range_labels().unwrap()[1], "A walkthrough o… · 1:15");
    }

    #[test]
    fn the_preview_frame_comes_from_inside_the_chosen_range() {
        let range = Some(ExportRange {
            start: 2.0,
            end: 5.0,
        });
        assert_eq!(preview_time_in_range(3.5, range), 3.5);
        assert_eq!(preview_time_in_range(0.5, range), 2.0);
        assert_eq!(preview_time_in_range(5.0, range), 2.0);
        assert_eq!(preview_time_in_range(9.0, range), 2.0);
        assert_eq!(preview_time_in_range(9.0, None), 9.0);
    }

    #[gpui::test]
    fn opening_the_export_page_clears_the_selection_and_remembers_the_clip(
        cx: &mut gpui::TestAppContext,
    ) {
        let project = tempfile::tempdir().unwrap();
        let path = project.path().to_path_buf();
        let (editor, cx) = cx.add_window_view(move |window, cx| {
            crate::editor_window::EditorWindow::new(path.clone(), window, cx)
        });
        editor.update_in(cx, |editor, window, cx| {
            editor.set_error("export fixture".into(), window, cx);
            editor.project.timeline = Some(
                serde_json::from_value(serde_json::json!({
                    "segments": [
                        { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 10.0 },
                        { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 8.0 }
                    ],
                    "zoomSegments": []
                }))
                .unwrap(),
            );
            editor.timeline =
                crate::editor_timeline::TimelineModel::build(&editor.project, false, false);
            for open in [
                (|editor, window, cx| editor.open_export(window, cx))
                    as fn(
                        &mut crate::editor_window::EditorWindow,
                        &mut gpui::Window,
                        &mut gpui::Context<crate::editor_window::EditorWindow>,
                    ),
                |editor, window, cx| editor.open_clip_export(1, window, cx),
            ] {
                editor.selection = Some(Selection::single(TrackKind::Clip, 1));
                open(editor, window, cx);
                assert_eq!(editor.selection(), None);
                let ui = editor.export.take().unwrap();
                assert_eq!(
                    ui.clip.as_ref().map(|clip| clip.range),
                    Some(ExportRange {
                        start: 10.0,
                        end: 18.0,
                    })
                );
            }
        });
    }

    #[test]
    fn only_one_selected_clip_offers_a_range() {
        assert_eq!(selected_clip(None), None);
        assert_eq!(
            selected_clip(Some(&Selection::single(TrackKind::Clip, 2))),
            Some(2)
        );
        assert_eq!(
            selected_clip(Some(&Selection {
                track: TrackKind::Clip,
                indices: vec![0, 1],
            })),
            None
        );
        assert_eq!(
            selected_clip(Some(&Selection::single(TrackKind::Zoom, 0))),
            None
        );
    }

    #[test]
    fn stale_preview_results_cannot_replace_the_latest_or_reopened_preview() {
        let mut ui = clipboard_export();
        let previous = ui.preview_request.clone();
        let current = Arc::new(());
        ui.preview_request = current.clone();
        assert!(ui.update_preview(&current, |ui| ui.preview_error = Some("Latest".into())));
        assert!(!ui.update_preview(&previous, |ui| ui.preview_error = Some("Stale".into())));
        assert_eq!(ui.preview_error.as_deref(), Some("Latest"));

        let mut reopened = clipboard_export();
        assert!(!reopened.update_preview(&current, |ui| ui.preview_error = Some("Closed".into())));
        assert!(reopened.preview_error.is_none());
    }

    #[test]
    fn failed_clipboard_copy_retries_the_completed_file() {
        let mut ui = clipboard_export();
        let output = PathBuf::from("finished recording.mp4");
        ui.copy_completed_export(output.clone(), |path| {
            assert_eq!(path, output);
            Err("Clipboard is busy".into())
        });
        assert_eq!(ui.phase, ExportPhase::Failed);
        assert_eq!(ui.error.as_deref(), Some("Clipboard is busy"));
        let retry = ui.clipboard_retry_path().unwrap().to_path_buf();
        assert_eq!(retry, output);

        ui.copy_completed_export(retry, |path| {
            assert_eq!(path, output);
            Ok(())
        });
        assert_eq!(ui.phase, ExportPhase::Done);
        assert_eq!(ui.output_path, Some(output));
        assert!(ui.error.is_none());
        assert!(ui.clipboard_retry_path().is_none());
    }

    #[test]
    fn cancellation_or_close_prevents_late_clipboard_mutation() {
        for closing in [false, true] {
            let mut ui = clipboard_export();
            ui.close_requested = closing;
            ui.cancel.store(!closing, Ordering::Relaxed);
            let output = PathBuf::from("finished recording.mp4");
            ui.copy_completed_export(output.clone(), |_| {
                panic!("cancelled exports must not replace the clipboard")
            });
            assert_eq!(ui.phase, ExportPhase::Idle);
            assert_eq!(ui.output_path, Some(output));
            assert!(ui.clipboard_retry_path().is_none());
        }
    }

    #[test]
    fn incomplete_or_non_clipboard_exports_cannot_retry_copy() {
        let mut ui = clipboard_export();
        ui.phase = ExportPhase::Failed;
        assert!(ui.clipboard_retry_path().is_none());
        ui.output_path = Some(PathBuf::from("finished recording.mp4"));
        ui.destination = ExportDestination::File;
        assert!(ui.clipboard_retry_path().is_none());
    }

    #[test]
    fn declining_cancel_preserves_the_running_export() {
        let cancel = Arc::new(AtomicBool::new(false));
        cancel_matching_export(&cancel, &cancel, false);
        assert!(!cancel.load(Ordering::Relaxed));
        cancel_matching_export(&cancel, &cancel, true);
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn late_confirmation_cannot_cancel_a_subsequent_export() {
        let previous = Arc::new(AtomicBool::new(false));
        let current = Arc::new(AtomicBool::new(false));
        cancel_matching_export(&current, &previous, true);
        assert!(!current.load(Ordering::Relaxed));
        assert!(!previous.load(Ordering::Relaxed));
    }

    #[test]
    fn choosing_a_file_blocks_duplicate_exports_without_showing_progress() {
        assert!(ExportPhase::ChoosingFile.is_busy());
        assert!(!ExportPhase::ChoosingFile.shows_progress());
    }

    #[test]
    fn cancelling_file_selection_returns_to_the_idle_page() {
        assert!(!ExportPhase::Idle.is_busy());
        assert!(!ExportPhase::Idle.shows_progress());
    }

    #[test]
    fn export_work_and_results_remain_visible() {
        for phase in [
            ExportPhase::Starting,
            ExportPhase::Rendering,
            ExportPhase::Copying,
            ExportPhase::Done,
            ExportPhase::Failed,
        ] {
            assert!(phase.shows_progress());
        }
    }
}
