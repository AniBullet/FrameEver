use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, Bounds, Context, CursorStyle, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, RenderImage, ScrollEvent, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, div, point, prelude::FluentBuilder, px, size,
};

use super::{HEADER_HEIGHT, LibraryWindow, kind_label, t, ui, view};
use crate::{
    library::{self, MediaKind, player::Direction, view::Entry},
    theme::motion,
};

const TOP_BAR: f32 = 48.;
const FOOT: f32 = 56.;
const EDGE_INSET: f32 = 12.;
const STEP_ICON: f32 = 16.;
const DRAG_THRESHOLD: f32 = 4.;
const SCRUB_PIXELS_PER_FRAME: f32 = 4.;
const FALLBACK_FRAME_INTERVAL: f64 = 1. / 30.;
const WHOLE_SCREEN_ICON: f32 = 14.;
const ACTUAL_SIZE_BOUNDS: (u32, u32) = (u32::MAX, u32::MAX);
const CONTROLS_HOLD: Duration = Duration::from_millis(2500);
const MIDDLE: (f32, f32) = (0.5, 0.5);
const WHEEL_LINE: f32 = 40.;
const ZOOM_PER_LINE: f32 = 1.2;
const MAX_ZOOM: f32 = 8.;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Zoom {
    Fit,
    Filled,
    Scaled { scale: f32, centre: (f32, f32) },
}

impl Zoom {
    fn scale(self, picture: (f32, f32), stage: (f32, f32)) -> f32 {
        match self {
            Self::Fit => fitted_scale(picture, stage),
            Self::Filled => contained_scale(picture, stage).min(MAX_ZOOM),
            Self::Scaled { scale, .. } => scale,
        }
    }

    fn centre(self) -> (f32, f32) {
        match self {
            Self::Fit | Self::Filled => MIDDLE,
            Self::Scaled { centre, .. } => centre,
        }
    }
}

fn contained_scale(picture: (f32, f32), stage: (f32, f32)) -> f32 {
    (stage.0 / picture.0.max(1.)).min(stage.1 / picture.1.max(1.))
}

fn fitted_scale(picture: (f32, f32), stage: (f32, f32)) -> f32 {
    contained_scale(picture, stage).min(1.)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SizeAction {
    ActualSize,
    FitWindow,
}

fn size_toggle(
    zoom: Zoom,
    point: (f32, f32),
    picture: (f32, f32),
    stage: (f32, f32),
) -> (Zoom, SizeAction) {
    let contained = contained_scale(picture, stage);
    match zoom {
        Zoom::Fit | Zoom::Filled if contained < 1. => (
            zoomed_to(zoom, point, 1., picture, stage),
            SizeAction::ActualSize,
        ),
        Zoom::Fit if contained > 1. => (Zoom::Filled, SizeAction::FitWindow),
        Zoom::Fit | Zoom::Filled => (Zoom::Fit, SizeAction::ActualSize),
        Zoom::Scaled { .. } if contained < 1. => (Zoom::Fit, SizeAction::FitWindow),
        Zoom::Scaled { .. } => (Zoom::Fit, SizeAction::ActualSize),
    }
}

pub(super) struct Placed {
    pub(super) origin: (f32, f32),
    pub(super) size: (f32, f32),
}

fn drawn_bounds(zoom: Zoom, picture: (f32, f32), stage: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let scale = zoom.scale(picture, stage);
    let drawn = (picture.0 * scale, picture.1 * scale);
    (actual_origin(zoom.centre(), drawn, stage), drawn)
}

fn zoomed_to(
    zoom: Zoom,
    point: (f32, f32),
    scale: f32,
    picture: (f32, f32),
    stage: (f32, f32),
) -> Zoom {
    let scale = scale.min(MAX_ZOOM);
    if scale <= fitted_scale(picture, stage) {
        return Zoom::Fit;
    }
    let (origin, drawn) = drawn_bounds(zoom, picture, stage);
    let axis = |point: f32, origin: f32, drawn: f32, picture: f32, stage: f32| {
        let fraction = ((point - origin) / drawn.max(1.)).clamp(0., 1.);
        let next = (picture * scale).max(1.);
        (stage / 2. - (point - fraction * next)) / next
    };
    Zoom::Scaled {
        scale,
        centre: (
            axis(point.0, origin.0, drawn.0, picture.0, stage.0),
            axis(point.1, origin.1, drawn.1, picture.1, stage.1),
        ),
    }
}

fn wheel_zoom(
    zoom: Zoom,
    point: (f32, f32),
    lines: f32,
    picture: (f32, f32),
    stage: (f32, f32),
) -> Zoom {
    let scale = zoom.scale(picture, stage) * ZOOM_PER_LINE.powf(lines);
    zoomed_to(zoom, point, scale, picture, stage)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shown {
    Video,
    Screenshot,
}

impl Shown {
    fn of(kind: MediaKind) -> Self {
        match kind {
            MediaKind::Screenshot => Self::Screenshot,
            _ => Self::Video,
        }
    }
}

fn opening_zoom(shown: Shown) -> Zoom {
    match shown {
        Shown::Video => Zoom::Filled,
        Shown::Screenshot => Zoom::Fit,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewKey {
    Close,
    HoldSpace,
    Previous,
    Next,
    StepFrame(Direction),
    PlayPause,
    Back,
    Forward,
    Mute,
    Loop,
    WholeScreen,
    ActualSize,
}

impl PreviewKey {
    fn repeats(self) -> bool {
        matches!(
            self,
            Self::Previous | Self::Next | Self::StepFrame(_) | Self::Back | Self::Forward
        )
    }
}

pub(super) fn preview_key(
    keystroke: &Keystroke,
    on_overlay: bool,
    shown: Shown,
) -> Option<PreviewKey> {
    if keystroke.modifiers.modified() {
        return None;
    }
    let key = match (keystroke.key.as_str(), shown) {
        ("space", Shown::Video) if on_overlay => PreviewKey::HoldSpace,
        ("space", Shown::Screenshot) if on_overlay => PreviewKey::Close,
        ("left", Shown::Video) => PreviewKey::StepFrame(Direction::Back),
        ("right", Shown::Video) => PreviewKey::StepFrame(Direction::Forward),
        ("left", Shown::Screenshot) | ("pageup", _) => PreviewKey::Previous,
        ("right", Shown::Screenshot) | ("pagedown", _) => PreviewKey::Next,
        ("k", Shown::Video) => PreviewKey::PlayPause,
        ("j", Shown::Video) => PreviewKey::Back,
        ("l", Shown::Video) => PreviewKey::Forward,
        ("m", Shown::Video) => PreviewKey::Mute,
        ("r", Shown::Video) => PreviewKey::Loop,
        ("f", _) => PreviewKey::WholeScreen,
        ("z", _) => PreviewKey::ActualSize,
        _ => return None,
    };
    Some(key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Space {
    #[default]
    Up,
    Held {
        panned: bool,
    },
}

impl Space {
    fn held(self) -> bool {
        matches!(self, Self::Held { .. })
    }

    fn panned(self) -> Self {
        match self {
            Self::Up => Self::Up,
            Self::Held { .. } => Self::Held { panned: true },
        }
    }

    fn plays_on_release(self) -> bool {
        self == Self::Held { panned: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Gesture {
    Pending {
        from: (f32, f32),
    },
    Scrubbing {
        from_x: f32,
        from_position: f64,
        target: f64,
    },
    Panning {
        button: MouseButton,
        last: (f32, f32),
    },
}

impl Gesture {
    fn button(self) -> MouseButton {
        match self {
            Self::Pending { .. } | Self::Scrubbing { .. } => MouseButton::Left,
            Self::Panning { button, .. } => button,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressAction {
    Pend,
    Pan,
    WholeScreen,
    SwitchSize,
}

fn press_action(
    shown: Shown,
    button: MouseButton,
    click_count: usize,
    space_held: bool,
) -> Option<PressAction> {
    match (button, shown) {
        (MouseButton::Middle, _) => Some(PressAction::Pan),
        (MouseButton::Left, _) if space_held => Some(PressAction::Pan),
        (MouseButton::Left, Shown::Video) if click_count >= 2 => Some(PressAction::WholeScreen),
        (MouseButton::Left, Shown::Screenshot) if click_count >= 2 => Some(PressAction::SwitchSize),
        (MouseButton::Left, _) => Some(PressAction::Pend),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragAction {
    Scrub,
    Pan,
}

fn drag_action(shown: Shown, space_held: bool, travel: (f32, f32)) -> Option<DragAction> {
    if travel.0.hypot(travel.1) <= DRAG_THRESHOLD {
        return None;
    }
    match shown {
        Shown::Video if !space_held => Some(DragAction::Scrub),
        _ => Some(DragAction::Pan),
    }
}

fn release_plays(shown: Shown, gesture: Option<Gesture>) -> bool {
    shown == Shown::Video && matches!(gesture, Some(Gesture::Pending { .. }))
}

fn scrub_target(from_position: f64, travel_x: f32, interval: f64, duration: f64) -> f64 {
    let frames = f64::from((travel_x / SCRUB_PIXELS_PER_FRAME).trunc());
    (from_position + frames * interval).clamp(0., duration.max(0.))
}

pub(super) fn stage_pixels(
    full_screen: bool,
    actual_size: bool,
    viewport: (f32, f32),
    scale: f32,
) -> (u32, u32) {
    if actual_size {
        return ACTUAL_SIZE_BOUNDS;
    }
    let (_, (width, height)) = stage_box(full_screen, viewport, HEADER_HEIGHT);
    (
        (width * scale).round().max(1.) as u32,
        (height * scale).round().max(1.) as u32,
    )
}

fn stage_box(full_screen: bool, viewport: (f32, f32), header: f32) -> ((f32, f32), (f32, f32)) {
    if full_screen {
        ((0., 0.), viewport)
    } else {
        ((0., header), (viewport.0, viewport.1 - header))
    }
}

fn axis_origin(centre: f32, picture: f32, stage: f32) -> f32 {
    if picture <= stage {
        (stage - picture) / 2.
    } else {
        (stage / 2. - centre * picture).clamp(stage - picture, 0.)
    }
}

pub(super) fn actual_origin(
    centre: (f32, f32),
    picture: (f32, f32),
    stage: (f32, f32),
) -> (f32, f32) {
    (
        axis_origin(centre.0, picture.0, stage.0),
        axis_origin(centre.1, picture.1, stage.1),
    )
}

fn axis_moved(centre: f32, delta: f32, picture: f32, stage: f32) -> f32 {
    if picture <= stage {
        return centre;
    }
    let origin = (axis_origin(centre, picture, stage) + delta).clamp(stage - picture, 0.);
    (stage / 2. - origin) / picture
}

fn moved_centre(
    centre: (f32, f32),
    delta: (f32, f32),
    picture: (f32, f32),
    stage: (f32, f32),
) -> (f32, f32) {
    (
        axis_moved(centre.0, delta.0, picture.0, stage.0),
        axis_moved(centre.1, delta.1, picture.1, stage.1),
    )
}

fn window_point(position: Point<Pixels>) -> (f32, f32) {
    (f32::from(position.x), f32::from(position.y))
}

pub(super) fn actual_size_of(image: (u32, u32), scale: f32) -> (f32, f32) {
    (image.0 as f32 / scale, image.1 as f32 / scale)
}

fn progress(elapsed: Duration, total: Duration) -> f32 {
    (elapsed.as_secs_f32() / total.as_secs_f32()).min(1.)
}

struct Still {
    path: PathBuf,
    image: Arc<RenderImage>,
    source: Option<(u32, u32)>,
}

pub(super) struct Overlay {
    opened_at: Instant,
    from: Option<Bounds<Pixels>>,
    closed_at: Option<Instant>,
    full_screen: bool,
    zoom: Zoom,
    gesture: Option<Gesture>,
    space: Space,
    click_played: bool,
    controls_until: Instant,
    controls_task: Option<Task<()>>,
    still: Option<Still>,
    still_asked: Option<(u32, u32)>,
    still_task: Option<Task<()>>,
}

impl Overlay {
    fn new(from: Option<Bounds<Pixels>>, now: Instant) -> Self {
        Self {
            opened_at: now,
            from,
            closed_at: None,
            full_screen: false,
            zoom: Zoom::Fit,
            gesture: None,
            space: Space::Up,
            click_played: false,
            controls_until: now,
            controls_task: None,
            still: None,
            still_asked: None,
            still_task: None,
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.closed_at.is_none()
    }

    pub(super) fn is_full_screen(&self) -> bool {
        self.full_screen
    }

    fn zoomed(&self) -> bool {
        matches!(self.zoom, Zoom::Scaled { .. })
    }

    pub(super) fn covers_page(&self, now: Instant) -> bool {
        self.is_open() && now.saturating_duration_since(self.opened_at) >= motion::MOVE
    }

    fn finished(&self, now: Instant) -> bool {
        self.closed_at
            .is_some_and(|closed| now.saturating_duration_since(closed) >= motion::EXIT)
    }

    fn animating(&self, now: Instant) -> bool {
        !self.covers_page(now)
    }

    fn opacity(&self, now: Instant) -> f32 {
        match self.closed_at {
            Some(closed) => {
                1. - motion::EASE_EXIT.at(progress(
                    now.saturating_duration_since(closed),
                    motion::EXIT,
                ))
            }
            None => motion::EASE_FLUENT_IN.at(progress(
                now.saturating_duration_since(self.opened_at),
                motion::MID,
            )),
        }
    }

    fn area(&self, whole: Bounds<Pixels>, now: Instant, reduce_motion: bool) -> Bounds<Pixels> {
        let Some(from) = self
            .from
            .filter(|_| !reduce_motion && self.closed_at.is_none())
        else {
            return whole;
        };
        let t = motion::ease_move(progress(
            now.saturating_duration_since(self.opened_at),
            motion::MOVE,
        ))
        .min(1.);
        let mix = |a: Pixels, b: Pixels| a + (b - a) * t;
        Bounds {
            origin: point(
                mix(from.origin.x, whole.origin.x),
                mix(from.origin.y, whole.origin.y),
            ),
            size: size(
                mix(from.size.width, whole.size.width),
                mix(from.size.height, whole.size.height),
            ),
        }
    }

    fn pointer_capture(&self) -> Option<MouseButton> {
        self.gesture.filter(|_| self.is_open()).map(Gesture::button)
    }

    fn let_go(&mut self) {
        self.space = Space::Up;
        self.gesture = None;
    }

    fn shows_controls(&self, playing: bool, now: Instant) -> bool {
        !playing || now < self.controls_until
    }

    fn held_still(&self, path: &Path) -> Option<&Still> {
        self.still.as_ref().filter(|still| still.path == path)
    }

    fn still_of(&self, path: &Path) -> Option<Arc<RenderImage>> {
        self.held_still(path).map(|still| still.image.clone())
    }
}

impl LibraryWindow {
    pub(super) fn library_overlay_covers_page(&self) -> bool {
        self.library
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.covers_page(Instant::now()))
    }

    pub(super) fn library_overlay_open(&self) -> bool {
        self.library.overlay.as_ref().is_some_and(Overlay::is_open)
    }

    pub(super) fn library_zoomed(&self) -> bool {
        self.library
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.is_open() && overlay.zoomed())
    }

    pub(super) fn library_full_screen(&self) -> bool {
        self.library
            .overlay
            .as_ref()
            .is_some_and(Overlay::is_full_screen)
    }

    pub(super) fn let_go_of_library_overlay(&mut self) {
        if let Some(overlay) = &mut self.library.overlay {
            overlay.let_go();
        }
    }

    pub(super) fn preview_library_lead(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library.context.is_some() {
            self.close_library_menu(cx);
        }
        match self.library.selection.lead().map(Path::to_path_buf) {
            Some(path) => self.open_library_overlay(&path, window, cx),
            None => cx.propagate(),
        }
    }

    pub(super) fn open_library_overlay(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_library_menu(cx);
        self.clear_library_scrub(cx);
        if self.library.selection.single() != Some(path) {
            self.change_selection(cx, |selection, _| selection.click(path));
        }
        let from = self.library.drawn.borrow().get(path).copied();
        let replaced = self
            .library
            .overlay
            .replace(Overlay::new(from, Instant::now()));
        self.retire_overlay(replaced);
        window.focus(&self.library.overlay_focus, cx);
        self.show_overlay_item(window, cx);
        self.wake_overlay_controls(cx);
        cx.notify();
    }

    pub(super) fn retire_overlay(&mut self, overlay: Option<Overlay>) {
        let preview = &mut self.library.detail.preview;
        preview
            .retired
            .extend(overlay.and_then(|overlay| overlay.still.map(|still| still.image)));
    }

    pub(super) fn library_stage_pixels(&self, window: &Window) -> (u32, u32) {
        let scale = window.scale_factor();
        match self.library.overlay.as_ref().filter(|o| o.is_open()) {
            Some(overlay) => {
                let viewport = window.viewport_size();
                stage_pixels(
                    overlay.full_screen,
                    overlay.zoomed(),
                    (f32::from(viewport.width), f32::from(viewport.height)),
                    scale,
                )
            }
            None => super::preview::bounds_at(scale),
        }
    }

    fn show_overlay_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.library.single_entry().cloned() else {
            return;
        };
        let bounds = self.library_stage_pixels(window);
        self.library.detail.preview.sync_bounds(bounds);
        if let Some(overlay) = &mut self.library.overlay {
            overlay.still_asked = None;
            overlay.let_go();
            overlay.zoom = opening_zoom(Shown::of(entry.kind));
        }
        if entry.screenshot.is_none() && !self.library.detail.preview.playing {
            self.toggle_library_preview(window, cx);
        }
    }

    fn load_overlay_still(
        &mut self,
        path: PathBuf,
        png: PathBuf,
        bounds: (u32, u32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = cx.spawn_in(window, async move |this, cx| {
            let image = cx
                .background_executor()
                .spawn(async move {
                    let source = image::image_dimensions(&png).ok();
                    library::decode_still(&png, bounds).map(|image| (image, source))
                })
                .await;
            this.update(cx, |this, cx| {
                let page = &mut this.library;
                let Some((image, source)) = image else {
                    return;
                };
                let wanted = page.selection.single() == Some(path.as_path());
                match page.overlay.as_mut().filter(|_| wanted) {
                    Some(overlay) => {
                        let replaced = overlay.still.replace(Still {
                            path,
                            image,
                            source,
                        });
                        page.detail
                            .preview
                            .retired
                            .extend(replaced.map(|still| still.image));
                        cx.notify();
                    }
                    None => page.detail.preview.retired.push(image),
                }
            })
            .ok();
        });
        if let Some(overlay) = &mut self.library.overlay {
            overlay.still_task = Some(task);
        }
    }

    pub(super) fn close_library_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return false;
        };
        overlay.closed_at = Some(Instant::now());
        overlay.let_go();
        overlay.controls_task = None;
        overlay.still_task = None;
        let was_full_screen = std::mem::take(&mut overlay.full_screen);
        if was_full_screen {
            window.toggle_fullscreen();
        }
        self.library.detail.preview.pause();
        self.library.reveal_lead = true;
        window.focus(&self.library.items_focus, cx);
        cx.notify();
        was_full_screen
    }

    pub(super) fn toggle_library_full_screen(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return;
        };
        overlay.full_screen = !overlay.full_screen;
        window.toggle_fullscreen();
        self.wake_overlay_controls(cx);
        cx.notify();
    }

    fn toggle_library_actual_size(&mut self, window: &Window, cx: &mut Context<Self>) {
        let (_, stage) = self.overlay_stage_box(window);
        let picture = self.overlay_picture(window);
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return;
        };
        overlay.zoom = match picture {
            Some(picture) => {
                size_toggle(overlay.zoom, (stage.0 / 2., stage.1 / 2.), picture, stage).0
            }
            None => Zoom::Fit,
        };
        overlay.gesture = None;
        self.wake_overlay_controls(cx);
        cx.notify();
    }

    fn overlay_stage_box(&self, window: &Window) -> ((f32, f32), (f32, f32)) {
        let viewport = window.viewport_size();
        let header = if cfg!(target_os = "windows") {
            HEADER_HEIGHT
        } else {
            0.
        };
        stage_box(
            self.library_full_screen(),
            (f32::from(viewport.width), f32::from(viewport.height)),
            header,
        )
    }

    fn overlay_picture(&self, window: &Window) -> Option<(f32, f32)> {
        let entry = self.library.single_entry()?;
        let overlay = self.library.overlay.as_ref()?;
        let source = match overlay.held_still(&entry.path) {
            Some(still) => still.source,
            None => self.library.detail.preview.frame_source,
        }
        .or_else(|| {
            let pixels = self.preview_image(entry, None)?.size(0);
            Some((u32::from(pixels.width), u32::from(pixels.height)))
        })?;
        Some(actual_size_of(source, window.scale_factor()))
    }

    fn overlay_placed(&self, window: &Window) -> Option<Placed> {
        let zoom = self.library.overlay.as_ref()?.zoom;
        let picture = self.overlay_picture(window)?;
        let (origin, size) = drawn_bounds(zoom, picture, self.overlay_stage_box(window).1);
        Some(Placed { origin, size })
    }

    fn stage_point(&self, position: Point<Pixels>, window: &Window) -> (f32, f32) {
        let (origin, _) = self.overlay_stage_box(window);
        (
            f32::from(position.x) - origin.0,
            f32::from(position.y) - origin.1,
        )
    }

    fn overlay_shown(&self) -> Option<Shown> {
        self.library
            .single_entry()
            .map(|entry| Shown::of(entry.kind))
    }

    fn press_library_overlay_stage(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(shown) = self.overlay_shown() else {
            return;
        };
        let (_, stage) = self.overlay_stage_box(window);
        let point = self.stage_point(event.position, window);
        let picture = self.overlay_picture(window);
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return;
        };
        let undo_click = std::mem::take(&mut overlay.click_played);
        overlay.gesture = None;
        let at = window_point(event.position);
        match press_action(shown, event.button, event.click_count, overlay.space.held()) {
            None => {}
            Some(PressAction::Pend) => {
                overlay.gesture = Some(Gesture::Pending { from: at });
                cx.notify();
            }
            Some(PressAction::Pan) => {
                overlay.space = overlay.space.panned();
                overlay.gesture = Some(Gesture::Panning {
                    button: event.button,
                    last: at,
                });
                cx.notify();
            }
            Some(PressAction::SwitchSize) => {
                overlay.zoom = match picture {
                    Some(picture) => size_toggle(overlay.zoom, point, picture, stage).0,
                    None => Zoom::Fit,
                };
                cx.notify();
            }
            Some(PressAction::WholeScreen) => {
                if undo_click {
                    self.toggle_library_preview(window, cx);
                }
                self.toggle_library_full_screen(window, cx);
            }
        }
    }

    fn drag_library_overlay_stage(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.library.single_entry() else {
            return;
        };
        let shown = Shown::of(entry.kind);
        let duration = self.library_preview_length(entry);
        let preview = &self.library.detail.preview;
        let position = preview.position();
        let interval = preview.frame_interval().unwrap_or(FALLBACK_FRAME_INTERVAL);
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return;
        };
        let Some(gesture) = overlay.gesture else {
            return;
        };
        if event.pressed_button != Some(gesture.button()) {
            overlay.gesture = None;
            cx.notify();
            return;
        }
        let at = window_point(event.position);
        let starting = matches!(gesture, Gesture::Pending { .. });
        let gesture = match gesture {
            Gesture::Pending { from } => {
                match drag_action(shown, overlay.space.held(), (at.0 - from.0, at.1 - from.1)) {
                    None => return,
                    Some(DragAction::Pan) => {
                        overlay.space = overlay.space.panned();
                        Gesture::Panning {
                            button: MouseButton::Left,
                            last: from,
                        }
                    }
                    Some(DragAction::Scrub) => Gesture::Scrubbing {
                        from_x: from.0,
                        from_position: position,
                        target: position,
                    },
                }
            }
            moving => moving,
        };
        match gesture {
            Gesture::Pending { .. } => {}
            Gesture::Panning { button, last } => {
                overlay.gesture = Some(Gesture::Panning { button, last: at });
                self.pan_library_overlay((at.0 - last.0, at.1 - last.1), window, cx);
            }
            Gesture::Scrubbing {
                from_x,
                from_position,
                target,
            } => {
                let next = scrub_target(from_position, at.0 - from_x, interval, duration);
                overlay.gesture = Some(Gesture::Scrubbing {
                    from_x,
                    from_position,
                    target: next,
                });
                if starting {
                    self.pause_library_preview(cx);
                }
                if next != target {
                    self.seek_library_preview(next, window, cx);
                }
            }
        }
    }

    fn release_library_overlay_stage(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(shown) = self.overlay_shown() else {
            return;
        };
        let Some(overlay) = &mut self.library.overlay else {
            return;
        };
        let Some(gesture) = overlay.gesture.take() else {
            return;
        };
        cx.notify();
        if release_plays(shown, Some(gesture)) {
            overlay.click_played = true;
            self.toggle_library_preview(window, cx);
        }
    }

    fn pan_library_overlay(
        &mut self,
        delta: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (_, stage) = self.overlay_stage_box(window);
        let Some(picture) = self.overlay_picture(window) else {
            return;
        };
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return;
        };
        if let Zoom::Scaled { scale, centre } = overlay.zoom {
            let drawn = (picture.0 * scale, picture.1 * scale);
            overlay.zoom = Zoom::Scaled {
                scale,
                centre: moved_centre(centre, delta, drawn, stage),
            };
            cx.notify();
        }
    }

    fn zoom_library_overlay(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let lines = f32::from(event.delta.pixel_delta(px(WHEEL_LINE)).y) / WHEEL_LINE;
        if lines == 0. {
            return;
        }
        let (_, stage) = self.overlay_stage_box(window);
        let point = self.stage_point(event.position, window);
        let Some(picture) = self.overlay_picture(window) else {
            return;
        };
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return;
        };
        let zoom = wheel_zoom(overlay.zoom, point, lines, picture, stage);
        if zoom != overlay.zoom {
            overlay.zoom = zoom;
            cx.notify();
        }
    }

    fn overlay_stage(
        &self,
        picture: AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("library-overlay-stage")
            .absolute()
            .inset_0()
            .when(self.library_zoomed(), |this| {
                this.cursor(CursorStyle::OpenHand)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(Self::press_library_overlay_stage),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(Self::press_library_overlay_stage),
            )
            .on_scroll(cx.listener(Self::zoom_library_overlay))
            .child(picture)
    }

    fn wake_overlay_controls(&mut self, cx: &mut Context<Self>) {
        let Some(overlay) = &mut self.library.overlay else {
            return;
        };
        let now = Instant::now();
        let hidden = now >= overlay.controls_until;
        overlay.controls_until = now + CONTROLS_HOLD;
        if !hidden {
            return;
        }
        overlay.controls_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(until)) = this.update(cx, |this, _| {
                    this.library
                        .overlay
                        .as_ref()
                        .map(|overlay| overlay.controls_until)
                }) else {
                    return;
                };
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    this.update(cx, |_, cx| cx.notify()).ok();
                    return;
                }
                cx.background_executor().timer(left).await;
            }
        }));
        cx.notify();
    }

    fn step_library_overlay(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let page = &self.library;
        let Some(path) = page
            .selection
            .lead()
            .and_then(|lead| view::neighbour(&page.order, lead, forward))
            .cloned()
        else {
            return;
        };
        self.change_selection(cx, |selection, _| selection.click(&path));
        self.show_overlay_item(window, cx);
    }

    fn on_library_overlay_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wake_overlay_controls(cx);
        let on_overlay = self.library.overlay_focus.is_focused(window);
        let Some(shown) = self.overlay_shown() else {
            return;
        };
        let Some(key) = preview_key(&event.keystroke, on_overlay, shown) else {
            return;
        };
        cx.stop_propagation();
        if event.is_held && !key.repeats() {
            return;
        }
        match key {
            PreviewKey::Close => {
                self.close_library_overlay(window, cx);
            }
            PreviewKey::HoldSpace => {
                if let Some(overlay) = &mut self.library.overlay {
                    overlay.space = Space::Held { panned: false };
                }
            }
            PreviewKey::Previous => self.step_library_overlay(false, window, cx),
            PreviewKey::Next => self.step_library_overlay(true, window, cx),
            PreviewKey::StepFrame(direction) => {
                self.step_library_preview(direction, event.is_held, window, cx)
            }
            PreviewKey::PlayPause => self.toggle_library_preview(window, cx),
            PreviewKey::Back => self.skip_library_preview(false, window, cx),
            PreviewKey::Forward => self.skip_library_preview(true, window, cx),
            PreviewKey::Mute => self.toggle_library_mute(cx),
            PreviewKey::Loop => self.toggle_library_loop(cx),
            PreviewKey::WholeScreen => self.toggle_library_full_screen(window, cx),
            PreviewKey::ActualSize => self.toggle_library_actual_size(window, cx),
        }
    }

    fn on_library_overlay_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "space" {
            return;
        }
        let Some(overlay) = &mut self.library.overlay else {
            return;
        };
        if std::mem::take(&mut overlay.space).plays_on_release() {
            cx.stop_propagation();
            self.toggle_library_preview(window, cx);
        }
    }

    pub(super) fn settle_library_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let Some(overlay) = &self.library.overlay else {
            return;
        };
        if overlay.finished(now) {
            let overlay = self.library.overlay.take();
            self.retire_overlay(overlay);
            return;
        }
        let animating = overlay.animating(now);
        if overlay.is_open() && self.library.single_entry().is_none() {
            self.close_library_overlay(window, cx);
        }
        let bounds = self.library_stage_pixels(window);
        let still = self
            .library
            .single_entry()
            .and_then(|entry| Some((entry.path.clone(), entry.screenshot.clone()?)));
        if let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open())
            && let Some((path, png)) = still
            && overlay.still_asked != Some(bounds)
        {
            overlay.still_asked = Some(bounds);
            self.load_overlay_still(path, png, bounds, window, cx);
        }
        if animating {
            window.request_animation_frame();
        }
    }

    fn render_overlay_top(&self, entry: &Entry, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let mut meta = kind_label(entry.kind).to_string();
        if let Some(duration) = entry.duration {
            meta = format!("{meta} · {}", view::format_clock(duration));
        }
        meta = format!("{meta} · {}", view::format_size(entry.size));
        let open = entry.clone();
        let exports =
            entry.opens_editor || entry.screenshot.is_some() || entry.kind == MediaKind::Instant;
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .h(px(TOP_BAR))
            .px(px(12.))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(entry.name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(meta)),
                    ),
            )
            .when(entry.opens_editor, |this| {
                this.child(
                    self.detail_button("library-overlay-open", t!("Open in editor"), false, false)
                        .flex_none()
                        .px(px(12.))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.close_library_overlay(window, cx);
                            this.open_library_entry(&open, window, cx);
                        })),
                )
            })
            .when(exports, |this| {
                this.child(
                    self.preview_button(
                        "library-overlay-export",
                        "icons/download.svg",
                        STEP_ICON,
                        t!("Export"),
                        &[],
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pause_library_preview(cx);
                        this.export_library_selection(window, cx);
                    })),
                )
            })
            .when(library::file_to_share(entry).is_some(), |this| {
                this.child(
                    self.preview_button(
                        "library-overlay-external",
                        "icons/external-link.svg",
                        STEP_ICON,
                        t!("Open in default app"),
                        &[],
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_library_file_externally(cx);
                    })),
                )
            })
            .child(
                self.preview_button(
                    "library-overlay-reveal",
                    "icons/folder-open.svg",
                    STEP_ICON,
                    t!("Show in folder"),
                    &[],
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reveal_library_selection(cx);
                })),
            )
            .child(
                self.preview_button(
                    "library-overlay-close",
                    "icons/x.svg",
                    STEP_ICON,
                    t!("Close"),
                    &["Esc"],
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.close_library_overlay(window, cx);
                })),
            )
    }

    fn overlay_step_button(
        &self,
        forward: bool,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let (id, icon, label, key): (_, _, _, &'static [&'static str]) = if forward {
            (
                "library-overlay-next",
                "icons/chevron-right.svg",
                t!("Next item"),
                &["PgDn"],
            )
        } else {
            (
                "library-overlay-previous",
                "icons/chevron-left.svg",
                t!("Previous item"),
                &["PgUp"],
            )
        };
        let reachable = view::neighbour(&self.library.order, &entry.path, forward).is_some();
        let button = self
            .preview_button(id, icon, STEP_ICON, label, key)
            .occlude()
            .bg(self.theme.popover)
            .when(!reachable, |this| this.opacity(0.5))
            .when(reachable, |this| {
                this.on_click(cx.listener(move |this, _, window, cx| {
                    this.step_library_overlay(forward, window, cx);
                }))
            });
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .when(forward, |this| this.right(px(EDGE_INSET)))
            .when(!forward, |this| this.left(px(EDGE_INSET)))
            .flex()
            .items_center()
            .child(button)
    }

    fn overlay_size_action(&self, window: &Window) -> SizeAction {
        let zoom = self.library.overlay.as_ref().map(|overlay| overlay.zoom);
        match (zoom, self.overlay_picture(window)) {
            (Some(zoom), Some(picture)) => {
                let (_, stage) = self.overlay_stage_box(window);
                size_toggle(zoom, (stage.0 / 2., stage.1 / 2.), picture, stage).1
            }
            _ => SizeAction::ActualSize,
        }
    }

    fn render_overlay_foot(
        &self,
        entry: &Entry,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let theme = self.theme;
        let full_screen = self.library_full_screen();
        let (icon, label) = if full_screen {
            ("icons/minimize.svg", t!("Exit whole screen"))
        } else {
            ("icons/maximize.svg", t!("Whole screen"))
        };
        let (size_icon, size_label) = match self.overlay_size_action(window) {
            SizeAction::FitWindow => ("icons/ratio.svg", t!("Fit to window")),
            SizeAction::ActualSize => ("icons/scan.svg", t!("Actual size")),
        };
        let duration = self.library_preview_length(entry);
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(10.))
            .h(px(FOOT))
            .px(px(24.))
            .text_size(px(12.))
            .text_color(theme.muted_foreground)
            .when(entry.kind == MediaKind::Screenshot, |this| {
                this.child(div().flex_1())
            })
            .when(entry.kind != MediaKind::Screenshot, |this| {
                this.child(self.preview_play_button(cx))
                    .child(self.preview_frame_time(duration))
                    .child(self.preview_seek(duration, cx))
                    .child(self.preview_mute_button(cx))
                    .child(self.preview_volume(cx))
                    .child(self.preview_loop_button(cx))
            })
            .child(
                self.preview_button(
                    "library-overlay-actual-size",
                    size_icon,
                    WHOLE_SCREEN_ICON,
                    size_label,
                    &["Z"],
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.toggle_library_actual_size(window, cx);
                })),
            )
            .child(
                self.preview_button(
                    "library-overlay-whole",
                    icon,
                    WHOLE_SCREEN_ICON,
                    label,
                    &["F"],
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.toggle_library_full_screen(window, cx);
                })),
            )
    }

    pub(super) fn render_library_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let overlay = self.library.overlay.as_ref()?;
        let entry = self.library.single_entry()?;
        let theme = self.theme;
        let now = Instant::now();
        let preview = &self.library.detail.preview;
        preview.note_drawn(now);
        let viewport = window.viewport_size();
        let top = if overlay.full_screen || !cfg!(target_os = "windows") {
            px(0.)
        } else {
            px(HEADER_HEIGHT)
        };
        let whole = Bounds {
            origin: point(px(0.), top),
            size: size(viewport.width, viewport.height - top),
        };
        let area = overlay.area(whole, now, cx.reduce_motion());
        let placed = self.overlay_placed(window);
        let picture = self.preview_picture(entry, overlay.still_of(&entry.path), placed, cx);
        let picture = self.overlay_stage(picture, cx);
        let controls = overlay.shows_controls(preview.playing, now);
        let full_screen = overlay.full_screen;
        let root = div()
            .id("library-overlay")
            .track_focus(&self.library.overlay_focus)
            .absolute()
            .left(area.origin.x)
            .top(area.origin.y)
            .w(area.size.width)
            .h(area.size.height)
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .opacity(overlay.opacity(now))
            .bg(if full_screen {
                theme.fixed_preview
            } else {
                theme.background.into()
            })
            .text_color(theme.foreground)
            .on_key_down(cx.listener(Self::on_library_overlay_key))
            .on_key_up(cx.listener(Self::on_library_overlay_key_up))
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| {
                this.wake_overlay_controls(cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(Self::release_library_overlay_stage),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(Self::release_library_overlay_stage),
            );
        Some(
            root.child(picture)
                .when(controls, |this| {
                    this.when(!full_screen, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .top_0()
                                .occlude()
                                .bg(theme.popover)
                                .child(self.render_overlay_top(entry, cx)),
                        )
                    })
                    .child(self.overlay_step_button(false, entry, cx))
                    .child(self.overlay_step_button(true, entry, cx))
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .occlude()
                            .bg(theme.popover)
                            .child(self.render_overlay_foot(entry, window, cx)),
                    )
                })
                .into_any_element(),
        )
    }

    pub(super) fn render_library_overlay_drag_layer(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let button = self.library.overlay.as_ref()?.pointer_capture()?;
        Some(
            ui::Slider::drag_layer_for(
                "library-overlay-drag",
                button,
                cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                    this.wake_overlay_controls(cx);
                    this.drag_library_overlay_stage(event, window, cx);
                }),
                cx.listener(Self::release_library_overlay_stage),
            )
            .occlude()
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use gpui::Keystroke;

    use super::*;

    fn key(text: &'static str, on_overlay: bool) -> Option<PreviewKey> {
        preview_key(&Keystroke::parse(text).unwrap(), on_overlay, Shown::Video)
    }

    fn screenshot_key(text: &'static str) -> Option<PreviewKey> {
        preview_key(&Keystroke::parse(text).unwrap(), true, Shown::Screenshot)
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(width), px(height)),
        }
    }

    #[test]
    fn every_video_preview_key_has_its_action() {
        assert_eq!(
            key("left", true),
            Some(PreviewKey::StepFrame(Direction::Back))
        );
        assert_eq!(
            key("right", true),
            Some(PreviewKey::StepFrame(Direction::Forward))
        );
        assert_eq!(key("pageup", true), Some(PreviewKey::Previous));
        assert_eq!(key("pagedown", true), Some(PreviewKey::Next));
        assert_eq!(key("space", true), Some(PreviewKey::HoldSpace));
        assert_eq!(key("k", true), Some(PreviewKey::PlayPause));
        assert_eq!(key("j", true), Some(PreviewKey::Back));
        assert_eq!(key("l", true), Some(PreviewKey::Forward));
        assert_eq!(key("m", true), Some(PreviewKey::Mute));
        assert_eq!(key("r", true), Some(PreviewKey::Loop));
        assert_eq!(key("f", true), Some(PreviewKey::WholeScreen));
        assert_eq!(key("z", true), Some(PreviewKey::ActualSize));
    }

    #[test]
    fn a_screenshot_steps_items_closes_on_space_and_has_no_playback_keys() {
        assert_eq!(screenshot_key("left"), Some(PreviewKey::Previous));
        assert_eq!(screenshot_key("right"), Some(PreviewKey::Next));
        assert_eq!(screenshot_key("pageup"), Some(PreviewKey::Previous));
        assert_eq!(screenshot_key("pagedown"), Some(PreviewKey::Next));
        assert_eq!(screenshot_key("space"), Some(PreviewKey::Close));
        assert_eq!(screenshot_key("f"), Some(PreviewKey::WholeScreen));
        assert_eq!(screenshot_key("z"), Some(PreviewKey::ActualSize));
        for playback in ["k", "j", "l", "m", "r"] {
            assert_eq!(screenshot_key(playback), None);
        }
    }

    #[test]
    fn esc_is_left_to_the_window_so_it_leaves_whole_screen_before_closing() {
        assert_eq!(key("escape", true), None);
        assert_eq!(screenshot_key("escape"), None);
    }

    #[test]
    fn space_on_a_focused_control_belongs_to_the_control() {
        assert_eq!(key("space", false), None);
        assert_eq!(
            preview_key(
                &Keystroke::parse("space").unwrap(),
                false,
                Shown::Screenshot
            ),
            None
        );
        assert_eq!(key("k", false), Some(PreviewKey::PlayPause));
    }

    #[test]
    fn a_tapped_space_plays_and_a_space_held_for_a_pan_does_not() {
        assert!(Space::Held { panned: false }.plays_on_release());
        assert!(!Space::Held { panned: false }.panned().plays_on_release());
        assert!(!Space::Up.plays_on_release());
        assert_eq!(Space::Up.panned(), Space::Up);
    }

    #[test]
    fn a_left_press_on_the_picture_waits_to_see_if_it_is_a_click_or_a_drag() {
        for shown in [Shown::Video, Shown::Screenshot] {
            assert_eq!(
                press_action(shown, MouseButton::Left, 1, false),
                Some(PressAction::Pend)
            );
        }
    }

    #[test]
    fn a_double_click_switches_whole_screen_for_a_video_and_size_for_a_screenshot() {
        assert_eq!(
            press_action(Shown::Video, MouseButton::Left, 2, false),
            Some(PressAction::WholeScreen)
        );
        assert_eq!(
            press_action(Shown::Screenshot, MouseButton::Left, 2, false),
            Some(PressAction::SwitchSize)
        );
    }

    #[test]
    fn the_middle_button_or_a_held_space_moves_the_picture() {
        for shown in [Shown::Video, Shown::Screenshot] {
            assert_eq!(
                press_action(shown, MouseButton::Middle, 1, false),
                Some(PressAction::Pan)
            );
            assert_eq!(
                press_action(shown, MouseButton::Left, 1, true),
                Some(PressAction::Pan)
            );
            assert_eq!(
                press_action(shown, MouseButton::Left, 2, true),
                Some(PressAction::Pan)
            );
        }
        assert_eq!(
            press_action(Shown::Video, MouseButton::Right, 1, false),
            None
        );
    }

    #[test]
    fn a_drag_starts_only_past_the_threshold() {
        let short = (DRAG_THRESHOLD - 1., 0.);
        let long = (DRAG_THRESHOLD + 1., 0.);
        assert_eq!(drag_action(Shown::Video, false, short), None);
        assert_eq!(drag_action(Shown::Screenshot, false, short), None);
        assert_eq!(
            drag_action(Shown::Video, false, long),
            Some(DragAction::Scrub)
        );
        assert_eq!(
            drag_action(Shown::Video, false, (0., -long.0)),
            Some(DragAction::Scrub)
        );
        assert_eq!(
            drag_action(Shown::Screenshot, false, long),
            Some(DragAction::Pan)
        );
        assert_eq!(drag_action(Shown::Video, true, long), Some(DragAction::Pan));
    }

    #[test]
    fn only_a_click_on_a_video_plays_or_pauses() {
        let pending = Some(Gesture::Pending { from: (10., 10.) });
        assert!(release_plays(Shown::Video, pending));
        assert!(!release_plays(Shown::Screenshot, pending));
        assert!(!release_plays(
            Shown::Video,
            Some(Gesture::Scrubbing {
                from_x: 10.,
                from_position: 1.,
                target: 2.,
            })
        ));
        assert!(!release_plays(
            Shown::Video,
            Some(Gesture::Panning {
                button: MouseButton::Left,
                last: (0., 0.),
            })
        ));
        assert!(!release_plays(Shown::Video, None));
    }

    #[test]
    fn scrubbing_moves_one_frame_per_few_pixels_of_travel() {
        let interval = 0.04;
        assert_eq!(scrub_target(2., 0., interval, 10.), 2.);
        assert_eq!(
            scrub_target(2., SCRUB_PIXELS_PER_FRAME - 0.5, interval, 10.),
            2.
        );
        assert_eq!(
            scrub_target(2., -(SCRUB_PIXELS_PER_FRAME - 0.5), interval, 10.),
            2.
        );
        assert!((scrub_target(2., SCRUB_PIXELS_PER_FRAME * 10., interval, 10.) - 2.4).abs() < 1e-9);
        assert!(
            (scrub_target(2., -SCRUB_PIXELS_PER_FRAME * 10., interval, 10.) - 1.6).abs() < 1e-9
        );
    }

    #[test]
    fn scrubbing_stops_at_either_end() {
        assert_eq!(scrub_target(0.5, -10_000., 0.04, 10.), 0.);
        assert_eq!(scrub_target(9.5, 10_000., 0.04, 10.), 10.);
    }

    #[test]
    fn a_video_opens_filling_the_stage_and_a_screenshot_at_actual_size() {
        assert_eq!(opening_zoom(Shown::Video), Zoom::Filled);
        assert_eq!(opening_zoom(Shown::Screenshot), Zoom::Fit);
    }

    #[test]
    fn from_filled_a_small_video_goes_to_actual_size_and_back() {
        let (picture, stage) = ((400., 200.), (1000., 600.));
        let (zoom, action) = size_toggle(Zoom::Filled, (500., 300.), picture, stage);
        assert_eq!(action, SizeAction::ActualSize);
        assert_eq!(zoom.scale(picture, stage), 1.);
        let (back, action) = size_toggle(zoom, (500., 300.), picture, stage);
        assert_eq!(action, SizeAction::FitWindow);
        assert_eq!(back, Zoom::Filled);
    }

    #[test]
    fn from_filled_a_large_video_goes_to_actual_size_and_back() {
        let (picture, stage) = ((3000., 2000.), (1000., 600.));
        let (zoom, action) = size_toggle(Zoom::Filled, (500., 300.), picture, stage);
        assert_eq!(action, SizeAction::ActualSize);
        assert_eq!(zoom.scale(picture, stage), 1.);
        let (back, action) = size_toggle(zoom, (500., 300.), picture, stage);
        assert_eq!(action, SizeAction::FitWindow);
        assert_eq!(
            drawn_bounds(back, picture, stage),
            drawn_bounds(Zoom::Filled, picture, stage)
        );
    }

    #[test]
    fn a_shortcut_with_a_modifier_is_left_alone() {
        assert_eq!(key("ctrl-l", true), None);
        assert_eq!(key("ctrl-f", true), None);
    }

    #[test]
    fn only_movement_repeats_while_held() {
        assert!(PreviewKey::Next.repeats());
        assert!(PreviewKey::StepFrame(Direction::Back).repeats());
        assert!(PreviewKey::StepFrame(Direction::Forward).repeats());
        assert!(PreviewKey::Forward.repeats());
        assert!(!PreviewKey::WholeScreen.repeats());
        assert!(!PreviewKey::PlayPause.repeats());
        assert!(!PreviewKey::HoldSpace.repeats());
        assert!(!PreviewKey::Close.repeats());
    }

    #[test]
    fn the_stage_decodes_at_the_pixels_it_covers() {
        assert_eq!(stage_pixels(true, false, (1920., 1080.), 1.5), (2880, 1620));
        assert_eq!(
            stage_pixels(false, false, (1000., 700.), 2.),
            (2000, ((700. - HEADER_HEIGHT) * 2.).round() as u32)
        );
    }

    #[test]
    fn a_window_too_small_for_the_chrome_still_asks_for_a_pixel() {
        assert_eq!(stage_pixels(false, false, (0., 0.), 1.), (1, 1));
    }

    #[test]
    fn actual_size_decodes_every_source_pixel_in_any_window() {
        assert_eq!(
            stage_pixels(false, true, (1000., 700.), 2.),
            ACTUAL_SIZE_BOUNDS
        );
        assert_eq!(
            stage_pixels(true, true, (1920., 1080.), 1.),
            ACTUAL_SIZE_BOUNDS
        );
    }

    #[test]
    fn the_picture_uses_the_whole_area_under_the_window_header() {
        assert_eq!(
            stage_box(false, (1000., 700.), 40.),
            ((0., 40.), (1000., 660.))
        );
        assert_eq!(
            stage_box(true, (1920., 1080.), 40.),
            ((0., 0.), (1920., 1080.))
        );
    }

    #[test]
    fn a_picture_smaller_than_the_stage_stays_in_the_middle() {
        assert_eq!(
            actual_origin((0., 0.), (400., 200.), (1000., 600.)),
            (300., 200.)
        );
        assert_eq!(
            moved_centre(MIDDLE, (500., 500.), (400., 200.), (1000., 600.)),
            MIDDLE
        );
    }

    #[test]
    fn moving_a_picture_that_fits_keeps_the_chosen_point() {
        assert_eq!(
            moved_centre((0.9, 0.1), (50., 50.), (400., 200.), (1000., 600.)),
            (0.9, 0.1)
        );
    }

    #[test]
    fn a_larger_picture_opens_on_its_middle() {
        assert_eq!(
            actual_origin(MIDDLE, (3000., 2000.), (1000., 600.)),
            (-1000., -700.)
        );
    }

    #[test]
    fn dragging_moves_the_picture_and_stops_at_its_edges() {
        let (picture, stage) = ((3000., 2000.), (1000., 600.));
        let moved = moved_centre(MIDDLE, (300., -100.), picture, stage);
        assert_eq!(actual_origin(moved, picture, stage), (-700., -800.));
        let far = moved_centre(MIDDLE, (9000., -9000.), picture, stage);
        assert_eq!(actual_origin(far, picture, stage), (0., -1400.));
        let back = moved_centre(far, (-100., 100.), picture, stage);
        assert_eq!(actual_origin(back, picture, stage), (-100., -1300.));
    }

    #[test]
    fn a_tall_picture_only_moves_up_and_down() {
        let (picture, stage) = ((400., 5000.), (1000., 600.));
        let moved = moved_centre(MIDDLE, (200., 200.), picture, stage);
        assert_eq!(actual_origin(moved, picture, stage), (300., -2000.));
    }

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    fn picture_point(
        zoom: Zoom,
        point: (f32, f32),
        picture: (f32, f32),
        stage: (f32, f32),
    ) -> (f32, f32) {
        let (origin, drawn) = drawn_bounds(zoom, picture, stage);
        (
            (point.0 - origin.0) / drawn.0,
            (point.1 - origin.1) / drawn.1,
        )
    }

    #[test]
    fn a_picture_smaller_than_the_stage_opens_at_its_own_size() {
        assert_eq!(
            drawn_bounds(Zoom::Fit, (400., 200.), (1000., 600.)),
            ((300., 200.), (400., 200.))
        );
    }

    #[test]
    fn a_picture_larger_than_the_stage_opens_whole_inside_it() {
        assert_eq!(
            drawn_bounds(Zoom::Fit, (2000., 1000.), (1000., 600.)),
            ((0., 50.), (1000., 500.))
        );
        assert_eq!(
            drawn_bounds(Zoom::Fit, (1080., 1920.), (1000., 600.)),
            ((331.25, 0.), (337.5, 600.))
        );
    }

    #[test]
    fn a_double_click_shows_actual_size_around_the_clicked_point() {
        let (picture, stage) = ((1080., 1920.), (1000., 600.));
        let point = (500., 450.);
        assert!(close(
            picture_point(Zoom::Fit, point, picture, stage),
            (0.5, 0.75)
        ));
        let zoom = zoomed_to(Zoom::Fit, point, 1., picture, stage);
        let Zoom::Scaled { scale, .. } = zoom else {
            panic!("still fitted");
        };
        assert_eq!(scale, 1.);
        assert!(close(
            picture_point(zoom, point, picture, stage),
            (0.5, 0.75)
        ));
        assert_eq!(drawn_bounds(zoom, picture, stage).1, picture);
    }

    #[test]
    fn a_picture_larger_than_the_stage_switches_between_fit_and_actual_size() {
        let (picture, stage) = ((3000., 2000.), (1000., 600.));
        let (zoom, action) = size_toggle(Zoom::Fit, (500., 300.), picture, stage);
        assert_eq!(action, SizeAction::ActualSize);
        assert_eq!(zoom.scale(picture, stage), 1.);
        let (back, action) = size_toggle(zoom, (500., 300.), picture, stage);
        assert_eq!(action, SizeAction::FitWindow);
        assert_eq!(back, Zoom::Fit);
    }

    #[test]
    fn a_picture_smaller_than_the_stage_grows_to_fill_it_without_cropping() {
        let (picture, stage) = ((400., 200.), (1000., 600.));
        assert_eq!(Zoom::Fit.scale(picture, stage), 1.);
        let (zoom, action) = size_toggle(Zoom::Fit, (900., 50.), picture, stage);
        assert_eq!(action, SizeAction::FitWindow);
        assert_eq!(zoom.scale(picture, stage), 2.5);
        assert_eq!(
            drawn_bounds(zoom, picture, stage),
            ((0., 50.), (1000., 500.))
        );
        let (back, action) = size_toggle(zoom, (500., 300.), picture, stage);
        assert_eq!(action, SizeAction::ActualSize);
        assert_eq!(back, Zoom::Fit);
    }

    #[test]
    fn a_filled_picture_follows_the_stage_when_it_changes_size() {
        let picture = (400., 200.);
        assert_eq!(
            drawn_bounds(Zoom::Filled, picture, (2000., 1200.)),
            ((0., 100.), (2000., 1000.))
        );
        assert_eq!(
            drawn_bounds(Zoom::Filled, picture, (800., 600.)),
            ((0., 100.), (800., 400.))
        );
    }

    #[test]
    fn a_picture_that_exactly_fits_stays_as_it_is() {
        let (picture, stage) = ((1000., 500.), (1000., 600.));
        assert_eq!(
            size_toggle(Zoom::Fit, (500., 300.), picture, stage).0,
            Zoom::Fit
        );
    }

    #[test]
    fn the_wheel_zooms_around_the_pointer() {
        let (picture, stage) = ((3000., 2000.), (1000., 600.));
        let point = (700., 200.);
        let before = picture_point(Zoom::Fit, point, picture, stage);
        let zoom = wheel_zoom(Zoom::Fit, point, 3., picture, stage);
        assert!(matches!(zoom, Zoom::Scaled { .. }));
        assert!(close(picture_point(zoom, point, picture, stage), before));
        let deeper = wheel_zoom(zoom, point, 2., picture, stage);
        assert!(close(picture_point(deeper, point, picture, stage), before));
    }

    #[test]
    fn the_wheel_also_zooms_a_picture_that_fits() {
        let zoom = wheel_zoom(Zoom::Fit, (500., 300.), 1., (400., 200.), (1000., 600.));
        let Zoom::Scaled { scale, .. } = zoom else {
            panic!("still fitted");
        };
        assert!((scale - ZOOM_PER_LINE).abs() < 1e-4);
    }

    #[test]
    fn zooming_out_past_the_fitted_size_returns_to_fit() {
        let (picture, stage) = ((3000., 2000.), (1000., 600.));
        let zoom = wheel_zoom(Zoom::Fit, (500., 300.), 2., picture, stage);
        assert_eq!(
            wheel_zoom(zoom, (500., 300.), -3., picture, stage),
            Zoom::Fit
        );
        assert_eq!(
            wheel_zoom(Zoom::Fit, (500., 300.), -1., picture, stage),
            Zoom::Fit
        );
    }

    #[test]
    fn the_wheel_stops_at_the_largest_zoom() {
        let zoom = wheel_zoom(Zoom::Fit, (500., 300.), 200., (400., 200.), (1000., 600.));
        assert_eq!(zoom.scale((400., 200.), (1000., 600.)), MAX_ZOOM);
    }

    #[test]
    fn the_clicked_point_lands_in_the_middle_of_the_stage() {
        let (picture, stage) = ((3000., 2000.), (1000., 600.));
        assert_eq!(actual_origin((0.75, 0.5), picture, stage), (-1750., -700.));
        assert_eq!(actual_origin((1., 1.), picture, stage), (-2000., -1400.));
    }

    #[test]
    fn actual_size_draws_one_source_pixel_per_screen_pixel() {
        assert_eq!(actual_size_of((1920, 1080), 1.), (1920., 1080.));
        assert_eq!(actual_size_of((1920, 1080), 1.5), (1280., 720.));
    }

    #[test]
    fn reduced_motion_only_fades() {
        let start = Instant::now();
        let overlay = Overlay::new(Some(rect(10., 10., 100., 60.)), start);
        let whole = rect(0., 36., 1000., 700.);
        assert_eq!(overlay.area(whole, start, true), whole);
        assert_eq!(overlay.opacity(start), 0.);
        assert_eq!(overlay.opacity(start + motion::MID), 1.);
    }

    #[test]
    fn the_overlay_grows_out_of_the_card() {
        let start = Instant::now();
        let card = rect(10., 10., 100., 60.);
        let overlay = Overlay::new(Some(card), start);
        let whole = rect(0., 36., 1000., 700.);
        assert_eq!(overlay.area(whole, start, false), card);
        let middle = overlay.area(whole, start + motion::MOVE / 3, false);
        assert!(middle.size.width > card.size.width && middle.size.width < whole.size.width);
        assert_eq!(overlay.area(whole, start + motion::MOVE, false), whole);
    }

    #[test]
    fn with_no_card_on_screen_the_overlay_only_fades() {
        let start = Instant::now();
        let whole = rect(0., 36., 1000., 700.);
        assert_eq!(Overlay::new(None, start).area(whole, start, false), whole);
    }

    #[test]
    fn the_page_is_drawn_until_the_overlay_has_arrived() {
        let start = Instant::now();
        let overlay = Overlay::new(None, start);
        assert!(!overlay.covers_page(start));
        assert!(overlay.covers_page(start + motion::MOVE));
    }

    #[test]
    fn closing_shows_the_page_and_fades_out() {
        let start = Instant::now();
        let mut overlay = Overlay::new(None, start);
        overlay.closed_at = Some(start + motion::MOVE);
        assert!(!overlay.covers_page(start + motion::MOVE));
        assert!(!overlay.finished(start + motion::MOVE));
        assert!(overlay.finished(start + motion::MOVE + motion::EXIT));
    }

    #[test]
    fn a_press_on_the_picture_captures_the_pointer_until_it_ends() {
        let start = Instant::now();
        let mut overlay = Overlay::new(None, start);
        assert_eq!(overlay.pointer_capture(), None);
        for (gesture, button) in [
            (Gesture::Pending { from: (1., 1.) }, MouseButton::Left),
            (
                Gesture::Scrubbing {
                    from_x: 1.,
                    from_position: 0.,
                    target: 0.,
                },
                MouseButton::Left,
            ),
            (
                Gesture::Panning {
                    button: MouseButton::Middle,
                    last: (1., 1.),
                },
                MouseButton::Middle,
            ),
        ] {
            overlay.gesture = Some(gesture);
            assert_eq!(overlay.pointer_capture(), Some(button));
        }
        overlay.closed_at = Some(start);
        assert_eq!(overlay.pointer_capture(), None);
    }

    #[test]
    fn letting_go_forgets_a_held_space_and_the_gesture_in_progress() {
        let mut overlay = Overlay::new(None, Instant::now());
        overlay.space = Space::Held { panned: false };
        overlay.gesture = Some(Gesture::Pending { from: (1., 1.) });
        overlay.let_go();
        assert_eq!(overlay.gesture, None);
        assert_eq!(overlay.pointer_capture(), None);
        assert!(!overlay.space.plays_on_release());
        assert_eq!(
            press_action(Shown::Video, MouseButton::Left, 1, overlay.space.held()),
            Some(PressAction::Pend)
        );
        assert_eq!(
            drag_action(
                Shown::Video,
                overlay.space.held(),
                (DRAG_THRESHOLD + 1., 0.)
            ),
            Some(DragAction::Scrub)
        );
    }

    #[test]
    fn controls_hide_only_while_playing_and_idle_in_a_window_and_on_the_whole_screen() {
        let start = Instant::now();
        for full_screen in [false, true] {
            let mut overlay = Overlay::new(None, start);
            overlay.full_screen = full_screen;
            overlay.controls_until = start + CONTROLS_HOLD;
            assert!(overlay.shows_controls(true, start));
            assert!(!overlay.shows_controls(true, start + CONTROLS_HOLD));
            assert!(overlay.shows_controls(false, start + CONTROLS_HOLD));
        }
    }
}
