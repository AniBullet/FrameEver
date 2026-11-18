use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, Keystroke, MouseButton, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, prelude::FluentBuilder, px,
};

use super::{
    OnboardingWindow, Step, copy, ease, glyph,
    shortcuts::{Leave, Note},
    stage::Paper,
};
use crate::{
    doodle::{Doodle, doodle},
    i18n::{n, t, tr},
    main_window::Mode,
    onboarding_doodles as art, settings_pages, store,
};

struct ModeCard {
    mode: Mode,
    icon: &'static str,
    doodle: &'static Doodle,
    title: &'static str,
    tag: &'static str,
}

const MODE_CARDS: [ModeCard; 2] = [
    ModeCard {
        mode: Mode::Studio,
        icon: "icons/clapperboard.svg",
        doodle: &art::TRACKS,
        title: n!("Studio"),
        tag: n!("Record, then edit"),
    },
    ModeCard {
        mode: Mode::Instant,
        icon: "icons/zap.svg",
        doodle: &art::BOLT,
        title: n!("Instant"),
        tag: n!("A video the moment you stop"),
    },
];

struct Detail {
    doodle: &'static Doodle,
    title: &'static str,
    tag: &'static str,
    body: &'static str,
    ticks: [&'static str; 4],
}

const STUDIO: Detail = Detail {
    doodle: MODE_CARDS[0].doodle,
    title: MODE_CARDS[0].title,
    tag: MODE_CARDS[0].tag,
    body: n!(
        "Screen, camera, microphone and cursor are kept as separate tracks, so you can polish the recording in the editor before you export it."
    ),
    ticks: [
        n!("Backgrounds, padding and auto zoom"),
        n!("Captions made on this computer"),
        n!("Chapter marks while you record"),
        n!("Export to MP4 or GIF"),
    ],
};

const INSTANT: Detail = Detail {
    doodle: MODE_CARDS[1].doodle,
    title: MODE_CARDS[1].title,
    tag: MODE_CARDS[1].tag,
    body: n!(
        "Instant records straight to an MP4 on this computer. When you stop, it shows up in the library, ready to play or drag into a chat."
    ),
    ticks: [
        n!("Ready to play, no rendering"),
        n!("Microphone, system audio and camera"),
        n!("Automatic compression if you want it"),
        n!("Or copy the file when you stop"),
    ],
};

const SCREENSHOT: Detail = Detail {
    doodle: &art::SNAP,
    title: n!("Screenshot"),
    tag: n!("Capture and annotate"),
    body: n!("Pick an area, mark it up in the screenshot editor, then copy or save the PNG."),
    ticks: [
        n!("A magnifier for pixel-exact edges"),
        n!("Arrows, step numbers and speech bubbles"),
        n!("Spotlight, blur and highlighter"),
        n!("Backgrounds and rounded corners"),
    ],
};

#[derive(Clone, Copy)]
pub(super) enum FaqAction {
    OpenStorageFolder,
    OpenReleases,
}

pub(super) const FAQ: [(&str, &str, Option<FaqAction>); 4] = [
    (
        n!("Is it free?"),
        n!(
            "Yes. FrameEver is free and open source, and everything runs on this computer. There is no account."
        ),
        None,
    ),
    (
        n!("Where do my files go?"),
        n!(
            "Into the storage folder, Videos\\FrameEver unless you change it. Projects, screenshots and exports all live there, and nothing is uploaded."
        ),
        Some(FaqAction::OpenStorageFolder),
    ),
    (
        n!("I used Cap-aniBullet 2.x or Cap. How do I bring my recordings?"),
        n!(
            "Run FrameEver Converter, a separate download on the releases page. It moves settings, projects and loose videos into the storage folder."
        ),
        Some(FaqAction::OpenReleases),
    ),
    (
        n!("Are captions uploaded?"),
        n!("No. FrameEver downloads a caption model once, then makes captions on this computer."),
        None,
    ),
];

pub(super) fn storage_folder() -> std::path::PathBuf {
    store::GeneralSettings::load()
        .storage
        .storage_path
        .unwrap_or_else(frameever_utils::storage::default_storage_dir)
}

impl OnboardingWindow {
    fn step_content(&self) -> Div {
        div()
            .flex_grow(1.)
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(28.))
            .pt(px(16.))
            .px(px(40.))
            .pb(px(8.))
    }

    fn rise(&self, element: Div, key: (&'static str, usize), millis: u64) -> AnyElement {
        if !self.animated {
            return element.into_any_element();
        }
        element
            .with_animation(
                (key.0, key.1 + self.step_revision * 16),
                Animation::new(Duration::from_millis(millis)),
                |el, t| el.relative().top(px(16. * (1. - ease(t)))).opacity(ease(t)),
            )
            .into_any_element()
    }

    fn doodle_at(&self, art: &'static Doodle, delay: u64, width: f32, height: f32) -> AnyElement {
        let paper = Paper::new(&self.theme);
        doodle(
            art,
            paper.palette(),
            self.step_at + Duration::from_millis(delay),
            !self.animated,
            width,
            height,
        )
        .into_any_element()
    }

    pub(super) fn render_modes(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let paper = Paper::new(&theme);
        let chosen = MODE_CARDS
            .iter()
            .find(|card| card.mode == self.mode)
            .unwrap_or(&MODE_CARDS[0]);
        let cards = MODE_CARDS.iter().enumerate().map(|(index, card)| {
            let on = card.mode == self.mode;
            let mode = card.mode;
            let pick = div()
                .absolute()
                .top(px(12.))
                .right(px(12.))
                .size(px(18.))
                .rounded_full()
                .border(px(1.5))
                .flex()
                .items_center()
                .justify_center()
                .map(|el| {
                    if on {
                        el.border_color(paper.ink).bg(paper.ink).child(glyph(
                            "icons/check.svg",
                            14.,
                            paper.bg,
                        ))
                    } else {
                        el.border_color(theme.border_strong)
                    }
                });
            let element = self
                .paper_card()
                .id(("onboarding-mode", index))
                .tab_index(0)
                .relative()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .pt(px(22.))
                .px(px(16.))
                .pb(px(20.))
                .cursor_pointer()
                .when(on, |el| el.border_color(paper.ink))
                .when(!on, |el| {
                    el.hover(move |el| el.border_color(theme.border_strong))
                })
                .on_click(cx.listener(move |this, _, _, cx| this.choose_mode(mode, cx)))
                .child(pick)
                .child(self.doodle_at(card.doodle, index as u64 * 180, 96., 84.))
                .child(
                    copy(tr(card.title), 15., paper.ink)
                        .mt(px(4.))
                        .text_center(),
                )
                .child(
                    copy(tr(card.tag), 12., paper.soft)
                        .font_weight(FontWeight::NORMAL)
                        .line_height(px(16.))
                        .text_center(),
                );
            let millis = 700 + index as u64 * 100;
            if self.animated {
                element
                    .with_animation(
                        ("onboarding-mode-rise", index + self.step_revision * 16),
                        Animation::new(Duration::from_millis(millis)),
                        |el, t| el.top(px(16. * (1. - ease(t)))).opacity(ease(t)),
                    )
                    .into_any_element()
            } else {
                element.into_any_element()
            }
        });
        let row = div()
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(12.))
            .bg(theme.muted)
            .children(MODE_CARDS.iter().map(|card| {
                let on = card.mode == self.mode;
                let color: Hsla = if on {
                    theme.foreground.into()
                } else {
                    theme.muted_foreground.into()
                };
                div()
                    .h(px(26.))
                    .px(px(10.))
                    .rounded(px(9.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .when(on, |el| el.bg(theme.secondary))
                    .child(glyph(card.icon, 14., color))
                    .child(copy(tr(card.title), 13., color))
            }));
        self.step_content()
            .child(self.heading(
                t!("Two ways to record"),
                t!(
                    "Pick the one you'll use most. You can switch any time at the top of the main window. Screenshots have their own button and shortcuts."
                ),
            ))
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .w_full()
                    .max_w(px(560.))
                    .children(cards),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .child(row)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(self.doodle_at(&art::UP_ARROW, 300, 14., 18.))
                            .child(
                                copy(
                                    t!(
                                        "{mode} is where the main window starts",
                                        mode = tr(chosen.title)
                                    ),
                                    12.,
                                    paper.soft,
                                )
                                .font_weight(FontWeight::NORMAL),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn choose_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.defer(move |cx| crate::app_windows::set_recording_mode(mode, cx));
        cx.notify();
    }

    pub(super) fn render_detail(&self) -> AnyElement {
        let paper = Paper::new(&self.theme);
        let detail = match self.step {
            Step::Instant => &INSTANT,
            Step::Screenshot => &SCREENSHOT,
            _ => &STUDIO,
        };
        let side = div()
            .w(px(240.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(self.doodle_at(detail.doodle, 0, 58., 50.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(copy(tr(detail.title), 20., paper.ink))
                            .child(
                                copy(tr(detail.tag), 12., paper.soft)
                                    .font_weight(FontWeight::NORMAL),
                            ),
                    ),
            )
            .child(
                copy(tr(detail.body), 13., paper.soft)
                    .font_weight(FontWeight::NORMAL)
                    .line_height(px(20.)),
            )
            .child(div().flex().flex_col().gap(px(10.)).children(
                detail.ticks.iter().enumerate().map(|(index, tick)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(self.doodle_at(&art::TICK, 400 + index as u64 * 140, 18., 18.))
                        .child(copy(tr(tick), 13., paper.ink).font_weight(FontWeight::NORMAL))
                }),
            ));
        self.step_content()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(32.))
                    .child(side)
                    .child(self.demo_card().flex_1().min_w_0()),
            )
            .into_any_element()
    }

    fn demo_card(&self) -> Div {
        self.paper_card()
            .h(px(300.))
            .rounded(px(16.))
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.demo_screen())
            .child(self.demo_labels())
    }

    fn kbd(&self, key: impl Into<SharedString>) -> Div {
        let theme = self.theme;
        div()
            .h(px(18.))
            .px(px(5.))
            .flex()
            .items_center()
            .rounded(px(4.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.card)
            .text_color(theme.muted_foreground)
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .child(key.into())
    }

    pub(super) fn render_library(&self) -> AnyElement {
        let paper = Paper::new(&self.theme);
        let keys = [
            (SharedString::from("Space"), t!("Preview")),
            (SharedString::from("Ctrl+K"), t!("Find any command")),
            (
                SharedString::from(t!("Drag")),
                t!("Drop the file into any app"),
            ),
        ];
        self.step_content()
            .child(self.heading(
                t!("Everything lands in the library"),
                t!(
                    "Press Ctrl+L or click Library and the library opens in its own window. The recording window stays where it is."
                ),
            ))
            .child(self.demo_card().w(px(640.)).flex_shrink_0())
            .child(
                div()
                    .flex()
                    .justify_center()
                    .gap(px(12.))
                    .children(keys.into_iter().map(|(key, label)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(self.kbd(key))
                            .child(copy(label, 12., paper.soft).font_weight(FontWeight::NORMAL))
                    })),
            )
            .into_any_element()
    }

    pub(super) fn stop_listening(&mut self, cx: &mut Context<Self>) {
        if self.shortcuts.listening.take().is_some() {
            cx.defer(|cx| {
                crate::hotkeys::resume(cx);
            });
        }
    }

    fn start_listening(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.shortcuts.listening == Some(index) {
            self.stop_listening(cx);
            cx.notify();
            return;
        }
        if self.shortcuts.listening.is_none() {
            cx.defer(crate::hotkeys::pause);
        }
        self.shortcuts.listening = Some(index);
        cx.notify();
    }

    pub(super) fn capture_shortcut(
        &mut self,
        keystroke: &Keystroke,
        shift_held: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.shortcuts.listening else {
            return;
        };
        let bare = !keystroke.modifiers.platform
            && !keystroke.modifiers.control
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.shift;
        match keystroke.key.as_str() {
            "escape" => self.stop_listening(cx),
            "backspace" | "delete" if bare => {
                self.shortcuts.clear(index);
                self.stop_listening(cx);
            }
            _ => match settings_pages::hotkey_from_keystroke(keystroke, shift_held) {
                Ok(hotkey) => {
                    self.stop_listening(cx);
                    self.shortcuts.set(index, hotkey, &store::hotkeys_raw());
                }
                Err(reason) => self.shortcuts.reject(index, reason),
            },
        }
        cx.notify();
    }

    pub(super) fn skip(&mut self, cx: &mut Context<Self>) {
        if self.step == Step::Shortcuts {
            self.leave_shortcuts(Leave::Skip, cx);
        }
        self.finish(cx);
    }

    pub(super) fn leave_shortcuts(&mut self, leave: Leave, cx: &mut Context<Self>) -> bool {
        self.stop_listening(cx);
        let mut hotkeys = store::hotkeys_raw();
        if !self.shortcuts.leave(leave, &mut hotkeys) {
            return true;
        }
        if !store::set_hotkeys_raw(&hotkeys) {
            self.error = Some(t!("Couldn't save your progress. Try again.").into());
            cx.notify();
            return false;
        }
        let this = cx.entity().downgrade();
        cx.defer(move |cx| {
            crate::hotkeys::reload(cx);
            let unavailable = crate::hotkeys::unavailable_actions(cx);
            let taken = this
                .update(cx, |this, cx| {
                    let mut hotkeys = store::hotkeys_raw();
                    let taken = this.shortcuts.mark_taken(&unavailable, &mut hotkeys);
                    if taken && !store::set_hotkeys_raw(&hotkeys) {
                        tracing::warn!("saving the hotkeys store failed");
                    }
                    cx.notify();
                    taken
                })
                .unwrap_or(false);
            if taken {
                crate::hotkeys::reload(cx);
            }
            cx.refresh_windows();
        });
        true
    }

    fn change_storage(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                let mut storage = store::GeneralSettings::load().storage;
                storage.choose_storage_folder(Some(frameever_utils::storage::storage_folder_in(
                    &path,
                )));
                let saved = storage
                    .general_settings_entries()
                    .into_iter()
                    .all(|(key, value)| {
                        store::set_store_setting(store::GENERAL_SETTINGS, key, value)
                    });
                if !saved {
                    this.error = Some(t!("Couldn't save your progress. Try again.").into());
                }
                this.storage = storage_folder();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn key_field(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let row = &self.shortcuts.rows[index];
        let listening = self.shortcuts.listening == Some(index);
        let field = div()
            .id(("onboarding-key", index))
            .tab_index(0)
            .min_w(px(132.))
            .h(px(crate::theme::CONTROL_HEIGHT))
            .pl(px(10.))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(12.))
            .border_1()
            .bg(theme.background)
            .cursor_pointer()
            .border_color(if listening { theme.ring } else { theme.input })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| this.start_listening(index, cx)));
        let keys = div()
            .flex_1()
            .flex()
            .gap(px(4.))
            .text_size(px(12.))
            .text_color(theme.muted_foreground)
            .whitespace_nowrap();
        match (&row.binding, listening) {
            (_, true) => field.child(keys.child(t!("Press keys…"))),
            (Some(binding), false) => field
                .child(
                    keys.children(
                        settings_pages::hotkey_display_keys(binding)
                            .iter()
                            .map(|key| self.kbd(settings_pages::hotkey_key_text(key))),
                    ),
                )
                .child(
                    div()
                        .id(("onboarding-key-clear", index))
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .hover(move |el| el.bg(theme.secondary_hover))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.shortcuts.clear(index);
                            this.stop_listening(cx);
                            cx.notify();
                        }))
                        .child(glyph("icons/x.svg", 14., theme.muted_foreground)),
                ),
            (None, false) => field.child(keys.child(t!("Not set"))),
        }
        .into_any_element()
    }

    fn setup_row(&self, icon: &'static str, title: &'static str, note: Div) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(18.))
            .py(px(14.))
            .child(glyph(icon, 16., theme.muted_foreground))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(copy(title, 13., theme.foreground))
                    .child(note),
            )
    }

    fn shortcut_note(&self, index: usize, fallback: &'static str) -> Div {
        let theme = self.theme;
        let (text, color): (String, Hsla) = match &self.shortcuts.rows[index].note {
            Some(Note::Taken(hotkey)) => (
                t!(
                    "{keys} is taken by another program. Pick other keys.",
                    keys = settings_pages::hotkey_inline_text(hotkey)
                ),
                theme.warning_text.into(),
            ),
            Some(Note::MovedFrom(action)) => (
                t!(
                    "Used by \"{shortcut}\" now. Continue moves it here.",
                    shortcut = settings_pages::shortcut_label(action)
                ),
                theme.muted_foreground.into(),
            ),
            Some(Note::Rejected(reason)) => (tr(reason).into(), theme.warning_text.into()),
            None => (tr(fallback).into(), theme.muted_foreground.into()),
        };
        copy(text, 11., color)
            .font_weight(FontWeight::NORMAL)
            .line_height(px(15.))
    }

    pub(super) fn render_shortcuts(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let again = self.again;
        let storage = &self.storage;
        let rows = [
            (
                "icons/video.svg",
                t!("Start recording"),
                n!("Opens the picker for a display, window or area"),
            ),
            (
                "icons/scan.svg",
                t!("Screenshot an area"),
                n!("Drag over the screen, then annotate"),
            ),
        ];
        let card = self
            .paper_card()
            .w_full()
            .max_w(px(520.))
            .overflow_hidden()
            .children(
                rows.into_iter()
                    .enumerate()
                    .map(|(index, (icon, title, hint))| {
                        self.setup_row(icon, title, self.shortcut_note(index, hint))
                            .when(index > 0, |el| el.border_t_1().border_color(theme.border))
                            .child(self.key_field(index, cx))
                    }),
            )
            .child(
                self.setup_row(
                    "icons/folder.svg",
                    t!("Storage folder"),
                    copy(
                        storage.to_string_lossy().to_string(),
                        11.,
                        theme.muted_foreground,
                    )
                    .font_family(settings_pages::MONO_FONT)
                    .font_weight(FontWeight::NORMAL)
                    .truncate(),
                )
                .border_t_1()
                .border_color(theme.border)
                .child(
                    div()
                        .id("onboarding-storage-change")
                        .tab_index(0)
                        .h(px(crate::theme::CONTROL_HEIGHT))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(theme.border)
                        .text_size(px(13.))
                        .cursor_pointer()
                        .hover(move |el| el.bg(theme.secondary_hover))
                        .on_click(cx.listener(|this, _, _, cx| this.change_storage(cx)))
                        .child(t!("Change…")),
                ),
            );
        let element = self.rise(card, ("onboarding-setup", 0), 700);
        self.step_content()
            .child(self.heading(
                t!("Set two shortcuts"),
                if again {
                    t!(
                        "These are your current shortcuts. Change them here or later in Settings → Shortcuts."
                    )
                } else {
                    t!(
                        "Start from anywhere without opening the main window. Change them later in Settings → Shortcuts."
                    )
                },
            ))
            .child(element)
            .into_any_element()
    }

    pub(super) fn render_faq(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let card = self
            .paper_card()
            .w_full()
            .max_w(px(520.))
            .overflow_hidden()
            .children(
                FAQ.iter()
                    .enumerate()
                    .map(|(index, (question, answer, action))| {
                        let open = self.faq_open[index];
                        let action = action.map(|action| match action {
                            FaqAction::OpenStorageFolder => {
                                (action, "icons/folder-open.svg", t!("Open storage folder"))
                            }
                            FaqAction::OpenReleases => {
                                (action, "icons/arrow-up-right.svg", t!("Open releases page"))
                            }
                        });
                        div()
                            .when(index > 0, |el| el.border_t_1().border_color(theme.border))
                            .child(
                                div()
                                    .id(("onboarding-faq", index))
                                    .tab_index(0)
                                    .cursor_pointer()
                                    .px(px(20.))
                                    .py(px(15.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.faq_open[index] = !this.faq_open[index];
                                        cx.notify();
                                    }))
                                    .child(copy(tr(question), 13., theme.foreground))
                                    .child(glyph(
                                        if open {
                                            "icons/chevron-up.svg"
                                        } else {
                                            "icons/chevron-down.svg"
                                        },
                                        14.,
                                        theme.muted_foreground,
                                    )),
                            )
                            .when(open, |el| {
                                el.child(
                                    div()
                                        .px(px(20.))
                                        .pb(px(16.))
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .gap(px(10.))
                                        .child(
                                            copy(tr(answer), 13., theme.muted_foreground)
                                                .font_weight(FontWeight::NORMAL)
                                                .line_height(px(20.)),
                                        )
                                        .when_some(action, |el, (action, icon, label)| {
                                            el.child(
                                                div()
                                                    .id(("onboarding-faq-action", index))
                                                    .tab_index(0)
                                                    .h(px(crate::theme::CONTROL_HEIGHT))
                                                    .px(px(12.))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.))
                                                    .rounded(px(12.))
                                                    .border_1()
                                                    .border_color(theme.border)
                                                    .text_size(px(13.))
                                                    .text_color(theme.foreground)
                                                    .cursor_pointer()
                                                    .hover(move |el| el.bg(theme.secondary_hover))
                                                    .on_click(move |_, _, cx| match action {
                                                        FaqAction::OpenStorageFolder => {
                                                            crate::library::open_with_system(
                                                                &crate::library::storage()
                                                                    .storage_dir(),
                                                                cx,
                                                            );
                                                        }
                                                        FaqAction::OpenReleases => {
                                                            cx.open_url(&format!(
                                                                "https://github.com/{}/releases",
                                                                crate::brand::RELEASES_REPO
                                                            ));
                                                        }
                                                    })
                                                    .child(glyph(icon, 14., theme.foreground))
                                                    .child(label),
                                            )
                                        }),
                                )
                            })
                    }),
            );
        let element = self.rise(card, ("onboarding-faq-card", 0), 700);
        self.step_content()
            .child(self.heading(
                t!("Good to know"),
                t!("Short answers to what people ask first."),
            ))
            .child(element)
            .into_any_element()
    }
}
