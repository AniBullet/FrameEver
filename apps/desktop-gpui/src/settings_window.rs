use std::{
    cell::Cell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AppContext as _, Bounds, Context, Entity, FocusHandle, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Pixels, Point, Render, ScrollAnchor,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div, img,
    prelude::FluentBuilder, px, svg, transparent_black,
};
use serde_json::Value;

use crate::{
    devices::WindowOption,
    doodle::{self, Doodle},
    i18n::{n, t, tr},
    main_window::Mode,
    onboarding_doodles,
    store::{
        self, AppTheme, DEFAULT_PROJECT_NAME_TEMPLATE, GENERAL_SETTINGS, GeneralSettings,
        MainWindowStartBehaviour, PostDeletionBehaviour, PostInstantBehaviour,
        PostScreenshotBehaviour, PostStudioBehaviour, RECORDING_START_SAFETY, SettingsEnum,
        StudioQuality, UpdateChannel, UpdateDownloadPolicy, WindowExclusion,
    },
    theme::Theme,
    ui,
};

pub mod catalog;

pub use catalog::{Page, Row};

pub const SETTINGS_WIDTH: f32 = 880.;
pub const SETTINGS_HEIGHT: f32 = 640.;
pub const SETTINGS_MIN_WIDTH: f32 = 720.;
pub const SETTINGS_MIN_HEIGHT: f32 = 520.;

pub const TRAFFIC_LIGHTS: Point<Pixels> = Point {
    x: px(22.),
    y: px(15.),
};

pub const SETTINGS_MATERIAL_RADIUS: f64 = 16.;

const SIDEBAR_WIDTH: f32 = 216.;
const TITLE_BAR_HEIGHT: f32 = 36.;
const CONTENT_PADDING_X: f32 = 24.;
const FLASH_FOR: Duration = Duration::from_millis(1300);

thread_local! {
    static LAST_PAGE: Cell<Page> = const { Cell::new(Page::General) };
}

pub fn last_page() -> Page {
    LAST_PAGE.with(Cell::get)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuKind {
    Countdown,
    MainWindowStart,
    PostDeletion,
    MaxFps,
    AfterScreenshot,
    AfterStudio,
    AfterInstant,
    AddWindow,
    AutomationTrigger(usize),
    AutomationMatchMode(usize),
    AutomationConditionType(usize, usize),
    AutomationConditionTarget(usize, usize),
    AutomationConditionMode(usize, usize),
    AutomationActionType(usize, usize),
    AutomationClipboardSource(usize, usize),
    AutomationExportFormat(usize, usize),
    AutomationExportResolution(usize, usize),
    AutomationExportFps(usize, usize),
    AutomationExportCompression(usize, usize),
    AutomationPreset(usize, usize),
}

struct OpenMenu {
    kind: MenuKind,
    state: ui::MenuState,
}

const MAX_FPS_OPTIONS: &[(u32, &str)] = &[
    (24, n!("24 FPS")),
    (25, n!("25 FPS")),
    (30, n!("30 FPS")),
    (60, n!("60 FPS (Recommended)")),
    (120, n!("120 FPS")),
];

const COUNTDOWN_OPTIONS: &[(u32, &str)] = &[
    (0, n!("Off")),
    (3, n!("3 seconds")),
    (5, n!("5 seconds")),
    (10, n!("10 seconds")),
];

const STUDIO_QUALITY_TIERS: &[(StudioQuality, &str, &str)] = &[
    (
        StudioQuality::Balanced,
        n!("Balanced"),
        n!("Sharp at a sensible size. Recommended."),
    ),
    (
        StudioQuality::Compatibility,
        n!("Smaller files"),
        n!("Saves disk space; fine detail can soften."),
    ),
    (
        StudioQuality::Ultra,
        n!("Maximum detail"),
        n!("For heavy editing; uses the most space."),
    ),
];

const INSTANT_RESOLUTION_TIERS: &[(u32, &str)] = &[
    (1280, "720p"),
    (1920, "1080p"),
    (2560, "1440p"),
    (3840, "4K"),
];

const PROJECT_NAME_TOKENS: &[&str] = &[
    "{recording_mode}",
    "{target_name}",
    "{date}",
    "{time}",
    "{random}",
];

fn theme_preview(theme: AppTheme) -> &'static str {
    match theme {
        AppTheme::System => "images/auto.jpg",
        AppTheme::Light => "images/light.jpg",
        AppTheme::Dark => "images/dark.jpg",
    }
}

fn hides_label(row: Row) -> bool {
    matches!(
        row,
        Row::StorageFolder | Row::NameTemplate | Row::ExcludedWindows | Row::Licences
    )
}

fn is_stacked(row: Row) -> bool {
    matches!(
        row,
        Row::StudioQuality
            | Row::StorageFolder
            | Row::NameTemplate
            | Row::ExcludedWindows
            | Row::Licences
            | Row::RememberedTerms
            | Row::Cli
            | Row::Diagnostic
    )
}

fn is_bare(row: Row) -> bool {
    matches!(row, Row::Rules | Row::Version | Row::Changelog)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    ProjectName,
}

pub struct SettingsWindow {
    pub(crate) theme: Theme,
    pub(crate) page: Page,
    pub(crate) settings: GeneralSettings,
    menu: Option<OpenMenu>,
    page_scroll: ScrollHandle,
    jump_anchor: ScrollAnchor,
    jump_pending: bool,
    flash: Option<Row>,
    flash_task: Option<gpui::Task<()>>,
    instant_quality_save_failed: bool,
    studio_quality_save_failed: bool,
    pub(crate) pages: crate::settings_pages::PagesState,
    project_name: String,
    project_name_input: Entity<ui::TextInputState>,
    search_input: Entity<ui::TextInputState>,
    search: String,
    _field_events: [gpui::Subscription; 2],
    placeholders: ui::CollapsibleState,
    placeholders_task: Option<gpui::Task<()>>,
    changelog_all: bool,
    slider_track: Rc<Cell<Option<Bounds<Pixels>>>>,
    slider_dragging: bool,
    windows: Vec<WindowOption>,
    opened_at: Instant,
    pub(crate) focus: FocusHandle,
}

impl SettingsWindow {
    pub fn new(page: Page, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::theme::bind_window(window, cx);
        let theme = Theme::for_window(window, cx, true);
        let settings = GeneralSettings::load();

        window.on_window_should_close(cx, |_window, cx| {
            cx.defer(crate::app_windows::settings_closed);
            true
        });

        let stored_template = settings
            .default_project_name_template
            .clone()
            .unwrap_or_else(|| DEFAULT_PROJECT_NAME_TEMPLATE.to_string());
        let project_name_input = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_text(stored_template.clone(), cx);
            input
        });
        let search_input = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_placeholder(t!("Search settings  Ctrl+F"));
            input
        });
        let field_events = [
            cx.subscribe(&project_name_input, |this, input, event, cx| {
                this.on_field_event(Field::ProjectName, input, event, cx)
            }),
            cx.subscribe_in(&search_input, window, |this, input, event, window, cx| {
                this.on_search_event(input, event, window, cx)
            }),
        ];

        let pages = crate::settings_pages::PagesState::new(window, cx);
        let page_scroll = ScrollHandle::new();
        LAST_PAGE.with(|last| last.set(page));

        Self {
            theme,
            page,
            pages,
            project_name: stored_template,
            settings,
            menu: None,
            jump_anchor: ScrollAnchor::for_handle(page_scroll.clone()),
            page_scroll,
            jump_pending: false,
            flash: None,
            flash_task: None,
            instant_quality_save_failed: false,
            studio_quality_save_failed: false,
            project_name_input,
            search_input,
            search: String::new(),
            _field_events: field_events,
            placeholders: ui::CollapsibleState::new(false),
            placeholders_task: None,
            changelog_all: false,
            slider_track: Rc::new(Cell::new(None)),
            slider_dragging: false,
            windows: Vec::new(),
            opened_at: Instant::now(),
            focus: cx.focus_handle(),
        }
    }

    pub fn start_enumeration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let windows = cx
                .background_executor()
                .spawn(async { crate::devices::DeviceSnapshot::enumerate().windows })
                .await;
            this.update(cx, |this, cx| {
                this.windows = windows;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn focus_root(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
    }

    pub fn set_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != page {
            self.page_scroll = ScrollHandle::new();
            self.jump_anchor = ScrollAnchor::for_handle(self.page_scroll.clone());
        }
        self.instant_quality_save_failed = false;
        self.studio_quality_save_failed = false;
        self.shortcuts_cancel(cx);
        self.page = page;
        LAST_PAGE.with(|last| last.set(page));
        self.menu = None;
        self.settings = GeneralSettings::load();
        self.page_shown(window, cx);
        cx.notify();
    }

    pub fn show_quality_settings(
        &mut self,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match mode {
            Mode::Studio => self.jump_to(Row::StudioQuality, window, cx),
            Mode::Instant => self.jump_to(Row::InstantResolution, window, cx),
            Mode::Screenshot => {}
        }
    }

    pub fn page_shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pages_shown(window, cx);
    }

    fn jump_to(&mut self, row: Row, window: &mut Window, cx: &mut Context<Self>) {
        let Some(place) = catalog::place_of(row) else {
            return;
        };
        self.clear_search(cx);
        if self.page != place.page {
            self.set_page(place.page, window, cx);
        }
        self.flash = Some(row);
        self.jump_pending = true;
        self.flash_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH_FOR).await;
            this.update(cx, |this, cx| {
                if this.flash == Some(row) {
                    this.flash = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn clear_search(&mut self, cx: &mut Context<Self>) {
        if self.search.is_empty() {
            return;
        }
        self.search.clear();
        let input = self.search_input.clone();
        input.update(cx, |input, cx| input.set_text("", cx));
        cx.notify();
    }

    fn on_search_event(
        &mut self,
        input: &Entity<ui::TextInputState>,
        event: &ui::TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ui::TextInputEvent::Changed => {
                self.search = input.read(cx).text().to_string();
                self.menu = None;
                cx.notify();
            }
            ui::TextInputEvent::Confirmed => {
                if let Some(first) = catalog::search(&self.search).first() {
                    let row = first.row;
                    self.jump_to(row, window, cx);
                    self.focus_root(window, cx);
                }
            }
            ui::TextInputEvent::Cancelled => self.clear_search(cx),
            _ => {}
        }
    }

    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.search_input.clone();
        input.update(cx, |input, cx| input.focus_and_select_all(window, cx));
    }

    pub(crate) fn doodle(&self, art: &'static Doodle, width: f32, cx: &gpui::App) -> AnyElement {
        doodle::doodle_once(
            art,
            doodle::Palette::new(&self.theme),
            self.opened_at,
            cx.reduce_motion(),
            width,
        )
        .into_any_element()
    }

    fn sync_appearance(&mut self, window: &Window, cx: &gpui::App) {
        self.theme.refresh(window, cx, true);
    }

    pub(crate) fn write(&mut self, key: &'static str, value: Value, cx: &mut Context<Self>) {
        if !store::set_store_setting(GENERAL_SETTINGS, key, value) {
            self.settings = GeneralSettings::load();
        }
        cx.notify();
    }

    pub(crate) fn write_bool(&mut self, key: &'static str, value: bool, cx: &mut Context<Self>) {
        self.write(key, Value::Bool(value), cx);
    }

    fn write_u32(&mut self, key: &'static str, value: u32, cx: &mut Context<Self>) {
        self.write(key, Value::from(value), cx);
    }

    pub(crate) fn write_enum<T: SettingsEnum>(
        &mut self,
        key: &'static str,
        value: T,
        cx: &mut Context<Self>,
    ) {
        self.write(key, Value::String(value.as_json().to_string()), cx);
    }

    fn write_excluded(&mut self, windows: Vec<WindowExclusion>, cx: &mut Context<Self>) {
        self.settings.excluded_windows = windows;
        let json = store::excluded_windows_to_json(&self.settings.excluded_windows);
        self.write("excludedWindows", json, cx);
    }

    fn write_storage(&mut self, cx: &mut Context<Self>) {
        for (key, value) in self.settings.storage.general_settings_entries() {
            self.write(key, value, cx);
        }
    }

    fn toggle_placeholders(&mut self, cx: &mut Context<Self>) {
        self.placeholders.toggle();
        self.placeholders_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let more = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        this.placeholders.is_animating()
                    })
                    .unwrap_or(false);
                if !more {
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(8))
                    .await;
            }
        }));
        cx.notify();
    }

    pub(crate) fn open_menu(
        &mut self,
        kind: MenuKind,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self.focus.clone();
        window.focus(&focus, cx);
        let items = self.menu_items(kind);
        self.menu = Some(OpenMenu {
            kind,
            state: ui::MenuState::new(origin, &items),
        });
        cx.notify();
    }

    fn menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(menu) = self.menu.as_mut() else {
            return false;
        };
        let kind = menu.kind;
        match menu.state.on_key(key) {
            ui::MenuKey::Moved => {
                cx.notify();
                true
            }
            ui::MenuKey::Commit(index) => {
                self.choose(kind, index, window, cx);
                true
            }
            ui::MenuKey::Dismiss => {
                self.menu = None;
                cx.notify();
                true
            }
            ui::MenuKey::Ignored => false,
        }
    }

    fn menu_items(&self, kind: MenuKind) -> Vec<ui::MenuItem> {
        match kind {
            MenuKind::Countdown => {
                let current = self.settings.recording_countdown.unwrap_or(0);
                COUNTDOWN_OPTIONS
                    .iter()
                    .map(|(value, label)| ui::MenuItem::new(tr(label), *value == current))
                    .collect()
            }
            MenuKind::MaxFps => MAX_FPS_OPTIONS
                .iter()
                .map(|(value, label)| ui::MenuItem::new(tr(label), *value == self.settings.max_fps))
                .collect(),
            MenuKind::MainWindowStart => {
                enum_items(self.settings.main_window_recording_start_behaviour)
            }
            MenuKind::PostDeletion => enum_items(self.settings.post_deletion_behaviour),
            MenuKind::AfterScreenshot => enum_items(self.settings.post_screenshot_behaviour),
            MenuKind::AfterStudio => enum_items(self.settings.post_studio_recording_behaviour),
            MenuKind::AfterInstant => enum_items(self.settings.post_instant_recording_behaviour),
            MenuKind::AddWindow => self
                .add_window_options()
                .into_iter()
                .map(|(label, _)| ui::MenuItem::new(label, false))
                .collect(),
            kind => self.pages_menu_items(kind),
        }
    }

    fn choose(
        &mut self,
        kind: MenuKind,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        match kind {
            MenuKind::Countdown => {
                if let Some((value, _)) = COUNTDOWN_OPTIONS.get(index) {
                    self.settings.recording_countdown = Some(*value);
                    self.write_u32("recordingCountdown", *value, cx);
                }
            }
            MenuKind::MaxFps => {
                if let Some((value, _)) = MAX_FPS_OPTIONS.get(index) {
                    self.settings.max_fps = *value;
                    self.write_u32("maxFps", *value, cx);
                }
            }
            MenuKind::MainWindowStart => {
                if let Some(value) = MainWindowStartBehaviour::ALL.get(index) {
                    self.settings.main_window_recording_start_behaviour = *value;
                    self.write_enum("mainWindowRecordingStartBehaviour", *value, cx);
                }
            }
            MenuKind::PostDeletion => {
                if let Some(value) = PostDeletionBehaviour::ALL.get(index) {
                    self.settings.post_deletion_behaviour = *value;
                    self.write_enum("postDeletionBehaviour", *value, cx);
                }
            }
            MenuKind::AfterScreenshot => {
                if let Some(value) = PostScreenshotBehaviour::ALL.get(index) {
                    self.settings.post_screenshot_behaviour = *value;
                    self.write_enum("postScreenshotBehaviour", *value, cx);
                }
            }
            MenuKind::AfterStudio => {
                if let Some(value) = PostStudioBehaviour::ALL.get(index) {
                    self.settings.post_studio_recording_behaviour = *value;
                    self.write_enum("postStudioRecordingBehaviour", *value, cx);
                }
            }
            MenuKind::AfterInstant => {
                if let Some(value) = PostInstantBehaviour::ALL.get(index) {
                    self.settings.post_instant_recording_behaviour = *value;
                    self.write_enum("postInstantRecordingBehaviour", *value, cx);
                }
            }
            MenuKind::AddWindow => {
                let Some((_, rule)) = self.add_window_options().into_iter().nth(index) else {
                    return;
                };
                let mut next = self.settings.excluded_windows.clone();
                next.push(rule);
                self.write_excluded(next, cx);
            }
            kind => self.pages_choose(kind, index, window, cx),
        }
        cx.notify();
    }

    fn add_window_options(&self) -> Vec<(String, WindowExclusion)> {
        if !store::OTHER_APP_WINDOWS_EXCLUDABLE {
            return self
                .missing_default_exclusions()
                .into_iter()
                .map(|rule| (rule.primary_label().to_string(), rule))
                .collect();
        }
        self.available_windows()
            .into_iter()
            .map(|window| {
                let label = window_option_label(&window);
                let rule = WindowExclusion {
                    bundle_identifier: None,
                    owner_name: Some(window.app),
                    window_title: Some(window.label),
                };
                (label, rule)
            })
            .collect()
    }

    fn available_windows(&self) -> Vec<WindowOption> {
        self.windows
            .iter()
            .filter(|window| {
                !self
                    .settings
                    .excluded_windows
                    .iter()
                    .any(|exclusion| matches_exclusion(exclusion, window))
            })
            .cloned()
            .collect()
    }

    fn missing_default_exclusions(&self) -> Vec<WindowExclusion> {
        store::default_excluded_windows()
            .into_iter()
            .filter(|default| {
                !self
                    .settings
                    .excluded_windows
                    .iter()
                    .any(|entry| covers_default_exclusion(entry, default))
            })
            .collect()
    }

    fn on_root_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        if self.menu_key(&keystroke.key, window, cx) {
            return;
        }
        if self.shortcuts_capture_key(keystroke, cx) {
            return;
        }
        let modifiers = keystroke.modifiers;
        let command = modifiers.control || modifiers.platform;
        if command && keystroke.key == "w" {
            cx.defer(crate::app_windows::close_settings);
        } else if command && keystroke.key == "f" {
            self.focus_search(window, cx);
        } else if keystroke.key == "escape" {
            self.clear_search(cx);
        }
    }
}

fn enum_items<T: SettingsEnum>(current: T) -> Vec<ui::MenuItem> {
    T::ALL
        .iter()
        .map(|value| ui::MenuItem::new(value.label(), *value == current))
        .collect()
}

fn window_option_label(window: &WindowOption) -> String {
    if window.label.is_empty() || window.label == window.app {
        window.app.clone()
    } else {
        format!("{} • {}", window.app, window.label)
    }
}

fn matches_exclusion(exclusion: &WindowExclusion, window: &WindowOption) -> bool {
    let owner_match = exclusion
        .owner_name
        .as_ref()
        .is_some_and(|owner| *owner == window.app);

    if let (Some(_), Some(title)) = (&exclusion.owner_name, &exclusion.window_title) {
        return owner_match && *title == window.label;
    }
    if owner_match {
        return true;
    }
    exclusion
        .window_title
        .as_ref()
        .is_some_and(|title| *title == window.label)
}

fn covers_default_exclusion(entry: &WindowExclusion, default: &WindowExclusion) -> bool {
    if entry == default {
        return true;
    }
    if default.window_title.is_some() && entry.window_title == default.window_title {
        return true;
    }
    if default.bundle_identifier.is_some() && entry.bundle_identifier == default.bundle_identifier {
        return true;
    }
    if default.owner_name.is_some() && entry.owner_name == default.owner_name {
        return entry.window_title.is_none() || entry.window_title == default.window_title;
    }
    false
}

gpui::actions!(settings_window, [FocusNext, FocusPrevious]);

pub fn bind_keys(cx: &mut gpui::App) {
    let context = Some("SettingsWindow");
    cx.bind_keys([
        gpui::KeyBinding::new("tab", FocusNext, context),
        gpui::KeyBinding::new("shift-tab", FocusPrevious, context),
    ]);
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_appearance(window, cx);
        let theme = self.theme;
        if self.jump_pending {
            self.jump_pending = false;
            self.jump_anchor.scroll_to(window, cx);
            window.request_animation_frame();
        }

        let shell = div()
            .track_focus(&self.focus)
            .key_context("SettingsWindow")
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                this.on_root_key(event, window, cx);
            }))
            .on_modifiers_changed(cx.listener(
                |this, event: &gpui::ModifiersChangedEvent, _window, cx| {
                    this.shortcuts_modifiers_changed(event.modifiers, cx);
                },
            ))
            .size_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            .when(!cfg!(target_os = "windows"), |shell| {
                shell.rounded(px(theme
                    .settings_window_radius()
                    .min(SETTINGS_MATERIAL_RADIUS as f32)))
            })
            .bg(theme.settings_content_bg())
            .font_family(crate::theme::ui_font())
            .font_weight(FontWeight::NORMAL)
            .text_color(theme.settings_text())
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_title_bar(window))
                    .child(self.render_content(window, cx)),
            );

        shell
            .children(self.render_menu(cx))
            .children(self.render_slider_drag_layer(cx))
    }
}

impl SettingsWindow {
    fn render_title_bar(&self, window: &Window) -> impl IntoElement {
        let theme = self.theme;
        let bar = div()
            .id("settings-title-bar")
            .h(px(TITLE_BAR_HEIGHT))
            .w_full()
            .flex_shrink_0()
            .flex()
            .justify_end()
            .window_control_area(gpui::WindowControlArea::Drag);
        #[cfg(target_os = "windows")]
        let bar = bar.child(ui::windows_caption_controls(
            theme,
            window.is_window_active(),
            window.is_maximized(),
            true,
            true,
        ));
        #[cfg(not(target_os = "windows"))]
        let bar = {
            let _ = (theme, window);
            bar.on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                window.start_window_move();
            })
        };
        bar
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let searching = !self.search.trim().is_empty();
        let update_ready = matches!(
            crate::updates::status(cx),
            crate::updates::UpdateStatus::Ready { .. }
                | crate::updates::UpdateStatus::Installed { .. }
        );

        let nav = Page::ALL.into_iter().map(|page| {
            let selected = !searching && page == self.page;
            div()
                .id(SharedString::from(format!("settings-nav-{}", page.slug())))
                .tab_index(0)
                .flex()
                .flex_row()
                .items_center()
                .h(px(crate::theme::ROW_HEIGHT))
                .px(px(8.))
                .gap(px(10.))
                .rounded(px(8.))
                .text_size(px(13.))
                .cursor_pointer()
                .border_1()
                .border_color(transparent_black())
                .focus_visible(move |style| style.border_color(theme.ring))
                .when(selected, |this| {
                    this.bg(theme.selection).font_weight(FontWeight::MEDIUM)
                })
                .when(!selected, |this| {
                    this.hover(|style| style.bg(theme.settings_hover()))
                })
                .child(
                    svg()
                        .path(page.icon())
                        .size(px(16.))
                        .flex_shrink_0()
                        .text_color(if selected {
                            theme.selection_foreground.into()
                        } else {
                            theme.settings_muted()
                        }),
                )
                .child(div().flex_1().min_w_0().truncate().child(tr(page.label())))
                .when(page == Page::About && update_ready, |this| {
                    this.child(
                        div()
                            .px(px(6.))
                            .rounded_full()
                            .bg(theme.primary)
                            .text_size(px(11.))
                            .text_color(theme.primary_foreground)
                            .child("1"),
                    )
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.clear_search(cx);
                    this.set_page(page, window, cx);
                }))
                .on_key_down(
                    cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                        if event.keystroke.modifiers == Default::default()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.clear_search(cx);
                            this.set_page(page, window, cx);
                            cx.stop_propagation();
                        }
                    }),
                )
        });

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .bg(theme.settings_sidebar_bg())
            .border_r_1()
            .border_color(theme.settings_divider())
            .child(
                div()
                    .px(px(10.))
                    .pt(px(10.))
                    .pb(px(8.))
                    .child(ui::TextInput::search(
                        &theme,
                        "settings-search",
                        &self.search_input,
                    )),
            )
            .child(
                div()
                    .id("settings-nav")
                    .flex()
                    .flex_col()
                    .px(px(10.))
                    .pb(px(10.))
                    .gap(px(2.))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(nav),
            )
    }

    fn render_content(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let query = self.search.trim();
        let (title, subtitle, body): (String, String, Vec<AnyElement>) = if query.is_empty() {
            let subtitle = if self.page == Page::About {
                t!(
                    "Version {version}, updates, licences",
                    version = env!("CARGO_PKG_VERSION")
                )
            } else {
                tr(self.page.summary()).to_string()
            };
            (
                tr(self.page.label()).to_string(),
                subtitle,
                self.render_page(window, cx),
            )
        } else {
            let hits = catalog::search(query);
            let subtitle = if hits.len() == 1 {
                t!("1 setting").to_string()
            } else {
                t!("{count} settings", count = hits.len())
            };
            (
                t!("Results").to_string(),
                subtitle,
                vec![self.render_results(query, &hits, cx)],
            )
        };

        div()
            .id("settings-page")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.page_scroll)
            .px(px(CONTENT_PADDING_X))
            .pb(px(28.))
            .gap(px(20.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child(subtitle),
                    ),
            )
            .children(body)
    }

    fn render_results(
        &self,
        query: &str,
        hits: &[catalog::Place],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        if hits.is_empty() {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(12.))
                .py(px(40.))
                .text_size(px(13.))
                .text_color(theme.settings_muted())
                .child(self.doodle(&onboarding_doodles::LENS, 76., cx))
                .child(t!("Nothing matches \"{query}\"", query = query))
                .into_any_element();
        }
        let rows = hits
            .iter()
            .map(|place| {
                let row = place.row;
                let where_it_is = if place.group.is_empty() || place.group == row.label() {
                    tr(place.page.label()).to_string()
                } else {
                    format!("{} › {}", tr(place.page.label()), tr(place.group))
                };
                div()
                    .id(SharedString::from(format!("settings-result-{}", row.id())))
                    .tab_index(0)
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .min_h(px(46.))
                    .p(px(12.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(transparent_black())
                    .rounded(px(8.))
                    .focus_visible(move |style| style.border_color(theme.ring))
                    .hover(|style| style.bg(theme.settings_hover()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(tr(row.label())),
                            )
                            .children(row.description().map(|description| {
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.settings_muted())
                                    .child(tr(description))
                            })),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child(where_it_is),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.jump_to(row, window, cx);
                    }))
                    .on_key_down(cx.listener(
                        move |this, event: &gpui::KeyDownEvent, window, cx| {
                            if event.keystroke.modifiers == Default::default()
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                this.jump_to(row, window, cx);
                                cx.stop_propagation();
                            }
                        },
                    ))
                    .into_any_element()
            })
            .collect();
        self.rows(rows).into_any_element()
    }

    fn render_page(&self, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.page
            .groups()
            .iter()
            .filter_map(|group| self.render_group(group, window, cx))
            .collect()
    }

    fn render_group(
        &self,
        group: &catalog::Group,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = self.theme;
        let rows: Vec<Row> = group
            .rows
            .iter()
            .copied()
            .filter(|row| row.is_shown())
            .collect();
        if rows.is_empty() {
            return None;
        }
        let bare = rows.iter().all(|row| is_bare(*row));
        let children: Vec<AnyElement> = rows
            .iter()
            .map(|row| self.render_placed_row(*row, window, cx))
            .collect();
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .when(!group.title.is_empty(), |this| {
                    this.child(
                        div()
                            .px(px(4.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.settings_muted())
                            .child(tr(group.title)),
                    )
                })
                .map(|this| {
                    if bare {
                        this.children(children)
                    } else {
                        this.child(self.rows(children))
                    }
                })
                .into_any_element(),
        )
    }

    fn render_placed_row(&self, row: Row, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let flashing = self.flash == Some(row);
        let body = if is_bare(row) || matches!(row, Row::Shortcut(_)) {
            self.render_row_control(row, window, cx)
        } else if is_stacked(row) {
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(12.))
                .when(!hides_label(row), |this| this.child(self.row_label(row)))
                .child(self.render_row_control(row, window, cx))
                .into_any_element()
        } else {
            ui::SettingRow::settings(
                &theme,
                tr(row.label()),
                row.description().map(tr),
                self.render_row_control(row, window, cx),
            )
            .into_any_element()
        };
        let dimmed = row == Row::KeepCompressed && !self.settings.auto_compress_instant;
        div()
            .id(SharedString::from(format!("settings-row-{}", row.id())))
            .rounded(px(12.))
            .when(flashing, |this| {
                this.bg(theme.selection)
                    .anchor_scroll(Some(self.jump_anchor.clone()))
            })
            .when(dimmed, |this| this.opacity(0.5))
            .child(body)
            .into_any_element()
    }

    fn row_label(&self, row: Row) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(13.)).child(tr(row.label())))
            .children(row.description().map(|description| {
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(theme.settings_muted())
                    .child(tr(description))
            }))
    }

    fn render_row_control(&self, row: Row, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let settings = &self.settings;
        match row {
            Row::Theme => self.render_theme_tiles(cx).into_any_element(),
            Row::Language => self.render_language(cx),
            Row::Notifications => self
                .toggle(
                    "enable-notifications",
                    settings.enable_notifications,
                    cx,
                    |this, cx| {
                        this.settings.enable_notifications = !this.settings.enable_notifications;
                        let value = this.settings.enable_notifications;
                        this.write_bool("enableNotifications", value, cx);
                    },
                )
                .into_any_element(),
            Row::DockIcon => self
                .toggle(
                    "hide-dock-icon",
                    !settings.hide_dock_icon,
                    cx,
                    |this, cx| {
                        this.settings.hide_dock_icon = !this.settings.hide_dock_icon;
                        let value = this.settings.hide_dock_icon;
                        this.write_bool("hideDockIcon", value, cx);
                        cx.defer(|cx: &mut gpui::App| crate::menus::schedule_dock_sync(cx));
                    },
                )
                .into_any_element(),
            Row::MainWindowStart => self
                .select(
                    "main-window-start",
                    settings.main_window_recording_start_behaviour.label(),
                    MenuKind::MainWindowStart,
                    cx,
                )
                .into_any_element(),
            Row::AfterDelete => self
                .select(
                    "post-deletion",
                    settings.post_deletion_behaviour.label(),
                    MenuKind::PostDeletion,
                    cx,
                )
                .into_any_element(),
            Row::StudioQuality => self.render_studio_quality(cx),
            Row::InstantResolution => self.render_instant_resolution(cx),
            Row::MaxFps => {
                let label = MAX_FPS_OPTIONS
                    .iter()
                    .find(|(value, _)| *value == settings.max_fps)
                    .map(|(_, label)| tr(label).to_string())
                    .unwrap_or_else(|| settings.max_fps.to_string());
                self.select("max-fps", label, MenuKind::MaxFps, cx)
                    .into_any_element()
            }
            Row::Countdown => {
                let current = settings.recording_countdown.unwrap_or(0);
                let label = COUNTDOWN_OPTIONS
                    .iter()
                    .find(|(value, _)| *value == current)
                    .map(|(_, label)| tr(label).to_string())
                    .unwrap_or_else(|| current.to_string());
                self.select("countdown", label, MenuKind::Countdown, cx)
                    .into_any_element()
            }
            Row::ConfirmMicrophone => self
                .toggle(
                    "confirm-without-mic",
                    settings.confirm_without_microphone,
                    cx,
                    |this, cx| {
                        this.settings.confirm_without_microphone =
                            !this.settings.confirm_without_microphone;
                        let value = this.settings.confirm_without_microphone;
                        if !store::set_store_setting(
                            RECORDING_START_SAFETY,
                            "confirmBeforeRecordingWithoutMicrophone",
                            Value::Bool(value),
                        ) {
                            this.settings = GeneralSettings::load();
                        }
                        cx.notify();
                    },
                )
                .into_any_element(),
            Row::SeparateCursor => self
                .toggle(
                    "custom-cursor",
                    settings.custom_cursor_capture,
                    cx,
                    |this, cx| {
                        this.settings.custom_cursor_capture = !this.settings.custom_cursor_capture;
                        let value = this.settings.custom_cursor_capture;
                        this.write_bool("custom_cursor_capture2", value, cx);
                    },
                )
                .into_any_element(),
            Row::KeyPresses => self
                .toggle(
                    "capture-keyboard",
                    settings.capture_keyboard_events,
                    cx,
                    |this, cx| {
                        this.settings.capture_keyboard_events =
                            !this.settings.capture_keyboard_events;
                        let value = this.settings.capture_keyboard_events;
                        this.write_bool("captureKeyboardEvents", value, cx);
                    },
                )
                .into_any_element(),
            Row::CrashRecovery => self
                .toggle(
                    "crash-recovery",
                    settings.crash_recovery_recording,
                    cx,
                    |this, cx| {
                        this.settings.crash_recovery_recording =
                            !this.settings.crash_recovery_recording;
                        let value = this.settings.crash_recovery_recording;
                        this.write_bool("crashRecoveryRecording", value, cx);
                    },
                )
                .into_any_element(),
            Row::NotchOverlay => self
                .toggle(
                    "notch-overlay",
                    settings.macbook_notch_overlay,
                    cx,
                    |this, cx| {
                        this.settings.macbook_notch_overlay = !this.settings.macbook_notch_overlay;
                        let value = this.settings.macbook_notch_overlay;
                        this.write_bool("macbookNotchOverlay", value, cx);
                    },
                )
                .into_any_element(),
            Row::ExcludedWindows => self.render_excluded_windows(cx),
            Row::AfterScreenshot => self
                .select(
                    "after-screenshot",
                    settings.post_screenshot_behaviour.label(),
                    MenuKind::AfterScreenshot,
                    cx,
                )
                .into_any_element(),
            Row::AfterStudio => self
                .select(
                    "after-studio",
                    settings.post_studio_recording_behaviour.label(),
                    MenuKind::AfterStudio,
                    cx,
                )
                .into_any_element(),
            Row::AfterInstant => self
                .select(
                    "after-instant",
                    settings.post_instant_recording_behaviour.label(),
                    MenuKind::AfterInstant,
                    cx,
                )
                .into_any_element(),
            Row::StudioSound => self
                .toggle(
                    "studio-sound-default",
                    settings.studio_sound_by_default,
                    cx,
                    |this, cx| {
                        let value = !this.settings.studio_sound_by_default;
                        if store::set_studio_sound_by_default(value) {
                            this.settings.studio_sound_by_default = value;
                        }
                        cx.notify();
                    },
                )
                .into_any_element(),
            Row::AutoZoom => self
                .toggle("auto-zoom", settings.auto_zoom_on_clicks, cx, |this, cx| {
                    this.settings.auto_zoom_on_clicks = !this.settings.auto_zoom_on_clicks;
                    let value = this.settings.auto_zoom_on_clicks;
                    this.write_bool("autoZoomOnClicks", value, cx);
                })
                .into_any_element(),
            Row::ZoomAmount => {
                let zoom = settings.default_zoom_amount.unwrap_or(1.5);
                let theme = self.theme;
                div()
                    .id("zoom-amount")
                    .tab_index(0)
                    .rounded(px(6.))
                    .focus_visible(move |style| style.bg(theme.selection))
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                        let current = this.settings.default_zoom_amount.unwrap_or(1.5);
                        if let Some(value) = ui::keyboard_slider_value(
                            event.keystroke.key.as_str(),
                            current,
                            ZOOM_MIN,
                            ZOOM_MAX,
                            0.1,
                        ) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.settings.default_zoom_amount = Some(value);
                            this.write("defaultZoomAmount", Value::from(f64::from(value)), cx);
                        }
                    }))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .w(px(208.))
                    .child(self.render_zoom_slider(zoom, cx))
                    .child(
                        div()
                            .w(px(36.))
                            .text_size(px(12.))
                            .text_color(self.theme.settings_muted())
                            .child(format!("{zoom:.1}x")),
                    )
                    .into_any_element()
            }
            Row::RememberedTerms => self.render_remembered_terms(cx),
            Row::StorageFolder => self.render_storage_folder(cx),
            Row::NameTemplate => self.render_name_template(window, cx),
            Row::CompressInstant => self
                .toggle(
                    "auto-compress-instant",
                    settings.auto_compress_instant,
                    cx,
                    |this, cx| {
                        this.settings.auto_compress_instant = !this.settings.auto_compress_instant;
                        let value = this.settings.auto_compress_instant;
                        this.write_bool("autoCompressInstant", value, cx);
                    },
                )
                .into_any_element(),
            Row::KeepCompressed => self
                .toggle(
                    "auto-compress-delete-original",
                    settings.auto_compress_delete_original,
                    cx,
                    |this, cx| {
                        if !this.settings.auto_compress_instant {
                            return;
                        }
                        this.settings.auto_compress_delete_original =
                            !this.settings.auto_compress_delete_original;
                        let value = this.settings.auto_compress_delete_original;
                        this.write_bool("autoCompressDeleteOriginal", value, cx);
                    },
                )
                .into_any_element(),
            Row::Shortcut(action) => self.render_shortcut(action, cx),
            Row::Rules => self.render_automations(cx),
            Row::Cli => self.render_cli(cx),
            Row::Muxer => self
                .toggle(
                    "out-of-process-muxer",
                    settings.out_of_process_muxer,
                    cx,
                    |this, cx| {
                        this.settings.out_of_process_muxer = !this.settings.out_of_process_muxer;
                        let value = this.settings.out_of_process_muxer;
                        this.write_bool("outOfProcessMuxer", value, cx);
                    },
                )
                .into_any_element(),
            Row::Diagnostic => self.render_diagnostic(cx),
            Row::Logs => self.render_logs(cx),
            Row::Version => self.render_version(cx),
            Row::UpdateDownloads => self
                .segmented::<UpdateDownloadPolicy>(
                    "update-download-policy",
                    settings.update_download_policy,
                    cx,
                    |this, value, cx| {
                        if this.settings.update_download_policy == value {
                            return;
                        }
                        this.settings.update_download_policy = value;
                        this.write_enum("updateDownloadPolicy", value, cx);
                        crate::updates::update_download_policy_changed(value, cx);
                    },
                )
                .into_any_element(),
            Row::UpdateChannel => self
                .segmented::<UpdateChannel>(
                    "update-channel",
                    settings.update_channel,
                    cx,
                    |this, value, cx| {
                        if this.settings.update_channel == value {
                            return;
                        }
                        this.settings.update_channel = value;
                        this.write_enum("updateChannel", value, cx);
                        crate::updates::update_channel_changed(value, cx);
                    },
                )
                .into_any_element(),
            Row::ReportProblem => self
                .button(
                    "report-problem",
                    (ui::ButtonVariant::Gray, None),
                    t!("Open GitHub"),
                    false,
                    cx,
                    |_, _, cx| cx.open_url(&crate::diagnostics::issue_report_url()),
                )
                .into_any_element(),
            Row::Changelog => self.render_changelog(cx),
            Row::Licences => self.render_licences(cx),
        }
    }

    fn render_theme_tiles(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let current = self.settings.theme;
        div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .children(AppTheme::ALL.iter().copied().map(|option| {
                let selected = option == current;
                let choose = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                    if this.settings.theme == option {
                        return;
                    }
                    this.settings.theme = option;
                    this.write_enum("theme", option, cx);
                    crate::theme::set_preference(option, cx);
                    crate::theme::apply_native(window, cx);
                    this.theme.refresh(window, cx, true);
                    cx.defer(|cx: &mut gpui::App| crate::app_windows::broadcast_theme(cx));
                };
                div()
                    .id(SharedString::from(format!("theme-{}", option.as_json())))
                    .tab_index(0)
                    .aria_label(option.label())
                    .aria_selected(selected)
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .child(
                        div()
                            .w(px(72.))
                            .h(px(44.))
                            .rounded(px(6.))
                            .overflow_hidden()
                            .border_2()
                            .border_color(if selected {
                                Hsla::from(theme.primary)
                            } else {
                                Hsla::from(theme.border)
                            })
                            .child(img(theme_preview(option)).size_full()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(if selected {
                                theme.settings_text()
                            } else {
                                theme.settings_muted()
                            })
                            .child(option.label()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| choose(this, window, cx)))
                    .on_key_down(cx.listener(
                        move |this, event: &gpui::KeyDownEvent, window, cx| {
                            if event.keystroke.modifiers == Default::default()
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                choose(this, window, cx);
                                cx.stop_propagation();
                            }
                        },
                    ))
            }))
    }

    fn render_language(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::i18n::Language;

        let chosen = self
            .settings
            .language
            .as_deref()
            .and_then(Language::from_code);
        let options = vec![
            ui::SegmentOption::new(t!("Follow system"), chosen.is_none()),
            ui::SegmentOption::new(
                Language::English.native_name(),
                chosen == Some(Language::English),
            ),
            ui::SegmentOption::new(
                Language::SimplifiedChinese.native_name(),
                chosen == Some(Language::SimplifiedChinese),
            ),
        ];
        self.segmented_raw("language", options, cx, |this, index, cx| {
            let choice = match index {
                0 => None,
                1 => Some(Language::English),
                _ => Some(Language::SimplifiedChinese),
            };
            this.settings.language = choice.map(|language| language.code().to_string());
            this.write(
                "language",
                choice.map_or(Value::Null, |language| {
                    Value::String(language.code().to_string())
                }),
                cx,
            );
            crate::i18n::set(crate::i18n::resolve(this.settings.language.as_deref()));
            if let Err(error) = crate::assets::Assets.load_chinese_font_if_needed(cx) {
                tracing::error!("failed to load the Chinese UI font: {error:#}");
            }
            cx.defer(|cx: &mut gpui::App| cx.refresh_windows());
        })
        .into_any_element()
    }

    fn select_instant_resolution(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((resolution, _)) = INSTANT_RESOLUTION_TIERS.get(index).copied() else {
            return;
        };
        if store::set_store_setting(
            GENERAL_SETTINGS,
            "instantModeMaxResolution",
            Value::from(resolution),
        ) {
            self.settings.instant_mode_max_resolution = resolution;
            self.instant_quality_save_failed = false;
        } else {
            self.instant_quality_save_failed = true;
        }
        cx.notify();
    }

    fn render_instant_resolution(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let current = self.settings.instant_mode_max_resolution;
        let options = INSTANT_RESOLUTION_TIERS
            .iter()
            .map(|(value, label)| ui::SegmentOption::new(*label, *value == current))
            .collect();
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(4.))
            .child(
                self.segmented_raw("instant-resolution", options, cx, |this, index, cx| {
                    this.select_instant_resolution(index, cx)
                }),
            )
            .when(self.instant_quality_save_failed, |this| {
                this.child(self.save_failed_note(theme))
            })
            .into_any_element()
    }

    fn save_failed_note(&self, theme: Theme) -> impl IntoElement {
        div()
            .text_size(px(12.))
            .text_color(Hsla::from(theme.warning_text))
            .child(t!("Couldn't save your recording settings. Try again."))
    }

    fn select_studio_quality(&mut self, quality: StudioQuality, cx: &mut Context<Self>) {
        self.studio_quality_save_failed = !store::set_store_setting(
            GENERAL_SETTINGS,
            "studioRecordingQuality",
            Value::String(quality.as_json().to_string()),
        );
        if !self.studio_quality_save_failed {
            self.settings.studio_recording_quality = quality;
        }
        cx.notify();
    }

    fn render_studio_quality(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let current = self.settings.studio_recording_quality;
        let tiers = STUDIO_QUALITY_TIERS
            .iter()
            .copied()
            .map(|(quality, label, description)| {
                let (label, description) = (tr(label), tr(description));
                let selected = current == quality;
                div()
                    .id(SharedString::from(format!(
                        "studio-quality-{}",
                        quality.as_json()
                    )))
                    .tab_index(0)
                    .aria_label(label)
                    .aria_description(description)
                    .aria_selected(selected)
                    .cursor_pointer()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .p(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(theme.primary)
                    } else {
                        Hsla::from(theme.border)
                    })
                    .focus_visible(move |style| style.border_color(theme.ring))
                    .when(!selected, |this| {
                        this.hover(|style| style.bg(theme.settings_hover()))
                    })
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(theme.settings_muted())
                            .child(description),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_studio_quality(quality, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.modifiers == Default::default()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.select_studio_quality(quality, cx);
                            cx.stop_propagation();
                        }
                    }))
            });
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().flex().flex_row().gap(px(8.)).children(tiers))
            .when(self.studio_quality_save_failed, |this| {
                this.child(self.save_failed_note(theme))
            })
            .into_any_element()
    }

    fn storage_path_box(&self, path: &Path) -> gpui::Div {
        let theme = self.theme;
        div()
            .flex()
            .flex_row()
            .flex_1()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .min_w_0()
            .rounded(px(8.))
            .bg(theme.settings_fill())
            .child(
                svg()
                    .path("icons/folder.svg")
                    .size(px(16.))
                    .flex_shrink_0()
                    .text_color(theme.settings_muted()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .child(path.to_string_lossy().to_string()),
            )
    }

    fn warning_line(&self, text: impl Into<SharedString>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(8.))
            .bg(theme.settings_fill())
            .text_size(px(12.))
            .text_color(Hsla::from(theme.warning_text))
            .child(
                svg()
                    .path("icons/triangle-alert.svg")
                    .size(px(14.))
                    .flex_shrink_0()
                    .mt(px(1.))
                    .text_color(Hsla::from(theme.warning_text)),
            )
            .child(div().flex_1().min_w_0().child(text.into()))
    }

    fn hint(&self, text: impl Into<SharedString>) -> impl IntoElement {
        div()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(self.theme.settings_muted())
            .child(text.into())
    }

    fn render_storage_folder(&self, cx: &mut Context<Self>) -> AnyElement {
        let storage = &self.settings.storage;
        let is_custom = storage.storage_path.is_some();
        let unavailable = storage
            .storage_path
            .as_ref()
            .is_some_and(|path| !path.is_dir());
        let display = storage
            .storage_path
            .clone()
            .unwrap_or_else(frameever_utils::storage::default_storage_dir);
        let reveal = display.clone();

        let mut actions = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.))
            .child(self.button(
                "storage-pick",
                (ui::ButtonVariant::Gray, None),
                t!("Change folder"),
                false,
                cx,
                |_, window, cx| {
                    let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: None,
                    });
                    cx.spawn_in(window, async move |this, cx| {
                        let Ok(Ok(Some(paths))) = paths.await else {
                            return;
                        };
                        let Some(path) = paths.into_iter().next() else {
                            return;
                        };
                        this.update(cx, |this, cx| {
                            this.settings.storage.choose_storage_folder(Some(path));
                            this.write_storage(cx);
                        })
                        .ok();
                    })
                    .detach();
                },
            ))
            .child(self.button(
                "storage-show",
                (ui::ButtonVariant::Gray, Some("icons/folder-open.svg")),
                t!("Show folder"),
                unavailable,
                cx,
                move |_, _, cx| crate::library::open_with_system(&reveal, cx),
            ));
        if is_custom {
            actions = actions.child(self.button(
                "storage-reset",
                (ui::ButtonVariant::Gray, None),
                t!("Use default folder"),
                false,
                cx,
                |this, _window, cx| {
                    this.settings.storage.choose_storage_folder(None);
                    this.write_storage(cx);
                },
            ));
        }

        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(self.storage_path_box(&display));
        if unavailable {
            content = content.child(self.warning_line(t!(
                "This folder isn't available. New projects go to the default folder until it's back."
            )));
        }
        content = content.child(actions);

        if !storage.previous_storage_paths.is_empty() {
            content = content
                .child(self.hint(t!("Previous folders. Their projects stay in the library.")));
            for (index, path) in storage.previous_storage_paths.iter().enumerate() {
                let folder = path.clone();
                content = content.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(self.storage_path_box(path))
                        .child(self.icon_button(
                            ("storage-previous-remove", index),
                            "icons/x.svg",
                            t!("Remove"),
                            cx,
                            move |this, cx| {
                                this.settings
                                    .storage
                                    .remove_previous_storage_folder(&folder);
                                this.write_storage(cx);
                            },
                        )),
                );
            }
        }
        content.into_any_element()
    }

    fn icon_button(
        &self,
        id: impl Into<gpui::ElementId>,
        icon: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id(id)
            .tab_index(0)
            .aria_label(label)
            .flex()
            .items_center()
            .justify_center()
            .size(px(24.))
            .flex_shrink_0()
            .rounded(px(6.))
            .cursor_pointer()
            .border_1()
            .border_color(transparent_black())
            .focus_visible(move |style| style.border_color(theme.ring))
            .hover(|style| style.bg(theme.settings_hover()))
            .child(
                svg()
                    .path(icon)
                    .size(px(14.))
                    .text_color(theme.settings_muted()),
            )
            .on_click(cx.listener(move |this, _, _window, cx| on_click(this, cx)))
    }

    fn insert_project_token(&mut self, token: &'static str, cx: &mut Context<Self>) {
        let next = if self.project_name.ends_with(' ') || self.project_name.is_empty() {
            format!("{}{token}", self.project_name)
        } else {
            format!("{} {token}", self.project_name)
        };
        self.project_name = next.clone();
        let input = self.project_name_input.clone();
        input.update(cx, |input, cx| input.set_text(next, cx));
        cx.notify();
    }

    fn render_name_template(&self, _window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let stored = self
            .settings
            .default_project_name_template
            .clone()
            .unwrap_or_else(|| DEFAULT_PROJECT_NAME_TEMPLATE.to_string());
        let draft = self.project_name.clone();
        let save_disabled = draft.is_empty() || draft == stored || draft.chars().count() <= 3;
        let reset_disabled = draft == DEFAULT_PROJECT_NAME_TEMPLATE
            && self.settings.default_project_name_template.is_none();

        let placeholder_rows: Vec<(&str, &str)> = vec![
            (
                "{recording_mode}",
                t!("\"Studio\", \"Instant\", or \"Screenshot\""),
            ),
            ("{mode}", t!("\"studio\", \"instant\", or \"screenshot\"")),
            ("{target_kind}", t!("\"Display\", \"Window\", or \"Area\"")),
            ("{target_name}", t!("Monitor name or window title.")),
            (
                "{random}",
                t!("10 filename-safe hexadecimal characters; use {random:N} for 1–32."),
            ),
            ("{date}", t!("the recording's date")),
            ("{time}", t!("the recording's time")),
        ];

        let tokens = div().flex().flex_row().flex_wrap().gap(px(6.)).children(
            PROJECT_NAME_TOKENS.iter().copied().map(|token| {
                div()
                    .id(SharedString::from(format!("project-token-{token}")))
                    .tab_index(0)
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(6.))
                    .bg(theme.settings_fill())
                    .text_size(px(11.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(transparent_black())
                    .focus_visible(move |style| style.border_color(theme.ring))
                    .hover(|style| style.bg(theme.settings_hover()))
                    .child(token)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.insert_project_token(token, cx);
                    }))
            }),
        );

        let actions = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(self.button(
                "project-name-save",
                (ui::ButtonVariant::Dark, None),
                t!("Save"),
                save_disabled,
                cx,
                |this, _window, cx| {
                    let template = this.project_name.clone();
                    this.settings.default_project_name_template = Some(template.clone());
                    this.write("defaultProjectNameTemplate", Value::String(template), cx);
                },
            ))
            .child(self.button(
                "project-name-reset",
                (ui::ButtonVariant::Gray, None),
                t!("Reset"),
                reset_disabled,
                cx,
                |this, _window, cx| {
                    this.settings.default_project_name_template = None;
                    this.project_name = DEFAULT_PROJECT_NAME_TEMPLATE.to_string();
                    let input = this.project_name_input.clone();
                    input.update(cx, |input, cx| {
                        input.set_text(DEFAULT_PROJECT_NAME_TEMPLATE, cx)
                    });
                    this.write("defaultProjectNameTemplate", Value::Null, cx);
                },
            ));

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(div().child(ui::TextInput::settings(
                &theme,
                "project-name-input",
                &self.project_name_input,
            )))
            .child(self.hint(t!("Preview: {name}", name = format_project_name(&draft))))
            .child(tokens)
            .child(actions)
            .child(
                div()
                    .id("placeholders-trigger")
                    .tab_index(0)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(12.))
                    .text_color(theme.settings_muted())
                    .cursor_pointer()
                    .child(
                        svg()
                            .path(if self.placeholders.is_open() {
                                "icons/chevron-down.svg"
                            } else {
                                "icons/chevron-right.svg"
                            })
                            .size(px(14.))
                            .text_color(theme.settings_muted()),
                    )
                    .child(t!("Available placeholders"))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.toggle_placeholders(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.toggle_placeholders(cx);
                            cx.stop_propagation();
                        }
                    })),
            )
            .when(
                self.placeholders.is_open() || self.placeholders.is_animating(),
                |this| {
                    if cx.reduce_motion() {
                        self.placeholders.settle();
                    }
                    let (height, _) = self.placeholders.height_for(Instant::now());
                    this.child(
                        ui::Collapsible::new(height, self.placeholders.measure_cell()).content(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .text_size(px(12.))
                                .children(placeholder_rows.into_iter().map(|(code, meaning)| {
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .px(px(6.))
                                                .py(px(2.))
                                                .rounded(px(6.))
                                                .bg(theme.settings_fill())
                                                .text_size(px(11.))
                                                .child(code.to_string()),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_color(theme.settings_muted())
                                                .child(meaning.to_string()),
                                        )
                                })),
                        ),
                    )
                },
            )
            .into_any_element()
    }

    fn render_excluded_windows(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let missing = self.missing_default_exclusions();
        let nothing_to_add = self.add_window_options().is_empty();

        let chips =
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(6.))
                .children(self.settings.excluded_windows.iter().enumerate().map(
                    |(index, entry)| {
                        let label = match entry.secondary_label() {
                            Some(secondary) => {
                                format!("{} · {}", entry.primary_label(), secondary)
                            }
                            None => entry.primary_label().to_string(),
                        };
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .pl(px(10.))
                            .pr(px(2.))
                            .h(px(26.))
                            .rounded(px(6.))
                            .bg(theme.settings_fill())
                            .text_size(px(12.))
                            .child(label)
                            .child(self.icon_button(
                                ("exclusion-remove", index),
                                "icons/x.svg",
                                t!("Remove"),
                                cx,
                                move |this, cx| {
                                    let mut next = this.settings.excluded_windows.clone();
                                    if index < next.len() {
                                        next.remove(index);
                                    }
                                    this.write_excluded(next, cx);
                                },
                            ))
                    },
                ))
                .child(
                    self.button(
                        "exclusions-add",
                        (ui::ButtonVariant::Gray, Some("icons/plus.svg")),
                        t!("Add window"),
                        nothing_to_add,
                        cx,
                        |_, _, _| {},
                    )
                    .on_click(cx.listener(
                        |this, event: &gpui::ClickEvent, window, cx| {
                            if !this.add_window_options().is_empty() {
                                this.open_menu(MenuKind::AddWindow, event.position(), window, cx);
                            }
                        },
                    )),
                );

        let mut content = div().flex().flex_col().gap(px(10.)).child(chips);
        if !missing.is_empty() {
            let names = missing
                .iter()
                .map(|entry| entry.primary_label().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            content = content.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().min_w_0().child(self.warning_line(t!(
                        "These FrameEver windows can show up in recordings: {names}.",
                        names = names
                    ))))
                    .child(self.button(
                        "exclusions-restore",
                        (ui::ButtonVariant::Gray, None),
                        t!("Restore"),
                        false,
                        cx,
                        |this, _window, cx| {
                            this.write_excluded(store::default_excluded_windows(), cx);
                        },
                    )),
            );
        }
        content
            .child(self.hint(t!(
                "Over Remote Desktop or a streaming app, FrameEver windows stay visible."
            )))
            .into_any_element()
    }

    fn render_version(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let configured = crate::installer::updates_configured();
        let status = crate::updates::status(cx);
        let action: AnyElement = match status {
            _ if !configured => self
                .hint(t!("Updates are not enabled in this build"))
                .into_any_element(),
            crate::updates::UpdateStatus::Idle => self
                .button(
                    "check-updates",
                    (ui::ButtonVariant::Gray, None),
                    t!("Check for updates"),
                    false,
                    cx,
                    |_, _, cx| crate::updates::check_manually(cx),
                )
                .into_any_element(),
            crate::updates::UpdateStatus::Checking => self
                .button(
                    "check-updates",
                    (ui::ButtonVariant::Gray, None),
                    t!("Checking…"),
                    true,
                    cx,
                    |_, _, _| {},
                )
                .into_any_element(),
            crate::updates::UpdateStatus::Downloading { version, fraction } => {
                let text = match fraction {
                    Some(fraction) => t!(
                        "Downloading {version}… {percent}%",
                        version = version,
                        percent = (fraction * 100.).round() as u32
                    ),
                    None => t!("Downloading {version}…", version = version),
                };
                self.hint(text).into_any_element()
            }
            crate::updates::UpdateStatus::Installing { version } => self
                .hint(t!("Installing {version}…", version = version))
                .into_any_element(),
            crate::updates::UpdateStatus::Ready { .. }
            | crate::updates::UpdateStatus::Installed { .. } => self
                .button(
                    "restart-to-update",
                    (ui::ButtonVariant::Primary, None),
                    t!("Restart to update"),
                    false,
                    cx,
                    |_, _, cx| crate::updates::install_and_relaunch(cx),
                )
                .into_any_element(),
        };

        self.card(true)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.))
            .child(crate::logo::app_icon(44.).flex_shrink_0())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("FrameEver · 帧刻"),
                    )
                    .child(self.doodle(&onboarding_doodles::UNDERLINE, 110., cx))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child(t!("Version {version}", version = env!("CARGO_PKG_VERSION"))),
                    ),
            )
            .child(action)
            .into_any_element()
    }

    fn render_licences(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let links = [
            (
                "licence-cap",
                t!("Based on Cap (AGPL-3.0)"),
                "https://github.com/CapSoftware/Cap",
            ),
            (
                "licence-lucide",
                t!("Icons: Lucide (ISC)"),
                "https://lucide.dev/license",
            ),
            (
                "licence-tabler",
                t!("Icons: Tabler (MIT)"),
                "https://github.com/tabler/tabler-icons/blob/main/LICENSE",
            ),
            (
                "licence-font",
                t!("Chinese font: HarmonyOS Sans by Huawei"),
                "https://developer.huawei.com/consumer/cn/design/resource/",
            ),
        ];
        let _ = cx;
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_x(px(14.))
            .gap_y(px(4.))
            .children(links.map(|(id, label, url)| {
                div()
                    .id(id)
                    .tab_index(0)
                    .text_size(px(12.))
                    .text_color(theme.settings_muted())
                    .cursor_pointer()
                    .border_1()
                    .border_color(transparent_black())
                    .rounded(px(4.))
                    .focus_visible(move |style| style.border_color(theme.ring))
                    .hover(|style| style.text_color(theme.settings_text()))
                    .child(label)
                    .on_click(move |_, _, cx| cx.open_url(url))
                    .on_key_down(move |event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            cx.open_url(url);
                        }
                    })
            }))
            .into_any_element()
    }

    pub(crate) fn card(&self, padded: bool) -> gpui::Div {
        ui::Card::settings(&self.theme, padded)
    }

    pub(crate) fn rows(&self, children: Vec<AnyElement>) -> gpui::Div {
        ui::Card::settings_rows(&self.theme, children)
    }

    pub(crate) fn section(
        &self,
        title: &'static str,
        description: Option<&'static str>,
        right: Option<AnyElement>,
        children: Vec<AnyElement>,
    ) -> ui::Section {
        ui::Section::settings(&self.theme, title, description, right, children)
    }

    pub(crate) fn setting_row(
        &self,
        label: &'static str,
        description: Option<&'static str>,
        control: AnyElement,
    ) -> AnyElement {
        ui::SettingRow::settings(&self.theme, label, description, control).into_any_element()
    }

    pub(crate) fn note_box(&self) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(8.))
            .bg(self.theme.settings_fill())
    }

    pub(crate) fn toggle(
        &self,
        id: impl Into<gpui::ElementId>,
        checked: bool,
        cx: &mut Context<Self>,
        on_change: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> ui::Toggle {
        ui::Toggle::settings(&self.theme, id, checked)
            .on_click(cx.listener(move |this, _, _window, cx| on_change(this, cx)))
    }

    pub(crate) fn select(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        kind: MenuKind,
        cx: &mut Context<Self>,
    ) -> ui::Select {
        ui::Select::settings(&self.theme, id, label).on_click(cx.listener(
            move |this, event: &gpui::ClickEvent, window, cx| {
                this.open_menu(kind, event.position(), window, cx);
            },
        ))
    }

    fn segmented<T: SettingsEnum>(
        &self,
        id: &'static str,
        current: T,
        cx: &mut Context<Self>,
        on_change: impl Fn(&mut Self, T, &mut Context<Self>) + Clone + 'static,
    ) -> ui::SegmentedControl {
        self.segmented_raw(
            id,
            T::ALL
                .iter()
                .map(|value| ui::SegmentOption::new(value.label(), *value == current))
                .collect(),
            cx,
            move |this, index, cx| {
                if let Some(value) = ui::option_at(T::ALL, index) {
                    on_change(this, value, cx);
                }
            },
        )
    }

    fn segmented_raw(
        &self,
        id: &'static str,
        options: Vec<ui::SegmentOption>,
        cx: &mut Context<Self>,
        on_change: impl Fn(&mut Self, usize, &mut Context<Self>) + Clone + 'static,
    ) -> ui::SegmentedControl {
        ui::SegmentedControl::settings(&self.theme, id, options).on_select(
            cx.listener(move |this, index: &usize, _window, cx| on_change(this, *index, cx)),
        )
    }

    pub(crate) fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        style: (ui::ButtonVariant, Option<&'static str>),
        label: impl Into<SharedString>,
        disabled: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> ui::Button {
        let (variant, icon) = style;
        let mut button = ui::Button::settings(&self.theme, id, variant, ui::ButtonSize::Sm)
            .label(label)
            .disabled_settings(&self.theme, disabled);
        if let Some(icon) = icon {
            button = button.icon(icon);
        }
        button.on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)))
    }

    fn on_field_event(
        &mut self,
        field: Field,
        input: Entity<ui::TextInputState>,
        event: &ui::TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        match (field, event) {
            (Field::ProjectName, ui::TextInputEvent::Changed) => {
                self.project_name = input.read(cx).text().to_string();
                cx.notify();
            }
            (Field::ProjectName, ui::TextInputEvent::Cancelled) => {
                let stored = self
                    .settings
                    .default_project_name_template
                    .clone()
                    .unwrap_or_else(|| DEFAULT_PROJECT_NAME_TEMPLATE.to_string());
                self.project_name = stored.clone();
                input.update(cx, |input, cx| input.set_text(stored, cx));
                cx.notify();
            }
            _ => {}
        }
    }

    fn render_zoom_slider(&self, value: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let fraction = ((value - ZOOM_MIN) / (ZOOM_MAX - ZOOM_MIN)).clamp(0., 1.);

        ui::Slider::new("zoom-slider", fraction, self.slider_track.clone())
            .flex()
            .track(px(5.), theme.settings_fill())
            .fill(Hsla::from(theme.primary))
            .thumb(
                px(16.),
                if theme.is_dark() {
                    Hsla::from(theme.gray_12)
                } else {
                    Hsla::from(theme.gray_1)
                },
                Some(theme.settings_border()),
            )
            .thumb_top(px(-6.))
            .on_drag_start(
                cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                    this.slider_dragging = true;
                    this.set_zoom_from(event.position, cx);
                }),
            )
    }

    fn render_slider_drag_layer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.slider_dragging {
            return None;
        }
        Some(
            ui::Slider::drag_layer(
                "zoom-slider-drag",
                cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                    this.set_zoom_from(event.position, cx);
                }),
                cx.listener(|this, _, _window, cx| {
                    this.slider_dragging = false;
                    let value = this.settings.default_zoom_amount.unwrap_or(1.5);
                    this.write(
                        "defaultZoomAmount",
                        Value::from(f64::from(ui::snap_to_step(value, ZOOM_MIN, ZOOM_MAX, 0.1))),
                        cx,
                    );
                }),
            )
            .into_any_element(),
        )
    }

    fn set_zoom_from(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(value) =
            ui::slider_value_at(&self.slider_track, position, ZOOM_MIN, ZOOM_MAX, 0.1)
        else {
            return;
        };
        if self.settings.default_zoom_amount != Some(value) {
            self.settings.default_zoom_amount = Some(value);
            cx.notify();
        }
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let kind = menu.kind;
        let items = self.menu_items(kind);

        Some(
            ui::Menu::settings(&self.theme, "settings-menu", items, &menu.state)
                .on_select(cx.listener(move |this, index: &usize, window, cx| {
                    this.choose(kind, *index, window, cx);
                }))
                .on_dismiss(cx.listener(|this, _, _window, cx| {
                    this.menu = None;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    pub(crate) fn toggle_changelog(&mut self, cx: &mut Context<Self>) {
        self.changelog_all = !self.changelog_all;
        cx.notify();
    }

    pub(crate) fn changelog_all(&self) -> bool {
        self.changelog_all
    }
}

const ZOOM_MIN: f32 = 1.;
const ZOOM_MAX: f32 = 4.5;

fn format_project_name(template: &str) -> String {
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    template
        .replace("{recording_mode}", "Instant")
        .replace("{mode}", "instant")
        .replace("{target_kind}", "Window")
        .replace("{target_name}", "Safari")
        .replace("{date}", &date)
        .replace("{time}", "09:41 AM")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_exclusions_are_covered_by_title() {
        let default = WindowExclusion {
            window_title: Some("FrameEver Camera".into()),
            ..Default::default()
        };
        assert!(covers_default_exclusion(&default, &default));
        assert!(covers_default_exclusion(
            &WindowExclusion {
                owner_name: Some("FrameEver".into()),
                window_title: Some("FrameEver Camera".into()),
                ..Default::default()
            },
            &default
        ));
        assert!(!covers_default_exclusion(
            &WindowExclusion {
                window_title: Some("FrameEver Settings".into()),
                ..Default::default()
            },
            &default
        ));
    }

    #[test]
    fn the_preview_substitutes_the_documented_placeholders() {
        let preview = format_project_name(DEFAULT_PROJECT_NAME_TEMPLATE);
        assert!(preview.starts_with("Safari (Window) "));
        assert!(preview.ends_with(" 09:41 AM"));
        assert_eq!(format_project_name("{nope}"), "{nope}");
    }
}
