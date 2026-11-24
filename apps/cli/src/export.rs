use std::{
    io::{Write, stdout},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};

use clap::{Args, ValueEnum};
use frameever_export::{ExporterBase, ExporterBuilder, Mp4ExporterBase, make_cursor_only_project};
use frameever_project::{RecordingMeta, RecordingMetaInner, XY};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ExportFormat {
    Mp4,
    Gif,
    Mov,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum FormatArg {
    Mp4,
    Gif,
    Mov,
    Mp3,
    Wav,
}

enum ChosenFormat {
    Video(Option<ExportFormat>),
    Audio(frameever_export::audio::AudioExportFormat),
}

impl ChosenFormat {
    fn of(format: Option<FormatArg>) -> Self {
        use frameever_export::audio::AudioExportFormat;
        match format {
            None => Self::Video(None),
            Some(FormatArg::Mp4) => Self::Video(Some(ExportFormat::Mp4)),
            Some(FormatArg::Gif) => Self::Video(Some(ExportFormat::Gif)),
            Some(FormatArg::Mov) => Self::Video(Some(ExportFormat::Mov)),
            Some(FormatArg::Mp3) => Self::Audio(AudioExportFormat::Mp3),
            Some(FormatArg::Wav) => Self::Audio(AudioExportFormat::Wav),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum QualityArg {
    Maximum,
    Social,
    Web,
    Potato,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CodecArg {
    H264,
    Hevc,
    Av1,
}

impl From<CodecArg> for frameever_export::mp4::ExportCodec {
    fn from(value: CodecArg) -> Self {
        match value {
            CodecArg::H264 => Self::H264,
            CodecArg::Hevc => Self::Hevc,
            CodecArg::Av1 => Self::Av1,
        }
    }
}

impl From<QualityArg> for frameever_export::mp4::ExportCompression {
    fn from(value: QualityArg) -> Self {
        match value {
            QualityArg::Maximum => Self::Maximum,
            QualityArg::Social => Self::Social,
            QualityArg::Web => Self::Web,
            QualityArg::Potato => Self::Potato,
        }
    }
}

#[derive(Args)]
#[command(long_about = "Render a '.ever' project to a video file.

NOTE: here --format selects the CONTAINER (mp4/gif/mov, or mp3/wav for the sound alone), NOT the output mode. For machine-readable
output pass --json (the global flag), which streams NDJSON progress + completion events to stdout.
The NDJSON uses PascalCase type tags and snake_case fields ({\"type\":\"Progress\",\"rendered_count\":N,
\"total_frames\":N} then {\"type\":\"Completed\",\"path\":\"...\"}); on failure a final
{\"type\":\"Error\",\"error\":\"...\"} is emitted.")]
pub struct Export {
    /// Path to a '.ever' project directory (as produced by `frameever record`)
    project_path: PathBuf,
    /// Output file (positional alternative to --output)
    output_path: Option<PathBuf>,
    /// Output file to write the export to
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,
    #[arg(
        long,
        value_enum,
        help = "Container to export: mp4 (default), gif or mov, or mp3 or wav for the project's edited sound alone (Studio projects; default '<project>/output/result.mp3'). NOT the output mode — use --json for JSON"
    )]
    format: Option<FormatArg>,
    /// Frames per second to render
    #[arg(long)]
    fps: Option<u32>,
    /// Output resolution as WIDTHxHEIGHT, e.g. 1920x1080
    #[arg(long)]
    resolution: Option<String>,
    /// Compression preset (mp4 only)
    #[arg(long, value_enum)]
    quality: Option<QualityArg>,
    #[arg(
        long,
        value_enum,
        help = "Video codec (mp4 only): h264 plays everywhere (default), hevc is about 40% smaller, av1 about half the size"
    )]
    codec: Option<CodecArg>,
    /// Optimise for smaller files using CRF (mp4 only)
    #[arg(long)]
    optimize_filesize: bool,
    /// Full export settings as JSON, e.g. {"format":"Mp4","fps":60,"resolution_base":{"x":1920,"y":1080},"compression":"Maximum","custom_bpp":null} (mutually exclusive with the flags above)
    #[arg(long)]
    settings_json: Option<String>,
    /// Decode source video with FFmpeg instead of the platform hardware decoder
    #[arg(long)]
    force_ffmpeg_decoder: bool,
    /// Stream newline-delimited JSON progress events to stdout ({"type":"Progress","rendered_count":N,"total_frames":N}; also emits a terminal {"type":"Error","error":"..."} on failure). Implied by --json
    #[arg(long)]
    progress_json: bool,
    /// Emit a final JSON completion event to stdout ({"type":"Completed","path":"..."}). Implied by --json
    #[arg(long)]
    completion_json: bool,
    #[arg(
        long,
        value_name = "TRACKS",
        help = "Export the recorded tracks as separate files instead of one video: a comma-separated list of screen, camera, microphone, system-audio, or all. They follow the edited timeline and are written to the --output folder (default '<project>/output/tracks') as Screen.mp4, Camera.mp4, Microphone.wav and System audio.wav. Takes --fps (default 60)"
    )]
    tracks: Option<String>,
}

#[derive(Default)]
pub struct ExportFlags {
    pub format: Option<ExportFormat>,
    pub fps: Option<u32>,
    pub resolution: Option<String>,
    pub quality: Option<QualityArg>,
    pub codec: Option<CodecArg>,
    pub optimize_filesize: bool,
    pub force_ffmpeg_decoder: bool,
}

impl ExportFlags {
    fn is_set(&self) -> bool {
        self.format.is_some()
            || self.fps.is_some()
            || self.resolution.is_some()
            || self.quality.is_some()
            || self.codec.is_some()
            || self.optimize_filesize
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(tag = "format")]
pub enum CliExportSettings {
    #[serde(alias = "mp4")]
    Mp4(frameever_export::mp4::Mp4ExportSettings),
    #[serde(alias = "gif")]
    Gif(frameever_export::gif::GifExportSettings),
    #[serde(alias = "mov")]
    Mov(frameever_export::mov::MovExportSettings),
}

impl CliExportSettings {
    async fn prepare(self, builder: ExporterBuilder) -> Result<PreparedExport, String> {
        match self {
            Self::Mp4(settings) => builder
                .build_for_mp4(Arc::new(AtomicBool::new(false)))
                .await
                .map(|base| PreparedExport::Mp4(settings, base)),
            Self::Gif(settings) => builder
                .build()
                .await
                .map(|base| PreparedExport::Gif(settings, base)),
            Self::Mov(settings) => builder
                .build()
                .await
                .map(|base| PreparedExport::Mov(settings, base)),
        }
        .map_err(|error| format!("Exporter build error: {error}"))
    }

    fn fps(&self) -> u32 {
        match self {
            Self::Mp4(settings) => settings.fps,
            Self::Gif(settings) => settings.fps,
            Self::Mov(settings) => settings.fps,
        }
    }

    fn resolution_base(&self) -> XY<u32> {
        match self {
            Self::Mp4(settings) => settings.resolution_base,
            Self::Gif(settings) => settings.resolution_base,
            Self::Mov(settings) => settings.resolution_base,
        }
    }

    fn force_ffmpeg_decoder(&self) -> bool {
        match self {
            Self::Mp4(settings) => settings.force_ffmpeg_decoder,
            Self::Gif(_) | Self::Mov(_) => false,
        }
    }

    fn cursor_only(&self) -> bool {
        match self {
            Self::Mov(settings) => settings.cursor_only,
            Self::Mp4(_) | Self::Gif(_) => false,
        }
    }
}

enum PreparedExport {
    Mp4(frameever_export::mp4::Mp4ExportSettings, Mp4ExporterBase),
    Gif(frameever_export::gif::GifExportSettings, ExporterBase),
    Mov(frameever_export::mov::MovExportSettings, ExporterBase),
}

impl PreparedExport {
    fn total_frames(&self, fps: u32) -> u32 {
        match self {
            Self::Mp4(_, base) => base.total_frames(fps),
            Self::Gif(_, base) | Self::Mov(_, base) => base.total_frames(fps),
        }
    }

    async fn export(
        self,
        on_progress: impl FnMut(u32) -> bool + Send + 'static,
    ) -> Result<PathBuf, String> {
        match self {
            Self::Mp4(settings, base) => settings.export_prepared(base, on_progress).await,
            Self::Gif(settings, base) => settings.export(base, on_progress).await,
            Self::Mov(settings, base) => settings.export(base, on_progress).await,
        }
        .map_err(|error| format!("Exporter error: {error}"))
    }
}

fn settings_from_json(json: &str) -> Result<CliExportSettings, String> {
    let settings: CliExportSettings =
        serde_json::from_str(json).map_err(|e| format!("Invalid export settings JSON: {e}"))?;
    if settings.fps() == 0 {
        return Err("--settings-json fps must be greater than zero".to_string());
    }
    let resolution = settings.resolution_base();
    if resolution.x == 0 || resolution.y == 0 {
        return Err("--settings-json resolution_base must be greater than zero".to_string());
    }
    Ok(settings)
}

fn default_fps(format: ExportFormat) -> u32 {
    match format {
        ExportFormat::Mp4 | ExportFormat::Mov => 60,
        ExportFormat::Gif => 30,
    }
}

fn parse_resolution(value: &str) -> Result<XY<u32>, String> {
    let (w, h) = value
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("Invalid resolution '{value}', expected WIDTHxHEIGHT"))?;
    let width: u32 = w
        .trim()
        .parse()
        .map_err(|_| format!("Invalid resolution width in '{value}'"))?;
    let height: u32 = h
        .trim()
        .parse()
        .map_err(|_| format!("Invalid resolution height in '{value}'"))?;
    if width == 0 || height == 0 {
        return Err(format!("Resolution '{value}' must be greater than zero"));
    }
    Ok(XY::new(width, height))
}

pub fn settings_from_flags(flags: &ExportFlags) -> Result<CliExportSettings, String> {
    let format = flags.format.unwrap_or(ExportFormat::Mp4);
    let fps = flags.fps.unwrap_or_else(|| default_fps(format));
    if fps == 0 {
        return Err("--fps must be greater than zero".to_string());
    }
    let resolution_base = match &flags.resolution {
        Some(value) => parse_resolution(value)?,
        None => XY::new(1920, 1080),
    };

    match format {
        ExportFormat::Mp4 => Ok(CliExportSettings::Mp4(
            frameever_export::mp4::Mp4ExportSettings {
                codec: flags.codec.map(Into::into).unwrap_or_default(),
                fps,
                resolution_base,
                compression: flags
                    .quality
                    .map(Into::into)
                    .unwrap_or(frameever_export::mp4::ExportCompression::Maximum),
                custom_bpp: None,
                force_ffmpeg_decoder: flags.force_ffmpeg_decoder,
                optimize_filesize: flags.optimize_filesize,
            },
        )),
        ExportFormat::Gif => {
            if flags.codec.is_some() {
                return Err("--codec is only supported for --format mp4".to_string());
            }
            if flags.quality.is_some() {
                return Err(
                    "--quality is only supported for --format mp4; use --settings-json for GIF quality"
                        .to_string(),
                );
            }
            if flags.optimize_filesize {
                return Err("--optimize-filesize is only supported for --format mp4".to_string());
            }
            Ok(CliExportSettings::Gif(
                frameever_export::gif::GifExportSettings {
                    fps,
                    resolution_base,
                    quality: None,
                },
            ))
        }
        ExportFormat::Mov => {
            if flags.codec.is_some() {
                return Err("--codec is only supported for --format mp4".to_string());
            }
            if flags.quality.is_some() {
                return Err("--quality is only supported for --format mp4".to_string());
            }
            if flags.optimize_filesize {
                return Err("--optimize-filesize is only supported for --format mp4".to_string());
            }
            Ok(CliExportSettings::Mov(
                frameever_export::mov::MovExportSettings {
                    fps,
                    resolution_base,
                    cursor_only: false,
                },
            ))
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ExportProgressMessage<'a> {
    Progress {
        rendered_count: u32,
        total_frames: u32,
    },
    Completed {
        path: &'a std::path::Path,
    },
    // Field is named `error` (not `message`) so a single `"error" in obj` predicate detects failure
    // across every JSON-emitting command. The desktop sidecar only parses Progress/Completed, so the
    // PascalCase `type` tag stays for back-compat while this terminal field is free to standardize.
    Error {
        error: &'a str,
    },
}

impl Export {
    fn resolve_output(&self) -> Result<Option<PathBuf>, String> {
        match (&self.output, &self.output_path) {
            (Some(_), Some(_)) => Err(
                "Specify the output path either positionally or with --output, not both"
                    .to_string(),
            ),
            (Some(path), None) | (None, Some(path)) => {
                frameever_export::validate_output_path(path)?;
                Ok(Some(path.clone()))
            }
            (None, None) => Ok(None),
        }
    }

    fn resolve_settings(&self, format: Option<ExportFormat>) -> Result<CliExportSettings, String> {
        let flags = ExportFlags {
            format,
            fps: self.fps,
            resolution: self.resolution.clone(),
            quality: self.quality,
            codec: self.codec,
            optimize_filesize: self.optimize_filesize,
            force_ffmpeg_decoder: self.force_ffmpeg_decoder,
        };

        match &self.settings_json {
            Some(json) => {
                if flags.is_set() {
                    return Err(
                        "--settings-json cannot be combined with --format/--fps/--resolution/--quality/--optimize-filesize"
                            .to_string(),
                    );
                }
                settings_from_json(json)
            }
            None => settings_from_flags(&flags),
        }
    }

    pub async fn run(self, json: bool) -> Result<(), String> {
        // The global --json flag is the agent-facing way to ask for machine-readable output; it
        // implies both NDJSON streams so export matches the rest of the CLI's --json convention.
        // --progress-json / --completion-json remain for the desktop sidecar, which passes them.
        let progress_json = self.progress_json || json;
        let completion_json = self.completion_json || json;
        let stdout = Arc::new(Mutex::new(stdout()));

        match self
            .run_inner(progress_json, completion_json, &stdout)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                // Without this an agent streaming the NDJSON feed sees the stream stop with no
                // terminal marker; mirror record's Error event so failures stay machine-readable.
                if progress_json || completion_json {
                    let _ = emit_export_message(
                        &stdout,
                        &ExportProgressMessage::Error { error: &error },
                    );
                }
                Err(error)
            }
        }
    }

    async fn run_inner(
        self,
        progress_json: bool,
        completion_json: bool,
        stdout: &Arc<Mutex<std::io::Stdout>>,
    ) -> Result<(), String> {
        let output = self.resolve_output()?;
        frameever_export::validate_output_path(&self.project_path)?;
        let project_hold = hold_project_for_reading(self.project_path.clone()).await?;
        if let Some(tracks) = self.tracks.clone() {
            return self
                .run_tracks(&tracks, output, progress_json, completion_json, stdout)
                .await;
        }
        let format = match ChosenFormat::of(self.format) {
            ChosenFormat::Video(format) => format,
            ChosenFormat::Audio(format) => {
                return self
                    .run_audio(format, output, progress_json, completion_json, stdout)
                    .await;
            }
        };
        let settings = self.resolve_settings(format)?;

        ensure_remuxed(self.project_path.clone()).await?;
        let meta = RecordingMeta::load_for_project(&self.project_path)
            .map_err(|e| format!("Failed to load recording meta: {e}"))?;

        if matches!(&meta.inner, RecordingMetaInner::Instant(_)) {
            drop(project_hold);
            return export_instant_project(
                self.project_path,
                output,
                &settings,
                progress_json,
                completion_json,
                stdout,
            )
            .await;
        }

        let force_ffmpeg_decoder = self.force_ffmpeg_decoder || settings.force_ffmpeg_decoder();
        let mut builder = ExporterBase::builder(self.project_path.clone())
            .with_force_ffmpeg_decoder(force_ffmpeg_decoder);

        if let Some(output_path) = output {
            builder = builder.with_output_path(output_path);
        }

        if settings.cursor_only() {
            builder = builder.with_config(make_cursor_only_project(meta.project_config()));
        }

        let fps = settings.fps();
        let exporter = settings.prepare(builder).await?;
        let total_frames = exporter.total_frames(fps);

        if progress_json {
            emit_export_message(
                stdout,
                &ExportProgressMessage::Progress {
                    rendered_count: 0,
                    total_frames,
                },
            )?;
        }

        let rendered = Arc::new(AtomicU32::new(0));
        let progress_stdout = Arc::clone(stdout);
        let progress_rendered = Arc::clone(&rendered);
        let on_progress = move |frame_index: u32| {
            let count = (frame_index + 1).min(total_frames);
            progress_rendered.store(count, Ordering::Relaxed);
            if progress_json {
                // Progress I/O must never cancel the render. Every exporter treats a `false`
                // return as cancellation, so a transient stdout failure (closed pipe, poisoned
                // lock) is swallowed here rather than aborting the export.
                let _ = emit_export_message(
                    &progress_stdout,
                    &ExportProgressMessage::Progress {
                        rendered_count: count,
                        total_frames,
                    },
                );
            }
            true
        };

        let output_path = exporter.export(on_progress).await?;

        // Defense in depth: an export that renders no frames writes an empty (~few hundred byte) file
        // but otherwise "succeeds". An agent must never silently get/upload that, so fail loudly and
        // remove the empty artifact instead of reporting completion.
        if total_frames > 0 && rendered.load(Ordering::Relaxed) == 0 {
            let _ = std::fs::remove_file(&output_path);
            return Err(format!(
                "Export rendered 0 of {total_frames} frames; the recording may be unplayable. No output written."
            ));
        }

        if progress_json || completion_json {
            emit_export_message(
                stdout,
                &ExportProgressMessage::Completed { path: &output_path },
            )?;
        } else {
            // Default callers pass no JSON flag; the resolved path is otherwise invisible (the
            // tracing log below is gated behind --log-level), so emit it to stdout unconditionally.
            println!("Exported video to {}", output_path.display());
        }

        info!("Exported video to '{}'", output_path.display());

        Ok(())
    }
}

impl Export {
    fn audio_conflicts(&self) -> Option<&'static str> {
        if self.settings_json.is_some() {
            Some("--settings-json")
        } else if self.fps.is_some() {
            Some("--fps")
        } else if self.resolution.is_some() {
            Some("--resolution")
        } else if self.quality.is_some() {
            Some("--quality")
        } else if self.codec.is_some() {
            Some("--codec")
        } else if self.optimize_filesize {
            Some("--optimize-filesize")
        } else {
            None
        }
    }

    async fn run_audio(
        self,
        format: frameever_export::audio::AudioExportFormat,
        output: Option<PathBuf>,
        progress_json: bool,
        completion_json: bool,
        stdout: &Arc<Mutex<std::io::Stdout>>,
    ) -> Result<(), String> {
        if let Some(flag) = self.audio_conflicts() {
            return Err(format!(
                "--format {} exports the sound alone and cannot be combined with {flag}",
                format.extension()
            ));
        }
        let project_path = self.project_path;
        ensure_remuxed(project_path.clone()).await?;
        let output = output.unwrap_or_else(|| {
            project_path
                .join("output")
                .join(format!("result.{}", format.extension()))
        });
        let progress_stdout = Arc::clone(stdout);
        let saved = frameever_export::audio::export_audio(
            project_path,
            None,
            format,
            output,
            Arc::new(AtomicBool::new(false)),
            move |progress| {
                if progress_json {
                    let _ = emit_export_message(
                        &progress_stdout,
                        &ExportProgressMessage::Progress {
                            rendered_count: progress.done.min(u64::from(u32::MAX)) as u32,
                            total_frames: progress.total.min(u64::from(u32::MAX)) as u32,
                        },
                    );
                }
            },
        )
        .await
        .map_err(|error| match error {
            frameever_export::tracks::TrackExportError::NotStudio => {
                "Instant recordings are a finished video; MP3 and WAV export is for Studio projects"
                    .to_string()
            }
            error => error.to_string(),
        })?;

        if progress_json || completion_json {
            emit_export_message(stdout, &ExportProgressMessage::Completed { path: &saved })?;
        } else {
            println!("Exported audio to {}", saved.display());
        }
        info!("Exported audio to '{}'", saved.display());
        Ok(())
    }

    fn tracks_conflicts(&self) -> Option<&'static str> {
        if self.settings_json.is_some() {
            Some("--settings-json")
        } else if self.format.is_some() {
            Some("--format")
        } else if self.resolution.is_some() {
            Some("--resolution")
        } else if self.quality.is_some() {
            Some("--quality")
        } else if self.codec.is_some() {
            Some("--codec")
        } else if self.optimize_filesize {
            Some("--optimize-filesize")
        } else {
            None
        }
    }

    async fn run_tracks(
        self,
        tracks: &str,
        output: Option<PathBuf>,
        progress_json: bool,
        completion_json: bool,
        stdout: &Arc<Mutex<std::io::Stdout>>,
    ) -> Result<(), String> {
        if let Some(flag) = self.tracks_conflicts() {
            return Err(format!(
                "--tracks cannot be combined with {flag}; tracks keep their recorded size and only take --fps"
            ));
        }
        let fps = self.fps.unwrap_or_else(|| default_fps(ExportFormat::Mp4));
        if fps == 0 {
            return Err("--fps must be greater than zero".to_string());
        }
        let project_path = self.project_path;

        ensure_remuxed(project_path.clone()).await?;
        let meta = RecordingMeta::load_for_project(&project_path)
            .map_err(|e| format!("Failed to load recording meta: {e}"))?;
        let Some(studio) = meta.studio_meta() else {
            return Err(frameever_export::tracks::TrackExportError::NotStudio.to_string());
        };
        let selection = parse_tracks(tracks, || {
            let recordings =
                frameever_rendering::ProjectRecordingsMeta::new(&meta.project_path, studio)?;
            Ok(frameever_export::tracks::available_tracks(
                &recordings,
                &meta.project_config(),
            ))
        })?;
        let output = output.unwrap_or_else(|| project_path.join("output").join("tracks"));

        let total = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicU64::new(0));
        let progress_stdout = Arc::clone(stdout);
        let progress_done = Arc::clone(&done);
        let progress_total = Arc::clone(&total);
        let files = frameever_export::tracks::export_tracks(
            project_path,
            None,
            frameever_export::tracks::TrackExportSettings {
                tracks: selection,
                fps,
                force_ffmpeg_decoder: self.force_ffmpeg_decoder,
            },
            output.clone(),
            Arc::new(AtomicBool::new(false)),
            move |progress| {
                progress_done.store(progress.done, Ordering::Relaxed);
                progress_total.store(progress.total, Ordering::Relaxed);
                if progress_json {
                    let _ = emit_export_message(
                        &progress_stdout,
                        &ExportProgressMessage::Progress {
                            rendered_count: progress.done.min(u64::from(u32::MAX)) as u32,
                            total_frames: progress.total.min(u64::from(u32::MAX)) as u32,
                        },
                    );
                }
            },
        )
        .await
        .map_err(|error| error.to_string())?;

        if progress_json || completion_json {
            emit_export_message(stdout, &ExportProgressMessage::Completed { path: &output })?;
        } else {
            println!("Exported {} tracks to {}", files.len(), output.display());
            for file in &files {
                if let Some(name) = file.file_name() {
                    println!("  {}", name.to_string_lossy());
                }
            }
        }
        info!(
            "Exported {} tracks to '{}' ({}/{} steps)",
            files.len(),
            output.display(),
            done.load(Ordering::Relaxed),
            total.load(Ordering::Relaxed)
        );
        Ok(())
    }
}

fn parse_tracks(
    value: &str,
    available: impl FnOnce() -> Result<Vec<frameever_export::tracks::TrackKind>, String>,
) -> Result<Vec<frameever_export::tracks::TrackKind>, String> {
    use frameever_export::tracks::TrackKind;
    let names = value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if names.is_empty() {
        return Err(
            "--tracks needs at least one of screen, camera, microphone, system-audio, or all"
                .to_string(),
        );
    }
    if names.iter().any(|name| name.eq_ignore_ascii_case("all")) {
        if names.len() > 1 {
            return Err("--tracks all cannot be combined with other track names".to_string());
        }
        return available();
    }
    let mut tracks = names
        .iter()
        .map(|name| {
            TrackKind::from_slug(name).ok_or_else(|| {
                format!(
                    "Unknown track '{name}'; use screen, camera, microphone, system-audio, or all"
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    tracks.sort();
    tracks.dedup();
    Ok(tracks)
}

/// Remux a recording left as fragments (status `NeedsRemux`) into a progressive `display.mp4` before
/// export, reusing the shared `RecoveryManager`. A graceful `frameever record` stop already remuxes in
/// `finalize`, so this only fires for recordings interrupted before that (e.g. a killed worker);
/// without it the exporter fails trying to open a fragment directory as a video. No-op for recordings
/// that are already progressive.
async fn hold_project_for_reading(
    project_path: PathBuf,
) -> Result<Option<frameever_recording::upload_resume::UploadLock>, String> {
    tokio::task::spawn_blocking(move || {
        crate::project::open_project(&project_path, crate::project::ProjectAccess::Read)
    })
    .await
    .map_err(|error| format!("project lock task failed: {error}"))?
}

async fn ensure_remuxed(project_path: PathBuf) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        frameever_recording::recovery::RecoveryManager::remux_if_needed(&project_path)
    })
    .await
    .map_err(|e| format!("recording remux task failed: {e}"))?
    .map_err(|e| format!("Failed to remux recording before export: {e}"))?;

    Ok(())
}

async fn prepare_instant_output(project_path: PathBuf) -> Result<PathBuf, String> {
    tokio::task::spawn_blocking(move || {
        if let Some(output) = completed_instant_output(&project_path)? {
            return Ok(output);
        }
        let ownership = frameever_recording::upload_resume::UploadLock::acquire(&project_path).map_err(
            |error| {
                format!(
                    "Instant output needs repair, but upload ownership is unavailable: {error}; local recording retained"
                )
            },
        )?;
        let project_path = ownership.project_path();
        if let Some(output) = completed_instant_output(project_path)? {
            return Ok(output);
        }
        frameever_recording::recovery::RecoveryManager::finalize_instant_output(
            &project_path.join("content/display"),
            &project_path.join("content/audio"),
            &project_path.join("content/output.mp4"),
        )
        .map_err(|error| format!("Failed to finalize instant recording before export: {error}"))
    })
    .await
    .map_err(|error| format!("instant export finalize task failed: {error}"))?
}

fn completed_instant_output(project_path: &Path) -> Result<Option<PathBuf>, String> {
    for path in [project_path.to_path_buf(), project_path.join("content")] {
        if let Some(metadata) = instant_input_metadata(&path)?
            && !metadata.is_dir()
        {
            return Err("Invalid Instant recording directory; local recording retained".into());
        }
    }
    let meta = RecordingMeta::load_for_project(project_path)
        .map_err(|error| format!("Failed to load Instant recording metadata: {error}"))?;
    let RecordingMetaInner::Instant(frameever_project::InstantRecordingMeta::Complete {
        sample_rate,
        ..
    }) = meta.inner
    else {
        return Ok(None);
    };
    let mut required_audio = sample_rate.is_some();
    for (name, field) in [
        ("tauri-upload.json", "required_audio"),
        ("instant-upload.json", "requested_audio"),
    ] {
        let path = project_path.join(name);
        if let Some(metadata) = instant_input_metadata(&path)? {
            if !metadata.is_file() || metadata.len() > 65536 {
                return Err("Invalid Instant upload intent; local recording retained".into());
            }
            let intent: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).map_err(|error| error.to_string())?)
                    .map_err(|error| format!("Invalid Instant upload intent: {error}"))?;
            if let Some(value) = intent.get(field).filter(|value| !value.is_null()) {
                required_audio |= value
                    .as_bool()
                    .ok_or("Invalid Instant audio intent; local recording retained")?;
            }
        }
    }
    let audio_dir = project_path.join("content/audio");
    let audio_metadata = instant_input_metadata(&audio_dir)?;
    let has_audio_directory = audio_metadata.is_some();
    let mut has_audio_sources = false;
    if let Some(metadata) = audio_metadata {
        if !metadata.is_dir() {
            return Err("Invalid Instant audio directory; local recording retained".into());
        }
        for entry in std::fs::read_dir(audio_dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let is_audio_media = path.extension().is_some_and(|extension| {
                ["m4s", "mp4", "ogg", "aac", "wav", "webm", "m4a"]
                    .iter()
                    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            });
            if let Some(metadata) = instant_input_metadata(&path)?
                && is_audio_media
                && metadata.is_file()
                && metadata.len() > 0
            {
                required_audio = true;
                has_audio_sources = true;
            }
        }
    }
    let output = project_path.join("content/output.mp4");
    if !required_audio
        && !has_audio_directory
        && instant_input_metadata(&output)?
            .is_some_and(|metadata| metadata.is_file() && metadata.len() > 0)
    {
        return Ok(Some(output));
    }
    match frameever_recording::recovery::RecoveryManager::validate_instant_output(
        project_path,
        required_audio,
    ) {
        Ok(output) => Ok(Some(output)),
        Err(error) if required_audio && !has_audio_sources => Err(format!(
            "Instant output failed validation and required audio cannot be repaired: {error}; local recording retained"
        )),
        Err(_) => Ok(None),
    }
}

fn instant_input_metadata(path: &Path) -> Result<Option<std::fs::Metadata>, String> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Instant input is a reparse point; local recording retained".into());
        }
    }
    if metadata.file_type().is_symlink() {
        return Err("Instant input is a symbolic link; local recording retained".into());
    }
    Ok(Some(metadata))
}

fn instant_export_settings_supported(settings: &CliExportSettings) -> bool {
    match settings {
        CliExportSettings::Mp4(settings) => {
            settings.fps == 60
                && settings.resolution_base == XY::new(1920, 1080)
                && matches!(
                    settings.compression,
                    frameever_export::mp4::ExportCompression::Maximum
                )
                && settings.custom_bpp.is_none()
                && !settings.optimize_filesize
        }
        CliExportSettings::Gif(_) | CliExportSettings::Mov(_) => false,
    }
}

fn validate_instant_output_path(path: &Path) -> Result<(), String> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| !extension.eq_ignore_ascii_case("mp4"))
    {
        return Err("Instant recordings can only be exported as mp4 files".to_string());
    }

    Ok(())
}

fn copy_instant_output(source_path: &Path, output_path: PathBuf) -> Result<PathBuf, String> {
    validate_instant_output_path(&output_path)?;

    if source_path == output_path.as_path() {
        return Ok(output_path);
    }

    if output_path.exists()
        && crate::file_identity::is_same_file(source_path, &output_path)
            .map_err(|e| format!("Failed to compare source and destination: {e}"))?
    {
        return Ok(output_path);
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create output directory {}: {e}",
                parent.display()
            )
        })?;
    }

    std::fs::copy(source_path, &output_path).map_err(|e| {
        format!(
            "Failed to copy instant recording from {} to {}: {e}",
            source_path.display(),
            output_path.display()
        )
    })?;

    Ok(output_path)
}

async fn export_instant_project(
    project_path: PathBuf,
    output: Option<PathBuf>,
    settings: &CliExportSettings,
    progress_json: bool,
    completion_json: bool,
    stdout: &Arc<Mutex<std::io::Stdout>>,
) -> Result<(), String> {
    if !instant_export_settings_supported(settings) {
        return Err(
            "Instant recordings are already finalized MP4 files; export supports copying them to an mp4 output path"
                .to_string(),
        );
    }

    let source_path = prepare_instant_output(project_path).await?;
    let output_path = output.unwrap_or_else(|| source_path.clone());

    if progress_json {
        emit_export_message(
            stdout,
            &ExportProgressMessage::Progress {
                rendered_count: 0,
                total_frames: 1,
            },
        )?;
    }

    let output_path = copy_instant_output(&source_path, output_path)?;

    if progress_json {
        emit_export_message(
            stdout,
            &ExportProgressMessage::Progress {
                rendered_count: 1,
                total_frames: 1,
            },
        )?;
    }

    if progress_json || completion_json {
        emit_export_message(
            stdout,
            &ExportProgressMessage::Completed { path: &output_path },
        )?;
    } else {
        println!("Exported video to {}", output_path.display());
    }

    info!("Exported instant video to '{}'", output_path.display());

    Ok(())
}

/// Render a project to its default output path with default settings (mp4, 1080p60, Maximum). Used by
/// `cap upload --export` to glue record -> export -> upload into one step.
pub async fn export_project_default(project_path: PathBuf) -> Result<PathBuf, String> {
    let settings = settings_from_flags(&ExportFlags::default())?;
    ensure_remuxed(project_path.clone()).await?;
    let meta = RecordingMeta::load_for_project(&project_path)
        .map_err(|e| format!("Failed to load recording meta: {e}"))?;
    if matches!(&meta.inner, RecordingMetaInner::Instant(_)) {
        return prepare_instant_output(project_path).await;
    }

    let fps = settings.fps();
    let force_ffmpeg_decoder = settings.force_ffmpeg_decoder();
    let exporter = settings
        .prepare(
            ExporterBase::builder(project_path).with_force_ffmpeg_decoder(force_ffmpeg_decoder),
        )
        .await?;

    let total_frames = exporter.total_frames(fps);
    let rendered = Arc::new(AtomicU32::new(0));
    let progress_rendered = Arc::clone(&rendered);
    let on_progress = move |frame_index: u32| {
        progress_rendered.store((frame_index + 1).min(total_frames), Ordering::Relaxed);
        true
    };

    let output_path = exporter.export(on_progress).await?;

    // Same 0-frame guard as Export::run_inner: a recording with missing media renders an empty,
    // unplayable file that otherwise "succeeds", and `cap upload --export` would sign + upload it and
    // hand back a valid-looking link. Fail loudly and remove the artifact instead.
    if total_frames > 0 && rendered.load(Ordering::Relaxed) == 0 {
        let _ = std::fs::remove_file(&output_path);
        return Err(format!(
            "Export rendered 0 of {total_frames} frames; the recording may be unplayable. No output written."
        ));
    }

    Ok(output_path)
}

#[derive(Args)]
pub struct ExportPreview {
    project_path: PathBuf,
    #[arg(long)]
    frame_time: f64,
    #[arg(long)]
    settings_json: String,
    #[arg(long)]
    force_ffmpeg_decoder: bool,
}

impl ExportPreview {
    pub async fn run(self) -> Result<(), String> {
        match self.run_inner().await {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = crate::write_json(&serde_json::json!({ "error": error }));
                Err(error)
            }
        }
    }

    async fn run_inner(self) -> Result<(), String> {
        let _project_hold = hold_project_for_reading(self.project_path.clone()).await?;
        let settings = serde_json::from_str::<frameever_export::preview::ExportPreviewSettings>(
            &self.settings_json,
        )
        .map_err(|e| format!("Invalid preview settings JSON: {e}"))?;
        let result = frameever_export::preview::render_preview(
            self.project_path,
            self.frame_time,
            settings,
            self.force_ffmpeg_decoder,
        )
        .await
        .map_err(|e| format!("Preview render error: {e}"))?;

        let mut stdout = stdout();
        serde_json::to_writer(&mut stdout, &result).map_err(|e| e.to_string())?;
        writeln!(&mut stdout).map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())
    }
}

fn emit_export_message(
    stdout: &Arc<Mutex<std::io::Stdout>>,
    message: &ExportProgressMessage<'_>,
) -> Result<(), String> {
    let mut stdout = stdout
        .lock()
        .map_err(|_| "Failed to lock stdout".to_string())?;
    serde_json::to_writer(&mut *stdout, message).map_err(|e| e.to_string())?;
    writeln!(&mut *stdout).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_lists_parse_in_a_stable_order() {
        use frameever_export::tracks::TrackKind;
        let none = || -> Result<Vec<TrackKind>, String> { panic!("only `all` lists the project") };
        assert_eq!(
            parse_tracks("system-audio, mic,screen,mic", none).unwrap(),
            [
                TrackKind::Screen,
                TrackKind::Microphone,
                TrackKind::SystemAudio
            ]
        );
        assert_eq!(
            parse_tracks("ALL", || Ok(vec![TrackKind::Screen, TrackKind::Camera])).unwrap(),
            [TrackKind::Screen, TrackKind::Camera]
        );
        assert!(parse_tracks("all,screen", none).is_err());
        assert!(parse_tracks(" , ", none).is_err());
        assert!(
            parse_tracks("screen,cursor", none)
                .unwrap_err()
                .contains("Unknown track 'cursor'")
        );
    }

    fn export_args(args: &[&str]) -> Export {
        #[derive(clap::Parser)]
        struct Cli {
            #[command(flatten)]
            export: Export,
        }
        let mut argv = vec!["export"];
        argv.extend_from_slice(args);
        <Cli as clap::Parser>::try_parse_from(argv).unwrap().export
    }

    fn studio_project_with_a_tone() -> tempfile::TempDir {
        ffmpeg::init().unwrap();
        let project = tempfile::tempdir().unwrap();
        frameever_enc_ffmpeg::standin::write_black_standin(
            &project.path().join("display.mp4"),
            64,
            64,
            30,
            std::time::Duration::from_secs(1),
            &["libx264"],
        )
        .unwrap();
        let frames = 48_000u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + frames * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&48_000u32.to_le_bytes());
        wav.extend_from_slice(&96_000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(frames * 2).to_le_bytes());
        for _ in 0..frames {
            wav.extend_from_slice(&8_000i16.to_le_bytes());
        }
        std::fs::write(project.path().join("mic.wav"), wav).unwrap();
        std::fs::write(
            project.path().join("recording-meta.json"),
            serde_json::to_vec(&serde_json::json!({
                "pretty_name": "CLI audio export",
                "segments": [{
                    "display": {"path": "display.mp4", "fps": 30},
                    "mic": {"path": "mic.wav"}
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        frameever_project::ProjectConfiguration::default()
            .write(project.path())
            .unwrap();
        project
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mp3_and_wav_export_the_sound_into_the_output_folder() {
        let project = studio_project_with_a_tone();
        let path = project.path().to_str().unwrap();
        let stdout = Arc::new(Mutex::new(stdout()));
        for extension in ["mp3", "wav"] {
            export_args(&[path, "--format", extension])
                .run_inner(false, false, &stdout)
                .await
                .unwrap();
            let saved = project
                .path()
                .join("output")
                .join(format!("result.{extension}"));
            let input = ffmpeg::format::input(&saved).unwrap();
            assert!(input.streams().best(ffmpeg::media::Type::Video).is_none());
            let seconds = input.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE);
            assert!((seconds - 1.0).abs() < 0.1, "{extension} lasts {seconds}");
        }
    }

    #[tokio::test]
    async fn audio_formats_refuse_video_settings() {
        let stdout = Arc::new(Mutex::new(stdout()));
        for flags in [
            ["--fps", "30"],
            ["--resolution", "640x360"],
            ["--codec", "hevc"],
        ] {
            let mut args = vec!["missing.ever", "--format", "wav"];
            args.extend_from_slice(&flags);
            let error = export_args(&args)
                .run_inner(false, false, &stdout)
                .await
                .unwrap_err();
            assert!(error.contains(flags[0]), "{error}");
        }
    }

    fn playable_instant_project() -> tempfile::TempDir {
        ffmpeg::init().unwrap();
        let project = tempfile::tempdir().unwrap();
        let content = project.path().join("content");
        std::fs::create_dir(&content).unwrap();
        let mut output = ffmpeg::format::output(&content.join("output.mp4")).unwrap();
        let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::MPEG4).unwrap();
        let mut encoder = ffmpeg::codec::Context::new_with_codec(codec)
            .encoder()
            .video()
            .unwrap();
        encoder.set_width(32);
        encoder.set_height(32);
        encoder.set_format(ffmpeg::format::Pixel::YUV420P);
        encoder.set_time_base((1, 30));
        encoder.set_frame_rate(Some((30, 1)));
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        let mut encoder = encoder.open_as(codec).unwrap();
        output.add_stream(codec).unwrap().set_parameters(&encoder);
        output.write_header().unwrap();
        let output_time_base = output.stream(0).unwrap().time_base();
        {
            let mut drain = |encoder: &mut ffmpeg::encoder::video::Encoder, finished: bool| loop {
                let mut packet = ffmpeg::Packet::empty();
                match encoder.receive_packet(&mut packet) {
                    Ok(()) => {
                        packet.set_stream(0);
                        packet.rescale_ts((1, 30), output_time_base);
                        packet.write_interleaved(&mut output).unwrap();
                    }
                    Err(ffmpeg::Error::Other { errno })
                        if !finished && errno == ffmpeg::ffi::EAGAIN =>
                    {
                        break;
                    }
                    Err(ffmpeg::Error::Eof) if finished => break,
                    Err(error) => panic!("Synthetic video encoding failed: {error}"),
                }
            };
            for index in 0..30 {
                let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::YUV420P, 32, 32);
                frame.data_mut(0).fill(32 + index as u8);
                frame.data_mut(1).fill(128);
                frame.data_mut(2).fill(128);
                frame.set_pts(Some(index));
                encoder.send_frame(&frame).unwrap();
                drain(&mut encoder, false);
            }
            encoder.send_eof().unwrap();
            drain(&mut encoder, true);
        }
        output.write_trailer().unwrap();
        drop(output);
        RecordingMeta {
            project_path: project.path().to_path_buf(),
            platform: None,
            pretty_name: "Owned Instant fixture".into(),
            sharing: None,
            inner: RecordingMetaInner::Instant(frameever_project::InstantRecordingMeta::Complete {
                fps: 30,
                sample_rate: None,
            }),
            upload: None,
        }
        .save_for_project()
        .unwrap();
        project
    }

    #[tokio::test]
    async fn instant_export_keeps_a_hard_link_to_the_recording() {
        let project = playable_instant_project();
        let source = project.path().join("content/output.mp4");
        let before = std::fs::read(&source).unwrap();
        let destination = project.path().join("export.mp4");
        std::fs::hard_link(&source, &destination).unwrap();

        assert_eq!(
            copy_instant_output(&source, destination.clone()).unwrap(),
            destination
        );

        assert_eq!(std::fs::read(&source).unwrap(), before);
        assert_eq!(std::fs::read(&destination).unwrap(), before);
    }

    #[tokio::test]
    async fn completed_instant_export_is_read_only_while_upload_owns_the_project() {
        let project = playable_instant_project();
        std::fs::create_dir(project.path().join("content/audio")).unwrap();
        let output = project.path().join("content/output.mp4");
        let before = std::fs::read(&output).unwrap();
        let _upload =
            frameever_recording::upload_resume::UploadLock::acquire(project.path()).unwrap();
        assert_eq!(
            prepare_instant_output(project.path().to_path_buf())
                .await
                .unwrap(),
            output
        );
        let exported = project.path().join("copy.mp4");
        assert_eq!(
            copy_instant_output(&output, exported.clone()).unwrap(),
            exported
        );
        assert_eq!(std::fs::read(&output).unwrap(), before);
        assert_eq!(std::fs::read(&exported).unwrap(), before);
        assert!(!project.path().join(".recovery.lock").exists());
    }

    fn playable_studio_project() -> tempfile::TempDir {
        let project = playable_instant_project();
        std::fs::rename(
            project.path().join("content/output.mp4"),
            project.path().join("display.mp4"),
        )
        .unwrap();
        let audio = std::process::Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-ac",
                "2",
                "-c:a",
                "aac",
            ])
            .arg(project.path().join("mic.m4a"))
            .output()
            .unwrap();
        assert!(audio.status.success(), "{audio:?}");
        std::fs::write(
            project.path().join("recording-meta.json"),
            serde_json::to_vec(&serde_json::json!({
                "pretty_name": "CLI streaming test",
                "segments": [{
                    "display": {"path": "display.mp4", "fps": 30, "start_time": 0.0},
                    "mic": {"path": "mic.m4a", "start_time": 0.0}
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        frameever_project::ProjectConfiguration {
            timeline: Some(
                serde_json::from_value(serde_json::json!({
                    "segments": [{
                        "recordingSegment": 0, "start": 0.0, "end": 1.0, "timescale": 1.0
                    }],
                    "zoomSegments": []
                }))
                .unwrap(),
            ),
            ..Default::default()
        }
        .write(project.path())
        .unwrap();
        project
    }

    #[test]
    #[ignore = "requires ffmpeg"]
    fn tracks_follow_the_edit_into_separate_files() {
        let project = playable_studio_project();
        let output = project.path().join("tracks");
        let export = Export {
            project_path: project.path().to_path_buf(),
            output_path: None,
            output: Some(output.clone()),
            format: None,
            fps: Some(30),
            resolution: None,
            quality: None,
            codec: None,
            optimize_filesize: false,
            settings_json: None,
            force_ffmpeg_decoder: true,
            progress_json: false,
            completion_json: false,
            tracks: Some("all".into()),
        };
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(export.run(false))
            .unwrap();

        let mut names = std::fs::read_dir(&output)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, ["Microphone.wav", "Screen.mp4"]);
        let one_second_of_48k_stereo_24_bit = 44 + 48_000 * 2 * 3;
        assert_eq!(
            std::fs::metadata(output.join("Microphone.wav"))
                .unwrap()
                .len(),
            one_second_of_48k_stereo_24_bit
        );
        let mut screen = ffmpeg::format::input(&output.join("Screen.mp4")).unwrap();
        assert_eq!(screen.packets().count(), 30);
    }

    #[test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    fn studio_mp4_streams_audio_and_an_existing_destination_keeps_eager_preparation() {
        std::thread::Builder::new()
            .stack_size(crate::TOKIO_WORKER_THREAD_STACK_SIZE)
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(studio_mp4_streams_audio_and_keeps_an_existing_destination())
            })
            .unwrap()
            .join()
            .unwrap();
    }

    async fn studio_mp4_streams_audio_and_keeps_an_existing_destination() {
        let project = playable_studio_project();
        let output = project.path().join("render.mp4");
        let settings = settings_from_flags(&ExportFlags {
            fps: Some(30),
            resolution: Some("64x64".into()),
            force_ffmpeg_decoder: true,
            ..Default::default()
        })
        .unwrap();
        let builder = || {
            ExporterBase::builder(project.path().to_path_buf())
                .with_force_ffmpeg_decoder(true)
                .with_output_path(output.clone())
        };
        assert!(!output.exists());
        let prepared = settings.prepare(builder()).await.unwrap();
        assert!(matches!(&prepared, PreparedExport::Mp4(_, base) if base.uses_streaming_audio()));
        assert_eq!(prepared.total_frames(30), 30);
        assert_eq!(prepared.export(|_| true).await.unwrap(), output);
        let original = std::fs::read(&output).unwrap();
        assert!(!original.is_empty());
        let prepared = settings.prepare(builder()).await.unwrap();
        assert!(matches!(&prepared, PreparedExport::Mp4(_, base) if !base.uses_streaming_audio()));
        drop(prepared);
        assert_eq!(std::fs::read(&output).unwrap(), original);

        for format in [ExportFormat::Gif, ExportFormat::Mov] {
            let settings = settings_from_flags(&ExportFlags {
                format: Some(format),
                ..Default::default()
            })
            .unwrap();
            let prepared = settings.prepare(builder()).await.unwrap();
            assert!(matches!(
                prepared,
                PreparedExport::Gif(_, _) | PreparedExport::Mov(_, _)
            ));
        }
    }

    #[tokio::test]
    async fn instant_export_needing_repair_cannot_rewrite_an_owned_upload() {
        let project = playable_instant_project();
        std::fs::create_dir(project.path().join("content/audio")).unwrap();
        let output = project.path().join("content/output.mp4");
        std::fs::write(&output, b"broken media must remain available for repair").unwrap();
        let before = std::fs::read(&output).unwrap();
        let _upload =
            frameever_recording::upload_resume::UploadLock::acquire(project.path()).unwrap();
        let error = prepare_instant_output(project.path().to_path_buf())
            .await
            .unwrap_err();
        assert!(error.contains("Another upload owns this recording"));
        assert_eq!(std::fs::read(&output).unwrap(), before);
        assert!(!project.path().join(".recovery.lock").exists());
    }

    #[tokio::test]
    async fn instant_export_cannot_ignore_persisted_or_recorded_audio() {
        for intent in ["tauri-upload.json", "instant-upload.json", "audio-media"] {
            let project = playable_instant_project();
            let output = project.path().join("content/output.mp4");
            let before = std::fs::read(&output).unwrap();
            match intent {
                "tauri-upload.json" => {
                    std::fs::write(project.path().join(intent), br#"{"required_audio":true}"#)
                        .unwrap();
                }
                "instant-upload.json" => {
                    std::fs::write(project.path().join(intent), br#"{"requested_audio":true}"#)
                        .unwrap();
                }
                _ => {
                    let audio = project.path().join("content/audio");
                    std::fs::create_dir(&audio).unwrap();
                    std::fs::write(audio.join("segment_001.m4s"), b"retained audio source")
                        .unwrap();
                }
            }
            let _upload =
                frameever_recording::upload_resume::UploadLock::acquire(project.path()).unwrap();
            assert!(
                prepare_instant_output(project.path().to_path_buf())
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read(output).unwrap(), before);
        }
    }

    #[test]
    fn default_flags_produce_maximum_mp4_1080p60() {
        let settings = settings_from_flags(&ExportFlags::default()).unwrap();
        match settings {
            CliExportSettings::Mp4(s) => {
                assert_eq!(s.fps, 60);
                assert_eq!(s.resolution_base, XY::new(1920, 1080));
                assert!(matches!(
                    s.compression,
                    frameever_export::mp4::ExportCompression::Maximum
                ));
                assert!(!s.optimize_filesize);
            }
            _ => panic!("expected mp4 settings"),
        }
    }

    #[test]
    fn codec_flag_selects_hevc_for_mp4_and_is_rejected_elsewhere() {
        let settings = settings_from_flags(&ExportFlags {
            format: Some(ExportFormat::Mp4),
            codec: Some(CodecArg::Hevc),
            ..Default::default()
        })
        .unwrap();
        match settings {
            CliExportSettings::Mp4(s) => {
                assert_eq!(s.codec, frameever_export::mp4::ExportCodec::Hevc)
            }
            _ => panic!("expected mp4 settings"),
        }
        assert!(
            settings_from_flags(&ExportFlags {
                format: Some(ExportFormat::Gif),
                codec: Some(CodecArg::Hevc),
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn gif_default_fps_is_30() {
        let settings = settings_from_flags(&ExportFlags {
            format: Some(ExportFormat::Gif),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(settings.fps(), 30);
        assert!(matches!(settings, CliExportSettings::Gif(_)));
    }

    #[test]
    fn resolution_and_fps_overrides_apply() {
        let settings = settings_from_flags(&ExportFlags {
            format: Some(ExportFormat::Mp4),
            fps: Some(24),
            resolution: Some("1280x720".to_string()),
            quality: Some(QualityArg::Web),
            ..Default::default()
        })
        .unwrap();
        match settings {
            CliExportSettings::Mp4(s) => {
                assert_eq!(s.fps, 24);
                assert_eq!(s.resolution_base, XY::new(1280, 720));
                assert!(matches!(
                    s.compression,
                    frameever_export::mp4::ExportCompression::Web
                ));
            }
            _ => panic!("expected mp4 settings"),
        }
    }

    #[test]
    fn parse_resolution_accepts_upper_and_lower_x() {
        assert_eq!(parse_resolution("800x600").unwrap(), XY::new(800, 600));
        assert_eq!(parse_resolution("800X600").unwrap(), XY::new(800, 600));
    }

    #[test]
    fn parse_resolution_rejects_garbage() {
        assert!(parse_resolution("nonsense").is_err());
        assert!(parse_resolution("1920x").is_err());
        assert!(parse_resolution("0x1080").is_err());
    }

    #[test]
    fn zero_fps_is_rejected() {
        let result = settings_from_flags(&ExportFlags {
            fps: Some(0),
            ..Default::default()
        });
        assert!(result.is_err());
    }

    #[test]
    fn quality_with_gif_is_rejected() {
        let result = settings_from_flags(&ExportFlags {
            format: Some(ExportFormat::Gif),
            quality: Some(QualityArg::Social),
            ..Default::default()
        });
        assert!(result.is_err());
    }

    #[test]
    fn quality_with_mov_is_rejected() {
        let result = settings_from_flags(&ExportFlags {
            format: Some(ExportFormat::Mov),
            quality: Some(QualityArg::Social),
            ..Default::default()
        });
        assert!(result.is_err());
    }

    #[test]
    fn settings_json_rejects_zero_fps_and_resolution_like_the_flags() {
        let json = |format: &str, fps: u32, x: u32, y: u32| {
            format!(
                r#"{{"format":"{format}","fps":{fps},"resolution_base":{{"x":{x},"y":{y}}},"compression":"Maximum","custom_bpp":null,"quality":null,"cursor_only":false}}"#
            )
        };
        for format in ["Mp4", "Gif", "Mov"] {
            assert!(
                settings_from_json(&json(format, 30, 1280, 720)).is_ok(),
                "{format}"
            );
            assert!(
                settings_from_json(&json(format, 0, 1280, 720)).is_err(),
                "{format}"
            );
            assert!(
                settings_from_json(&json(format, 30, 0, 0)).is_err(),
                "{format}"
            );
            assert!(
                settings_from_json(&json(format, 30, 1280, 0)).is_err(),
                "{format}"
            );
        }
    }

    #[test]
    fn settings_json_accepts_lowercase_and_pascalcase_format_tags() {
        let lower = r#"{"format":"mp4","fps":30,"resolution_base":{"x":1280,"y":720},"compression":"Web","custom_bpp":null}"#;
        assert!(matches!(
            serde_json::from_str::<CliExportSettings>(lower).unwrap(),
            CliExportSettings::Mp4(_)
        ));
        let pascal = lower.replace("\"mp4\"", "\"Mp4\"");
        assert!(matches!(
            serde_json::from_str::<CliExportSettings>(&pascal).unwrap(),
            CliExportSettings::Mp4(_)
        ));
    }

    #[test]
    fn optimize_filesize_only_for_mp4() {
        assert!(
            settings_from_flags(&ExportFlags {
                format: Some(ExportFormat::Mov),
                optimize_filesize: true,
                ..Default::default()
            })
            .is_err()
        );
    }
}
