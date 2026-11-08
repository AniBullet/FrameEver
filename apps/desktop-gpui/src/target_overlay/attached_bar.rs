use super::AreaRect;

pub(super) const BAR_HEIGHT: f32 = 36.;
pub(super) const BAR_EDGE: f32 = 8.;
const BAR_GAP: f32 = 8.;
const SCREEN_INSET: f32 = 16.;
const SIZE_TAG_ROOM: f32 = 30.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BarTarget {
    Display,
    Pending,
    Rect(AreaRect),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BarSpot {
    Below,
    Above,
    Inside,
    BottomCentre,
    TopCentre,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BarPlacement {
    pub spot: BarSpot,
    pub top: f32,
    pub anchor_x: f32,
}

pub(super) fn place_bar(target: BarTarget, work: AreaRect) -> BarPlacement {
    let centre = work.x + work.width / 2.;
    let target = match target {
        BarTarget::Display => {
            return BarPlacement {
                spot: BarSpot::BottomCentre,
                top: work.bottom() - SCREEN_INSET - BAR_HEIGHT,
                anchor_x: centre,
            };
        }
        BarTarget::Pending => {
            return BarPlacement {
                spot: BarSpot::TopCentre,
                top: work.y + SCREEN_INSET,
                anchor_x: centre,
            };
        }
        BarTarget::Rect(target) => target,
    };
    let anchor_x = target
        .right()
        .min(work.right() - BAR_EDGE)
        .max(work.x + BAR_EDGE);
    let (spot, top) = if target.bottom() + BAR_GAP + BAR_HEIGHT <= work.bottom() {
        (BarSpot::Below, target.bottom() + BAR_GAP)
    } else if target.y - BAR_GAP - BAR_HEIGHT - SIZE_TAG_ROOM >= work.y {
        (BarSpot::Above, target.y - BAR_GAP - BAR_HEIGHT)
    } else {
        (
            BarSpot::Inside,
            target.bottom().min(work.bottom()) - BAR_GAP - BAR_HEIGHT,
        )
    };
    BarPlacement {
        spot,
        top,
        anchor_x,
    }
}

pub(super) fn work_area_on_display(
    display_origin: (f32, f32),
    display_size: (f32, f32),
    work_area: AreaRect,
) -> AreaRect {
    let left = (work_area.x - display_origin.0).max(0.);
    let top = (work_area.y - display_origin.1).max(0.);
    let right = (work_area.right() - display_origin.0).min(display_size.0);
    let bottom = (work_area.bottom() - display_origin.1).min(display_size.1);
    if right <= left || bottom <= top {
        return AreaRect {
            x: 0.,
            y: 0.,
            width: display_size.0,
            height: display_size.1,
        };
    }
    AreaRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
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

    struct Screen {
        name: &'static str,
        origin: (f32, f32),
        size: (f32, f32),
        work: AreaRect,
    }

    fn screens() -> Vec<Screen> {
        vec![
            Screen {
                name: "1920x1080 at 100%",
                origin: (0., 0.),
                size: (1920., 1080.),
                work: rect(0., 0., 1920., 1032.),
            },
            Screen {
                name: "2560x1440 at 150%",
                origin: (0., 0.),
                size: (1706.6666, 960.),
                work: rect(0., 0., 1706.6666, 928.),
            },
            Screen {
                name: "2560x1440 at 200%",
                origin: (0., 0.),
                size: (1280., 720.),
                work: rect(0., 0., 1280., 696.),
            },
            Screen {
                name: "second display right of the first at 150%",
                origin: (1920., -120.),
                size: (1280., 720.),
                work: rect(1920., -120., 1280., 688.),
            },
            Screen {
                name: "second display left of the first with no taskbar",
                origin: (-1920., 0.),
                size: (1920., 1080.),
                work: rect(-1920., 0., 1920., 1080.),
            },
        ]
    }

    fn local_work(screen: &Screen) -> AreaRect {
        work_area_on_display(screen.origin, screen.size, screen.work)
    }

    #[test]
    fn work_area_is_relative_to_its_own_display() {
        let left_taskbar =
            work_area_on_display((2560., 0.), (1280., 720.), rect(2608., 0., 1232., 720.));
        assert_eq!(left_taskbar, rect(48., 0., 1232., 720.));

        let above = work_area_on_display(
            (-200., -1080.),
            (1920., 1080.),
            rect(-200., -1080., 1920., 1040.),
        );
        assert_eq!(above, rect(0., 0., 1920., 1040.));
    }

    #[test]
    fn work_area_never_reaches_past_its_display() {
        let spill = work_area_on_display((0., 0.), (1280., 720.), rect(-10., 0., 2000., 800.));
        assert_eq!(spill, rect(0., 0., 1280., 720.));
    }

    #[test]
    fn a_target_with_room_below_gets_the_bar_8_px_under_its_right_edge() {
        for screen in screens() {
            let work = local_work(&screen);
            let target = rect(100., 100., 400., 300.);
            let placed = place_bar(BarTarget::Rect(target), work);
            assert_eq!(placed.spot, BarSpot::Below, "{}", screen.name);
            assert_eq!(placed.top, 408., "{}", screen.name);
            assert_eq!(placed.anchor_x, 500., "{}", screen.name);
        }
    }

    #[test]
    fn a_target_touching_the_taskbar_gets_the_bar_above_it() {
        for screen in screens() {
            let work = local_work(&screen);
            let target = rect(200., 300., 300., work.bottom() - 300. - 20.);
            let placed = place_bar(BarTarget::Rect(target), work);
            assert_eq!(placed.spot, BarSpot::Above, "{}", screen.name);
            assert_eq!(placed.top, 256., "{}", screen.name);
            assert_eq!(placed.anchor_x, 500., "{}", screen.name);
        }
    }

    #[test]
    fn above_needs_room_for_the_size_tag_too() {
        let work = rect(0., 0., 1280., 696.);
        let tall = rect(100., 73., 400., 600.);
        assert_eq!(place_bar(BarTarget::Rect(tall), work).spot, BarSpot::Inside);
        let room = rect(100., 74., 400., 600.);
        assert_eq!(place_bar(BarTarget::Rect(room), work).spot, BarSpot::Above);
    }

    #[test]
    fn a_target_filling_the_screen_gets_the_bar_inside_above_the_taskbar() {
        for screen in screens() {
            let work = local_work(&screen);
            let target = rect(0., 0., screen.size.0, screen.size.1);
            let placed = place_bar(BarTarget::Rect(target), work);
            assert_eq!(placed.spot, BarSpot::Inside, "{}", screen.name);
            assert_eq!(placed.top, work.bottom() - 44., "{}", screen.name);
            assert_eq!(placed.anchor_x, work.right() - 8., "{}", screen.name);
        }
    }

    #[test]
    fn inside_sits_on_the_target_bottom_when_it_ends_above_the_taskbar() {
        let work = rect(0., 0., 1280., 696.);
        let target = rect(0., 0., 600., 680.);
        let placed = place_bar(BarTarget::Rect(target), work);
        assert_eq!(placed.spot, BarSpot::Inside);
        assert_eq!(placed.top, 636.);
        assert_eq!(placed.anchor_x, 600.);
    }

    #[test]
    fn a_whole_display_gets_the_bar_at_the_bottom_centre() {
        for screen in screens() {
            let work = local_work(&screen);
            let placed = place_bar(BarTarget::Display, work);
            assert_eq!(placed.spot, BarSpot::BottomCentre, "{}", screen.name);
            assert_eq!(placed.top, work.bottom() - 52., "{}", screen.name);
            assert_eq!(placed.anchor_x, work.x + work.width / 2., "{}", screen.name);
        }
    }

    #[test]
    fn a_side_taskbar_moves_the_bottom_centre_with_the_work_area() {
        let work = rect(48., 0., 1232., 720.);
        let placed = place_bar(BarTarget::Display, work);
        assert_eq!(placed.top, 668.);
        assert_eq!(placed.anchor_x, 664.);
    }

    #[test]
    fn nothing_picked_yet_puts_the_bar_at_the_top_centre() {
        let work = rect(0., 0., 1280., 696.);
        let placed = place_bar(BarTarget::Pending, work);
        assert_eq!(placed.spot, BarSpot::TopCentre);
        assert_eq!(placed.top, 16.);
        assert_eq!(placed.anchor_x, 640.);
    }

    #[test]
    fn the_right_edge_stays_on_screen_for_a_target_past_the_edge() {
        let work = rect(0., 0., 1280., 696.);
        let past = rect(1000., 100., 400., 200.);
        assert_eq!(place_bar(BarTarget::Rect(past), work).anchor_x, 1272.);
        let off_left = rect(-500., 100., 502., 200.);
        assert_eq!(place_bar(BarTarget::Rect(off_left), work).anchor_x, 8.);
    }
}
