use std::sync::Arc;

use frameever_project::{
    Annotation, AnnotationType, Crop, ImageCut, ImageCutAxis, ProjectConfiguration, XY,
};
use frameever_rendering::{ProjectUniforms, RenderOptions};

use crate::screenshot_annotations::{self as annotations, ImageTransform};

pub const MIN_BAND: f64 = 5.;
pub const CLICK_BAND: f64 = 120.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameBand {
    pub axis: ImageCutAxis,
    pub start: f64,
    pub length: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutLayout {
    pub image_size: (u32, u32),
    pub frame_size: (f32, f32),
    pub transform: ImageTransform,
}

pub fn cut_size(size: (u32, u32), cuts: &[ImageCut]) -> (u32, u32) {
    cuts.iter().fold(size, |size, cut| {
        let (_, length) = clamped(*cut, size);
        match cut.axis {
            ImageCutAxis::Columns => (size.0 - length, size.1),
            ImageCutAxis::Rows => (size.0, size.1 - length),
        }
    })
}

fn clamped(cut: ImageCut, size: (u32, u32)) -> (u32, u32) {
    let extent = match cut.axis {
        ImageCutAxis::Columns => size.0,
        ImageCutAxis::Rows => size.1,
    };
    let start = cut.start.min(extent);
    let length = cut.length.min(extent - start).min(extent.saturating_sub(1));
    (start, length)
}

fn remove_band(rgba: &[u8], size: (u32, u32), cut: ImageCut) -> (Vec<u8>, (u32, u32)) {
    let (start, length) = clamped(cut, size);
    let (width, height) = (size.0 as usize, size.1 as usize);
    let (start, length) = (start as usize, length as usize);
    match cut.axis {
        ImageCutAxis::Columns => {
            let mut out = Vec::with_capacity((width - length) * height * 4);
            for row in rgba.chunks_exact(width * 4) {
                out.extend_from_slice(&row[..start * 4]);
                out.extend_from_slice(&row[(start + length) * 4..]);
            }
            (out, (size.0 - length as u32, size.1))
        }
        ImageCutAxis::Rows => {
            let mut out = Vec::with_capacity(width * (height - length) * 4);
            out.extend_from_slice(&rgba[..start * width * 4]);
            out.extend_from_slice(&rgba[(start + length) * width * 4..]);
            (out, (size.0, size.1 - length as u32))
        }
    }
}

pub fn cut_source(
    original: &Arc<Vec<u8>>,
    size: (u32, u32),
    cuts: &[ImageCut],
) -> (Arc<Vec<u8>>, (u32, u32)) {
    if cuts.is_empty() || original.len() != size.0 as usize * size.1 as usize * 4 {
        return (original.clone(), size);
    }
    let mut current = remove_band(original, size, cuts[0]);
    for cut in &cuts[1..] {
        current = remove_band(&current.0, current.1, *cut);
    }
    (Arc::new(current.0), current.1)
}

pub fn band_from_drag(start: (f64, f64), end: (f64, f64)) -> FrameBand {
    let (dx, dy) = ((end.0 - start.0).abs(), (end.1 - start.1).abs());
    if dx < MIN_BAND && dy < MIN_BAND {
        return FrameBand {
            axis: ImageCutAxis::Columns,
            start: start.0 - CLICK_BAND / 2.,
            length: CLICK_BAND,
        };
    }
    if dx >= dy {
        FrameBand {
            axis: ImageCutAxis::Columns,
            start: start.0.min(end.0),
            length: dx,
        }
    } else {
        FrameBand {
            axis: ImageCutAxis::Rows,
            start: start.1.min(end.1),
            length: dy,
        }
    }
}

fn axis_parts(
    axis: ImageCutAxis,
    transform: &ImageTransform,
    image: (u32, u32),
    crop: Option<&Crop>,
) -> (f64, f64, u32, u32) {
    match axis {
        ImageCutAxis::Columns => (
            transform.offset.0,
            transform.size.0,
            crop.map_or(0, |crop| crop.position.x),
            crop.map_or(image.0, |crop| crop.size.x),
        ),
        ImageCutAxis::Rows => (
            transform.offset.1,
            transform.size.1,
            crop.map_or(0, |crop| crop.position.y),
            crop.map_or(image.1, |crop| crop.size.y),
        ),
    }
}

pub fn plan_cut(
    band: FrameBand,
    transform: &ImageTransform,
    image: (u32, u32),
    crop: Option<&Crop>,
) -> Option<(ImageCut, FrameBand)> {
    let (offset, extent, visible_start, visible) = axis_parts(band.axis, transform, image, crop);
    if extent <= 0. || visible < 2 {
        return None;
    }
    let scale = f64::from(visible) / extent;
    let to_image = |position: f64| {
        ((position - offset) * scale)
            .round()
            .clamp(0., f64::from(visible)) as u32
    };
    let first = to_image(band.start);
    let last = to_image(band.start + band.length);
    let length = last.saturating_sub(first).min(visible - 1);
    if length == 0 {
        return None;
    }
    Some((
        ImageCut {
            axis: band.axis,
            start: visible_start + first,
            length,
        },
        FrameBand {
            axis: band.axis,
            start: offset + f64::from(first) / scale,
            length: f64::from(length) / scale,
        },
    ))
}

fn close_up(position: f64, band: &FrameBand) -> f64 {
    if position <= band.start {
        position
    } else if position >= band.start + band.length {
        position - band.length
    } else {
        band.start
    }
}

fn inside(position: f64, band: &FrameBand) -> bool {
    position > band.start && position < band.start + band.length
}

pub fn cut_annotations(list: &mut Vec<Annotation>, band: &FrameBand) {
    list.retain_mut(|annotation| {
        let (position, extent) = match band.axis {
            ImageCutAxis::Columns => (annotation.x, annotation.width),
            ImageCutAxis::Rows => (annotation.y, annotation.height),
        };
        let keeps_size = matches!(
            annotation.annotation_type,
            AnnotationType::Step | AnnotationType::Text
        );
        let (moved, resized) = if keeps_size {
            let middle = position + extent / 2.;
            if inside(middle, band) {
                return false;
            }
            (position + close_up(middle, band) - middle, extent)
        } else {
            if inside(position, band) && inside(position + extent, band) {
                return false;
            }
            let moved = close_up(position, band);
            (moved, close_up(position + extent, band) - moved)
        };
        let tail = annotation.tail.map(|tail| match band.axis {
            ImageCutAxis::Columns => [close_up(tail[0], band), tail[1]],
            ImageCutAxis::Rows => [tail[0], close_up(tail[1], band)],
        });
        match band.axis {
            ImageCutAxis::Columns => {
                annotation.x = moved;
                annotation.width = resized;
            }
            ImageCutAxis::Rows => {
                annotation.y = moved;
                annotation.height = resized;
            }
        }
        annotation.tail = tail;
        true
    });
}

fn cut_crop(crop: &mut Crop, cut: &ImageCut) {
    let close = |position: u32| {
        if position <= cut.start {
            position
        } else if position >= cut.start + cut.length {
            position - cut.length
        } else {
            cut.start
        }
    };
    let (position, size) = match cut.axis {
        ImageCutAxis::Columns => (&mut crop.position.x, &mut crop.size.x),
        ImageCutAxis::Rows => (&mut crop.position.y, &mut crop.size.y),
    };
    let first = close(*position);
    let last = close(*position + *size);
    *position = first;
    *size = (last - first).max(1);
}

pub fn preview_frame_size(image: (u32, u32), project: &ProjectConfiguration) -> (f32, f32) {
    let options = RenderOptions {
        screen_size: XY::new(image.0, image.1),
        camera_size: None,
        preserve_screen_alpha: true,
    };
    let (base_width, base_height) = ProjectUniforms::get_base_size(&options, project);
    let (width, height) =
        ProjectUniforms::get_output_size(&options, project, XY::new(base_width, base_height));
    (width as f32, height as f32)
}

pub fn layout(image: (u32, u32), project: &ProjectConfiguration) -> CutLayout {
    let frame_size = preview_frame_size(image, project);
    CutLayout {
        image_size: image,
        frame_size,
        transform: annotations::calculate_image_transform(
            (f64::from(frame_size.0), f64::from(frame_size.1)),
            (f64::from(image.0), f64::from(image.1)),
            project.background.padding,
            project.background.crop.as_ref(),
            project.aspect_ratio.as_ref(),
        ),
    }
}

pub fn cut_project(
    project: &mut ProjectConfiguration,
    image: (u32, u32),
    transform: &ImageTransform,
    band: FrameBand,
) -> Option<CutLayout> {
    let (cut, band) = plan_cut(band, transform, image, project.background.crop.as_ref())?;
    cut_annotations(&mut project.annotations, &band);
    if let Some(crop) = project.background.crop.as_mut() {
        cut_crop(crop, &cut);
    }
    project.image_cuts.push(cut);
    let closed = match band.axis {
        ImageCutAxis::Columns => ImageTransform {
            offset: transform.offset,
            size: (transform.size.0 - band.length, transform.size.1),
        },
        ImageCutAxis::Rows => ImageTransform {
            offset: transform.offset,
            size: (transform.size.0, transform.size.1 - band.length),
        },
    };
    let after = layout(cut_size(image, &[cut]), project);
    annotations::rescale_annotations(&mut project.annotations, &closed, &after.transform);
    Some(after)
}

pub fn band_rect(band: &FrameBand, frame: (f64, f64)) -> annotations::Rect {
    match band.axis {
        ImageCutAxis::Columns => annotations::Rect {
            x: band.start,
            y: 0.,
            width: band.length,
            height: frame.1,
        },
        ImageCutAxis::Rows => annotations::Rect {
            x: 0.,
            y: band.start,
            width: frame.0,
            height: band.length,
        },
    }
}

pub fn uses_current_format(project: &ProjectConfiguration) -> bool {
    !project.image_cuts.is_empty()
        || project
            .annotations
            .iter()
            .any(|annotation| annotation.annotation_type == AnnotationType::Image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_edits::ProjectHistory;
    use crate::screenshot_shapes::tests::shape;

    fn numbered(width: u32, height: u32) -> Arc<Vec<u8>> {
        Arc::new(
            (0..width * height)
                .flat_map(|index| [(index % width) as u8, (index / width) as u8, 0, 255])
                .collect(),
        )
    }

    fn columns(start: u32, length: u32) -> ImageCut {
        ImageCut {
            axis: ImageCutAxis::Columns,
            start,
            length,
        }
    }

    fn rows(start: u32, length: u32) -> ImageCut {
        ImageCut {
            axis: ImageCutAxis::Rows,
            start,
            length,
        }
    }

    fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> (u8, u8) {
        let at = ((y * width + x) * 4) as usize;
        (rgba[at], rgba[at + 1])
    }

    fn identity(width: f64, height: f64) -> ImageTransform {
        ImageTransform {
            offset: (0., 0.),
            size: (width, height),
        }
    }

    #[test]
    fn cutting_columns_closes_the_image_up_and_leaves_the_original_alone() {
        let original = numbered(6, 2);
        let before = original.as_ref().clone();
        let (cut, size) = cut_source(&original, (6, 2), &[columns(2, 3)]);
        assert_eq!(size, (3, 2));
        assert_eq!(
            (0..3).map(|x| pixel(&cut, 3, x, 1)).collect::<Vec<_>>(),
            vec![(0, 1), (1, 1), (5, 1)]
        );
        assert_eq!(*original, before);
    }

    #[test]
    fn cutting_rows_closes_the_image_up() {
        let (cut, size) = cut_source(&numbered(2, 5), (2, 5), &[rows(1, 2)]);
        assert_eq!(size, (2, 3));
        assert_eq!(
            (0..3).map(|y| pixel(&cut, 2, 1, y)).collect::<Vec<_>>(),
            vec![(1, 0), (1, 3), (1, 4)]
        );
    }

    #[test]
    fn later_cuts_are_measured_on_the_image_the_earlier_ones_left() {
        let cuts = [columns(1, 2), rows(0, 1), columns(1, 1)];
        let (cut, size) = cut_source(&numbered(6, 3), (6, 3), &cuts);
        assert_eq!(size, (3, 2));
        assert_eq!(cut_size((6, 3), &cuts), (3, 2));
        assert_eq!(
            (0..3).map(|x| pixel(&cut, 3, x, 0)).collect::<Vec<_>>(),
            vec![(0, 1), (4, 1), (5, 1)]
        );
    }

    #[test]
    fn a_cut_never_removes_the_whole_image() {
        let (cut, size) = cut_source(&numbered(4, 1), (4, 1), &[columns(0, 9)]);
        assert_eq!(size, (1, 1));
        assert_eq!(cut.len(), 4);
        assert_eq!(cut_size((4, 1), &[columns(0, 9)]), (1, 1));
    }

    #[test]
    fn no_cuts_share_the_original_pixels() {
        let original = numbered(3, 3);
        let (cut, size) = cut_source(&original, (3, 3), &[]);
        assert!(Arc::ptr_eq(&cut, &original));
        assert_eq!(size, (3, 3));
    }

    #[test]
    fn a_wide_drag_cuts_columns_a_tall_drag_cuts_rows_and_a_click_cuts_a_default_band() {
        assert_eq!(
            band_from_drag((300., 40.), (100., 90.)),
            FrameBand {
                axis: ImageCutAxis::Columns,
                start: 100.,
                length: 200.
            }
        );
        assert_eq!(
            band_from_drag((10., 200.), (30., 80.)),
            FrameBand {
                axis: ImageCutAxis::Rows,
                start: 80.,
                length: 120.
            }
        );
        assert_eq!(
            band_from_drag((200., 50.), (202., 51.)),
            FrameBand {
                axis: ImageCutAxis::Columns,
                start: 140.,
                length: 120.
            }
        );
    }

    #[test]
    fn a_band_is_planned_in_image_pixels_through_padding_and_crop() {
        let transform = ImageTransform {
            offset: (50., 20.),
            size: (200., 100.),
        };
        let crop = Crop {
            position: XY::new(100, 0),
            size: XY::new(400, 200),
        };
        let band = FrameBand {
            axis: ImageCutAxis::Columns,
            start: 100.,
            length: 50.,
        };
        let (cut, snapped) = plan_cut(band, &transform, (800, 200), Some(&crop)).unwrap();
        assert_eq!(cut, columns(200, 100));
        assert_eq!(snapped, band);

        let outside = FrameBand {
            axis: ImageCutAxis::Columns,
            start: 0.,
            length: 40.,
        };
        assert_eq!(plan_cut(outside, &transform, (800, 200), Some(&crop)), None);

        let everything = FrameBand {
            axis: ImageCutAxis::Rows,
            start: -10.,
            length: 500.,
        };
        let (cut, _) = plan_cut(everything, &transform, (800, 200), Some(&crop)).unwrap();
        assert_eq!(cut, rows(0, 199));
    }

    #[test]
    fn annotations_move_with_the_image_when_a_band_is_cut() {
        let band = FrameBand {
            axis: ImageCutAxis::Columns,
            start: 100.,
            length: 50.,
        };
        let mut list = vec![
            shape(AnnotationType::Rectangle, 10., 10., 40., 20.),
            shape(AnnotationType::Rectangle, 200., 10., 40., 20.),
            shape(AnnotationType::Rectangle, 80., 10., 100., 20.),
            shape(AnnotationType::Rectangle, 110., 10., 20., 20.),
            shape(AnnotationType::Arrow, 220., 10., -160., 30.),
            shape(AnnotationType::Step, 130., 10., 42., 42.),
            shape(AnnotationType::Step, 100., 10., 42., 42.),
        ];
        list[4].id = "arrow".into();
        list[5].id = "kept step".into();
        cut_annotations(&mut list, &band);
        let boxes: Vec<_> = list
            .iter()
            .map(|annotation| (annotation.x, annotation.width))
            .collect();
        assert_eq!(
            boxes,
            vec![
                (10., 40.),
                (150., 40.),
                (80., 50.),
                (170., -110.),
                (80., 42.)
            ]
        );
        assert_eq!(list[4].id, "kept step");
        assert!(list.iter().all(|annotation| annotation.y == 10.));
    }

    #[test]
    fn a_bubble_tail_and_rows_follow_the_cut() {
        let band = FrameBand {
            axis: ImageCutAxis::Rows,
            start: 50.,
            length: 30.,
        };
        let mut bubble = shape(AnnotationType::Bubble, 20., 100., 120., 80.);
        bubble.tail = Some([40., 60.]);
        let mut list = vec![bubble];
        cut_annotations(&mut list, &band);
        assert_eq!((list[0].y, list[0].height), (70., 80.));
        assert_eq!(list[0].tail, Some([40., 50.]));
        assert_eq!(list[0].x, 20.);
    }

    fn screenshot_project() -> ProjectConfiguration {
        let mut project = ProjectConfiguration::default();
        project.background.padding = 0.;
        project
    }

    #[test]
    fn cutting_a_project_stores_the_cut_moves_annotations_and_shrinks_the_crop() {
        let mut project = screenshot_project();
        project.background.crop = Some(Crop {
            position: XY::new(0, 0),
            size: XY::new(400, 200),
        });
        project
            .annotations
            .push(shape(AnnotationType::Rectangle, 300., 20., 40., 40.));
        let before = layout((400, 200), &project);
        assert_eq!(before.frame_size, (400., 200.));
        let band = FrameBand {
            axis: ImageCutAxis::Columns,
            start: 100.,
            length: 100.,
        };
        let after = cut_project(&mut project, (400, 200), &before.transform, band).unwrap();
        assert_eq!(project.image_cuts, vec![columns(100, 100)]);
        assert_eq!(after.image_size, (300, 200));
        assert_eq!(after.frame_size, (300., 200.));
        assert_eq!(after.transform, identity(300., 200.));
        let crop = project.background.crop.unwrap();
        assert_eq!((crop.position.x, crop.size.x), (0, 300));
        let moved = &project.annotations[0];
        assert_eq!((moved.x, moved.y, moved.width), (200., 20., 40.));
    }

    #[test]
    fn undo_after_a_cut_restores_the_layout_exactly_and_redo_repeats_it() {
        let mut project = screenshot_project();
        project.background.padding = 12.;
        project
            .annotations
            .push(shape(AnnotationType::Arrow, 310.5, 20.25, -200., 40.));
        let mut bubble = shape(AnnotationType::Bubble, 150., 60., 120., 80.);
        bubble.tail = Some([140.75, 180.5]);
        project.annotations.push(bubble);
        let original = serde_json::to_value(&project).unwrap();
        let mut history = ProjectHistory::new(project.clone());

        let before = layout((640, 360), &project);
        let band = FrameBand {
            axis: ImageCutAxis::Columns,
            start: before.transform.offset.0 + 100.,
            length: 80.,
        };
        let after = cut_project(&mut project, (640, 360), &before.transform, band).unwrap();
        history.record(&project);
        let cut = serde_json::to_value(&project).unwrap();
        assert_ne!(cut, original);
        assert_eq!(after, layout(after.image_size, &project));

        let undone = history.undo().cloned().unwrap();
        assert_eq!(serde_json::to_value(&undone).unwrap(), original);
        assert_eq!(
            layout(cut_size((640, 360), &undone.image_cuts), &undone),
            before
        );

        let redone = history.redo().cloned().unwrap();
        assert_eq!(serde_json::to_value(&redone).unwrap(), cut);
        assert_eq!(
            layout(cut_size((640, 360), &redone.image_cuts), &redone),
            after
        );
    }

    #[test]
    fn a_cut_keeps_an_annotation_on_the_same_image_pixel_under_padding() {
        let mut project = screenshot_project();
        project.background.padding = 20.;
        let before = layout((800, 400), &project);
        let scale = before.transform.size.0 / 800.;
        let at_pixel = |pixel: f64, transform: &ImageTransform, width: f64| {
            transform.offset.0 + pixel * transform.size.0 / width
        };
        project.annotations.push(shape(
            AnnotationType::Rectangle,
            at_pixel(600., &before.transform, 800.),
            50.,
            40. * scale,
            30.,
        ));
        let band = FrameBand {
            axis: ImageCutAxis::Columns,
            start: at_pixel(200., &before.transform, 800.),
            length: 200. * scale,
        };
        let after = cut_project(&mut project, (800, 400), &before.transform, band).unwrap();
        assert_eq!(project.image_cuts, vec![columns(200, 200)]);
        let expected = at_pixel(400., &after.transform, 600.);
        assert!((project.annotations[0].x - expected).abs() < 1e-6);
    }

    #[test]
    fn an_older_project_opens_without_cuts_and_writes_no_cut_key() {
        let dir = tempfile::tempdir().unwrap();
        let mut document = serde_json::to_value(ProjectConfiguration::default()).unwrap();
        document.as_object_mut().unwrap().remove("imageCuts");
        std::fs::write(
            dir.path().join("project-config.json"),
            serde_json::to_string(&document).unwrap(),
        )
        .unwrap();
        let config = ProjectConfiguration::load(dir.path()).unwrap();
        assert!(config.image_cuts.is_empty());
        let written = serde_json::to_value(&config).unwrap();
        assert!(written.get("imageCuts").is_none());

        let mut cut = config.clone();
        cut.image_cuts.push(columns(3, 4));
        cut.write(dir.path()).unwrap();
        let reopened = ProjectConfiguration::load(dir.path()).unwrap();
        assert_eq!(reopened.image_cuts, vec![columns(3, 4)]);
    }

    #[test]
    fn only_images_and_cuts_need_the_current_format() {
        let mut project = screenshot_project();
        project
            .annotations
            .push(shape(AnnotationType::Rectangle, 0., 0., 10., 10.));
        assert!(!uses_current_format(&project));
        project.image_cuts.push(columns(0, 1));
        assert!(uses_current_format(&project));
        project.image_cuts.clear();
        project
            .annotations
            .push(shape(AnnotationType::Image, 0., 0., 10., 10.));
        assert!(uses_current_format(&project));
    }
}
