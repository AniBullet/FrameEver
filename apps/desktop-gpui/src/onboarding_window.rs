use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, Context, Div, FocusHandle, FontWeight, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, svg,
};

use crate::{
    i18n::t,
    main_window::Mode,
    onboarding_audio::OnboardingAudio,
    permissions::{self, OSPermission, OSPermissionStatus},
    permissions_ui::{PermissionsState, RowAction},
    store,
    theme::Theme,
};

#[cfg(target_os = "windows")]
use crate::ui;
#[cfg(not(target_os = "windows"))]
use gpui::{rgb, rgba};

mod demos;
mod shortcuts;
mod stage;
mod steps;

use shortcuts::Leave;

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
    Modes,
    Studio,
    Instant,
    Screenshot,
    Library,
    Shortcuts,
    Faq,
}

fn guided_permission(permission: OSPermission) -> bool {
    permission.required() && crate::permission_guide::eligible(permission)
}

impl Step {
    const ALL: [Self; 8] = [
        Self::Permissions,
        Self::Modes,
        Self::Studio,
        Self::Instant,
        Self::Screenshot,
        Self::Library,
        Self::Shortcuts,
        Self::Faq,
    ];

    fn first(macos: bool, permissions_only: bool, again: bool) -> Self {
        if permissions_only || (macos && !again) {
            Self::Permissions
        } else {
            Self::Modes
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
            Self::Studio => Some((
                &[600, 1600, 2600, 3600, 4400, 5000, 6000, 7200, 7400, 9200],
                11000,
            )),
            Self::Instant => Some((&[600, 1600, 2600, 3400, 4000, 5600], 8000)),
            Self::Screenshot => Some((&[500, 1700, 2300, 2900, 3500, 4100, 5000], 6800)),
            Self::Library => Some((&[900, 2200, 2700, 4200, 5600, 6600], 8400)),
            _ => None,
        }
    }
}

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
    mode: Mode,
    again: bool,
    shortcuts: shortcuts::Draft,
    step_revision: usize,
    transition_direction: f32,
    faq_open: [bool; steps::FAQ.len()],
    storage: std::path::PathBuf,
    error: Option<String>,
    focus: FocusHandle,
    stage: stage::StageState,
    step_at: std::time::Instant,
}

impl OnboardingWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.on_window_should_close(cx, |_window, cx| {
            cx.defer(|cx| {
                crate::hotkeys::resume(cx);
            });
            cx.defer(crate::app_windows::onboarding_closed);
            true
        });
        let forced = crate::permissions_ui::surface_forced();
        let state = PermissionsState::initial();
        let revisit = store::has_completed_onboarding() && !state.necessary_granted();
        let permissions_only = revisit || forced;
        let again = crate::app_windows::take_onboarding_again(cx);
        let first_step = Step::first(cfg!(target_os = "macos"), permissions_only, again);
        let welcome = !again && !store::has_completed_startup() && !permissions_only;
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
            animated: animates(window, cx),
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
            mode: Mode::from_store(),
            again,
            shortcuts: shortcuts::Draft::new(&store::hotkeys_raw(), !again),
            step_revision: 0,
            transition_direction: 1.,
            faq_open: std::array::from_fn(|index| index == 1),
            storage: steps::storage_folder(),
            error: None,
            focus: cx.focus_handle(),
            stage: stage::StageState::new(granted),
            step_at: std::time::Instant::now(),
        };
        cx.observe_window_activation(window, |this: &mut Self, window, cx| {
            this.animated = animates(window, cx);
            if window.is_window_active() {
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
            self.error = Some(t!("Couldn't save your progress. Try again.").into());
            cx.notify();
            return;
        }
        self.stop_listening(cx);
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
            self.error = Some(t!("Couldn't save your progress. Try again.").into());
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
        let exit = if cx.reduce_motion() {
            Duration::ZERO
        } else {
            WELCOME_EXIT
        };
        self.welcome_exit = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(exit).await;
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
        self.error = None;
        self.stop_listening(cx);
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
        if self.step == Step::Shortcuts && !self.leave_shortcuts(Leave::Continue, cx) {
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
        if self.step == Step::Shortcuts {
            self.leave_shortcuts(Leave::Back, cx);
        }
        if !self.permissions_only
            && let Some(previous) = self.step.previous(self.first_step)
        {
            self.go_to(previous, window, cx);
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if self.shortcuts.listening.is_some() {
            cx.stop_propagation();
            self.capture_shortcut(key, window.modifiers().shift, cx);
            return;
        }
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
        if self.welcome || !animates(window, cx) || self.demo_task.is_some() {
            return;
        }
        let Some((timings, cycle)) = self.step.demo_timing() else {
            return;
        };
        self.demo_phase = 0;
        self.demo_cycle = self.demo_cycle.wrapping_add(1);
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
                            if this.welcome || this.step != step || !animates(window, cx) {
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
                        if this.welcome || this.step != step || !animates(window, cx) {
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

fn animates(window: &Window, cx: &gpui::App) -> bool {
    window.is_window_active() && !cx.reduce_motion()
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
    fn paper_card(&self) -> Div {
        let paper = stage::Paper::new(&self.theme);
        div()
            .rounded(px(20.))
            .border(px(1.5))
            .border_color(paper.track)
            .bg(paper.card)
    }

    fn heading(&self, title: impl Into<SharedString>, subtitle: impl Into<SharedString>) -> Div {
        let paper = stage::Paper::new(&self.theme);
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .max_w(px(520.))
            .text_center()
            .child(
                copy(title, 28., paper.ink)
                    .font_weight(FontWeight::MEDIUM)
                    .line_height(px(34.)),
            )
            .child(
                copy(subtitle, 14., paper.soft)
                    .font_weight(FontWeight::NORMAL)
                    .line_height(px(21.)),
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

        let theme = self.theme;
        let button = div()
            .id("onboarding-get-started")
            .tab_index(0)
            .cursor_pointer()
            .h(px(46.))
            .px(px(30.))
            .rounded(px(12.))
            .bg(theme.primary)
            .hover(move |el| el.bg(theme.primary_hover))
            .active(|el| el.opacity(0.92))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(|this, _, window, cx| this.continue_from_welcome(window, cx)))
            .child(
                copy(t!("Get started"), 15., theme.primary_foreground)
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
                el.child(copy(error, 12., self.theme.danger_text).mt(px(12.)))
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

    fn render_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let paper = stage::Paper::new(&theme);
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
            t!("Start using")
        } else {
            t!("Continue")
        };
        let skip = self.state.necessary_granted() && !self.permissions_only && !last;
        let primary = div()
            .id("onboarding-continue")
            .tab_index(0)
            .h(px(36.))
            .px(px(18.))
            .rounded(px(12.))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .border_1()
            .text_size(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .when(relaunch, |el| {
                el.child(glyph("icons/rotate-ccw.svg", 16., theme.primary_foreground))
            })
            .child(label);
        let primary = if disabled {
            primary
                .border_color(theme.border)
                .text_color(theme.muted_foreground)
        } else {
            primary
                .border_color(theme.primary)
                .bg(theme.primary)
                .text_color(theme.primary_foreground)
                .cursor_pointer()
                .hover(move |el| el.bg(theme.primary_hover).border_color(theme.primary_hover))
                .active(move |el| {
                    el.bg(theme.primary_active)
                        .border_color(theme.primary_active)
                })
                .on_click(cx.listener(|this, _, window, cx| this.next(window, cx)))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .flex_shrink_0()
            .px(px(24.))
            .pt(px(12.))
            .pb(px(18.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().when(current > 0, |el| {
                        el.child(
                            copy(t!("← Back"), 13., paper.soft)
                                .id("onboarding-back")
                                .px(px(8.))
                                .py(px(6.))
                                .cursor_pointer()
                                .hover(move |el| el.text_color(paper.ink))
                                .on_click(cx.listener(|this, _, window, cx| this.back(window, cx))),
                        )
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.))
                            .when(total > 1, |el| {
                                el.child(div().flex().gap(px(6.)).children((0..total).map(
                                    |index| {
                                        div()
                                            .h(px(6.))
                                            .w(px(if current == index { 22. } else { 6. }))
                                            .rounded(px(3.))
                                            .bg(if current == index {
                                                paper.ink
                                            } else if current > index {
                                                paper.soft
                                            } else {
                                                paper.track
                                            })
                                    },
                                )))
                            })
                            .child(copy(
                                if disabled {
                                    t!("Allow the required permissions to continue")
                                } else {
                                    t!("Press Enter or use ← →")
                                },
                                11.,
                                theme.faint_foreground,
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(12.))
                            .when(skip, |el| {
                                el.child(
                                    copy(t!("Skip the tour"), 11., paper.soft)
                                        .id("onboarding-skip")
                                        .cursor_pointer()
                                        .hover(move |el| el.text_color(paper.ink))
                                        .on_click(cx.listener(|this, _, _, cx| this.skip(cx))),
                                )
                            })
                            .child(primary),
                    ),
            )
            .when_some(self.error.clone(), |el, error| {
                el.child(
                    div()
                        .flex()
                        .justify_center()
                        .child(copy(error, 12., theme.danger_text)),
                )
            })
            .into_any_element()
    }
}

impl Render for OnboardingWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = Theme::for_window(window, cx, true);
        let animated = animates(window, cx);
        if animated != self.animated {
            self.animated = animated;
            if animated {
                self.arm_demo(window, cx);
            } else {
                self.demo_task = None;
            }
        }
        if !self.animated
            && let Some((timings, _)) = self.step.demo_timing()
        {
            self.demo_phase = timings.len();
        }
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
            Step::Modes => self.render_modes(cx),
            Step::Studio | Step::Instant | Step::Screenshot => self.render_detail(),
            Step::Library => self.render_library(),
            Step::Shortcuts => self.render_shortcuts(cx),
            Step::Faq => self.render_faq(cx),
        };
        let content = div()
            .relative()
            .flex()
            .flex_col()
            .flex_grow(1.)
            .flex_shrink_0()
            .w_full()
            .child(content);
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
                                .flex()
                                .flex_col()
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
            let first = Step::first(macos, false, false);
            let mut visited = vec![first];
            while let Some(next) = visited.last().unwrap().next(false, true) {
                visited.push(next);
            }
            assert_eq!(visited.len(), expected);
            let tour: Vec<Step> = visited
                .iter()
                .copied()
                .filter(|step| *step != Step::Permissions)
                .collect();
            assert_eq!(
                tour,
                [
                    Step::Modes,
                    Step::Studio,
                    Step::Instant,
                    Step::Screenshot,
                    Step::Library,
                    Step::Shortcuts,
                    Step::Faq,
                ]
            );
            for pair in visited.windows(2).rev() {
                assert_eq!(pair[1].previous(first), Some(pair[0]));
            }
            assert_eq!(first.previous(first), None);
            assert_eq!(visited.contains(&Step::Permissions), macos);
        }
    }

    #[test]
    fn showing_onboarding_again_skips_permissions_and_starts_at_modes() {
        for macos in [false, true] {
            let first = Step::first(macos, false, true);
            assert_eq!(first, Step::Modes);
            assert_eq!(first.previous(first), None);
            assert_eq!(Step::first(macos, true, true), Step::Permissions);
        }
    }

    #[test]
    fn permissions_revisit_stays_on_required_permissions_on_every_platform() {
        for macos in [false, true] {
            let first = Step::first(macos, true, false);
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
                (screen && accessibility).then_some(Step::Modes)
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
            Some(Step::Modes)
        );
        raw.screen_granted = false;
        assert!(state.apply_raw(Some(raw)));
        assert_eq!(
            Step::Permissions.next(false, state.necessary_granted()),
            None
        );
    }
}
