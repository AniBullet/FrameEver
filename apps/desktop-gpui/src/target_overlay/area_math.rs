use scap_targets::DisplayId;

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

pub(super) const QUICK_AREA_RATIOS: [Option<(u32, u32)>; 5] = [
    None,
    Some((16, 9)),
    Some((4, 3)),
    Some((1, 1)),
    Some((9, 16)),
];

pub(super) const AREA_SNAP_WINDOW_MIN: f32 = 48.;

pub(super) fn more_area_ratios() -> Vec<(u32, u32)> {
    AREA_RATIOS
        .iter()
        .flatten()
        .copied()
        .filter(|ratio| !QUICK_AREA_RATIOS.contains(&Some(*ratio)))
        .collect()
}

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

pub(super) fn stored_locked_area() -> Option<(String, AreaRect)> {
    let section = crate::store::store_section(AREA_SELECTION_STORE);
    let locked = section.get("locked")?;
    let display = locked.get("screenId")?.as_str()?.to_string();
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
        .then_some((display, bounds))
}

pub(super) fn store_locked_area(locked: Option<(&DisplayId, AreaRect)>) {
    let value = match locked {
        Some((display, bounds)) => serde_json::json!({
            "screenId": display.to_string(),
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
    display: &DisplayId,
    display_size: (f32, f32),
    min: f32,
) -> Option<AreaRect> {
    let (locked_display, bounds) = stored_locked_area()?;
    (locked_display == display.to_string())
        .then(|| bounds.clamped(display_size))
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
    fn quick_and_more_ratios_cover_every_ratio_once() {
        let more = more_area_ratios();
        assert_eq!(more, vec![(16, 10), (21, 9), (3, 2), (2, 1)]);
        let mut all: Vec<Option<(u32, u32)>> = QUICK_AREA_RATIOS.to_vec();
        all.extend(more.into_iter().map(Some));
        all.sort();
        let mut expected = AREA_RATIOS.to_vec();
        expected.sort();
        assert_eq!(all, expected);
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
}
