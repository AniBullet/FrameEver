use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px, svg,
    transparent_black,
};

use frameever_recording::sources::screen_capture::ScreenCaptureTarget;

use crate::{
    feeds::Feeds,
    i18n::t,
    recording::RecordingMode,
    screen_ink::{INK_COLOURS, InkAction, InkTool, ScreenInk},
    session::{InputKind, Phase, RecordingSession},
    theme::Theme,
    ui,
};

mod keyboard;

use keyboard::{Command, Control, Group};

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
    options_header: Option<SharedString>,
    dismissed_issue: Option<RecordingIssue>,
    issue_details_open: bool,
    window_height: f32,
    confirmation: Option<DestructiveAction>,
    remember_position: bool,
    last_saved_position: Option<(f64, f64)>,
    position_save_generation: Arc<AtomicU64>,
    ink: Option<Entity<ScreenInk>>,
    focus: FocusHandle,
    focused: Option<Control>,
    opener: Option<Control>,
    return_to: Option<isize>,
    ringed: Option<Control>,
}

const DRAW_STRIP_ROOM: f32 = 44.;

const OPTIONS_WIDTH: f32 = 300.;

fn source_label(target: &ScreenCaptureTarget) -> Option<&'static str> {
    match target {
        ScreenCaptureTarget::Display { .. } => Some(t!("Display")),
        ScreenCaptureTarget::Window { .. } => Some(t!("Window")),
        ScreenCaptureTarget::Area { .. } => Some(t!("Area")),
        ScreenCaptureTarget::CameraOnly => None,
    }
}

fn format_options_header(
    mode: RecordingMode,
    source: Option<&str>,
    size: Option<(u32, u32)>,
    fps: u32,
) -> String {
    let mode = match mode {
        RecordingMode::Studio => t!("Studio"),
        RecordingMode::Instant => t!("Instant"),
    };
    std::iter::once(mode.to_owned())
        .chain(source.map(str::to_owned))
        .chain(size.map(|(width, height)| format!("{width} × {height}")))
        .chain(std::iter::once(t!("{fps} FPS", fps = fps)))
        .collect::<Vec<_>>()
        .join(" · ")
}

const DRAW_TOOLS: [InkTool; 3] = [InkTool::Pen, InkTool::Highlighter, InkTool::Arrow];

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordingIssue {
    message: String,
    details: Option<String>,
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
        let ink = ScreenInk::try_global(cx);
        if let Some(ink) = &ink {
            cx.observe_in(ink, window, |this, _, window, cx| {
                this.fit_height(window, cx);
                cx.notify();
            })
            .detach();
        }
        cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.focused = None;
                this.opener = None;
                this.return_to = None;
                cx.notify();
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
            options_header: None,
            dismissed_issue: None,
            issue_details_open: false,
            window_height: crate::app_windows::CONTROLS_BAR_HEIGHT,
            confirmation: None,
            remember_position: false,
            last_saved_position: None,
            position_save_generation: Arc::new(AtomicU64::new(0)),
            ink,
            focus: cx.focus_handle(),
            focused: None,
            opener: None,
            return_to: None,
            ringed: None,
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
            }
            + if self.drawing(cx) {
                DRAW_STRIP_ROOM
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

    fn drawing(&self, cx: &gpui::App) -> bool {
        self.ink.as_ref().is_some_and(|ink| ink.read(cx).active())
    }

    fn toggle_draw(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::screen_ink::can_draw(self.session.read(cx).phase)
            || self.session.read(cx).pause_unavailable()
        {
            return;
        }
        self.options_open = false;
        self.expanded = None;
        self.confirmation = None;
        self.fit_height(window, cx);
        cx.defer(crate::screen_ink::toggle);
        cx.notify();
    }

    fn ink_action(&mut self, action: InkAction, cx: &mut Context<Self>) {
        if let Some(ink) = &self.ink {
            ink.update(cx, |ink, cx| ink.apply(action, cx));
        }
    }

    fn render_draw_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let (tool, colour) = self
            .ink
            .as_ref()
            .map(|ink| {
                let ink = ink.read(cx);
                (ink.ink().tool(), ink.ink().colour())
            })
            .unwrap_or((InkTool::Pen, 0));
        let separator = || {
            div()
                .w(px(1.))
                .h(px(16.))
                .mx(px(6.))
                .bg(theme.border)
                .flex_shrink_0()
        };
        let tools = [
            ("draw-pen", "icons/pencil.svg", t!("Pen"), "P"),
            (
                "draw-highlighter",
                "icons/highlighter.svg",
                t!("Highlighter"),
                "H",
            ),
            ("draw-arrow", "icons/arrow-up-right.svg", t!("Arrow"), "A"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (id, icon, label, key))| {
            self.action_button(id, icon, false, Control::DrawTool(index))
                .when(tool == DRAW_TOOLS[index], |this| this.bg(theme.selection))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).keys([key]).view(cx))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.press(Control::DrawTool(index), window, cx);
                }))
        })
        .collect::<Vec<_>>();
        let swatches = (0..INK_COLOURS)
            .map(|index| {
                let number = index + 1;
                self.ring(
                    div()
                        .id(("draw-colour", index))
                        .size(px(24.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full(),
                    Control::DrawColour(index),
                )
                .child(
                    div()
                        .size(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_2()
                        .border_color(if colour == index {
                            theme.foreground
                        } else {
                            theme.popover
                        })
                        .child(
                            div()
                                .size(px(14.))
                                .rounded_full()
                                .bg(crate::screen_ink::ink_colour(index)),
                        ),
                )
                .tooltip(move |_, cx| {
                    ui::Tooltip::new(&theme, t!("Colour {number}", number = number))
                        .keys([number.to_string()])
                        .view(cx)
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.press(Control::DrawColour(index), window, cx);
                }))
            })
            .collect::<Vec<_>>();
        div()
            .id("draw-strip")
            .h(px(36.))
            .mb(px(8.))
            .flex_shrink_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .px(px(4.))
                    .rounded(px(20.))
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .children(tools)
                    .child(separator())
                    .children(swatches)
                    .child(separator())
                    .child(
                        self.action_button(
                            "draw-undo",
                            "icons/undo-2.svg",
                            false,
                            Control::DrawUndo,
                        )
                        .tooltip(move |_, cx| {
                            ui::Tooltip::new(&theme, t!("Undo"))
                                .keys(["Ctrl", "Z"])
                                .view(cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.press(Control::DrawUndo, window, cx);
                        })),
                    )
                    .child(
                        self.action_button(
                            "draw-clear",
                            "icons/eraser.svg",
                            false,
                            Control::DrawClear,
                        )
                        .tooltip(move |_, cx| {
                            ui::Tooltip::new(&theme, t!("Clear")).keys(["Del"]).view(cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.press(Control::DrawClear, window, cx);
                        })),
                    ),
            )
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
            let session = self.session.read(cx);
            let mode = session.mode();
            let target = session.capture_target();
            cx.spawn(async move |this, cx| {
                let (microphones, cameras, header) = cx
                    .background_executor()
                    .spawn(async move {
                        let header = mode.map(|mode| {
                            let settings = crate::store::GeneralSettings::load();
                            let size = target.as_ref().and_then(|target| {
                                let size = target.physical_size()?;
                                let capture =
                                    (size.width().round() as u32, size.height().round() as u32);
                                Some(match mode {
                                    RecordingMode::Studio => capture,
                                    RecordingMode::Instant => {
                                        frameever_recording::instant_recording::output_size(
                                            capture,
                                            Some(settings.instant_mode_max_resolution),
                                        )
                                    }
                                })
                            });
                            format_options_header(
                                mode,
                                target.as_ref().and_then(source_label),
                                size,
                                settings.max_fps,
                            )
                        });
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
                        (microphones, cameras, header)
                    })
                    .await;
                this.update(cx, |this, cx| {
                    this.microphones = microphones;
                    this.cameras = cameras;
                    this.options_header = header.map(Into::into);
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

    fn ring<E: Styled>(&self, element: E, control: Control) -> E {
        element
            .border_1()
            .border_color(if self.ringed == Some(control) {
                Hsla::from(self.theme.ring)
            } else {
                transparent_black()
            })
    }

    fn ringed_button(&self, button: ui::Button, control: Control) -> impl IntoElement {
        self.ring(div().rounded_full(), control).child(button)
    }

    fn action_button(
        &self,
        id: &'static str,
        icon: &'static str,
        disabled: bool,
        control: Control,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        self.ring(
            div()
                .id(id)
                .w(px(28.))
                .h(px(28.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.)),
            control,
        )
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
        let countdown = self.session.read(cx).countdown_remaining();
        let can_stop = self.can_stop(cx);

        self.ring(
            div()
                .id("stop")
                .size(px(28.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full(),
            Control::Stop,
        )
        .bg(if countdown.is_some() {
            theme.secondary
        } else {
            theme.primary
        })
        .when(can_stop, |this| {
            this.hover(|style| style.bg(theme.primary_hover))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.press(Control::Stop, window, cx);
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
            session.auto_stop_countdown(),
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
        let muted = self.session.read(cx).mic_muted;
        let can_mute = self.can_mute(cx);
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

        self.ring(
            div()
                .id("microphone")
                .w(px(28.))
                .h(px(28.))
                .flex_shrink_0()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.)),
            Control::Microphone,
        )
        .when(can_mute, |this| {
            this.rounded(px(8.))
                .hover(|style| style.bg(Theme::with_alpha(theme.gray_12, 0.06)))
                .active(|style| style.bg(Theme::with_alpha(theme.gray_12, 0.10)))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.press(Control::Microphone, window, cx);
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
        control: Control,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        self.ring(div().id(id), control)
            .w_full()
            .h(px(crate::theme::ROW_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .rounded(px(12.))
            .text_size(px(13.))
            .text_color(if enabled {
                theme.foreground
            } else {
                theme.faint_foreground
            })
            .when(enabled, |this| this.hover(|style| style.bg(theme.accent)))
    }

    fn quiet_tint(&self, available: bool) -> Hsla {
        if available {
            self.theme.muted_foreground.into()
        } else {
            self.theme.faint_foreground.into()
        }
    }

    fn row_icon(&self, path: &'static str, available: bool) -> gpui::Svg {
        svg()
            .path(path)
            .size(px(16.))
            .text_color(self.quiet_tint(available))
    }

    fn device_row(
        &self,
        kind: InputKind,
        value: SharedString,
        in_take: bool,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let open = self.expanded == Some(kind);
        let (id, control, icon, label) = match kind {
            InputKind::Microphone => (
                "pick-microphone",
                Control::PickMicrophone,
                "icons/mic.svg",
                t!("Microphone"),
            ),
            InputKind::Camera => (
                "pick-camera",
                Control::PickCamera,
                "icons/webcam.svg",
                t!("Camera"),
            ),
        };
        self.option_row(id, enabled, control)
            .when(in_take, |this| this.text_color(self.theme.foreground))
            .when(enabled, |this| {
                this.on_click(cx.listener(move |this, _, window, cx| {
                    this.press(control, window, cx);
                }))
            })
            .child(self.row_icon(icon, in_take))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(
                div()
                    .max_w(px(130.))
                    .truncate()
                    .text_color(self.quiet_tint(in_take))
                    .child(value),
            )
            .when(enabled, |this| {
                this.child(self.row_icon(
                    if open {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    true,
                ))
            })
    }

    fn choice_row(
        &self,
        id: (&'static str, usize),
        name: String,
        checked: bool,
        enabled: bool,
        control: Control,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        self.option_row(id, enabled, control)
            .pl(px(34.))
            .child(div().flex_1().min_w_0().truncate().child(name))
            .when(checked, |this| {
                this.child(self.row_icon("icons/check.svg", enabled))
            })
            .when(enabled && !checked, |this| {
                this.on_click(cx.listener(move |this, _, window, cx| {
                    this.press(control, window, cx);
                }))
            })
    }

    fn render_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let session = self.session.read(cx);
        let can_change_microphone = session.can_change_input(InputKind::Microphone);
        let can_change_camera = session.can_change_input(InputKind::Camera);
        let mode = session.mode();
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
        let divider = || div().h(px(1.)).my(px(4.)).flex_shrink_0().bg(theme.border);

        let microphone_rows = (self.expanded == Some(InputKind::Microphone)).then(|| {
            self.microphones
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, name)| {
                    let checked = microphone.as_deref() == Some(name.as_str());
                    self.choice_row(
                        ("microphone-choice", index),
                        name,
                        checked,
                        can_change_microphone,
                        Control::MicrophoneChoice(index),
                        cx,
                    )
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
                    self.choice_row(
                        ("camera-choice", index),
                        name,
                        checked,
                        can_change_camera,
                        Control::CameraChoice(index),
                        cx,
                    )
                })
                .collect::<Vec<_>>()
        });

        div()
            .id("recording-options")
            .w(px(OPTIONS_WIDTH))
            .max_w_full()
            .max_h(px(crate::app_windows::CONTROLS_OPTIONS_HEIGHT - 8.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .mb(px(8.))
            .p(px(4.))
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .when_some(
                mode.zip(self.options_header.clone()),
                |this, (mode, header)| {
                    this.child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(10.))
                            .py(px(8.))
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(self.row_icon(
                                match mode {
                                    RecordingMode::Studio => "icons/clapperboard.svg",
                                    RecordingMode::Instant => "icons/zap.svg",
                                },
                                true,
                            ))
                            .child(div().flex_1().min_w_0().truncate().child(header)),
                    )
                    .child(divider())
                },
            )
            .child(
                self.device_row(
                    InputKind::Microphone,
                    microphone
                        .clone()
                        .map_or_else(|| not_used.clone(), Into::into),
                    microphone.is_some(),
                    can_change_microphone,
                    cx,
                ),
            )
            .children(microphone_rows.into_iter().flatten())
            .child(self.device_row(
                InputKind::Camera,
                camera.as_ref().map_or_else(
                    || not_used.clone(),
                    |_| camera_label.unwrap_or_default().into(),
                ),
                camera.is_some(),
                can_change_camera,
                cx,
            ))
            .children(camera_rows.into_iter().flatten())
            .child(
                self.option_row("camera-preview", camera.is_some(), Control::CameraPreview)
                    .child(self.row_icon("icons/maximize.svg", camera.is_some()))
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
                        camera_window_open && camera.is_some(),
                    ))
                    .when(camera.is_some(), |this| {
                        this.on_click(cx.listener(|this, _, window, cx| {
                            this.press(Control::CameraPreview, window, cx);
                        }))
                    }),
            )
            .child(divider())
            .child(
                self.option_row("hide-controls", true, Control::HideControls)
                    .child(self.row_icon("icons/eye-off.svg", true))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(t!("Hide controls")),
                    )
                    .children(self.hide_shortcut_keys(cx))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.press(Control::HideControls, window, cx);
                    })),
            )
    }

    fn hide_shortcut_keys(&self, cx: &gpui::App) -> Option<gpui::Div> {
        let action = crate::hotkeys::HotkeyAction::ToggleRecordingControls;
        crate::hotkeys::accelerator(action, cx)?;
        let binding = crate::store::hotkeys_raw()
            .get(action.store_key())
            .and_then(crate::store::hotkey_from_value)?;
        let theme = self.theme;
        Some(
            div().flex().gap(px(2.)).children(
                crate::settings_pages::hotkey_display_keys(&binding)
                    .iter()
                    .map(|key| {
                        ui::KbdChip::row(&theme, crate::settings_pages::hotkey_key_text(key))
                    }),
            ),
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
                        self.ringed_button(
                            ui::Button::plain(
                                &theme,
                                "cancel-recording-action",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Sm,
                            )
                            .label(t!("Cancel"))
                            .on_click(cx.listener(
                                |this, _, window, cx| this.press(Control::CancelAction, window, cx),
                            )),
                            Control::CancelAction,
                        ),
                    )
                    .child(
                        self.ringed_button(
                            ui::Button::plain(
                                &theme,
                                "accept-recording-action",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Sm,
                            )
                            .text_color(theme.danger_text.into())
                            .label(accept)
                            .on_click(cx.listener(
                                |this, _, window, cx| this.press(Control::AcceptAction, window, cx),
                            )),
                            Control::AcceptAction,
                        ),
                    ),
            )
    }

    pub(crate) fn tray_hidden(&self) -> bool {
        self.tray_hidden
    }

    pub(crate) fn set_tray_hidden(
        &mut self,
        hidden: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if hidden {
            self.give_back_focus(cx);
        }
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
        self.set_tray_hidden(false, window, cx);
        self.confirm_action(DestructiveAction::Restart, window, cx);
    }

    pub(crate) fn take_keyboard_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tray_hidden {
            self.set_tray_hidden(false, window, cx);
        }
        if self.focused.is_none() {
            self.return_to = crate::platform::foreground_window();
        }
        window.activate_window();
        window.focus(&self.focus, cx);
        self.focused = keyboard::resolve(&self.groups(cx), self.focused, None);
        cx.notify();
    }

    fn give_back_focus(&mut self, cx: &mut Context<Self>) {
        self.focused = None;
        self.opener = None;
        if let Some(previous) = self.return_to.take() {
            cx.spawn(async move |_, _| crate::platform::restore_foreground_window(previous))
                .detach();
        }
        cx.notify();
    }

    fn busy(&self, cx: &gpui::App) -> bool {
        let session = self.session.read(cx);
        !matches!(session.phase, Phase::Recording { .. }) || session.pause_unavailable()
    }

    fn can_mute(&self, cx: &gpui::App) -> bool {
        let session = self.session.read(cx);
        session.mode() == Some(RecordingMode::Instant)
            && session.has_microphone()
            && matches!(session.phase, Phase::Recording { .. })
    }

    fn can_stop(&self, cx: &gpui::App) -> bool {
        let session = self.session.read(cx);
        !matches!(session.phase, Phase::Starting | Phase::Stopping)
            || session.countdown_remaining().is_some()
    }

    fn groups(&self, cx: &gpui::App) -> Vec<Group> {
        let mut groups = Vec::new();
        if self.options_open {
            let session = self.session.read(cx);
            let can_change_microphone = session.can_change_input(InputKind::Microphone);
            let can_change_camera = session.can_change_input(InputKind::Camera);
            let has_camera = session.camera_id().is_some();
            let mut options = vec![(Control::PickMicrophone, can_change_microphone)];
            if self.expanded == Some(InputKind::Microphone) {
                options.extend(
                    (0..self.microphones.len())
                        .map(|index| (Control::MicrophoneChoice(index), can_change_microphone)),
                );
            }
            options.push((Control::PickCamera, can_change_camera));
            if self.expanded == Some(InputKind::Camera) {
                options.extend(
                    (0..self.cameras.len())
                        .map(|index| (Control::CameraChoice(index), can_change_camera)),
                );
            }
            options.push((Control::CameraPreview, has_camera));
            options.push((Control::HideControls, true));
            groups.push(options);
        }
        if self.confirmation.is_some() {
            groups.push(vec![
                (Control::CancelAction, true),
                (Control::AcceptAction, true),
            ]);
        }
        if let Some(issue) = self.issue_card_shown(cx) {
            let details = issue.details.is_some();
            groups.push(vec![
                (Control::DismissIssue, true),
                (Control::IssueDetails, details),
                (Control::ExportLogs, details),
            ]);
        }
        if self.drawing(cx) {
            let mut strip: Group = (0..DRAW_TOOLS.len())
                .map(|index| (Control::DrawTool(index), true))
                .collect();
            strip.extend((0..INK_COLOURS).map(|index| (Control::DrawColour(index), true)));
            strip.push((Control::DrawUndo, true));
            strip.push((Control::DrawClear, true));
            groups.push(strip);
        }
        let busy = self.busy(cx);
        groups.push(vec![
            (Control::Microphone, self.can_mute(cx)),
            (Control::Pause, !busy),
            (Control::Restart, !busy),
            (Control::Issue, self.issue(cx).is_some()),
            (Control::Delete, !busy),
            (Control::Draw, !busy),
            (Control::Options, true),
            (Control::Stop, self.can_stop(cx)),
        ]);
        groups
    }

    fn press(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        let from_keyboard = self.focused.is_some();
        if from_keyboard {
            self.focused = Some(control);
        }
        match control {
            Control::Microphone => {
                if self.can_mute(cx) {
                    self.session
                        .update(cx, |session, cx| session.toggle_mic_mute(cx));
                }
            }
            Control::Pause => {
                if !self.busy(cx) {
                    self.session
                        .update(cx, |session, cx| session.toggle_pause(cx));
                }
            }
            Control::Restart => self.confirm_action(DestructiveAction::Restart, window, cx),
            Control::Delete => self.confirm_action(DestructiveAction::Delete, window, cx),
            Control::Issue | Control::DismissIssue => self.toggle_issue_card(window, cx),
            Control::Draw => self.toggle_draw(window, cx),
            Control::Options => self.toggle_options(window, cx),
            Control::Stop => {
                if self.can_stop(cx) {
                    self.session.update(cx, |session, cx| session.stop(cx));
                }
            }
            Control::DrawTool(index) => {
                if let Some(tool) = DRAW_TOOLS.get(index) {
                    self.ink_action(InkAction::Tool(*tool), cx);
                }
            }
            Control::DrawColour(index) => self.ink_action(InkAction::Colour(index), cx),
            Control::DrawUndo => self.ink_action(InkAction::Undo, cx),
            Control::DrawClear => self.ink_action(InkAction::Clear, cx),
            Control::PickMicrophone => self.toggle_expanded(InputKind::Microphone),
            Control::PickCamera => self.toggle_expanded(InputKind::Camera),
            Control::MicrophoneChoice(index) => self.choose_microphone(index, cx),
            Control::CameraChoice(index) => self.choose_camera(index, cx),
            Control::CameraPreview => self.toggle_camera_preview(cx),
            Control::HideControls => {
                self.options_open = false;
                self.expanded = None;
                self.fit_height(window, cx);
                self.set_tray_hidden(true, window, cx);
                cx.defer(crate::tray::refresh_menu);
            }
            Control::CancelAction => self.cancel_confirmation(window, cx),
            Control::AcceptAction => self.accept_confirmation(window, cx),
            Control::IssueDetails => {
                self.issue_details_open = !self.issue_details_open;
                self.fit_height(window, cx);
            }
            Control::ExportLogs => self.export_logs(window, cx),
        }
        if from_keyboard
            && self.focused.is_some()
            && matches!(
                control,
                Control::Restart | Control::Delete | Control::Options | Control::Issue
            )
        {
            self.opener = Some(control);
            let groups = self.groups(cx);
            if groups.len() > 1 {
                self.focused = keyboard::resolve(&groups[..1], None, None);
            }
        }
        cx.notify();
    }

    fn toggle_expanded(&mut self, kind: InputKind) {
        self.expanded = if self.expanded == Some(kind) {
            None
        } else {
            Some(kind)
        };
    }

    fn choose_microphone(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(name) = self.microphones.get(index).cloned() else {
            return;
        };
        let session = self.session.read(cx);
        if !session.can_change_input(InputKind::Microphone)
            || session.microphone_label().as_deref() == Some(&*name)
        {
            return;
        }
        self.expanded = None;
        self.session.update(cx, |session, cx| {
            session.change_input(crate::session::InputChoice::Microphone(name), cx)
        });
    }

    fn choose_camera(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((_, id)) = self.cameras.get(index).cloned() else {
            return;
        };
        let session = self.session.read(cx);
        if !session.can_change_input(InputKind::Camera) || session.camera_id().as_ref() == Some(&id)
        {
            return;
        }
        self.expanded = None;
        let selection = crate::deeplink::camera_selection(id);
        self.session.update(cx, |session, cx| {
            session.change_input(crate::session::InputChoice::Camera(selection), cx)
        });
    }

    fn export_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                .spawn(async move { crate::diagnostics::export_logs(&path).map(|()| path) })
                .await;
            match result {
                Ok(path) => {
                    cx.update(|_, cx| crate::library::reveal_path(&path, cx))
                        .ok();
                }
                Err(error) => {
                    tracing::error!(error, "could not export recording logs");
                    crate::platform::alert_dialog(
                        t!("Couldn't export logs"),
                        t!("Choose a writable folder and try again."),
                    );
                }
            }
        })
        .detach();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        if self.handle_key(&event.keystroke.key, modifiers.shift, window, cx) {
            cx.stop_propagation();
        }
    }

    fn handle_key(
        &mut self,
        key: &str,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(command) = keyboard::command(key, shift) else {
            return false;
        };
        let groups = self.groups(cx);
        let Some(current) = keyboard::resolve(&groups, self.focused, self.opener) else {
            return false;
        };
        self.focused = Some(current);
        match command {
            Command::Press => self.press(current, window, cx),
            Command::Escape => self.escape(window, cx),
            _ => self.focused = Some(keyboard::moved(&groups, current, command)),
        }
        cx.notify();
        true
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let back_to = if self.confirmation.is_some() {
            self.cancel_confirmation(window, cx);
            self.opener.unwrap_or(Control::Pause)
        } else if self.options_open {
            self.toggle_options(window, cx);
            Control::Options
        } else if self.issue_card_shown(cx).is_some() {
            self.toggle_issue_card(window, cx);
            Control::Issue
        } else if self.drawing(cx) {
            self.toggle_draw(window, cx);
            Control::Draw
        } else {
            self.give_back_focus(cx);
            return;
        };
        self.opener = None;
        self.focused = keyboard::resolve(&self.groups(cx), Some(back_to), None);
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
                        self.action_button(
                            "dismiss-issue",
                            "icons/x.svg",
                            false,
                            Control::DismissIssue,
                        )
                        .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Dismiss")).view(cx))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.press(Control::DismissIssue, window, cx)
                        })),
                    ),
            )
            .when_some(issue.details, |this, details| {
                this.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            self.ringed_button(
                                ui::Button::plain(
                                    &theme,
                                    "recording-details",
                                    ui::ButtonVariant::Gray,
                                    ui::ButtonSize::Sm,
                                )
                                .label(t!("Details"))
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.press(Control::IssueDetails, window, cx)
                                    },
                                )),
                                Control::IssueDetails,
                            ),
                        )
                        .child(
                            self.ringed_button(
                                ui::Button::plain(
                                    &theme,
                                    "recording-export-logs",
                                    ui::ButtonVariant::Gray,
                                    ui::ButtonSize::Sm,
                                )
                                .label(t!("Export logs"))
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.press(Control::ExportLogs, window, cx)
                                    },
                                )),
                                Control::ExportLogs,
                            ),
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
        let busy = self.busy(cx);
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
                    Control::Pause,
                )
                .when(pause_pending, |this| this.opacity(0.5))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, pause_tooltip).view(cx))
                .when(!busy, |this| {
                    this.on_click(cx.listener(|this, _, window, cx| {
                        this.press(Control::Pause, window, cx);
                    }))
                }),
            )
            .child(
                self.action_button("restart", "icons/rotate-ccw.svg", busy, Control::Restart)
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Restart")).view(cx))
                    .when(!busy, |this| {
                        this.on_click(cx.listener(|this, _, window, cx| {
                            this.press(Control::Restart, window, cx);
                        }))
                    }),
            )
            .when(self.issue(cx).is_some(), |this| {
                this.child(
                    self.action_button("issue", "icons/triangle-alert.svg", false, Control::Issue)
                        .tooltip(move |_, cx| {
                            ui::Tooltip::new(&theme, t!("Recording issue")).view(cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.press(Control::Issue, window, cx)
                        })),
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
                self.action_button("delete", "icons/trash.svg", busy, Control::Delete)
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Delete")).view(cx))
                    .when(!busy, |this| {
                        this.on_click(cx.listener(|this, _, window, cx| {
                            this.press(Control::Delete, window, cx);
                        }))
                    }),
            )
            .child(
                self.action_button("draw", "icons/pencil.svg", busy, Control::Draw)
                    .when(self.drawing(cx), |this| this.bg(theme.selection))
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Draw on screen")).view(cx))
                    .when(!busy, |this| {
                        this.on_click(
                            cx.listener(|this, _, window, cx| {
                                this.press(Control::Draw, window, cx)
                            }),
                        )
                    }),
            )
            .child(
                self.action_button(
                    "settings",
                    "icons/sliders-horizontal.svg",
                    false,
                    Control::Options,
                )
                .when(self.options_open, |this| this.bg(theme.selection))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Recording options")).view(cx))
                .on_click(
                    cx.listener(|this, _, window, cx| this.press(Control::Options, window, cx)),
                ),
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
        if self.focused.is_some() {
            self.focused = keyboard::resolve(&self.groups(cx), self.focused, self.opener);
        }
        self.ringed = self
            .focused
            .filter(|_| window.is_window_active() && self.focus.is_focused(window));
        let key_down = cx.listener(Self::key_down);
        let issue = self.issue_card_shown(cx);
        let options = self.options_open.then(|| self.render_options(cx));

        div()
            .track_focus(&self.focus)
            .on_key_down(key_down)
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .p(px(12.))
            .font_family(crate::theme::ui_font())
            // `body { font-weight: 500 }` (`ui-solid/src/main.css:189-192`).
            .font_weight(FontWeight::MEDIUM)
            .when_some(options, |this, options| {
                this.child(div().flex().justify_center().child(options))
            })
            .when_some(self.confirmation, |this, action| {
                this.child(self.render_confirmation(action, cx))
            })
            .when_some(issue, |this, issue| {
                this.child(self.render_issue(issue, cx))
            })
            .when(self.drawing(cx), |this| {
                this.child(self.render_draw_strip(cx))
            })
            .child(self.render_bar(cx))
    }
}

fn recording_time_label(
    phase: Phase,
    countdown: Option<u32>,
    pending: bool,
    elapsed: Duration,
    auto_stop_left: Option<Duration>,
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
    } else if let Some(left) = auto_stop_left {
        let shown = Duration::from_secs(left.as_secs() + u64::from(left.subsec_nanos() > 0));
        t!(
            "Stops in {time}",
            time = ControlsWindow::format_elapsed(shown)
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

    #[gpui::test]
    fn controls_draw_opens_the_tool_strip_and_closes_other_cards(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            Feeds::init(cx);
            RecordingSession::init(cx);
            ScreenInk::init(cx);
        });
        let (controls, cx) = cx.add_window_view(|window, cx| {
            let session = RecordingSession::global(cx);
            session.update(cx, |session, _| {
                session.phase = Phase::Recording { paused: false }
            });
            ControlsWindow::new(session, false, window, cx)
        });
        controls.update_in(cx, |controls, window, cx| {
            controls.confirm_action(DestructiveAction::Delete, window, cx);
            controls.toggle_draw(window, cx);
            assert_eq!(controls.confirmation, None);
        });
        cx.run_until_parked();
        controls.update_in(cx, |controls, _, cx| {
            assert!(controls.drawing(cx));
            assert_eq!(
                controls.window_height,
                crate::app_windows::CONTROLS_BAR_HEIGHT + DRAW_STRIP_ROOM
            );
            controls.ink_action(InkAction::Tool(InkTool::Arrow), cx);
            controls.ink_action(InkAction::Colour(2), cx);
            let ink = controls.ink.clone().unwrap();
            assert_eq!(ink.read(cx).ink().tool(), InkTool::Arrow);
            assert_eq!(ink.read(cx).ink().colour(), 2);
            controls.ink_action(InkAction::Leave, cx);
        });
        cx.run_until_parked();
        controls.update_in(cx, |controls, _, cx| {
            assert!(!controls.drawing(cx));
            assert_eq!(
                controls.window_height,
                crate::app_windows::CONTROLS_BAR_HEIGHT
            );
        });
    }

    #[gpui::test]
    fn the_focus_shortcut_walks_the_bar_and_esc_closes_a_card_before_leaving(
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
            assert_eq!(controls.focused, None);
            controls.take_keyboard_focus(window, cx);
            assert_eq!(controls.focused, Some(Control::Pause));
            assert!(controls.handle_key("right", false, window, cx));
            assert_eq!(controls.focused, Some(Control::Restart));
            controls.handle_key("end", false, window, cx);
            assert_eq!(controls.focused, Some(Control::Stop));
            controls.handle_key("home", false, window, cx);
            assert_eq!(controls.focused, Some(Control::Pause));

            controls.focused = Some(Control::Delete);
            controls.handle_key("enter", false, window, cx);
            assert_eq!(controls.confirmation, Some(DestructiveAction::Delete));
            assert_eq!(controls.focused, Some(Control::CancelAction));
            controls.handle_key("tab", false, window, cx);
            assert_eq!(controls.focused, Some(Control::Pause));
            controls.handle_key("tab", true, window, cx);
            assert_eq!(controls.focused, Some(Control::CancelAction));

            controls.handle_key("escape", false, window, cx);
            assert_eq!(controls.confirmation, None);
            assert_eq!(controls.focused, Some(Control::Delete));
            assert_eq!(
                controls.session.read(cx).phase,
                Phase::Recording { paused: false }
            );
            controls.handle_key("escape", false, window, cx);
            assert_eq!(controls.focused, None);
            assert!(!controls.handle_key("a", false, window, cx));
        });
    }

    #[gpui::test]
    fn hide_controls_sits_in_the_options_and_closes_them(cx: &mut gpui::TestAppContext) {
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
            controls.take_keyboard_focus(window, cx);
            controls.focused = Some(Control::Options);
            controls.handle_key("space", false, window, cx);
            assert!(controls.options_open);
            assert_eq!(controls.focused, Some(Control::HideControls));
            assert_eq!(
                controls.groups(cx)[0].last(),
                Some(&(Control::HideControls, true))
            );
            controls.handle_key("enter", false, window, cx);
            assert!(!controls.options_open);
            assert_eq!(controls.focused, None);
        });
    }

    #[gpui::test]
    fn controls_draw_is_unavailable_until_the_recording_runs(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            Feeds::init(cx);
            RecordingSession::init(cx);
            ScreenInk::init(cx);
        });
        let (controls, cx) = cx.add_window_view(|window, cx| {
            let session = RecordingSession::global(cx);
            session.update(cx, |session, _| session.phase = Phase::Starting);
            ControlsWindow::new(session, false, window, cx)
        });
        controls.update_in(cx, |controls, window, cx| controls.toggle_draw(window, cx));
        cx.run_until_parked();
        controls.update_in(cx, |controls, _, cx| assert!(!controls.drawing(cx)));
    }

    #[test]
    fn paused_recordings_keep_the_elapsed_time_and_pending_states_stay_truthful() {
        let elapsed = Duration::from_secs(3753);
        assert_eq!(
            recording_time_label(
                Phase::Recording { paused: true },
                None,
                false,
                elapsed,
                None
            ),
            "Paused 1:02:33"
        );
        assert_eq!(
            recording_time_label(
                Phase::Recording { paused: false },
                None,
                true,
                elapsed,
                None
            ),
            "Pausing…"
        );
        assert_eq!(
            recording_time_label(Phase::Recording { paused: true }, None, true, elapsed, None),
            "Resuming…"
        );
        assert_eq!(
            recording_time_label(Phase::Starting, Some(3), false, elapsed, None),
            "Starts in 3"
        );
        assert_eq!(
            recording_time_label(Phase::Stopping, None, false, elapsed, None),
            "Saving…"
        );
    }

    #[test]
    fn the_last_minute_before_auto_stop_shows_the_time_left() {
        let elapsed = Duration::from_millis(290_500);
        let left = Some(Duration::from_millis(9_500));
        let live = Phase::Recording { paused: false };
        assert_eq!(
            recording_time_label(live, None, false, elapsed, left),
            "Stops in 0:10"
        );
        assert_eq!(
            recording_time_label(live, None, false, elapsed, Some(Duration::from_secs(60))),
            "Stops in 1:00"
        );
        assert_eq!(
            recording_time_label(live, None, false, elapsed, None),
            "4:50"
        );
        assert_eq!(
            recording_time_label(
                Phase::Recording { paused: true },
                None,
                false,
                elapsed,
                left
            ),
            "Paused 4:50"
        );
        assert_eq!(
            recording_time_label(live, None, true, elapsed, left),
            "Pausing…"
        );
        assert_eq!(
            recording_time_label(Phase::Stopping, None, false, elapsed, left),
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

#[cfg(test)]
mod options_header_tests {
    use super::*;

    #[test]
    fn the_options_header_names_mode_source_recorded_size_and_frame_rate() {
        assert_eq!(
            format_options_header(
                RecordingMode::Studio,
                Some("Display"),
                Some((2560, 1440)),
                60
            ),
            "Studio · Display · 2560 × 1440 · 60 FPS"
        );
        assert_eq!(
            format_options_header(RecordingMode::Instant, None, None, 30),
            "Instant · 30 FPS"
        );
    }
}
