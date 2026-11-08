//! The fullscreen target-select overlays -- `target-select-overlay.tsx`,
//! natively.
//!
//! One transparent window per display, opened when a capture target mode is
//! armed in the main window. The overlays own the real "Start Recording" flow:
//! the display variant records the display it covers, the window variant the
//! window under the cursor, the area variant a crop drawn on top of the
//! desktop.
//!
//! Two pieces live here:
//!
//! * [`TargetSelect`], an app-scoped entity holding the armed mode and what is
//!   under the cursor. The Tauri app computes the same thing in a 50ms tokio
//!   loop and pushes it to every overlay webview as a `TargetUnderCursor`
//!   event (`src-tauri/src/target_select_overlay.rs`); here the loop runs on
//!   gpui's background executor and the overlays observe the entity.
//! * [`OverlayWindow`], the per-display view.
//!
//! Metrics are transcribed from the TSX with the Tailwind class quoted next to
//! them, the same convention `main_window.rs` uses.

use std::cell::Cell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use frameever_recording::sources::screen_capture::ScreenCaptureTarget;
use gpui::{
    App, AppContext as _, Context, Entity, FontWeight, Global, Hsla, InteractiveElement as _,
    IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Render, SharedString, StatefulInteractiveElement as _, Styled, Window, div,
    prelude::FluentBuilder as _, px, svg,
};
use scap_targets::{
    DisplayId, WindowId,
    bounds::{LogicalBounds, LogicalPosition, LogicalSize},
};

mod area_math;

use area_math::{
    AREA_RATIOS, area_between, area_exact_size, area_fill, area_from_anchor_with_ratio,
    area_resize_with_ratio, area_resized_by, area_snap_rects, area_with_ratio, drag_ratio,
    fit_candidate, locked_area_for, ratio_value, snap_area_point, snap_area_rect,
    snap_resize_point, store_area_ratio, store_locked_area, stored_area_ratio, stored_locked_area,
};

const AREA_SELECTION_STORE: &str = "area_selection";

use crate::{
    app_windows,
    editor_canvas::{GuideAxis, NormRect, SnapGuide},
    i18n::{n, t, tr},
    main_window::{Mode, TargetType},
    theme::Theme,
};

/// How often the cursor probe runs. The Tauri loop sleeps 50ms between
/// `Display::get_containing_cursor` / `Window::get_topmost_at_cursor` reads;
/// this one is a touch lazier because each probe walks the whole window list
/// through `CGWindowListCopyWindowInfo` and the result only drives a
/// highlight rectangle.
const POLL_INTERVAL: Duration = Duration::from_millis(80);

/// `MIN_SIZE` in the TSX -- the smallest area selection that can be recorded.
const AREA_MIN_SIZE: f32 = 150.;

/// `MIN_SCREENSHOT_SIZE` in the TSX: a screenshot area only has to be a
/// pixel, not the 150px a recording needs.
const SCREENSHOT_MIN_SIZE: f32 = 1.;

fn area_min_size(mode: Mode) -> f32 {
    match mode {
        Mode::Screenshot => SCREENSHOT_MIN_SIZE,
        _ => AREA_MIN_SIZE,
    }
}

fn required_window_title_matches(required: Option<&str>, actual: Option<&str>) -> bool {
    required.is_none_or(|required| actual == Some(required))
}

fn overlay_display_is_active(
    cursor_display: Option<&DisplayId>,
    display: &DisplayId,
    only_overlay: bool,
) -> bool {
    cursor_display.map_or(only_overlay, |cursor| cursor == display)
}

fn overlay_viewport_size(viewport: (f32, f32), previous: (f32, f32)) -> (f32, f32) {
    if viewport.0.is_finite() && viewport.1.is_finite() && viewport.0 > 0. && viewport.1 > 0. {
        viewport
    } else {
        previous
    }
}

fn recording_devices_available(cx: &App) -> bool {
    crate::session::RecordingSession::global(cx).read(cx).phase == crate::session::Phase::Idle
        && cx
            .global::<app_windows::AppWindows>()
            .main
            .read(cx)
            .is_ok_and(|view| !view.is_preparing_recording())
}

/// How close to an edge or corner counts as grabbing that handle. The TSX's
/// corner buttons are 30px boxes hung 12px outside the crop and its edge
/// buttons are 10px strips straddling the border; this is the same reach
/// expressed as a distance test, because the hit zones are computed in one
/// place here rather than being eight elements with their own listeners.
const AREA_HANDLE_GRAB: f32 = 12.;

/// `w-104` on the controls cluster, and the height its one card comes out at
/// (`p-3` + `h-11` + `p-3`, plus `my-2.5` above and below). The TSX measures
/// its own controls element with a ResizeObserver to place it against the
/// crop; gpui has no layout read-back, so the two numbers are constants.
const CLUSTER_WIDTH: f32 = 416.;
const CLUSTER_HEIGHT: f32 = 88.;

fn area_controls_position(crop: AreaRect, available: AreaRect, cluster_height: f32) -> (f32, f32) {
    let left = available.x + 16.;
    let right = (available.right() - CLUSTER_WIDTH - 16.).max(left);
    let top = available.y + 40.;
    let bottom = (available.bottom() - cluster_height - 16.).max(top);
    let below = crop.bottom() + 16.;
    let above = crop.y - cluster_height - 16.;
    let y = if below <= bottom {
        below
    } else if above >= top {
        above
    } else {
        crop.y + 40.
    };
    (
        (crop.x + crop.width / 2. - CLUSTER_WIDTH / 2.).clamp(left, right),
        y.clamp(top, bottom),
    )
}

const COUNTDOWN_OPTIONS: &[(u32, &str)] = &[
    (0, n!("Off")),
    (3, n!("3 seconds")),
    (5, n!("5 seconds")),
    (10, n!("10 seconds")),
];

fn snap_off(modifiers: &gpui::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.platform
    } else {
        modifiers.control
    }
}

fn render_thirds(crop: AreaRect, theme: &Theme) -> impl IntoElement {
    let line = Theme::with_alpha(theme.fixed_white, 0.28);
    let mut grid = div()
        .absolute()
        .left(px(crop.x))
        .top(px(crop.y))
        .w(px(crop.width))
        .h(px(crop.height));
    for third in [1. / 3., 2. / 3.] {
        grid = grid
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px((crop.width * third).round()))
                    .w(px(1.))
                    .h_full()
                    .bg(line),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px((crop.height * third).round()))
                    .h(px(1.))
                    .w_full()
                    .bg(line),
            );
    }
    grid
}

const AREA_RESHAPE: Duration = Duration::from_millis(240);

/// The window under the cursor, as the overlays need it.
#[derive(Clone, Debug, PartialEq)]
pub struct HoveredWindow {
    pub id: WindowId,
    pub app_name: String,
    pub display_id: DisplayId,
    /// Bounds relative to the display's top-left, in logical points -- exactly
    /// what the overlay window's own coordinate space is. Carried as our own
    /// rect rather than `LogicalBounds` so the poll can compare frames for
    /// equality (`scap_targets`' bounds are not `PartialEq`).
    pub bounds: AreaRect,
}

impl HoveredWindow {
    pub fn from_window(window: &scap_targets::Window) -> Option<Self> {
        let bounds = window.display_relative_logical_bounds()?;
        Some(Self {
            id: window.id(),
            app_name: window.owner_name()?,
            display_id: window.display()?.id(),
            bounds: AreaRect {
                x: bounds.position().x() as f32,
                y: bounds.position().y() as f32,
                width: bounds.size().width() as f32,
                height: bounds.size().height() as f32,
            },
        })
    }
}

/// App-scoped target-select state: which mode is armed, what the cursor is
/// over, and which window a click has locked onto.
pub struct TargetSelect {
    /// `None` when no overlays are up.
    pub mode: Option<TargetType>,
    /// The recording mode the start cluster labels itself with.
    pub recording_mode: Mode,
    pub cursor_display: Option<DisplayId>,
    pub hovered_window: Option<HoveredWindow>,
    /// A window locked in by a click (or seeded from the main window's picker),
    /// which the highlight sticks to until another window is clicked.
    pub pinned_window: Option<HoveredWindow>,
    microphone_fill: f32,
    /// App icons by window id, fetched once per window.
    arranged: HashMap<String, ArrangedWindow>,
    pub focus_window: bool,
    pub focus_center: bool,
    _poll: Option<gpui::Task<()>>,
}

#[derive(Clone)]
pub struct ArrangedWindow {
    pub window: HoveredWindow,
    pub placement: Option<crate::foreign_window::Placement>,
}

enum Arrangement {
    Frame(AreaRect),
    PutBack(ArrangedWindow),
}

const FOCUS_SUPPORTED: bool = cfg!(any(target_os = "macos", windows));

fn saved_focus() -> bool {
    FOCUS_SUPPORTED
        && crate::store::GeneralSettings::load().window_capture_style
            == crate::store::WindowCaptureStyle::FocusedWindow
}

struct TargetSelectGlobal(Entity<TargetSelect>);
impl Global for TargetSelectGlobal {}

impl TargetSelect {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let select = cx.new(|_| Self {
            mode: None,
            recording_mode: Mode::Instant,
            cursor_display: None,
            hovered_window: None,
            pinned_window: None,
            microphone_fill: 0.,
            arranged: HashMap::new(),
            focus_window: saved_focus(),
            focus_center: crate::store::GeneralSettings::load().focus_center,
            _poll: None,
        });
        cx.set_global(TargetSelectGlobal(select.clone()));
        select
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<TargetSelectGlobal>().0.clone()
    }

    /// Arm (or disarm) a target mode. Starts and stops the cursor probe.
    pub fn arm(
        &mut self,
        mode: Option<TargetType>,
        recording_mode: Mode,
        pinned_window: Option<HoveredWindow>,
        cx: &mut Context<Self>,
    ) {
        self.recording_mode = recording_mode;
        self.pinned_window = pinned_window;
        if self.mode.is_none() && mode.is_some() {
            crate::foreign_window::put_back_all(self.take_arranged(), cx.background_executor());
            self.focus_window = saved_focus();
            self.focus_center = crate::store::GeneralSettings::load().focus_center;
        }
        if self.mode == mode {
            cx.notify();
            return;
        }
        self.mode = mode;
        self.hovered_window = None;
        if mode.is_some() {
            self.start_polling(cx);
        } else {
            self._poll = None;
            self.cursor_display = None;
        }
        cx.notify();
    }

    pub fn set_recording_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.recording_mode != mode {
            self.recording_mode = mode;
            cx.notify();
        }
    }

    /// The window the window-variant overlay should highlight: a locked
    /// selection wins over what the cursor is currently over, matching
    /// `activeWindow = selectedWindow() ?? targetUnderCursor.window`.
    pub fn active_window(&self) -> Option<&HoveredWindow> {
        self.pinned_window.as_ref().or(self.hovered_window.as_ref())
    }

    pub fn remember_arranged(&mut self, window: &HoveredWindow) {
        self.arranged
            .entry(window.id.to_string())
            .or_insert_with(|| ArrangedWindow {
                window: window.clone(),
                placement: crate::foreign_window::placement(&window.id),
            });
    }

    pub fn arranged_original(&self, id: &WindowId) -> Option<&ArrangedWindow> {
        self.arranged.get(&id.to_string())
    }

    pub fn has_arranged(&self) -> bool {
        !self.arranged.is_empty()
    }

    pub fn take_arranged(&mut self) -> Vec<ArrangedWindow> {
        self.arranged.drain().map(|(_, window)| window).collect()
    }

    pub fn set_pinned(&mut self, window: HoveredWindow, cx: &mut Context<Self>) {
        if self.pinned_window.as_ref() != Some(&window) {
            self.pinned_window = Some(window);
            cx.notify();
        }
    }

    /// Lock the highlight onto whatever the cursor is over -- the TSX's
    /// click-anywhere-to-select behavior.
    fn pin_hovered(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(hovered) = self.hovered_window.clone()
            && self.pinned_window.as_ref().map(|window| &window.id) != Some(&hovered.id)
        {
            self.pinned_window = Some(hovered);
            cx.notify();
            return true;
        }
        false
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        self._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                // Only the window variant needs the (much more expensive)
                // window probe; the others just want to know which display the
                // cursor is on.
                let Ok(want_window) =
                    this.read_with(cx, |this, _| this.mode == Some(TargetType::Window))
                else {
                    return;
                };

                let probe = cx
                    .background_executor()
                    .spawn(async move {
                        let display = scap_targets::Display::get_containing_cursor()
                            .map(|display| display.id());
                        let window = want_window
                            .then(crate::devices::topmost_foreign_window_at_cursor)
                            .flatten()
                            .as_ref()
                            .and_then(HoveredWindow::from_window);
                        (display, window)
                    })
                    .await;

                let Ok((active, changed)) = this.update(cx, |this: &mut Self, cx| {
                    let (display, window) = probe;
                    let microphone_fill = crate::ui::MicrophoneLevel::capture_fill(cx);
                    let changed = this.cursor_display != display
                        || this.hovered_window != window
                        || this.microphone_fill != microphone_fill;
                    this.microphone_fill = microphone_fill;
                    this.cursor_display = display;
                    this.hovered_window = window;
                    if changed {
                        cx.notify();
                    }

                    (this.active_window().cloned(), changed)
                }) else {
                    return;
                };
                // The overlay's camera-parking effect runs off the same signal
                // as the highlight (`target-select-overlay.tsx:518-545`), and
                // like the repaint below it has to happen with no entity borrow
                // held -- it moves a native window.
                cx.update(|cx| app_windows::sync_camera_park(active, cx));

                // Repaint the overlays explicitly: none of them is the active
                // window while the user hovers another app, and an inactive
                // window only repaints when asked (the unit-2 finding). A tick
                // that changed nothing they draw has nothing to repaint.
                if changed {
                    cx.update(app_windows::refresh_target_overlays);
                }
                cx.background_executor().timer(POLL_INTERVAL).await;
            }
        }));
    }
}

/// A rectangle in the overlay's own coordinate space: display-relative logical
/// points, which is also what `ScreenCaptureTarget::Area` wants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl AreaRect {
    fn right(&self) -> f32 {
        self.x + self.width
    }

    fn bottom(&self) -> f32 {
        self.y + self.height
    }

    fn is_valid_for(&self, min: f32) -> bool {
        self.width >= min && self.height >= min
    }

    fn intersected(self, display: (f32, f32)) -> Option<Self> {
        let (left, top) = (self.x.max(0.), self.y.max(0.));
        let (right, bottom) = (self.right().min(display.0), self.bottom().min(display.1));
        (right > left && bottom > top).then_some(Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }

    fn scaled(self, scale: f32) -> Self {
        Self {
            x: self.x * scale,
            y: self.y * scale,
            width: self.width * scale,
            height: self.height * scale,
        }
    }

    fn clamped(self, display: (f32, f32)) -> Self {
        let width = self.width.min(display.0);
        let height = self.height.min(display.1);
        Self {
            x: self.x.clamp(0., (display.0 - width).max(0.)),
            y: self.y.clamp(0., (display.1 - height).max(0.)),
            width,
            height,
        }
    }

    /// `x,y,width,height` -- the `FRAMEEVER_GPUI_AUTO_AREA` harness format.
    pub fn parse(spec: &str) -> Option<Self> {
        let values: Vec<f32> = spec
            .split(',')
            .map(|part| part.trim().parse().ok())
            .collect::<Option<Vec<f32>>>()?;
        let [x, y, width, height] = values[..] else {
            return None;
        };
        Some(Self {
            x,
            y,
            width,
            height,
        })
    }

    fn to_bounds(self) -> LogicalBounds {
        LogicalBounds::new(
            LogicalPosition::new(self.x as f64, self.y as f64),
            LogicalSize::new(self.width as f64, self.height as f64),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AreaHandle {
    North,
    South,
    East,
    West,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

impl AreaHandle {
    const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
    ];

    fn north(self) -> bool {
        matches!(self, Self::North | Self::NorthWest | Self::NorthEast)
    }

    fn south(self) -> bool {
        matches!(self, Self::South | Self::SouthWest | Self::SouthEast)
    }

    fn west(self) -> bool {
        matches!(self, Self::West | Self::NorthWest | Self::SouthWest)
    }

    fn east(self) -> bool {
        matches!(self, Self::East | Self::NorthEast | Self::SouthEast)
    }

    fn is_corner(self) -> bool {
        matches!(
            self,
            Self::NorthWest | Self::NorthEast | Self::SouthWest | Self::SouthEast
        )
    }

    fn cursor(self) -> gpui::CursorStyle {
        use gpui::CursorStyle::*;
        match self {
            Self::North | Self::South => ResizeUpDown,
            Self::East | Self::West => ResizeLeftRight,
            Self::NorthWest | Self::SouthEast => ResizeUpLeftDownRight,
            Self::NorthEast | Self::SouthWest => ResizeUpRightDownLeft,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::North => "handle-n",
            Self::South => "handle-s",
            Self::East => "handle-e",
            Self::West => "handle-w",
            Self::NorthWest => "handle-nw",
            Self::NorthEast => "handle-ne",
            Self::SouthWest => "handle-sw",
            Self::SouthEast => "handle-se",
        }
    }
}

enum AreaDrag {
    /// Drawing a fresh selection from an anchor point.
    Draw {
        anchor: (f32, f32),
        previous: Option<AreaRect>,
    },
    /// Dragging the whole selection.
    Move {
        grab: (f32, f32),
        start: AreaRect,
    },
    Resize {
        handle: AreaHandle,
        start: AreaRect,
    },
}

/// One display's overlay.
pub struct OverlayWindow {
    theme: Theme,
    select: Entity<TargetSelect>,
    display_id: DisplayId,
    display_name: String,
    /// Logical size, which is this window's size and the space every
    /// coordinate here lives in.
    display_size: (f32, f32),
    controls_work_area: AreaRect,
    /// Physical pixels, for the `1920x1080 · 60FPS` line.
    physical_size: Option<(u32, u32)>,
    refresh_rate: f64,
    focus: gpui::FocusHandle,
    crop: Option<AreaRect>,
    drag: Option<AreaDrag>,
    snap_guides: Vec<SnapGuide>,
    area_ratio: Option<(u32, u32)>,
    area_locked: bool,
    area_snap: Vec<NormRect>,
    area_windows: Vec<AreaRect>,
    fit_hover: Option<AreaRect>,
    area_windows_requested: bool,
    aspect_menu: Option<crate::ui::MenuState>,
    reshape: Option<(AreaRect, Instant)>,
    window_menu: Option<crate::ui::MenuState>,
    window_reshape: Option<(AreaRect, Instant)>,
    window_drawn: Cell<Option<AreaRect>>,
    recording_area: bool,
    /// The mode menu the start pill's caret drops down -- `menuModes()`
    /// popped up at the event location (`target-select-overlay.tsx:2064-2090,
    /// 2206-2210`). `Some` while it is open.
    mode_menu: Option<crate::ui::MenuState>,
    countdown_menu: Option<crate::ui::MenuState>,
    inline_camera: Option<Entity<crate::camera_window::CameraWindow>>,
    inline_camera_subscription: Option<gpui::Subscription>,
    empty_camera_state: crate::store::CameraWindowState,
    device_menu: Option<OverlayDeviceMenu>,
    _main_subscription: Option<gpui::Subscription>,
}

#[derive(Clone)]
enum OverlayDeviceChoice {
    Camera(Option<crate::devices::CameraOption>),
    Microphone(Option<crate::devices::MicrophoneOption>),
    Access(crate::permissions::OSPermission),
}

struct OverlayDeviceMenu {
    state: crate::ui::MenuState,
    items: Vec<crate::ui::MenuItem>,
    choices: Vec<OverlayDeviceChoice>,
}

/// `menuModes` (`target-select-overlay.tsx:2064-2090`): three check items, in
/// this order, checked on the current mode. Picking one switches the mode and
/// never starts a recording.
const MODE_MENU: [(&str, Mode); 3] = [
    (n!("Studio Mode"), Mode::Studio),
    (n!("Instant Mode"), Mode::Instant),
    (n!("Screenshot Mode"), Mode::Screenshot),
];

fn mode_menu_items(current: Mode) -> Vec<crate::ui::MenuItem> {
    MODE_MENU
        .into_iter()
        .map(|(label, mode)| crate::ui::MenuItem::new(tr(label), mode == current))
        .collect()
}

/// Overlays are never the active window while the picker is up, and an
/// inactive window may not present a bare notify, so every state an overlay
/// draws asks for its repaint explicitly.
fn repaint(window: &mut Window, cx: &mut Context<OverlayWindow>) {
    cx.notify();
    window.refresh();
}

impl OverlayWindow {
    pub fn new(
        display: &scap_targets::Display,
        select: Entity<TargetSelect>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::theme::bind_window(window, cx);
        let theme = Theme::for_window(window, cx, false);
        cx.observe_in(&select, window, |_, _, window, cx| repaint(window, cx))
            .detach();
        let session = crate::session::RecordingSession::global(cx);
        let mut drawn_phase = session.read(cx).phase;
        cx.observe_in(&session, window, move |_, session, window, cx| {
            let phase = session.read(cx).phase;
            if phase != drawn_phase {
                drawn_phase = phase;
                repaint(window, cx);
            }
        })
        .detach();
        let main = cx.global::<app_windows::AppWindows>().main;
        let main_subscription = main
            .update(cx, |_, _, cx| cx.entity())
            .ok()
            .map(|main| cx.observe_in(&main, window, |_, _, window, cx| repaint(window, cx)));
        let media = crate::media_access::MediaAccess::global(cx);
        media.update(cx, |access, cx| access.refresh(cx));
        cx.observe_in(&media, window, |_, _, window, cx| repaint(window, cx))
            .detach();

        let logical = display
            .logical_size()
            .map(|size| (size.width() as f32, size.height() as f32))
            .unwrap_or((1920., 1080.));

        let controls_work_area = display
            .raw_handle()
            .logical_bounds()
            .zip(app_windows::display_work_area(Some(display), cx))
            .map(|(bounds, available)| AreaRect {
                x: f32::from(available.origin.x) - bounds.position().x() as f32,
                y: f32::from(available.origin.y) - bounds.position().y() as f32,
                width: f32::from(available.size.width),
                height: f32::from(available.size.height),
            })
            .unwrap_or(AreaRect {
                x: 0.,
                y: 0.,
                width: logical.0,
                height: logical.1,
            })
            .clamped(logical);

        let locked_area = locked_area_for(
            &display.id(),
            logical,
            area_min_size(select.read(cx).recording_mode),
        );

        Self {
            theme,
            select,
            display_id: display.id(),
            display_name: display
                .name()
                .unwrap_or_else(|| t!("Display {id}", id = display.id())),
            display_size: logical,
            controls_work_area,
            physical_size: display
                .physical_size()
                .map(|size| (size.width() as u32, size.height() as u32)),
            refresh_rate: display.refresh_rate(),
            focus: cx.focus_handle(),
            crop: locked_area,
            drag: None,
            snap_guides: Vec::new(),
            area_ratio: stored_area_ratio(),
            area_locked: locked_area.is_some(),
            area_snap: area_snap_rects(controls_work_area, &[], logical),
            area_windows: Vec::new(),
            fit_hover: None,
            area_windows_requested: false,
            aspect_menu: None,
            reshape: None,
            window_menu: None,
            window_reshape: None,
            window_drawn: Cell::new(None),
            recording_area: false,
            mode_menu: None,
            countdown_menu: None,
            inline_camera: None,
            inline_camera_subscription: None,
            empty_camera_state: crate::store::load().camera_window.unwrap_or_default(),
            device_menu: None,
            _main_subscription: main_subscription,
        }
    }

    pub fn focus_handle(&self) -> gpui::FocusHandle {
        self.focus.clone()
    }

    /// Seed an area selection without drawing one -- the harness path
    /// (`FRAMEEVER_GPUI_AUTO_AREA`), since unprivileged synthetic drags are dropped.
    pub fn set_crop(&mut self, crop: AreaRect, cx: &mut Context<Self>) {
        if self.recording_area {
            return;
        }
        self.crop = Some(crop.clamped(self.display_size));
        self.sync_area_camera(cx);
        cx.notify();
    }

    pub fn commit_area_for_recording(&mut self, cx: &mut Context<Self>) -> bool {
        if self.select.read(cx).mode != Some(TargetType::Area) || self.target(cx).is_none() {
            return false;
        }
        self.recording_area = true;
        self.drag = None;
        self.snap_guides.clear();
        self.mode_menu = None;
        self.countdown_menu = None;
        self.device_menu = None;
        self.aspect_menu = None;
        self.reshape = None;
        cx.notify();
        true
    }

    pub fn is_recording_area(&self) -> bool {
        self.recording_area
    }

    /// The target this overlay would record, or `None` when it has nothing to
    /// record yet (no window under the cursor, no valid area drawn).
    pub fn target(&self, cx: &App) -> Option<ScreenCaptureTarget> {
        if self.recording_area {
            return self.crop.map(|crop| ScreenCaptureTarget::Area {
                screen: self.display_id.clone(),
                bounds: crop.to_bounds(),
            });
        }
        let select = self.select.read(cx);
        let min = area_min_size(select.recording_mode);
        match select.mode? {
            TargetType::Display => Some(ScreenCaptureTarget::Display {
                id: self.display_id.clone(),
            }),
            TargetType::Window => {
                select
                    .active_window()
                    .map(|window| ScreenCaptureTarget::Window {
                        id: window.id.clone(),
                    })
            }
            TargetType::Area => self.crop.filter(|crop| crop.is_valid_for(min)).map(|crop| {
                ScreenCaptureTarget::Area {
                    screen: self.display_id.clone(),
                    bounds: crop.to_bounds(),
                }
            }),
            TargetType::CameraOnly => Some(ScreenCaptureTarget::CameraOnly),
        }
    }

    /// The overlay's Start button: close every overlay, then hand the target to
    /// the main window's start path (which opens the bar, hides the main
    /// window and starts the engine).
    pub fn start_recording(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target(cx) else {
            return;
        };
        if let ScreenCaptureTarget::Window { id } = &target {
            let window = scap_targets::Window::from_id(id);
            let expected_title = crate::main_window::auto_window_title();
            let actual_title = window.as_ref().and_then(scap_targets::Window::name);
            if window.is_none()
                || !required_window_title_matches(
                    expected_title.as_deref(),
                    actual_title.as_deref(),
                )
            {
                tracing::warn!(
                    window = %id,
                    expected = ?expected_title,
                    actual = ?actual_title,
                    "refusing unavailable or unexpected recording window"
                );
                cx.defer(app_windows::reject_unavailable_window);
                return;
            }
        }
        tracing::info!(target = ?target.kind_str(), "overlay start pressed");
        if self.select.read(cx).recording_mode == Mode::Screenshot {
            // Screenshots never reach the recording actors: the target goes
            // straight to the capture path, which closes these overlays
            // itself (`startRecording`'s screenshot branch,
            // `target-select-overlay.tsx:1995`).
            cx.defer(move |cx: &mut App| crate::screenshot::take_screenshot(target, cx));
            return;
        }
        // Deferred: this runs inside the overlay's own window update and the
        // orchestrator closes that very window.
        cx.defer(move |cx: &mut App| app_windows::start_recording_from_overlay(target, cx));
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.defer(app_windows::dismiss_target_overlays);
    }

    fn sync_appearance(&mut self, window: &Window, cx: &gpui::App) {
        self.theme.refresh(window, cx, false);
    }

    fn sync_inline_camera(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let camera = (self.select.read(cx).mode == Some(TargetType::CameraOnly))
            .then(|| app_windows::camera_preview_entity(cx))
            .flatten();
        if self.inline_camera.as_ref().map(Entity::entity_id)
            == camera.as_ref().map(Entity::entity_id)
        {
            return;
        }
        self.inline_camera_subscription = camera
            .as_ref()
            .map(|camera| cx.observe_in(camera, window, |_, _, window, cx| repaint(window, cx)));
        self.inline_camera = camera;
    }

    fn sync_area_camera(&self, cx: &mut Context<Self>) {
        if self.select.read(cx).recording_mode == Mode::Screenshot {
            return;
        }
        let display = self.display_id.clone();
        let crop = self.crop;
        let epoch = app_windows::camera_picker_epoch(cx);
        cx.defer(move |cx| {
            if app_windows::camera_picker_epoch(cx) == epoch {
                app_windows::sync_camera_area(display, crop, cx);
            }
        });
    }

    /// True when the cursor is on this overlay's display -- `data-over` in the
    /// display variant, `isActiveDisplay` in the area variant.
    fn is_active_display(&self, cx: &App) -> bool {
        overlay_display_is_active(
            self.select.read(cx).cursor_display.as_ref(),
            &self.display_id,
            cx.global::<app_windows::AppWindows>().overlays.len() == 1,
        )
    }
}

impl Render for OverlayWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_appearance(window, cx);
        self.sync_inline_camera(window, cx);
        let viewport = window.viewport_size();
        let display_size = overlay_viewport_size(
            (f32::from(viewport.width), f32::from(viewport.height)),
            self.display_size,
        );
        if !self.recording_area && self.display_size != display_size {
            self.display_size = display_size;
            self.crop = self.crop.map(|crop| crop.clamped(display_size));
            self.drag = None;
            self.snap_guides.clear();
            self.sync_area_camera(cx);
        }
        let mode = self.select.read(cx).mode;

        let root = div()
            .id("overlay-root")
            .track_focus(&self.focus)
            .key_context("TargetSelectOverlay")
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if let Some(menu) = &mut this.countdown_menu {
                    match menu.on_key(&event.keystroke.key) {
                        crate::ui::MenuKey::Commit(index) => this.choose_countdown(index, cx),
                        crate::ui::MenuKey::Dismiss => {
                            this.countdown_menu = None;
                            cx.notify();
                        }
                        crate::ui::MenuKey::Moved => cx.notify(),
                        crate::ui::MenuKey::Ignored => return,
                    }
                    cx.stop_propagation();
                    return;
                }
                if let Some(menu) = &mut this.window_menu {
                    match menu.on_key(&event.keystroke.key) {
                        crate::ui::MenuKey::Commit(index) => this.choose_window_size(index, cx),
                        crate::ui::MenuKey::Dismiss => {
                            this.window_menu = None;
                            cx.notify();
                        }
                        crate::ui::MenuKey::Moved => cx.notify(),
                        crate::ui::MenuKey::Ignored => return,
                    }
                    cx.stop_propagation();
                    return;
                }
                if let Some(menu) = &mut this.aspect_menu {
                    match menu.on_key(&event.keystroke.key) {
                        crate::ui::MenuKey::Commit(index) => this.choose_aspect(index, cx),
                        crate::ui::MenuKey::Dismiss => {
                            this.aspect_menu = None;
                            cx.notify();
                        }
                        crate::ui::MenuKey::Moved => cx.notify(),
                        crate::ui::MenuKey::Ignored => return,
                    }
                    cx.stop_propagation();
                    return;
                }
                if let Some(menu) = &mut this.device_menu {
                    match menu.state.on_key(&event.keystroke.key) {
                        crate::ui::MenuKey::Commit(index) => this.choose_device(index, cx),
                        crate::ui::MenuKey::Dismiss => {
                            this.device_menu = None;
                            cx.notify();
                        }
                        crate::ui::MenuKey::Moved => cx.notify(),
                        crate::ui::MenuKey::Ignored => return,
                    }
                    cx.stop_propagation();
                    return;
                }
                // The Tauri app registers Escape as a *global* shortcut while
                // the overlays are up; here it is a plain key handler on the
                // overlay that has focus (see the README deviation).
                if this.mode_menu.is_none() && this.area_key(&event.keystroke, window, cx) {
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.key.as_str() == "escape" {
                    // An open mode menu takes the Escape before the
                    // overlay does -- the native menu in the Tauri app
                    // eats it the same way during menu tracking.
                    if this.mode_menu.is_some() {
                        this.mode_menu = None;
                        cx.notify();
                    } else {
                        this.dismiss(cx);
                    }
                }
            }))
            .size_full()
            .relative()
            .font_family(crate::theme::ui_font())
            // `body { font-weight: 500 }` (`ui-solid/src/main.css:189-192`).
            .font_weight(FontWeight::MEDIUM)
            .text_color(gpui::white());

        let root = if self.recording_area {
            root.child(self.render_area_variant(window, cx))
        } else {
            match mode {
                Some(TargetType::Display) => root.child(self.render_display_variant(cx)),
                Some(TargetType::Window) => root.child(self.render_window_variant(window, cx)),
                Some(TargetType::Area) => root.child(self.render_area_variant(window, cx)),
                Some(TargetType::CameraOnly) => root.child(self.render_camera_variant(cx)),
                None => root,
            }
        };
        // Painted after the variant so the dropdown sits above the start
        // cluster it drops from.
        root.children(self.render_mode_menu(cx))
            .children(self.render_countdown_menu(cx))
            .children(self.render_aspect_menu(cx))
            .children(self.render_window_menu(cx))
            .children(self.render_device_menu(cx))
            .into_any_element()
    }
}

impl OverlayWindow {
    fn render_device_menu(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let menu = self.device_menu.as_ref()?;
        Some(
            crate::ui::Menu::plain(
                &self.theme,
                "overlay-device-menu",
                menu.items.clone(),
                &menu.state,
            )
            .on_select(cx.listener(|this, index: &usize, _, cx| {
                this.choose_device(*index, cx);
            }))
            .on_dismiss(cx.listener(|this, _, _, cx| {
                this.device_menu = None;
                cx.notify();
            }))
            .into_any_element(),
        )
    }

    fn choose_device(&mut self, index: usize, cx: &mut Context<Self>) {
        let choice = self
            .device_menu
            .take()
            .and_then(|menu| menu.choices.get(index).cloned());
        if let Some(choice) = choice {
            self.apply_device_choice(choice, cx);
        }
        cx.notify();
    }

    fn apply_device_choice(&mut self, choice: OverlayDeviceChoice, cx: &mut Context<Self>) {
        self.device_menu = None;
        let main = cx.global::<app_windows::AppWindows>().main;
        let needs_access = match &choice {
            OverlayDeviceChoice::Camera(Some(_)) => Some(crate::permissions::OSPermission::Camera),
            OverlayDeviceChoice::Microphone(Some(_)) => {
                Some(crate::permissions::OSPermission::Microphone)
            }
            OverlayDeviceChoice::Access(permission) => Some(*permission),
            _ => None,
        };
        if let Some(permission) = needs_access {
            request_media_access(permission, cx);
        }
        cx.defer(move |cx| {
            if !recording_devices_available(cx) {
                return;
            }
            if let Err(error) = main.update(cx, |view, _, cx| match choice {
                OverlayDeviceChoice::Camera(camera) => view.set_camera_selection(camera, cx),
                OverlayDeviceChoice::Microphone(mic) => view.set_microphone_selection(mic, cx),
                OverlayDeviceChoice::Access(_) => {}
            }) {
                tracing::warn!(%error, "could not update the recording device");
            }
        });
        cx.notify();
    }

    fn open_device_menu(&mut self, camera: bool, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !recording_devices_available(cx) {
            return;
        }
        let main = cx.global::<app_windows::AppWindows>().main;
        let Ok(view) = main.read(cx) else { return };
        if view.is_enumerating_devices() {
            return;
        }
        let permission = if camera {
            crate::permissions::OSPermission::Camera
        } else {
            crate::permissions::OSPermission::Microphone
        };
        let access = crate::media_access::MediaAccess::global(cx);
        let access_item = (!access.read(cx).allowed(permission)).then(|| {
            let label = match (
                camera,
                access.read(cx).status(permission)
                    == crate::permissions::OSPermissionStatus::Denied,
            ) {
                (true, true) => t!("Turn on camera access in Settings…"),
                (true, false) => t!("Allow camera access…"),
                (false, true) => t!("Turn on microphone access in Settings…"),
                (false, false) => t!("Allow microphone access…"),
            };
            (
                crate::ui::MenuItem::new(label, false),
                OverlayDeviceChoice::Access(permission),
            )
        });
        let (mut items, mut choices) = if camera {
            let mut items = vec![crate::ui::MenuItem::new(
                t!("No Camera"),
                view.camera_selection().is_none(),
            )];
            let mut choices = vec![OverlayDeviceChoice::Camera(None)];
            for camera in &view.device_snapshot().cameras {
                items.push(crate::ui::MenuItem::new(
                    camera.label.clone(),
                    view.camera_selection()
                        .is_some_and(|selected| selected.device_id == camera.device_id),
                ));
                choices.push(OverlayDeviceChoice::Camera(Some(camera.clone())));
            }
            (items, choices)
        } else {
            let mut items = vec![crate::ui::MenuItem::new(
                t!("No Microphone"),
                view.microphone_selection().is_none(),
            )];
            let mut choices = vec![OverlayDeviceChoice::Microphone(None)];
            for mic in &view.device_snapshot().microphones {
                items.push(crate::ui::MenuItem::new(
                    mic.name.clone(),
                    view.microphone_selection()
                        .is_some_and(|selected| selected.name == mic.name),
                ));
                choices.push(OverlayDeviceChoice::Microphone(Some(mic.clone())));
            }
            (items, choices)
        };
        if let Some((item, choice)) = access_item {
            items.insert(0, item);
            choices.insert(0, choice);
        }
        self.mode_menu = None;
        self.countdown_menu = None;
        self.device_menu = Some(OverlayDeviceMenu {
            state: crate::ui::MenuState::new(position, &items),
            items,
            choices,
        });
        cx.notify();
    }

    fn render_device_control(&self, camera: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let main = cx.global::<app_windows::AppWindows>().main;
        let Ok(view) = main.read(cx) else {
            return div().into_any_element();
        };
        let selected = if camera {
            view.camera_selection()
                .map(|camera| crate::main_window::camera_label(&camera.label))
        } else {
            view.microphone_selection().map(|mic| mic.name.clone())
        };
        let enabled = selected.is_some();
        let disabled = view.is_enumerating_devices() || !recording_devices_available(cx);
        let theme = self.theme;
        let permission = if camera {
            crate::permissions::OSPermission::Camera
        } else {
            crate::permissions::OSPermission::Microphone
        };
        let access = crate::media_access::MediaAccess::global(cx);
        let access = access.read(cx);
        let blocked = enabled && !access.allowed(permission);
        let pill_label = if !blocked {
            if enabled { t!("On") } else { t!("Off") }
        } else if access.requesting(permission) {
            t!("Asking…")
        } else if access.status(permission) == crate::permissions::OSPermissionStatus::Denied {
            t!("Allow in Settings")
        } else {
            t!("Allow")
        };
        div()
            .id(if camera {
                "overlay-camera"
            } else {
                "overlay-microphone"
            })
            .relative()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .h(px(42.))
            .flex_1()
            .min_w_0()
            .rounded(px(8.))
            .border_1()
            .border_color(theme.gray_5)
            .bg(theme.gray_3)
            .text_color(theme.gray_12)
            .overflow_hidden()
            .when(disabled, |row| row.opacity(0.7))
            .when(!camera && enabled && !disabled, |row| {
                row.child(
                    crate::ui::MicrophoneLevel::snapshot(
                        self.select.read(cx).microphone_fill,
                        theme.success.into(),
                    )
                    .absolute()
                    .top_0()
                    .left_0(),
                )
            })
            .child(
                svg()
                    .path(if camera {
                        "icons/webcam.svg"
                    } else {
                        "icons/mic.svg"
                    })
                    .size(px(16.))
                    .flex_shrink_0()
                    .text_color(theme.gray_11),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(14.))
                    .child(selected.unwrap_or_else(|| {
                        if camera {
                            t!("No Camera")
                        } else {
                            t!("No Microphone")
                        }
                        .into()
                    })),
            )
            .child(
                div()
                    .id(if camera {
                        "overlay-camera-toggle"
                    } else {
                        "overlay-microphone-toggle"
                    })
                    .px(px(10.))
                    .h(px(24.))
                    .min_w(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .text_size(px(11.))
                    .bg(if blocked {
                        Hsla::from(theme.amber_3)
                    } else if enabled {
                        Hsla::from(theme.primary)
                    } else {
                        Hsla::from(theme.gray_5)
                    })
                    .text_color(if blocked {
                        Hsla::from(theme.amber_11)
                    } else if enabled {
                        gpui::white()
                    } else {
                        Hsla::from(theme.gray_11)
                    })
                    .child(pill_label)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                        cx.stop_propagation();
                        if disabled {
                            return;
                        }
                        if blocked {
                            this.apply_device_choice(OverlayDeviceChoice::Access(permission), cx);
                        } else if enabled {
                            this.apply_device_choice(
                                if camera {
                                    OverlayDeviceChoice::Camera(None)
                                } else {
                                    OverlayDeviceChoice::Microphone(None)
                                },
                                cx,
                            );
                        } else {
                            this.open_device_menu(camera, event.position(), cx);
                        }
                    })),
            )
            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                this.open_device_menu(camera, event.position(), cx);
            }))
            .into_any_element()
    }

    /// The caret's dropdown while it is open -- `Menu.popup` in the source,
    /// rendered with the app's own menu component here (the closest thing to
    /// a native context menu this side of AppKit; check state, outside-click
    /// dismissal and Escape behave the same).
    fn render_mode_menu(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let state = self.mode_menu.as_ref()?;
        let items = mode_menu_items(self.select.read(cx).recording_mode);
        Some(
            crate::ui::Menu::plain(&self.theme, "overlay-mode-menu", items, state)
                .on_select(cx.listener(|this, index: &usize, _window, cx| {
                    this.choose_mode(*index, cx);
                }))
                .on_dismiss(cx.listener(|this, _, _window, cx| {
                    this.mode_menu = None;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    fn render_countdown_menu(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let state = self.countdown_menu.as_ref()?;
        let current = crate::store::GeneralSettings::load()
            .recording_countdown
            .unwrap_or(0);
        let items = COUNTDOWN_OPTIONS
            .iter()
            .map(|(value, label)| crate::ui::MenuItem::new(tr(label), *value == current))
            .collect();
        Some(
            crate::ui::Menu::plain(&self.theme, "overlay-countdown-menu", items, state)
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.choose_countdown(*index, cx);
                }))
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.countdown_menu = None;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    fn choose_countdown(&mut self, index: usize, cx: &mut Context<Self>) {
        self.countdown_menu = None;
        if let Some((value, _)) = COUNTDOWN_OPTIONS.get(index)
            && !crate::store::set_store_setting(
                crate::store::GENERAL_SETTINGS,
                "recordingCountdown",
                serde_json::Value::from(*value),
            )
        {
            tracing::warn!(value, "could not save the recording countdown");
        }
        cx.notify();
    }

    /// A `CheckMenuItem` action: `setOptions("mode", ..)` +
    /// `commands.setRecordingMode(..)` -- the app's single mode funnel
    /// (`app_windows::set_recording_mode` -> `MainWindow::set_mode`), which
    /// writes the shared store key, relabels every open overlay through
    /// `TargetSelect::set_recording_mode`, and refreshes the tray.
    fn choose_mode(&mut self, index: usize, cx: &mut Context<Self>) {
        self.mode_menu = None;
        if crate::session::RecordingSession::global(cx)
            .read(cx)
            .editor_recording_target()
            .is_some()
        {
            cx.notify();
            return;
        }
        if let Some((_, mode)) = MODE_MENU.get(index).copied() {
            app_windows::set_recording_mode(mode, cx);
        }
        cx.notify();
    }

    fn render_display_variant(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let over = self.is_active_display(cx);
        let mut label = self.display_name.clone();
        if let Some((width, height)) = self.physical_size {
            label.push_str(&format!(
                " · {width} × {height} · {} FPS",
                self.refresh_rate.round()
            ));
        }

        div()
            .size_full()
            .relative()
            .when(!over, |this| {
                this.children(scrim(None, self.display_size, self.theme.overlay.into()))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                overlay_badge(&self.theme)
                                    .h(px(32.))
                                    .text_size(px(16.))
                                    .child(SharedString::from(self.display_name.clone())),
                            ),
                    )
            })
            .when(over, |this| {
                this.child(
                    overlay_badge(&self.theme)
                        .absolute()
                        .left(px(12.))
                        .top(px(12.))
                        .child(label),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(self.render_controls_cluster(self.target(cx), false, cx)),
                )
            })
    }

    fn render_window_variant(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let select = self.select.read(cx);
        let focus = select.focus_window && crate::foreign_window::can_arrange();
        let active = select
            .active_window()
            .filter(|window| window.display_id == self.display_id)
            .cloned();

        let Some(active) = active else {
            self.window_drawn.set(None);
            return div().size_full().into_any_element();
        };
        let target = active.bounds;
        let bounds = match self.window_reshape {
            Some((from, started)) => {
                let t = started.elapsed().as_secs_f32() / AREA_RESHAPE.as_secs_f32();
                if t >= 1. {
                    self.window_reshape = None;
                    target
                } else {
                    window.request_animation_frame();
                    area_between(from, target, t)
                }
            }
            None => target,
        };
        self.window_drawn.set(Some(bounds));

        let scale = self.pixels_per_point();
        let tag = format!(
            "{} · {} × {}",
            active.app_name,
            (target.width * scale).round(),
            (target.height * scale).round()
        );
        let (tag_x, tag_y) = tag_position(bounds);

        div()
            .size_full()
            .relative()
            .id("window-variant")
            .on_click(cx.listener(|this, _, _window, cx| {
                this.window_menu = None;
                let picked = this.select.update(cx, |select, cx| select.pin_hovered(cx));
                if picked && this.select.read(cx).focus_window {
                    this.set_focus(true, cx);
                }
                cx.defer(app_windows::release_camera_park);
            }))
            .children(scrim(
                Some(bounds),
                self.display_size,
                self.theme.overlay.into(),
            ))
            .child(selection_frame(bounds, &self.theme))
            .when(focus, |variant| {
                variant.child(focus_ring(bounds, &self.theme))
            })
            .child(
                overlay_badge(&self.theme)
                    .absolute()
                    .left(px(tag_x))
                    .top(px(tag_y))
                    .child(tag),
            )
            .child(
                div()
                    .absolute()
                    .left(px(bounds.x))
                    .top(px(bounds.y))
                    .w(px(bounds.width))
                    .h(px(bounds.height))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .child(self.render_window_size(target, cx))
                    .children(self.render_focus_toggle(cx))
                    .child(self.render_controls_cluster(self.target(cx), false, cx)),
            )
            .into_any_element()
    }

    fn render_camera_variant(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = if let Some(camera) = &self.inline_camera {
            let (width, height) = camera.read(cx).inline_size(self.display_size);
            div()
                .w(px(width))
                .h(px(height))
                .flex_none()
                .child(camera.clone())
                .into_any_element()
        } else {
            let (width, height) = crate::camera_window::inline_preview_size(
                &self.empty_camera_state,
                None,
                self.display_size,
            );
            div()
                .w(px(width))
                .h(px(height))
                .pt(px(crate::camera_window::CAMERA_TOOLBAR_HEIGHT))
                .child(
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(8.))
                        .rounded(px(crate::camera_window::preview_radius(
                            &self.empty_camera_state,
                        )))
                        .border_1()
                        .border_color(self.theme.gray_6)
                        .bg(gpui::black())
                        .text_size(px(14.))
                        .text_color(self.theme.gray_11)
                        .child(svg().path("icons/webcam.svg").size(px(32.)))
                        .child(t!("Please select a camera")),
                )
                .into_any_element()
        };
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            // `bg-black/70` with a second `bg-black/60` layer behind it.
            .bg(gpui::hsla(0., 0., 0., 0.7))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .mb(px(16.))
                    .child(
                        div()
                            .mb(px(8.))
                            .text_size(px(30.))
                            // `text-3xl font-semibold`
                            // (`target-select-overlay.tsx:392`): renders 700,
                            // no 600 face loaded.
                            .font_weight(FontWeight::BOLD)
                            .child(t!("Camera Only")),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(self.theme.gray_11)
                            .child(t!("Record using only your camera and microphone")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .w_full()
                    .px(px(24.))
                    .mb(px(16.))
                    .child(preview),
            )
            .child(self.render_controls_cluster(self.target(cx), false, cx))
    }

    /// The crop overlay: `bg-black/45` outside the selection, a `border-white/50`
    /// region with eight handles, a size readout on top and the start cluster
    /// placed against the crop.
    fn render_area_variant(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let interacting = self.drag.is_some();
        // `shouldShowOverlay = isInteracting() || isActiveDisplay()`.
        let visible = self.recording_area || interacting || self.is_active_display(cx);
        let crop = self.crop;
        let shown = self.displayed_crop(window);
        let min = area_min_size(self.select.read(cx).recording_mode);
        let valid = crop.is_some_and(|crop| crop.is_valid_for(min));
        let interactive = !self.recording_area;
        let editable = interactive && !self.area_locked;
        if interactive && !self.area_windows_requested {
            self.load_area_windows(cx);
        }

        let mut root = div()
            .id("area-root")
            .size_full()
            .relative()
            .when(interactive, |this| {
                this.when(editable, |this| this.cursor(gpui::CursorStyle::Crosshair))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                            this.area_mouse_down(event.position, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                        this.area_mouse_move(
                            event.position,
                            snap_off(&event.modifiers),
                            event.modifiers.shift,
                            cx,
                        );
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseUpEvent, _window, cx| {
                            this.area_mouse_up(
                                event.position,
                                snap_off(&event.modifiers),
                                event.modifiers.shift,
                                cx,
                            );
                        }),
                    )
            });

        if !visible {
            return root.invisible();
        }

        if self.recording_area {
            if let Some(shown) = shown {
                root = root.children(corner_marks(shown, &self.theme));
            }
            return root;
        }

        root = root.children(scrim(shown, self.display_size, self.theme.overlay.into()));

        if let Some(shown) = shown {
            root = root.child(self.render_crop_region(shown, editable));
            if self.drag.is_some() {
                root = root.child(render_thirds(shown, &self.theme));
            }
        }
        if let Some(hover) = self.fit_hover.filter(|_| editable && !interacting) {
            root = root.child(
                div()
                    .absolute()
                    .left(px(hover.x))
                    .top(px(hover.y))
                    .w(px(hover.width))
                    .h(px(hover.height))
                    .border_1()
                    .border_dashed()
                    .border_color(self.theme.fixed_white),
            );
        }
        if let Some(shown) = shown {
            root = root.child(self.render_size_tag(shown, valid, min));
        }
        if let Some(shown) = shown.filter(|_| self.area_locked && !interacting) {
            root = root.child(self.render_locked_badge(shown, cx));
        }

        root = root.children(self.render_snap_guides());
        if crop.is_none() && !interacting {
            root = root.child(self.render_selection_hint(cx));
        }
        root = root.child(self.render_area_toolbar(crop, valid, cx));
        if crop.is_some() {
            root.child(self.render_area_controls(shown, valid, cx))
        } else {
            root
        }
    }

    fn render_crop_region(&self, crop: AreaRect, interactive: bool) -> impl IntoElement {
        let white = self.theme.fixed_white;
        let shade = edge_shade(&self.theme);
        let mut region = div()
            .absolute()
            .left(px(crop.x))
            .top(px(crop.y))
            .w(px(crop.width))
            .h(px(crop.height))
            .border_2()
            .border_color(white)
            .when(matches!(self.drag, Some(AreaDrag::Move { .. })), |this| {
                this.bg(Theme::with_alpha(white, 0.06))
            })
            .when(interactive, |this| this.cursor(gpui::CursorStyle::OpenHand))
            .child(
                div()
                    .absolute()
                    .left(px(-3.))
                    .top(px(-3.))
                    .w(px(crop.width + 2.))
                    .h(px(crop.height + 2.))
                    .border_1()
                    .border_color(shade),
            );

        for handle in AreaHandle::ALL.into_iter().filter(|_| interactive) {
            let mut zone = div().id(handle.id()).absolute().cursor(handle.cursor());
            let reach = px(AREA_HANDLE_GRAB * 2.);
            let grip = |width: f32, height: f32| {
                div()
                    .flex_shrink_0()
                    .w(px(width + 2.))
                    .h(px(height + 2.))
                    .p(px(1.))
                    .rounded(px(3.))
                    .bg(shade)
                    .child(div().size_full().rounded(px(2.)).bg(white))
            };

            zone = if handle.is_corner() {
                let zone = zone.size(reach).child(
                    grip(10., 10.)
                        .absolute()
                        .when(handle.west(), |this| this.left(px(5.)))
                        .when(handle.east(), |this| this.right(px(5.)))
                        .when(handle.north(), |this| this.top(px(5.)))
                        .when(handle.south(), |this| this.bottom(px(5.))),
                );
                let zone = if handle.west() {
                    zone.left(px(-AREA_HANDLE_GRAB))
                } else {
                    zone.right(px(-AREA_HANDLE_GRAB))
                };
                if handle.north() {
                    zone.top(px(-AREA_HANDLE_GRAB))
                } else {
                    zone.bottom(px(-AREA_HANDLE_GRAB))
                }
            } else if handle.north() || handle.south() {
                let zone = zone
                    .left(px(AREA_HANDLE_GRAB))
                    .right(px(AREA_HANDLE_GRAB))
                    .h(reach)
                    .flex()
                    .justify_center()
                    .child(grip(20., 4.).mt(px(if handle.north() { 8. } else { 10. })));
                if handle.north() {
                    zone.top(px(-AREA_HANDLE_GRAB))
                } else {
                    zone.bottom(px(-AREA_HANDLE_GRAB))
                }
            } else {
                let zone = zone
                    .top(px(AREA_HANDLE_GRAB))
                    .bottom(px(AREA_HANDLE_GRAB))
                    .w(reach)
                    .flex()
                    .items_center()
                    .child(grip(4., 20.).ml(px(if handle.west() { 8. } else { 10. })));
                if handle.west() {
                    zone.left(px(-AREA_HANDLE_GRAB))
                } else {
                    zone.right(px(-AREA_HANDLE_GRAB))
                }
            };

            region = region.child(zone);
        }

        region
    }

    fn render_snap_guides(&self) -> Vec<gpui::AnyElement> {
        let white: Hsla = self.theme.fixed_white.into();
        let shade = edge_shade(&self.theme);
        let (width, height) = self.display_size;
        self.snap_guides
            .iter()
            .map(|guide| {
                let line = |thickness: f32, color: Hsla| match guide.axis {
                    GuideAxis::V => div()
                        .absolute()
                        .left(px(guide.pos as f32 * width - (thickness - 1.) / 2.))
                        .top(px(guide.start as f32 * height))
                        .w(px(thickness))
                        .h(px((guide.end - guide.start) as f32 * height))
                        .bg(color),
                    GuideAxis::H => div()
                        .absolute()
                        .top(px(guide.pos as f32 * height - (thickness - 1.) / 2.))
                        .left(px(guide.start as f32 * width))
                        .h(px(thickness))
                        .w(px((guide.end - guide.start) as f32 * width))
                        .bg(color),
                };
                div()
                    .absolute()
                    .inset_0()
                    .child(line(3., shade))
                    .child(line(1., white))
                    .into_any_element()
            })
            .collect()
    }

    fn render_selection_hint(&self, cx: &App) -> impl IntoElement {
        let theme = self.theme;
        let screenshot = self.select.read(cx).recording_mode == Mode::Screenshot;
        let key = |label: &'static str| crate::ui::KbdChip::row(&theme, label);
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .px(px(20.))
                    .py(px(16.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .text_color(theme.foreground)
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t!("Drag to select an area")),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(t!("Or click a window to fit it")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(key("ctrl"))
                            .child(key("A"))
                            .child(t!("Whole display"))
                            .child(key("escape"))
                            .child(if screenshot {
                                t!("Cancel")
                            } else {
                                t!("Close")
                            }),
                    ),
            )
    }

    /// The floating readout: `top-12 left-1/2 -translate-x-1/2` over the
    /// liquid-glass surface, `min-w-28 px-2 text-base tabular-nums`. The
    fn render_area_toolbar(
        &self,
        crop: Option<AreaRect>,
        valid: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        const WIDTH: f32 = 440.;
        let theme = self.theme;
        let locked = self.area_locked;
        let label = match crop.filter(|_| valid) {
            Some(crop) => self.area_readout(crop),
            None => t!("Draw an area").to_string(),
        };
        let ratio_label = match self.area_ratio {
            Some((width, height)) => format!("{width}:{height}"),
            None => t!("Free").to_string(),
        };
        let divider = || div().h(px(24.)).w(px(1.)).flex_shrink_0().bg(theme.gray_5);
        let icon_button =
            |id: &'static str, icon: &'static str, title: SharedString, key: &'static str| {
                div()
                    .id(id)
                    .size(px(36.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.))
                    .cursor_pointer()
                    .group(id)
                    .hover(move |style| style.bg(Theme::with_alpha(theme.gray_12, 0.08)))
                    .tooltip(move |_, cx| {
                        crate::ui::Tooltip::new(&theme, title.clone())
                            .keys([key])
                            .view(cx)
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        svg()
                            .path(icon)
                            .size(px(16.))
                            .text_color(theme.gray_11)
                            .group_hover(id, move |style| style.text_color(theme.gray_12)),
                    )
            };

        div()
            .absolute()
            .top(px(48.))
            .left(px((self.display_size.0 - WIDTH) / 2.))
            .w(px(WIDTH))
            .flex()
            .justify_center()
            .child(
                self.glass_surface()
                    .occlude()
                    .h(px(48.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .p(px(6.))
                    .text_color(theme.gray_12)
                    .child(
                        div()
                            .min_w(px(112.))
                            .px(px(8.))
                            .text_size(px(16.))
                            .font_weight(FontWeight::NORMAL)
                            .child(label),
                    )
                    .child(divider())
                    .child(
                        div()
                            .id("area-ratio")
                            .h(px(36.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .rounded(px(12.))
                            .cursor_pointer()
                            .text_size(px(13.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(theme.gray_11)
                            .when(locked, |button| button.opacity(0.4))
                            .hover(move |style| style.bg(Theme::with_alpha(theme.gray_12, 0.08)))
                            .tooltip(move |_, cx| {
                                crate::ui::Tooltip::new(&theme, t!("Aspect ratio")).view(cx)
                            })
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if !this.area_locked {
                                    this.open_aspect_menu(event.position(), cx);
                                }
                            }))
                            .child(ratio_label)
                            .child(
                                svg()
                                    .path("icons/chevron-down.svg")
                                    .size(px(12.))
                                    .text_color(theme.gray_11),
                            ),
                    )
                    .child(divider())
                    .child(
                        icon_button(
                            "area-clear",
                            "icons/rotate-ccw.svg",
                            SharedString::from(t!("Clear area")),
                            "R",
                        )
                        .when(crop.is_none(), |button| button.opacity(0.4))
                        .on_click(cx.listener(|this, _, _, cx| this.reset_area(cx))),
                    )
                    .child(
                        icon_button(
                            "area-fill",
                            "icons/maximize.svg",
                            SharedString::from(t!("Fill display")),
                            "F",
                        )
                        .when(locked, |button| button.opacity(0.4))
                        .on_click(cx.listener(|this, _, _, cx| this.fill_area(cx))),
                    )
                    .child(divider())
                    .child(
                        icon_button(
                            "area-lock",
                            if locked {
                                "icons/lock.svg"
                            } else {
                                "icons/lock-open.svg"
                            },
                            if locked {
                                SharedString::from(t!("Unlock area"))
                            } else {
                                SharedString::from(t!("Lock area"))
                            },
                            "L",
                        )
                        .when(locked, |button| {
                            button.bg(Theme::with_alpha(theme.gray_12, 0.12))
                        })
                        .when(!locked && !valid, |button| button.opacity(0.4))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_area_lock(cx))),
                    ),
            )
    }

    /// The controls cluster, placed against the crop the way `controlsStyle`
    /// places it: below when it fits, above when it does not, inside the crop
    /// otherwise.
    fn render_area_controls(
        &self,
        crop: Option<AreaRect>,
        valid: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let crop = crop.unwrap_or(AreaRect {
            x: 0.,
            y: 0.,
            width: 0.,
            height: 0.,
        });
        let min = area_min_size(self.select.read(cx).recording_mode);
        let available = self.controls_work_area.clamped(self.display_size);
        let cluster_height = CLUSTER_HEIGHT
            + if self.select.read(cx).recording_mode == Mode::Screenshot {
                0.
            } else {
                78.
            };
        let (x, y) = area_controls_position(crop, available, cluster_height);

        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(CLUSTER_WIDTH))
            .flex()
            .flex_col()
            .items_center()
            .when(valid, |this| {
                this.child(self.render_controls_cluster(self.target(cx), false, cx))
            })
            .when(!valid, |this| {
                // `Minimum size is 150 x 150` / `W x H is too small`.
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .items_center()
                        .p(px(10.))
                        .my(px(8.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(self.theme.red_4)
                        .bg(self.theme.red_2)
                        .text_size(px(14.))
                        .text_color(self.theme.gray_12)
                        .child(t!(
                            "Minimum size is {width} x {height}",
                            width = min as u32,
                            height = min as u32
                        ))
                        .child(div().text_size(px(11.)).child(t!(
                            "{width} x {height} is too small",
                            width = crop.width.round(),
                            height = crop.height.round()
                        ))),
                )
            })
    }

    fn glass_surface(&self) -> gpui::Div {
        let theme = self.theme;
        div()
            .rounded(px(16.))
            .border_1()
            .border_color(if theme.is_dark() {
                gpui::hsla(0., 0., 1., 0.1)
            } else {
                Theme::with_alpha(theme.gray_12, 0.1)
            })
            .bg(if theme.is_dark() {
                Theme::with_alpha(theme.gray_2, 0.82)
            } else {
                Theme::with_alpha(theme.gray_1, 0.82)
            })
    }

    fn render_controls_cluster(
        &self,
        target: Option<ScreenCaptureTarget>,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let mode = self.select.read(cx).recording_mode;
        let disabled = disabled || target.is_none();

        // `flex flex-col gap-2.5 items-stretch my-2.5 w-104`.
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .items_stretch()
            .my(px(10.))
            .w(px(CLUSTER_WIDTH))
            // The cluster sits on top of the area overlay's drawing surface;
            // without this a click on Start would also start a new crop.
            .occlude()
            .child(
                self.glass_surface().p(px(12.)).child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(10.))
                        .items_center()
                        .child(self.render_close_button(cx))
                        .child(self.render_start_button(target, disabled, mode, cx))
                        .child(
                            div()
                                .id("overlay-countdown")
                                .flex()
                                .justify_center()
                                .items_center()
                                .size(px(36.))
                                .flex_shrink_0()
                                .rounded_full()
                                .border_1()
                                .border_color(theme.gray_5)
                                .bg(theme.gray_6)
                                .child(
                                    svg()
                                        .path("icons/settings.svg")
                                        .size(px(20.))
                                        .text_color(theme.gray_12),
                                )
                                .hover(|style| style.opacity(0.8))
                                .tooltip(move |_, cx| {
                                    crate::ui::Tooltip::new(&theme, t!("Recording countdown"))
                                        .view(cx)
                                })
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    let current = crate::store::GeneralSettings::load()
                                        .recording_countdown
                                        .unwrap_or(0);
                                    let items = COUNTDOWN_OPTIONS
                                        .iter()
                                        .map(|(value, label)| {
                                            crate::ui::MenuItem::new(tr(label), *value == current)
                                        })
                                        .collect::<Vec<_>>();
                                    this.mode_menu = None;
                                    this.device_menu = None;
                                    this.countdown_menu =
                                        Some(crate::ui::MenuState::new(event.position(), &items));
                                    cx.notify();
                                })),
                        ),
                ),
            )
            .when(mode != Mode::Screenshot, |cluster| {
                cluster.child(
                    self.glass_surface().p(px(12.)).child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .w_full()
                            .child(self.render_device_control(true, cx))
                            .child(self.render_device_control(false, cx)),
                    ),
                )
            })
    }

    /// `size-9 rounded-full bg-gray-12` with the inverted X glyph.
    fn render_close_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;

        div()
            .id("overlay-close")
            .flex()
            .justify_center()
            .items_center()
            .size(px(36.))
            .flex_shrink_0()
            .rounded_full()
            .bg(theme.gray_12)
            .hover(|style| style.opacity(0.8))
            .child(
                svg()
                    .path("icons/x.svg")
                    // `invert dark:invert-0` on a black glyph: white on the
                    // light theme's dark circle, black on the dark theme's
                    // light one.
                    .size(px(14.))
                    .text_color(theme.gray_1),
            )
            .on_click(cx.listener(|this, _, _window, cx| {
                cx.stop_propagation();
                this.dismiss(cx);
            }))
    }

    fn render_start_button(
        &self,
        target: Option<ScreenCaptureTarget>,
        disabled: bool,
        mode: Mode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let label = match mode {
            Mode::Screenshot => t!("Take Screenshot"),
            _ => t!("Start Recording"),
        };
        let icon = match mode {
            Mode::Instant => "icons/zap.svg",
            Mode::Studio => "icons/clapperboard.svg",
            Mode::Screenshot => "icons/image.svg",
        };
        let mode_label = match mode {
            Mode::Instant => t!("Instant Mode — Record straight to a video file"),
            Mode::Studio => t!("Studio Mode — Record and edit locally"),
            Mode::Screenshot => t!("Screenshot Mode"),
        };
        let _ = target;

        div()
            .id("overlay-start")
            .flex()
            .flex_1()
            .min_w_0()
            .max_w(px(288.))
            .overflow_hidden()
            .flex_row()
            .h(px(44.))
            .rounded_full()
            .text_color(gpui::white())
            .bg(theme.primary)
            .child(
                div()
                    .id("overlay-start-action")
                    .flex()
                    .flex_1()
                    .items_center()
                    .py(px(4.))
                    .pl(px(16.))
                    .min_w_0()
                    // The pill's `overflow-hidden rounded-full` clips these
                    // fills in the Tauri app; gpui masks are rectangles, so
                    // the halves round their own outer caps (h-11 -> 22px) or
                    // they paint square corners past the pill onto the glass
                    // card behind it.
                    .rounded_l(px(22.))
                    // `opacity-60 cursor-not-allowed hover:bg-transparent`.
                    .when(disabled, |this| this.opacity(0.6))
                    .when(!disabled, |this| {
                        this.hover(|style| style.bg(gpui::hsla(0., 0., 1., 0.1)))
                    })
                    .tooltip(move |_, cx| crate::ui::Tooltip::new(&theme, label).view(cx))
                    .child(
                        svg()
                            .path(icon)
                            .size(px(16.))
                            .flex_shrink_0()
                            .text_color(gpui::white()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .ml(px(12.))
                            .mr(px(8.))
                            .min_w_0()
                            .child(
                                div()
                                    // `text-[0.95rem] font-medium`.
                                    .text_size(px(15.2))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .mt(px(-2.))
                                    .text_size(px(11.))
                                    .whitespace_nowrap()
                                    .truncate()
                                    // `text-[11px] ... text-white/90
                                    //  font-light` (`t-s-overlay.tsx:2200`).
                                    // `font-light` is 300 and no 300 face is
                                    // loaded (`ui-solid/vite.js:31-33` ships
                                    // 400/500/700), so CSS font-matching
                                    // resolves it to 400 -- Regular, not Light
                                    // -- and it must opt out of the `body`
                                    // Medium default explicitly.
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(gpui::hsla(0., 0., 1., 0.9))
                                    .child(mode_label),
                            ),
                    ),
            )
            .child(
                // `pl-2.5 pr-3 py-1.5 border-l border-white/20 bg-white/5`,
                // `onMouseDown/onClick={showMenu(menuModes(), e)}`: the caret
                // pops the mode menu at the press location and must never arm
                // the pill's start click -- gpui arms clicks on mouse-down per
                // hitbox, so the stop has to happen here, not on a click
                // handler. Active even while start is disabled, same as the
                // source (the disabled styling only wraps the leading half).
                div()
                    .id("overlay-mode-picker")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                            cx.stop_propagation();
                            if crate::session::RecordingSession::global(cx)
                                .read(cx)
                                .editor_recording_target()
                                .is_some()
                            {
                                return;
                            }
                            let current = this.select.read(cx).recording_mode;
                            this.device_menu = None;
                            this.countdown_menu = None;
                            this.mode_menu = Some(crate::ui::MenuState::new(
                                event.position,
                                &mode_menu_items(current),
                            ));
                            cx.notify();
                        }),
                    )
                    .flex()
                    .items_center()
                    .pl(px(10.))
                    .pr(px(12.))
                    .py(px(6.))
                    // The right cap of the pill, for the same reason the
                    // leading half takes `rounded_l`: its `bg-white/5` panel
                    // otherwise pokes square corners past `rounded-full`.
                    .rounded_r(px(22.))
                    .border_l_1()
                    .border_color(gpui::hsla(0., 0., 1., 0.2))
                    .bg(gpui::hsla(0., 0., 1., 0.05))
                    .tooltip(move |_, cx| {
                        crate::ui::Tooltip::new(&theme, t!("Choose recording mode")).view(cx)
                    })
                    .child(
                        svg()
                            .path("icons/chevron-down.svg")
                            .size(px(14.))
                            .text_color(gpui::white()),
                    ),
            )
            .when(!disabled, |this| {
                this.on_click(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.start_recording(window, cx);
                }))
            })
    }
}

/// Area interaction: which part of the selection a point grabs.
enum AreaZone {
    Handle(AreaHandle),
    Inside,
}

impl OverlayWindow {
    fn area_zone_at(&self, point: (f32, f32)) -> Option<AreaZone> {
        let crop = self.crop?;
        let (x, y) = point;
        let near_left = (x - crop.x).abs() <= AREA_HANDLE_GRAB;
        let near_right = (x - crop.right()).abs() <= AREA_HANDLE_GRAB;
        let near_top = (y - crop.y).abs() <= AREA_HANDLE_GRAB;
        let near_bottom = (y - crop.bottom()).abs() <= AREA_HANDLE_GRAB;
        let within_x = x >= crop.x - AREA_HANDLE_GRAB && x <= crop.right() + AREA_HANDLE_GRAB;
        let within_y = y >= crop.y - AREA_HANDLE_GRAB && y <= crop.bottom() + AREA_HANDLE_GRAB;

        let handle = match (near_left, near_right, near_top, near_bottom) {
            (true, _, true, _) => Some(AreaHandle::NorthWest),
            (_, true, true, _) => Some(AreaHandle::NorthEast),
            (true, _, _, true) => Some(AreaHandle::SouthWest),
            (_, true, _, true) => Some(AreaHandle::SouthEast),
            (true, _, _, _) if within_y => Some(AreaHandle::West),
            (_, true, _, _) if within_y => Some(AreaHandle::East),
            (_, _, true, _) if within_x => Some(AreaHandle::North),
            (_, _, _, true) if within_x => Some(AreaHandle::South),
            _ => None,
        };

        if let Some(handle) = handle {
            return Some(AreaZone::Handle(handle));
        }
        (x > crop.x && x < crop.right() && y > crop.y && y < crop.bottom())
            .then_some(AreaZone::Inside)
    }

    fn area_mouse_down(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.recording_area {
            return;
        }
        if self.area_locked {
            self.aspect_menu = None;
            cx.notify();
            return;
        }
        let point = (f32::from(position.x), f32::from(position.y));
        self.snap_guides.clear();
        self.fit_hover = None;
        if let (Some(crop), Some((from, started))) = (self.crop, self.reshape.take()) {
            self.crop = Some(area_between(
                from,
                crop,
                started.elapsed().as_secs_f32() / AREA_RESHAPE.as_secs_f32(),
            ));
        }
        self.aspect_menu = None;

        self.drag = match self.area_zone_at(point) {
            Some(AreaZone::Handle(handle)) => Some(AreaDrag::Resize {
                handle,
                start: self.crop.unwrap_or(AreaRect {
                    x: point.0,
                    y: point.1,
                    width: 0.,
                    height: 0.,
                }),
            }),
            Some(AreaZone::Inside) => Some(AreaDrag::Move {
                grab: point,
                start: self.crop.unwrap(),
            }),
            None => {
                let previous = self.crop.replace(AreaRect {
                    x: point.0,
                    y: point.1,
                    width: 0.,
                    height: 0.,
                });
                Some(AreaDrag::Draw {
                    anchor: point,
                    previous,
                })
            }
        };
        self.sync_area_camera(cx);
        cx.notify();
    }

    fn area_mouse_move(
        &mut self,
        position: Point<Pixels>,
        free: bool,
        keep: bool,
        cx: &mut Context<Self>,
    ) {
        if self.recording_area {
            return;
        }
        let Some(drag) = &self.drag else {
            let hover = fit_candidate(
                &self.area_windows,
                (f32::from(position.x), f32::from(position.y)),
                self.crop,
            )
            .filter(|_| !self.area_locked);
            if hover != self.fit_hover {
                self.fit_hover = hover;
                cx.notify();
            }
            return;
        };
        let x = f32::from(position.x).clamp(0., self.display_size.0);
        let y = f32::from(position.y).clamp(0., self.display_size.1);

        let start = match drag {
            AreaDrag::Resize { start, .. } => Some(*start),
            _ => None,
        };
        let ratio = drag_ratio(self.area_ratio.map(ratio_value), keep, start);
        let (next, guides) = match (drag, ratio) {
            (AreaDrag::Draw { anchor, .. }, Some(ratio)) => (
                area_from_anchor_with_ratio(*anchor, (x, y), ratio, self.display_size),
                Vec::new(),
            ),
            (AreaDrag::Resize { handle, start }, Some(ratio)) => (
                area_resize_with_ratio(*handle, *start, (x, y), ratio, self.display_size),
                Vec::new(),
            ),
            (AreaDrag::Draw { anchor, .. }, None) => {
                let ((x, y), guides) = if free {
                    ((x, y), Vec::new())
                } else {
                    snap_area_point((x, y), self.display_size, &self.area_snap)
                };
                (
                    AreaRect {
                        x: anchor.0.min(x),
                        y: anchor.1.min(y),
                        width: (x - anchor.0).abs(),
                        height: (y - anchor.1).abs(),
                    },
                    guides,
                )
            }
            (AreaDrag::Move { grab, start }, _) => {
                let moved = AreaRect {
                    x: start.x + (x - grab.0),
                    y: start.y + (y - grab.1),
                    ..*start
                };
                if free {
                    (moved.clamped(self.display_size), Vec::new())
                } else {
                    snap_area_rect(moved, self.display_size, &self.area_snap)
                }
            }
            (AreaDrag::Resize { handle, start }, None) => {
                let ((x, y), guides) = if free {
                    ((x, y), Vec::new())
                } else {
                    snap_resize_point(
                        (x, y),
                        self.display_size,
                        &self.area_snap,
                        handle.east() || handle.west(),
                        handle.north() || handle.south(),
                    )
                };
                let mut left = start.x;
                let mut top = start.y;
                let mut right = start.right();
                let mut bottom = start.bottom();
                if handle.west() {
                    left = x;
                }
                if handle.east() {
                    right = x;
                }
                if handle.north() {
                    top = y;
                }
                if handle.south() {
                    bottom = y;
                }
                (
                    AreaRect {
                        x: left.min(right),
                        y: top.min(bottom),
                        width: (right - left).abs(),
                        height: (bottom - top).abs(),
                    },
                    guides,
                )
            }
        };

        self.crop = Some(next);
        self.snap_guides = guides;
        self.sync_area_camera(cx);
        cx.notify();
    }

    fn area_mouse_up(
        &mut self,
        position: Point<Pixels>,
        free: bool,
        keep: bool,
        cx: &mut Context<Self>,
    ) {
        if self.recording_area {
            return;
        }
        self.area_mouse_move(position, free, keep, cx);
        let Some(drag) = self.drag.take() else {
            return;
        };
        self.snap_guides.clear();
        let clicked = match drag {
            AreaDrag::Draw { anchor, .. } => {
                (f32::from(position.x) - anchor.0).abs() < 3.
                    && (f32::from(position.y) - anchor.1).abs() < 3.
            }
            _ => false,
        };
        if let Some(crop) = self.crop {
            if clicked || crop.width < 2. || crop.height < 2. {
                self.crop = None;
                if clicked {
                    self.fit_area_to_window_at((f32::from(position.x), f32::from(position.y)), cx);
                }
            } else {
                self.crop = Some(crop.clamped(self.display_size));
            }
        }
        self.sync_area_camera(cx);
        if matches!(drag, AreaDrag::Draw { .. })
            && !clicked
            && self.select.read(cx).recording_mode == Mode::Screenshot
            && let Some(target) = self.target(cx)
        {
            cx.defer(move |cx: &mut App| crate::screenshot::take_screenshot(target, cx));
        }
        cx.notify();
    }

    pub fn cancel_area_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        self.crop = match drag {
            AreaDrag::Draw { previous, .. } => previous,
            AreaDrag::Move { start, .. } | AreaDrag::Resize { start, .. } => Some(start),
        };
        self.snap_guides.clear();
        self.sync_area_camera(cx);
        cx.notify();
        true
    }

    fn unlock_area(&mut self) {
        self.area_locked = false;
        if stored_locked_area().is_some_and(|(display, _)| display == self.display_id.to_string()) {
            store_locked_area(None);
        }
    }

    fn toggle_area_lock(&mut self, cx: &mut Context<Self>) {
        if self.area_locked {
            self.unlock_area();
        } else {
            let min = area_min_size(self.select.read(cx).recording_mode);
            let Some(crop) = self.crop.filter(|crop| crop.is_valid_for(min)) else {
                return;
            };
            self.area_locked = true;
            self.drag = None;
            self.snap_guides.clear();
            store_locked_area(Some((&self.display_id, crop)));
            let display = self.display_id.clone();
            cx.defer(move |cx| app_windows::release_other_area_locks(&display, cx));
        }
        cx.notify();
    }

    pub fn release_area_lock(&mut self, cx: &mut Context<Self>) {
        if self.area_locked {
            self.area_locked = false;
            cx.notify();
        }
    }

    fn persist_locked_area(&self) {
        let Some(crop) = self.crop.filter(|_| self.area_locked) else {
            return;
        };
        if stored_locked_area().is_some_and(|(display, _)| display == self.display_id.to_string()) {
            store_locked_area(Some((&self.display_id, crop)));
        }
    }

    fn load_area_windows(&mut self, cx: &mut Context<Self>) {
        self.area_windows_requested = true;
        let display = self.display_id.clone();
        cx.spawn(async move |this, cx| {
            let windows = cx
                .background_executor()
                .spawn(async move {
                    crate::devices::list_window_targets()
                        .into_iter()
                        .filter_map(|(_, window)| HoveredWindow::from_window(&window))
                        .filter(|window| window.display_id == display)
                        .map(|window| window.bounds)
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, _| {
                this.area_snap =
                    area_snap_rects(this.controls_work_area, &windows, this.display_size);
                this.area_windows = windows
                    .iter()
                    .filter_map(|window| window.intersected(this.display_size))
                    .collect();
            })
            .ok();
        })
        .detach();
    }

    fn fit_area_to_window_at(&mut self, point: (f32, f32), cx: &mut Context<Self>) {
        let Some(window) = crate::devices::topmost_foreign_window_at_cursor()
            .as_ref()
            .and_then(HoveredWindow::from_window)
            .filter(|window| window.display_id == self.display_id)
        else {
            return;
        };
        let Some(mut bounds) = window.bounds.intersected(self.display_size) else {
            return;
        };
        if let Some(ratio) = self.area_ratio {
            bounds = area_with_ratio(bounds, ratio_value(ratio), self.display_size);
        }
        if !bounds.is_valid_for(area_min_size(self.select.read(cx).recording_mode)) {
            return;
        }
        self.crop = Some(AreaRect {
            x: point.0,
            y: point.1,
            width: 0.,
            height: 0.,
        });
        self.reshape_area(bounds, cx);
    }

    fn area_key(
        &mut self,
        keystroke: &gpui::Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.recording_area || self.select.read(cx).mode != Some(TargetType::Area) {
            return false;
        }
        let key = keystroke.key.as_str();
        if key == "escape" && self.cancel_area_drag(cx) {
            return true;
        }
        if key == "enter" && self.drag.is_none() && self.target(cx).is_some() {
            self.start_recording(window, cx);
            return true;
        }
        let modifiers = &keystroke.modifiers;
        let command = modifiers.control || modifiers.platform;
        if self.drag.is_none() {
            match (key, command, modifiers.shift || modifiers.alt) {
                ("a", true, false) | ("f", false, false) => {
                    self.fill_area(cx);
                    return true;
                }
                ("r", false, false) => {
                    self.reset_area(cx);
                    return true;
                }
                ("l", false, false) => {
                    if self.crop.is_some() {
                        self.toggle_area_lock(cx);
                    }
                    return true;
                }
                _ => {}
            }
        }
        let step = if modifiers.shift { 10. } else { 1. };
        let (dx, dy) = match key {
            "left" => (-step, 0.),
            "right" => (step, 0.),
            "up" => (0., -step),
            "down" => (0., step),
            _ => return false,
        };
        let Some(crop) = self
            .crop
            .filter(|_| !self.area_locked && self.drag.is_none())
        else {
            return false;
        };
        self.reshape = None;
        self.crop = Some(if command {
            area_resized_by(
                crop,
                (dx, dy),
                area_min_size(self.select.read(cx).recording_mode),
                self.display_size,
            )
        } else {
            AreaRect {
                x: crop.x + dx,
                y: crop.y + dy,
                ..crop
            }
            .clamped(self.display_size)
        });
        self.persist_locked_area();
        self.sync_area_camera(cx);
        cx.notify();
        true
    }

    fn open_aspect_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let items = self.aspect_menu_items();
        self.mode_menu = None;
        self.countdown_menu = None;
        self.device_menu = None;
        self.aspect_menu = Some(crate::ui::MenuState::new(position, &items));
        cx.notify();
    }

    fn aspect_menu_items(&self) -> Vec<crate::ui::MenuItem> {
        let ratios = AREA_RATIOS.iter().map(|ratio| {
            let label = match ratio {
                Some((width, height)) => format!("{width}:{height}"),
                None => t!("Free").to_string(),
            };
            crate::ui::MenuItem::new(label, self.area_ratio == *ratio)
        });
        let scale = self.pixels_per_point();
        let recorded = self.crop.map(|crop| {
            (
                (crop.width * scale).round() as u32,
                (crop.height * scale).round() as u32,
            )
        });
        let sizes = self.area_sizes().map(|(name, width, height, _)| {
            crate::ui::MenuItem::new(
                format!("{name}  ·  {width} × {height}"),
                recorded == Some((width, height)),
            )
        });
        ratios.chain(sizes).collect()
    }

    fn area_sizes(&self) -> impl Iterator<Item = (&'static str, u32, u32, AreaRect)> + '_ {
        let scale = self.pixels_per_point();
        crate::window_arrange::SIZES
            .into_iter()
            .filter_map(move |(name, width, height)| {
                area_exact_size(self.crop, (width, height), scale, self.display_size)
                    .map(|area| (name, width, height, area))
            })
    }

    fn render_aspect_menu(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let state = self.aspect_menu.as_ref()?;
        Some(
            crate::ui::Menu::plain(
                &self.theme,
                "overlay-aspect-menu",
                self.aspect_menu_items(),
                state,
            )
            .on_select(cx.listener(|this, index: &usize, _, cx| {
                this.choose_aspect(*index, cx);
            }))
            .on_dismiss(cx.listener(|this, _, _, cx| {
                this.aspect_menu = None;
                cx.notify();
            }))
            .into_any_element(),
        )
    }

    fn choose_aspect(&mut self, index: usize, cx: &mut Context<Self>) {
        self.aspect_menu = None;
        if let Some(ratio) = AREA_RATIOS.get(index) {
            self.set_area_ratio(*ratio, cx);
            return;
        }
        let size = self.area_sizes().nth(index - AREA_RATIOS.len());
        match size.filter(|_| !self.area_locked) {
            Some((_, _, _, area)) => {
                self.area_ratio = None;
                store_area_ratio(None);
                self.reshape_area(area, cx);
            }
            None => cx.notify(),
        }
    }

    fn displayed_crop(&mut self, window: &Window) -> Option<AreaRect> {
        let crop = self.crop?;
        let Some((from, started)) = self.reshape else {
            return Some(crop);
        };
        let t = started.elapsed().as_secs_f32() / AREA_RESHAPE.as_secs_f32();
        if t >= 1. {
            self.reshape = None;
            return Some(crop);
        }
        window.request_animation_frame();
        Some(area_between(from, crop, t))
    }

    fn reshape_area(&mut self, to: AreaRect, cx: &mut Context<Self>) {
        let from = match (self.crop, self.reshape) {
            (Some(crop), Some((from, started))) => area_between(
                from,
                crop,
                started.elapsed().as_secs_f32() / AREA_RESHAPE.as_secs_f32(),
            ),
            (Some(crop), None) => crop,
            (None, _) => area_fill(self.display_size, None),
        };
        self.reshape = Some((from, Instant::now()));
        self.crop = Some(to.clamped(self.display_size));
        self.persist_locked_area();
        self.sync_area_camera(cx);
        cx.notify();
    }

    fn fill_area(&mut self, cx: &mut Context<Self>) {
        if self.area_locked {
            return;
        }
        let to = area_fill(self.display_size, self.area_ratio.map(ratio_value));
        self.reshape_area(to, cx);
    }

    fn reset_area(&mut self, cx: &mut Context<Self>) {
        if self.area_locked {
            self.unlock_area();
        }
        self.reshape = None;
        self.crop = None;
        self.sync_area_camera(cx);
        cx.notify();
    }

    fn set_area_ratio(&mut self, ratio: Option<(u32, u32)>, cx: &mut Context<Self>) {
        if self.area_locked {
            return;
        }
        self.area_ratio = ratio;
        store_area_ratio(ratio);
        if let (Some(ratio), Some(crop)) = (ratio, self.crop) {
            self.reshape_area(
                area_with_ratio(crop, ratio_value(ratio), self.display_size),
                cx,
            );
        } else {
            cx.notify();
        }
    }

    fn pixels_per_point(&self) -> f32 {
        self.physical_size
            .map(|(width, _)| width as f32 / self.display_size.0.max(1.))
            .unwrap_or(1.)
    }

    fn area_readout(&self, crop: AreaRect) -> String {
        let scale = self.pixels_per_point();
        format!(
            "{} × {}",
            (crop.width * scale).round(),
            (crop.height * scale).round()
        )
    }

    fn render_locked_badge(&self, crop: AreaRect, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        overlay_badge(&theme)
            .id("area-locked-badge")
            .absolute()
            .left(px(crop.x + 8.))
            .top(px(crop.y + 8.))
            .cursor_pointer()
            .tooltip(move |_, cx| {
                crate::ui::Tooltip::new(&theme, SharedString::from(t!("Unlock area")))
                    .keys(["L"])
                    .view(cx)
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, _, cx| this.toggle_area_lock(cx)))
            .child(
                svg()
                    .path("icons/lock.svg")
                    .size(px(12.))
                    .text_color(theme.fixed_white),
            )
            .child(t!("Locked"))
    }

    fn render_size_tag(&self, crop: AreaRect, valid: bool, min: f32) -> impl IntoElement {
        let label = if valid {
            let mut label = self.area_readout(crop);
            if let Some((width, height)) = self.area_ratio {
                label.push_str(&format!(" · {width}:{height}"));
            }
            label
        } else {
            let least = (min * self.pixels_per_point()).round();
            t!(
                "Too small. At least {width} × {height}",
                width = least,
                height = least
            )
            .to_string()
        };
        let badge = overlay_badge(&self.theme).absolute().when(!valid, |this| {
            this.bg(self.theme.danger)
                .text_color(self.theme.danger_foreground)
        });
        if self.drag.is_some() {
            let below = crop.bottom() + 32. <= self.display_size.1;
            badge
                .right(px(self.display_size.0 - crop.right()))
                .top(px(if below {
                    crop.bottom() + 8.
                } else {
                    crop.bottom() - 32.
                }))
                .child(if valid {
                    self.area_readout(crop)
                } else {
                    label
                })
        } else {
            let (x, y) = tag_position(crop);
            badge.left(px(x)).top(px(y)).child(label)
        }
    }
}

impl OverlayWindow {
    fn active_window_logical(&self, cx: &App) -> Option<AreaRect> {
        self.select
            .read(cx)
            .active_window()
            .filter(|window| window.display_id == self.display_id)
            .map(|window| window.bounds)
    }

    fn window_menu_entries(&self, window: AreaRect) -> Vec<(String, bool, AreaRect)> {
        use crate::window_arrange::{RATIOS, SIZES, fit_to_ratio, fit_to_size, matching_ratio};
        let work_area = self.controls_work_area;
        let scale = self.pixels_per_point();
        let current = matching_ratio(window);
        let recorded = (
            (window.width * scale).round() as u32,
            (window.height * scale).round() as u32,
        );
        let mut entries: Vec<(String, bool, AreaRect)> = RATIOS
            .into_iter()
            .map(|ratio| {
                (
                    format!("{}:{}", ratio.0, ratio.1),
                    current == Some(ratio),
                    fit_to_ratio(window, ratio, work_area),
                )
            })
            .collect();
        entries.extend(SIZES.into_iter().filter_map(|(name, width, height)| {
            fit_to_size(window, (width, height), scale, work_area).map(|frame| {
                (
                    format!("{name}  ·  {width} × {height}"),
                    recorded == (width, height),
                    frame,
                )
            })
        }));
        entries
    }

    fn open_window_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(window) = self.active_window_logical(cx) else {
            return;
        };
        if let Some(active) = self.select.read(cx).active_window().cloned() {
            self.select
                .update(cx, |select, cx| select.set_pinned(active, cx));
        }
        let items: Vec<crate::ui::MenuItem> = self
            .window_menu_entries(window)
            .into_iter()
            .map(|(label, checked, _)| crate::ui::MenuItem::new(label, checked))
            .collect();
        self.mode_menu = None;
        self.countdown_menu = None;
        self.device_menu = None;
        self.aspect_menu = None;
        self.window_menu = Some(crate::ui::MenuState::new(position, &items));
        cx.notify();
    }

    fn render_window_menu(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let state = self.window_menu.as_ref()?;
        let window = self.active_window_logical(cx)?;
        let items = self
            .window_menu_entries(window)
            .into_iter()
            .map(|(label, checked, _)| crate::ui::MenuItem::new(label, checked))
            .collect();
        Some(
            crate::ui::Menu::plain(&self.theme, "overlay-window-menu", items, state)
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.choose_window_size(*index, cx);
                }))
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.window_menu = None;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    fn choose_window_size(&mut self, index: usize, cx: &mut Context<Self>) {
        self.window_menu = None;
        let frame = self
            .active_window_logical(cx)
            .and_then(|window| self.window_menu_entries(window).into_iter().nth(index))
            .map(|(_, _, frame)| frame);
        match frame {
            Some(frame) => self.arrange_active_window(Arrangement::Frame(frame), false, cx),
            None => cx.notify(),
        }
    }

    fn set_focus(&mut self, focus: bool, cx: &mut Context<Self>) {
        self.select.update(cx, |select, cx| {
            select.focus_window = focus;
            cx.notify();
        });
        let style = if focus {
            crate::store::WindowCaptureStyle::FocusedWindow
        } else {
            crate::store::WindowCaptureStyle::ScreenRegion
        };
        if !crate::store::set_store_setting(
            crate::store::GENERAL_SETTINGS,
            "windowCaptureStyle",
            serde_json::Value::from(crate::store::SettingsEnum::as_json(style)),
        ) {
            tracing::warn!("could not save the window capture style");
        }
        let Some(window) = self.active_window_logical(cx) else {
            cx.notify();
            return;
        };
        if focus {
            if self.select.read(cx).focus_center {
                let centred = crate::window_arrange::centre(window, self.controls_work_area);
                self.arrange_active_window(Arrangement::Frame(centred), true, cx);
            } else {
                cx.notify();
            }
        } else {
            self.put_active_window_back(cx);
        }
    }

    fn set_focus_center(&mut self, center: bool, cx: &mut Context<Self>) {
        self.select.update(cx, |select, cx| {
            select.focus_center = center;
            cx.notify();
        });
        if !crate::store::set_store_setting(
            crate::store::GENERAL_SETTINGS,
            "focusCenter",
            serde_json::Value::Bool(center),
        ) {
            tracing::warn!("could not save Focus centering");
        }
        if !self.select.read(cx).focus_window {
            return;
        }
        let Some(window) = self.active_window_logical(cx) else {
            return;
        };
        if center {
            let centred = crate::window_arrange::centre(window, self.controls_work_area);
            self.arrange_active_window(Arrangement::Frame(centred), true, cx);
        } else {
            self.put_active_window_back(cx);
        }
    }

    fn put_active_window_back(&mut self, cx: &mut Context<Self>) {
        let original = self
            .select
            .read(cx)
            .active_window()
            .and_then(|active| self.select.read(cx).arranged_original(&active.id).cloned());
        match original {
            Some(original) => self.arrange_active_window(Arrangement::PutBack(original), false, cx),
            None => cx.notify(),
        }
    }

    fn arrange_active_window(
        &mut self,
        arrangement: Arrangement,
        raise: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(window) = self.select.read(cx).active_window().cloned() else {
            return;
        };
        let from = self.window_drawn.get().unwrap_or(window.bounds);
        self.select.update(cx, |select, cx| {
            select.remember_arranged(&window);
            select.set_pinned(window.clone(), cx);
        });
        let (id, display) = (window.id.clone(), window.display_id.clone());
        let before = window.bounds;
        let requested = id.clone();
        let select = self.select.clone();
        cx.spawn(async move |this, cx| {
            let moved = crate::foreign_window::in_order(move || {
                let arranged = match arrangement {
                    Arrangement::Frame(frame) => {
                        crate::foreign_window::arrange(&id, &display, frame)
                    }
                    Arrangement::PutBack(original) => crate::foreign_window::put_back(&original),
                };
                if raise {
                    crate::foreign_window::raise(&id);
                }
                let read = || {
                    scap_targets::Window::from_id(&id)
                        .as_ref()
                        .and_then(HoveredWindow::from_window)
                };
                let mut fresh = read();
                let deadline = Instant::now() + Duration::from_millis(500);
                while arranged
                    && fresh.as_ref().is_some_and(|fresh| fresh.bounds == before)
                    && Instant::now() < deadline
                {
                    std::thread::sleep(Duration::from_millis(16));
                    fresh = read();
                }
                (arranged, fresh)
            })
            .await
            .unwrap_or((false, None));
            let (arranged, fresh) = moved;
            if !arranged {
                tracing::warn!("the picked window could not be arranged");
            }
            let still_picked = this
                .update(cx, |_, cx| {
                    select
                        .read(cx)
                        .active_window()
                        .is_some_and(|window| window.id == requested)
                })
                .unwrap_or(false);
            if !still_picked {
                return;
            }
            if let Some(fresh) = fresh {
                select.update(cx, |select, cx| select.set_pinned(fresh, cx));
            }
            this.update(cx, |this, cx| {
                this.window_reshape = Some((from, Instant::now()));
                cx.notify();
            })
            .ok();
            cx.update(app_windows::refresh_target_overlays);
        })
        .detach();
    }

    fn render_window_size(&self, window: AreaRect, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let scale = self.pixels_per_point();
        let mut label = format!(
            "{} × {}",
            (window.width * scale).round(),
            (window.height * scale).round()
        );
        if let Some((w, h)) = crate::window_arrange::matching_ratio(window) {
            label.push_str(&format!("  ·  {w}:{h}"));
        }
        let arrangeable = crate::foreign_window::can_arrange();
        self.glass_surface()
            .id("window-size")
            .h(px(28.))
            .rounded_full()
            .mb(px(8.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .text_color(theme.gray_12)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(label)
            .when(arrangeable, |chip| {
                chip.cursor_pointer()
                    .hover(move |style| style.bg(Theme::with_alpha(theme.gray_12, 0.08)))
                    .tooltip(move |_, cx| {
                        crate::ui::Tooltip::new(&theme, t!("Window size")).view(cx)
                    })
                    .child(
                        svg()
                            .path("icons/chevron-down.svg")
                            .size(px(12.))
                            .text_color(theme.gray_12),
                    )
                    .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.open_window_menu(event.position(), cx);
                    }))
            })
    }

    fn render_focus_toggle(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !FOCUS_SUPPORTED || !crate::foreign_window::can_arrange() {
            return None;
        }
        let theme = self.theme;
        let select = self.select.read(cx);
        let (focus, center) = (select.focus_window, select.focus_center);
        let row = |id: &'static str, label: SharedString| {
            div()
                .id(id)
                .h(px(40.))
                .px(px(12.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.))
                .cursor_pointer()
                .text_size(px(13.))
                .text_color(theme.gray_12)
                .child(label)
        };
        Some(
            self.glass_surface()
                .mb(px(10.))
                .w(px(260.))
                .flex()
                .flex_col()
                .when(focus, |card| {
                    card.border_color(Hsla::from(theme.primary).opacity(0.7))
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    row(
                        "window-focus",
                        SharedString::from(t!("Record only this window")),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        let focus = this.select.read(cx).focus_window;
                        this.set_focus(!focus, cx);
                    }))
                    .child(crate::ui::Toggle::glass(
                        &theme,
                        "window-focus-toggle",
                        focus,
                    )),
                )
                .when(focus, |card| {
                    card.child(
                        row(
                            "window-focus-center",
                            SharedString::from(t!("Center on screen")),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            let center = this.select.read(cx).focus_center;
                            this.set_focus_center(!center, cx);
                        }))
                        .child(crate::ui::Toggle::glass(
                            &theme,
                            "window-focus-center-toggle",
                            center,
                        )),
                    )
                }),
        )
    }
}

fn overlay_badge(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(24.))
        .px(px(8.))
        .rounded(px(6.))
        .bg(theme.badge)
        .text_color(theme.fixed_white)
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
}

fn tag_position(target: AreaRect) -> (f32, f32) {
    if target.y >= 32. {
        (target.x, target.y - 30.)
    } else {
        (target.x + 8., target.y + 8.)
    }
}

fn selection_frame(target: AreaRect, theme: &Theme) -> impl IntoElement {
    div()
        .absolute()
        .left(px(target.x - 1.))
        .top(px(target.y - 1.))
        .w(px(target.width + 2.))
        .h(px(target.height + 2.))
        .border_1()
        .border_color(edge_shade(theme))
        .child(div().size_full().border_2().border_color(theme.fixed_white))
}

fn edge_shade(theme: &Theme) -> Hsla {
    Theme::with_alpha(theme.badge, 0.55)
}

fn scrim(target: Option<AreaRect>, display: (f32, f32), color: Hsla) -> Vec<gpui::AnyElement> {
    let piece = |x: f32, y: f32, width: f32, height: f32| {
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(width.max(0.)))
            .h(px(height.max(0.)))
            .bg(color)
            .into_any_element()
    };
    match target {
        None => vec![piece(0., 0., display.0, display.1)],
        Some(t) => vec![
            piece(0., 0., t.x, display.1),
            piece(t.right(), 0., display.0 - t.right(), display.1),
            piece(t.x, 0., t.width, t.y),
            piece(t.x, t.bottom(), t.width, display.1 - t.bottom()),
        ],
    }
}

fn corner_marks(target: AreaRect, theme: &Theme) -> Vec<gpui::AnyElement> {
    let white: Hsla = theme.fixed_white.into();
    let shade = edge_shade(theme);
    [(false, false), (true, false), (false, true), (true, true)]
        .into_iter()
        .map(|(right, bottom)| {
            let x = if right {
                target.right() - 12.
            } else {
                target.x - 2.
            };
            let y = if bottom {
                target.bottom() - 12.
            } else {
                target.y - 2.
            };
            let arm = |size: f32, inset: f32, color: Hsla| {
                div()
                    .absolute()
                    .left(px(inset))
                    .top(px(inset))
                    .size(px(size))
                    .border_color(color)
                    .when(right, |this| this.border_r_2())
                    .when(!right, |this| this.border_l_2())
                    .when(bottom, |this| this.border_b_2())
                    .when(!bottom, |this| this.border_t_2())
            };
            div()
                .absolute()
                .left(px(x - 3.))
                .top(px(y - 3.))
                .size(px(20.))
                .child(arm(20., 0., shade))
                .child(arm(14., 3., white))
                .into_any_element()
        })
        .collect()
}

fn focus_ring(window: AreaRect, theme: &Theme) -> impl IntoElement {
    const OUTSET: f32 = 4.;
    const WIDTH: f32 = 2.;
    const RADIUS: f32 = if cfg!(target_os = "macos") { 16. } else { 8. };
    let color = Hsla::from(theme.primary);
    let band = |outset: f32, alpha: f32| {
        div()
            .absolute()
            .left(px(window.x - outset))
            .top(px(window.y - outset))
            .w(px(window.width + 2. * outset))
            .h(px(window.height + 2. * outset))
            .rounded(px(RADIUS + outset))
            .border(px(WIDTH))
            .border_color(color.opacity(alpha))
    };
    div()
        .absolute()
        .size_full()
        .child(band(OUTSET + 2. * WIDTH, 0.1))
        .child(band(OUTSET + WIDTH, 0.28))
        .child(band(OUTSET, 0.9))
}

/// Ask for camera/mic access from the overlay. The macOS prompt floats above
/// the overlays, but System Settings is an ordinary window they would cover,
/// so a denied permission steps the picker aside before opening the pane.
fn request_media_access(permission: crate::permissions::OSPermission, cx: &mut App) {
    let access = crate::media_access::MediaAccess::global(cx);
    if access.read(cx).allowed(permission) {
        return;
    }
    let denied =
        access.read(cx).status(permission) == crate::permissions::OSPermissionStatus::Denied;
    if denied {
        cx.defer(move |cx| {
            app_windows::dismiss_target_overlays(cx);
            crate::media_access::MediaAccess::global(cx)
                .update(cx, |access, cx| access.request(permission, cx));
        });
    } else {
        access.update(cx, |access, cx| access.request(permission, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_overlay_remains_interactive_without_a_global_cursor_probe() {
        let display = "1".parse::<DisplayId>().unwrap();
        let other = "2".parse::<DisplayId>().unwrap();
        assert!(overlay_display_is_active(None, &display, true));
        assert!(!overlay_display_is_active(None, &display, false));
        assert!(overlay_display_is_active(Some(&display), &display, false));
        assert!(!overlay_display_is_active(Some(&other), &display, true));
    }

    #[test]
    fn area_controls_stay_above_the_dock_on_a_small_display() {
        let available = rect(0., 25., 1024., 684.);
        let height = CLUSTER_HEIGHT + 78.;
        let (x, y) = area_controls_position(rect(202., 216., 641., 361.), available, height);
        assert!(x >= available.x && x + CLUSTER_WIDTH <= available.right());
        assert!(y >= available.y && y + height <= available.bottom());
        assert!(y < 577.);
    }

    #[test]
    fn area_controls_respect_side_taskbars_and_bottom_edge_crops() {
        let available = rect(64., 24., 736., 416.);
        for crop in [rect(0., 0., 800., 480.), rect(600., 350., 180., 120.)] {
            let (x, y) = area_controls_position(crop, available, CLUSTER_HEIGHT + 78.);
            assert!(x >= available.x && x + CLUSTER_WIDTH <= available.right());
            assert!(y >= available.y && y + CLUSTER_HEIGHT + 78. <= available.bottom());
        }
    }

    #[test]
    fn area_controls_keep_available_space_below_or_above_a_crop() {
        let available = rect(0., 0., 1280., 800.);
        assert_eq!(
            area_controls_position(rect(100., 100., 640., 360.), available, 166.).1,
            476.
        );
        assert_eq!(
            area_controls_position(rect(100., 500., 640., 240.), available, 166.).1,
            318.
        );
    }

    #[test]
    fn compositor_viewport_replaces_placeholder_size_and_clamps_the_selection() {
        let viewport = overlay_viewport_size((1024., 768.), (1920., 1080.));
        assert_eq!(viewport, (1024., 768.));
        let crop = rect(700., 400., 640., 480.).clamped(viewport);
        assert!(crop.x >= 0. && crop.y >= 0.);
        assert!(crop.right() <= 1024. && crop.bottom() <= 768.);
        assert_eq!(overlay_viewport_size((0., 0.), viewport), viewport);
        assert_eq!(overlay_viewport_size((f32::NAN, 768.), viewport), viewport);
        assert_eq!(
            overlay_viewport_size((1024., f32::INFINITY), viewport),
            viewport
        );
    }

    #[test]
    fn mode_menu_matches_menu_modes() {
        // `menuModes` order (`target-select-overlay.tsx:2064-2090`) with the
        // check on the current mode.
        let items = mode_menu_items(Mode::Instant);
        assert_eq!(
            items
                .iter()
                .map(|item| (item.label.to_string(), item.checked))
                .collect::<Vec<_>>(),
            vec![
                ("Studio Mode".to_string(), false),
                ("Instant Mode".to_string(), true),
                ("Screenshot Mode".to_string(), false),
            ],
        );
        assert_eq!(MODE_MENU[2].1, Mode::Screenshot);
    }

    #[test]
    fn required_window_titles_match_exactly_or_fail_closed() {
        assert!(required_window_title_matches(None, None));
        assert!(required_window_title_matches(None, Some("Other window")));
        assert!(required_window_title_matches(
            Some("Synthetic target"),
            Some("Synthetic target")
        ));
        assert!(!required_window_title_matches(
            Some("Synthetic target"),
            Some("Synthetic target - private window")
        ));
        assert!(!required_window_title_matches(
            Some("Synthetic target"),
            Some("Private window")
        ));
        assert!(!required_window_title_matches(
            Some("Synthetic target"),
            None
        ));
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> AreaRect {
        AreaRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn area_rects_clamp_into_the_display() {
        let display = (1000., 800.);
        // Dragged off the right edge: pushed back, size preserved.
        assert_eq!(
            rect(950., 10., 200., 100.).clamped(display),
            rect(800., 10., 200., 100.)
        );
        // Bigger than the display: capped, not offset.
        assert_eq!(
            rect(-50., -50., 2000., 2000.).clamped(display),
            rect(0., 0., 1000., 800.)
        );
    }

    /// `MIN_SIZE` is 150x150 for a recording; anything smaller is drawn but
    /// not recordable. A screenshot only needs `MIN_SCREENSHOT_SIZE` (1px).
    #[test]
    fn area_validity_matches_the_min_size() {
        assert!(rect(0., 0., 150., 150.).is_valid_for(area_min_size(Mode::Studio)));
        assert!(!rect(0., 0., 149., 400.).is_valid_for(area_min_size(Mode::Studio)));
        assert!(!rect(0., 0., 400., 149.).is_valid_for(area_min_size(Mode::Studio)));
        assert!(rect(0., 0., 20., 20.).is_valid_for(area_min_size(Mode::Screenshot)));
        assert!(!rect(0., 0., 0.5, 20.).is_valid_for(area_min_size(Mode::Screenshot)));
    }

    /// The `ScreenCaptureTarget::Area` bounds are display-relative logical
    /// points -- the same space the overlay draws in, which is what makes the
    /// crop and the recording agree.
    #[test]
    fn area_bounds_are_display_relative_logical_points() {
        let bounds = rect(120., 64., 800., 600.).to_bounds();
        assert_eq!(bounds.position().x(), 120.);
        assert_eq!(bounds.position().y(), 64.);
        assert_eq!(bounds.size().width(), 800.);
        assert_eq!(bounds.size().height(), 600.);
    }

    #[test]
    fn moving_area_snaps_to_display_centres_and_edges() {
        let display = (1000., 800.);
        let (centered, center_guides) = snap_area_rect(rect(446., 180., 100., 120.), display, &[]);
        assert_eq!(centered, rect(450., 180., 100., 120.));
        assert!(
            center_guides
                .iter()
                .any(|guide| guide.axis == GuideAxis::V && guide.pos == 0.5)
        );

        let (edge, edge_guides) = snap_area_rect(rect(894., 180., 100., 120.), display, &[]);
        assert_eq!(edge, rect(900., 180., 100., 120.));
        assert!(
            edge_guides
                .iter()
                .any(|guide| guide.axis == GuideAxis::V && guide.pos == 1.)
        );
    }

    #[test]
    fn drawing_endpoint_snaps_per_axis_within_seven_pixels() {
        let display = (1000., 800.);
        let (snapped, guides) = snap_area_point((496., 404.), display, &[]);
        assert_eq!(snapped, (500., 400.));
        assert!(
            guides
                .iter()
                .any(|guide| guide.axis == GuideAxis::V && guide.pos == 0.5)
        );
        assert!(
            guides
                .iter()
                .any(|guide| guide.axis == GuideAxis::H && guide.pos == 0.5)
        );

        let (unsnapped, guides) = snap_area_point((490., 390.), display, &[]);
        assert_eq!(unsnapped, (490., 390.));
        assert!(guides.is_empty());
    }
}
