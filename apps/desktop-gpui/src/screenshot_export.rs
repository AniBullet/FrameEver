//! Screenshot export compositing -- `screenshot-editor/screenshotExport.ts`
//! and the encode half of `useScreenshotExport.ts`, natively.
//!
//! The Tauri editor exports in two stages: the GPU renders the *styled*
//! screenshot (background, padding, shadow -- everything but annotations) at
//! export resolution, and a `<canvas>` pass then bakes the annotations in
//! (`renderScreenshotExportCanvas`, `screenshotExport.ts:250-338`). This
//! module is that canvas pass: scale the frame-space annotations up to the
//! export frame, re-apply the mask filters at export resolution, draw the
//! shapes with tiny-skia and the text with cosmic-text, and expand the output
//! to the union of the image and every annotation's painted extents.
//!
//! It also carries the share fingerprint (`screenshotProjectFingerprint`,
//! `:419-431`) and the per-destination encodings (`exportImage`,
//! `useScreenshotExport.ts:139-253`): PNG for Save, PNG-over-white for Copy,
//! and JPEG at 0.9 for Share unless the output actually needs its alpha.
//!
//! Everything here is pure CPU work on plain buffers -- the callers run it on
//! the background executor.

use std::sync::{Mutex, OnceLock};

use frameever_gpui_kernels::{codec, screenshot as pixels};
use frameever_project::{Annotation, AnnotationType, ProjectConfiguration};

use crate::i18n::t;
use crate::screenshot_annotations::{self as annotations, Rect};
use crate::screenshot_editor::has_no_visible_background;

/// One export-resolution GPU render, plus the size the *preview* renders the
/// same config at. Annotations live in preview-frame space
/// (`AnnotationLayer.tsx`'s viewBox), so the compositor's scale factors are
/// `export / base` -- exactly `canvas.width / frame.width` over there, where
/// `frame` is `latestFrame()` after `waitForSyncedPreview`.
pub struct RawFrame {
    /// Tight RGBA, straight alpha -- the wgpu row padding already stripped.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// What `ProjectUniforms::get_output_size` yields for the preview's own
    /// `get_base_size` resolution -- i.e. the dimensions the preview frame for
    /// this config actually has.
    pub base_width: u32,
    pub base_height: u32,
}

/// The composited output canvas -- straight-alpha RGBA.
pub struct Composited {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The output canvas rect before rounding: where the working canvas lands
/// inside it, and its final pixel size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub width: u32,
    pub height: u32,
}

// ---------------------------------------------------------------------------
// Annotation scaling (`scaleAnnotations`, `screenshotExport.ts:232-248`)
// ---------------------------------------------------------------------------

/// Scale every annotation from preview-frame space into export space: x/w by
/// the x factor, y/h by the y factor, and strokeWidth/maskLevel by the average
/// of the two. `points` are fractions of the stroke's own box, so they ride
/// along untouched.
pub fn scale_annotations(list: &[Annotation], scale_x: f64, scale_y: f64) -> Vec<Annotation> {
    let scalar = (scale_x + scale_y) / 2.;
    list.iter()
        .map(|annotation| Annotation {
            x: annotation.x * scale_x,
            y: annotation.y * scale_y,
            width: annotation.width * scale_x,
            height: annotation.height * scale_y,
            stroke_width: annotation.stroke_width * scalar,
            mask_level: annotation.mask_level.map(|level| level * scalar),
            ..annotation.clone()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The composite (`renderScreenshotExportCanvas`)
// ---------------------------------------------------------------------------

pub fn export_bounds(scaled: &[Annotation], canvas: (u32, u32)) -> ExportBounds {
    let mut min_x = 0f64;
    let mut min_y = 0f64;
    let mut max_x = f64::from(canvas.0);
    let mut max_y = f64::from(canvas.1);

    for annotation in scaled {
        if annotation.annotation_type == AnnotationType::Mask {
            continue;
        }
        let mut include = |x: f64, y: f64, width: f64, height: f64| {
            min_x = min_x.min(x);
            max_x = max_x.max(x + width);
            min_y = min_y.min(y);
            max_y = max_y.max(y + height);
        };
        let rect = annotations::normalized_rect(annotation);
        include(rect.x, rect.y, rect.width, rect.height);
        let has_stroke = annotations::parse_css_rgba(&annotation.stroke_color, annotation.opacity)
            .is_some_and(|(_, alpha)| alpha > 0.);
        let stroke_width = if has_stroke {
            annotation.stroke_width.max(0.)
        } else {
            0.
        };
        let radius = stroke_width / 2.;
        match annotation.annotation_type {
            AnnotationType::Rectangle | AnnotationType::Circle => include(
                rect.x - radius,
                rect.y - radius,
                rect.width + stroke_width,
                rect.height + stroke_width,
            ),
            AnnotationType::Arrow if has_stroke => {
                let end = (
                    annotation.x + annotation.width,
                    annotation.y + annotation.height,
                );
                let angle = (end.1 - annotation.y).atan2(end.0 - annotation.x);
                let head =
                    annotations::arrow_head_points(end.0, end.1, angle, annotation.stroke_width);
                for (x, y) in [(annotation.x, annotation.y), head.base] {
                    include(
                        f64::from(x as f32) - radius,
                        f64::from(y as f32) - radius,
                        stroke_width,
                        stroke_width,
                    );
                }
                for (x, y) in head.points {
                    include(f64::from(x as f32), f64::from(y as f32), 0., 0.);
                }
            }
            AnnotationType::Draw if stroke_width > 0. => {
                for [x, y] in annotations::draw_points(annotation) {
                    include(x - radius, y - radius, stroke_width, stroke_width);
                }
            }
            AnnotationType::Text => rasterize_text(annotation, |at, size, color| {
                if color[3] > 0 {
                    include(
                        at.0.round(),
                        at.1.round(),
                        f64::from(size.0),
                        f64::from(size.1),
                    );
                }
            }),
            AnnotationType::Mask | AnnotationType::Arrow | AnnotationType::Draw => {}
        }
    }

    let min_x = min_x.floor();
    let min_y = min_y.floor();
    ExportBounds {
        min_x,
        min_y,
        width: (max_x.ceil() - min_x).max(1.) as u32,
        height: (max_y.ceil() - min_y).max(1.) as u32,
    }
}

pub fn composite(raw: &RawFrame, config: &ProjectConfiguration) -> Composited {
    let width = raw.width.max(1);
    let height = raw.height.max(1);
    if config.annotations.is_empty() && raw.rgba.len() == width as usize * height as usize * 4 {
        let rgba = if has_no_visible_background(&config.background.source)
            || pixels::is_opaque(&raw.rgba)
        {
            raw.rgba.clone()
        } else {
            pixels::flatten_onto_white(&raw.rgba)
        };

        return Composited {
            rgba,
            width,
            height,
        };
    }
    let scale_x = f64::from(width) / f64::from(raw.base_width.max(1));
    let scale_y = f64::from(height) / f64::from(raw.base_height.max(1));
    let mut scaled = scale_annotations(&config.annotations, scale_x, scale_y);

    let mut canvas = raw.rgba.clone();

    // `applyMaskAnnotations(ctx, sourceCanvas, .., {x:0,y:0,w,h})` -- every
    // mask samples the untouched `raw.rgba`, so overlapping masks never
    // re-filter each other's output.
    let full = Rect {
        x: 0.,
        y: 0.,
        width: f64::from(width),
        height: f64::from(height),
    };
    for mask in &scaled {
        if mask.annotation_type != AnnotationType::Mask {
            continue;
        }
        if let Some((x0, y0, region)) =
            annotations::masked_region_image(&raw.rgba, (width, height), mask, full)
        {
            pixels::blit_over(&mut canvas, (width, height), &region, (x0, y0));
        }
    }

    let bounds = export_bounds(&scaled, (width, height));
    let fits_frame = bounds.min_x == 0.
        && bounds.min_y == 0.
        && bounds.width == width
        && bounds.height == height;
    if fits_frame {
        draw_annotations_onto(&mut canvas, width, height, &scaled);
    }
    let mut out = vec![0u8; bounds.width as usize * bounds.height as usize * 4];
    if !has_no_visible_background(&config.background.source) {
        out.fill(255);
    }
    let offset = (
        (-bounds.min_x).round() as i64,
        (-bounds.min_y).round() as i64,
    );
    pixels::blit_over_offset(
        &mut out,
        (bounds.width, bounds.height),
        &canvas,
        (width, height),
        offset,
    );
    if !fits_frame {
        for annotation in &mut scaled {
            annotation.x += offset.0 as f64;
            annotation.y += offset.1 as f64;
        }
        draw_annotations_onto(&mut out, bounds.width, bounds.height, &scaled);
    }

    Composited {
        rgba: out,
        width: bounds.width,
        height: bounds.height,
    }
}

// ---------------------------------------------------------------------------
// Transparency + encodings (`canvasNeedsTransparency`, `exportImage`)
// ---------------------------------------------------------------------------

/// `withWhiteBackground` (`useScreenshotExport.ts:18-28`).
pub fn flatten_onto_white(out: &Composited) -> Composited {
    Composited {
        rgba: pixels::flatten_onto_white(&out.rgba),
        width: out.width,
        height: out.height,
    }
}

/// The one PNG encode every destination shares -- and the Phase 1-2 "styled
/// frame straight to PNG" path, still callable through
/// `screenshot_editor::render_export_png`.
pub fn encode_rgba_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    codec::encode_png_rgba(rgba, width, height)
        .map_err(|e| t!("Failed to encode screenshot export: {error}", error = e))
}

fn encode_png(out: &Composited) -> Result<Vec<u8>, String> {
    encode_rgba_png(&out.rgba, out.width, out.height)
}

/// Copy: always PNG, composited over white when transparency is not needed
/// (`:183-198` -- `withWhiteBackground` only on the clipboard path).
pub fn encode_for_copy(out: &Composited, config: &ProjectConfiguration) -> Result<Vec<u8>, String> {
    if has_no_visible_background(&config.background.source) || pixels::is_opaque(&out.rgba) {
        encode_png(out)
    } else {
        encode_png(&flatten_onto_white(out))
    }
}

/// Save: PNG of the output canvas as it stands (`:198` -- no transparency
/// check and no white pass on the file path).
pub fn encode_for_save(out: &Composited) -> Result<Vec<u8>, String> {
    encode_png(out)
}

// ---------------------------------------------------------------------------
// The share fingerprint (`screenshotProjectFingerprint`)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The shape draw (`drawAnnotations`, `screenshotExport.ts:42-121`)
// ---------------------------------------------------------------------------

fn skia_color(rgb: [u8; 3], alpha: f32) -> Option<tiny_skia::Color> {
    tiny_skia::Color::from_rgba(
        f32::from(rgb[0]) / 255.,
        f32::from(rgb[1]) / 255.,
        f32::from(rgb[2]) / 255.,
        alpha.clamp(0., 1.),
    )
}

fn skia_paint(color: tiny_skia::Color) -> tiny_skia::Paint<'static> {
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    paint
}

/// `ctx.lineWidth` semantics: canvas' default join is miter with limit 10.
fn skia_stroke(width: f64, round: bool) -> tiny_skia::Stroke {
    tiny_skia::Stroke {
        width: width.max(0.) as f32,
        miter_limit: 10.,
        line_cap: if round {
            tiny_skia::LineCap::Round
        } else {
            tiny_skia::LineCap::Butt
        },
        line_join: if round {
            tiny_skia::LineJoin::Round
        } else {
            tiny_skia::LineJoin::Miter
        },
        dash: None,
    }
}

/// Draw every non-mask annotation onto the straight-alpha canvas, in list
/// order -- rect, ellipse, arrow, freehand with tiny-skia, text with
/// cosmic-text, each under its own `globalAlpha`.
fn draw_annotations_onto(canvas: &mut [u8], width: u32, height: u32, scaled: &[Annotation]) {
    if scaled
        .iter()
        .all(|annotation| annotation.annotation_type == AnnotationType::Mask)
    {
        return;
    }
    pixels::premultiply(canvas);
    if let Some(mut pixmap) = tiny_skia::PixmapMut::from_bytes(canvas, width, height) {
        for annotation in scaled {
            if annotation.annotation_type == AnnotationType::Mask {
                continue;
            }
            draw_one(&mut pixmap, annotation);
        }
    }
    pixels::demultiply(canvas);
}

fn draw_one(pixmap: &mut tiny_skia::PixmapMut<'_>, annotation: &Annotation) {
    let stroke_color = annotations::parse_css_rgba(&annotation.stroke_color, annotation.opacity);
    let fill_color = annotations::parse_css_rgba(&annotation.fill_color, annotation.opacity);
    let transform = tiny_skia::Transform::identity();

    match annotation.annotation_type {
        AnnotationType::Mask => {}
        AnnotationType::Rectangle => {
            let rect = annotations::normalized_rect(annotation);
            let Some(skia_rect) = tiny_skia::Rect::from_xywh(
                rect.x as f32,
                rect.y as f32,
                (rect.width as f32).max(f32::EPSILON),
                (rect.height as f32).max(f32::EPSILON),
            ) else {
                return;
            };
            let path = {
                let mut builder = tiny_skia::PathBuilder::new();
                builder.push_rect(skia_rect);
                builder.finish()
            };
            let Some(path) = path else { return };
            if let Some(color) = fill_color.and_then(|(rgb, a)| skia_color(rgb, a)) {
                pixmap.fill_path(
                    &path,
                    &skia_paint(color),
                    tiny_skia::FillRule::Winding,
                    transform,
                    None,
                );
            }
            if let Some(color) = stroke_color.and_then(|(rgb, a)| skia_color(rgb, a))
                && annotation.stroke_width > 0.
            {
                pixmap.stroke_path(
                    &path,
                    &skia_paint(color),
                    &skia_stroke(annotation.stroke_width, false),
                    transform,
                    None,
                );
            }
        }
        AnnotationType::Circle => {
            let rect = annotations::normalized_rect(annotation);
            // The preview's own guard: SVG (and the canvas) render nothing for
            // a zero radius.
            if rect.width <= 0. || rect.height <= 0. {
                return;
            }
            let Some(oval) = tiny_skia::Rect::from_xywh(
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
            ) else {
                return;
            };
            let path = {
                let mut builder = tiny_skia::PathBuilder::new();
                builder.push_oval(oval);
                builder.finish()
            };
            let Some(path) = path else { return };
            if let Some(color) = fill_color.and_then(|(rgb, a)| skia_color(rgb, a)) {
                pixmap.fill_path(
                    &path,
                    &skia_paint(color),
                    tiny_skia::FillRule::Winding,
                    transform,
                    None,
                );
            }
            if let Some(color) = stroke_color.and_then(|(rgb, a)| skia_color(rgb, a))
                && annotation.stroke_width > 0.
            {
                pixmap.stroke_path(
                    &path,
                    &skia_paint(color),
                    &skia_stroke(annotation.stroke_width, false),
                    transform,
                    None,
                );
            }
        }
        AnnotationType::Arrow => {
            let Some(color) = stroke_color.and_then(|(rgb, a)| skia_color(rgb, a)) else {
                return;
            };
            let end = (
                annotation.x + annotation.width,
                annotation.y + annotation.height,
            );
            let angle = (end.1 - annotation.y).atan2(end.0 - annotation.x);
            let head = annotations::arrow_head_points(end.0, end.1, angle, annotation.stroke_width);
            if annotation.stroke_width > 0. {
                let mut builder = tiny_skia::PathBuilder::new();
                builder.move_to(annotation.x as f32, annotation.y as f32);
                builder.line_to(head.base.0 as f32, head.base.1 as f32);
                if let Some(path) = builder.finish() {
                    pixmap.stroke_path(
                        &path,
                        &skia_paint(color),
                        &skia_stroke(annotation.stroke_width, true),
                        transform,
                        None,
                    );
                }
            }
            let mut builder = tiny_skia::PathBuilder::new();
            builder.move_to(head.points[0].0 as f32, head.points[0].1 as f32);
            builder.line_to(head.points[1].0 as f32, head.points[1].1 as f32);
            builder.line_to(head.points[2].0 as f32, head.points[2].1 as f32);
            builder.close();
            if let Some(path) = builder.finish() {
                pixmap.fill_path(
                    &path,
                    &skia_paint(color),
                    tiny_skia::FillRule::Winding,
                    transform,
                    None,
                );
            }
        }
        AnnotationType::Draw => {
            let Some(color) = stroke_color.and_then(|(rgb, a)| skia_color(rgb, a)) else {
                return;
            };
            if annotation.stroke_width <= 0. {
                return;
            }
            let points = annotations::draw_points(annotation);
            if points.len() < 2 {
                return;
            }
            let segments = annotations::smooth_path(&points);
            let mut builder = tiny_skia::PathBuilder::new();
            for segment in &segments {
                match *segment {
                    annotations::PathSegment::Move { x, y } => builder.move_to(x as f32, y as f32),
                    annotations::PathSegment::Line { x, y } => builder.line_to(x as f32, y as f32),
                    annotations::PathSegment::Quad { cx, cy, x, y } => {
                        builder.quad_to(cx as f32, cy as f32, x as f32, y as f32)
                    }
                }
            }
            if let Some(path) = builder.finish() {
                pixmap.stroke_path(
                    &path,
                    &skia_paint(color),
                    &skia_stroke(annotation.stroke_width, true),
                    transform,
                    None,
                );
            }
        }
        AnnotationType::Text => {
            let canvas_size = (pixmap.width(), pixmap.height());
            rasterize_text(annotation, |at, size, color| {
                pixels::blend_rect_premultiplied(pixmap.data_mut(), canvas_size, at, size, color);
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Text (`ctx.font = `${ann.height}px sans-serif``, `:113-117`)
// ---------------------------------------------------------------------------

/// One process-wide font system: `FontSystem::new` walks the installed font
/// database, which is far too slow to redo per export.
static TEXT_STACK: OnceLock<Mutex<(cosmic_text::FontSystem, cosmic_text::SwashCache)>> =
    OnceLock::new();

/// `<text>` at `font-size = height`, baseline at `y + height`, filled with the
/// stroke colour -- the same placement the preview's `paint_text` resolves,
/// down to the Helvetica the renderer maps `sans-serif` to.
fn rasterize_text(annotation: &Annotation, mut pixel: impl FnMut((f64, f64), (u32, u32), [u8; 4])) {
    let Some(text) = annotation.text.as_deref().filter(|text| !text.is_empty()) else {
        return;
    };
    let Some((rgb, alpha)) =
        annotations::parse_css_rgba(&annotation.stroke_color, annotation.opacity)
    else {
        return;
    };
    let font_size = (annotation.height as f32).max(1.);
    // One line, like the preview: a stray newline becomes a space.
    let text = text.replace('\n', " ");

    let stack = TEXT_STACK.get_or_init(|| {
        Mutex::new((
            frameever_rendering::new_font_system(),
            cosmic_text::SwashCache::new(),
        ))
    });
    let mut guard = match stack.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let (font_system, swash_cache) = &mut *guard;

    let mut buffer =
        cosmic_text::Buffer::new(font_system, cosmic_text::Metrics::new(font_size, font_size));
    buffer.set_size(None, None);
    let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Name(annotations::TEXT_FONT));
    buffer.set_text(&text, &attrs, cosmic_text::Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);

    // The buffer's own baseline, cancelled out so ours lands exactly at
    // `y + height`.
    let line_y = buffer
        .layout_runs()
        .next()
        .map(|run| run.line_y)
        .unwrap_or(font_size);
    let origin_x = annotation.x;
    let origin_y = annotation.y + annotation.height - f64::from(line_y);

    let color = cosmic_text::Color::rgba(rgb[0], rgb[1], rgb[2], (alpha * 255.).round() as u8);
    buffer.draw(font_system, swash_cache, color, |x, y, w, h, color| {
        pixel(
            (origin_x + f64::from(x), origin_y + f64::from(y)),
            (w, h),
            [color.r(), color.g(), color.b(), color.a()],
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn needs_transparency(out: &Composited, config: &ProjectConfiguration) -> bool {
        has_no_visible_background(&config.background.source)
            && pixels::has_translucent_pixel(&out.rgba)
    }
    use frameever_project::BackgroundSource;

    fn annotation(kind: AnnotationType, x: f64, y: f64, w: f64, h: f64) -> Annotation {
        Annotation {
            id: "a".into(),
            annotation_type: kind,
            x,
            y,
            width: w,
            height: h,
            stroke_color: "#F05656".into(),
            stroke_width: 4.,
            fill_color: "transparent".into(),
            opacity: 1.,
            rotation: 0.,
            text: None,
            mask_type: None,
            mask_level: None,
            points: None,
        }
    }

    fn invisible_background(config: &mut ProjectConfiguration) {
        config.background.source = BackgroundSource::Color {
            value: [0, 0, 0],
            alpha: 0,
        };
    }

    #[test]
    fn mask_only_draw_preserves_straight_alpha_pixels() {
        let original: Vec<u8> = (0..=255u8).flat_map(|alpha| [17, 30, 77, alpha]).collect();
        for scaled in [
            Vec::new(),
            vec![annotation(AnnotationType::Mask, 0., 0., 16., 16.)],
        ] {
            let mut canvas = original.clone();
            draw_annotations_onto(&mut canvas, 16, 16, &scaled);
            assert_eq!(canvas, original);
        }
    }

    #[test]
    fn off_canvas_mask_does_not_change_translucent_export_pixels() {
        let raw = RawFrame {
            rgba: vec![17, 30, 77, 1, 123, 45, 67, 127],
            width: 2,
            height: 1,
            base_width: 2,
            base_height: 1,
        };
        let mut config = ProjectConfiguration::default();
        invisible_background(&mut config);
        config
            .annotations
            .push(annotation(AnnotationType::Mask, 3., 0., 2., 1.));

        let out = composite(&raw, &config);

        assert_eq!((out.width, out.height), (2, 1));
        assert_eq!(out.rgba, raw.rgba);
    }

    #[test]
    fn pixelated_export_preserves_pixels_outside_the_mask_and_draws_shapes() {
        let raw = RawFrame {
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 17, 30, 77, 1, 123, 45, 67, 127, 255, 255, 0, 255,
                0, 0, 255, 255, 23, 41, 83, 3, 111, 55, 99, 129,
            ],
            width: 4,
            height: 2,
            base_width: 4,
            base_height: 2,
        };
        let mut config = ProjectConfiguration::default();
        invisible_background(&mut config);
        let mut mask = annotation(AnnotationType::Mask, 0., 0., 2., 2.);
        mask.mask_type = Some(frameever_project::MaskType::Pixelate);
        mask.mask_level = Some(2.);
        config.annotations.push(mask);
        let mut expected = raw.rgba.clone();
        for index in [0, 1, 4, 5] {
            expected[index * 4..index * 4 + 4].copy_from_slice(&[0, 0, 255, 255]);
        }

        let out = composite(&raw, &config);

        assert_eq!((out.width, out.height), (4, 2));
        assert_eq!(out.rgba, expected);

        let mut rectangle = annotation(AnnotationType::Rectangle, 2., 0., 2., 2.);
        rectangle.fill_color = "#ff0000".into();
        rectangle.stroke_width = 0.;
        config.annotations.push(rectangle);
        for index in [2, 3, 6, 7] {
            expected[index * 4..index * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }

        assert_eq!(composite(&raw, &config).rgba, expected);
    }

    /// `scaleAnnotations`: x/w by the x factor, y/h by the y factor,
    /// strokeWidth and maskLevel by the average, points untouched.
    #[test]
    fn scale_annotations_uses_axis_factors_and_the_average_scalar() {
        let mut mask = annotation(AnnotationType::Mask, 10., 20., 30., 40.);
        mask.mask_level = Some(8.);
        let mut stroke = annotation(AnnotationType::Draw, 1., 2., 3., 4.);
        stroke.points = Some(vec![[0.25, 0.75]]);

        let scaled = scale_annotations(&[mask, stroke], 2., 4.);
        assert_eq!(
            (scaled[0].x, scaled[0].y, scaled[0].width, scaled[0].height),
            (20., 80., 60., 160.)
        );
        assert_eq!(scaled[0].stroke_width, 12.); // 4 * (2+4)/2
        assert_eq!(scaled[0].mask_level, Some(24.)); // 8 * 3
        assert_eq!(scaled[1].mask_level, None);
        assert_eq!(scaled[1].points, Some(vec![[0.25, 0.75]]));
    }

    /// The union expansion: annotations dragged past the frame stretch the
    /// output, negative boxes normalize first, and masks never count.
    #[test]
    fn export_bounds_unions_the_canvas_with_non_mask_annotations() {
        let inside = annotation(AnnotationType::Rectangle, 10., 10., 20., 20.);
        assert_eq!(
            export_bounds(&[inside], (100, 50)),
            ExportBounds {
                min_x: 0.,
                min_y: 0.,
                width: 100,
                height: 50
            }
        );

        let overflowing = annotation(AnnotationType::Rectangle, 90., -10., 30., 15.);
        let bounds = export_bounds(&[overflowing], (100, 50));
        assert_eq!((bounds.min_x, bounds.min_y), (0., -12.));
        assert_eq!((bounds.width, bounds.height), (122, 62));

        let negative = annotation(AnnotationType::Rectangle, -5., 0., -10., 10.);
        let bounds = export_bounds(&[negative], (100, 50));
        assert_eq!((bounds.min_x, bounds.min_y), (-17., -2.));
        assert_eq!((bounds.width, bounds.height), (117, 52));

        let mask = annotation(AnnotationType::Mask, -50., -50., 300., 300.);
        assert_eq!(
            export_bounds(&[mask], (100, 50)),
            ExportBounds {
                min_x: 0.,
                min_y: 0.,
                width: 100,
                height: 50
            }
        );
    }

    fn reference_flatten(out: &Composited) -> Composited {
        let mut rgba = vec![255; out.rgba.len()];
        pixels::blit_over_offset(
            &mut rgba,
            (out.width, out.height),
            &out.rgba,
            (out.width, out.height),
            (0, 0),
        );
        Composited {
            rgba,
            width: out.width,
            height: out.height,
        }
    }

    fn reference_unannotated_composite(
        raw: &RawFrame,
        config: &ProjectConfiguration,
    ) -> Composited {
        let width = raw.width.max(1);
        let height = raw.height.max(1);
        let mut canvas = raw.rgba.clone();
        draw_annotations_onto(&mut canvas, width, height, &[]);
        let mut rgba = vec![0; width as usize * height as usize * 4];
        if !has_no_visible_background(&config.background.source) {
            rgba.fill(255);
        }
        pixels::blit_over_offset(&mut rgba, (width, height), &canvas, (width, height), (0, 0));
        Composited {
            rgba,
            width,
            height,
        }
    }

    #[test]
    fn unannotated_composite_and_copy_preserve_all_alpha_values() {
        for opaque in [false, true] {
            let raw = RawFrame {
                rgba: (0..=255u8)
                    .flat_map(|value| {
                        [
                            value,
                            255 - value,
                            value.wrapping_mul(17),
                            if opaque { 255 } else { value },
                        ]
                    })
                    .collect(),
                width: 16,
                height: 16,
                base_width: 16,
                base_height: 16,
            };
            for invisible in [false, true] {
                let mut config = ProjectConfiguration::default();
                if invisible {
                    invisible_background(&mut config);
                }
                let out = composite(&raw, &config);
                let reference = if invisible {
                    Composited {
                        rgba: raw.rgba.clone(),
                        width: raw.width.max(1),
                        height: raw.height.max(1),
                    }
                } else {
                    reference_unannotated_composite(&raw, &config)
                };
                assert_eq!(out.rgba, reference.rgba);
                let expected = if needs_transparency(&reference, &config) {
                    reference
                } else {
                    reference_flatten(&reference)
                };
                let png = encode_for_copy(&out, &config).unwrap();
                let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
                assert_eq!(decoded.dimensions(), (16, 16));
                assert_eq!(decoded.as_raw(), &expected.rgba);
            }
        }
    }

    #[test]
    fn unannotated_composite_preserves_extra_pixel_handling() {
        let raw = RawFrame {
            rgba: vec![10, 20, 30, 255, 40, 50, 60, 255],
            width: 1,
            height: 1,
            base_width: 1,
            base_height: 1,
        };
        let config = ProjectConfiguration::default();
        assert_eq!(
            composite(&raw, &config).rgba,
            reference_unannotated_composite(&raw, &config).rgba,
        );
    }

    #[test]
    #[ignore]
    fn benchmark_mask_only_screenshot_composite() {
        use std::{hint::black_box, time::Instant};
        for (width, height) in [(1920, 1080), (3840, 2160)] {
            for translucent_border in [false, true] {
                let raw = RawFrame {
                    rgba: (0..width * height)
                        .flat_map(|i: u32| {
                            let x = i % width;
                            let y = i / width;
                            let border = x < 32 || y < 32 || x >= width - 32 || y >= height - 32;
                            let alpha = if translucent_border && border {
                                (i % 254 + 1) as u8
                            } else {
                                255
                            };
                            [
                                i as u8,
                                i.wrapping_mul(17) as u8,
                                i.wrapping_mul(31) as u8,
                                alpha,
                            ]
                        })
                        .collect(),
                    width,
                    height,
                    base_width: width,
                    base_height: height,
                };
                let mut config = ProjectConfiguration::default();
                invisible_background(&mut config);
                let mut mask = annotation(AnnotationType::Mask, 100., 100., 256., 256.);
                mask.mask_type = Some(frameever_project::MaskType::Pixelate);
                mask.mask_level = Some(16.);
                config.annotations.push(mask);
                let mut samples_ns = Vec::new();
                for iteration in 0..35 {
                    let start = Instant::now();
                    black_box(composite(black_box(&raw), black_box(&config)));
                    let elapsed = start.elapsed().as_nanos();
                    if iteration >= 5 {
                        samples_ns.push(elapsed);
                    }
                }
                samples_ns.sort_unstable();
                let median_ms = (samples_ns[14] + samples_ns[15]) as f64 / 2_000_000.;
                println!(
                    "{}",
                    serde_json::json!({
                        "width": width, "height": height,
                        "translucent_border": translucent_border,
                        "median_ms": median_ms, "samples_ns": samples_ns,
                        "scope": "CPU mask-only composite; excludes GPU, PNG, disk and native UI"
                    })
                );
            }
        }
    }

    #[test]
    #[ignore]
    fn benchmark_screenshot_copy_preparation() {
        use std::{hint::black_box, time::Instant};
        for (width, height) in [(1920, 1080), (3840, 2160)] {
            for opaque in [false, true] {
                let raw = RawFrame {
                    rgba: (0..width * height)
                        .flat_map(|i| {
                            [
                                (i % 251) as u8,
                                ((i / width) % 253) as u8,
                                (i % 247) as u8,
                                if opaque { 255 } else { (i % 256) as u8 },
                            ]
                        })
                        .collect(),
                    width,
                    height,
                    base_width: width,
                    base_height: height,
                };
                let config = ProjectConfiguration::default();
                let mut before = Vec::new();
                let mut after = Vec::new();
                for iteration in 0..8 {
                    let start = Instant::now();
                    let reference = reference_unannotated_composite(black_box(&raw), &config);
                    let reference = reference_flatten(&reference);
                    let baseline_ms = start.elapsed().as_secs_f64() * 1000.0;
                    let start = Instant::now();
                    let result = composite(black_box(&raw), &config);
                    let result = if pixels::is_opaque(&result.rgba) {
                        result
                    } else {
                        flatten_onto_white(&result)
                    };
                    let candidate_ms = start.elapsed().as_secs_f64() * 1000.0;
                    assert_eq!(result.rgba, reference.rgba);
                    black_box(result);
                    if iteration > 0 {
                        before.push(baseline_ms);
                        after.push(candidate_ms);
                    }
                }
                before.sort_by(f64::total_cmp);
                after.sort_by(f64::total_cmp);
                println!(
                    "{}",
                    serde_json::json!({"width":width,"height":height,"opaque":opaque,"baseline_ms":before[3],"candidate_ms":after[3],"pixels_equal":true,"scope":"CPU composite and clipboard flatten; excludes GPU, PNG and native pasteboard"})
                );
            }
        }
    }

    #[test]
    fn export_bounds_rounds_fractional_strokes_outward() {
        let negative = annotation(AnnotationType::Rectangle, -30.25, -20.75, 20., 10.);
        assert_eq!(
            export_bounds(&[negative], (100, 50)),
            ExportBounds {
                min_x: -33.,
                min_y: -23.,
                width: 133,
                height: 73
            }
        );
        let positive = annotation(AnnotationType::Rectangle, 110.25, 60.75, 20., 10.);
        assert_eq!(
            export_bounds(&[positive], (100, 50)),
            ExportBounds {
                min_x: 0.,
                min_y: 0.,
                width: 133,
                height: 73
            }
        );
    }

    #[test]
    fn composite_exports_text_glyphs_outside_the_annotation_box() {
        let raw = RawFrame {
            rgba: vec![0; 100 * 100 * 4],
            width: 100,
            height: 100,
            base_width: 100,
            base_height: 100,
        };
        let mut text = annotation(AnnotationType::Text, 95., -5., 1., 32.);
        text.text = Some("jÁW".into());
        let mut config = ProjectConfiguration::default();
        invisible_background(&mut config);
        config.annotations.push(text.clone());

        text.x += 128.;
        text.y += 128.;
        let mut reference = vec![0; 512 * 512 * 4];
        draw_annotations_onto(&mut reference, 512, 512, &[text]);
        let reference = image::RgbaImage::from_raw(512, 512, reference).unwrap();
        let (mut left, mut top, mut right, mut bottom) = (0i64, -5i64, 100i64, 100i64);
        for (x, y, pixel) in reference.enumerate_pixels() {
            if pixel.0[3] > 0 {
                let x = i64::from(x) - 128;
                let y = i64::from(y) - 128;
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
        assert!(right > 100);
        let expected = image::imageops::crop_imm(
            &reference,
            (left + 128) as u32,
            (top + 128) as u32,
            (right - left) as u32,
            (bottom - top) as u32,
        )
        .to_image();
        let out = composite(&raw, &config);
        assert_eq!((out.width, out.height), expected.dimensions());
        assert_eq!(out.rgba, expected.into_raw());
    }

    #[test]
    fn composite_keeps_frame_interior_rendering_unchanged() {
        let raw = RawFrame {
            rgba: [0u8, 0, 255, 255].repeat(100 * 100),
            width: 100,
            height: 100,
            base_width: 100,
            base_height: 100,
        };
        let mut config = ProjectConfiguration {
            annotations: vec![
                annotation(AnnotationType::Rectangle, 10., 10., 20., 20.),
                annotation(AnnotationType::Circle, 40., 10., 20., 20.),
                annotation(AnnotationType::Arrow, 10., 50., 40., 0.),
                annotation(AnnotationType::Draw, 10., 70., 20., 20.),
                annotation(AnnotationType::Text, 50., 70., 40., 10.),
            ],
            ..ProjectConfiguration::default()
        };
        config.annotations[3].points = Some(vec![[0., 0.], [1., 1.]]);
        config.annotations[4].text = Some("cap".into());
        for annotation in &mut config.annotations {
            annotation.rotation = 45.;
        }
        let mut expected = raw.rgba.clone();
        draw_annotations_onto(&mut expected, raw.width, raw.height, &config.annotations);
        let out = composite(&raw, &config);
        assert_eq!((out.width, out.height), (100, 100));
        assert_eq!(out.rgba, expected);
    }

    #[test]
    fn composite_keeps_translucent_in_frame_exports_byte_identical() {
        let rgba = (0..64u32 * 64)
            .flat_map(|i| {
                [
                    (i * 7) as u8,
                    (i * 13) as u8,
                    (i * 29) as u8,
                    (i % 256) as u8,
                ]
            })
            .collect::<Vec<_>>();
        let raw = RawFrame {
            rgba,
            width: 64,
            height: 64,
            base_width: 64,
            base_height: 64,
        };
        for visible in [true, false] {
            let mut config = ProjectConfiguration::default();
            config.background.source = BackgroundSource::Color {
                value: [10, 20, 30],
                alpha: if visible { 255 } else { 0 },
            };
            let mut rectangle = annotation(AnnotationType::Rectangle, 10., 10., 30., 20.);
            rectangle.stroke_width = 3.;
            rectangle.stroke_color = "#ff0000".into();
            rectangle.fill_color = "#00ff00".into();
            rectangle.opacity = 0.5;
            config.annotations.push(rectangle);

            let mut canvas = raw.rgba.clone();
            draw_annotations_onto(&mut canvas, 64, 64, &config.annotations);
            let mut expected = vec![if visible { 255 } else { 0 }; 64 * 64 * 4];
            pixels::blit_over_offset(&mut expected, (64, 64), &canvas, (64, 64), (0, 0));

            let out = composite(&raw, &config);
            assert_eq!((out.width, out.height), (64, 64));
            assert_eq!(out.rgba, expected);
        }
    }

    #[test]
    fn composite_exports_outermost_strokes_and_arrow_heads() {
        let raw = RawFrame {
            rgba: [0u8, 0, 255, 255].repeat(100 * 100),
            width: 100,
            height: 100,
            base_width: 50,
            base_height: 50,
        };
        for invisible in [false, true] {
            for (x, y, pixel, size) in [
                (55., 10., (131, 30), (132, 100)),
                (-15., 10., (0, 30), (132, 100)),
                (10., 55., (30, 131), (100, 132)),
                (10., -15., (30, 0), (100, 132)),
            ] {
                let mut config = ProjectConfiguration::default();
                if invisible {
                    invisible_background(&mut config);
                }
                let mut rectangle = annotation(AnnotationType::Rectangle, x, y, 10., 10.);
                rectangle.stroke_width = 2.;
                rectangle.stroke_color = "#00ff00".into();
                config.annotations.push(rectangle);

                let out = composite(&raw, &config);
                let decoded = image::load_from_memory(&encode_for_save(&out).unwrap())
                    .unwrap()
                    .to_rgba8();
                assert_eq!(decoded.dimensions(), size);
                assert_eq!(decoded.get_pixel(pixel.0, pixel.1).0, [0, 255, 0, 255]);
            }

            for (x, y, width, height, pixel, size) in [
                (10., 2., 20., 0., (36, 0), (100, 106)),
                (10., 48., 20., 0., (36, 105), (100, 106)),
                (2., 10., 0., 20., (0, 36), (106, 100)),
                (48., 10., 0., 20., (105, 36), (106, 100)),
            ] {
                let mut config = ProjectConfiguration::default();
                if invisible {
                    invisible_background(&mut config);
                }
                let mut arrow = annotation(AnnotationType::Arrow, x, y, width, height);
                arrow.stroke_width = 2.;
                arrow.stroke_color = "#00ff00".into();
                config.annotations.push(arrow);
                let out = composite(&raw, &config);
                assert_eq!((out.width, out.height), size);
                let start = (pixel.1 * out.width as usize + pixel.0) * 4;
                assert!(out.rgba[start] < 255);
                assert_eq!(out.rgba[start + 1], 255);
                assert!(out.rgba[start + 3] > 0);
            }
        }
    }

    #[test]
    fn composite_exports_annotations_past_each_frame_edge() {
        let raw = RawFrame {
            rgba: [0u8, 0, 255, 255].repeat(16),
            width: 4,
            height: 4,
            base_width: 2,
            base_height: 2,
        };
        for invisible in [false, true] {
            for (x, y, pixel, size) in [
                (3., 0., (7, 1), (10, 4)),
                (-3., 0., (1, 1), (10, 4)),
                (0., 3., (1, 7), (4, 10)),
                (0., -3., (1, 1), (4, 10)),
            ] {
                let mut config = ProjectConfiguration::default();
                if invisible {
                    invisible_background(&mut config);
                }
                let mut rectangle = annotation(AnnotationType::Rectangle, x, y, 2., 2.);
                rectangle.stroke_width = 0.;
                rectangle.fill_color = "#00ff00".into();
                config.annotations.push(rectangle);

                let out = composite(&raw, &config);
                let png = encode_for_save(&out).unwrap();
                let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
                assert_eq!(decoded.dimensions(), size);
                assert_eq!(decoded.get_pixel(pixel.0, pixel.1).0, [0, 255, 0, 255]);
                let origin = ((-x * 2.).max(0.) as u32, (-y * 2.).max(0.) as u32);
                assert_eq!(decoded.get_pixel(origin.0, origin.1).0, [0, 0, 255, 255]);
                let gap = if x == 0. { (1, 5) } else { (5, 1) };
                assert_eq!(
                    decoded.get_pixel(gap.0, gap.1).0,
                    if invisible { [0; 4] } else { [255; 4] }
                );
            }
        }
    }
}
