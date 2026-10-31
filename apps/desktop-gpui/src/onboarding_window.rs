use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, Context, Div, FocusHandle, FontWeight, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ObjectFit, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, StyledImage, Window, div, img, prelude::FluentBuilder, px,
    relative, rgb, rgba, svg,
};

use crate::{
    i18n::{n, t, tr},
    main_window::Mode,
    onboarding_audio::OnboardingAudio,
    permissions::{self, OSPermission, OSPermissionStatus},
    permissions_ui::{PermissionsState, RowAction},
    store,
    theme::Theme,
};

#[cfg(target_os = "windows")]
use crate::ui;

mod stage;

pub use stage::Paper;

pub const ONBOARDING_WIDTH: f32 = 860.;
pub const ONBOARDING_HEIGHT: f32 = 690.;
pub const ONBOARDING_HEADER_HEIGHT: f32 = 36.;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const REQUEST_GRACE: Duration = Duration::from_secs(2);
const WELCOME_EXIT: Duration = Duration::from_millis(1100);
const WELCOME_REVEAL_DELAY: Duration = Duration::from_millis(450);
const WELCOME_ENTRANCE_SECS: f32 = 2.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
enum Step {
    Permissions,
    Overview,
    Instant,
    Studio,
    Screenshot,
    Toggle,
    Shortcuts,
    Faq,
}

fn guided_permission(permission: OSPermission) -> bool {
    permission.required() && crate::permission_guide::eligible(permission)
}

impl Step {
    const ALL: [Self; 8] = [
        Self::Permissions,
        Self::Overview,
        Self::Instant,
        Self::Studio,
        Self::Screenshot,
        Self::Toggle,
        Self::Shortcuts,
        Self::Faq,
    ];

    fn first(macos: bool, permissions_only: bool) -> Self {
        if macos || permissions_only {
            Self::Permissions
        } else {
            Self::Overview
        }
    }

    fn next(self, permissions_only: bool, core_granted: bool) -> Option<Self> {
        if permissions_only || (self == Self::Permissions && !core_granted) {
            return None;
        }
        Self::ALL.get(self as usize + 1).copied()
    }

    fn previous(self, first: Self) -> Option<Self> {
        if self as usize <= first as usize {
            return None;
        }
        Self::ALL.get(self as usize - 1).copied()
    }

    fn demo_timing(self) -> Option<(&'static [u64], u64)> {
        match self {
            Self::Instant => Some((&[300, 2350, 3350, 4350, 5350, 6350, 7350, 8350], 9550)),
            Self::Studio => Some((
                &[
                    300, 2350, 3350, 4350, 5350, 6350, 7350, 8150, 9150, 10150, 11150,
                ],
                12250,
            )),
            Self::Screenshot => Some((&[200, 700, 1400, 2600, 3400, 3900, 4900, 5900, 6700], 8000)),
            Self::Toggle => Some((&[2500, 5000], 7500)),
            _ => None,
        }
    }
}

struct ModeDetail {
    mode: Mode,
    title: &'static str,
    tagline: &'static str,
    description: &'static str,
    icon: &'static str,
    features: [&'static str; 4],
}

const MODES: [ModeDetail; 3] = [
    ModeDetail {
        mode: Mode::Instant,
        title: n!("Instant Mode"),
        tagline: n!("Record & save in seconds"),
        description: n!(
            "Records straight to a video file on your computer. Stop recording and it is ready to play — no editing step."
        ),
        icon: "icons/instant.svg",
        features: [
            n!("Ready-to-play video file"),
            n!("Microphone & system audio"),
            n!("Camera overlay"),
            n!("Stays on your computer"),
        ],
    },
    ModeDetail {
        mode: Mode::Studio,
        title: n!("Studio Mode"),
        tagline: n!("Professional editing tools"),
        description: n!(
            "Record in full quality locally, then use the built-in editor to add backgrounds, padding, cursor effects, and more."
        ),
        icon: "icons/film-cut.svg",
        features: [
            n!("Full quality local recording"),
            n!("Built-in editor & effects"),
            n!("Custom backgrounds & padding"),
            n!("Export when ready"),
        ],
    },
    ModeDetail {
        mode: Mode::Screenshot,
        title: n!("Screenshot Mode"),
        tagline: n!("Capture & beautify instantly"),
        description: n!(
            "Take screenshots with a single hotkey, add annotations and beautiful backgrounds, then copy or save instantly."
        ),
        icon: "icons/screenshot.svg",
        features: [
            n!("Instant hotkey capture"),
            n!("Annotation & drawing tools"),
            n!("Beautiful backgrounds"),
            n!("Copy or save"),
        ],
    },
];

const FAQ: [(&str, &str); 5] = [
    (
        n!("Is it free to use?"),
        n!("Yes. It is free and open source, and everything runs on your computer."),
    ),
    (
        n!("What's the difference between Instant and Studio?"),
        n!(
            "Instant mode records straight to a ready-to-play video file. Studio mode records each source separately in full quality, letting you edit with backgrounds, effects, and more before exporting."
        ),
    ),
    (
        n!("Where are my recordings stored?"),
        n!(
            "All recordings are stored locally on your computer and are never uploaded anywhere. You can manage storage in Settings."
        ),
    ),
    (
        n!("Can I change my shortcuts later?"),
        n!("Head to Settings → Shortcuts at any time to customize all your keyboard shortcuts."),
    ),
    (
        n!("How do I share a recording?"),
        n!(
            "Export or open the video file and share it however you like. The app never uploads anything."
        ),
    ),
];

pub struct OnboardingWindow {
    theme: Theme,
    step: Step,
    first_step: Step,
    permissions_only: bool,
    state: PermissionsState,
    pending: Option<OSPermission>,
    media_request_pending: bool,
    poll: Option<gpui::Task<()>>,
    verify: Option<gpui::Task<()>>,
    refresh: Option<gpui::Task<()>>,
    animated: bool,
    welcome: bool,
    welcome_exiting: bool,
    welcome_exit: Option<gpui::Task<()>>,
    welcome_at: Option<std::time::Instant>,
    welcome_exit_at: Option<std::time::Instant>,
    reduce_motion: bool,
    step_delay: Duration,
    audio: Option<OnboardingAudio>,
    muted: bool,
    demo_task: Option<gpui::Task<()>>,
    demo_phase: usize,
    demo_cycle: usize,
    chosen_mode: Option<usize>,
    step_revision: usize,
    transition_direction: f32,
    faq_open: [bool; FAQ.len()],
    error: Option<String>,
    focus: FocusHandle,
    stage: stage::StageState,
    step_at: std::time::Instant,
}

impl OnboardingWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.on_window_should_close(cx, |_window, cx| {
            cx.defer(crate::app_windows::onboarding_closed);
            true
        });
        let forced = crate::permissions_ui::surface_forced();
        let state = PermissionsState::initial();
        let revisit = store::has_completed_onboarding() && !state.necessary_granted();
        let permissions_only = revisit || forced;
        let first_step = Step::first(cfg!(target_os = "macos"), permissions_only);
        let welcome = !store::has_completed_startup() && !permissions_only;
        let muted = store::store_section("audioSettings")
            .get("isMuted")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        crate::theme::bind_window(window, cx);
        let granted = std::array::from_fn(|i| state.status(OSPermission::ALL[i]).permitted());
        let mut this = Self {
            theme: Theme::for_window(window, cx, true),
            step: first_step,
            first_step,
            permissions_only,
            state,
            pending: None,
            media_request_pending: false,
            poll: None,
            verify: None,
            refresh: None,
            animated: window.is_window_active(),
            welcome,
            welcome_exiting: false,
            welcome_exit: None,
            welcome_at: None,
            welcome_exit_at: None,
            reduce_motion: cx.reduce_motion(),
            step_delay: Duration::ZERO,
            audio: welcome.then(|| OnboardingAudio::new(muted, cx)),
            muted,
            demo_task: None,
            demo_phase: 0,
            demo_cycle: 0,
            chosen_mode: None,
            step_revision: 0,
            transition_direction: 1.,
            faq_open: [false; FAQ.len()],
            error: None,
            focus: cx.focus_handle(),
            stage: stage::StageState::new(granted),
            step_at: std::time::Instant::now(),
        };
        cx.observe_window_activation(window, |this: &mut Self, window, cx| {
            this.animated = window.is_window_active();
            if this.animated {
                if this.permissions_visible() {
                    this.refresh_permissions(window, cx);
                }
                this.arm_demo(window, cx);
            } else {
                this.demo_task = None;
                if !surface_visible(window) {
                    this.poll = None;
                    this.refresh = None;
                }
            }
            cx.notify();
        })
        .detach();
        #[cfg(debug_assertions)]
        if !this.welcome
            && !this.permissions_only
            && let Ok(name) = std::env::var("FRAMEEVER_GPUI_ONBOARDING_STEP")
            && let Some(step) = Step::ALL
                .into_iter()
                .find(|step| format!("{step:?}").eq_ignore_ascii_case(&name))
        {
            this.step = step;
        }
        this.arm_poll(window, cx);
        this.arm_demo(window, cx);
        this
    }

    pub fn focus_root(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
    }

    fn permissions_visible(&self) -> bool {
        !self.welcome && self.step == Step::Permissions
    }

    fn arm_poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.permissions_visible()
            || !surface_visible(window)
            || self.poll.is_some()
            || self.state.all_shown_granted()
        {
            return;
        }
        self.poll = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let visible = this
                    .update_in(cx, |this, window, _| {
                        let visible = this.permissions_visible() && surface_visible(window);
                        if !visible {
                            this.poll = None;
                        }
                        visible
                    })
                    .unwrap_or(false);
                if !visible {
                    return;
                }
                let raw = cx
                    .background_executor()
                    .spawn(async { crate::permissions_ui::sweep_raw() })
                    .await;
                let done = this
                    .update_in(cx, |this, window, cx| {
                        if !this.permissions_visible() || !surface_visible(window) {
                            return true;
                        }
                        if this.state.apply_raw(raw) {
                            this.grants_changed();
                            cx.notify();
                        }
                        let media_request_pending = permissions::media_request_pending();
                        if media_request_pending != this.media_request_pending {
                            this.media_request_pending = media_request_pending;
                            cx.notify();
                        }
                        this.state.all_shown_granted()
                    })
                    .unwrap_or(true);
                if done {
                    this.update_in(cx, |this, _, _| {
                        this.poll = None;
                        this.pending = None;
                        this.verify = None;
                    })
                    .ok();
                    return;
                }
            }
        }));
    }

    fn grants_changed(&mut self) {
        if let Some(pending) = self.pending
            && self.state.status(pending) != OSPermissionStatus::NotDetermined
        {
            self.pending = None;
            self.verify = None;
        }
    }

    fn refresh_permissions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !surface_visible(window) || self.refresh.is_some() {
            return;
        }
        self.refresh = Some(cx.spawn_in(window, async move |this, cx| {
            let raw = cx
                .background_executor()
                .spawn(async { crate::permissions_ui::sweep_raw() })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.refresh = None;
                if !this.permissions_visible() || !surface_visible(window) {
                    return;
                }
                if this.state.apply_raw(raw) {
                    this.grants_changed();
                }
                if this.state.all_shown_granted() {
                    this.pending = None;
                    this.verify = None;
                    this.poll = None;
                } else {
                    this.arm_poll(window, cx);
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        if !self.state.necessary_granted() {
            return;
        }
        if !store::set_has_completed_onboarding(true) || !store::set_has_completed_startup(true) {
            self.error = Some(t!("Could not save onboarding progress. Please try again.").into());
            cx.notify();
            return;
        }
        self.poll = None;
        self.verify = None;
        self.refresh = None;
        self.demo_task = None;
        self.audio = None;
        // A Screen Recording grant made in this session only takes effect in a
        // new process, however onboarding was left (Skip the tour included).
        if self.needs_relaunch() {
            permissions::relaunch(cx);
            return;
        }
        cx.defer(crate::app_windows::onboarding_finished);
    }

    fn continue_from_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.welcome || self.welcome_exiting {
            return;
        }
        if !store::set_has_completed_startup(true) {
            self.error = Some(t!("Could not save onboarding progress. Please try again.").into());
            cx.notify();
            return;
        }
        self.welcome_exiting = true;
        self.welcome_exit_at = Some(std::time::Instant::now());
        self.step_at = std::time::Instant::now();
        self.step_revision = self.step_revision.wrapping_add(1);
        self.step_delay = WELCOME_REVEAL_DELAY;
        self.transition_direction = 1.;
        cx.notify();
        self.welcome_exit = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(WELCOME_EXIT).await;
            this.update_in(cx, |this, window, cx| {
                this.welcome = false;
                this.welcome_exiting = false;
                this.welcome_exit_at = None;
                this.audio = None;
                this.focus_root(window, cx);
                if this.permissions_visible() {
                    this.refresh_permissions(window, cx);
                }
                this.arm_poll(window, cx);
                this.arm_demo(window, cx);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn go_to(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        if self.welcome || self.permissions_only || self.step == step {
            return;
        }
        self.transition_direction = if step as usize > self.step as usize {
            1.
        } else {
            -1.
        };
        self.step = step;
        self.step_at = std::time::Instant::now();
        self.step_revision = self.step_revision.wrapping_add(1);
        self.step_delay = Duration::ZERO;
        self.poll = None;
        self.verify = None;
        self.refresh = None;
        self.pending = None;
        self.demo_task = None;
        self.demo_phase = 0;
        self.demo_cycle = 0;
        self.chosen_mode = None;
        self.focus_root(window, cx);
        if self.permissions_visible() {
            self.refresh_permissions(window, cx);
        }
        self.arm_poll(window, cx);
        self.arm_demo(window, cx);
        cx.notify();
    }

    fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.welcome || (self.step == Step::Permissions && !self.state.necessary_granted()) {
            return;
        }
        if self.step == Step::Permissions && self.needs_relaunch() {
            permissions::relaunch(cx);
            return;
        }
        if self.permissions_only || self.step == Step::Faq {
            self.finish(cx);
        } else if let Some(next) = self
            .step
            .next(self.permissions_only, self.state.necessary_granted())
        {
            self.go_to(next, window, cx);
        }
    }

    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.permissions_only
            && let Some(previous) = self.step.previous(self.first_step)
        {
            self.go_to(previous, window, cx);
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.platform && key.key == "w" {
            cx.stop_propagation();
            cx.defer(crate::app_windows::close_onboarding);
            return;
        }
        if key.modifiers.platform || key.modifiers.control || key.modifiers.alt {
            return;
        }
        if self.welcome {
            if key.key == "space" || key.key == " " {
                cx.stop_propagation();
                self.continue_from_welcome(window, cx);
            }
            return;
        }
        match key.key.as_str() {
            "right" if !self.permissions_only && self.step != Step::Faq => self.next(window, cx),
            "left" => self.back(window, cx),
            "enter" => self.next(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn arm_demo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.welcome
            || !window.is_window_active()
            || self.demo_task.is_some()
            || self.chosen_mode.is_some()
        {
            return;
        }
        let Some((timings, cycle)) = self.step.demo_timing() else {
            return;
        };
        let step = self.step;
        self.demo_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                let mut previous = 0_u64;
                for (index, &at) in timings.iter().enumerate() {
                    cx.background_executor()
                        .timer(Duration::from_millis(at.saturating_sub(previous)))
                        .await;
                    previous = at;
                    let current = this
                        .update_in(cx, |this, window, cx| {
                            if this.welcome || this.step != step || !window.is_window_active() {
                                this.demo_task = None;
                                return false;
                            }
                            this.demo_phase = index + 1;
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !current {
                        return;
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_millis(cycle.saturating_sub(previous)))
                    .await;
                let current = this
                    .update_in(cx, |this, window, cx| {
                        if this.welcome || this.step != step || !window.is_window_active() {
                            this.demo_task = None;
                            return false;
                        }
                        this.demo_phase = 0;
                        this.demo_cycle = this.demo_cycle.wrapping_add(1);
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !current {
                    return;
                }
            }
        }));
    }

    fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.muted = !self.muted;
        if let Some(audio) = &self.audio {
            audio.set_muted(self.muted);
        }
        if !store::set_store_setting(
            "audioSettings",
            "isMuted",
            serde_json::Value::Bool(self.muted),
        ) {
            self.error = Some(t!("Could not save the sound preference.").into());
        }
        cx.notify();
    }

    fn act(&mut self, permission: OSPermission, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() || !self.permissions_visible() {
            return;
        }
        self.pending = Some(permission);
        self.stage.waiting_on = Some(permission);
        self.verify = Some(cx.spawn_in(window, async move |this, cx| {
            let raw = cx
                .background_executor()
                .spawn(async { crate::permissions_ui::sweep_raw() })
                .await;
            let should_verify = this
                .update_in(cx, |this, window, cx| {
                    if !this.permissions_visible() {
                        this.pending = None;
                        return false;
                    }
                    let action = this.state.refreshed_action(permission, raw);
                    match action {
                        None => {
                            this.pending = None;
                            cx.notify();
                            false
                        }
                        Some(RowAction::OpenSettings) if guided_permission(permission) => {
                            this.pending = None;
                            this.state.note_settings_opened(permission);
                            permissions::open_permission_settings(permission);
                            cx.defer(move |cx| crate::permission_guide::start(permission, cx));
                            this.arm_poll(window, cx);
                            cx.notify();
                            false
                        }
                        Some(RowAction::OpenSettings) => {
                            this.pending = None;
                            this.state.note_settings_opened(permission);
                            permissions::open_permission_settings(permission);
                            this.arm_poll(window, cx);
                            cx.notify();
                            false
                        }
                        Some(RowAction::Request) => {
                            permissions::request_permission(permission);
                            this.arm_poll(window, cx);
                            cx.notify();
                            true
                        }
                    }
                })
                .unwrap_or(false);
            if !should_verify {
                return;
            }
            cx.background_executor().timer(REQUEST_GRACE).await;
            let raw = cx
                .background_executor()
                .spawn(async { crate::permissions_ui::sweep_raw() })
                .await;
            this.update_in(cx, |this, _, cx| {
                if !this.permissions_visible() {
                    return;
                }
                this.state.apply_raw(raw);
                this.pending = None;
                this.media_request_pending = permissions::media_request_pending();
                if permission.required() && !this.state.status(permission).permitted() {
                    this.state.note_request_failed(permission);
                    // The request has added Cap to the pane's list (the way
                    // the Tauri app always asked first); if the alert did not
                    // end in a grant, the pane opens with the drag guide.
                    if guided_permission(permission) {
                        this.state.note_settings_opened(permission);
                        permissions::open_permission_settings(permission);
                        cx.defer(move |cx| crate::permission_guide::start(permission, cx));
                    }
                }
                if this.state.all_shown_granted() {
                    this.poll = None;
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn success_palette(&self) -> (Hsla, Hsla, Hsla) {
        (
            self.theme.success_tint.into(),
            self.theme.success.into(),
            self.theme.success_text.into(),
        )
    }
}

fn mode_doodle(mode: Mode) -> &'static crate::doodle::Doodle {
    match mode {
        Mode::Instant => &crate::onboarding_doodles::UPLOAD,
        Mode::Studio => &crate::onboarding_doodles::TRACKS,
        Mode::Screenshot => &crate::onboarding_doodles::SNAP,
    }
}

fn surface_visible(window: &Window) -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::platform::window_is_visible(window)
    }
    #[cfg(not(target_os = "macos"))]
    {
        window.is_window_active()
    }
}

fn copy(text: impl Into<SharedString>, size: f32, color: impl Into<Hsla>) -> Div {
    div()
        .text_size(px(size))
        .text_color(color.into())
        .child(text.into())
}

fn glyph(path: &'static str, size: f32, color: impl Into<Hsla>) -> gpui::Svg {
    svg().path(path).size(px(size)).text_color(color.into())
}

fn ease(value: f32) -> f32 {
    value * value * (3. - 2. * value)
}

fn ease_out(value: f32) -> f32 {
    1. - (1. - value).powi(3)
}

fn beat(elapsed: f32, delay: f32, duration: f32) -> f32 {
    ((elapsed - delay) / duration).clamp(0., 1.)
}

#[derive(Clone, Copy)]
struct WelcomeMotion {
    t: f32,
    exit: f32,
    /// 0 under Reduce Motion: fades only.
    travel: f32,
}

impl OnboardingWindow {
    fn card(&self) -> Div {
        div()
            .rounded(px(12.))
            .border_1()
            .border_color(self.theme.body_border(4))
            .bg(if self.theme.is_dark() {
                self.theme.body_fill(2)
            } else {
                gpui::white()
            })
    }

    fn paper_card(&self) -> Div {
        let paper = stage::Paper::new(&self.theme);
        div()
            .rounded(px(20.))
            .border(px(1.5))
            .border_color(paper.track)
            .bg(paper.card)
    }

    fn heading(&self, title: &'static str, subtitle: &'static str) -> Div {
        let paper = stage::Paper::new(&self.theme);
        div()
            .flex()
            .flex_col()
            .items_center()
            .max_w(px(500.))
            .text_center()
            .child(
                copy(title, 28., paper.ink)
                    .font_weight(FontWeight::MEDIUM)
                    .line_height(px(34.)),
            )
            .child(div().mt(px(4.)).child(crate::doodle::doodle(
                &crate::onboarding_doodles::UNDERLINE,
                paper.palette(),
                self.step_at,
                !self.animated,
                132.,
                12.,
            )))
            .child(
                copy(subtitle, 15., paper.soft)
                    .mt(px(10.))
                    .font_weight(FontWeight::NORMAL)
                    .line_height(px(22.)),
            )
    }

    fn tick_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        self.reduce_motion = cx.reduce_motion();
        let started = *self.welcome_at.get_or_insert(now);
        let entering = !self.reduce_motion
            && now.duration_since(started).as_secs_f32() < WELCOME_ENTRANCE_SECS;
        let running =
            self.welcome_exit_at.is_some() || entering || (self.animated && !self.reduce_motion);
        if running {
            window.request_animation_frame();
        }
    }

    fn welcome_motion(&self) -> WelcomeMotion {
        let still = self.reduce_motion;
        WelcomeMotion {
            t: match self.welcome_at {
                Some(at) if !still => at.elapsed().as_secs_f32(),
                _ => WELCOME_ENTRANCE_SECS,
            },
            exit: self.welcome_exit_at.map_or(0., |at| {
                (at.elapsed().as_secs_f32() / WELCOME_EXIT.as_secs_f32()).clamp(0., 1.)
            }),
            travel: if still { 0. } else { 1. },
        }
    }

    fn render_welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = self.welcome_motion();
        let travel = m.travel;
        let paper = stage::Paper::new(&self.theme);
        let started = self.welcome_at.unwrap_or_else(std::time::Instant::now);

        let rise = |delay: f32, exit_delay: f32| {
            let arrive = ease_out(beat(m.t, delay, 1.1));
            let leave = ease(beat(m.exit, exit_delay, 0.4));
            let offset = (12. * (1. - arrive) - 24. * leave) * travel;
            (offset, ease(beat(m.t, delay, 1.)) * (1. - leave))
        };
        let (logo_y, logo_opacity) = rise(0.15, 0.12);
        let (art_y, art_opacity) = rise(0.35, 0.09);
        let (title_y, title_opacity) = rise(0.9, 0.06);
        let (body_y, body_opacity) = rise(1.15, 0.03);
        let (button_y, button_opacity) = rise(1.45, 0.);
        let lifted =
            |element: Div, y: f32, opacity: f32| element.relative().top(px(y)).opacity(opacity);

        let logo = crate::logo::logo(&self.theme, 24.);

        let art = crate::doodle::doodle(
            &crate::onboarding_doodles::SCREEN,
            paper.palette(),
            started + Duration::from_millis(350),
            !self.animated || travel == 0.,
            240.,
            208.,
        );

        let title = div()
            .flex()
            .flex_col()
            .items_center()
            .child(
                copy(t!("Welcome to FrameEver"), 32., paper.ink)
                    .font_weight(FontWeight::SEMIBOLD)
                    .line_height(px(40.)),
            )
            .child(div().mt(px(4.)).child(crate::doodle::doodle(
                &crate::onboarding_doodles::UNDERLINE,
                paper.palette(),
                started + Duration::from_millis(1300),
                !self.animated || travel == 0.,
                150.,
                14.,
            )));

        let body = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .max_w(px(460.))
            .text_center()
            .child(
                copy(t!("A moment, kept forever."), 17., paper.ink)
                    .font_weight(FontWeight::MEDIUM)
                    .line_height(px(24.)),
            )
            .child(
                copy(
                    t!("Record your screen, polish it in the editor, and keep every file on this computer."),
                    14.,
                    paper.soft,
                )
                .font_weight(FontWeight::NORMAL)
                .line_height(px(21.)),
            );

        let accent = paper.accent;
        let button = div()
            .id("onboarding-get-started")
            .tab_index(0)
            .cursor_pointer()
            .h(px(46.))
            .px(px(30.))
            .rounded(px(12.))
            .bg(accent)
            .hover(move |el| el.bg(accent.opacity(0.88)))
            .active(|el| el.opacity(0.92))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(|this, _, window, cx| this.continue_from_welcome(window, cx)))
            .child(
                copy(t!("Get started"), 15., gpui::white())
                    .font_weight(FontWeight::SEMIBOLD)
                    .line_height(px(20.)),
            );
        let hint = div()
            .flex()
            .items_center()
            .gap(px(5.))
            .mt(px(12.))
            .child(copy(t!("or press"), 12., paper.soft).font_weight(FontWeight::NORMAL))
            .child(
                copy(t!("Space"), 11., paper.soft)
                    .font_weight(FontWeight::MEDIUM)
                    .px(px(5.))
                    .py(px(1.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(paper.track),
            );

        let content = div()
            .relative()
            .size_full()
            .pt(px(ONBOARDING_HEADER_HEIGHT))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px(px(24.))
            .child(lifted(div().child(logo), logo_y, logo_opacity))
            .child(lifted(div().mt(px(26.)).child(art), art_y, art_opacity))
            .child(lifted(
                div().mt(px(18.)).child(title),
                title_y,
                title_opacity,
            ))
            .child(lifted(div().mt(px(14.)).child(body), body_y, body_opacity))
            .child(lifted(
                div()
                    .mt(px(34.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(button)
                    .child(hint),
                button_y,
                button_opacity,
            ))
            .when_some(self.error.clone(), |el, error| {
                el.child(copy(error, 12., paper.red).mt(px(12.)))
            });
        let mute = div()
            .id("onboarding-mute")
            .tab_index(0)
            .absolute()
            .top(px(ONBOARDING_HEADER_HEIGHT + 12.))
            .p(px(4.))
            .when(cfg!(target_os = "macos"), |el| el.right(px(16.)))
            .when(!cfg!(target_os = "macos"), |el| el.left(px(16.)))
            .cursor_pointer()
            .hover(|el| el.opacity(0.8))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_mute(cx)))
            .child(glyph(
                if self.muted {
                    "onboarding/volume-x.svg"
                } else {
                    "onboarding/volume-2.svg"
                },
                22.,
                paper.soft,
            ));
        let dissolve = ease(beat(m.exit, 0.45, 0.55));
        div()
            .id("onboarding-welcome")
            .absolute()
            .inset_0()
            .overflow_hidden()
            .bg(paper.bg)
            .opacity(1. - dissolve)
            .child(content)
            .when(!self.welcome_exiting, |el| el.child(mute))
            .into_any_element()
    }

    fn render_overview(&self) -> AnyElement {
        let theme = self.theme;
        div().flex().flex_col().items_center().justify_center().min_h_full().px(px(40.)).gap(px(32.))
            .child(self.heading(t!("One app, every workflow"), t!("Whether you need speed, studio quality, or a quick screenshot — FrameEver has a mode for it.")))
            .child(div().flex().gap(px(16.)).w_full().max_w(px(540.)).children(MODES.iter().enumerate().map(|(index, mode)| {
                let paper = stage::Paper::new(&theme);
                let card = self.paper_card().flex_1().min_w_0().flex().flex_col().items_center().gap(px(10.)).px(px(20.)).pt(px(22.)).pb(px(24.))
                    .child(crate::doodle::doodle(mode_doodle(mode.mode), paper.palette(), self.step_at + Duration::from_millis(index as u64 * 180), !self.animated, 96., 84.))
                    .child(copy(tr(mode.title), 15., paper.ink).font_weight(FontWeight::MEDIUM).text_center().mt(px(4.)))
                    .child(copy(tr(mode.tagline), 12., paper.soft).font_weight(FontWeight::NORMAL).line_height(px(16.)).text_center());
                if !self.animated { return card.into_any_element(); }
                card.with_animation(("overview-card", index), Animation::new(Duration::from_millis(700 + index as u64 * 100)),
                        |el, t| el.relative().top(px(16. * (1. - ease(t)))).opacity(ease(t))).into_any_element()
            }))).into_any_element()
    }

    fn render_detail(&self, index: usize) -> AnyElement {
        let paper = stage::Paper::new(&self.theme);
        let mode = &MODES[index];
        div()
            .flex()
            .items_center()
            .min_h_full()
            .px(px(40.))
            .py(px(24.))
            .gap(px(32.))
            .child(
                div()
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
                            .child(crate::doodle::doodle(
                                mode_doodle(mode.mode),
                                paper.palette(),
                                self.step_at,
                                !self.animated,
                                58.,
                                50.,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        copy(tr(mode.title), 20., paper.ink)
                                            .font_weight(FontWeight::MEDIUM),
                                    )
                                    .child(
                                        copy(tr(mode.tagline), 12., paper.soft)
                                            .font_weight(FontWeight::NORMAL),
                                    ),
                            ),
                    )
                    .child(
                        copy(tr(mode.description), 13., paper.soft)
                            .font_weight(FontWeight::NORMAL)
                            .line_height(px(20.)),
                    )
                    .child(div().flex().flex_col().gap(px(10.)).children(
                        mode.features.iter().enumerate().map(|(i, feature)| {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(crate::doodle::doodle(
                                    &crate::onboarding_doodles::TICK,
                                    paper.palette(),
                                    self.step_at + Duration::from_millis(400 + i as u64 * 140),
                                    !self.animated,
                                    18.,
                                    18.,
                                ))
                                .child(
                                    copy(tr(feature), 13., paper.ink)
                                        .font_weight(FontWeight::NORMAL),
                                )
                        }),
                    )),
            )
            .child(
                self.paper_card()
                    .flex_1()
                    .min_w_0()
                    .h(px(if index == 1 { 300. } else { 288. }))
                    .rounded(px(16.))
                    .p(px(16.))
                    .child(self.render_demo(index)),
            )
            .into_any_element()
    }

    fn choose_mode(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(mode) = MODES.get(index) else {
            return;
        };
        self.chosen_mode = Some(index);
        self.demo_task = None;
        let mode = mode.mode;
        cx.defer(move |cx| crate::app_windows::set_recording_mode(mode, cx));
        cx.notify();
    }

    fn render_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let active = self.chosen_mode.unwrap_or(self.demo_phase % MODES.len());
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .min_h_full()
            .px(px(48.))
            .gap(px(32.))
            .child(self.heading(
                t!("Switch modes anytime"),
                t!("Toggle between modes with a single click from the main FrameEver window."),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(20.))
                    .child(
                        self.paper_card()
                            .rounded_full()
                            .flex()
                            .gap(px(24.))
                            .p(px(16.))
                            .children(MODES.iter().enumerate().map(|(index, mode)| {
                                div()
                                    .id(("onboarding-mode", index))
                                    .tab_index(0)
                                    .size(px(80.))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(if active == index {
                                        theme.foreground
                                    } else {
                                        theme.gray_5
                                    })
                                    .bg(if active == index {
                                        theme.body_fill(7)
                                    } else if theme.is_dark() {
                                        theme.body_fill(3)
                                    } else {
                                        gpui::white()
                                    })
                                    .cursor_pointer()
                                    .hover(|el| el.opacity(0.85))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.choose_mode(index, cx)
                                    }))
                                    .child(
                                        glyph(
                                            mode.icon,
                                            if active == index { 35. } else { 32. },
                                            theme.gray_12,
                                        )
                                        .opacity(if active == index { 1. } else { 0.5 }),
                                    )
                            })),
                    )
                    .child(div().flex().gap(px(24.)).px(px(16.)).children(
                        MODES.iter().enumerate().map(|(index, mode)| {
                            copy(
                                tr(mode.title),
                                14.,
                                if active == index {
                                    theme.gray_12
                                } else {
                                    theme.gray_9
                                },
                            )
                            .w(px(80.))
                            .text_center()
                            .font_weight(FontWeight::MEDIUM)
                            .id(("onboarding-mode-label", index))
                            .cursor_pointer()
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.choose_mode(index, cx)),
                            )
                        }),
                    )),
            )
            .into_any_element()
    }

    fn render_shortcuts(&self) -> AnyElement {
        let theme = self.theme;
        div().flex().flex_col().items_center().justify_center().min_h_full().px(px(48.)).gap(px(24.))
            .child(self.heading(t!("Make FrameEver yours"), t!("Customize everything from keyboard shortcuts to storage. FrameEver adapts to your workflow.")))
            .child(div().flex().flex_col().w_full().max_w(px(420.)).gap(px(8.)).children([
                (t!("Keyboard Shortcuts"), t!("Global hotkeys for recording, screenshots, and switching modes")),
                (t!("Recording Preferences"), t!("FPS, quality, countdown timer, cursor effects, and more")),
            ].into_iter().map(|(title, description)| {
                self.paper_card().rounded(px(16.)).px(px(18.)).py(px(13.)).flex().flex_col().gap(px(4.))
                    .child(copy(title, 13., theme.gray_12).font_weight(FontWeight::MEDIUM))
                    .child(copy(description, 11., theme.gray_10).line_height(px(15.)))
            })))
            .child(copy(t!("Change any of these at any time in Settings"), 12., theme.gray_9))
            .into_any_element()
    }

    fn render_faq(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .min_h_full()
            .px(px(48.))
            .py(px(24.))
            .gap(px(24.))
            .child(self.heading(
                t!("Frequently Asked Questions"),
                t!("Everything you need to know to get started."),
            ))
            .child(
                self.paper_card()
                    .w_full()
                    .max_w(px(480.))
                    .overflow_hidden()
                    .children(FAQ.iter().enumerate().map(|(index, (question, answer))| {
                        let open = self.faq_open[index];
                        div()
                            .when(index > 0, |el| {
                                el.border_t_1().border_color(theme.body_border(4))
                            })
                            .child(
                                div()
                                    .id(("onboarding-faq", index))
                                    .tab_index(0)
                                    .cursor_pointer()
                                    .px(px(20.))
                                    .py(px(16.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.faq_open[index] = !this.faq_open[index];
                                        cx.notify();
                                    }))
                                    .child(
                                        copy(tr(question), 13., theme.gray_12)
                                            .font_weight(FontWeight::MEDIUM),
                                    )
                                    .child(glyph(
                                        if open {
                                            "icons/chevron-up.svg"
                                        } else {
                                            "icons/chevron-down.svg"
                                        },
                                        16.,
                                        theme.gray_10,
                                    )),
                            )
                            .when(open, |el| {
                                el.child(div().px(px(20.)).pb(px(16.)).child(
                                    copy(tr(answer), 13., theme.gray_10).line_height(px(20.)),
                                ))
                            })
                    })),
            )
            .into_any_element()
    }

    fn render_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let paper = stage::Paper::new(&self.theme);
        let first = self.first_step as usize;
        let current = self.step as usize - first;
        let total = if self.permissions_only {
            1
        } else {
            Step::ALL.len() - first
        };
        let disabled = self.step == Step::Permissions && !self.state.necessary_granted();
        let last = self.permissions_only || self.step == Step::Faq;
        let relaunch = self.step == Step::Permissions && !disabled && self.needs_relaunch();
        let label = if relaunch {
            t!("Relaunch FrameEver")
        } else if self.permissions_only {
            t!("Continue to FrameEver")
        } else if last {
            t!("Start Using FrameEver")
        } else {
            t!("Continue")
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.))
            .px(px(36.))
            .pb(px(18.))
            .pt(px(8.))
            .flex_shrink_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(div().flex_1().when(current > 0, |el| {
                        el.child(
                            copy(t!("←  Back"), 13., paper.soft)
                                .id("onboarding-back")
                                .cursor_pointer()
                                .hover(move |el| el.text_color(paper.ink))
                                .on_click(cx.listener(|this, _, window, cx| this.back(window, cx))),
                        )
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .when(total > 1, |el| {
                                el.children((0..total).map(|index| {
                                    div()
                                        .h(px(6.))
                                        .w(px(if current == index { 22. } else { 6. }))
                                        .rounded_full()
                                        .bg(if current == index {
                                            paper.ink
                                        } else if current > index {
                                            paper.soft
                                        } else {
                                            paper.track
                                        })
                                }))
                            }),
                    )
                    .child(
                        div().flex_1().flex().justify_end().child(
                            div().flex().flex_col().items_end().gap(px(8.)).child(
                                div()
                                    .id("onboarding-continue")
                                    .tab_index(0)
                                    .h(px(46.))
                                    .px(px(26.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(8.))
                                    .border(px(1.5))
                                    .border_color(if disabled { paper.track } else { paper.ink })
                                    .bg(if disabled {
                                        gpui::transparent_black()
                                    } else {
                                        paper.ink
                                    })
                                    .text_color(if disabled { paper.soft } else { paper.bg })
                                    .when(!disabled, |el| {
                                        el.cursor_pointer().hover(|el| el.opacity(0.88)).on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.next(window, cx)
                                            }),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(label),
                                    )
                                    .child(glyph(
                                        if relaunch {
                                            "icons/rotate-ccw.svg"
                                        } else if last {
                                            "icons/check.svg"
                                        } else {
                                            "icons/move-right.svg"
                                        },
                                        15.,
                                        if disabled { paper.soft } else { paper.bg },
                                    )),
                            ),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(copy(
                        if disabled {
                            t!("Allow the required permissions to continue")
                        } else {
                            t!("Press Enter ↵ or use ← → arrow keys")
                        },
                        11.,
                        paper.soft,
                    ))
                    .when(
                        self.state.necessary_granted() && !self.permissions_only && !last,
                        |el| {
                            el.child(
                                copy(t!("Skip the tour"), 11., paper.soft)
                                    .id("onboarding-skip")
                                    .cursor_pointer()
                                    .hover(move |el| el.text_color(paper.ink))
                                    .on_click(cx.listener(|this, _, _, cx| this.finish(cx))),
                            )
                        },
                    ),
            )
            .when_some(self.error.clone(), |el, error| {
                el.child(copy(error, 12., paper.red))
            })
            .into_any_element()
    }
}

impl OnboardingWindow {
    fn demo_steps(&self, labels: [&'static str; 3], active: usize) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .pb(px(12.))
            .children(labels.into_iter().enumerate().map(|(index, label)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when(index > 0, |el| {
                        el.child(div().w(px(12.)).h(px(1.)).bg(theme.gray_5))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .px(px(8.))
                            .py(px(4.))
                            .rounded_full()
                            .border_1()
                            .border_color(if index == active {
                                theme.border_strong
                            } else {
                                theme.gray_4
                            })
                            .bg(if index == active {
                                Hsla::from(theme.selection)
                            } else {
                                theme.body_fill(1)
                            })
                            .child(copy(
                                format!("{}", index + 1),
                                9.,
                                if index == active {
                                    theme.selection_foreground
                                } else {
                                    theme.gray_9
                                },
                            ))
                            .child(copy(
                                label,
                                10.,
                                if index == active {
                                    theme.selection_foreground
                                } else {
                                    theme.gray_10
                                },
                            )),
                    )
            }))
    }

    fn demo_cursor(&self) -> gpui::Img {
        let windows = cfg!(target_os = "windows");
        img(if windows {
            "onboarding/cursor-windows.svg"
        } else {
            "onboarding/cursor-macos.svg"
        })
        .w(px(if windows { 24. } else { 22. }))
        .h(px(if windows { 34. } else { 32. }))
        .object_fit(ObjectFit::Contain)
    }

    fn demo_start(&self, index: usize) -> AnyElement {
        let theme = self.theme;
        let cursor = div().absolute().child(self.demo_cursor());
        let cursor = if self.animated {
            cursor
                .with_animation(
                    ("demo-start-cursor", self.demo_cycle),
                    Animation::new(Duration::from_millis(1950)),
                    |el, t| {
                        let arrival = ease(((t * 1950. - 40.) / 1450.).clamp(0., 1.));
                        let press = if t * 1950. >= 1770. { 3. } else { 0. };
                        el.left(px(-44. + 124. * arrival))
                            .top(px(54. - 28. * arrival + press))
                    },
                )
                .into_any_element()
        } else {
            cursor.left(px(80.)).top(px(29.)).into_any_element()
        };
        div()
            .relative()
            .w_full()
            .max_w(px(288.))
            .pb(px(32.))
            .child(
                div()
                    .h(px(44.))
                    .w_full()
                    .rounded_full()
                    .flex()
                    .overflow_hidden()
                    .bg(theme.primary)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .pl(px(16.))
                            .gap(px(12.))
                            .child(glyph(MODES[index].icon, 16., gpui::white()))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        copy(t!("Start Recording"), 15., gpui::white())
                                            .font_weight(FontWeight::MEDIUM)
                                            .whitespace_nowrap(),
                                    )
                                    .child(
                                        copy(MODES[index].title, 11., gpui::white()).opacity(0.9),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px(px(12.))
                            .border_l_1()
                            .border_color(rgba(0xffffff33))
                            .bg(rgba(0xffffff0d))
                            .child(glyph("icons/caret-down.svg", 16., gpui::white())),
                    ),
            )
            .child(cursor)
            .into_any_element()
    }

    fn demo_recording(&self) -> Div {
        let theme = self.theme;
        let stopped = self.demo_phase >= 6;
        let time = format!("0:0{}", self.demo_phase.saturating_sub(2).min(3));
        self.card()
            .bg(if theme.is_dark() {
                theme.gray_1.into()
            } else {
                gpui::white()
            })
            .w_full()
            .min_w(px(280.))
            .h(px(40.))
            .rounded(px(16.))
            .flex()
            .items_stretch()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .px(px(8.))
                            .flex_shrink_0()
                            .when(stopped, |el| {
                                el.child(div().size(px(8.)).rounded_full().bg(theme.gray_8))
                            })
                            .when(!stopped, |el| {
                                el.child(glyph("icons/stop-circle.svg", 20., theme.danger_text))
                            })
                            .child(copy(
                                if stopped { t!("Stopped").into() } else { time },
                                14.,
                                if stopped {
                                    Hsla::from(theme.gray_10)
                                } else {
                                    theme.danger_text.into()
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .opacity(if stopped { 0.45 } else { 1. })
                            .child(
                                div()
                                    .relative()
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(glyph("icons/microphone.svg", 20., theme.gray_12))
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(3.))
                                            .left(px(4.))
                                            .right(px(4.))
                                            .h(px(2.))
                                            .rounded_full()
                                            .bg(theme.gray_10)
                                            .child(
                                                div().w(relative(0.6)).h_full().bg(theme.primary),
                                            ),
                                    ),
                            )
                            .children(
                                [
                                    "icons/pause-circle.svg",
                                    "icons/restart.svg",
                                    "icons/trash.svg",
                                    "icons/settings.svg",
                                ]
                                .into_iter()
                                .map(|path| {
                                    div()
                                        .size(px(28.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(glyph(path, 20., theme.gray_11))
                                }),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(32.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_l_1()
                    .border_color(theme.gray_5)
                    .opacity(if stopped { 0.45 } else { 1. })
                    .child(glyph("icons/more-vertical.svg", 20., theme.gray_10)),
            )
    }

    fn demo_link(&self) -> Div {
        let theme = self.theme;
        let copied = self.demo_phase >= 8;
        self.card()
            .bg(if theme.is_dark() {
                theme.gray_1.into()
            } else {
                gpui::white()
            })
            .w_full()
            .max_w(px(340.))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .child(
                        div()
                            .size(px(20.))
                            .rounded_full()
                            .bg(self.success_palette().0)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(glyph("icons/check.svg", 12., self.success_palette().2)),
                    )
                    .child(copy(t!("Video saved!"), 12., theme.gray_12)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .m(px(12.))
                    .mt_0()
                    .p(px(8.))
                    .rounded(px(8.))
                    .bg(theme.body_fill(3))
                    .child(copy("Recording.mp4", 11., theme.gray_11).flex_1().min_w_0())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .px(px(8.))
                            .py(px(5.))
                            .rounded(px(6.))
                            .bg(if copied {
                                self.success_palette().0
                            } else {
                                theme.body_fill(1)
                            })
                            .child(glyph(
                                if copied {
                                    "icons/check.svg"
                                } else {
                                    "icons/copy.svg"
                                },
                                12.,
                                if copied {
                                    self.success_palette().2
                                } else {
                                    theme.body_text()
                                },
                            ))
                            .child(copy(
                                if copied { t!("Copied!") } else { t!("Copy") },
                                11.,
                                if copied {
                                    self.success_palette().2
                                } else {
                                    theme.body_text()
                                },
                            )),
                    ),
            )
    }

    fn demo_document(&self) -> Div {
        let theme = self.theme;
        self.card()
            .bg(if theme.is_dark() {
                theme.gray_1.into()
            } else {
                gpui::white()
            })
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(6.))
            .p(px(12.))
            .child(
                div()
                    .w(relative(0.75))
                    .h(px(6.))
                    .rounded_full()
                    .bg(theme.gray_5)
                    .opacity(0.5),
            )
            .child(
                div()
                    .w(relative(0.5))
                    .h(px(6.))
                    .rounded_full()
                    .bg(theme.gray_5)
                    .opacity(0.3),
            )
            .child(
                div()
                    .w_full()
                    .h(px(20.))
                    .mt(px(4.))
                    .rounded(px(3.))
                    .bg(theme.gray_5)
                    .opacity(0.2),
            )
    }

    fn demo_editor(&self) -> Div {
        let theme = self.theme;
        let phase = self.demo_phase;
        let progress = if phase >= 11 {
            100
        } else if phase >= 10 {
            75
        } else {
            25
        };
        self.card()
            .bg(if theme.is_dark() {
                theme.gray_1.into()
            } else {
                gpui::white()
            })
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .h(px(36.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .border_b_1()
                    .border_color(theme.gray_3)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(self.demo_window_dots())
                            .child(copy("FrameEver Editor", 10., theme.gray_11)),
                    )
                    .child(
                        copy(t!("Export"), 9., gpui::white())
                            .px(px(10.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .bg(theme.primary)
                            .when(phase >= 8, |el| {
                                el.border_2().border_color(theme.foreground)
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div().flex_1().min_w_0().p(px(12.)).child(
                            div()
                                .size_full()
                                .p(px(10.))
                                .rounded(px(8.))
                                .bg(theme.preview)
                                .child(self.demo_document().w_full().h(px(80.))),
                        ),
                    )
                    .child(
                        div()
                            .w(px(90.))
                            .flex_shrink_0()
                            .border_l_1()
                            .border_color(theme.gray_3)
                            .p(px(8.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(copy("STYLE", 8., theme.gray_9))
                            .child(
                                div()
                                    .h(px(20.))
                                    .rounded(px(3.))
                                    .border_1()
                                    .border_color(theme.gray_3)
                                    .bg(theme.body_fill(2)),
                            )
                            .child(copy("BACKGROUND", 8., theme.gray_9).mt(px(4.)))
                            .child(div().flex().gap(px(4.)).children(
                                [0x667eea, 0xf78c88, 0xcccccc].map(|color| {
                                    div()
                                        .size(px(16.))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(theme.gray_3)
                                        .bg(rgb(color))
                                }),
                            )),
                    )
                    .when(phase >= 9, |el| {
                        el.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(rgba(0x00000040))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    self.card()
                                        .min_w(px(200.))
                                        .px(px(24.))
                                        .py(px(20.))
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap(px(12.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .when(phase >= 11, |el| {
                                                    el.child(glyph(
                                                        "icons/check.svg",
                                                        18.,
                                                        self.success_palette().2,
                                                    ))
                                                })
                                                .child(copy(
                                                    if phase >= 11 {
                                                        t!("Export complete!")
                                                    } else {
                                                        t!("Exporting...")
                                                    },
                                                    14.,
                                                    theme.gray_12,
                                                )),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .h(px(8.))
                                                .rounded_full()
                                                .bg(theme.gray_4)
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .h_full()
                                                        .w(relative(progress as f32 / 100.))
                                                        .rounded_full()
                                                        .bg(theme.primary),
                                                ),
                                        )
                                        .child(copy(format!("{progress}%"), 12., theme.gray_10)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(theme.gray_3)
                    .px(px(12.))
                    .py(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(24.))
                            .px(px(8.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(theme.gray_3)
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(3.))
                                    .rounded_full()
                                    .bg(theme.gray_4)
                                    .child(
                                        div()
                                            .w(relative(0.42))
                                            .h_full()
                                            .rounded_full()
                                            .bg(theme.gray_8),
                                    ),
                            )
                            .child(copy("0:12", 8., theme.gray_10)),
                    ),
            )
    }

    fn demo_window_dots(&self) -> Div {
        div()
            .flex()
            .gap(px(4.))
            .children((0..3).map(|_| div().size(px(8.)).rounded_full().bg(self.theme.gray_6)))
    }

    fn demo_screenshot(&self) -> AnyElement {
        let theme = self.theme;
        let phase = self.demo_phase;
        if phase <= 5 {
            let selection = div()
                .absolute()
                .left(relative(0.06))
                .top(relative(0.1))
                .w(relative(0.84))
                .h(relative(0.78))
                .border_2()
                .border_color(theme.foreground)
                .bg(rgba(0xffffff12))
                .when(phase >= 4, |el| {
                    el.children(
                        [(false, false), (false, true), (true, false), (true, true)].map(
                            |(right, bottom)| {
                                div()
                                    .absolute()
                                    .size(px(6.))
                                    .rounded(px(1.))
                                    .bg(gpui::white())
                                    .border_1()
                                    .border_color(theme.foreground)
                                    .when(right, |el| el.right(px(-3.)))
                                    .when(!right, |el| el.left(px(-3.)))
                                    .when(bottom, |el| el.bottom(px(-3.)))
                                    .when(!bottom, |el| el.top(px(-3.)))
                            },
                        ),
                    )
                })
                .child(
                    copy("640 × 480", 10., gpui::white())
                        .absolute()
                        .bottom(px(-26.))
                        .left(relative(0.35))
                        .rounded(px(4.))
                        .px(px(8.))
                        .py(px(3.))
                        .bg(rgba(0x000000b3)),
                );
            let selection = if phase == 3 && self.animated {
                selection
                    .with_animation(
                        ("screenshot-selection", self.demo_cycle),
                        Animation::new(Duration::from_millis(1200)).with_easing(ease),
                        |el, t| el.w(relative(0.84 * t)).h(relative(0.78 * t)),
                    )
                    .into_any_element()
            } else {
                selection.into_any_element()
            };
            return self
                .demo_document()
                .relative()
                .w_full()
                .max_w(px(380.))
                .h(px(200.))
                .overflow_hidden()
                .opacity(if phase == 0 { 0. } else { 1. })
                .when(phase >= 2, |el| {
                    el.child(div().absolute().inset_0().bg(rgba(0x00000073)))
                })
                .when(phase >= 3, |el| el.child(selection))
                .when(phase == 2, |el| {
                    el.child(
                        self.demo_cursor()
                            .absolute()
                            .left(relative(0.06))
                            .top(relative(0.1)),
                    )
                })
                .when(phase == 5, |el| {
                    el.child(div().absolute().inset_0().bg(rgba(0xffffff4d)))
                })
                .into_any_element();
        }
        self.card()
            .w_full()
            .max_w(px(420.))
            .overflow_hidden()
            .child(
                div()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .border_b_1()
                    .border_color(theme.gray_3)
                    .child(self.demo_window_dots())
                    .child(
                        div().flex().gap(px(4.)).children(
                            [
                                "icons/cursor.svg",
                                "icons/square.svg",
                                "icons/circle.svg",
                                "icons/move-right.svg",
                                "icons/type.svg",
                            ]
                            .map(|path| {
                                div()
                                    .size(px(20.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .bg(theme.body_fill(3))
                                    .child(glyph(path, 12., theme.gray_11))
                            }),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                copy(t!("Copy"), 9., theme.gray_12)
                                    .px(px(6.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .bg(theme.body_fill(3)),
                            )
                            .child(
                                copy(t!("Save"), 9., gpui::white())
                                    .px(px(6.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .bg(theme.primary),
                            ),
                    ),
            )
            .child(
                div().p(px(12.)).child(
                    div()
                        .relative()
                        .h(px(140.))
                        .w_full()
                        .overflow_hidden()
                        .rounded(px(4.))
                        .when(phase >= 7, |el| el.bg(theme.preview))
                        .child(
                            self.demo_document()
                                .absolute()
                                .inset(px(if phase >= 8 { 12. } else { 0. }))
                                .rounded(px(if phase >= 8 { 8. } else { 0. })),
                        )
                        .when(phase >= 9, |el| {
                            el.child(
                                div()
                                    .absolute()
                                    .bottom(px(8.))
                                    .left_0()
                                    .right_0()
                                    .flex()
                                    .justify_center()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.))
                                            .px(px(12.))
                                            .py(px(6.))
                                            .rounded_full()
                                            .bg(theme.body_fill(1))
                                            .child(glyph(
                                                "icons/check.svg",
                                                12.,
                                                self.success_palette().2,
                                            ))
                                            .child(copy(
                                                t!("Copied to clipboard"),
                                                11.,
                                                theme.gray_12,
                                            )),
                                    ),
                            )
                        }),
                ),
            )
            .into_any_element()
    }

    fn render_demo(&self, index: usize) -> AnyElement {
        let phase = self.demo_phase;
        let (labels, active) = match index {
            0 => (
                [t!("Record"), t!("Stop"), t!("Saved")],
                if phase <= 5 {
                    0
                } else if phase == 6 {
                    1
                } else {
                    2
                },
            ),
            1 => (
                [t!("Record"), t!("Edit"), t!("Export")],
                if phase < 7 {
                    0
                } else if phase < 9 {
                    1
                } else {
                    2
                },
            ),
            _ => (
                [t!("Select area"), t!("Beautify"), t!("Copy")],
                if phase <= 5 {
                    0
                } else if phase <= 8 {
                    1
                } else {
                    2
                },
            ),
        };
        let content = if index == 2 {
            self.demo_screenshot()
        } else if phase <= 1 {
            div()
                .w_full()
                .flex()
                .justify_center()
                .opacity(if phase == 0 { 0. } else { 1. })
                .child(self.demo_start(index))
                .into_any_element()
        } else if phase <= 6 {
            self.demo_recording().into_any_element()
        } else if index == 0 {
            self.demo_link().into_any_element()
        } else {
            self.demo_editor().into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.demo_steps(labels, active))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(content),
            )
            .into_any_element()
    }
}

impl Render for OnboardingWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = Theme::for_window(window, cx, true);
        if self.permissions_visible() {
            self.sync_stage(window, cx);
        }
        if self.welcome {
            self.tick_welcome(window, cx);
        }
        let theme = self.theme;
        let paper = stage::Paper::new(&theme);
        let content = match self.step {
            Step::Permissions => self.render_permissions(cx),
            Step::Overview => self.render_overview(),
            Step::Instant => self.render_detail(0),
            Step::Studio => self.render_detail(1),
            Step::Screenshot => self.render_detail(2),
            Step::Toggle => self.render_toggle(cx),
            Step::Shortcuts => self.render_shortcuts(),
            Step::Faq => self.render_faq(cx),
        };
        let content = div().relative().min_h_full().w_full().child(content);
        let step_visible = !self.welcome || self.welcome_exiting;
        let content = if self.animated && step_visible {
            let direction = self.transition_direction;
            let delay = self.step_delay.as_secs_f32();
            content
                .with_animation(
                    ("onboarding-step", self.step_revision),
                    Animation::new(Duration::from_secs_f32(0.5 + delay)),
                    move |el, t| {
                        let t = ease(((t * (0.5 + delay) - delay) / 0.5).clamp(0., 1.));
                        el.left(px(direction * 24. * (1. - t))).opacity(t)
                    },
                )
                .into_any_element()
        } else {
            content.into_any_element()
        };
        let header = div()
            .id("onboarding-header")
            .h(px(ONBOARDING_HEADER_HEIGHT))
            .flex_shrink_0()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .when(!self.welcome, |el| el.bg(paper.bg))
            .window_control_area(gpui::WindowControlArea::Drag);
        #[cfg(target_os = "windows")]
        let header = header.justify_end().child(ui::windows_caption_controls(
            theme,
            window.is_window_active(),
            window.is_maximized(),
            true,
            false,
        ));
        #[cfg(not(target_os = "windows"))]
        let header = header
            .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(
                div()
                    .id("onboarding-close")
                    .group("onboarding-close")
                    .tab_index(0)
                    .size(px(14.))
                    .ml(px(12.))
                    .rounded_full()
                    .bg(rgb(Theme::TRAFFIC_CLOSE))
                    .hover(|el| el.bg(rgb(0xf25a53)))
                    .active(|el| el.bg(rgb(0xe6564e)))
                    .cursor_default()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(
                        cx.listener(|_, _, _, cx| cx.defer(crate::app_windows::close_onboarding)),
                    )
                    .child(
                        glyph("icons/traffic-close.svg", 10., rgba(0x00000080))
                            .invisible()
                            .group_hover("onboarding-close", |el| el.visible()),
                    ),
            );
        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .when(step_visible, |el| {
                el.bg(paper.bg).child(
                    div()
                        .relative()
                        .size_full()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .id("onboarding-scroll")
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .overflow_y_scroll()
                                .child(content),
                        )
                        .child(self.render_navigation(cx)),
                )
            });
        let root = div()
            .id("onboarding")
            .track_focus(&self.focus)
            .key_context("OnboardingWindow")
            .capture_key_down(cx.listener(Self::on_key))
            .relative()
            .size_full()
            .overflow_hidden()
            .when(!cfg!(target_os = "windows"), |root| root.rounded(px(16.)))
            .bg(theme.shell_bg())
            .border_1()
            .border_color(paper.track)
            .font_family(crate::theme::ui_font())
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.body_text());
        if self.welcome {
            root.child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .bg(paper.bg)
                    .child(div().h(px(ONBOARDING_HEADER_HEIGHT)).flex_shrink_0())
                    .child(body),
            )
            .child(self.render_welcome(cx))
            .child(header.absolute().top_0().left_0().right_0())
        } else {
            root.flex().flex_col().child(header).child(body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::{MediaAuthorization, RawPermissions};

    #[test]
    fn first_run_visits_every_platform_stage_and_back_never_opens_an_omitted_step() {
        for (macos, expected) in [(true, 8), (false, 7)] {
            let first = Step::first(macos, false);
            let mut visited = vec![first];
            while let Some(next) = visited.last().unwrap().next(false, true) {
                visited.push(next);
            }
            assert_eq!(visited.len(), expected);
            assert_eq!(visited.last(), Some(&Step::Faq));
            for pair in visited.windows(2).rev() {
                assert_eq!(pair[1].previous(first), Some(pair[0]));
            }
            assert_eq!(first.previous(first), None);
            assert_eq!(visited.contains(&Step::Permissions), macos);
        }
    }

    #[test]
    fn permissions_revisit_stays_on_required_permissions_on_every_platform() {
        for macos in [false, true] {
            let first = Step::first(macos, true);
            assert_eq!(first, Step::Permissions);
            assert_eq!(first.next(true, false), None);
            assert_eq!(first.next(true, true), None);
            assert_eq!(first.previous(first), None);
        }
    }

    #[test]
    fn denied_optional_devices_do_not_block_the_tour_but_either_required_permission_does() {
        for (screen, accessibility) in [(false, false), (false, true), (true, false), (true, true)]
        {
            let state = PermissionsState::from_raw(Some(RawPermissions {
                screen_granted: screen,
                accessibility_granted: accessibility,
                microphone: MediaAuthorization::Denied,
                camera: MediaAuthorization::Restricted,
            }));
            assert_eq!(
                Step::Permissions.next(false, state.necessary_granted()),
                (screen && accessibility).then_some(Step::Overview)
            );
            assert!(!state.all_shown_granted());
        }
    }

    #[test]
    fn a_revoked_required_permission_blocks_continuing_after_a_refresh() {
        let mut raw = RawPermissions {
            screen_granted: true,
            accessibility_granted: true,
            microphone: MediaAuthorization::Authorized,
            camera: MediaAuthorization::Authorized,
        };
        let mut state = PermissionsState::from_raw(Some(raw));
        assert_eq!(
            Step::Permissions.next(false, state.necessary_granted()),
            Some(Step::Overview)
        );
        raw.screen_granted = false;
        assert!(state.apply_raw(Some(raw)));
        assert_eq!(
            Step::Permissions.next(false, state.necessary_granted()),
            None
        );
    }
}
