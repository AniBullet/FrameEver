use std::time::Duration;

use crate::store::MainWindowMemory;
use crate::theme::motion;

pub const COMPACT_WIDTH: f64 = crate::MAIN_WINDOW_WIDTH as f64;
pub const COMPACT_HEIGHT: f64 = crate::MAIN_WINDOW_HEIGHT as f64;
const FIRST_EXPANDED_WIDTH: f64 = 1200.;
const FIRST_EXPANDED_HEIGHT: f64 = 760.;
const MIN_EXPANDED_WIDTH: f64 = 900.;
const MIN_EXPANDED_HEIGHT: f64 = 600.;
const WORK_AREA_MARGIN: f64 = 12.;
const FIRST_EXPANDED_SHARE: f64 = 0.9;
const SAME_SPOT_TOLERANCE: f64 = 1.;
const LIBRARY_SLIDE: f32 = 24.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub frame: Frame,
    pub work_area: Frame,
    pub scale: f64,
    pub maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub frame: Frame,
    pub maximized: bool,
    pub on_top: bool,
    pub resizable: bool,
    pub min_width: f64,
    pub min_height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Compact,
    OpeningFrame,
    LibraryArriving,
    Expanded,
    LibraryLeaving,
    ClosingFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Expand,
    Collapse,
    Toggle,
    FrameSnapped,
    FadeFinished,
    HideToTray,
    ReturnToCompact,
    Track,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    Snap { placement: Placement, hide: bool },
    Fade(Duration),
    Persist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EscLayers {
    pub warning: bool,
    pub error: bool,
    pub mode_hover: bool,
    pub search_text: bool,
    pub search_focused: bool,
    pub panel: bool,
    pub clip_flow: bool,
    pub library_menu: bool,
    pub library_selection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscStep {
    DismissWarning,
    DismissError,
    ClearModeHover,
    ClearSearch,
    BlurSearch,
    BackPanel,
    AbortClipFlow,
    CloseLibraryMenu,
    ClearLibrarySelection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Anchor {
    compact: (f64, f64),
    placed: (f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    stage: Stage,
    memory: MainWindowMemory,
    anchor: Option<Anchor>,
}

impl Shell {
    pub fn launch(memory: MainWindowMemory) -> Self {
        Self {
            stage: Stage::Compact,
            memory,
            anchor: None,
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn memory(&self) -> &MainWindowMemory {
        &self.memory
    }

    pub fn shows_library(&self) -> bool {
        matches!(
            self.stage,
            Stage::LibraryArriving | Stage::Expanded | Stage::LibraryLeaving
        )
    }

    pub fn is_fading(&self) -> bool {
        matches!(self.stage, Stage::LibraryArriving | Stage::LibraryLeaving)
    }

    pub fn is_compact(&self) -> bool {
        self.stage == Stage::Compact
    }

    pub fn shows_arrow(&self) -> bool {
        !self.memory.library_opened
    }

    pub fn library_fade(&self, elapsed: Duration, reduce_motion: bool) -> (f32, f32) {
        let progress = |total: Duration| (elapsed.as_secs_f32() / total.as_secs_f32()).min(1.);
        match self.stage {
            Stage::LibraryArriving => {
                let t = progress(motion::EXPAND);
                let slide = if reduce_motion {
                    0.
                } else {
                    LIBRARY_SLIDE * (motion::ease_move(t) - 1.)
                };
                (motion::EASE_FLUENT_IN.at(t), slide)
            }
            Stage::LibraryLeaving => (
                1. - motion::EASE_FLUENT_OUT.at(progress(motion::COLLAPSE)),
                0.,
            ),
            _ => (1., 0.),
        }
    }

    pub fn handle(&mut self, event: Event, screen: Screen) -> Vec<Effect> {
        match event {
            Event::Toggle => match self.stage {
                Stage::Compact | Stage::LibraryLeaving => self.handle(Event::Expand, screen),
                Stage::Expanded | Stage::LibraryArriving => self.handle(Event::Collapse, screen),
                Stage::OpeningFrame | Stage::ClosingFrame => Vec::new(),
            },
            Event::Expand => self.expand(screen),
            Event::Collapse => self.collapse(),
            Event::FrameSnapped => self.frame_snapped(),
            Event::FadeFinished => self.fade_finished(screen),
            Event::HideToTray => self.snap_to_compact(screen, true),
            Event::ReturnToCompact => self.snap_to_compact(screen, false),
            Event::Track => self.track(screen),
        }
    }

    pub fn escape(&self, layers: EscLayers) -> Option<EscStep> {
        if layers.warning {
            return Some(EscStep::DismissWarning);
        }
        if layers.error {
            return Some(EscStep::DismissError);
        }
        if self.shows_library() {
            return if layers.library_menu {
                Some(EscStep::CloseLibraryMenu)
            } else if layers.search_text {
                Some(EscStep::ClearSearch)
            } else if layers.search_focused {
                Some(EscStep::BlurSearch)
            } else if layers.library_selection {
                Some(EscStep::ClearLibrarySelection)
            } else {
                None
            };
        }
        if layers.mode_hover {
            Some(EscStep::ClearModeHover)
        } else if layers.panel && layers.search_text {
            Some(EscStep::ClearSearch)
        } else if layers.panel {
            Some(EscStep::BackPanel)
        } else if layers.clip_flow {
            Some(EscStep::AbortClipFlow)
        } else {
            None
        }
    }

    fn expand(&mut self, screen: Screen) -> Vec<Effect> {
        match self.stage {
            Stage::Compact => {
                self.remember_compact(screen);
                let placement = self.expanded_placement(screen);
                self.anchor = Some(Anchor {
                    compact: self.memory.compact_position.unwrap_or_default(),
                    placed: (placement.frame.x, placement.frame.y),
                });
                self.memory.library_opened = true;
                self.stage = Stage::OpeningFrame;
                vec![
                    Effect::Snap {
                        placement,
                        hide: false,
                    },
                    Effect::Persist,
                ]
            }
            Stage::LibraryLeaving => {
                self.stage = Stage::LibraryArriving;
                vec![Effect::Fade(motion::EXPAND)]
            }
            _ => Vec::new(),
        }
    }

    fn collapse(&mut self) -> Vec<Effect> {
        match self.stage {
            Stage::Expanded | Stage::LibraryArriving => {
                self.stage = Stage::LibraryLeaving;
                vec![Effect::Fade(motion::COLLAPSE)]
            }
            _ => Vec::new(),
        }
    }

    fn frame_snapped(&mut self) -> Vec<Effect> {
        match self.stage {
            Stage::OpeningFrame => {
                self.stage = Stage::LibraryArriving;
                vec![Effect::Fade(motion::EXPAND)]
            }
            Stage::ClosingFrame => {
                self.stage = Stage::Compact;
                self.anchor = None;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn fade_finished(&mut self, screen: Screen) -> Vec<Effect> {
        match self.stage {
            Stage::LibraryArriving => {
                self.stage = Stage::Expanded;
                Vec::new()
            }
            Stage::LibraryLeaving => self.close_frame(screen, false),
            _ => Vec::new(),
        }
    }

    fn snap_to_compact(&mut self, screen: Screen, hide: bool) -> Vec<Effect> {
        if self.shows_library() {
            self.close_frame(screen, hide)
        } else {
            Vec::new()
        }
    }

    fn close_frame(&mut self, screen: Screen, hide: bool) -> Vec<Effect> {
        self.remember_expanded(screen);
        let position = self.collapsed_position(screen);
        let placement = compact_placement(position, screen);
        self.memory.compact_position = Some((placement.frame.x, placement.frame.y));
        self.stage = Stage::ClosingFrame;
        vec![Effect::Snap { placement, hide }, Effect::Persist]
    }

    fn track(&mut self, screen: Screen) -> Vec<Effect> {
        let before = self.memory.clone();
        match self.stage {
            Stage::Compact => self.remember_compact(screen),
            Stage::LibraryArriving | Stage::Expanded | Stage::LibraryLeaving => {
                self.remember_expanded(screen)
            }
            Stage::OpeningFrame | Stage::ClosingFrame => {}
        }
        if self.memory == before {
            Vec::new()
        } else {
            vec![Effect::Persist]
        }
    }

    fn remember_compact(&mut self, screen: Screen) {
        self.memory.compact_position = Some((screen.frame.x, screen.frame.y));
    }

    fn remember_expanded(&mut self, screen: Screen) {
        self.memory.expanded_maximized = screen.maximized;
        if !screen.maximized {
            self.memory.expanded_size = Some((
                screen.frame.width / screen.scale,
                screen.frame.height / screen.scale,
            ));
        }
    }

    fn collapsed_position(&self, screen: Screen) -> (f64, f64) {
        let top_left = (screen.frame.x, screen.frame.y);
        match self.anchor {
            Some(anchor) if screen.maximized || same_spot(top_left, anchor.placed) => {
                anchor.compact
            }
            Some(_) => top_left,
            None => self.memory.compact_position.unwrap_or(top_left),
        }
    }

    fn expanded_placement(&self, screen: Screen) -> Placement {
        let available_width = usable(screen.work_area.width, screen.scale);
        let available_height = usable(screen.work_area.height, screen.scale);
        let (width, height) = self.memory.expanded_size.unwrap_or((
            FIRST_EXPANDED_WIDTH.min(screen.work_area.width / screen.scale * FIRST_EXPANDED_SHARE),
            FIRST_EXPANDED_HEIGHT
                .min(screen.work_area.height / screen.scale * FIRST_EXPANDED_SHARE),
        ));
        let min_width = MIN_EXPANDED_WIDTH.min(available_width);
        let min_height = MIN_EXPANDED_HEIGHT.min(available_height);
        let frame = pushed_inside(
            Frame {
                x: screen.frame.x,
                y: screen.frame.y,
                width: width.clamp(min_width, available_width) * screen.scale,
                height: height.clamp(min_height, available_height) * screen.scale,
            },
            screen,
        );
        Placement {
            frame,
            maximized: self.memory.expanded_maximized,
            on_top: false,
            resizable: true,
            min_width,
            min_height,
        }
    }
}

fn usable(extent: f64, scale: f64) -> f64 {
    (extent / scale - WORK_AREA_MARGIN * 2.).max(1.)
}

fn same_spot(first: (f64, f64), second: (f64, f64)) -> bool {
    (first.0 - second.0).abs() <= SAME_SPOT_TOLERANCE
        && (first.1 - second.1).abs() <= SAME_SPOT_TOLERANCE
}

fn compact_placement(position: (f64, f64), screen: Screen) -> Placement {
    let frame = pushed_inside(
        Frame {
            x: position.0,
            y: position.1,
            width: COMPACT_WIDTH * screen.scale,
            height: COMPACT_HEIGHT * screen.scale,
        },
        screen,
    );
    Placement {
        frame,
        maximized: false,
        on_top: true,
        resizable: false,
        min_width: COMPACT_WIDTH,
        min_height: COMPACT_HEIGHT,
    }
}

fn pushed_inside(frame: Frame, screen: Screen) -> Frame {
    let margin = WORK_AREA_MARGIN * screen.scale;
    let area = screen.work_area;
    let left = area.x + margin;
    let top = area.y + margin;
    let right = (area.x + area.width - margin - frame.width).max(left);
    let bottom = (area.y + area.height - margin - frame.height).max(top);
    Frame {
        x: frame.x.clamp(left, right),
        y: frame.y.clamp(top, bottom),
        ..frame
    }
}

pub fn launch_placement(memory: &MainWindowMemory, screen: Screen) -> Placement {
    let centred = (
        screen.work_area.x + (screen.work_area.width - COMPACT_WIDTH * screen.scale) / 2.,
        screen.work_area.y + (screen.work_area.height - COMPACT_HEIGHT * screen.scale) / 2.,
    );
    let inside_this_display = |&(x, y): &(f64, f64)| {
        let area = screen.work_area;
        x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
    };
    let position = memory
        .compact_position
        .filter(inside_this_display)
        .unwrap_or(centred);
    compact_placement(position, screen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: f64, height: f64) -> Frame {
        Frame {
            x: 0.,
            y: 0.,
            width,
            height,
        }
    }

    fn compact_at(x: f64, y: f64, scale: f64, work_area: Frame) -> Screen {
        Screen {
            frame: Frame {
                x,
                y,
                width: COMPACT_WIDTH * scale,
                height: COMPACT_HEIGHT * scale,
            },
            work_area,
            scale,
            maximized: false,
        }
    }

    fn snapped(effects: &[Effect]) -> Placement {
        effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Snap { placement, .. } => Some(*placement),
                _ => None,
            })
            .expect("a frame snap")
    }

    fn at(placement: Placement) -> Screen {
        Screen {
            frame: placement.frame,
            work_area: area(2560., 1400.),
            scale: 1.,
            maximized: false,
        }
    }

    fn expanded(shell: &mut Shell, screen: Screen) -> Placement {
        let placement = snapped(&shell.handle(Event::Expand, screen));
        shell.handle(Event::FrameSnapped, at(placement));
        shell.handle(Event::FadeFinished, at(placement));
        placement
    }

    fn shell() -> Shell {
        Shell::launch(MainWindowMemory::default())
    }

    #[test]
    fn launch_is_compact_even_when_the_last_session_was_maximised() {
        let shell = Shell::launch(MainWindowMemory {
            expanded_maximized: true,
            expanded_size: Some((1500., 900.)),
            ..Default::default()
        });
        assert_eq!(shell.stage(), Stage::Compact);
        assert!(!shell.shows_library());
    }

    #[test]
    fn expanding_hides_content_then_snaps_once_then_the_library_arrives() {
        let mut shell = shell();
        let screen = compact_at(100., 80., 1., area(2560., 1400.));

        let effects = shell.handle(Event::Expand, screen);
        assert_eq!(shell.stage(), Stage::OpeningFrame);
        assert!(!shell.shows_library());
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::Snap { .. }))
                .count(),
            1
        );

        let placement = snapped(&effects);
        assert_eq!(
            shell.handle(Event::FrameSnapped, at(placement)),
            vec![Effect::Fade(motion::EXPAND)]
        );
        assert_eq!(shell.stage(), Stage::LibraryArriving);
        assert!(shell.shows_library());

        shell.handle(Event::FadeFinished, at(placement));
        assert_eq!(shell.stage(), Stage::Expanded);
    }

    #[test]
    fn collapsing_fades_first_and_only_then_snaps() {
        let mut shell = shell();
        let placement = expanded(&mut shell, compact_at(100., 80., 1., area(2560., 1400.)));

        assert_eq!(
            shell.handle(Event::Collapse, at(placement)),
            vec![Effect::Fade(motion::COLLAPSE)]
        );
        assert_eq!(shell.stage(), Stage::LibraryLeaving);

        let compact = snapped(&shell.handle(Event::FadeFinished, at(placement)));
        assert_eq!(shell.stage(), Stage::ClosingFrame);
        assert!(compact.on_top);
        assert!(!compact.resizable);

        shell.handle(Event::FrameSnapped, at(compact));
        assert_eq!(shell.stage(), Stage::Compact);
    }

    #[test]
    fn toggle_turns_around_during_a_fade_and_waits_during_a_snap() {
        let mut shell = shell();
        let screen = compact_at(100., 80., 1., area(2560., 1400.));
        let placement = snapped(&shell.handle(Event::Toggle, screen));
        assert!(shell.handle(Event::Toggle, screen).is_empty());
        assert_eq!(shell.stage(), Stage::OpeningFrame);

        shell.handle(Event::FrameSnapped, at(placement));
        shell.handle(Event::Toggle, at(placement));
        assert_eq!(shell.stage(), Stage::LibraryLeaving);
        assert_eq!(
            shell.handle(Event::Toggle, at(placement)),
            vec![Effect::Fade(motion::EXPAND)]
        );
        assert_eq!(shell.stage(), Stage::LibraryArriving);
    }

    #[test]
    fn the_first_expansion_keeps_the_top_left_and_is_an_ordinary_window() {
        let mut shell = shell();
        let placement =
            snapped(&shell.handle(Event::Expand, compact_at(100., 80., 1., area(2560., 1400.))));
        assert_eq!(
            placement.frame,
            Frame {
                x: 100.,
                y: 80.,
                width: 1200.,
                height: 760.
            }
        );
        assert!(!placement.on_top);
        assert!(placement.resizable);
        assert_eq!((placement.min_width, placement.min_height), (900., 600.));
    }

    #[test]
    fn near_an_edge_the_whole_window_is_pushed_back_with_a_twelve_pixel_margin() {
        let mut shell = shell();
        let placement = snapped(&shell.handle(
            Event::Expand,
            compact_at(1500., 900., 1., area(1920., 1040.)),
        ));
        assert_eq!(placement.frame.x, 1920. - 12. - 1200.);
        assert_eq!(placement.frame.y, 1040. - 12. - 760.);
    }

    #[test]
    fn the_margin_and_sizes_scale_with_windows_from_100_to_200_percent() {
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            let mut shell = shell();
            let work = area(3840., 2100.);
            let far = compact_at(3700., 2000., scale, work);
            let placement = snapped(&shell.handle(Event::Expand, far));
            assert_eq!(placement.frame.width, 1200. * scale);
            assert_eq!(placement.frame.height, 760. * scale);
            assert_eq!(
                placement.frame.x + placement.frame.width,
                3840. - 12. * scale
            );
            assert_eq!(
                placement.frame.y + placement.frame.height,
                2100. - 12. * scale
            );
        }
    }

    #[test]
    fn clamping_works_on_a_display_left_of_or_above_the_primary() {
        let work = Frame {
            x: -1920.,
            y: -200.,
            width: 1920.,
            height: 1040.,
        };
        let mut shell = shell();
        let placement = snapped(&shell.handle(Event::Expand, compact_at(-500., 700., 1., work)));
        assert_eq!(placement.frame.x, -12. - 1200.);
        assert_eq!(placement.frame.y, -200. + 1040. - 12. - 760.);
    }

    #[test]
    fn the_first_expansion_takes_at_most_ninety_percent_of_the_work_area() {
        let mut shell = shell();
        let placement =
            snapped(&shell.handle(Event::Expand, compact_at(100., 80., 1., area(1280., 760.))));
        assert_eq!(placement.frame.width, 1152.);
        assert_eq!(placement.frame.height, 684.);
    }

    #[test]
    fn a_work_area_smaller_than_the_minimum_still_fits_and_never_inverts() {
        let mut shell = shell();
        let work = area(800., 500.);
        let placement = snapped(&shell.handle(Event::Expand, compact_at(300., 90., 1., work)));
        assert_eq!(placement.frame.x, 12.);
        assert_eq!(placement.frame.y, 12.);
        assert_eq!(placement.frame.width, 776.);
        assert_eq!(placement.frame.height, 476.);
        assert_eq!((placement.min_width, placement.min_height), (776., 476.));
    }

    #[test]
    fn collapsing_after_being_pushed_back_returns_to_the_compact_position() {
        let mut shell = shell();
        let work = area(1920., 1040.);
        let screen = compact_at(1500., 600., 1., work);
        let placement = expanded(&mut shell, screen);

        let mut open = at(placement);
        open.work_area = work;
        shell.handle(Event::Collapse, open);
        let compact = snapped(&shell.handle(Event::FadeFinished, open));
        assert_eq!((compact.frame.x, compact.frame.y), (1500., 600.));
        assert_eq!(compact.frame.width, COMPACT_WIDTH);
    }

    #[test]
    fn collapsing_after_moving_the_library_keeps_its_top_left() {
        let mut shell = shell();
        let placement = expanded(&mut shell, compact_at(100., 80., 1., area(2560., 1400.)));
        let mut moved = at(placement);
        moved.frame.x = 400.;
        moved.frame.y = 300.;

        shell.handle(Event::Collapse, moved);
        let compact = snapped(&shell.handle(Event::FadeFinished, moved));
        assert_eq!((compact.frame.x, compact.frame.y), (400., 300.));
    }

    #[test]
    fn expanded_remembers_size_and_maximised_state_but_not_position() {
        let mut shell = shell();
        let placement = expanded(&mut shell, compact_at(100., 80., 1., area(2560., 1400.)));
        let mut resized = at(placement);
        resized.frame.width = 1400.;
        resized.frame.height = 900.;
        assert_eq!(shell.handle(Event::Track, resized), vec![Effect::Persist]);

        shell.handle(Event::Collapse, resized);
        let compact = snapped(&shell.handle(Event::FadeFinished, resized));
        shell.handle(Event::FrameSnapped, at(compact));

        let next = snapped(&shell.handle(
            Event::Expand,
            compact_at(500., 200., 1., area(2560., 1400.)),
        ));
        assert_eq!(
            next.frame,
            Frame {
                x: 500.,
                y: 200.,
                width: 1400.,
                height: 900.
            }
        );
    }

    #[test]
    fn a_maximised_library_comes_back_maximised_and_keeps_its_restored_size() {
        let mut shell = shell();
        let placement = expanded(&mut shell, compact_at(100., 80., 1., area(2560., 1400.)));
        let maximised = Screen {
            frame: area(2560., 1400.),
            work_area: area(2560., 1400.),
            scale: 1.,
            maximized: true,
        };

        shell.handle(Event::Collapse, maximised);
        let compact = snapped(&shell.handle(Event::FadeFinished, maximised));
        assert!(!compact.maximized);
        assert_eq!((compact.frame.x, compact.frame.y), (100., 80.));
        assert_eq!(compact.frame.width, COMPACT_WIDTH);
        shell.handle(Event::FrameSnapped, at(compact));

        let next =
            snapped(&shell.handle(Event::Expand, compact_at(100., 80., 1., area(2560., 1400.))));
        assert!(next.maximized);
        assert_eq!(next.frame, placement.frame);
    }

    #[test]
    fn expanded_sizes_are_remembered_in_logical_pixels_across_scales() {
        let mut shell = shell();
        let placement = expanded(&mut shell, compact_at(100., 80., 2., area(3840., 2100.)));
        let mut resized = Screen {
            frame: placement.frame,
            work_area: area(3840., 2100.),
            scale: 2.,
            maximized: false,
        };
        resized.frame.width = 2800.;
        shell.handle(Event::Track, resized);
        assert_eq!(shell.memory().expanded_size, Some((1400., 760.)));
    }

    #[test]
    fn compact_remembers_where_it_was_moved() {
        let mut shell = shell();
        let moved = compact_at(640., 320., 1., area(2560., 1400.));
        assert_eq!(shell.handle(Event::Track, moved), vec![Effect::Persist]);
        assert_eq!(shell.handle(Event::Track, moved), Vec::new());
        assert_eq!(shell.memory().compact_position, Some((640., 320.)));
    }

    #[test]
    fn launch_uses_the_remembered_position_and_centres_when_its_display_is_gone() {
        let screen = compact_at(0., 0., 1., area(1920., 1040.));
        let remembered = MainWindowMemory {
            compact_position: Some((640., 320.)),
            ..Default::default()
        };
        let placement = launch_placement(&remembered, screen);
        assert_eq!((placement.frame.x, placement.frame.y), (640., 320.));
        assert!(placement.on_top);

        let gone = MainWindowMemory {
            compact_position: Some((5000., 320.)),
            ..Default::default()
        };
        let placement = launch_placement(&gone, screen);
        assert_eq!(placement.frame.x, (1920. - COMPACT_WIDTH) / 2.);
        assert_eq!(placement.frame.y, (1040. - COMPACT_HEIGHT) / 2.);
    }

    #[test]
    fn hiding_to_the_tray_collapses_at_once_behind_the_hide() {
        let mut shell = shell();
        let placement = expanded(&mut shell, compact_at(100., 80., 1., area(2560., 1400.)));

        let effects = shell.handle(Event::HideToTray, at(placement));
        assert!(matches!(
            effects[0],
            Effect::Snap { hide: true, placement } if placement.on_top
        ));
        shell.handle(Event::FrameSnapped, at(placement));
        assert!(shell.is_compact());
    }

    #[test]
    fn returning_to_compact_snaps_without_a_fade_and_keeps_the_window_shown() {
        let mut shell = shell();
        let placement = expanded(&mut shell, compact_at(100., 80., 1., area(2560., 1400.)));

        let effects = shell.handle(Event::ReturnToCompact, at(placement));
        assert!(matches!(
            effects[0],
            Effect::Snap { hide: false, placement } if placement.on_top
        ));
        shell.handle(Event::FrameSnapped, at(placement));
        assert!(shell.is_compact());
        assert!(
            shell
                .handle(Event::ReturnToCompact, at(placement))
                .is_empty()
        );
    }

    #[test]
    fn hiding_compact_to_the_tray_moves_nothing() {
        let mut shell = shell();
        let screen = compact_at(100., 80., 1., area(2560., 1400.));
        assert!(shell.handle(Event::HideToTray, screen).is_empty());
        assert!(shell.is_compact());
    }

    #[test]
    fn the_arrow_shows_until_the_library_is_first_opened() {
        let mut shell = shell();
        assert!(shell.shows_arrow());
        let effects = shell.handle(Event::Expand, compact_at(100., 80., 1., area(2560., 1400.)));
        assert!(effects.contains(&Effect::Persist));
        assert!(!shell.shows_arrow());
        assert!(!Shell::launch(shell.memory().clone()).shows_arrow());
    }

    #[test]
    fn esc_in_the_library_unwinds_search_and_never_collapses() {
        let mut shell = shell();
        expanded(&mut shell, compact_at(100., 80., 1., area(2560., 1400.)));

        assert_eq!(
            shell.escape(EscLayers {
                error: true,
                search_text: true,
                search_focused: true,
                ..Default::default()
            }),
            Some(EscStep::DismissError)
        );
        assert_eq!(
            shell.escape(EscLayers {
                search_text: true,
                search_focused: true,
                ..Default::default()
            }),
            Some(EscStep::ClearSearch)
        );
        assert_eq!(
            shell.escape(EscLayers {
                search_focused: true,
                ..Default::default()
            }),
            Some(EscStep::BlurSearch)
        );
        assert_eq!(
            shell.escape(EscLayers {
                library_menu: true,
                search_text: true,
                ..Default::default()
            }),
            Some(EscStep::CloseLibraryMenu)
        );
        assert_eq!(
            shell.escape(EscLayers {
                search_focused: true,
                library_selection: true,
                ..Default::default()
            }),
            Some(EscStep::BlurSearch)
        );
        assert_eq!(
            shell.escape(EscLayers {
                library_selection: true,
                ..Default::default()
            }),
            Some(EscStep::ClearLibrarySelection)
        );
        assert_eq!(shell.escape(EscLayers::default()), None);
    }

    #[test]
    fn esc_in_compact_goes_warning_error_hover_search_panel_clip_and_then_nothing() {
        let shell = shell();
        let all = EscLayers {
            warning: true,
            error: true,
            mode_hover: true,
            search_text: true,
            search_focused: true,
            panel: true,
            clip_flow: true,
            library_menu: false,
            library_selection: false,
        };
        assert_eq!(shell.escape(all), Some(EscStep::DismissWarning));
        let all = EscLayers {
            warning: false,
            ..all
        };
        assert_eq!(shell.escape(all), Some(EscStep::DismissError));
        let all = EscLayers {
            error: false,
            ..all
        };
        assert_eq!(shell.escape(all), Some(EscStep::ClearModeHover));
        let all = EscLayers {
            mode_hover: false,
            ..all
        };
        assert_eq!(shell.escape(all), Some(EscStep::ClearSearch));
        let all = EscLayers {
            search_text: false,
            ..all
        };
        assert_eq!(shell.escape(all), Some(EscStep::BackPanel));
        let all = EscLayers {
            panel: false,
            ..all
        };
        assert_eq!(shell.escape(all), Some(EscStep::AbortClipFlow));
        let all = EscLayers {
            clip_flow: false,
            ..all
        };
        assert_eq!(shell.escape(all), None);
    }

    #[test]
    fn the_library_fades_and_slides_in_and_only_fades_with_reduced_motion() {
        let mut shell = shell();
        let placement =
            snapped(&shell.handle(Event::Expand, compact_at(100., 80., 1., area(2560., 1400.))));
        shell.handle(Event::FrameSnapped, at(placement));

        assert_eq!(shell.library_fade(Duration::ZERO, false), (0., -24.));
        assert_eq!(shell.library_fade(Duration::ZERO, true), (0., 0.));
        assert_eq!(shell.library_fade(motion::EXPAND, false), (1., 0.));

        shell.handle(Event::FadeFinished, at(placement));
        assert_eq!(shell.library_fade(Duration::ZERO, false), (1., 0.));
        shell.handle(Event::Collapse, at(placement));
        assert_eq!(shell.library_fade(Duration::ZERO, false), (1., 0.));
        assert_eq!(shell.library_fade(motion::COLLAPSE, true), (0., 0.));
    }
}
