use super::{
    INPUT_DISPLAY_DEFAULT_SIZE, INPUT_DISPLAY_MARGIN, INPUT_DISPLAY_SIZE_RANGE, InputDisplay,
};
use crate::XY;

pub const INPUT_DISPLAY_SNAP_STOPS: [f64; 3] = [0.0, 0.5, 1.0];
pub const INPUT_DISPLAY_SNAP_DISTANCE: f64 = 0.04;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn display_rect(
    block: &InputDisplay,
    preset_size: (u32, u32),
    frame_share: f64,
    output_size: (u32, u32),
) -> DisplayRect {
    let output_width = f64::from(output_size.0);
    let output_height = f64::from(output_size.1);
    let size = if block.size.is_finite() {
        block
            .size
            .clamp(INPUT_DISPLAY_SIZE_RANGE.0, INPUT_DISPLAY_SIZE_RANGE.1)
    } else {
        INPUT_DISPLAY_DEFAULT_SIZE
    };
    let width = output_width * frame_share * size / INPUT_DISPLAY_DEFAULT_SIZE;
    let height = width * f64::from(preset_size.1) / f64::from(preset_size.0.max(1));
    let margin = output_width * INPUT_DISPLAY_MARGIN;
    let along = |fraction: f64, extent: f64, length: f64| {
        let fraction = if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        margin + fraction * (extent - 2.0 * margin - length).max(0.0)
    };
    DisplayRect {
        x: along(block.position.x, output_width, width),
        y: along(block.position.y, output_height, height),
        width,
        height,
    }
}

pub fn position_for_origin(
    origin: (f64, f64),
    rect_size: (f64, f64),
    output_size: (u32, u32),
) -> XY<f64> {
    let output_width = f64::from(output_size.0);
    let margin = output_width * INPUT_DISPLAY_MARGIN;
    let along = |start: f64, extent: f64, length: f64| {
        let travel = extent - 2.0 * margin - length;
        if travel > 0.0 && start.is_finite() {
            ((start - margin) / travel).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    XY::new(
        along(origin.0, output_width, rect_size.0),
        along(origin.1, f64::from(output_size.1), rect_size.1),
    )
}

pub fn size_for_width(width: f64, frame_share: f64, output_width: u32) -> f64 {
    let full = f64::from(output_width) * frame_share;
    if full > 0.0 && width.is_finite() {
        (width / full * INPUT_DISPLAY_DEFAULT_SIZE)
            .round()
            .clamp(INPUT_DISPLAY_SIZE_RANGE.0, INPUT_DISPLAY_SIZE_RANGE.1)
    } else {
        INPUT_DISPLAY_DEFAULT_SIZE
    }
}

pub fn snap_position(position: XY<f64>) -> (XY<f64>, [Option<f64>; 2]) {
    let snap = |value: f64| {
        let value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let stop = INPUT_DISPLAY_SNAP_STOPS
            .into_iter()
            .find(|stop| (stop - value).abs() < INPUT_DISPLAY_SNAP_DISTANCE);
        (stop.unwrap_or(value), stop)
    };
    let (x, stop_x) = snap(position.x);
    let (y, stop_y) = snap(position.y);
    (XY::new(x, y), [stop_x, stop_y])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(x: f64, y: f64, size: f64) -> InputDisplay {
        InputDisplay {
            position: XY::new(x, y),
            size,
            ..Default::default()
        }
    }

    #[test]
    fn the_default_block_sits_bottom_left_inside_the_margin() {
        let rect = display_rect(&InputDisplay::default(), (400, 200), 0.3, (2000, 1000));

        assert_eq!(
            rect,
            DisplayRect {
                x: 60.0,
                y: 640.0,
                width: 600.0,
                height: 300.0
            }
        );
    }

    #[test]
    fn position_is_a_share_of_the_free_travel() {
        let centred = display_rect(&block(0.5, 0.5, 60.0), (400, 200), 0.3, (2000, 1000));
        let far = display_rect(&block(1.0, 0.0, 60.0), (400, 200), 0.3, (2000, 1000));

        assert_eq!((centred.x, centred.y), (700.0, 350.0));
        assert_eq!((far.x, far.y), (1340.0, 60.0));
    }

    #[test]
    fn size_scales_the_width_and_keeps_the_preset_shape() {
        let rect = display_rect(&block(0.0, 0.0, 120.0), (400, 200), 0.3, (2000, 1000));

        assert_eq!((rect.width, rect.height), (1200.0, 600.0));
    }

    #[test]
    fn the_same_block_keeps_its_relative_place_in_another_frame_shape() {
        let wide = display_rect(&block(1.0, 1.0, 60.0), (400, 200), 0.3, (2000, 1000));
        let tall = display_rect(&block(1.0, 1.0, 60.0), (400, 200), 0.3, (1000, 2000));

        assert_eq!(wide.x + wide.width, 2000.0 - 60.0);
        assert_eq!(wide.y + wide.height, 1000.0 - 60.0);
        assert_eq!(tall.x + tall.width, 1000.0 - 30.0);
        assert_eq!(tall.y + tall.height, 2000.0 - 30.0);
    }

    #[test]
    fn values_outside_their_ranges_are_clamped() {
        let rect = display_rect(&block(7.0, f64::NAN, 5000.0), (400, 200), 0.3, (2000, 1000));

        assert_eq!((rect.width, rect.x, rect.y), (1200.0, 740.0, 60.0));
    }

    #[test]
    fn a_place_on_the_frame_reads_back_as_the_same_share_of_travel() {
        for (x, y) in [(0.0, 1.0), (0.5, 0.5), (0.25, 0.8), (1.0, 0.0)] {
            let rect = display_rect(&block(x, y, 90.0), (400, 200), 0.3, (1920, 1080));
            let position =
                position_for_origin((rect.x, rect.y), (rect.width, rect.height), (1920, 1080));

            assert!((position.x - x).abs() < 1e-9, "{x} {y}");
            assert!((position.y - y).abs() < 1e-9, "{x} {y}");
        }
    }

    #[test]
    fn a_place_outside_the_margin_is_pulled_inside_it() {
        let position = position_for_origin((-500.0, 5000.0), (600.0, 300.0), (2000, 1000));

        assert_eq!((position.x, position.y), (0.0, 1.0));
        assert_eq!(
            position_for_origin((100.0, 100.0), (1990.0, 990.0), (2000, 1000)),
            XY::new(0.0, 0.0)
        );
    }

    #[test]
    fn a_drawn_width_reads_back_as_a_whole_size_inside_the_range() {
        assert_eq!(size_for_width(600.0, 0.3, 2000), 60.0);
        assert_eq!(size_for_width(904.0, 0.3, 2000), 90.0);
        assert_eq!(size_for_width(10.0, 0.3, 2000), 30.0);
        assert_eq!(size_for_width(90000.0, 0.3, 2000), 120.0);
    }

    #[test]
    fn a_position_near_the_margin_or_the_centre_snaps_to_it() {
        let (snapped, stops) = snap_position(XY::new(0.03, 0.47));
        assert_eq!(
            (snapped, stops),
            (XY::new(0.0, 0.5), [Some(0.0), Some(0.5)])
        );

        let (snapped, stops) = snap_position(XY::new(0.97, 0.3));
        assert_eq!((snapped, stops), (XY::new(1.0, 0.3), [Some(1.0), None]));

        let (snapped, stops) = snap_position(XY::new(0.05, 1.7));
        assert_eq!((snapped, stops), (XY::new(0.05, 1.0), [None, Some(1.0)]));
    }
}
