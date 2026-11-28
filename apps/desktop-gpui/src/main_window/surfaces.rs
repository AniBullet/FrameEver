use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, canvas, div, prelude::FluentBuilder, px, svg,
    transparent_black,
};

use super::{
    MainWindow, Mode, Panel, PickerPurpose, ScreenCaptureTarget, ScreenshotButton, TargetType,
    app_windows,
};
use crate::{doodle, i18n::t, onboarding_doodles, theme::Theme, ui};

const CONTENT_GAP: f32 = 10.;
const CONTENT_PADDING: f32 = 12.;
const SEGMENT_HEIGHT: f32 = 28.;
const TARGET_HEIGHT: f32 = 58.;
const TARGET_GAP: f32 = 6.;
const START_HEIGHT: f32 = 40.;
const FOOTER_HEIGHT: f32 = 44.;
pub(super) const ABOVE_START: f32 = FOOTER_HEIGHT + CONTENT_PADDING + START_HEIGHT + CONTENT_GAP;
const ARROW_WIDTH: f32 = 26.;

pub(crate) fn focus_ring<E: InteractiveElement + Styled>(element: E, theme: Theme) -> E {
    element
        .border_1()
        .border_color(transparent_black())
        .focus_visible(move |style| style.border_color(theme.ring))
}

fn item_count_label(count: usize) -> String {
    if count == 1 {
        t!("1 item").to_string()
    } else {
        t!("{count} items", count = count).to_string()
    }
}

pub(crate) fn title_icon(
    theme: Theme,
    id: &'static str,
    path: &'static str,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    focus_ring(
        div()
            .id(id)
            .role(gpui::Role::Button)
            .aria_label(label)
            .accessibility_id(id)
            .group(id)
            .when(cfg!(target_os = "windows"), |this| this.occlude())
            .tab_index(0)
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .flex_shrink_0()
            .rounded(px(6.))
            .hover(move |style| style.bg(theme.accent))
            .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
            .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
            .child(
                svg()
                    .path(path)
                    .size(px(16.))
                    .text_color(theme.muted_foreground)
                    .group_hover(id, move |style| style.text_color(theme.foreground)),
            ),
        theme,
    )
}

impl MainWindow {
    fn settings_button(&self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        title_icon(self.theme, "settings", "icons/settings.svg", t!("Settings")).on_click(
            cx.listener(|_, _, _, cx| {
                cx.defer(|cx: &mut gpui::App| {
                    app_windows::open_settings(crate::settings_window::last_page(), cx)
                });
            }),
        )
    }

    pub(super) fn render_compact_title(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .items_center()
            .gap(px(4.))
            .pl(px(10.))
            .pr(px(4.))
            .child(crate::logo::app_icon(16.))
            .child(
                div()
                    .pl(px(4.))
                    .text_size(px(12.))
                    .text_color(theme.foreground)
                    .child("FrameEver"),
            )
            .child(div().flex_1().min_w_0())
            .children(crate::update_panel::title_pill(theme, cx))
            .child(
                title_icon(
                    theme,
                    "screenshot",
                    "icons/image.svg",
                    ScreenshotButton::title(self.chosen_target()),
                )
                .on_click(cx.listener(|this, _, _, cx| this.screenshot_chosen_target(cx))),
            )
            .child(
                title_icon(
                    theme,
                    "teleprompter",
                    "icons/scan-text.svg",
                    t!("Teleprompter"),
                )
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.defer(app_windows::open_teleprompter);
                })),
            )
            .child(
                title_icon(
                    theme,
                    "help",
                    "icons/circle-question-mark.svg",
                    t!("Help & Tour"),
                )
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.defer(app_windows::show_onboarding_again);
                })),
            )
    }

    pub(super) fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.panel {
            Some(panel) => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .px(px(13.))
                .pb(px(8.))
                .gap(px(8.))
                .text_color(self.theme.body_text())
                .child(self.render_panel(panel, cx))
                .into_any_element(),
            None => self.render_compact_home(cx).into_any_element(),
        }
    }

    fn render_compact_home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.sidebar)
            .text_color(theme.foreground)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(CONTENT_GAP))
                    .p(px(CONTENT_PADDING))
                    .child(self.render_mode_row(cx))
                    .child(self.render_targets(cx))
                    .child(self.render_base_controls(cx))
                    .child(self.render_start_button(cx)),
            )
            .child(self.render_compact_footer(cx))
    }

    fn render_mode_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        if self.session.read(cx).editor_recording_target().is_some() {
            return div()
                .flex()
                .flex_col()
                .justify_center()
                .h(px(32.))
                .flex_shrink_0()
                .child(div().text_size(px(13.)).child(t!("Record a new clip")))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(theme.muted_foreground)
                        .child(t!("Add to your current project")),
                )
                .into_any_element();
        }
        let selected_mode = self.effective_mode(cx);
        let segment = |mode: Mode, id: &'static str, index: usize| {
            let selected = mode == selected_mode;
            let bounds = self.mode_hover_bounds[index].clone();
            let color = if selected {
                theme.foreground
            } else {
                theme.muted_foreground
            };
            focus_ring(
                div()
                    .id(SharedString::from(id))
                    .role(gpui::Role::Tab)
                    .aria_label(mode.panel_title())
                    .aria_selected(selected)
                    .accessibility_id(id)
                    .tab_index(0)
                    .relative()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h(px(SEGMENT_HEIGHT))
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color)
                    .when(selected, |this| this.bg(theme.secondary))
                    .hover(move |style| style.text_color(theme.foreground))
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        this.update_mode_hover(mode, false, *hovered, cx);
                    }))
                    .child(
                        canvas(move |value, _, _| bounds.set(Some(value)), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .child(svg().path(mode.icon()).size(px(14.)).text_color(color))
                    .child(mode.panel_title())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_mode(mode, cx);
                        cx.defer(app_windows::refresh_target_overlays);
                    })),
                theme,
            )
        };
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .flex_shrink_0()
            .child(
                div()
                    .id("modes")
                    .role(gpui::Role::TabList)
                    .aria_label(t!("Recording mode"))
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h(px(32.))
                    .p(px(2.))
                    .gap(px(2.))
                    .rounded(px(6.))
                    .bg(theme.muted)
                    .child(segment(Mode::Instant, "mode-instant", 0))
                    .child(segment(Mode::Studio, "mode-studio", 1)),
            )
            .child(
                title_icon(
                    self.theme,
                    "mode-info",
                    "icons/info.svg",
                    t!("Recording mode info"),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_panel(Panel::ModeInfo, window, cx);
                })),
            )
            .into_any_element()
    }

    fn render_targets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("targets")
            .role(gpui::Role::TabList)
            .aria_label(t!("Target"))
            .flex()
            .flex_row()
            .gap(px(TARGET_GAP))
            .w_full()
            .flex_shrink_0()
            .child(self.render_target_tile(TargetType::Display, cx))
            .child(self.render_target_tile(TargetType::Window, cx))
            .child(self.render_target_tile(TargetType::Area, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h(px(TARGET_HEIGHT))
                    .gap(px(TARGET_GAP))
                    .child(self.render_target_tile(TargetType::CameraOnly, cx))
                    .child(self.render_target_tile(TargetType::AudioOnly, cx)),
            )
    }

    fn render_target_tile(&self, target: TargetType, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let selected = self.target.or(self.start_target) == Some(target);
        let color = if selected {
            theme.selection_foreground
        } else {
            theme.muted_foreground
        };
        let chooser = matches!(target, TargetType::Display | TargetType::Window);
        let stacked = matches!(target, TargetType::CameraOnly | TargetType::AudioOnly);

        div()
            .id(target.tile_id())
            .role(gpui::Role::Tab)
            .aria_label(target.label())
            .aria_selected(selected)
            .accessibility_id(target.tile_id())
            .tab_index(0)
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .map(|tile| {
                if stacked {
                    tile.gap(px(5.))
                        .tooltip(move |_, cx| ui::Tooltip::new(&theme, target.label()).view(cx))
                        .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                } else {
                    tile.flex_col().h(px(TARGET_HEIGHT)).gap(px(4.))
                }
            })
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .border_1()
            .border_color(if selected {
                theme.selection
            } else {
                theme.border
            })
            .bg(if selected {
                theme.selection
            } else {
                theme.card
            })
            .text_size(px(11.))
            .text_color(color)
            .hover(move |style| {
                if selected {
                    style
                } else {
                    style.bg(theme.accent).text_color(theme.foreground)
                }
            })
            .focus_visible(move |style| style.border_color(theme.ring))
            .child(
                svg()
                    .path(target.icon())
                    .size(px(if stacked { 14. } else { 20. }))
                    .flex_shrink_0()
                    .text_color(color),
            )
            .child(
                div()
                    .min_w_0()
                    .max_w_full()
                    .when(!stacked, |label| label.px(px(2.)))
                    .truncate()
                    .child(target.tile_label()),
            )
            .when(chooser, |tile| {
                tile.child(
                    div()
                        .id(SharedString::from(format!("{}-choose", target.tile_id())))
                        .role(gpui::Role::Button)
                        .aria_label(match target {
                            TargetType::Display => t!("Choose a display"),
                            _ => t!("Choose a window"),
                        })
                        .accessibility_id(format!("{}-choose", target.tile_id()))
                        .tab_index(0)
                        .absolute()
                        .top(px(1.))
                        .right(px(1.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(18.))
                        .rounded(px(4.))
                        .hover(move |style| style.bg(theme.accent))
                        .focus_visible(move |style| style.bg(theme.accent))
                        .tooltip(move |_, cx| {
                            ui::Tooltip::new(
                                &theme,
                                match target {
                                    TargetType::Display => t!("Choose a display"),
                                    _ => t!("Choose a window"),
                                },
                            )
                            .view(cx)
                        })
                        .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                        .child(
                            svg()
                                .path("icons/chevron-down.svg")
                                .size(px(14.))
                                .text_color(color),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.open_panel(Panel::Target(target), window, cx);
                        })),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.start_target = Some(target);
                this.purpose = PickerPurpose::Record;
                if target == TargetType::AudioOnly {
                    if this.target.take().is_some() {
                        cx.defer(app_windows::dismiss_target_overlays);
                    }
                    cx.notify();
                    return;
                }
                this.target = if this.target == Some(target) {
                    None
                } else {
                    Some(target)
                };
                this.sync_overlays(cx);
                cx.notify();
            }))
    }

    fn render_start_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let blocked = super::audio_start_blocked(
            self.chosen_target(),
            self.microphone.is_some(),
            self.system_audio,
        );
        let start = focus_ring(
            div()
                .id("start")
                .role(gpui::Role::Button)
                .aria_label(t!("Start recording"))
                .accessibility_id("start-recording")
                .when(!blocked, |start| start.tab_index(0))
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .h(px(START_HEIGHT))
                .rounded(px(6.))
                .bg(theme.primary)
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.primary_foreground)
                .child(ui::record_mark(&theme))
                .child(t!("Start recording")),
            theme,
        )
        .map(|start| {
            if blocked {
                start
                    .opacity(0.5)
                    .tooltip(move |_, cx| {
                        ui::Tooltip::new(&theme, t!("Audio needs a microphone or system audio"))
                            .view(cx)
                    })
                    .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
            } else {
                start
                    .hover(move |style| style.bg(theme.primary_hover))
                    .on_click(cx.listener(|this, _, _, cx| this.start_from_chosen_target(cx)))
            }
        });
        div().flex().w_full().flex_shrink_0().child(start)
    }

    fn chosen_target(&self) -> TargetType {
        self.start_target.unwrap_or(TargetType::Display)
    }

    pub(crate) fn start_from_chosen_target(&mut self, cx: &mut Context<Self>) {
        self.purpose = PickerPurpose::Record;
        if self.chosen_target() == TargetType::AudioOnly {
            cx.defer(|cx| {
                app_windows::start_recording_from_overlay(ScreenCaptureTarget::AudioOnly, cx)
            });
            return;
        }
        self.target = Some(self.chosen_target());
        self.sync_overlays(cx);
        cx.notify();
    }

    pub(super) fn screenshot_chosen_target(&mut self, cx: &mut Context<Self>) {
        let target = self.chosen_target();
        match ScreenshotButton::for_target(target) {
            ScreenshotButton::Capture => {
                let id = self
                    .selected_display
                    .as_ref()
                    .map(|display| display.id.clone())
                    .unwrap_or_else(|| {
                        scap_targets::Display::get_containing_cursor()
                            .unwrap_or_else(scap_targets::Display::primary)
                            .id()
                    });
                cx.defer(move |cx| {
                    crate::screenshot::take_screenshot(ScreenCaptureTarget::Display { id }, cx)
                });
            }
            ScreenshotButton::Pick => {
                if self.session.read(cx).editor_recording_target().is_some() {
                    return;
                }
                self.purpose = PickerPurpose::Screenshot;
                self.target = Some(ScreenshotButton::picker_target(target));
                self.sync_overlays(cx);
                cx.notify();
            }
        }
    }

    fn render_compact_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let library = focus_ring(
            div()
                .id("library")
                .role(gpui::Role::Button)
                .aria_label(t!("Library"))
                .accessibility_id("library")
                .tab_index(0)
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(4.))
                .h(px(32.))
                .pl(px(10.))
                .pr(px(6.))
                .rounded(px(6.))
                .text_size(px(13.))
                .text_color(theme.foreground)
                .hover(move |style| style.bg(theme.accent))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Library  Ctrl+L")).view(cx))
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                .child(t!("Library"))
                .when(!self.unseen.is_empty(), |this| {
                    this.child(crate::library_window::library_badge(
                        &theme,
                        self.unseen.len(),
                    ))
                })
                .child(
                    svg()
                        .path("icons/chevron-right.svg")
                        .size(px(16.))
                        .text_color(theme.foreground),
                ),
            theme,
        )
        .on_click(cx.listener(|this, _, _, cx| this.open_library(cx)));

        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(FOOTER_HEIGHT))
            .flex_shrink_0()
            .px(px(6.))
            .border_t_1()
            .border_color(theme.border)
            .text_size(px(11.))
            .text_color(theme.muted_foreground)
            .child(self.settings_button(cx))
            .child(
                self.render_recording_options_summary(cx)
                    .unwrap_or_else(|| self.render_footer_note()),
            )
            .child(library)
    }

    fn render_footer_note(&self) -> AnyElement {
        let note = div()
            .flex()
            .flex_1()
            .min_w_0()
            .pl(px(6.))
            .items_center()
            .gap(px(8.));
        if self.library_opened {
            return note
                .children(self.project_count.map(item_count_label))
                .into_any_element();
        }
        note.justify_end()
            .child(div().truncate().child(t!("Your recordings are here")))
            .child(doodle::doodle_once_turned(
                &onboarding_doodles::UP_ARROW,
                doodle::Palette::new(&self.theme),
                self.arrow_shown_at,
                false,
                ARROW_WIDTH,
            ))
            .into_any_element()
    }
}
