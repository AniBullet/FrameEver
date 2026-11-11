use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use gpui::{
    AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled,
    Window, div, prelude::FluentBuilder as _, px, svg,
};

use crate::{
    feeds::Feeds,
    i18n::t,
    recording::RecordingMode,
    session::{Phase, RecordingSession},
    theme::Theme,
    ui,
};

pub struct ControlsWindow {
    tray_hidden: bool,
    session: Entity<RecordingSession>,
    theme: Theme,
    has_microphone: bool,
    /// Repaints the timer. An inactive window is repainted lazily by the
    /// platform, so the tick both notifies and asks for a frame explicitly.
    _tick: gpui::Task<()>,
    ticking_live: bool,
    level_bar: Entity<ui::MicrophoneLevel>,
    options_open: bool,
    expanded: Option<InputKind>,
    microphones: Vec<String>,
    cameras: Vec<(String, crate::recording::DeviceOrModelID)>,
    dismissed_issue: Option<RecordingIssue>,
    issue_details_open: bool,
    window_height: f32,
    confirmation: Option<DestructiveAction>,
    remember_position: bool,
    last_saved_position: Option<(f64, f64)>,
    position_save_generation: Arc<AtomicU64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordingIssue {
    message: String,
    details: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputKind {
    Microphone,
    Camera,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestructiveAction {
    Restart,
    Delete,
}

impl ControlsWindow {
    pub fn new(
        session: Entity<RecordingSession>,
        has_microphone: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::theme::bind_window(window, cx);
        let theme = Theme::for_window(window, cx, false);
        cx.observe_in(&session, window, |this, _, window, cx| {
            if !matches!(this.session.read(cx).phase, Phase::Recording { .. }) {
                this.confirmation = None;
            }
            if this.issue(cx).is_none() {
                this.dismissed_issue = None;
                this.issue_details_open = false;
            }
            this.fit_height(window, cx);
            let live = this.timer_is_live(cx);
            if live != this.ticking_live {
                this.ticking_live = live;
                this._tick = Self::spawn_tick(window, cx);
            }
            cx.notify();
        })
        .detach();

        cx.observe_in(&Feeds::global(cx), window, |this, _, window, cx| {
            if this.issue(cx).is_none() {
                this.dismissed_issue = None;
            }
            this.fit_height(window, cx);
            cx.notify();
        })
        .detach();
        cx.observe_window_bounds(window, |this, window, cx| {
            if this.remember_position {
                this.queue_position_save(window, cx);
            }
        })
        .detach();
        let mic_level = Feeds::global(cx).read(cx).mic_level().clone();
        let level_bar = cx.new(|cx| {
            ui::MicrophoneLevel::new(
                ui::MicrophoneLevelKind::RecordingBar,
                &mic_level,
                window,
                cx,
            )
        });

        let mut this = Self {
            tray_hidden: false,
            session,
            theme,
            has_microphone,
            _tick: gpui::Task::ready(()),
            ticking_live: false,
            level_bar,
            options_open: false,
            expanded: None,
            microphones: Vec::new(),
            cameras: Vec::new(),
            dismissed_issue: None,
            issue_details_open: false,
            window_height: crate::app_windows::CONTROLS_BAR_HEIGHT,
            confirmation: None,
            remember_position: false,
            last_saved_position: None,
            position_save_generation: Arc::new(AtomicU64::new(0)),
        };
        this.ticking_live = this.timer_is_live(cx);
        this._tick = Self::spawn_tick(window, cx);
        cx.defer_in(window, |this, window, cx| this.fit_height(window, cx));
        this
    }

    fn issue(&self, cx: &gpui::App) -> Option<RecordingIssue> {
        let session = self.session.read(cx);
        let mut lines = Vec::new();
        if session.phase == Phase::Starting && session.countdown_overlay_unavailable {
            lines
                .push(t!("Couldn't show full-screen countdown. Use the recording bar.").to_owned());
        }
        if session.mic_disconnected {
            lines
                .push(t!("Microphone disconnected. Recording silence until it's back.").to_owned());
        }
        if Feeds::global(cx).read(cx).camera_disconnected() {
            lines.push(t!("Camera disconnected. Recording continues without it.").to_owned());
        }
        if let Some(error) = &session.error {
            lines.push(if session.pause_uncertain() {
                t!("Couldn't confirm the capture state. Stop to save what you have.").to_owned()
            } else if error.contains("Insufficient disk space") && error.contains("Your recording is still paused") {
                t!("Couldn't resume. There isn't enough disk space. Free up space and try again, or stop to save.").to_owned()
            } else {
                t!("Couldn't complete the recording action. Use Stop to save what you have. See Details for the cause.").to_owned()
            });
        }
        if session.storage_warning {
            lines.push(
                t!("Low storage. FrameEver will stop soon to save your recording.").to_owned(),
            );
        }
        (!lines.is_empty()).then(|| RecordingIssue {
            message: lines.join("\n"),
            details: session.error.clone(),
        })
    }

    fn issue_card_shown(&self, cx: &gpui::App) -> Option<RecordingIssue> {
        self.issue(cx)
            .filter(|issue| self.dismissed_issue.as_ref() != Some(issue))
    }

    fn fit_height(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let issue_room = if self.issue_card_shown(cx).is_some() {
            if self.issue_details_open { 248. } else { 136. }
        } else {
            0.
        };
        let height = crate::app_windows::controls_window_height(false, self.options_open)
            + issue_room
            + if self.confirmation.is_some() {
                152.
            } else {
                0.
            };
        if height == self.window_height {
            return;
        }
        self.window_height = height;
        let Some(native) = crate::platform::native_window(window) else {
            return;
        };
        let available = window.display(cx).map(|display| display.visible_bounds());
        cx.spawn(async move |_, _| {
            crate::app_windows::fit_controls_height(&native, height, available);
        })
        .detach();
    }

    fn queue_position_save(&mut self, window: &Window, cx: &mut Context<Self>) {
        let bounds = window.bounds();
        let position = (
            f64::from(f32::from(bounds.origin.x)),
            f64::from(f32::from(bounds.bottom()) - crate::app_windows::CONTROLS_BAR_HEIGHT),
        );
        if self.last_saved_position == Some(position) {
            return;
        }
        self.last_saved_position = Some(position);
        let generation = self.position_save_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let generations = self.position_save_generation.clone();
        let executor = cx.background_executor().clone();
        cx.background_executor()
            .spawn(async move {
                executor.timer(Duration::from_millis(200)).await;
                if generations.load(Ordering::Acquire) == generation {
                    crate::store::update(|state| state.controls_position = Some(position));
                }
            })
            .detach();
    }

    fn toggle_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.options_open = !self.options_open;
        self.confirmation = None;
        self.expanded = None;
        if self.options_open {
            cx.spawn(async move |this, cx| {
                let (microphones, cameras) = cx
                    .background_executor()
                    .spawn(async {
                        let microphones =
                            frameever_recording::feeds::microphone::MicrophoneFeed::list_names();
                        let cameras = frameever_camera::list_cameras()
                            .map(|camera| {
                                let id = camera
                                    .model_id()
                                    .cloned()
                                    .map(crate::recording::DeviceOrModelID::ModelID)
                                    .unwrap_or_else(|| {
                                        crate::recording::DeviceOrModelID::DeviceID(
                                            camera.device_id().to_string(),
                                        )
                                    });
                                (camera.display_name().to_string(), id)
                            })
                            .collect::<Vec<_>>();
                        (microphones, cameras)
                    })
                    .await;
                this.update(cx, |this, cx| {
                    this.microphones = microphones;
                    this.cameras = cameras;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        self.fit_height(window, cx);
        cx.notify();
    }

    fn toggle_issue_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let issue = self.issue(cx);
        self.dismissed_issue = if self.issue_card_shown(cx).is_some() {
            issue
        } else {
            None
        };
        self.issue_details_open = false;
        self.fit_height(window, cx);
        cx.notify();
    }

    fn toggle_camera_preview(&mut self, cx: &mut Context<Self>) {
        cx.defer(|cx| {
            if cx
                .global::<crate::app_windows::AppWindows>()
                .camera
                .is_some()
            {
                crate::app_windows::close_camera_window(cx);
            } else {
                crate::app_windows::open_camera_window(cx);
            }
        });
        cx.notify();
    }

    fn timer_is_live(&self, cx: &gpui::App) -> bool {
        matches!(
            self.session.read(cx).phase,
            Phase::Recording { paused: false }
        )
    }

    /// The timer shows whole seconds, so wake just after each one ticks over;
    /// `refresh` (not just `notify`) because this window is usually not the
    /// active one while a recording runs. Respawned whenever the timer starts
    /// or stops running, so a resume lines up with the next second.
    fn spawn_tick(window: &mut Window, cx: &mut Context<Self>) -> gpui::Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let Ok(delay) = this.update_in(cx, |this, _, cx| {
                    let session = this.session.read(cx);
                    tick_delay(this.timer_is_live(cx), session.elapsed())
                }) else {
                    return;
                };
                cx.background_executor().timer(delay).await;
                if this
                    .update_in(cx, |_, window, cx| {
                        cx.notify();
                        window.refresh();
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
    }

    fn format_elapsed(elapsed: Duration) -> String {
        let total = elapsed.as_secs();
        let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }

    fn action_button(
        &self,
        id: &'static str,
        icon: &'static str,
        disabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        div()
            .id(id)
            .w(px(28.))
            .h(px(28.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .when(!disabled, |this| {
                this.hover(|style| style.bg(theme.accent))
                    .active(|style| style.bg(theme.secondary_active))
            })
            .child(svg().path(icon).size(px(16.)).text_color(if disabled {
                theme.muted_foreground
            } else {
                theme.foreground
            }))
    }

    fn render_stop(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let session = self.session.read(cx);
        let starting = session.phase == Phase::Starting;
        let stopping = session.phase == Phase::Stopping;
        let countdown = session.countdown_remaining();
        let can_stop = (!starting && !stopping) || countdown.is_some();

        div()
            .id("stop")
            .size(px(28.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(if countdown.is_some() {
                theme.secondary
            } else {
                theme.primary
            })
            .when(can_stop, |this| {
                this.hover(|style| style.bg(theme.primary_hover))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.session.update(cx, |session, cx| session.stop(cx));
                    }))
            })
            .when(!can_stop, |this| this.opacity(0.5))
            .tooltip(move |_, cx| {
                ui::Tooltip::new(
                    &theme,
                    if countdown.is_some() {
                        t!("Cancel")
                    } else {
                        t!("Stop")
                    },
                )
                .view(cx)
            })
            .when(countdown.is_some(), |this| {
                this.child(
                    svg()
                        .path("icons/x.svg")
                        .size(px(16.))
                        .text_color(theme.foreground),
                )
            })
            .when(countdown.is_none(), |this| {
                this.child(crate::ui::stop_mark(&theme))
            })
    }

    fn render_time(&self, cx: &gpui::App) -> impl IntoElement {
        let session = self.session.read(cx);
        let label = recording_time_label(
            session.phase,
            session.countdown_remaining(),
            session.pause_pending(),
            session.elapsed(),
        );
        div()
            .min_w(px(50.))
            .flex_1()
            .min_w_0()
            .truncate()
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(13.))
            .text_color(if session.is_paused() {
                self.theme.warning_text
            } else {
                self.theme.foreground
            })
            .when(
                matches!(session.phase, Phase::Recording { paused: false })
                    && !session.pause_pending(),
                |this| {
                    this.child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(self.theme.recording)
                            .flex_shrink_0(),
                    )
                },
            )
            .child(label)
    }

    fn render_microphone(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let session = self.session.read(cx);
        let muted = session.mic_muted;
        let can_mute = session.mode() == Some(RecordingMode::Instant)
            && session.has_microphone()
            && matches!(session.phase, Phase::Recording { .. });
        self.level_bar.update(cx, |bar, cx| {
            bar.configure(
                self.has_microphone && !muted,
                theme.success.into(),
                theme.gray_10.into(),
                cx,
            );
        });

        let icon = if !self.has_microphone || muted {
            "icons/mic-off.svg"
        } else {
            "icons/mic.svg"
        };
        let icon_color: gpui::Hsla = if muted {
            theme.danger_text.into()
        } else if self.has_microphone {
            theme.foreground.into()
        } else {
            // `IconLucideMicOff text-gray-7`.
            theme.muted_foreground.into()
        };

        div()
            .id("microphone")
            .w(px(28.))
            .h(px(28.))
            .flex_shrink_0()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .when(can_mute, |this| {
                this.rounded(px(8.))
                    .hover(|style| style.bg(Theme::with_alpha(theme.gray_12, 0.06)))
                    .active(|style| style.bg(Theme::with_alpha(theme.gray_12, 0.10)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.session
                            .update(cx, |session, cx| session.toggle_mic_mute(cx));
                    }))
            })
            .child(svg().path(icon).size(px(20.)).text_color(icon_color))
            .when(self.has_microphone && !muted, |this| {
                this.child(self.level_bar.clone())
            })
    }

    fn option_row(
        &self,
        id: impl Into<gpui::ElementId>,
        enabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        div()
            .id(id)
            .w_full()
            .h(px(32.))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .rounded(px(8.))
            .text_size(px(13.))
            .text_color(theme.gray_12)
            .when(enabled, |this| {
                this.hover(|style| style.bg(Theme::with_alpha(theme.gray_12, 0.06)))
            })
            .when(!enabled, |this| this.opacity(0.5))
    }

    fn device_row(
        &self,
        kind: InputKind,
        icon: &'static str,
        label: SharedString,
        value: Option<SharedString>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let open = self.expanded == Some(kind);
        let id = match kind {
            InputKind::Microphone => "pick-microphone",
            InputKind::Camera => "pick-camera",
        };
        self.option_row(id, enabled)
            .when(enabled, |this| {
                this.on_click(cx.listener(move |this, _, _, cx| {
                    this.expanded = if this.expanded == Some(kind) {
                        None
                    } else {
                        Some(kind)
                    };
                    cx.notify();
                }))
            })
            .child(svg().path(icon).size(px(16.)).text_color(theme.gray_11))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(
                div()
                    .max_w(px(130.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(theme.gray_10)
                    .children(value),
            )
            .when(enabled, |this| {
                this.child(
                    svg()
                        .path(if open {
                            "icons/chevron-up.svg"
                        } else {
                            "icons/chevron-down.svg"
                        })
                        .size(px(14.))
                        .text_color(theme.gray_10),
                )
            })
    }

    fn render_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let session = self.session.read(cx);
        let can_change = session.can_change_inputs();
        let microphone = session.microphone_label();
        let camera = session.camera_id();
        let camera_label = camera.as_ref().and_then(|id| {
            self.cameras
                .iter()
                .find(|(_, candidate)| candidate == id)
                .map(|(label, _)| label.clone())
        });
        let camera_window_open = cx
            .global::<crate::app_windows::AppWindows>()
            .camera
            .is_some();
        let not_used: SharedString = t!("Not in this recording").into();

        let microphone_rows = (self.expanded == Some(InputKind::Microphone)).then(|| {
            self.microphones
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, name)| {
                    let checked = microphone.as_deref() == Some(name.as_str());
                    let pick = name.clone();
                    self.option_row(("microphone-choice", index), can_change)
                        .pl(px(32.))
                        .child(div().flex_1().min_w_0().truncate().child(name))
                        .when(checked, |this| {
                            this.child(
                                svg()
                                    .path("icons/check.svg")
                                    .size(px(14.))
                                    .text_color(theme.gray_11),
                            )
                        })
                        .when(can_change && !checked, |this| {
                            this.on_click(cx.listener(move |this, _, _, cx| {
                                this.expanded = None;
                                let label = pick.clone();
                                this.session.update(cx, |session, cx| {
                                    session.change_input(
                                        crate::session::InputChoice::Microphone(label),
                                        cx,
                                    )
                                });
                            }))
                        })
                })
                .collect::<Vec<_>>()
        });
        let camera_rows = (self.expanded == Some(InputKind::Camera)).then(|| {
            self.cameras
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, (name, id))| {
                    let checked = camera.as_ref() == Some(&id);
                    self.option_row(("camera-choice", index), can_change)
                        .pl(px(32.))
                        .child(div().flex_1().min_w_0().truncate().child(name))
                        .when(checked, |this| {
                            this.child(
                                svg()
                                    .path("icons/check.svg")
                                    .size(px(14.))
                                    .text_color(theme.gray_11),
                            )
                        })
                        .when(can_change && !checked, |this| {
                            this.on_click(cx.listener(move |this, _, _, cx| {
                                this.expanded = None;
                                let selection = crate::deeplink::camera_selection(id.clone());
                                this.session.update(cx, |session, cx| {
                                    session.change_input(
                                        crate::session::InputChoice::Camera(selection),
                                        cx,
                                    )
                                });
                            }))
                        })
                })
                .collect::<Vec<_>>()
        });

        div()
            .id("recording-options")
            .w_full()
            .max_h(px(crate::app_windows::CONTROLS_OPTIONS_HEIGHT - 8.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .mb(px(8.))
            .p(px(4.))
            .rounded(px(12.))
            .border_1()
            .border_color(theme.gray_5)
            .bg(theme.gray_1)
            .child(
                self.device_row(
                    InputKind::Microphone,
                    "icons/mic.svg",
                    t!("Microphone").into(),
                    Some(
                        microphone
                            .clone()
                            .map_or_else(|| not_used.clone(), Into::into),
                    ),
                    microphone.is_some() && can_change,
                    cx,
                ),
            )
            .children(microphone_rows.into_iter().flatten())
            .child(self.device_row(
                InputKind::Camera,
                "icons/webcam.svg",
                t!("Camera").into(),
                Some(camera.as_ref().map_or_else(
                    || not_used.clone(),
                    |_| camera_label.unwrap_or_default().into(),
                )),
                camera.is_some() && can_change,
                cx,
            ))
            .children(camera_rows.into_iter().flatten())
            .child(
                self.option_row("camera-preview", camera.is_some())
                    .child(
                        svg()
                            .path("icons/maximize.svg")
                            .size(px(16.))
                            .text_color(theme.gray_11),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(t!("Show camera preview")),
                    )
                    .child(ui::Toggle::settings(
                        &theme,
                        "camera-preview-switch",
                        camera_window_open,
                    ))
                    .when(camera.is_some(), |this| {
                        this.on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_camera_preview(cx);
                        }))
                    }),
            )
    }

    fn confirm_action(
        &mut self,
        action: DestructiveAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.session.read(cx).phase, Phase::Recording { .. })
            || self.session.read(cx).pause_unavailable()
        {
            return;
        }
        self.options_open = false;
        self.confirmation = Some(action);
        self.fit_height(window, cx);
        cx.notify();
    }

    fn cancel_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirmation = None;
        self.fit_height(window, cx);
        cx.notify();
    }

    fn accept_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.confirmation.take() else {
            return;
        };
        self.fit_height(window, cx);
        self.session.update(cx, |session, cx| match action {
            DestructiveAction::Restart => session.restart(cx),
            DestructiveAction::Delete => session.delete(cx),
        });
        cx.notify();
    }

    fn render_confirmation(
        &self,
        action: DestructiveAction,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let (title, message, accept) = match action {
            DestructiveAction::Restart => (
                t!("Restart this recording?"),
                t!("What you've recorded so far is discarded. This can't be undone."),
                t!("Restart"),
            ),
            DestructiveAction::Delete => (
                t!("Delete this recording?"),
                t!("It won't be saved. This can't be undone."),
                t!("Delete"),
            ),
        };
        div()
            .id("recording-confirmation")
            .h(px(144.))
            .flex_shrink_0()
            .w_full()
            .p(px(12.))
            .mb(px(8.))
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_size(px(13.))
            .text_color(theme.foreground)
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(div().text_color(theme.muted_foreground).child(message))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        ui::Button::plain(
                            &theme,
                            "cancel-recording-action",
                            ui::ButtonVariant::Gray,
                            ui::ButtonSize::Sm,
                        )
                        .label(t!("Cancel"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.cancel_confirmation(window, cx)),
                        ),
                    )
                    .child(
                        ui::Button::plain(
                            &theme,
                            "accept-recording-action",
                            ui::ButtonVariant::Gray,
                            ui::ButtonSize::Sm,
                        )
                        .text_color(theme.danger_text.into())
                        .label(accept)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.accept_confirmation(window, cx)),
                        ),
                    ),
            )
    }

    pub(crate) fn tray_hidden(&self) -> bool {
        self.tray_hidden
    }

    pub(crate) fn set_tray_hidden(&mut self, hidden: bool, window: &mut Window) {
        let Some(native) = crate::platform::native_window(window) else {
            tracing::error!("recording controls have no native window");
            return;
        };
        if hidden {
            crate::platform::hide_native(&native);
        } else {
            crate::platform::show_window_without_focus(window);
        }
        self.tray_hidden = hidden;
    }

    pub(crate) fn restart_from_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_tray_hidden(false, window);
        self.confirm_action(DestructiveAction::Restart, window, cx);
    }

    fn render_issue(&self, issue: RecordingIssue, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let critical = issue.details.is_some();
        div()
            .id("recording-issue")
            .w_full()
            .h(px(if self.issue_details_open { 240. } else { 128. }))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .mb(px(8.))
            .p(px(8.))
            .rounded(px(12.))
            .border_1()
            .border_color(if critical {
                theme.danger
            } else {
                theme.warning
            })
            .bg(theme.popover)
            .text_color(theme.foreground)
            .text_size(px(13.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .child(
                        svg()
                            .path("icons/triangle-alert.svg")
                            .size(px(16.))
                            .flex_shrink_0()
                            .text_color(if critical {
                                theme.danger_text
                            } else {
                                theme.warning_text
                            }),
                    )
                    .child(
                        div()
                            .id("recording-issue-text")
                            .flex_1()
                            .min_w_0()
                            .max_h(px(72.))
                            .overflow_y_scroll()
                            .child(issue.message),
                    )
                    .child(
                        self.action_button("dismiss-issue", "icons/x.svg", false)
                            .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Dismiss")).view(cx))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_issue_card(window, cx)
                            })),
                    ),
            )
            .when_some(issue.details, |this, details| {
                this.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            ui::Button::plain(
                                &theme,
                                "recording-details",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Sm,
                            )
                            .label(t!("Details"))
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.issue_details_open = !this.issue_details_open;
                                    this.fit_height(window, cx);
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            ui::Button::plain(
                                &theme,
                                "recording-export-logs",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Sm,
                            )
                            .label(t!("Export logs"))
                            .on_click(cx.listener(
                                |_, _, window, cx| {
                                    cx.spawn_in(window, async move |_, cx| {
                                        let Some(path) = crate::platform::save_file_panel_async(
                                            "frameever-recording-logs.txt",
                                            &["txt"],
                                            cx,
                                        )
                                        .await
                                        else {
                                            return;
                                        };
                                        let result = cx
                                            .background_executor()
                                            .spawn(async move {
                                                crate::diagnostics::export_logs(&path)
                                                    .map(|()| path)
                                            })
                                            .await;
                                        match result {
                                            Ok(path) => {
                                                cx.update(|_, cx| {
                                                    crate::library::reveal_path(&path, cx)
                                                })
                                                .ok();
                                            }
                                            Err(error) => {
                                                tracing::error!(
                                                    error,
                                                    "could not export recording logs"
                                                );
                                                crate::platform::alert_dialog(
                                                    t!("Couldn't export logs"),
                                                    t!("Choose a writable folder and try again."),
                                                );
                                            }
                                        }
                                    })
                                    .detach();
                                },
                            )),
                        ),
                )
                .when(self.issue_details_open, |this| {
                    this.child(
                        div()
                            .id("recording-raw-error")
                            .min_h_0()
                            .flex_1()
                            .overflow_y_scroll()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(details),
                    )
                })
            })
    }

    fn render_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let session = self.session.read(cx);
        let paused = session.is_paused();
        let busy = !matches!(session.phase, Phase::Recording { .. }) || session.pause_unavailable();
        let pause_pending = session.pause_pending();
        let pause_tooltip = if session.pause_uncertain() {
            t!("Capture state is unconfirmed. Use Stop.")
        } else if pause_pending {
            if paused {
                t!("Resuming recording…")
            } else {
                t!("Pausing recording…")
            }
        } else if paused {
            t!("Resume recording")
        } else {
            t!("Pause recording")
        };

        div()
            .h(px(36.))
            .flex_shrink_0()
            .w_full()
            .flex()
            .items_center()
            .gap(px(2.))
            .px(px(4.))
            .rounded(px(20.))
            .bg(theme.popover)
            .border_1()
            .border_color(if paused { theme.warning } else { theme.border })
            .child(
                div()
                    .id("drag")
                    .w(px(16.))
                    .h(px(28.))
                    .flex_shrink_0()
                    .cursor_move()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, _| {
                            this.remember_position = true;
                            window.start_window_move();
                        }),
                    )
                    .child(
                        svg()
                            .path("icons/grip-vertical.svg")
                            .size(px(14.))
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(self.render_time(cx))
            .child(self.render_microphone(cx))
            .child(
                self.action_button(
                    "pause",
                    if paused {
                        "icons/play.svg"
                    } else {
                        "icons/pause.svg"
                    },
                    busy,
                )
                .when(pause_pending, |this| this.opacity(0.5))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, pause_tooltip).view(cx))
                .when(!busy, |this| {
                    this.on_click(cx.listener(|this, _, _, cx| {
                        this.session
                            .update(cx, |session, cx| session.toggle_pause(cx));
                    }))
                }),
            )
            .child(
                self.action_button("restart", "icons/rotate-ccw.svg", busy)
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Restart")).view(cx))
                    .when(!busy, |this| {
                        this.on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_action(DestructiveAction::Restart, window, cx);
                        }))
                    }),
            )
            .when(self.issue(cx).is_some(), |this| {
                this.child(
                    self.action_button("issue", "icons/triangle-alert.svg", false)
                        .tooltip(move |_, cx| {
                            ui::Tooltip::new(&theme, t!("Recording issue")).view(cx)
                        })
                        .on_click(
                            cx.listener(|this, _, window, cx| this.toggle_issue_card(window, cx)),
                        ),
                )
            })
            .child(
                div()
                    .w(px(1.))
                    .h(px(16.))
                    .mx(px(6.))
                    .bg(theme.border)
                    .flex_shrink_0(),
            )
            .child(
                self.action_button("delete", "icons/trash.svg", busy)
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Delete")).view(cx))
                    .when(!busy, |this| {
                        this.on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_action(DestructiveAction::Delete, window, cx);
                        }))
                    }),
            )
            .child(
                self.action_button("draw", "icons/pencil.svg", true)
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Draw on screen")).view(cx)),
            )
            .child(
                self.action_button("settings", "icons/sliders-horizontal.svg", false)
                    .when(self.options_open, |this| this.bg(theme.selection))
                    .tooltip(move |_, cx| {
                        ui::Tooltip::new(&theme, t!("Recording options")).view(cx)
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_options(window, cx))),
            )
            .child(
                div()
                    .w(px(1.))
                    .h(px(16.))
                    .mx(px(6.))
                    .bg(theme.border)
                    .flex_shrink_0(),
            )
            .child(self.render_stop(cx))
    }
}

impl Render for ControlsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme.refresh(window, cx, false);
        let issue = self.issue_card_shown(cx);
        let options = self.options_open.then(|| self.render_options(cx));

        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .p(px(12.))
            .font_family(crate::theme::ui_font())
            // `body { font-weight: 500 }` (`ui-solid/src/main.css:189-192`).
            .font_weight(FontWeight::MEDIUM)
            .when_some(options, |this, options| this.child(options))
            .when_some(self.confirmation, |this, action| {
                this.child(self.render_confirmation(action, cx))
            })
            .when_some(issue, |this, issue| {
                this.child(self.render_issue(issue, cx))
            })
            .child(self.render_bar(cx))
    }
}

fn recording_time_label(
    phase: Phase,
    countdown: Option<u32>,
    pending: bool,
    elapsed: Duration,
) -> String {
    if let Some(count) = countdown.filter(|count| *count > 0) {
        t!("Starts in {count}", count = count)
    } else if phase == Phase::Starting {
        t!("Starting…").to_owned()
    } else if phase == Phase::Stopping {
        t!("Saving…").to_owned()
    } else if pending {
        if matches!(phase, Phase::Recording { paused: true }) {
            t!("Resuming…")
        } else {
            t!("Pausing…")
        }
        .to_owned()
    } else if matches!(phase, Phase::Recording { paused: true }) {
        format!(
            "{} {}",
            t!("Paused"),
            ControlsWindow::format_elapsed(elapsed)
        )
    } else {
        ControlsWindow::format_elapsed(elapsed)
    }
}

fn tick_delay(live: bool, elapsed: Duration) -> Duration {
    if live {
        until_next_second(elapsed)
    } else {
        Duration::from_secs(1)
    }
}

fn until_next_second(elapsed: Duration) -> Duration {
    Duration::from_secs(1)
        .saturating_sub(Duration::from_nanos(u64::from(elapsed.subsec_nanos())))
        .max(Duration::from_millis(20))
}

#[cfg(test)]
mod repaint_tests {
    use super::*;

    #[gpui::test]
    fn recording_issues_keep_raw_causes_in_details_and_can_be_reopened(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            Feeds::init(cx);
            RecordingSession::init(cx);
        });
        let (controls, cx) = cx.add_window_view(|window, cx| {
            ControlsWindow::new(RecordingSession::global(cx), false, window, cx)
        });
        controls.update_in(cx, |controls, window, cx| {
            controls.session.update(cx, |session, _| {
                session.error = Some("raw encoder error 0xdead".into());
                session.mic_disconnected = true;
                session.storage_warning = true;
            });
            let issue = controls.issue(cx).unwrap();
            assert!(!issue.message.contains("0xdead"));
            assert!(issue.message.contains("Stop"));
            assert!(issue.message.contains("Microphone disconnected"));
            assert!(issue.message.contains("Low storage"));
            assert_eq!(issue.details.as_deref(), Some("raw encoder error 0xdead"));
            controls.toggle_issue_card(window, cx);
            assert!(controls.issue_card_shown(cx).is_none());
            controls.toggle_issue_card(window, cx);
            assert_eq!(controls.issue_card_shown(cx), Some(issue));
            controls.toggle_issue_card(window, cx);
            controls.session.update(cx, |session, _| {
                session.error = Some("a different raw failure".into())
            });
            assert!(controls.issue_card_shown(cx).is_some());
        });
    }

    #[gpui::test]
    fn destructive_actions_open_a_card_and_cancelling_keeps_the_recording(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            Feeds::init(cx);
            RecordingSession::init(cx);
        });
        let (controls, cx) = cx.add_window_view(|window, cx| {
            let session = RecordingSession::global(cx);
            session.update(cx, |session, _| {
                session.phase = Phase::Recording { paused: false }
            });
            ControlsWindow::new(session, false, window, cx)
        });
        controls.update_in(cx, |controls, window, cx| {
            controls.confirm_action(DestructiveAction::Restart, window, cx);
            assert_eq!(controls.confirmation, Some(DestructiveAction::Restart));
            assert_eq!(
                controls.session.read(cx).phase,
                Phase::Recording { paused: false }
            );
            controls.cancel_confirmation(window, cx);
            assert_eq!(controls.confirmation, None);
            controls.confirm_action(DestructiveAction::Delete, window, cx);
            assert_eq!(controls.confirmation, Some(DestructiveAction::Delete));
            controls.cancel_confirmation(window, cx);
            assert_eq!(
                controls.session.read(cx).phase,
                Phase::Recording { paused: false }
            );
        });
    }

    #[test]
    fn paused_recordings_keep_the_elapsed_time_and_pending_states_stay_truthful() {
        let elapsed = Duration::from_secs(3753);
        assert_eq!(
            recording_time_label(Phase::Recording { paused: true }, None, false, elapsed),
            "Paused 1:02:33"
        );
        assert_eq!(
            recording_time_label(Phase::Recording { paused: false }, None, true, elapsed),
            "Pausing…"
        );
        assert_eq!(
            recording_time_label(Phase::Recording { paused: true }, None, true, elapsed),
            "Resuming…"
        );
        assert_eq!(
            recording_time_label(Phase::Starting, Some(3), false, elapsed),
            "Starts in 3"
        );
        assert_eq!(
            recording_time_label(Phase::Stopping, None, false, elapsed),
            "Saving…"
        );
    }

    #[test]
    fn the_timer_wakes_once_per_displayed_second() {
        assert_eq!(
            until_next_second(Duration::from_millis(3_250)),
            Duration::from_millis(750)
        );
        assert_eq!(
            until_next_second(Duration::from_millis(3_995)),
            Duration::from_millis(20)
        );
        assert_eq!(until_next_second(Duration::ZERO), Duration::from_secs(1));
    }

    #[test]
    fn the_timer_shows_hours_from_an_hour_on() {
        let label = |seconds| ControlsWindow::format_elapsed(Duration::from_secs_f64(seconds));
        assert_eq!(label(0.), "0:00");
        assert_eq!(label(95.7), "1:35");
        assert_eq!(label(3_599.9), "59:59");
        assert_eq!(label(3_600.), "1:00:00");
        assert_eq!(label(3_905.), "1:05:05");
    }

    #[test]
    fn a_resumed_timer_wakes_at_the_next_second_and_a_paused_one_idles() {
        assert_eq!(
            tick_delay(true, Duration::from_millis(3_900)),
            Duration::from_millis(100)
        );
        assert_eq!(
            tick_delay(false, Duration::from_millis(3_900)),
            Duration::from_secs(1)
        );
    }
}
