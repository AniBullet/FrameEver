use std::{collections::HashMap, sync::Arc};

use gpui::{App, AppContext as _, Bounds, Context, FocusHandle, FontWeight, Image, ImageFormat, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled, Window, WindowBounds, WindowHandle, WindowKind, WindowOptions, div, img, point, prelude::FluentBuilder as _, px, size, svg};

use super::{Entry, TrayItem};
use crate::{i18n::t, main_window::Mode, platform, theme::Theme};

pub struct TrayPopup {
    focus: FocusHandle,
    entries: Vec<Entry>,
    recent: Option<Vec<Entry>>,
    selected: Option<usize>,
    mode: Mode,
    images: HashMap<Vec<u8>, Arc<Image>>,
    menu_height: f64,
    viewport_height: f32,
    viewport_width: f32,
    recent_on_left: bool,
    export_progress: Option<u8>,
    recording_status: Option<bool>,
    main_scroll: gpui::ScrollHandle,
    recent_scroll: gpui::ScrollHandle,
    warning: Option<String>,
    pub(super) opened_at: Option<std::time::Instant>,
    pub(super) last_render_us: Option<u128>,
}

impl TrayPopup {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::theme::bind_window(window, cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.observe_window_activation(window, |_, window, _| {
            if !window.is_window_active() {
                hide(window);
            }
        }).detach();
        Self { focus, entries: Vec::new(), recent: None, selected: None, mode: Mode::Studio, images: HashMap::new(), warning: None, menu_height: 500., viewport_height: 500., viewport_width: 280., recent_on_left: true, export_progress: None, recording_status: None, main_scroll: gpui::ScrollHandle::new(), recent_scroll: gpui::ScrollHandle::new(), opened_at: None, last_render_us: None }
    }

    pub(super) fn set_entries(&mut self, entries: Vec<Entry>, mode: Mode, warning: Option<String>, state: &super::SurfaceState, window: &mut Window, cx: &mut Context<Self>) {
        let export_progress = match state { super::SurfaceState::Exporting { percent, .. } => Some(*percent), _ => None };
        self.recording_status = match state { super::SurfaceState::Recording { paused, .. } => Some(*paused), _ => None };
        let before = self.visible_entries().to_vec();
        if self.entries != entries { self.images.clear(); }
        self.menu_height = menu_height(&entries, warning.is_some(), export_progress.is_some());
        self.entries = entries;
        self.mode = mode;
        self.warning = warning;
        self.export_progress = export_progress;
        if self.recent.is_some() {
            self.recent = recent_entries(&self.entries);
        }
        self.selected = refreshed_selection(&before, self.selected, self.visible_entries());
        cache_images(&self.entries, &mut self.images);
        self.fit(window, cx);
        self.reveal_selection();
        cx.notify();
    }
    pub(super) fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.recent = None;
        self.selected = selectable(&self.entries).first().copied();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn visible_entries(&self) -> &[Entry] {
        self.recent.as_deref().unwrap_or(&self.entries)
    }

    fn change_mode(&mut self, direction: i32, cx: &mut Context<Self>) {
        let modes = [Mode::Studio, Mode::Instant, Mode::Screenshot];
        let current = modes.iter().position(|mode| *mode == self.mode).unwrap_or(0) as i32;
        self.mode = modes[(current + direction).rem_euclid(3) as usize];
        let item = match self.mode { Mode::Studio => TrayItem::ModeStudio, Mode::Instant => TrayItem::ModeInstant, Mode::Screenshot => TrayItem::ModeScreenshot };
        cx.defer(move |cx| super::handle_item(item, cx));
    }

    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.visible_entries().get(index).cloned() else { return };
        match entry {
            Entry::Submenu { items, .. } if !is_modes(&entry) => {
                self.recent = Some(items);
                self.selected = selectable(self.visible_entries()).first().copied();
                cx.notify();
            }
            Entry::Item { item: Some(item), enabled: true, .. } => {
                hide(window);
                cx.defer(move |cx| super::handle_item(item, cx));
            }
            _ => {}
        }
    }

    fn key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match key {
            "escape" | "left" if self.recent.is_some() => { self.recent = None; self.selected = self.entries.iter().position(|entry| matches!(entry, Entry::Submenu { .. }) && !is_modes(entry)); cx.notify(); }
            "escape" => hide(window),
            "up" | "down" | "tab" => {
                self.selected = next_selection(self.visible_entries(), self.selected, if key == "up" { -1 } else { 1 });
                cx.notify();
            }
            "home" => { self.selected = selectable(self.visible_entries()).first().copied(); cx.notify(); }
            "end" => { self.selected = selectable(self.visible_entries()).last().copied(); cx.notify(); }
            "left" | "right" if self.selected.and_then(|i| self.visible_entries().get(i)).is_some_and(is_modes) => self.change_mode(if key == "left" { -1 } else { 1 }, cx),
            "right" | "enter" | "space" => { if let Some(index) = self.selected && self.visible_entries().get(index).is_some_and(|entry| key_activates(key, entry)) { self.choose(index, window, cx); } }
            _ => return false,
        }
        true
    }
}

fn cache_images(entries: &[Entry], images: &mut HashMap<Vec<u8>, Arc<Image>>) {
    for entry in entries {
        match entry {
            Entry::Item { icon: Some(icon), .. } => { images.entry(icon.clone()).or_insert_with(|| Arc::new(Image::from_bytes(ImageFormat::Png, icon.clone()))); }
            Entry::Submenu { items, .. } => cache_images(items, images),
            _ => {}
        }
    }
}

pub(super) fn is_modes(entry: &Entry) -> bool {
    matches!(entry, Entry::Submenu { items, .. } if items.iter().all(|entry| matches!(entry, Entry::Item { item: Some(TrayItem::ModeStudio | TrayItem::ModeInstant | TrayItem::ModeScreenshot), .. })))
}

fn action(entry: &Entry) -> Option<TrayItem> {
    match entry { Entry::Item { item, .. } => item.clone(), _ => None }
}

fn selectable(entries: &[Entry]) -> Vec<usize> {
    entries.iter().enumerate().filter_map(|(index, entry)| match entry {
        Entry::Item { enabled: true, item: Some(_), .. } | Entry::Submenu { enabled: true, .. } => Some(index),
        _ => None,
    }).collect()
}

fn refreshed_selection(before: &[Entry], selected: Option<usize>, after: &[Entry]) -> Option<usize> {
    let previous = selected.and_then(|index| before.get(index));
    previous.and_then(|previous| after.iter().position(|candidate| match (action(previous), action(candidate)) {
        (Some(previous), Some(candidate)) => previous == candidate,
        _ if is_modes(previous) => is_modes(candidate),
        _ => matches!((previous, candidate), (Entry::Submenu { .. }, Entry::Submenu { .. })) && !is_modes(candidate),
    })).or_else(|| selectable(after).first().copied())
}

fn recent_entries(entries: &[Entry]) -> Option<Vec<Entry>> {
    entries.iter().find_map(|entry| match entry { Entry::Submenu { enabled: true, items, .. } if !is_modes(entry) => Some(items.clone()), _ => None })
}

pub(super) fn menu_height(entries: &[Entry], warning: bool, export_progress: bool) -> f64 {
    10. + entries.iter().map(|entry| if matches!(entry, Entry::Separator) { 9. } else { 34. }).sum::<f64>() + if warning { 48. } else { 0. } + if export_progress { 12. } else { 0. }
}
pub(super) fn next_selection(entries: &[Entry], selected: Option<usize>, direction: i32) -> Option<usize> {
    let rows = selectable(entries);
    if rows.is_empty() { return None }
    let index = selected.and_then(|selected| rows.iter().position(|row| *row == selected));
    Some(rows[match index { Some(index) => (index as i32 + direction).rem_euclid(rows.len() as i32) as usize, None if direction < 0 => rows.len() - 1, None => 0 }])
}

impl TrayPopup {
    fn reveal_selection(&self) {
        if let Some(index) = self.selected { if self.recent.is_some() { self.recent_scroll.scroll_to_item(index + usize::from(self.viewport_width < 560.)); } else { self.main_scroll.scroll_to_item(index + usize::from(self.warning.is_some())); } }
    }
    fn fit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let width = if self.recent.is_some() { 564. } else { 280. };
        let height = self.menu_height;
        match super::windows::fit_popup(window, width, height, cx) {
            Ok(on_left) => self.recent_on_left = on_left,
            Err(error) => tracing::error!(%error, "tray submenu positioning failed"),
        }
    }

    fn pane(&mut self, entries: &[Entry], recent: bool, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut pane = div().id(if recent { "tray-recent" } else { "tray-menu" }).w(px(280.)).max_w(px(self.viewport_width)).flex_shrink_0().flex().flex_col().p(px(4.)).bg(theme.popover).text_color(theme.popover_foreground).rounded(px(8.)).border_1().border_color(theme.border).max_h(px(self.viewport_height)).overflow_y_scroll().track_scroll(if recent { &self.recent_scroll } else { &self.main_scroll });
        if recent && self.viewport_width < 560. {
            pane = pane.child(div().id("tray-recent-back").h(px(34.)).flex().items_center().gap(px(8.)).px(px(8.)).child(svg().path("icons/chevron-left.svg").size(px(14.)).text_color(theme.muted_foreground)).child(t!("Recent")).on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| { this.recent = None; this.selected = this.entries.iter().position(|entry| matches!(entry, Entry::Submenu { .. }) && !is_modes(entry)); this.fit(window, cx); cx.notify(); })));
        }
        if (!recent || self.viewport_width < 560.) && let Some(warning) = &self.warning {
            pane = pane.child(div().px(px(8.)).py(px(6.)).text_size(px(11.)).text_color(theme.warning_text).child(warning.clone()));
        }
        for (index, entry) in entries.iter().enumerate() {
            if matches!(entry, Entry::Separator) { pane = pane.child(div().h(px(1.)).my(px(4.)).mx(px(4.)).bg(theme.border)); continue }
            let selected = self.recent.is_some() == recent && self.selected == Some(index);
            if is_modes(entry) {
                let mut segments = div().id(("tray-mode", index)).h(px(34.)).flex().items_center().gap(px(2.)).p(px(2.)).rounded(px(5.)).bg(theme.secondary).when(selected, |el| el.border_1().border_color(theme.ring));
                for (mode, title, item) in [(Mode::Studio, t!("Studio"), TrayItem::ModeStudio), (Mode::Instant, t!("Instant"), TrayItem::ModeInstant), (Mode::Screenshot, t!("Screenshot"), TrayItem::ModeScreenshot)] {
                    segments = segments.child(div().id(("tray-mode-segment", mode as usize)).flex_1().h_full().flex().items_center().justify_center().rounded(px(4.)).when(mode == self.mode, |el| el.bg(theme.background)).child(title).on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, window, cx| { this.mode = mode; this.recent = None; this.selected = Some(index); this.fit(window, cx); let item = item.clone(); cx.defer(move |cx| super::handle_item(item, cx)); })));
                }
                pane = pane.child(segments);
                continue;
            }
            let (title, icon, enabled, item, accelerator, submenu) = match entry {
                Entry::Item { title, icon, enabled, item, accelerator } => (title, icon, *enabled, item.as_ref(), accelerator.as_ref(), false),
                Entry::Submenu { title, icon, enabled, .. } => (title, icon, *enabled, None, None, true),
                Entry::Separator => continue,
            };
            let mut row = div().id((if recent { "tray-recent-row" } else { "tray-row" }, index)).h(px(34.)).flex_shrink_0().px(px(8.)).flex().items_center().gap(px(8.)).rounded(px(4.)).when(selected && enabled, |el| el.bg(theme.accent)).when(!enabled, |el| el.text_color(theme.muted_foreground));
            if !enabled && let Some(paused) = self.recording_status {
                row = row.child(div().size(px(8.)).flex_shrink_0().rounded_full().bg(if paused { theme.warning } else { theme.recording }));
            }
            if recent && let Some(image) = icon.as_ref().and_then(|icon| self.images.get(icon)) {
                row = row.child(img(image.clone()).size(px(24.)).rounded(px(3.)));
            } else if let Some(path) = item.and_then(item_icon) {
                row = row.child(svg().path(path).size(px(16.)).text_color(theme.foreground));
            } else if submenu {
                row = row.child(svg().path("icons/film.svg").size(px(16.)).text_color(theme.foreground));
            }
            row = row.child(div().flex_1().overflow_hidden().text_ellipsis().when(matches!(item, Some(TrayItem::OpenCap | TrayItem::StopRecording | TrayItem::CancelCountdown | TrayItem::ResumeRecording | TrayItem::RequestPermissions)), |el| el.font_weight(FontWeight::SEMIBOLD)).child(title.clone()));
            if submenu { row = row.child(svg().path("icons/chevron-right.svg").size(px(14.)).text_color(theme.muted_foreground)); }
            if matches!(item, Some(TrayItem::Quit)) { row = row.child(div().text_size(px(11.)).text_color(theme.muted_foreground).child(SharedString::from(crate::menus::app_version()))); }
            if let Some(accelerator) = accelerator {
                for key in accelerator_keys(accelerator) {
                    row = row.child(div().px(px(3.)).h(px(18.)).border_1().border_color(theme.border).rounded(px(3.)).text_size(px(10.)).text_color(theme.muted_foreground).child(key));
                }
            }
            if enabled {
                row = row.on_mouse_move(cx.listener(move |this, _, _, cx| { if this.recent.is_some() == recent && this.selected != Some(index) { this.selected = Some(index); cx.notify(); } })).on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, window, cx| { if !recent { this.recent = None; } this.choose(index, window, cx); this.fit(window, cx); }));
            }
            pane = pane.child(row);
            if matches!(item, Some(TrayItem::ShowExport)) && let Some(percent) = self.export_progress {
                pane = pane.child(div().h(px(12.)).px(px(8.)).pb(px(8.)).child(div().h(px(4.)).rounded(px(2.)).bg(theme.component.slider_bar).child(div().h_full().w(gpui::relative(f32::from(percent.min(100)) / 100.)).rounded(px(2.)).bg(theme.component.progress_bar))));
            }
        }
        pane.into_any_element()
    }
}

impl Render for TrayPopup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(opened) = self.opened_at.take() { let elapsed = opened.elapsed().as_micros(); self.last_render_us = Some(elapsed); tracing::info!(elapsed_us = elapsed, "tray popup first render"); }
        self.viewport_height = f32::from(window.viewport_size().height);
        self.viewport_width = f32::from(window.viewport_size().width);
        let theme = Theme::for_window(window, cx, false);
        let entries = self.entries.clone();
        let recent = self.recent.clone();
        let mut root = div().id("tray-popup").track_focus(&self.focus).size_full().flex().items_end().gap(px(4.)).font_family(crate::theme::ui_font()).text_size(px(13.)).on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
            if event.keystroke.key == "tab" && event.keystroke.modifiers.shift {
                this.selected = next_selection(this.visible_entries(), this.selected, -1); this.reveal_selection(); cx.notify(); cx.stop_propagation();
            } else if this.key(&event.keystroke.key, window, cx) { this.fit(window, cx); this.reveal_selection(); cx.stop_propagation(); }
        }));
        if let Some(recent) = &recent && self.viewport_width < 560. {
            return root.justify_end().child(self.pane(recent, true, &theme, cx));
        }
        if self.recent_on_left {
            if let Some(recent) = recent { root = root.child(self.pane(&recent, true, &theme, cx)); }
            root.child(self.pane(&entries, false, &theme, cx))
        } else {
            root = root.child(self.pane(&entries, false, &theme, cx));
            if let Some(recent) = recent { root = root.child(self.pane(&recent, true, &theme, cx)); }
            root
        }
    }
}

fn accelerator_keys(accelerator: &super::Accelerator) -> Vec<String> {
    use global_hotkey::hotkey::Modifiers;
    let mut keys = Vec::new();
    for (modifier, name) in [(Modifiers::CONTROL, "Ctrl"), (Modifiers::ALT, "Alt"), (Modifiers::SHIFT, "Shift"), (Modifiers::SUPER, "Win")] {
        if accelerator.modifiers.contains(modifier) { keys.push(name.into()); }
    }
    keys.push(accelerator.code.to_string().trim_start_matches("Key").trim_start_matches("Digit").to_string());
    keys
}
fn item_icon(item: &TrayItem) -> Option<&'static str> {
    Some(match item { TrayItem::RecordDisplay | TrayItem::ScreenshotDisplay => "icons/monitor.svg", TrayItem::RecordWindow | TrayItem::ScreenshotWindow => "icons/app-window.svg", TrayItem::RecordArea | TrayItem::ScreenshotArea => "icons/scan.svg", TrayItem::TakeScreenshot => "icons/image.svg", TrayItem::ImportMedia => "icons/download.svg", TrayItem::OpenSettings => "icons/settings.svg", TrayItem::StopRecording => "icons/square.svg", TrayItem::PauseRecording => "icons/pause.svg", TrayItem::ResumeRecording => "icons/play.svg", TrayItem::RestartRecording | TrayItem::RecordLastArea | TrayItem::ScreenshotLastArea => "icons/rotate-ccw.svg", TrayItem::ToggleControls => "icons/eye-off.svg", TrayItem::CancelCountdown => "icons/x.svg", _ => return None })
}

pub(super) fn hide(window: &mut Window) {
    if let Some(native) = platform::native_window(window) { platform::hide_native(&native); }
}

pub(super) fn create(cx: &mut App) -> anyhow::Result<WindowHandle<TrayPopup>> {
    cx.open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(280.), px(500.)) })),
        titlebar: None, kind: WindowKind::PopUp, focus: false, show: false, is_resizable: false, is_minimizable: false,
        window_background: gpui::WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }, |window, cx| cx.new(|cx| TrayPopup::new(window, cx)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_keyboard_wraps_between_action_rows_skipping_status_and_separators() {
        let entries = vec![Entry::disabled("Recording"), Entry::Separator, Entry::item("Stop", TrayItem::StopRecording), Entry::item("Pause", TrayItem::PauseRecording)];
        assert_eq!(next_selection(&entries, None, 1), Some(2));
        assert_eq!(next_selection(&entries, Some(2), -1), Some(3));
        assert_eq!(next_selection(&entries, Some(3), 1), Some(2));
        assert_eq!(next_selection(&[], None, 1), None);
    }
}
#[cfg(test)]
mod refresh_tests {
    use super::*;

    #[test]
    fn tray_mode_keyboard_keeps_mode_row_selected_after_refresh() {
        let studio = super::super::build_surface_menu(Mode::Studio, &[], "1.0.0");
        let instant = super::super::build_surface_menu(Mode::Instant, &[], "1.0.0");
        assert_eq!(refreshed_selection(&studio, Some(3), &instant), Some(3));
    }
}
fn key_activates(key: &str, entry: &Entry) -> bool { matches!(key, "enter" | "space") || key == "right" && matches!(entry, Entry::Submenu { enabled: true, .. }) }

#[cfg(test)]
mod activation_tests {
    use super::*;

    #[test]
    fn tray_right_arrow_never_activates_stop_or_other_action_rows() {
        let stop = Entry::item("Stop", TrayItem::StopRecording);
        assert!(!key_activates("right", &stop));
        assert!(key_activates("enter", &stop));
    }
}
#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn tray_recent_disappears_when_live_recording_replaces_idle_actions() {
        let previous = vec![super::super::PreviousItem { path: "demo.ever".into(), kind: crate::library::MediaKind::Screenshot, pretty_name: "Demo".into(), thumbnail: None }];
        let idle = super::super::build_surface_menu(Mode::Studio, &previous, "1.0.0");
        assert!(recent_entries(&idle).is_some());
        let recording = super::super::build_state_menu(Mode::Studio, &previous, "1.0.0", &super::super::SurfaceState::Recording { paused: false, elapsed: 1, controls_hidden: false }, false);
        assert!(recent_entries(&recording).is_none());
        assert_eq!(menu_height(&recording, false, false), 198.);
    }

    #[test]
    fn tray_progress_and_remote_notice_are_included_in_window_height() {
        let entries = vec![Entry::item("Export", TrayItem::ShowExport), Entry::Separator, Entry::item("Quit", TrayItem::Quit)];
        assert_eq!(menu_height(&entries, true, true), 147.);
    }
}
