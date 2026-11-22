use std::{borrow::Cow, path::PathBuf, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD};
use frameever_editor::EditorInstance;
use frameever_project::{
    CursorEvents, ProjectConfiguration, RecordingMeta, TimelineFrameMapping, XY,
};
use frameever_rendering::{
    FrameRenderer, PrecomputedCursorTimeline, ProjectUniforms, RecordingSegmentDecoders,
    RenderQuality, RenderVideoConstants, RenderedFrame, RendererLayers, TransitionRenderInput,
    ZoomTransformTimeline,
};
use image::{
    ImageFormat, Rgba, RgbaImage,
    codecs::jpeg::JpegEncoder,
    flat::{FlatSamples, SampleLayout},
};
use serde::{Deserialize, Serialize};

use crate::{
    ExportError, ExporterBase, ExporterBuilder,
    gif::{DEFAULT_GIF_QUALITY, GifQuality},
    make_cursor_only_project, synthesize_default_timeline,
};

const GIF_PREVIEW_JPEG_QUALITY: u8 = 95;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExportPreviewSettings {
    pub fps: u32,
    pub resolution_base: XY<u32>,
    pub compression_bpp: f32,
    #[serde(default)]
    pub cursor_only: bool,
    #[serde(default)]
    pub gif_quality: Option<GifQuality>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportPreviewResult {
    pub jpeg_base64: String,
    pub estimated_size_mb: f64,
    pub actual_width: u32,
    pub actual_height: u32,
    pub frame_render_time_ms: f64,
    pub total_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StillFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub async fn render_preview(
    project_path: PathBuf,
    frame_time: f64,
    settings: ExportPreviewSettings,
    force_ffmpeg_decoder: bool,
) -> Result<ExportPreviewResult, ExportError> {
    let mut exporter_builder =
        ExporterBase::builder(project_path.clone()).with_force_ffmpeg_decoder(force_ffmpeg_decoder);

    if settings.cursor_only {
        let meta = RecordingMeta::load_for_project(&project_path)
            .map_err(|e| ExportError::Other(format!("Failed to load recording meta: {e}")))?;
        exporter_builder =
            exporter_builder.with_config(make_cursor_only_project(meta.project_config()));
    }

    let exporter_base = exporter_builder
        .build_video_only()
        .await
        .map_err(|e| ExportError::Other(format!("Exporter build error: {e}")))?;

    render_preview_with_base(exporter_base, frame_time, settings).await
}

fn preview_builder_with_config(
    project_path: PathBuf,
    project_config: ProjectConfiguration,
    settings: ExportPreviewSettings,
    force_ffmpeg_decoder: bool,
) -> ExporterBuilder {
    let project_config = if settings.cursor_only {
        make_cursor_only_project(project_config)
    } else {
        project_config
    };
    ExporterBase::builder(project_path)
        .with_config(project_config)
        .with_force_ffmpeg_decoder(force_ffmpeg_decoder)
}

pub async fn render_preview_with_config(
    project_path: PathBuf,
    project_config: ProjectConfiguration,
    frame_time: f64,
    settings: ExportPreviewSettings,
    force_ffmpeg_decoder: bool,
) -> Result<ExportPreviewResult, ExportError> {
    let exporter_base =
        preview_builder_with_config(project_path, project_config, settings, force_ffmpeg_decoder)
            .build_video_only()
            .await
            .map_err(|error| ExportError::Other(format!("Exporter build error: {error}")))?;
    render_preview_with_base(exporter_base, frame_time, settings).await
}

/// Everything a preview frame needs, borrowed from whichever owner already
/// holds the decoders and GPU pipelines.
struct PreviewSource<'a> {
    project_config: Cow<'a, ProjectConfiguration>,
    render_constants: &'a RenderVideoConstants,
    segments: Vec<PreviewSegment<'a>>,
    total_duration: f64,
}

struct PreviewSegment<'a> {
    decoders: &'a RecordingSegmentDecoders,
    cursor: &'a CursorEvents,
    hover_changes_ms: Arc<[f64]>,
}

async fn render_preview_with_base(
    exporter_base: ExporterBase,
    frame_time: f64,
    settings: ExportPreviewSettings,
) -> Result<ExportPreviewResult, ExportError> {
    render_preview_frame(base_source(&exporter_base), frame_time, settings).await
}

fn base_source(exporter_base: &ExporterBase) -> PreviewSource<'_> {
    PreviewSource {
        project_config: Cow::Borrowed(&exporter_base.project_config),
        render_constants: &exporter_base.render_constants,
        segments: exporter_base
            .segments
            .iter()
            .map(|segment| PreviewSegment {
                decoders: &segment.decoders,
                cursor: &segment.cursor,
                hover_changes_ms: segment.hover_changes.times_ms(),
            })
            .collect(),
        total_duration: frameever_rendering::get_duration(
            &exporter_base.recordings,
            &exporter_base.recording_meta,
            &exporter_base.studio_meta,
            &exporter_base.project_config,
        ),
    }
}

pub async fn render_still_with_config(
    project_path: PathBuf,
    project_config: ProjectConfiguration,
    frame_time: f64,
    fps: u32,
    force_ffmpeg_decoder: bool,
) -> Result<StillFrame, ExportError> {
    let exporter_base = ExporterBase::builder(project_path)
        .with_config(project_config)
        .with_force_ffmpeg_decoder(force_ffmpeg_decoder)
        .build()
        .await
        .map_err(|error| ExportError::Other(format!("Exporter build error: {error}")))?;
    render_still(&base_source(&exporter_base), frame_time, fps).await
}

pub async fn render_still_with_editor(
    editor: &EditorInstance,
    project_config: ProjectConfiguration,
    frame_time: f64,
    fps: u32,
) -> Result<StillFrame, ExportError> {
    let source = editor_source(editor, project_config, false)?;
    let _pause_prefetch = EditorPreviewGuard::new(editor);
    render_still(&source, frame_time, fps).await
}

async fn render_still(
    source: &PreviewSource<'_>,
    frame_time: f64,
    fps: u32,
) -> Result<StillFrame, ExportError> {
    let (width, height) =
        ProjectUniforms::get_base_size(&source.render_constants.options, &source.project_config);
    let frame = render_source_frame(source, frame_time, fps, XY::new(width, height), false).await?;
    still_from_rendered(&frame)
}

pub fn still_from_rendered(frame: &RenderedFrame) -> Result<StillFrame, ExportError> {
    let (row_bytes, stride) = checked_layout(frame)?;
    let rgba = frame
        .data
        .chunks(stride)
        .take(frame.height as usize)
        .flat_map(|row| &row[..row_bytes])
        .copied()
        .collect();
    Ok(StillFrame {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}

/// Render the preview through an open editor's decoders and GPU pipelines
/// instead of building a fresh exporter (a new wgpu device, every shader, and
/// a cold decoder per recording segment) for each frame. This is what makes the
/// export page's preview respond in one frame's worth of work rather than
/// seconds, and it is the same path the Tauri editor's fast preview takes.
pub async fn render_preview_with_editor(
    editor: &EditorInstance,
    project_config: ProjectConfiguration,
    frame_time: f64,
    settings: ExportPreviewSettings,
) -> Result<ExportPreviewResult, ExportError> {
    let source = editor_source(editor, project_config, settings.cursor_only)?;
    let _pause_prefetch = EditorPreviewGuard::new(editor);
    render_preview_frame(source, frame_time, settings).await
}

fn editor_source(
    editor: &EditorInstance,
    project_config: ProjectConfiguration,
    cursor_only: bool,
) -> Result<PreviewSource<'_>, ExportError> {
    let recording_meta = editor.meta();
    let studio_meta = recording_meta
        .studio_meta()
        .ok_or_else(|| ExportError::Other("Cannot preview non-studio recordings".to_string()))?;
    let mut project_config = project_config;
    if cursor_only {
        project_config = make_cursor_only_project(project_config);
    }
    synthesize_default_timeline(&mut project_config, &editor.recordings);
    frameever_project::synchronize_legacy_keyboard(recording_meta, &mut project_config);
    frameever_project::synchronize_captions(
        &mut project_config,
        &editor
            .recordings
            .segments
            .iter()
            .map(|segment| segment.display.duration)
            .collect::<Vec<_>>(),
    );
    let total_duration = frameever_rendering::get_duration(
        &editor.recordings,
        recording_meta,
        studio_meta,
        &project_config,
    );

    Ok(PreviewSource {
        project_config: Cow::Owned(project_config),
        render_constants: &editor.render_constants,
        segments: editor
            .segment_medias
            .iter()
            .map(|segment| PreviewSegment {
                decoders: &segment.decoders,
                cursor: &segment.cursor,
                hover_changes_ms: segment.hover_changes.times_ms(),
            })
            .collect(),
        total_duration,
    })
}

/// Holds the editor's `export_preview_active` flag so its idle prefetch does
/// not compete with the preview for the decoders.
struct EditorPreviewGuard<'a>(&'a EditorInstance);

impl<'a> EditorPreviewGuard<'a> {
    fn new(editor: &'a EditorInstance) -> Self {
        editor
            .export_preview_active
            .store(true, std::sync::atomic::Ordering::Release);
        Self(editor)
    }
}

impl Drop for EditorPreviewGuard<'_> {
    fn drop(&mut self) {
        self.0
            .export_preview_active
            .store(false, std::sync::atomic::Ordering::Release);
    }
}

async fn render_preview_frame(
    source: PreviewSource<'_>,
    frame_time: f64,
    settings: ExportPreviewSettings,
) -> Result<ExportPreviewResult, ExportError> {
    let render_start = std::time::Instant::now();
    let frame = render_source_frame(
        &source,
        frame_time,
        settings.fps,
        settings.resolution_base,
        settings.cursor_only,
    )
    .await?;
    let total_duration = source.total_duration;
    let frame_render_time_ms = render_start.elapsed().as_secs_f64() * 1000.0;
    let width = frame.width;
    let height = frame.height;

    let jpeg_buffer = match settings.gif_quality.filter(|_| !settings.cursor_only) {
        Some(quality) => match encode_gif_preview_jpeg(&frame, quality) {
            Ok(jpeg) => jpeg,
            Err(error) => {
                tracing::warn!(%error, "GIF preview fell back to the unquantized frame");
                encode_preview_jpeg(&frame, GIF_PREVIEW_JPEG_QUALITY)?
            }
        },
        None => encode_preview_jpeg(&frame, bpp_to_jpeg_quality(settings.compression_bpp))?,
    };
    drop(frame);

    let duration_seconds = total_duration;
    let fps_f64 = settings.fps as f64;
    let total_frames = (duration_seconds * fps_f64).ceil() as u32;
    let total_pixels = (settings.resolution_base.x * settings.resolution_base.y) as f64;
    let estimated_size_mb = if settings.cursor_only {
        let total_frames_f64 = (duration_seconds * fps_f64).ceil();
        estimate_cursor_only_size_mb(total_pixels, total_frames_f64)
    } else {
        let effective_fps = ((fps_f64 - 30.0).max(0.0) * 0.6) + fps_f64.min(30.0);
        let video_bitrate = total_pixels * settings.compression_bpp as f64 * effective_fps;
        let audio_bitrate = 192_000.0;
        let total_bitrate = video_bitrate + audio_bitrate;
        let encoder_efficiency = 0.5;
        (total_bitrate * encoder_efficiency * duration_seconds) / (8.0 * 1024.0 * 1024.0)
    };

    Ok(ExportPreviewResult {
        jpeg_base64: STANDARD.encode(&jpeg_buffer),
        estimated_size_mb,
        actual_width: width,
        actual_height: height,
        frame_render_time_ms,
        total_frames,
    })
}

async fn render_source_frame(
    source: &PreviewSource<'_>,
    frame_time: f64,
    fps: u32,
    resolution_base: XY<u32>,
    cursor_only: bool,
) -> Result<RenderedFrame, ExportError> {
    let project_config: &ProjectConfiguration = &source.project_config;
    let render_constants = source.render_constants;
    let segments = &source.segments;
    let total_duration = source.total_duration;
    let transition_mapping = project_config.timeline.as_ref().and_then(|timeline| {
        if timeline.transitions.is_empty() {
            return None;
        }
        match timeline.get_frame_mapping(frame_time) {
            Some(TimelineFrameMapping::Transition {
                outgoing,
                kind,
                progress,
                ..
            }) => Some((outgoing, kind, progress)),
            _ => None,
        }
    });
    let Some((segment_time, segment)) = project_config.get_segment_time(frame_time) else {
        return Err(ExportError::Other(
            "Frame time is outside video duration".to_string(),
        ));
    };

    let segment_media = segments
        .get(segment.recording_clip as usize)
        .ok_or_else(|| ExportError::Other("Recording clip is unavailable".to_string()))?;
    let clip_config = project_config
        .clips
        .iter()
        .find(|v| v.index == segment.recording_clip);

    let segment_frames = segment_media
        .decoders
        .get_frames(
            segment_time as f32,
            project_config.requires_camera() && !cursor_only,
            !cursor_only,
            clip_config.map(|v| v.offsets).unwrap_or_default(),
        )
        .await
        .ok_or_else(|| ExportError::Other("Failed to decode frame".to_string()))?;

    let frame_number = (frame_time * fps as f64).floor() as u32;

    let mut zoom_timeline = ZoomTransformTimeline::from_project_for_clip(
        project_config,
        segment_media.cursor,
        total_duration,
        render_constants.options.screen_size,
        segment.recording_clip,
    );
    zoom_timeline.ensure_precomputed_until((frame_number as f32 + 1.0) / fps as f32);

    let cursor_timeline = PrecomputedCursorTimeline::from_project(
        segment_media.cursor,
        Arc::clone(&segment_media.hover_changes_ms),
        project_config,
    );
    let uniforms = ProjectUniforms::new_with_precomputed_cursor(
        render_constants,
        project_config,
        frame_number,
        fps,
        resolution_base,
        segment_media.cursor,
        &segment_frames,
        total_duration,
        &zoom_timeline,
        &cursor_timeline,
    )
    .with_quality(RenderQuality::Export);

    let mut frame_renderer = FrameRenderer::new(render_constants);
    let mut layers = RendererLayers::new_with_options(
        &render_constants.device,
        &render_constants.queue,
        render_constants.is_software_adapter,
    );

    if let Some((outgoing, kind, progress)) = transition_mapping {
        let outgoing_media = segments
            .get(outgoing.segment.recording_clip as usize)
            .ok_or_else(|| {
                ExportError::Other("Outgoing recording clip is unavailable".to_string())
            })?;
        let outgoing_offsets = project_config
            .clips
            .iter()
            .find(|clip| clip.index == outgoing.segment.recording_clip)
            .map(|clip| clip.offsets)
            .unwrap_or_default();
        let outgoing_decoders = if outgoing.segment.recording_clip == segment.recording_clip {
            outgoing_media.decoders.transition_side().await
        } else {
            outgoing_media.decoders
        };
        let outgoing_frames = outgoing_decoders
            .get_frames(
                outgoing.source_time as f32,
                project_config.requires_camera() && !cursor_only,
                !cursor_only,
                outgoing_offsets,
            )
            .await
            .ok_or_else(|| ExportError::Other("Failed to decode outgoing frame".to_string()))?;
        let mut outgoing_zoom = ZoomTransformTimeline::from_project_for_outgoing_clip(
            project_config,
            outgoing_media.cursor,
            total_duration,
            render_constants.options.screen_size,
            outgoing.segment.recording_clip,
        );
        outgoing_zoom.ensure_precomputed_until((frame_number as f32 + 1.0) / fps as f32);
        let outgoing_cursor_timeline = (outgoing.segment.recording_clip != segment.recording_clip)
            .then(|| {
                PrecomputedCursorTimeline::from_project(
                    outgoing_media.cursor,
                    Arc::clone(&outgoing_media.hover_changes_ms),
                    project_config,
                )
            });
        let outgoing_uniforms = ProjectUniforms::new_with_precomputed_cursor(
            render_constants,
            project_config,
            frame_number,
            fps,
            resolution_base,
            outgoing_media.cursor,
            &outgoing_frames,
            total_duration,
            &outgoing_zoom,
            outgoing_cursor_timeline
                .as_ref()
                .unwrap_or(&cursor_timeline),
        )
        .with_quality(RenderQuality::Export);

        frame_renderer
            .render_transition_immediate(
                TransitionRenderInput {
                    segment_frames: outgoing_frames,
                    uniforms: outgoing_uniforms,
                    cursor: outgoing_media.cursor,
                    render_display: !cursor_only,
                },
                TransitionRenderInput {
                    segment_frames,
                    uniforms,
                    cursor: segment_media.cursor,
                    render_display: !cursor_only,
                },
                kind,
                progress as f32,
                &mut layers,
            )
            .await
            .map_err(ExportError::from)
    } else {
        frame_renderer
            .render_immediate(
                segment_frames,
                uniforms,
                segment_media.cursor,
                !cursor_only,
                &mut layers,
            )
            .await
            .map_err(ExportError::from)
    }
}

fn estimate_cursor_only_size_mb(total_pixels: f64, total_frames: f64) -> f64 {
    let bytes_per_frame = total_pixels * 0.4;
    (bytes_per_frame * total_frames) / (1024.0 * 1024.0)
}

fn bpp_to_jpeg_quality(bpp: f32) -> u8 {
    ((bpp - 0.04) / (0.3 - 0.04) * (95.0 - 40.0) + 40.0).clamp(40.0, 95.0) as u8
}

fn checked_layout(frame: &RenderedFrame) -> Result<(usize, usize), ExportError> {
    let width = usize::try_from(frame.width)
        .map_err(|_| ExportError::Other("Preview frame width is too large".to_string()))?;
    let height = usize::try_from(frame.height)
        .map_err(|_| ExportError::Other("Preview frame height is too large".to_string()))?;
    if width == 0 || height == 0 {
        return Err(ExportError::Other(
            "Preview frame dimensions must be non-zero".to_string(),
        ));
    }

    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| ExportError::Other("Preview frame row is too large".to_string()))?;
    let height_stride = usize::try_from(frame.padded_bytes_per_row)
        .map_err(|_| ExportError::Other("Preview frame row stride is too large".to_string()))?;
    if height_stride < row_bytes {
        return Err(ExportError::Other("Preview frame rows overlap".to_string()));
    }
    let required_len = height_stride
        .checked_mul(height - 1)
        .and_then(|offset| offset.checked_add(row_bytes))
        .ok_or_else(|| ExportError::Other("Preview frame buffer is too large".to_string()))?;
    if frame.data.len() < required_len {
        return Err(ExportError::Other(
            "Preview frame buffer is shorter than its layout".to_string(),
        ));
    }
    Ok((row_bytes, height_stride))
}

fn encode_gif_preview_jpeg(
    frame: &RenderedFrame,
    quality: GifQuality,
) -> Result<Vec<u8>, ExportError> {
    let image = gif_quantized_frame(frame, quality)?;
    let mut jpeg_buffer = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg_buffer, GIF_PREVIEW_JPEG_QUALITY)
        .encode_image(&image)
        .map_err(|e| ExportError::Other(format!("Failed to encode JPEG: {e}")))?;
    Ok(jpeg_buffer)
}

fn gif_quantized_frame(
    frame: &RenderedFrame,
    quality: GifQuality,
) -> Result<RgbaImage, ExportError> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let stride = frame.padded_bytes_per_row as usize;
    let row_bytes = width.saturating_mul(4);
    let required_len = stride
        .checked_mul(height.saturating_sub(1))
        .and_then(|offset| offset.checked_add(row_bytes));
    if width == 0
        || height == 0
        || stride < row_bytes
        || required_len.is_none_or(|required| frame.data.len() < required)
    {
        return Err(ExportError::Other(
            "Preview frame buffer does not match its layout".to_string(),
        ));
    }

    let mut pixels = Vec::with_capacity(width * height);
    for row in frame.data.chunks(stride).take(height) {
        pixels.extend(
            row[..row_bytes]
                .chunks_exact(4)
                .map(|pixel| rgb::RGBA8::new(pixel[0], pixel[1], pixel[2], pixel[3])),
        );
    }

    let (collector, writer) = gifski::new(gifski::Settings {
        width: Some(frame.width),
        height: Some(frame.height),
        quality: quality.quality.unwrap_or(DEFAULT_GIF_QUALITY),
        fast: quality.fast.unwrap_or(false),
        repeat: gifski::Repeat::Infinite,
    })
    .map_err(|e| ExportError::Other(format!("Failed to create GIF preview encoder: {e}")))?;

    let mut gif = Vec::new();
    std::thread::scope(|scope| {
        let written = scope.spawn(|| writer.write(&mut gif, &mut gifski::progress::NoProgress {}));
        let added = collector.add_frame_rgba(0, imgref::Img::new(pixels, width, height), 0.0);
        drop(collector);
        let written = written
            .join()
            .map_err(|_| ExportError::Other("GIF preview encoder panicked".to_string()))?;
        added
            .and(written)
            .map_err(|e| ExportError::Other(format!("Failed to encode GIF preview: {e}")))
    })?;

    image::load_from_memory_with_format(&gif, ImageFormat::Gif)
        .map(|image| image.into_rgba8())
        .map_err(|e| ExportError::Other(format!("Failed to decode GIF preview: {e}")))
}

fn encode_preview_jpeg(frame: &RenderedFrame, quality: u8) -> Result<Vec<u8>, ExportError> {
    let (_, height_stride) = checked_layout(frame)?;
    let samples = FlatSamples {
        samples: frame.data.as_ref().as_slice(),
        layout: SampleLayout {
            channels: 4,
            channel_stride: 1,
            width: frame.width,
            width_stride: 4,
            height: frame.height,
            height_stride,
        },
        color_hint: None,
    };
    let view = samples
        .as_view::<Rgba<u8>>()
        .map_err(|e| ExportError::Other(format!("Invalid preview frame layout: {e}")))?;

    let mut jpeg_buffer = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buffer, quality);
    encoder
        .encode_image(&view)
        .map_err(|e| ExportError::Other(format!("Failed to encode JPEG: {e}")))?;
    Ok(jpeg_buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: u32, height: u32, stride: usize, mut state: u64) -> RenderedFrame {
        let mut data = vec![0xa5; stride * height as usize];
        for y in 0..height as usize {
            for x in 0..width as usize {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let offset = y * stride + x * 4;
                data[offset] = (state >> 56) as u8;
                data[offset + 1] = (state >> 48) as u8;
                data[offset + 2] = (state >> 40) as u8;
                data[offset + 3] = (state >> 32) as u8;
            }
        }
        RenderedFrame {
            data: Arc::new(data),
            width,
            height,
            padded_bytes_per_row: stride as u32,
            frame_number: 0,
            target_time_ns: 0,
        }
    }

    fn legacy_jpeg(frame: &RenderedFrame, quality: u8) -> Vec<u8> {
        let rgb_data: Vec<u8> = frame
            .data
            .chunks(frame.padded_bytes_per_row as usize)
            .flat_map(|row| {
                row[..(frame.width * 4) as usize]
                    .chunks(4)
                    .flat_map(|chunk| [chunk[0], chunk[1], chunk[2]])
            })
            .collect();
        let mut jpeg_buffer = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buffer, quality);
        encoder
            .encode(
                &rgb_data,
                frame.width,
                frame.height,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        jpeg_buffer
    }

    #[test]
    fn flat_samples_jpeg_matches_legacy_bytes() {
        for (width, height, stride) in [(1, 1, 4), (13, 9, 64)] {
            let frame = frame(width, height, stride, 0x1234_5678_9abc_def0);
            for quality in [40, 70, 95] {
                assert_eq!(
                    encode_preview_jpeg(&frame, quality).unwrap(),
                    legacy_jpeg(&frame, quality),
                    "{width}x{height} stride={stride} quality={quality}"
                );
            }
        }

        let padded = frame(13, 9, 64, 0x1234_5678_9abc_def0);
        let required_len = 64 * (9 - 1) + 13 * 4;
        let mut without_trailing_padding = padded.clone();
        without_trailing_padding.data = Arc::new(padded.data[..required_len].to_vec());
        for quality in [40, 70, 95] {
            assert_eq!(
                encode_preview_jpeg(&without_trailing_padding, quality).unwrap(),
                legacy_jpeg(&without_trailing_padding, quality),
                "13x9 stride=64 without trailing padding quality={quality}"
            );
        }
    }

    #[test]
    fn a_still_keeps_every_pixel_and_drops_the_row_padding() {
        let padded = RenderedFrame {
            data: Arc::new(vec![
                1, 2, 3, 4, 5, 6, 7, 8, 0xee, 0xee, 9, 10, 11, 12, 13, 14, 15, 16,
            ]),
            width: 2,
            height: 2,
            padded_bytes_per_row: 10,
            frame_number: 0,
            target_time_ns: 0,
        };

        assert_eq!(
            still_from_rendered(&padded).unwrap(),
            StillFrame {
                width: 2,
                height: 2,
                rgba: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            }
        );

        let mut short = padded;
        short.data = Arc::new(vec![0; 17]);
        assert!(still_from_rendered(&short).is_err());
    }

    #[test]
    fn gif_preview_keeps_the_frame_size_and_uses_a_gif_palette() {
        for (width, height, stride) in [(48, 32, 48 * 4), (33, 17, 256)] {
            let frame = frame(width, height, stride, 7);
            for quality in [None, Some(70), Some(50)] {
                let quality = GifQuality {
                    quality,
                    fast: None,
                };
                let image = gif_quantized_frame(&frame, quality).unwrap();
                assert_eq!((image.width(), image.height()), (width, height));
                let colors: std::collections::HashSet<[u8; 4]> =
                    image.pixels().map(|pixel| pixel.0).collect();
                assert!(colors.len() <= 256, "{} colors", colors.len());
                assert!(!encode_gif_preview_jpeg(&frame, quality).unwrap().is_empty());
            }
        }
    }

    #[test]
    fn gif_preview_rejects_a_frame_that_does_not_match_its_layout() {
        let mut short = frame(8, 8, 32, 3);
        short.data = Arc::new(vec![0; 16]);
        let quality = GifQuality {
            quality: None,
            fast: None,
        };
        assert!(gif_quantized_frame(&short, quality).is_err());
        let mut overlapping = frame(8, 8, 32, 3);
        overlapping.padded_bytes_per_row = 16;
        assert!(gif_quantized_frame(&overlapping, quality).is_err());
    }

    #[test]
    fn rejects_overlapping_rows_and_short_buffers() {
        let mut overlapping = frame(13, 9, 64, 1);
        overlapping.padded_bytes_per_row = 51;
        assert!(encode_preview_jpeg(&overlapping, 70).is_err());

        let valid = frame(13, 9, 64, 2);
        let mut short = valid.clone();
        let required_len = 64 * (9 - 1) + 13 * 4;
        short.data = Arc::new(vec![0; required_len - 1]);
        assert!(encode_preview_jpeg(&short, 70).is_err());

        let mut empty = valid;
        empty.width = 0;
        assert!(encode_preview_jpeg(&empty, 70).is_err());
    }
}

#[cfg(test)]
mod subtitle_preview_tests {
    use super::*;

    #[test]
    fn immediate_export_preview_keeps_enabled_captions_regardless_of_legacy_export_toggle() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("project-config.json");
        for export in [false, true] {
            let current = ProjectConfiguration {
                captions: Some(frameever_project::CaptionsData {
                    settings: frameever_project::CaptionSettings {
                        enabled: true,
                        export_with_subtitles: export,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                ..Default::default()
            };
            let original = serde_json::to_value(&current).unwrap();
            let mut stale = current.clone();
            stale
                .captions
                .as_mut()
                .unwrap()
                .settings
                .export_with_subtitles = !export;
            let stale_bytes = serde_json::to_vec(&stale).unwrap();
            std::fs::write(&path, &stale_bytes).unwrap();
            let mut builder = preview_builder_with_config(
                temp.path().to_path_buf(),
                current.clone(),
                ExportPreviewSettings {
                    fps: 30,
                    resolution_base: XY::new(1280, 720),
                    compression_bpp: 0.15,
                    cursor_only: false,
                    gif_quality: None,
                },
                true,
            );
            let preview = builder.load_project_config().unwrap();
            assert!(preview.captions.as_ref().unwrap().settings.enabled);
            assert_eq!(
                preview.captions.unwrap().settings.export_with_subtitles,
                export
            );
            assert_eq!(std::fs::read(&path).unwrap(), stale_bytes);
            assert_eq!(serde_json::to_value(current).unwrap(), original);
            assert!(builder.force_ffmpeg_decoder);
        }
    }

    #[test]
    fn current_config_export_preview_preserves_cursor_only_override() {
        let current = ProjectConfiguration {
            captions: Some(frameever_project::CaptionsData {
                settings: frameever_project::CaptionSettings {
                    enabled: true,
                    export_with_subtitles: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut builder = preview_builder_with_config(
            PathBuf::from("unused-current-config-project"),
            current,
            ExportPreviewSettings {
                fps: 30,
                resolution_base: XY::new(1280, 720),
                compression_bpp: 0.15,
                cursor_only: true,
                gif_quality: None,
            },
            false,
        );
        assert!(builder.load_project_config().unwrap().captions.is_none());
    }
}

#[cfg(test)]
mod video_only_preview_tests {
    use super::*;

    fn studio_project() -> tempfile::TempDir {
        let project = tempfile::tempdir().unwrap();
        let generated = std::process::Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x64:rate=15:duration=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-map",
                "0:v",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-threads",
                "1",
            ])
            .arg(project.path().join("display.mp4"))
            .args(["-map", "1:a", "-c:a", "aac", "-ac", "2"])
            .arg(project.path().join("mic.m4a"))
            .output()
            .unwrap();
        assert!(generated.status.success(), "{generated:?}");
        std::fs::write(
            project.path().join("recording-meta.json"),
            serde_json::to_vec(&serde_json::json!({
                "pretty_name": "Video-only preview test",
                "segments": [{
                    "display": {"path": "display.mp4", "fps": 15},
                    "mic": {"path": "mic.m4a"}
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        ProjectConfiguration::default()
            .write(project.path())
            .unwrap();
        project
    }

    fn settings(cursor_only: bool) -> ExportPreviewSettings {
        ExportPreviewSettings {
            fps: 15,
            resolution_base: XY::new(64, 64),
            compression_bpp: 0.2,
            cursor_only,
            gif_quality: None,
        }
    }

    fn assert_same_preview(actual: ExportPreviewResult, expected: &ExportPreviewResult) {
        assert_eq!(actual.jpeg_base64, expected.jpeg_base64);
        assert_eq!(actual.actual_width, expected.actual_width);
        assert_eq!(actual.actual_height, expected.actual_height);
        assert_eq!(actual.total_frames, expected.total_frames);
        assert_eq!(
            actual.estimated_size_mb.to_bits(),
            expected.estimated_size_mb.to_bits()
        );
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_still_follows_hover_highlights_like_an_export() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        let display = root.join("display.mp4");
        let generated = std::process::Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-f", "lavfi", "-i"])
            .arg("color=gray:size=320x180:rate=30:duration=2")
            .args(["-c:v", "libx264", "-preset", "ultrafast"])
            .arg(&display)
            .output()
            .unwrap();
        assert!(generated.status.success(), "{generated:?}");
        image::RgbaImage::from_pixel(24, 24, image::Rgba([255, 0, 0, 255]))
            .save(root.join("cursor.png"))
            .unwrap();
        let moves = (0..=125)
            .map(|step| {
                let time_ms = f64::from(step) * 16.0;
                serde_json::json!({
                    "active_modifiers": [],
                    "cursor_id": "arrow",
                    "time_ms": time_ms,
                    "x": 0.15 + 0.6 * ((time_ms - 400.0) / 150.0).clamp(0.0, 1.0),
                    "y": 0.5
                })
            })
            .collect::<Vec<_>>();
        std::fs::write(
            root.join("cursor.json"),
            serde_json::json!({ "clicks": [], "moves": moves }).to_string(),
        )
        .unwrap();
        std::fs::write(
            root.join("display.cursor-hover.json"),
            serde_json::json!({
                "version": 1,
                "displayBytes": std::fs::metadata(&display).unwrap().len(),
                "changesMs": [480.0]
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            root.join("recording-meta.json"),
            serde_json::json!({
                "pretty_name": "Hover still test",
                "segments": [{
                    "display": { "path": "display.mp4", "fps": 30 },
                    "cursor": "cursor.json"
                }],
                "cursors": { "arrow": { "imagePath": "cursor.png", "hotspot": { "x": 0.5, "y": 0.5 } } }
            })
            .to_string(),
        )
        .unwrap();

        let red_centre = |config: ProjectConfiguration| async move {
            let still = render_still_with_config(root.to_path_buf(), config, 0.48, 30, true)
                .await
                .unwrap();
            let (mut sum, mut count) = (0_usize, 0_usize);
            for (index, pixel) in still.rgba.chunks_exact(4).enumerate() {
                if pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60 {
                    sum += index % still.width as usize;
                    count += 1;
                }
            }
            assert!(count > 0, "the cursor is not in the still");
            sum as f64 / count as f64 / f64::from(still.width)
        };
        let mut following = ProjectConfiguration::default();
        following.cursor.use_svg = false;
        following.cursor.motion_blur = 0.0;
        following.cursor.hide_when_idle = false;
        following.cursor.rotation_amount = 0.0;
        following.background.padding = 0.0;
        following.background.source = frameever_project::BackgroundSource::Color {
            value: [0, 0, 0],
            alpha: 255,
        };
        following.screen_motion_blur = 0.0;
        following.cursor.hover_anchors = true;
        let mut ignoring = following.clone();
        ignoring.cursor.hover_anchors = false;

        let pointer = 0.15 + 0.6 * (480.0 - 400.0) / 150.0;
        let mut styled = ignoring.clone();
        styled.timeline = Some(
            serde_json::from_value(serde_json::json!({
                "segments": [{ "timescale": 1.0, "start": 0.0, "end": 2.0 }],
                "zoomSegments": [],
                "styleSegments": [{
                    "start": 0.0,
                    "end": 2.0,
                    "overrides": { "cursor": following.cursor }
                }]
            }))
            .unwrap(),
        );
        let followed = (red_centre(following).await - pointer).abs();
        let ignored = (red_centre(ignoring).await - pointer).abs();
        let styled = (red_centre(styled).await - pointer).abs();

        assert!(
            followed + 0.005 < ignored,
            "{followed} from the pointer with highlights, {ignored} without"
        );
        assert!(followed < 0.01, "{followed} from the pointer");
        assert!(
            styled < 0.01,
            "{styled} from the pointer in a style segment"
        );
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn video_only_previews_match_eager_jpeg_and_metadata() {
        let project = studio_project();
        for cursor_only in [false, true] {
            let config = ProjectConfiguration::load(project.path()).unwrap();
            let base = preview_builder_with_config(
                project.path().to_path_buf(),
                config.clone(),
                settings(cursor_only),
                true,
            )
            .build()
            .await
            .unwrap();
            let expected = render_preview_with_base(base, 0.5, settings(cursor_only))
                .await
                .unwrap();
            let actual = render_preview(
                project.path().to_path_buf(),
                0.5,
                settings(cursor_only),
                true,
            )
            .await
            .unwrap();
            assert_same_preview(actual, &expected);
            let actual = render_preview_with_config(
                project.path().to_path_buf(),
                config,
                0.5,
                settings(cursor_only),
                true,
            )
            .await
            .unwrap();
            assert_same_preview(actual, &expected);
        }
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg, ffprobe and a wgpu adapter"]
    async fn preview_does_not_decode_unused_corrupt_audio() {
        let project = studio_project();
        let audio = project.path().join("mic.m4a");
        let expected = render_preview(project.path().to_path_buf(), 0.5, settings(false), true)
            .await
            .unwrap();
        let packets = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_packets",
                "-show_entries",
                "packet=pts_time,pos,size",
                "-of",
                "json",
            ])
            .arg(&audio)
            .output()
            .unwrap();
        assert!(packets.status.success());
        let packets: serde_json::Value = serde_json::from_slice(&packets.stdout).unwrap();
        let packet = packets["packets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|packet| packet["pts_time"].as_str().unwrap().parse::<f64>().unwrap() > 1.5)
            .unwrap();
        let position: usize = packet["pos"].as_str().unwrap().parse().unwrap();
        let size: usize = packet["size"].as_str().unwrap().parse().unwrap();
        let mut bytes = std::fs::read(&audio).unwrap();
        bytes[position..position + size].fill(0xff);
        std::fs::write(audio, bytes).unwrap();
        let eager = ExporterBase::builder(project.path().to_path_buf())
            .with_force_ffmpeg_decoder(true)
            .build()
            .await;
        assert!(matches!(
            eager,
            Err(crate::ExporterBuildError::MediaLoad(_))
        ));
        let actual = render_preview(project.path().to_path_buf(), 0.5, settings(false), true)
            .await
            .unwrap();
        assert_same_preview(actual, &expected);
        let config = ProjectConfiguration::load(project.path()).unwrap();
        let actual = render_preview_with_config(
            project.path().to_path_buf(),
            config,
            0.5,
            settings(false),
            true,
        )
        .await
        .unwrap();
        assert_same_preview(actual, &expected);
    }
}
