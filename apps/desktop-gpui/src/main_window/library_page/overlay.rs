use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, Bounds, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    Keystroke, ParentElement, Pixels, RenderImage, SharedString, StatefulInteractiveElement,
    Styled, Task, Window, div, point, prelude::FluentBuilder, px, size,
};

use super::{MainWindow, kind_label, t, view};
use crate::{
    library::{self, MediaKind, view::Entry},
    main_window::HEADER_HEIGHT,
    theme::motion,
};

const TOP_BAR: f32 = 48.;
const FOOT: f32 = 56.;
const SIDE: f32 = 44.;
const STEP_ICON: f32 = 16.;
const WHOLE_SCREEN_ICON: f32 = 14.;
const CONTROLS_HOLD: Duration = Duration::from_millis(2500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewKey {
    Close,
    Previous,
    Next,
    PlayPause,
    Back,
    Forward,
    Mute,
    WholeScreen,
}

impl PreviewKey {
    fn repeats(self) -> bool {
        matches!(
            self,
            Self::Previous | Self::Next | Self::Back | Self::Forward
        )
    }

    fn video_only(self) -> bool {
        matches!(
            self,
            Self::PlayPause | Self::Back | Self::Forward | Self::Mute
        )
    }
}

pub(super) fn preview_key(keystroke: &Keystroke, on_overlay: bool) -> Option<PreviewKey> {
    if keystroke.modifiers.modified() {
        return None;
    }
    match keystroke.key.as_str() {
        "space" if on_overlay => Some(PreviewKey::Close),
        "left" => Some(PreviewKey::Previous),
        "right" => Some(PreviewKey::Next),
        "k" => Some(PreviewKey::PlayPause),
        "j" => Some(PreviewKey::Back),
        "l" => Some(PreviewKey::Forward),
        "m" => Some(PreviewKey::Mute),
        "f" => Some(PreviewKey::WholeScreen),
        _ => None,
    }
}

pub(super) fn stage_pixels(full_screen: bool, viewport: (f32, f32), scale: f32) -> (u32, u32) {
    let (width, height) = if full_screen {
        viewport
    } else {
        (
            viewport.0 - SIDE * 2.,
            viewport.1 - HEADER_HEIGHT - TOP_BAR - FOOT,
        )
    };
    (
        (width * scale).round().max(1.) as u32,
        (height * scale).round().max(1.) as u32,
    )
}

fn progress(elapsed: Duration, total: Duration) -> f32 {
    (elapsed.as_secs_f32() / total.as_secs_f32()).min(1.)
}

pub(super) struct Overlay {
    opened_at: Instant,
    from: Option<Bounds<Pixels>>,
    closed_at: Option<Instant>,
    full_screen: bool,
    controls_until: Instant,
    controls_task: Option<Task<()>>,
    still: Option<(PathBuf, Arc<RenderImage>)>,
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

    fn shows_controls(&self, playing: bool, now: Instant) -> bool {
        !self.full_screen || !playing || now < self.controls_until
    }

    fn still_of(&self, path: &Path) -> Option<Arc<RenderImage>> {
        self.still
            .as_ref()
            .filter(|(held, _)| held == path)
            .map(|(_, image)| image.clone())
    }
}

impl MainWindow {
    pub(in crate::main_window) fn library_overlay_covers_page(&self) -> bool {
        self.library
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.covers_page(Instant::now()))
    }

    pub(in crate::main_window) fn library_overlay_open(&self) -> bool {
        self.library.overlay.as_ref().is_some_and(Overlay::is_open)
    }

    pub(in crate::main_window) fn library_full_screen(&self) -> bool {
        self.library
            .overlay
            .as_ref()
            .is_some_and(Overlay::is_full_screen)
    }

    pub(in crate::main_window) fn preview_library_lead(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        cx.notify();
    }

    pub(super) fn retire_overlay(&mut self, overlay: Option<Overlay>) {
        let preview = &mut self.library.detail.preview;
        preview
            .retired
            .extend(overlay.and_then(|overlay| overlay.still.map(|(_, image)| image)));
    }

    pub(super) fn library_stage_pixels(&self, window: &Window) -> (u32, u32) {
        let scale = window.scale_factor();
        match self.library.overlay.as_ref().filter(|o| o.is_open()) {
            Some(overlay) => {
                let viewport = window.viewport_size();
                stage_pixels(
                    overlay.full_screen,
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
                .spawn(async move { library::decode_still(&png, bounds) })
                .await;
            this.update(cx, |this, cx| {
                let page = &mut this.library;
                let Some(image) = image else {
                    return;
                };
                let wanted = page.selection.single() == Some(path.as_path());
                match page.overlay.as_mut().filter(|_| wanted) {
                    Some(overlay) => {
                        let replaced = overlay.still.replace((path, image));
                        page.detail
                            .preview
                            .retired
                            .extend(replaced.map(|(_, image)| image));
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

    pub(in crate::main_window) fn close_library_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(overlay) = self.library.overlay.as_mut().filter(|o| o.is_open()) else {
            return false;
        };
        overlay.closed_at = Some(Instant::now());
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

    pub(in crate::main_window) fn toggle_library_full_screen(
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

    fn wake_overlay_controls(&mut self, cx: &mut Context<Self>) {
        let Some(overlay) = &mut self.library.overlay else {
            return;
        };
        if !overlay.full_screen {
            return;
        }
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
        let Some(key) = preview_key(&event.keystroke, on_overlay) else {
            return;
        };
        if event.is_held && !key.repeats() {
            return;
        }
        cx.stop_propagation();
        let video = self
            .library
            .single_entry()
            .is_some_and(|entry| entry.kind != MediaKind::Screenshot);
        if key.video_only() && !video {
            return;
        }
        match key {
            PreviewKey::Close => {
                self.close_library_overlay(window, cx);
            }
            PreviewKey::Previous => self.step_library_overlay(false, window, cx),
            PreviewKey::Next => self.step_library_overlay(true, window, cx),
            PreviewKey::PlayPause => self.toggle_library_preview(window, cx),
            PreviewKey::Back => self.skip_library_preview(false, window, cx),
            PreviewKey::Forward => self.skip_library_preview(true, window, cx),
            PreviewKey::Mute => self.toggle_library_mute(cx),
            PreviewKey::WholeScreen => self.toggle_library_full_screen(window, cx),
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
                    self.detail_button("library-overlay-export", t!("Export"), false, false)
                        .flex_none()
                        .px(px(12.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pause_library_preview(cx);
                            this.export_library_selection(window, cx);
                        })),
                )
            })
            .child(
                self.detail_button("library-overlay-reveal", t!("Show in folder"), false, false)
                    .flex_none()
                    .px(px(12.))
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
        id: &'static str,
        icon: &'static str,
        label: &'static str,
        key: &'static [&'static str],
        forward: bool,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let reachable = view::neighbour(&self.library.order, &entry.path, forward).is_some();
        self.preview_button(id, icon, STEP_ICON, label, key)
            .when(!reachable, |this| this.opacity(0.5))
            .when(reachable, |this| {
                this.on_click(cx.listener(move |this, _, window, cx| {
                    this.step_library_overlay(forward, window, cx);
                }))
            })
    }

    fn render_overlay_foot(&self, entry: &Entry, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let full_screen = self.library_full_screen();
        let (icon, label) = if full_screen {
            ("icons/minimize.svg", t!("Exit whole screen"))
        } else {
            ("icons/maximize.svg", t!("Whole screen"))
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
                    .child(self.preview_time(duration))
                    .child(self.preview_seek(duration, cx))
                    .child(self.preview_mute_button(cx))
            })
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

    pub(in crate::main_window) fn render_library_overlay(
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
        let picture = self.preview_picture(entry, overlay.still_of(&entry.path), cx);
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
            .on_mouse_move(cx.listener(|this, _: &gpui::MouseMoveEvent, _, cx| {
                this.wake_overlay_controls(cx);
            }));
        Some(
            if full_screen {
                root.child(div().absolute().inset_0().child(picture))
                    .when(controls, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .bg(theme.popover)
                                .child(self.render_overlay_foot(entry, cx)),
                        )
                    })
            } else {
                root.child(self.render_overlay_top(entry, cx))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .items_center()
                            .gap(px(8.))
                            .px(px(12.))
                            .child(self.overlay_step_button(
                                "library-overlay-previous",
                                "icons/chevron-left.svg",
                                t!("Previous item"),
                                &["←"],
                                false,
                                entry,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .rounded(px(6.))
                                    .overflow_hidden()
                                    .child(picture),
                            )
                            .child(self.overlay_step_button(
                                "library-overlay-next",
                                "icons/chevron-right.svg",
                                t!("Next item"),
                                &["→"],
                                true,
                                entry,
                                cx,
                            )),
                    )
                    .child(self.render_overlay_foot(entry, cx))
            }
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use gpui::Keystroke;

    use super::*;

    fn key(text: &'static str, on_overlay: bool) -> Option<PreviewKey> {
        preview_key(&Keystroke::parse(text).unwrap(), on_overlay)
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(width), px(height)),
        }
    }

    #[test]
    fn every_preview_key_has_its_action() {
        assert_eq!(key("left", true), Some(PreviewKey::Previous));
        assert_eq!(key("right", true), Some(PreviewKey::Next));
        assert_eq!(key("k", true), Some(PreviewKey::PlayPause));
        assert_eq!(key("j", true), Some(PreviewKey::Back));
        assert_eq!(key("l", true), Some(PreviewKey::Forward));
        assert_eq!(key("m", true), Some(PreviewKey::Mute));
        assert_eq!(key("f", true), Some(PreviewKey::WholeScreen));
        assert_eq!(key("space", true), Some(PreviewKey::Close));
    }

    #[test]
    fn space_on_a_focused_control_belongs_to_the_control() {
        assert_eq!(key("space", false), None);
        assert_eq!(key("k", false), Some(PreviewKey::PlayPause));
    }

    #[test]
    fn a_shortcut_with_a_modifier_is_left_alone() {
        assert_eq!(key("ctrl-l", true), None);
        assert_eq!(key("ctrl-f", true), None);
    }

    #[test]
    fn only_movement_repeats_while_held() {
        assert!(PreviewKey::Next.repeats());
        assert!(PreviewKey::Forward.repeats());
        assert!(!PreviewKey::WholeScreen.repeats());
        assert!(!PreviewKey::PlayPause.repeats());
        assert!(!PreviewKey::Close.repeats());
    }

    #[test]
    fn a_screenshot_ignores_the_playback_keys_only() {
        assert!(PreviewKey::Mute.video_only());
        assert!(!PreviewKey::WholeScreen.video_only());
        assert!(!PreviewKey::Next.video_only());
    }

    #[test]
    fn the_stage_decodes_at_the_pixels_it_covers() {
        assert_eq!(stage_pixels(true, (1920., 1080.), 1.5), (2880, 1620));
        assert_eq!(
            stage_pixels(false, (1000., 700.), 2.),
            (1824, (560. * 2.) as u32)
        );
    }

    #[test]
    fn a_window_too_small_for_the_chrome_still_asks_for_a_pixel() {
        assert_eq!(stage_pixels(false, (10., 10.), 1.), (1, 1));
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
    fn whole_screen_controls_hide_only_while_playing_and_idle() {
        let start = Instant::now();
        let mut overlay = Overlay::new(None, start);
        assert!(overlay.shows_controls(true, start));
        overlay.full_screen = true;
        overlay.controls_until = start + CONTROLS_HOLD;
        assert!(overlay.shows_controls(true, start));
        assert!(!overlay.shows_controls(true, start + CONTROLS_HOLD));
        assert!(overlay.shows_controls(false, start + CONTROLS_HOLD));
    }
}
