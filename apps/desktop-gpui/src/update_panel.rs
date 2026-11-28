use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, FontWeight, Global, Hsla, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Window, WindowBounds, WindowHandle, WindowKind, WindowOptions, div,
    prelude::FluentBuilder as _, px, size, svg,
};

use crate::{
    brand::RELEASES_REPO,
    i18n::t,
    installer::Release,
    store::UpdateChannel,
    theme::Theme,
    ui::{self, Button, ButtonSize, ButtonVariant},
    updates::{self, Indicator, UpdateFailure, UpdateStatus},
};

const WIDTH: f32 = 440.;
const HEIGHT: f32 = 360.;
const BUSY_RECHECK: std::time::Duration = std::time::Duration::from_secs(1);

#[derive(Default)]
struct PanelGlobal {
    handle: Option<WindowHandle<UpdatePanel>>,
}

impl Global for PanelGlobal {}

pub fn open(cx: &mut App) {
    if let Some(handle) = cx
        .try_global::<PanelGlobal>()
        .and_then(|panel| panel.handle)
    {
        let reused = handle
            .update(cx, |_, window, cx| {
                window.activate_window();
                cx.notify();
            })
            .is_ok();
        if reused {
            cx.activate(true);
            return;
        }
    }

    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            kind: WindowKind::Normal,
            focus: true,
            show: true,
            is_resizable: false,
            is_minimizable: false,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title(t!("Update FrameEver"));
            cx.new(|cx| UpdatePanel::new(window, cx))
        },
    );
    match opened {
        Ok(handle) => {
            cx.set_global(PanelGlobal {
                handle: Some(handle),
            });
            cx.activate(true);
        }
        Err(error) => tracing::warn!("update panel failed to open: {error:#}"),
    }
}

pub(crate) fn refresh(cx: &mut App) {
    if let Some(handle) = cx
        .try_global::<PanelGlobal>()
        .and_then(|panel| panel.handle)
    {
        handle.update(cx, |_, _, cx| cx.notify()).ok();
    }
}

fn closed(cx: &mut App) {
    cx.set_global(PanelGlobal::default());
}

fn releases_url(release: Option<&Release>) -> String {
    match release {
        Some(release) => format!(
            "https://github.com/{RELEASES_REPO}/releases/tag/v{}",
            release.version
        ),
        None => format!("https://github.com/{RELEASES_REPO}/releases"),
    }
}

pub(crate) fn note_lines(notes: &str) -> Vec<String> {
    notes
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches(['-', '*', '•'])
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn channel_label(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => t!("Stable"),
        UpdateChannel::Nightly => t!("Pre-release"),
    }
}

pub(crate) fn title_pill(theme: Theme, cx: &App) -> Option<gpui::Stateful<gpui::Div>> {
    let indicator = updates::current_indicator(cx)?;
    let (icon, label, failed): (Option<&str>, SharedString, bool) = match &indicator {
        Indicator::Available(version) => (
            Some("icons/download.svg"),
            t!("{version} available", version = version).into(),
            false,
        ),
        Indicator::Downloading { percent, .. } => (
            None,
            match percent {
                Some(percent) => t!("Downloading {percent}%", percent = percent).into(),
                None => t!("Downloading…").into(),
            },
            false,
        ),
        Indicator::Ready(_) => (
            Some("icons/refresh-cw.svg"),
            t!("Restart to update").into(),
            false,
        ),
        Indicator::Installing(_) => (None, t!("Installing…").into(), false),
        Indicator::Failed => (
            Some("icons/triangle-alert.svg"),
            t!("Update failed").into(),
            true,
        ),
    };
    let ink: Hsla = if failed {
        theme.danger_text.into()
    } else {
        theme.primary.into()
    };
    Some(
        div()
            .id("update-pill")
            .when(cfg!(target_os = "windows"), |this| this.occlude())
            .tab_index(0)
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.))
            .h(px(22.))
            .px(px(8.))
            .rounded(px(11.))
            .border_1()
            .border_color(ink.opacity(0.4))
            .text_size(px(12.))
            .text_color(ink)
            .cursor_pointer()
            .hover(move |style| style.bg(theme.accent))
            .when_some(icon, |this, icon| {
                this.child(svg().path(icon).size(px(14.)).text_color(ink))
            })
            .child(label)
            .on_click(|_, _, cx| updates::open_panel(cx)),
    )
}

pub struct UpdatePanel {
    theme: Theme,
    focus: FocusHandle,
    busy_recheck: Option<gpui::Task<()>>,
}

impl UpdatePanel {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.on_window_should_close(cx, |_, cx| {
            cx.defer(closed);
            true
        });
        crate::theme::bind_window(window, cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            theme: Theme::for_window(window, cx, true),
            focus,
            busy_recheck: None,
        }
    }

    fn recheck_while_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        if !busy {
            self.busy_recheck = None;
            return;
        }
        if self.busy_recheck.is_some() {
            return;
        }
        self.busy_recheck = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BUSY_RECHECK).await;
            this.update(cx, |this, cx| {
                this.busy_recheck = None;
                cx.notify();
            })
            .ok();
        }));
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.remove_window();
        cx.defer(closed);
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" {
            self.close(window, cx);
        }
    }

    fn button(
        &self,
        id: &'static str,
        variant: ButtonVariant,
        label: impl Into<SharedString>,
        disabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Button {
        Button::plain(&self.theme, id, variant, ButtonSize::Sm)
            .label(label)
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
    }

    fn link(&self, id: &'static str, label: &'static str, url: String) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(4.))
            .text_size(px(12.))
            .text_color(theme.link)
            .cursor_pointer()
            .hover(|style| style.underline())
            .child(label)
            .child(
                svg()
                    .path("icons/external-link.svg")
                    .size(px(12.))
                    .text_color(theme.link),
            )
            .on_click(move |_, _, cx| cx.open_url(&url))
    }

    fn notes(&self, release: &Release) -> impl IntoElement {
        let theme = self.theme;
        let lines = release.notes.as_deref().map(note_lines).unwrap_or_default();
        div()
            .id("update-notes")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap(px(6.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(theme.muted)
            .text_size(px(13.))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(t!("What's new in {version}", version = release.version)),
            )
            .children(lines.into_iter().map(move |line| {
                div()
                    .flex()
                    .gap(px(6.))
                    .text_color(theme.muted_foreground)
                    .child("•")
                    .child(div().flex_1().min_w_0().child(line))
            }))
            .child(self.link(
                "update-changelog",
                t!("Full changelog"),
                releases_url(Some(release)),
            ))
    }

    fn progress(&self, fraction: Option<f32>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .h(px(4.))
            .w_full()
            .rounded(px(2.))
            .bg(theme.muted)
            .child(
                div()
                    .h_full()
                    .rounded(px(2.))
                    .bg(theme.primary)
                    .w(gpui::relative(fraction.unwrap_or(0.).clamp(0., 1.))),
            )
    }

    fn message(&self, icon: &'static str, color: Hsla, text: String) -> impl IntoElement {
        div()
            .flex()
            .items_start()
            .gap(px(8.))
            .text_size(px(13.))
            .text_color(self.theme.foreground)
            .child(svg().path(icon).size(px(16.)).text_color(color))
            .child(div().flex_1().min_w_0().child(text))
    }

    fn actions(&self) -> gpui::Div {
        div().flex().items_center().gap(px(8.))
    }

    fn body(&mut self, status: &UpdateStatus, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let grow = || div().flex_1();
        match status {
            UpdateStatus::Available { release } => {
                let skipped = updates::skipped_version(cx).as_ref() == Some(&release.version);
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(12.))
                    .child(self.notes(release))
                    .child(
                        self.actions()
                            .map(|row| {
                                if skipped {
                                    row.child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.muted_foreground)
                                            .child(t!("You skipped this version.")),
                                    )
                                } else {
                                    row.child(self.button(
                                        "update-skip",
                                        ButtonVariant::Outline,
                                        t!("Skip this version"),
                                        false,
                                        cx,
                                        |this, window, cx| {
                                            updates::skip_version(cx);
                                            this.close(window, cx);
                                        },
                                    ))
                                }
                            })
                            .child(grow())
                            .child(self.button(
                                "update-later",
                                ButtonVariant::Gray,
                                t!("Remind me later"),
                                false,
                                cx,
                                |this, window, cx| {
                                    updates::remind_later(cx);
                                    this.close(window, cx);
                                },
                            ))
                            .child(self.button(
                                "update-download",
                                ButtonVariant::Primary,
                                t!("Download and install"),
                                false,
                                cx,
                                |_, _, cx| updates::download(cx),
                            )),
                    )
                    .into_any_element()
            }
            UpdateStatus::Downloading { release, fraction } => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap(px(12.))
                .child(self.notes(release))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            self.actions()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child(match fraction {
                                    Some(fraction) => t!(
                                        "Downloading · {percent}%",
                                        percent = (fraction * 100.).round() as u32
                                    ),
                                    None => t!("Downloading…").to_string(),
                                })
                                .child(grow())
                                .child(self.button(
                                    "update-cancel",
                                    ButtonVariant::Gray,
                                    t!("Cancel"),
                                    false,
                                    cx,
                                    |_, _, cx| updates::cancel_download(cx),
                                )),
                        )
                        .child(self.progress(*fraction)),
                )
                .child(self.actions().child(grow()).child(self.button(
                    "update-hide",
                    ButtonVariant::Gray,
                    t!("Hide"),
                    false,
                    cx,
                    |this, window, cx| this.close(window, cx),
                )))
                .into_any_element(),
            UpdateStatus::Ready { .. } | UpdateStatus::Installed { .. } => {
                let busy = updates::work_in_flight(cx);
                self.recheck_while_busy(busy, cx);
                let notes = match status {
                    UpdateStatus::Ready { release } => Some(self.notes(release)),
                    _ => None,
                };
                let without_notes = notes.is_none();
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(12.))
                    .children(notes)
                    .when(without_notes, |this| this.child(grow()))
                    .child(
                        self.actions()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(if busy {
                                        updates::busy_message()
                                    } else {
                                        t!("Your recordings and projects are kept.")
                                    }),
                            )
                            .child(self.button(
                                "update-ready-later",
                                ButtonVariant::Gray,
                                t!("Later"),
                                false,
                                cx,
                                |this, window, cx| this.close(window, cx),
                            ))
                            .child(self.button(
                                "update-restart",
                                ButtonVariant::Primary,
                                t!("Restart to update"),
                                busy,
                                cx,
                                |_, _, cx| updates::install_and_relaunch(cx),
                            )),
                    )
                    .into_any_element()
            }
            UpdateStatus::Installing { version } => div()
                .flex()
                .flex_col()
                .flex_1()
                .justify_center()
                .gap(px(10.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme.foreground)
                        .child(t!(
                            "Installing {version}… FrameEver reopens by itself.",
                            version = version
                        )),
                )
                .child(self.progress(Some(1.)))
                .into_any_element(),
            UpdateStatus::Failed(failure) => {
                let release = match failure {
                    UpdateFailure::Download { release, .. } => Some(release.clone()),
                    UpdateFailure::Check(_) => None,
                };
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(12.))
                    .child(self.message(
                        "icons/triangle-alert.svg",
                        theme.danger_text.into(),
                        failure.message().to_string(),
                    ))
                    .child(grow())
                    .child(
                        self.actions()
                            .child(self.link(
                                "update-github",
                                t!("Download from GitHub"),
                                releases_url(release.as_ref()),
                            ))
                            .child(grow())
                            .child(self.button(
                                "update-failed-close",
                                ButtonVariant::Gray,
                                t!("Close"),
                                false,
                                cx,
                                |this, window, cx| this.close(window, cx),
                            ))
                            .child(self.button(
                                "update-retry",
                                ButtonVariant::Primary,
                                t!("Try again"),
                                false,
                                cx,
                                |_, _, cx| updates::retry(cx),
                            )),
                    )
                    .into_any_element()
            }
            UpdateStatus::Checking => div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .text_color(theme.muted_foreground)
                .child(t!("Checking for updates…"))
                .into_any_element(),
            UpdateStatus::UpToDate | UpdateStatus::Idle => div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(12.))
                .child(self.message(
                    "icons/circle-check.svg",
                    theme.primary.into(),
                    t!(
                        "FrameEver {version} is the newest version.",
                        version = updates::current_version_label()
                    ),
                ))
                .child(grow())
                .child(self.actions().child(grow()).child(self.button(
                    "update-current-close",
                    ButtonVariant::Gray,
                    t!("Close"),
                    false,
                    cx,
                    |this, window, cx| this.close(window, cx),
                )))
                .into_any_element(),
        }
    }
}

fn heading(status: &UpdateStatus) -> String {
    match status {
        UpdateStatus::Available { release }
        | UpdateStatus::Downloading { release, .. }
        | UpdateStatus::Ready { release } => {
            t!("FrameEver {version}", version = release.version)
        }
        UpdateStatus::Installing { version } | UpdateStatus::Installed { version, .. } => {
            t!("FrameEver {version}", version = version)
        }
        UpdateStatus::Failed(_) => t!("Update FrameEver").to_string(),
        UpdateStatus::Checking => t!("Checking for updates…").to_string(),
        UpdateStatus::UpToDate | UpdateStatus::Idle => t!("FrameEver is up to date").to_string(),
    }
}

impl Render for UpdatePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = Theme::for_window(window, cx, true);
        let theme = self.theme;
        let status = updates::status(cx);
        let subtitle = t!(
            "You have {version} · {channel}",
            version = updates::current_version_label(),
            channel = channel_label(updates::channel(cx))
        );
        div()
            .id("update-panel")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(18.))
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .font_family(crate::theme::ui_font())
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                        window.start_window_move();
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(heading(&status)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(subtitle),
                            ),
                    )
                    .child(
                        div()
                            .id("update-close")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Close")).view(cx))
                            .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size(px(14.))
                                    .text_color(theme.muted_foreground),
                            )
                            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation()
                            })
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    ),
            )
            .child(self.body(&status, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_become_plain_bullets_without_headings() {
        assert_eq!(
            note_lines(
                "## 1.2.0\n\n- Library: open an export\n* Compression queue\n  • Fixed a hidden prompt\n"
            ),
            vec![
                "Library: open an export".to_string(),
                "Compression queue".to_string(),
                "Fixed a hidden prompt".to_string(),
            ]
        );
    }
}
