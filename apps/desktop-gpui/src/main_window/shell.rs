use crate::store::{MainWindowMemory, MainWindowStartBehaviour};

pub const COMPACT_WIDTH: f64 = crate::MAIN_WINDOW_WIDTH as f64;
pub const COMPACT_HEIGHT: f64 = crate::MAIN_WINDOW_HEIGHT as f64;
pub const FIRST_LIBRARY_WIDTH: f64 = 1440.;
pub const FIRST_LIBRARY_HEIGHT: f64 = 860.;
pub const MIN_LIBRARY_WIDTH: f64 = 900.;
pub const MIN_LIBRARY_HEIGHT: f64 = 600.;
const WORK_AREA_MARGIN: f64 = 12.;
const FIRST_LIBRARY_SHARE: f64 = 0.9;

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
pub enum ShellWindow {
    Recorder,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    OpenLibrary,
    CloseLibrary,
    ShowRecorder,
    CloseRecorder,
    Switch { from: ShellWindow },
    StartRecording(MainWindowStartBehaviour),
    OpenSettings,
    CloseSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    OpenLibrary,
    FrontLibrary,
    CloseLibrary,
    ShowRecorder,
    HideRecorder,
    MinimiseRecorder,
    LowerRecorder,
    RaiseRecorder,
}

pub fn route(request: Request, library_open: bool) -> Action {
    match request {
        Request::OpenLibrary
        | Request::Switch {
            from: ShellWindow::Recorder,
        } => {
            if library_open {
                Action::FrontLibrary
            } else {
                Action::OpenLibrary
            }
        }
        Request::CloseLibrary => Action::CloseLibrary,
        Request::ShowRecorder
        | Request::Switch {
            from: ShellWindow::Library,
        } => Action::ShowRecorder,
        Request::CloseRecorder => Action::HideRecorder,
        Request::StartRecording(MainWindowStartBehaviour::Minimise) => Action::MinimiseRecorder,
        Request::StartRecording(MainWindowStartBehaviour::Close) => Action::HideRecorder,
        Request::OpenSettings => Action::LowerRecorder,
        Request::CloseSettings => Action::RaiseRecorder,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecorderEscLayers {
    pub warning: bool,
    pub error: bool,
    pub mode_hover: bool,
    pub search_text: bool,
    pub panel: bool,
    pub clip_flow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderEscStep {
    DismissWarning,
    DismissError,
    ClearModeHover,
    ClearSearch,
    BackPanel,
    AbortClipFlow,
}

pub fn recorder_escape(layers: RecorderEscLayers) -> Option<RecorderEscStep> {
    if layers.warning {
        Some(RecorderEscStep::DismissWarning)
    } else if layers.error {
        Some(RecorderEscStep::DismissError)
    } else if layers.mode_hover {
        Some(RecorderEscStep::ClearModeHover)
    } else if layers.panel && layers.search_text {
        Some(RecorderEscStep::ClearSearch)
    } else if layers.panel {
        Some(RecorderEscStep::BackPanel)
    } else if layers.clip_flow {
        Some(RecorderEscStep::AbortClipFlow)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LibraryEscLayers {
    pub dialog: bool,
    pub whole_screen: bool,
    pub preview: bool,
    pub menu: bool,
    pub search_text: bool,
    pub search_focused: bool,
    pub selection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryEscStep {
    CloseDialog,
    LeaveWholeScreen,
    ClosePreview,
    CloseMenu,
    ClearSearch,
    BlurSearch,
    ClearSelection,
}

pub fn library_escape(layers: LibraryEscLayers) -> Option<LibraryEscStep> {
    if layers.dialog {
        Some(LibraryEscStep::CloseDialog)
    } else if layers.whole_screen {
        Some(LibraryEscStep::LeaveWholeScreen)
    } else if layers.preview {
        Some(LibraryEscStep::ClosePreview)
    } else if layers.menu {
        Some(LibraryEscStep::CloseMenu)
    } else if layers.search_text {
        Some(LibraryEscStep::ClearSearch)
    } else if layers.search_focused {
        Some(LibraryEscStep::BlurSearch)
    } else if layers.selection {
        Some(LibraryEscStep::ClearSelection)
    } else {
        None
    }
}

fn usable(extent: f64, scale: f64) -> f64 {
    (extent / scale - WORK_AREA_MARGIN * 2.).max(1.)
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

fn inside(area: Frame, (x, y): (f64, f64)) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

fn centred(width: f64, height: f64, area: Frame) -> (f64, f64) {
    (
        area.x + (area.width - width) / 2.,
        area.y + (area.height - height) / 2.,
    )
}

pub fn launch_placement(memory: &MainWindowMemory, screen: Screen) -> Placement {
    let width = COMPACT_WIDTH * screen.scale;
    let height = COMPACT_HEIGHT * screen.scale;
    let (x, y) = memory
        .compact_position
        .filter(|position| inside(screen.work_area, *position))
        .unwrap_or_else(|| centred(width, height, screen.work_area));
    Placement {
        frame: pushed_inside(
            Frame {
                x,
                y,
                width,
                height,
            },
            screen,
        ),
        maximized: false,
        on_top: true,
        resizable: false,
        min_width: COMPACT_WIDTH,
        min_height: COMPACT_HEIGHT,
    }
}

pub fn library_placement(memory: &MainWindowMemory, screen: Screen) -> Placement {
    let available_width = usable(screen.work_area.width, screen.scale);
    let available_height = usable(screen.work_area.height, screen.scale);
    let (width, height) = memory.library_size.unwrap_or((
        FIRST_LIBRARY_WIDTH.min(screen.work_area.width / screen.scale * FIRST_LIBRARY_SHARE),
        FIRST_LIBRARY_HEIGHT.min(screen.work_area.height / screen.scale * FIRST_LIBRARY_SHARE),
    ));
    let min_width = MIN_LIBRARY_WIDTH.min(available_width);
    let min_height = MIN_LIBRARY_HEIGHT.min(available_height);
    let width = width.clamp(min_width, available_width) * screen.scale;
    let height = height.clamp(min_height, available_height) * screen.scale;
    let (x, y) = memory
        .library_position
        .filter(|position| inside(screen.work_area, *position))
        .unwrap_or_else(|| centred(width, height, screen.work_area));
    Placement {
        frame: pushed_inside(
            Frame {
                x,
                y,
                width,
                height,
            },
            screen,
        ),
        maximized: memory.library_maximized,
        on_top: false,
        resizable: true,
        min_width,
        min_height,
    }
}

pub fn library_anchor(memory: &MainWindowMemory) -> Option<(f64, f64)> {
    memory.library_position.or(memory.compact_position)
}

pub fn remember_recorder(memory: &mut MainWindowMemory, screen: Screen) -> bool {
    let position = Some((screen.frame.x, screen.frame.y));
    let changed = memory.compact_position != position;
    memory.compact_position = position;
    changed
}

pub fn remember_library(memory: &mut MainWindowMemory, screen: Screen) -> bool {
    let before = memory.clone();
    memory.library_maximized = screen.maximized;
    if !screen.maximized {
        memory.library_position = Some((screen.frame.x, screen.frame.y));
        memory.library_size = Some((
            screen.frame.width / screen.scale,
            screen.frame.height / screen.scale,
        ));
    }
    *memory != before
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

    fn display(work_area: Frame, scale: f64) -> Screen {
        Screen {
            frame: work_area,
            work_area,
            scale,
            maximized: false,
        }
    }

    fn at(frame: Frame, work_area: Frame, scale: f64) -> Screen {
        Screen {
            frame,
            work_area,
            scale,
            maximized: false,
        }
    }

    fn acted_on(action: Action) -> ShellWindow {
        match action {
            Action::OpenLibrary | Action::FrontLibrary | Action::CloseLibrary => {
                ShellWindow::Library
            }
            Action::ShowRecorder
            | Action::HideRecorder
            | Action::MinimiseRecorder
            | Action::LowerRecorder
            | Action::RaiseRecorder => ShellWindow::Recorder,
        }
    }

    const REQUESTS: [Request; 10] = [
        Request::OpenLibrary,
        Request::CloseLibrary,
        Request::ShowRecorder,
        Request::CloseRecorder,
        Request::Switch {
            from: ShellWindow::Recorder,
        },
        Request::Switch {
            from: ShellWindow::Library,
        },
        Request::StartRecording(MainWindowStartBehaviour::Close),
        Request::StartRecording(MainWindowStartBehaviour::Minimise),
        Request::OpenSettings,
        Request::CloseSettings,
    ];

    #[test]
    fn opening_the_library_opens_it_once_and_then_only_brings_it_forward() {
        assert_eq!(route(Request::OpenLibrary, false), Action::OpenLibrary);
        assert_eq!(route(Request::OpenLibrary, true), Action::FrontLibrary);
    }

    #[test]
    fn every_request_acts_on_one_window_and_never_on_the_other() {
        for library_open in [false, true] {
            for request in REQUESTS {
                let expected = match request {
                    Request::OpenLibrary
                    | Request::CloseLibrary
                    | Request::Switch {
                        from: ShellWindow::Recorder,
                    } => ShellWindow::Library,
                    Request::ShowRecorder
                    | Request::CloseRecorder
                    | Request::StartRecording(_)
                    | Request::OpenSettings
                    | Request::CloseSettings
                    | Request::Switch {
                        from: ShellWindow::Library,
                    } => ShellWindow::Recorder,
                };
                assert_eq!(acted_on(route(request, library_open)), expected);
            }
        }
    }

    #[test]
    fn settings_lowers_the_recorder_while_open_and_never_hides_a_window() {
        for library_open in [false, true] {
            assert_eq!(
                route(Request::OpenSettings, library_open),
                Action::LowerRecorder
            );
            assert_eq!(
                route(Request::CloseSettings, library_open),
                Action::RaiseRecorder
            );
        }
    }

    #[test]
    fn closing_the_library_closes_only_the_library() {
        assert_eq!(route(Request::CloseLibrary, true), Action::CloseLibrary);
    }

    #[test]
    fn closing_the_recorder_hides_it_whether_or_not_the_library_is_open() {
        for library_open in [false, true] {
            assert_eq!(
                route(Request::CloseRecorder, library_open),
                Action::HideRecorder
            );
        }
    }

    #[test]
    fn starting_a_recording_puts_only_the_recorder_away_as_the_setting_says() {
        for library_open in [false, true] {
            assert_eq!(
                route(
                    Request::StartRecording(MainWindowStartBehaviour::Close),
                    library_open
                ),
                Action::HideRecorder
            );
            assert_eq!(
                route(
                    Request::StartRecording(MainWindowStartBehaviour::Minimise),
                    library_open
                ),
                Action::MinimiseRecorder
            );
        }
    }

    #[test]
    fn ctrl_l_goes_to_the_other_window() {
        let from_recorder = Request::Switch {
            from: ShellWindow::Recorder,
        };
        assert_eq!(route(from_recorder, false), Action::OpenLibrary);
        assert_eq!(route(from_recorder, true), Action::FrontLibrary);
        let from_library = Request::Switch {
            from: ShellWindow::Library,
        };
        assert_eq!(route(from_library, true), Action::ShowRecorder);
    }

    #[test]
    fn the_first_library_is_1440_by_860_centred_and_an_ordinary_window() {
        let placement = library_placement(
            &MainWindowMemory::default(),
            display(area(2560., 1400.), 1.),
        );
        assert_eq!(
            placement.frame,
            Frame {
                x: 560.,
                y: 270.,
                width: 1440.,
                height: 860.
            }
        );
        assert!(!placement.on_top);
        assert!(placement.resizable);
        assert!(!placement.maximized);
        assert_eq!((placement.min_width, placement.min_height), (900., 600.));
    }

    #[test]
    fn the_first_library_takes_at_most_ninety_percent_of_the_work_area() {
        let placement =
            library_placement(&MainWindowMemory::default(), display(area(1280., 760.), 1.));
        assert_eq!(placement.frame.width, 1152.);
        assert_eq!(placement.frame.height, 684.);
    }

    #[test]
    fn library_sizes_scale_with_windows_from_100_to_200_percent() {
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            let placement = library_placement(
                &MainWindowMemory::default(),
                display(area(3840., 2100.), scale),
            );
            assert_eq!(placement.frame.width, 1440. * scale);
            assert_eq!(placement.frame.height, 860. * scale);
        }
    }

    #[test]
    fn a_work_area_smaller_than_the_minimum_still_fits_and_never_inverts() {
        let placement =
            library_placement(&MainWindowMemory::default(), display(area(800., 500.), 1.));
        assert_eq!(placement.frame.x, 12.);
        assert_eq!(placement.frame.y, 12.);
        assert_eq!(placement.frame.width, 776.);
        assert_eq!(placement.frame.height, 476.);
        assert_eq!((placement.min_width, placement.min_height), (776., 476.));
    }

    #[test]
    fn the_library_comes_back_where_and_how_big_it_was() {
        let work = area(2560., 1400.);
        let mut memory = MainWindowMemory::default();
        let moved = Frame {
            x: 300.,
            y: 200.,
            width: 1400.,
            height: 900.,
        };
        assert!(remember_library(&mut memory, at(moved, work, 1.)));
        assert!(!remember_library(&mut memory, at(moved, work, 1.)));
        assert_eq!(library_placement(&memory, display(work, 1.)).frame, moved);
    }

    #[test]
    fn a_maximised_library_comes_back_maximised_and_keeps_its_restored_frame() {
        let work = area(2560., 1400.);
        let mut memory = MainWindowMemory::default();
        let restored = Frame {
            x: 300.,
            y: 200.,
            width: 1400.,
            height: 900.,
        };
        remember_library(&mut memory, at(restored, work, 1.));
        let maximised = Screen {
            frame: work,
            work_area: work,
            scale: 1.,
            maximized: true,
        };
        assert!(remember_library(&mut memory, maximised));
        let placement = library_placement(&memory, display(work, 1.));
        assert!(placement.maximized);
        assert_eq!(placement.frame, restored);
    }

    #[test]
    fn library_sizes_are_remembered_in_logical_pixels_across_scales() {
        let mut memory = MainWindowMemory::default();
        let frame = Frame {
            x: 100.,
            y: 80.,
            width: 2800.,
            height: 1520.,
        };
        remember_library(&mut memory, at(frame, area(3840., 2100.), 2.));
        assert_eq!(memory.library_size, Some((1400., 760.)));
    }

    #[test]
    fn a_library_remembered_on_a_display_that_is_gone_is_centred() {
        let memory = MainWindowMemory {
            library_position: Some((5000., 320.)),
            ..Default::default()
        };
        let placement = library_placement(&memory, display(area(1920., 1040.), 1.));
        assert_eq!(placement.frame.x, (1920. - 1440.) / 2.);
        assert_eq!(placement.frame.y, (1040. - 860.) / 2.);
    }

    #[test]
    fn a_remembered_library_is_pushed_back_inside_with_a_twelve_pixel_margin() {
        let memory = MainWindowMemory {
            library_position: Some((1500., 900.)),
            ..Default::default()
        };
        let placement = library_placement(&memory, display(area(1920., 1040.), 1.));
        assert_eq!(placement.frame.x, 1920. - 12. - 1440.);
        assert_eq!(placement.frame.y, 1040. - 12. - 860.);
    }

    #[test]
    fn placement_works_on_a_display_left_of_or_above_the_primary() {
        let work = Frame {
            x: -1920.,
            y: -200.,
            width: 1920.,
            height: 1040.,
        };
        let memory = MainWindowMemory {
            library_position: Some((-500., 700.)),
            ..Default::default()
        };
        let placement = library_placement(&memory, display(work, 1.));
        assert_eq!(placement.frame.x, -12. - 1440.);
        assert_eq!(placement.frame.y, -200. + 1040. - 12. - 860.);
    }

    #[test]
    fn the_first_library_opens_on_the_display_the_recorder_is_on() {
        let memory = MainWindowMemory {
            compact_position: Some((-900., 300.)),
            ..Default::default()
        };
        assert_eq!(library_anchor(&memory), Some((-900., 300.)));
        let remembered = MainWindowMemory {
            library_position: Some((40., 60.)),
            ..memory
        };
        assert_eq!(library_anchor(&remembered), Some((40., 60.)));
    }

    #[test]
    fn the_recorder_remembers_where_it_was_moved_and_nothing_about_the_library() {
        let mut memory = MainWindowMemory {
            library_position: Some((40., 60.)),
            library_size: Some((1400., 900.)),
            ..Default::default()
        };
        let moved = at(
            Frame {
                x: 640.,
                y: 320.,
                width: COMPACT_WIDTH,
                height: COMPACT_HEIGHT,
            },
            area(2560., 1400.),
            1.,
        );
        assert!(remember_recorder(&mut memory, moved));
        assert!(!remember_recorder(&mut memory, moved));
        assert_eq!(memory.compact_position, Some((640., 320.)));
        assert_eq!(memory.library_position, Some((40., 60.)));
        assert_eq!(memory.library_size, Some((1400., 900.)));
    }

    #[test]
    fn the_library_remembers_nothing_about_the_recorder() {
        let mut memory = MainWindowMemory {
            compact_position: Some((640., 320.)),
            ..Default::default()
        };
        let frame = Frame {
            x: 300.,
            y: 200.,
            width: 1400.,
            height: 900.,
        };
        remember_library(&mut memory, at(frame, area(2560., 1400.), 1.));
        assert_eq!(memory.compact_position, Some((640., 320.)));
    }

    #[test]
    fn launch_uses_the_remembered_position_and_centres_when_its_display_is_gone() {
        let screen = display(area(1920., 1040.), 1.);
        let remembered = MainWindowMemory {
            compact_position: Some((640., 320.)),
            ..Default::default()
        };
        let placement = launch_placement(&remembered, screen);
        assert_eq!((placement.frame.x, placement.frame.y), (640., 320.));
        assert!(placement.on_top);
        assert!(!placement.resizable);

        let gone = MainWindowMemory {
            compact_position: Some((5000., 320.)),
            ..Default::default()
        };
        let placement = launch_placement(&gone, screen);
        assert_eq!(placement.frame.x, (1920. - COMPACT_WIDTH) / 2.);
        assert_eq!(placement.frame.y, (1040. - COMPACT_HEIGHT) / 2.);
    }

    #[test]
    fn launch_is_the_recorder_even_when_the_library_was_left_maximised() {
        let memory = MainWindowMemory {
            library_maximized: true,
            library_size: Some((1500., 900.)),
            ..Default::default()
        };
        let placement = launch_placement(&memory, display(area(1920., 1040.), 1.));
        assert!(!placement.maximized);
        assert_eq!(placement.frame.width, COMPACT_WIDTH);
        assert_eq!(placement.frame.height, COMPACT_HEIGHT);
    }

    #[test]
    fn esc_in_the_library_unwinds_one_layer_at_a_time_and_then_does_nothing() {
        let mut layers = LibraryEscLayers {
            dialog: true,
            whole_screen: true,
            preview: true,
            menu: true,
            search_text: true,
            search_focused: true,
            selection: true,
        };
        assert_eq!(library_escape(layers), Some(LibraryEscStep::CloseDialog));
        layers.dialog = false;
        assert_eq!(
            library_escape(layers),
            Some(LibraryEscStep::LeaveWholeScreen)
        );
        layers.whole_screen = false;
        assert_eq!(library_escape(layers), Some(LibraryEscStep::ClosePreview));
        layers.preview = false;
        assert_eq!(library_escape(layers), Some(LibraryEscStep::CloseMenu));
        layers.menu = false;
        assert_eq!(library_escape(layers), Some(LibraryEscStep::ClearSearch));
        layers.search_text = false;
        assert_eq!(library_escape(layers), Some(LibraryEscStep::BlurSearch));
        layers.search_focused = false;
        assert_eq!(library_escape(layers), Some(LibraryEscStep::ClearSelection));
        layers.selection = false;
        assert_eq!(library_escape(layers), None);
    }

    #[test]
    fn esc_in_the_recorder_goes_warning_error_hover_search_panel_clip_and_then_nothing() {
        let mut layers = RecorderEscLayers {
            warning: true,
            error: true,
            mode_hover: true,
            search_text: true,
            panel: true,
            clip_flow: true,
        };
        assert_eq!(
            recorder_escape(layers),
            Some(RecorderEscStep::DismissWarning)
        );
        layers.warning = false;
        assert_eq!(recorder_escape(layers), Some(RecorderEscStep::DismissError));
        layers.error = false;
        assert_eq!(
            recorder_escape(layers),
            Some(RecorderEscStep::ClearModeHover)
        );
        layers.mode_hover = false;
        assert_eq!(recorder_escape(layers), Some(RecorderEscStep::ClearSearch));
        layers.search_text = false;
        assert_eq!(recorder_escape(layers), Some(RecorderEscStep::BackPanel));
        layers.panel = false;
        assert_eq!(
            recorder_escape(layers),
            Some(RecorderEscStep::AbortClipFlow)
        );
        layers.clip_flow = false;
        assert_eq!(recorder_escape(layers), None);
    }
}
