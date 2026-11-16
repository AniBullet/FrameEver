use super::{
    INPUT_DISPLAY_DEFAULT_SIZE, INPUT_DISPLAY_MARGIN, INPUT_DISPLAY_SIZE_RANGE, InputDisplay,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XY;

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
}
