use crate::{recording::RecordingMode, session::Phase};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CameraLayout {
    pub frame: [f32; 4],
    pub toolbar_below: bool,
}

pub(super) fn camera_layout(bubble: [f32; 4], work: [f32; 4], snap: bool) -> CameraLayout {
    let [mut x, mut y, width, height] = bubble;
    let [left, top, work_width, work_height] = work;
    let max_x = left + (work_width - width).max(0.);
    let max_y = top + (work_height - height).max(0.);
    x = x.clamp(left, max_x);
    y = y.clamp(top, max_y);
    if snap {
        for (line, offset) in [
            (left + 16., 0.),
            (left + work_width / 2., width / 2.),
            (left + work_width - 16., width),
        ] {
            if (x + offset - line).abs() <= 18. {
                x = line - offset;
                break;
            }
        }
        for (line, offset) in [
            (top + 16., 0.),
            (top + work_height / 2., height / 2.),
            (top + work_height - 16., height),
        ] {
            if (y + offset - line).abs() <= 18. {
                y = line - offset;
                break;
            }
        }
    }
    let toolbar_below = y - top < super::CAMERA_TOOLBAR_HEIGHT;
    let window_height = height + super::CAMERA_TOOLBAR_HEIGHT;
    let window_y = if toolbar_below {
        y
    } else {
        y - super::CAMERA_TOOLBAR_HEIGHT
    };
    CameraLayout {
        frame: [
            x.clamp(left, max_x),
            window_y.clamp(top, top + (work_height - window_height).max(0.)),
            width,
            window_height,
        ],
        toolbar_below,
    }
}

pub(super) fn controls_allowed(inline: bool, phase: Phase, mode: Option<RecordingMode>) -> bool {
    !inline && (phase == Phase::Idle || mode == Some(RecordingMode::Studio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_motion_snaps_edges_and_corners_and_ctrl_keeps_the_free_position() {
        let work = [0., 0., 1280., 680.];
        assert_eq!(
            camera_layout([28., 30., 230., 230.], work, true),
            CameraLayout {
                frame: [16., 16., 230., 274.],
                toolbar_below: true,
            }
        );
        assert_eq!(
            camera_layout([28., 30., 230., 230.], work, false),
            CameraLayout {
                frame: [28., 30., 230., 274.],
                toolbar_below: true,
            }
        );
        assert_eq!(
            camera_layout([1020., 425., 230., 230.], work, true),
            CameraLayout {
                frame: [1034., 390., 230., 274.],
                toolbar_below: false,
            }
        );
    }

    #[test]
    fn camera_toolbar_flips_at_the_top_and_stays_in_a_negative_work_area() {
        assert_eq!(
            camera_layout(
                [-1200., -780., 230., 230.],
                [-1280., -800., 1280., 760.],
                false
            ),
            CameraLayout {
                frame: [-1200., -780., 230., 274.],
                toolbar_below: true,
            }
        );
        assert_eq!(
            camera_layout(
                [-1200., -700., 230., 230.],
                [-1280., -800., 1280., 760.],
                false
            ),
            CameraLayout {
                frame: [-1200., -744., 230., 274.],
                toolbar_below: false,
            }
        );
    }

    #[test]
    fn camera_instant_capture_hides_controls_for_every_live_phase_but_studio_keeps_them() {
        for phase in [
            Phase::Starting,
            Phase::Recording { paused: false },
            Phase::Recording { paused: true },
            Phase::Stopping,
        ] {
            assert!(!controls_allowed(
                false,
                phase,
                Some(RecordingMode::Instant)
            ));
            assert!(controls_allowed(false, phase, Some(RecordingMode::Studio)));
        }
        assert!(controls_allowed(
            false,
            Phase::Idle,
            Some(RecordingMode::Instant)
        ));
        assert!(!controls_allowed(true, Phase::Idle, None));
    }
}
