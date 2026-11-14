use std::time::Instant;

use gpui::{
    App, AppContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle, FontWeight, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px, svg,
};

use super::{Command, Entry, Outcome, Palette, Recents, Scope, Source, Surface, Target};
use crate::app_windows;
use crate::i18n::{t, tc, tr};
use crate::library::{self, MediaKind};
use crate::session::{Phase, RecordingSession};
use crate::theme::{ROW_HEIGHT, Theme, motion};
use crate::ui::{self, text_input};

const MAX_WIDTH: f32 = 640.;
const SIDE_GAP: f32 = 12.;
const TOP_SHARE: f32 = 0.2;
const INPUT_HEIGHT: f32 = 44.;
const FOOT_HEIGHT: f32 = 32.;
const LIST_HEIGHT: f32 = 320.;
const SHORT_LIST_HEIGHT: f32 = 240.;
const SHORT_WINDOW: f32 = 560.;
const DROP: f32 = 6.;

gpui::actions!(command_palette, [Toggle]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([gpui::KeyBinding::new("ctrl-k", Toggle, None)]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([gpui::KeyBinding::new("cmd-k", Toggle, None)]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteEvent {
    Chosen(Outcome),
    Dismissed,
}

pub struct CommandPalette {
    palette: Palette,
    source: Source,
    input: Entity<ui::TextInputState>,
    library: Vec<library::view::Entry>,
    scroll: ScrollHandle,
    opened_at: Instant,
    closing_at: Option<Instant>,
    _input_events: Subscription,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl CommandPalette {
    pub fn new(source: Source, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_placeholder(t!("Search commands, projects and settings"));
            input
        });
        let input_events = cx.subscribe_in(&input, window, Self::on_input_event);
        Self {
            palette: Palette::default(),
            source,
            input,
            library: Vec::new(),
            scroll: ScrollHandle::new(),
            opened_at: Instant::now(),
            closing_at: None,
            _input_events: input_events,
        }
    }

    fn set_library(&mut self, mut entries: Vec<library::view::Entry>, cx: &mut Context<Self>) {
        if self.source.recording {
            return;
        }
        entries.sort_by(|a, b| b.created_millis.total_cmp(&a.created_millis));
        self.source.projects = entries
            .iter()
            .map(|entry| {
                let (detail, icon) = match entry.kind {
                    MediaKind::Studio => (t!("Studio"), "icons/clapperboard.svg"),
                    MediaKind::Instant => (t!("Instant"), "icons/zap.svg"),
                    MediaKind::Screenshot => (t!("Screenshot"), "icons/image.svg"),
                };
                Entry::project(
                    entry.path.clone(),
                    entry.name.clone(),
                    detail.to_string(),
                    icon,
                )
            })
            .collect();
        self.library = entries;
        cx.notify();
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle()
    }

    pub fn is_open(&self) -> bool {
        self.closing_at.is_none()
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.closing_at.is_none() {
            self.closing_at = Some(Instant::now());
            cx.notify();
        }
    }

    fn on_input_event(
        &mut self,
        input: &Entity<ui::TextInputState>,
        event: &ui::TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ui::TextInputEvent::Changed => {
                let text = input.read(cx).text().to_string();
                if self.palette.typed(&text, self.source.recording) {
                    let rest = self.palette.query().to_string();
                    input.update(cx, |input, cx| input.set_text(rest, cx));
                }
                self.reveal_chosen();
                cx.notify();
            }
            ui::TextInputEvent::Confirmed => self.confirm(window.modifiers().shift, cx),
            ui::TextInputEvent::Cancelled => cx.emit(PaletteEvent::Dismissed),
            ui::TextInputEvent::Blurred => {}
        }
    }

    fn confirm(&mut self, shift: bool, cx: &mut Context<Self>) {
        if let Some(outcome) = self.palette.confirm(&self.source, shift) {
            cx.emit(PaletteEvent::Chosen(outcome));
        }
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.palette.step(forward, &self.source);
        self.reveal_chosen();
        cx.notify();
    }

    fn reveal_chosen(&self) {
        let chosen = self.palette.index(&self.source);
        let mut child = 0;
        let mut row = 0;
        for group in self.palette.groups(&self.source) {
            child += 1;
            for _ in &group.entries {
                if row == chosen {
                    self.scroll.scroll_to_item(child);
                    return;
                }
                child += 1;
                row += 1;
            }
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.key == "enter" && keystroke.modifiers.shift {
            self.confirm(true, cx);
            cx.stop_propagation();
        }
    }

    fn progress(&self, reduce_motion: bool) -> (f32, bool) {
        if reduce_motion {
            return (
                if self.closing_at.is_some() { 0. } else { 1. },
                self.closing_at.is_some(),
            );
        }
        match self.closing_at {
            Some(closing_at) => {
                let t = closing_at.elapsed().as_secs_f32() / motion::PALETTE_OUT.as_secs_f32();
                (1. - motion::EASE_FLUENT_OUT.at(t.min(1.)), t >= 1.)
            }
            None => {
                let t = self.opened_at.elapsed().as_secs_f32() / motion::PALETTE_IN.as_secs_f32();
                (motion::EASE_FLUENT_IN.at(t.min(1.)), t >= 1.)
            }
        }
    }

    fn render_hint(theme: &Theme, keys: &[&str], label: &'static str) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .flex_shrink_0()
            .children(keys.iter().map(|key| key_chip(theme, key)))
            .child(label)
    }
}

fn key_chip(theme: &Theme, key: &str) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .flex_shrink_0()
        .h(px(18.))
        .px(px(5.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.card)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.muted_foreground)
        .child(ui::kbd_symbol(key))
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window, cx, false);
        let (shown, settled) = self.progress(cx.reduce_motion());
        if self.closing_at.is_some() && settled {
            return div().into_any_element();
        }
        if !settled {
            window.request_animation_frame();
        }
        let closing = self.closing_at.is_some();
        let viewport = window.viewport_size();
        let width = (f32::from(viewport.width) - SIDE_GAP * 2.).min(MAX_WIDTH);
        let top = f32::from(viewport.height) * TOP_SHARE - DROP * (1. - shown);
        let list_height = if f32::from(viewport.height) < SHORT_WINDOW {
            SHORT_LIST_HEIGHT
        } else {
            LIST_HEIGHT
        };
        let groups = self.palette.groups(&self.source);
        let chosen_index = self.palette.index(&self.source);
        let chosen_is_project = self
            .palette
            .chosen(&self.source)
            .map(|entry| matches!(entry.target, Target::Project(_)));

        let mut list = div()
            .id("command-palette-list")
            .flex()
            .flex_col()
            .max_h(px(list_height))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(px(4.));
        let mut row = 0;
        for group in &groups {
            list = list.child(
                div()
                    .flex_shrink_0()
                    .pt(px(8.))
                    .px(px(8.))
                    .pb(px(4.))
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .child(tr(group.heading.label())),
            );
            for entry in &group.entries {
                let index = row;
                row += 1;
                let on = index == chosen_index;
                list =
                    list.child(
                        div()
                            .id(("command-palette-row", index))
                            .flex()
                            .items_center()
                            .flex_shrink_0()
                            .gap(px(10.))
                            .h(px(ROW_HEIGHT))
                            .px(px(8.))
                            .rounded(px(12.))
                            .cursor_pointer()
                            .when(on, |this| this.bg(theme.accent))
                            .hover(|this| this.bg(theme.accent))
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                this.palette.select(index);
                                this.confirm(event.modifiers().shift, cx);
                            }))
                            .child(
                                svg()
                                    .path(entry.icon)
                                    .size(px(16.))
                                    .flex_shrink_0()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .gap(px(6.))
                                    .child(div().truncate().child(entry.name.clone()))
                                    .when_some(entry.detail.clone(), |this, detail| {
                                        this.child(
                                            div()
                                                .flex_shrink_0()
                                                .text_size(px(12.))
                                                .text_color(theme.muted_foreground)
                                                .child(detail),
                                        )
                                    }),
                            )
                            .when(!entry.keys.is_empty(), |this| {
                                this.child(
                                    div().flex().gap(px(4.)).flex_shrink_0().children(
                                        entry.keys.iter().map(|key| key_chip(&theme, key)),
                                    ),
                                )
                            }),
                    );
            }
        }
        if groups.is_empty() {
            list = list.child(
                div()
                    .py(px(28.))
                    .px(px(12.))
                    .flex()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child(t!("No results")),
            );
        }

        let foot =
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .h(px(FOOT_HEIGHT))
                .px(px(12.))
                .border_t_1()
                .border_color(theme.border)
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .overflow_hidden()
                .when_some(chosen_is_project, |this, project| {
                    if project {
                        this.child(Self::render_hint(&theme, &["Enter"], t!("Open")))
                            .child(Self::render_hint(
                                &theme,
                                &["Shift", "Enter"],
                                tc!("palette hint", "Show in library"),
                            ))
                    } else {
                        this.child(Self::render_hint(&theme, &["Enter"], t!("Run")))
                    }
                })
                .when(self.palette.shows_prefix_hints(&self.source), |this| {
                    this.child(div().flex_1()).children(Scope::ALL.map(|scope| {
                        Self::render_hint(&theme, &[scope.prefix()], tr(scope.label()))
                    }))
                });

        let panel = div()
            .id("command-palette")
            .absolute()
            .top(px(top))
            .left(px((f32::from(viewport.width) - width) / 2.))
            .w(px(width))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_size(px(13.5))
            .text_color(theme.foreground)
            .opacity(shown)
            .when(!closing, |this| {
                this.occlude()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(Self::on_key))
                    .capture_action(cx.listener(|this, _: &text_input::MoveUp, _, cx| {
                        this.step(false, cx);
                        cx.stop_propagation();
                    }))
                    .capture_action(cx.listener(|this, _: &text_input::MoveDown, _, cx| {
                        this.step(true, cx);
                        cx.stop_propagation();
                    }))
                    .capture_action(cx.listener(|this, _: &text_input::Backspace, _, cx| {
                        if this.palette.backspace_on_empty() {
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(INPUT_HEIGHT))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        svg()
                            .path("icons/search.svg")
                            .size(px(16.))
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground),
                    )
                    .when_some(self.palette.scope(), |this, scope| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .flex_shrink_0()
                                .h(px(24.))
                                .px(px(8.))
                                .rounded_full()
                                .bg(theme.selection)
                                .text_size(px(12.))
                                .text_color(theme.selection_foreground)
                                .child(tr(scope.label())),
                        )
                    })
                    .child(
                        div().flex_1().min_w_0().child(
                            ui::TextInput::bare(&theme, "command-palette-input", &self.input)
                                .placeholder_color(Hsla::from(theme.muted_foreground))
                                .text_color(Hsla::from(theme.foreground)),
                        ),
                    ),
            )
            .child(list)
            .child(foot);

        div()
            .id("command-palette-scrim")
            .absolute()
            .inset_0()
            .when(!closing, |this| {
                this.occlude()
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(PaletteEvent::Dismissed)))
            })
            .bg(Hsla::from(theme.overlay).opacity(shown))
            .child(panel)
            .into_any_element()
    }
}

#[derive(Default)]
pub struct Slot {
    view: Option<Entity<CommandPalette>>,
    back: Option<FocusHandle>,
    events: Option<Subscription>,
}

impl Slot {
    pub fn is_open(&self, cx: &App) -> bool {
        self.view
            .as_ref()
            .is_some_and(|view| view.read(cx).is_open())
    }

    pub fn view(&self) -> Option<Entity<CommandPalette>> {
        self.view.clone()
    }

    pub fn open(
        &mut self,
        view: Entity<CommandPalette>,
        events: Subscription,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.back = window.focused(cx);
        let focus = view.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.view = Some(view);
        self.events = Some(events);
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if !self.is_open(cx) {
            return false;
        }
        if let Some(view) = &self.view {
            view.update(cx, |view, cx| view.close(cx));
        }
        self.events = None;
        if let Some(back) = self.back.take() {
            window.focus(&back, cx);
        }
        true
    }
}

pub type OnEvent<T> =
    fn(&mut T, &Entity<CommandPalette>, &PaletteEvent, &mut Window, &mut Context<T>);

pub fn toggle<T: 'static>(
    slot: &mut Slot,
    surface: Surface,
    on_event: OnEvent<T>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    cx.notify();
    if slot.close(window, cx) {
        return;
    }
    let recording = matches!(
        RecordingSession::global(cx).read(cx).phase,
        Phase::Recording { .. }
    );
    let source = Source::new(
        surface,
        recording,
        crate::i18n::current(),
        Vec::new(),
        Recents::from_ids(crate::store::load().recent_commands),
    );
    let view = cx.new(|cx| CommandPalette::new(source, window, cx));
    let events = cx.subscribe_in(&view, window, on_event);
    let weak = view.downgrade();
    slot.open(view, events, window, cx);
    cx.spawn(async move |_, cx| {
        let entries = cx.background_spawn(async { library::list_entries() }).await;
        weak.update(cx, |view, cx| view.set_library(entries, cx))
            .ok();
    })
    .detach();
}

pub fn act(palette: &Entity<CommandPalette>, outcome: &Outcome, cx: &mut App) -> Option<Command> {
    match outcome {
        Outcome::Run(command) => {
            let command = *command;
            cx.background_spawn(async move {
                crate::store::update(|stored| {
                    let mut recents =
                        Recents::from_ids(std::mem::take(&mut stored.recent_commands));
                    recents.note(command);
                    stored.recent_commands = recents.ids().to_vec();
                });
            })
            .detach();
            match command {
                Command::StopRecording => {
                    RecordingSession::global(cx).update(cx, |session, cx| session.stop(cx));
                }
                Command::PauseRecording => {
                    RecordingSession::global(cx).update(cx, |session, cx| session.toggle_pause(cx));
                }
                Command::RestartRecording => {
                    cx.defer(app_windows::tray_restart_recording);
                }
                Command::OpenSettings => cx.defer(|cx| {
                    app_windows::open_settings(crate::settings_window::last_page(), cx);
                    cx.activate(true);
                }),
                Command::OpenStorageFolder => {
                    library::open_with_system(&library::storage().storage_dir(), cx);
                }
                Command::ShowLibrary => cx.defer(|cx| {
                    app_windows::show_library(cx);
                    cx.activate(true);
                }),
                Command::TakeScreenshot => cx.defer(|cx| {
                    crate::hotkeys::dispatch(crate::hotkeys::HotkeyAction::ScreenshotDisplay, cx);
                }),
                command => return Some(command),
            }
        }
        Outcome::OpenProject(path) => {
            let entry = palette
                .read(cx)
                .library
                .iter()
                .find(|entry| entry.path == *path)
                .cloned();
            let path = path.clone();
            match entry {
                Some(library::view::Entry {
                    screenshot: Some(png),
                    ..
                }) => cx.defer(move |cx| app_windows::open_screenshot_editor(png, cx)),
                Some(entry) if entry.opens_editor => {
                    cx.defer(move |cx| app_windows::open_editor(path, cx));
                }
                _ => cx.defer(move |cx| app_windows::tray_open_library_item(path, cx)),
            }
        }
        Outcome::ShowInLibrary(path) => {
            let path = path.clone();
            cx.defer(move |cx| app_windows::tray_open_library_item(path, cx));
        }
        Outcome::OpenSetting(row) => {
            let row = *row;
            cx.defer(move |cx| app_windows::open_settings_row(row, cx));
        }
    }
    None
}
