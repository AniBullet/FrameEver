use super::{AREA_SELECTION_STORE, AreaHandle, AreaRect};
use crate::editor_canvas::{
    GuideAxis, NormRect, SNAP_PX, SnapGuide, build_snap_targets, snap_moving_rect,
};

pub(super) const AREA_RATIOS: [Option<(u32, u32)>; 9] = [
    None,
    Some((16, 9)),
    Some((4, 3)),
    Some((1, 1)),
    Some((9, 16)),
    Some((16, 10)),
    Some((21, 9)),
    Some((3, 2)),
    Some((2, 1)),
];

pub(super) const AREA_SNAP_WINDOW_MIN: f32 = 48.;

pub(super) fn ratio_value((width, height): (u32, u32)) -> f32 {
    width as f32 / height.max(1) as f32
}

pub(super) fn stored_area_ratio() -> Option<(u32, u32)> {
    let section = crate::store::store_section(AREA_SELECTION_STORE);
    let ratio = section.get("aspectRatio")?.as_array()?;
    let [width, height] = ratio.as_slice() else {
        return None;
    };
    let ratio = (width.as_u64()? as u32, height.as_u64()? as u32);
    AREA_RATIOS.contains(&Some(ratio)).then_some(ratio)
}

pub(super) fn store_area_ratio(ratio: Option<(u32, u32)>) {
    crate::store::set_store_setting(
        AREA_SELECTION_STORE,
        "aspectRatio",
        ratio.map_or(serde_json::Value::Null, |(width, height)| {
            serde_json::json!([width, height])
        }),
    );
}

pub(super) fn stored_area_snap_to_ratio() -> bool {
    crate::store::store_section(AREA_SELECTION_STORE)
        .get("snapToRatio")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn store_area_snap_to_ratio(enabled: bool) {
    crate::store::set_store_setting(AREA_SELECTION_STORE, "snapToRatio", enabled.into());
}

const RATIO_SNAP_MIN: f32 = 30.;

pub(super) fn snap_area_to_ratio(
    rect: AreaRect,
    anchor: (f32, f32),
    display_size: (f32, f32),
    held_x: bool,
    held_y: bool,
) -> Option<AreaRect> {
    if rect.width <= RATIO_SNAP_MIN || rect.height <= RATIO_SNAP_MIN || (held_x && held_y) {
        return None;
    }
    let ratio = crate::editor_crop::find_closest_ratio(
        f64::from(rect.width),
        f64::from(rect.height),
        crate::editor_crop::RATIO_SNAP_THRESHOLD,
    )?;
    let value = ratio_value(ratio);
    let (width, height) = if held_x {
        (rect.width.round(), (rect.width / value).round())
    } else {
        ((rect.height * value).round(), rect.height.round())
    };
    let snapped = AreaRect {
        x: if rect.x < anchor.0 {
            anchor.0 - width
        } else {
            anchor.0
        },
        y: if rect.y < anchor.1 {
            anchor.1 - height
        } else {
            anchor.1
        },
        width,
        height,
    };
    const EPSILON: f32 = 0.01;
    (snapped.x >= -EPSILON
        && snapped.y >= -EPSILON
        && snapped.right() <= display_size.0 + EPSILON
        && snapped.bottom() <= display_size.1 + EPSILON)
        .then_some(snapped)
}

pub(super) fn exact_common_ratio(area: AreaRect) -> Option<(u32, u32)> {
    if area.width <= 0. || area.height <= 0. {
        return None;
    }
    crate::editor_crop::COMMON_RATIOS
        .into_iter()
        .flat_map(|(a, b)| [(a, b), (b, a)])
        .find(|ratio| {
            let value = ratio_value(*ratio);
            (area.width / area.height - value).abs() <= 0.5 * value.max(1.) / area.height
        })
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct AreaScreen {
    pub(super) id: String,
    pub(super) uuid: Option<String>,
}

impl AreaScreen {
    pub(super) fn owns(&self, locked: &LockedArea) -> bool {
        match (locked.screen_uuid.as_deref(), self.uuid.as_deref()) {
            (Some(saved), Some(current)) => saved == current,
            _ => locked.screen_id == self.id,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct LockedArea {
    pub(super) screen_id: String,
    pub(super) screen_uuid: Option<String>,
    pub(super) bounds: AreaRect,
}

pub(super) fn stored_locked_area() -> Option<LockedArea> {
    let section = crate::store::store_section(AREA_SELECTION_STORE);
    let locked = section.get("locked")?;
    let screen_id = locked.get("screenId")?.as_str()?.to_string();
    let screen_uuid = locked
        .get("screenUuid")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let bounds = locked.get("bounds")?;
    let value = |key: &str| bounds.get(key)?.as_f64().map(|value| value as f32);
    let bounds = AreaRect {
        x: value("x")?,
        y: value("y")?,
        width: value("width")?,
        height: value("height")?,
    };
    [bounds.x, bounds.y, bounds.width, bounds.height]
        .iter()
        .all(|value| value.is_finite())
        .then_some(LockedArea {
            screen_id,
            screen_uuid,
            bounds,
        })
}

pub(super) fn store_locked_area(locked: Option<(&AreaScreen, AreaRect)>) {
    let value = match locked {
        Some((screen, bounds)) => serde_json::json!({
            "screenId": screen.id,
            "screenUuid": screen.uuid,
            "bounds": {
                "x": bounds.x,
                "y": bounds.y,
                "width": bounds.width,
                "height": bounds.height,
            },
        }),
        None => serde_json::Value::Null,
    };
    crate::store::set_store_setting(AREA_SELECTION_STORE, "locked", value);
}

pub(super) fn locked_area_for(
    screen: &AreaScreen,
    display_size: (f32, f32),
    min: f32,
) -> Option<AreaRect> {
    let locked = stored_locked_area()?;
    screen
        .owns(&locked)
        .then(|| locked.bounds.clamped(display_size))
        .filter(|bounds| bounds.is_valid_for(min))
}

pub(super) fn norm_area_rect(rect: AreaRect, display_size: (f32, f32)) -> NormRect {
    let width = f64::from(display_size.0.max(1.));
    let height = f64::from(display_size.1.max(1.));
    NormRect {
        x: f64::from(rect.x) / width,
        y: f64::from(rect.y) / height,
        w: f64::from(rect.width) / width,
        h: f64::from(rect.height) / height,
    }
}

pub(super) fn snap_area_rect(
    rect: AreaRect,
    display_size: (f32, f32),
    others: &[NormRect],
) -> (AreaRect, Vec<SnapGuide>) {
    let rect = rect.clamped(display_size);
    let targets = build_snap_targets(others, None);
    let normalized = norm_area_rect(rect, display_size);
    let (dx, dy, guides) = snap_moving_rect(
        normalized,
        &targets,
        SNAP_PX / f64::from(display_size.0.max(1.)),
        SNAP_PX / f64::from(display_size.1.max(1.)),
    );
    (
        AreaRect {
            x: rect.x + (dx * f64::from(display_size.0)) as f32,
            y: rect.y + (dy * f64::from(display_size.1)) as f32,
            ..rect
        }
        .clamped(display_size),
        guides,
    )
}

pub(super) fn snap_area_point(
    point: (f32, f32),
    display_size: (f32, f32),
    others: &[NormRect],
) -> ((f32, f32), Vec<SnapGuide>) {
    let rect = AreaRect {
        x: point.0,
        y: point.1,
        width: 0.,
        height: 0.,
    };
    let (snapped, guides) = snap_area_rect(rect, display_size, others);
    ((snapped.x, snapped.y), guides)
}

pub(super) fn snap_resize_point(
    point: (f32, f32),
    display_size: (f32, f32),
    others: &[NormRect],
    horizontal: bool,
    vertical: bool,
) -> ((f32, f32), Vec<SnapGuide>) {
    let ((x, y), guides) = snap_area_point(point, display_size, others);
    let guides = guides
        .into_iter()
        .filter(|guide| match guide.axis {
            GuideAxis::V => horizontal,
            GuideAxis::H => vertical,
        })
        .collect();
    (
        (
            if horizontal { x } else { point.0 },
            if vertical { y } else { point.1 },
        ),
        guides,
    )
}

pub(super) fn area_snap_rects(
    work_area: AreaRect,
    windows: &[AreaRect],
    display_size: (f32, f32),
) -> Vec<NormRect> {
    let whole = |rect: &AreaRect| {
        rect.x.abs() < 1.
            && rect.y.abs() < 1.
            && (rect.width - display_size.0).abs() < 1.
            && (rect.height - display_size.1).abs() < 1.
    };
    std::iter::once(work_area)
        .chain(windows.iter().copied())
        .filter_map(|rect| rect.intersected(display_size))
        .filter(|rect| {
            rect.width >= AREA_SNAP_WINDOW_MIN
                && rect.height >= AREA_SNAP_WINDOW_MIN
                && !whole(rect)
        })
        .map(|rect| norm_area_rect(rect, display_size))
        .collect()
}

pub(super) fn area_fill(display: (f32, f32), ratio: Option<f32>) -> AreaRect {
    let (width, height) = match ratio {
        Some(ratio) if display.0 / display.1 > ratio => (display.1 * ratio, display.1),
        Some(ratio) => (display.0, display.0 / ratio),
        None => display,
    };
    AreaRect {
        x: (display.0 - width) / 2.,
        y: (display.1 - height) / 2.,
        width,
        height,
    }
}

pub(super) fn area_between(from: AreaRect, to: AreaRect, t: f32) -> AreaRect {
    let t = 1. - (1. - t.clamp(0., 1.)).powi(3);
    let mix = |a: f32, b: f32| a + (b - a) * t;
    AreaRect {
        x: mix(from.x, to.x),
        y: mix(from.y, to.y),
        width: mix(from.width, to.width),
        height: mix(from.height, to.height),
    }
}

pub(super) fn area_from_anchor_with_ratio(
    anchor: (f32, f32),
    pointer: (f32, f32),
    ratio: f32,
    display: (f32, f32),
) -> AreaRect {
    let (dx, dy) = (pointer.0 - anchor.0, pointer.1 - anchor.1);
    let (mut width, mut height) = (dx.abs(), dy.abs());
    if width / ratio >= height {
        height = width / ratio;
    } else {
        width = height * ratio;
    }
    let room_x = if dx >= 0. {
        display.0 - anchor.0
    } else {
        anchor.0
    };
    let room_y = if dy >= 0. {
        display.1 - anchor.1
    } else {
        anchor.1
    };
    let fit = [
        1.,
        room_x / width.max(f32::EPSILON),
        room_y / height.max(f32::EPSILON),
    ]
    .into_iter()
    .fold(f32::INFINITY, f32::min)
    .max(0.);
    width *= fit;
    height *= fit;
    AreaRect {
        x: if dx >= 0. { anchor.0 } else { anchor.0 - width },
        y: if dy >= 0. {
            anchor.1
        } else {
            anchor.1 - height
        },
        width,
        height,
    }
}

pub(super) fn area_edge_resize_with_ratio(
    handle: AreaHandle,
    start: AreaRect,
    pointer: (f32, f32),
    ratio: f32,
    display: (f32, f32),
) -> AreaRect {
    let (center_x, center_y) = (start.x + start.width / 2., start.y + start.height / 2.);
    if handle.east() || handle.west() {
        let anchor = if handle.west() {
            start.right()
        } else {
            start.x
        };
        let before = pointer.0 < anchor;
        let room_x = if before { anchor } else { display.0 - anchor };
        let room_y = 2. * center_y.min(display.1 - center_y);
        let width = (pointer.0 - anchor)
            .abs()
            .min(room_x)
            .min(room_y * ratio)
            .max(0.);
        let height = width / ratio;
        AreaRect {
            x: if before { anchor - width } else { anchor },
            y: center_y - height / 2.,
            width,
            height,
        }
    } else {
        let anchor = if handle.north() {
            start.bottom()
        } else {
            start.y
        };
        let before = pointer.1 < anchor;
        let room_y = if before { anchor } else { display.1 - anchor };
        let room_x = 2. * center_x.min(display.0 - center_x);
        let height = (pointer.1 - anchor)
            .abs()
            .min(room_y)
            .min(room_x / ratio)
            .max(0.);
        let width = height * ratio;
        AreaRect {
            x: center_x - width / 2.,
            y: if before { anchor - height } else { anchor },
            width,
            height,
        }
    }
}

pub(super) fn area_resize_with_ratio(
    handle: AreaHandle,
    start: AreaRect,
    pointer: (f32, f32),
    ratio: f32,
    display: (f32, f32),
) -> AreaRect {
    if handle.is_corner() {
        let anchor = (
            if handle.west() {
                start.right()
            } else {
                start.x
            },
            if handle.north() {
                start.bottom()
            } else {
                start.y
            },
        );
        area_from_anchor_with_ratio(anchor, pointer, ratio, display)
    } else {
        area_edge_resize_with_ratio(handle, start, pointer, ratio, display)
    }
}

pub(super) fn area_with_ratio(area: AreaRect, ratio: f32, display: (f32, f32)) -> AreaRect {
    let (center_x, center_y) = (area.x + area.width / 2., area.y + area.height / 2.);
    let mut width = area.width.min(display.0);
    let mut height = width / ratio;
    if height > display.1 {
        height = display.1;
        width = height * ratio;
    }
    AreaRect {
        x: center_x - width / 2.,
        y: center_y - height / 2.,
        width,
        height,
    }
    .clamped(display)
}

pub(super) fn area_resized_by(
    crop: AreaRect,
    (dw, dh): (f32, f32),
    min: f32,
    display: (f32, f32),
) -> AreaRect {
    let grow = |size: f32, delta: f32, room: f32| (size + delta).min(room).max(min.min(room));
    AreaRect {
        width: grow(crop.width, dw, display.0 - crop.x),
        height: grow(crop.height, dh, display.1 - crop.y),
        ..crop
    }
}

pub(super) fn drag_ratio(chosen: Option<f32>, keep: bool, start: Option<AreaRect>) -> Option<f32> {
    chosen.or_else(|| {
        start
            .filter(|start| keep && start.width > 0. && start.height > 0.)
            .map(|start| start.width / start.height)
    })
}

pub(super) fn area_exact_size(
    crop: Option<AreaRect>,
    pixels: (u32, u32),
    scale: f32,
    display: (f32, f32),
) -> Option<AreaRect> {
    let (width, height) = (pixels.0 as f32 / scale, pixels.1 as f32 / scale);
    if width > display.0 + 0.5 || height > display.1 + 0.5 {
        return None;
    }
    let (width, height) = (width.min(display.0), height.min(display.1));
    let (cx, cy) = crop.map_or((display.0 / 2., display.1 / 2.), |crop| {
        (crop.x + crop.width / 2., crop.y + crop.height / 2.)
    });
    Some(AreaRect {
        x: (cx - width / 2.).clamp(0., display.0 - width),
        y: (cy - height / 2.).clamp(0., display.1 - height),
        width,
        height,
    })
}

pub(super) fn fit_candidate(
    windows: &[AreaRect],
    point: (f32, f32),
    area: Option<AreaRect>,
) -> Option<AreaRect> {
    let contains = |rect: &AreaRect| {
        (rect.x..rect.right()).contains(&point.0) && (rect.y..rect.bottom()).contains(&point.1)
    };
    if area.as_ref().is_some_and(contains) {
        return None;
    }
    windows.iter().copied().find(contains)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> AreaRect {
        AreaRect {
            x,
            y,
            width,
            height,
        }
    }

    fn ratio_of(area: AreaRect) -> f32 {
        area.width / area.height
    }

    fn inside(area: AreaRect, display: (f32, f32)) -> bool {
        area.x >= -0.01
            && area.y >= -0.01
            && area.right() <= display.0 + 0.01
            && area.bottom() <= display.1 + 0.01
    }

    #[test]
    fn a_drawn_area_keeps_the_chosen_aspect_ratio() {
        let display = (1920., 1080.);
        let ratio = ratio_value((16, 9));
        for pointer in [(900., 300.), (100., 900.), (1500., 1000.), (120., 140.)] {
            let area = area_from_anchor_with_ratio((800., 500.), pointer, ratio, display);
            assert!(
                (ratio_of(area) - ratio).abs() < 0.001,
                "{pointer:?}: {area:?}"
            );
            assert!(inside(area, display), "{pointer:?}: {area:?}");
        }
        let wide = area_from_anchor_with_ratio((100., 100.), (900., 300.), ratio, display);
        assert_eq!((wide.x, wide.y, wide.width), (100., 100., 800.));
        assert!((wide.height - 450.).abs() < 0.01);
        let edge = area_from_anchor_with_ratio((1800., 100.), (1900., 1000.), ratio, display);
        assert!(inside(edge, display) && (ratio_of(edge) - ratio).abs() < 0.001);
    }

    #[test]
    fn resizing_an_area_keeps_the_chosen_aspect_ratio() {
        let display = (1920., 1080.);
        let ratio = ratio_value((1, 1));
        let start = rect(400., 300., 400., 400.);
        for handle in AreaHandle::ALL {
            for pointer in [(1000., 900.), (100., 50.), (650., 520.)] {
                let area = area_resize_with_ratio(handle, start, pointer, ratio, display);
                assert!(
                    area.height == 0. || (ratio_of(area) - ratio).abs() < 0.001,
                    "{handle:?} {pointer:?}: {area:?}"
                );
                assert!(inside(area, display), "{handle:?} {pointer:?}: {area:?}");
            }
        }
        let east =
            area_edge_resize_with_ratio(AreaHandle::East, start, (1000., 0.), ratio, display);
        assert_eq!((east.x, east.width), (400., 600.));
        assert_eq!(east.y + east.height / 2., 500.);
        let crossed =
            area_edge_resize_with_ratio(AreaHandle::East, start, (300., 0.), ratio, display);
        assert_eq!((crossed.x, crossed.width), (300., 100.));
        let crossed =
            area_edge_resize_with_ratio(AreaHandle::South, start, (0., 200.), ratio, display);
        assert_eq!((crossed.y, crossed.height), (200., 100.));
        let crossed =
            area_edge_resize_with_ratio(AreaHandle::West, start, (900., 0.), ratio, display);
        assert_eq!((crossed.x, crossed.width), (800., 100.));
    }

    #[test]
    fn fill_takes_the_display_or_the_largest_centred_area_of_the_ratio() {
        let display = (1512., 982.);
        let free = area_fill(display, None);
        assert_eq!(
            (free.x, free.y, free.width, free.height),
            (0., 0., 1512., 982.)
        );
        let wide = area_fill(display, Some(16. / 9.));
        assert_eq!((wide.x, wide.width), (0., 1512.));
        assert!((wide.height - 850.5).abs() < 0.01);
        assert!((wide.y - (982. - wide.height) / 2.).abs() < 0.01);
        let tall = area_fill(display, Some(9. / 16.));
        assert_eq!((tall.y, tall.height), (0., 982.));
        assert!((tall.width - 552.375).abs() < 0.01);
        assert!((tall.x - (1512. - tall.width) / 2.).abs() < 0.01);
    }

    #[test]
    fn a_reshape_starts_and_ends_exactly_and_moves_monotonically() {
        let from = rect(10., 20., 300., 200.);
        let to = rect(100., 50., 640., 360.);
        assert_eq!(area_between(from, to, 0.), from);
        assert_eq!(area_between(from, to, 1.), to);
        assert_eq!(area_between(from, to, 3.), to);
        let mut previous = from.width;
        for step in 1..=20 {
            let width = area_between(from, to, step as f32 / 20.).width;
            assert!(width >= previous);
            previous = width;
        }
        assert!(area_between(from, to, 0.5).width > (from.width + to.width) / 2.);
    }

    #[test]
    fn choosing_a_ratio_reshapes_the_area_around_its_centre() {
        let display = (1920., 1080.);
        let area = area_with_ratio(rect(100., 300., 800., 200.), ratio_value((16, 9)), display);
        assert_eq!((area.x, area.width), (100., 800.));
        assert!((area.height - 450.).abs() < 0.01);
        assert!((area.y + area.height / 2. - 400.).abs() < 0.01);
        assert!(inside(area, display));
    }

    #[test]
    fn an_area_snaps_to_windows_and_the_work_area() {
        let display = (1000., 800.);
        let window = rect(200., 150., 400., 300.);
        let work_area = rect(0., 25., 1000., 700.);
        let others = area_snap_rects(work_area, &[window], display);

        let (moved, guides) = snap_area_rect(rect(204., 500., 120., 90.), display, &others);
        assert_eq!(moved.x, 200.);
        assert!(guides.iter().any(|guide| guide.axis == GuideAxis::V));

        let ((_, y), _) = snap_area_point((700., 722.), display, &others);
        assert_eq!(y, 725.);
    }

    #[test]
    fn an_edge_handle_snaps_only_its_own_axis() {
        let display = (1000., 800.);
        let others = area_snap_rects(
            rect(0., 0., 1000., 800.),
            &[rect(300., 300., 200., 200.)],
            display,
        );
        let ((x, y), guides) = snap_resize_point((303., 297.), display, &others, true, false);
        assert_eq!((x, y), (300., 297.));
        assert!(guides.iter().all(|guide| guide.axis == GuideAxis::V));
    }

    #[test]
    fn snap_rects_drop_tiny_whole_display_and_off_screen_windows() {
        let display = (1000., 800.);
        let whole = rect(0., 0., 1000., 800.);
        let windows = [
            rect(10., 10., 20., 20.),
            whole,
            rect(1200., 0., 400., 400.),
            rect(800., 100., 400., 300.),
        ];
        let rects = area_snap_rects(whole, &windows, display);
        assert_eq!(rects.len(), 1);
        assert!((rects[0].x - 0.8).abs() < 1e-6);
        assert!((rects[0].w - 0.2).abs() < 1e-6);
    }

    #[test]
    fn scaled_overlay_selections_preserve_the_physical_capture_region() {
        let capture = rect(300., 240., 900., 600.);
        for (scale, expected) in [
            (1., rect(300., 240., 900., 600.)),
            (1.5, rect(200., 160., 600., 400.)),
            (2., rect(150., 120., 450., 300.)),
        ] {
            let overlay = capture.scaled(1. / scale);
            assert_eq!(overlay, expected);
            let bounds = overlay.scaled(scale).to_bounds();
            assert_eq!(bounds.position().x(), 300.);
            assert_eq!(bounds.position().y(), 240.);
            assert_eq!(bounds.size().width(), 900.);
            assert_eq!(bounds.size().height(), 600.);
        }
    }

    #[test]
    fn keyboard_resize_moves_the_right_and_bottom_edges_within_the_display_and_min() {
        let display = (1920., 1080.);
        let crop = rect(100., 100., 400., 300.);
        assert_eq!(
            area_resized_by(crop, (10., -1.), 150., display),
            rect(100., 100., 410., 299.)
        );
        assert_eq!(
            area_resized_by(crop, (-500., 0.), 150., display),
            rect(100., 100., 150., 300.)
        );
        assert_eq!(
            area_resized_by(rect(1800., 900., 100., 160.), (50., 50.), 1., display),
            rect(1800., 900., 120., 180.)
        );
    }

    #[test]
    fn shift_keeps_the_ratio_the_resize_started_with_unless_a_ratio_is_chosen() {
        let start = rect(0., 0., 400., 200.);
        assert_eq!(drag_ratio(None, true, Some(start)), Some(2.));
        assert_eq!(drag_ratio(None, false, Some(start)), None);
        assert_eq!(drag_ratio(Some(1.), true, Some(start)), Some(1.));
        assert_eq!(drag_ratio(None, true, None), None);
        assert_eq!(drag_ratio(None, true, Some(rect(0., 0., 400., 0.))), None);
    }

    #[test]
    fn an_exact_size_is_recorded_pixels_centred_on_the_area_and_refused_when_too_big() {
        let display = (1920., 1080.);
        assert_eq!(
            area_exact_size(Some(rect(0., 0., 400., 400.)), (1280, 720), 1., display),
            Some(rect(0., 0., 1280., 720.))
        );
        assert_eq!(
            area_exact_size(None, (1920, 1080), 2., display),
            Some(rect(480., 270., 960., 540.))
        );
        assert_eq!(
            area_exact_size(None, (1920, 1080), 1.5, (1280., 720.)),
            Some(rect(0., 0., 1280., 720.))
        );
        assert_eq!(area_exact_size(None, (3840, 2160), 1., display), None);
    }

    #[test]
    fn the_fit_candidate_is_the_frontmost_window_under_the_pointer_outside_the_area() {
        let front = rect(100., 100., 400., 300.);
        let back = rect(0., 0., 1000., 800.);
        let windows = [front, back];
        assert_eq!(fit_candidate(&windows, (150., 150.), None), Some(front));
        assert_eq!(fit_candidate(&windows, (700., 700.), None), Some(back));
        assert_eq!(fit_candidate(&windows, (1500., 900.), None), None);
        let area = rect(120., 120., 50., 50.);
        assert_eq!(fit_candidate(&windows, (130., 130.), Some(area)), None);
        assert_eq!(
            fit_candidate(&windows, (300., 300.), Some(area)),
            Some(front)
        );
    }

    fn drawn_from(anchor: (f32, f32), direction: (f32, f32), width: f32, height: f32) -> AreaRect {
        rect(
            if direction.0 < 0. {
                anchor.0 - width
            } else {
                anchor.0
            },
            if direction.1 < 0. {
                anchor.1 - height
            } else {
                anchor.1
            },
            width,
            height,
        )
    }

    fn pinned_corner(area: AreaRect, direction: (f32, f32)) -> (f32, f32) {
        (
            if direction.0 < 0. {
                area.right()
            } else {
                area.x
            },
            if direction.1 < 0. {
                area.bottom()
            } else {
                area.y
            },
        )
    }

    #[test]
    fn a_free_area_near_every_ratio_snaps_onto_it_from_every_corner() {
        let directions = [(1., 1.), (-1., 1.), (1., -1.), (-1., -1.)];
        for display in [(1512., 982.), (2560., 1440.), (1080., 1920.)] {
            let anchor = (display.0 / 2., display.1 / 2.);
            for (a, b) in crate::editor_crop::COMMON_RATIOS {
                for ratio in [(a, b), (b, a)] {
                    let value = ratio_value(ratio);
                    let height = 200.;
                    for offset in [-0.008, 0.005] {
                        let width = height * (value + offset);
                        for direction in directions {
                            let drawn = drawn_from(anchor, direction, width, height);
                            let snapped = snap_area_to_ratio(drawn, anchor, display, false, false)
                                .unwrap_or_else(|| panic!("{ratio:?} {direction:?}"));
                            assert_eq!(exact_common_ratio(snapped), Some(ratio));
                            assert_eq!(snapped.height, height);
                            assert!(
                                (ratio_of(snapped) - value).abs() <= 0.5 / snapped.height,
                                "{ratio:?} {snapped:?}"
                            );
                            assert_eq!(pinned_corner(snapped, direction), anchor);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_free_area_far_from_any_ratio_or_too_small_stays_as_drawn() {
        let display = (1920., 1080.);
        let anchor = (100., 100.);
        assert_eq!(
            snap_area_to_ratio(rect(100., 100., 612., 600.), anchor, display, false, false),
            None
        );
        assert_eq!(
            snap_area_to_ratio(rect(100., 100., 30., 30.), anchor, display, false, false),
            None
        );
    }

    #[test]
    fn a_held_vertical_edge_lets_the_width_lead_and_holding_both_does_not_snap() {
        let display = (1920., 1080.);
        let anchor = (100., 100.);
        let drawn = rect(100., 100., 800., 451.);
        let snapped = snap_area_to_ratio(drawn, anchor, display, true, false).expect("snaps");
        assert_eq!((snapped.width, snapped.height), (800., 450.));
        assert_eq!(snap_area_to_ratio(drawn, anchor, display, true, true), None);
    }

    #[test]
    fn a_snap_that_would_leave_the_display_does_not_happen() {
        let display = (1920., 1080.);
        let anchor = (1798., 100.);
        let drawn = rect(1798., 100., 122., 69.);
        assert_eq!(
            snap_area_to_ratio(drawn, anchor, display, false, false),
            None
        );
    }

    #[test]
    fn the_size_tag_names_a_common_ratio_only_when_the_area_lands_on_it() {
        assert_eq!(
            exact_common_ratio(rect(0., 0., 1920., 1080.)),
            Some((16, 9))
        );
        assert_eq!(exact_common_ratio(rect(0., 0., 356., 200.)), Some((16, 9)));
        assert_eq!(
            exact_common_ratio(rect(0., 0., 1080., 1920.)),
            Some((9, 16))
        );
        assert_eq!(exact_common_ratio(rect(0., 0., 801., 401.)), Some((2, 1)));
        assert_eq!(exact_common_ratio(rect(0., 0., 612., 600.)), None);
        assert_eq!(exact_common_ratio(rect(0., 0., 0., 600.)), None);
    }

    #[test]
    fn a_lock_follows_its_display_by_uuid_and_falls_back_to_the_id() {
        let bounds = rect(560., 315., 800., 450.);
        let screen = |id: &str, uuid: Option<&str>| AreaScreen {
            id: id.into(),
            uuid: uuid.map(str::to_string),
        };
        let locked = |id: &str, uuid: Option<&str>| LockedArea {
            screen_id: id.into(),
            screen_uuid: uuid.map(str::to_string),
            bounds,
        };
        let saved = locked("34", Some("UUID-A"));
        assert!(screen("34", Some("UUID-A")).owns(&saved));
        assert!(screen("46", Some("UUID-A")).owns(&saved));
        assert!(!screen("46", Some("UUID-B")).owns(&saved));
        assert!(!screen("34", Some("UUID-B")).owns(&saved));
        assert!(!screen("46", None).owns(&saved));
        assert!(screen("34", None).owns(&saved));
        let without_uuid = locked("34", None);
        assert!(screen("34", Some("UUID-A")).owns(&without_uuid));
        assert!(!screen("46", Some("UUID-A")).owns(&without_uuid));
    }
}
