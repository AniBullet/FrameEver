use std::time::{Duration, Instant};

use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, FontWeight, Global, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, Styled, Task, Window,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, prelude::FluentBuilder as _, px,
    size,
};

use crate::doodle::{self, Doodle};
use crate::i18n::t;
use crate::onboarding_doodles as art;
use crate::onboarding_window::{Paper, paper_button};
use crate::permissions::{self, OSPermission};
use crate::theme::Theme;

const WIDTH: f32 = 440.;
const POLL: Duration = Duration::from_millis(800);
const CLOSE_AFTER_GRANT: Duration = Duration::from_millis(2600);
const ANIMATE_FOR: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Need {
    Keystrokes,
    WindowControl { app: Option<String> },
}

impl Need {
    pub fn permission(&self) -> OSPermission {
        match self {
            Self::Keystrokes => permissions::keystroke_permission(),
            Self::WindowControl { .. } => OSPermission::Accessibility,
        }
    }
}

pub fn steps(need: &Need, settings_only: bool, held: bool) -> Vec<String> {
    let mut steps = vec![t!("Click Open System Settings below.").to_string()];
    match (need, settings_only) {
        (Need::WindowControl { app }, true) if held => {
            steps.push(t!("Click ⓘ next to FrameEver.").to_string());
            steps.push(match app {
                Some(app) => t!("Click + and add {app}.", app = app),
                None => t!("Click + and add the app you're recording.").to_string(),
            });
            steps.push(
                t!("Set it to At Any Time so macOS doesn't ask again every day.").to_string(),
            );
        }
        (Need::Keystrokes, true) => {
            steps.push(t!("Click + under Input Monitoring.").to_string());
            steps.push(t!("Choose FrameEver and enter your Mac password.").to_string());
        }
        (Need::WindowControl { app }, true) => {
            steps.push(t!("Click + in the pane that opens and choose FrameEver.").to_string());
            steps.push(match app {
                Some(app) => t!("Pick Choose Apps, then add {app}.", app = app),
                None => t!("Pick Choose Apps, then add the app you're recording.").to_string(),
            });
            steps.push(
                t!("Set it to At Any Time so macOS doesn't ask again every day.").to_string(),
            );
        }
        (_, false) => {
            steps.push(t!("Find FrameEver under Accessibility and switch it on.").to_string())
        }
    }
    steps
}

#[derive(Default)]
struct HelpGlobal {
    handle: Option<WindowHandle<PermissionHelp>>,
    reveal_main_on_close: bool,
}

fn closed(cx: &mut App) {
    let reveal_main = cx
        .try_global::<HelpGlobal>()
        .is_some_and(|help| help.reveal_main_on_close);
    cx.set_global(HelpGlobal::default());
    if reveal_main {
        crate::app_windows::show_main_window(cx);
    }
}

pub fn show_from_picker(need: Need, cx: &mut App) {
    crate::app_windows::close_target_overlays(cx);
    open(need, true, cx);
}

impl Global for HelpGlobal {}

pub fn keystrokes_missing() -> bool {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    const TTL: Duration = Duration::from_secs(2);

    if !cfg!(target_os = "macos") {
        return false;
    }
    let Ok(mut cache) = CACHE.lock() else {
        return false;
    };
    if let Some((at, missing)) = *cache
        && at.elapsed() < TTL
        && !INVALIDATED.swap(false, std::sync::atomic::Ordering::AcqRel)
    {
        return missing;
    }
    let missing = !permissions::keystrokes_live();
    *cache = Some((Instant::now(), missing));
    missing
}

static INVALIDATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn show(need: Need, cx: &mut App) {
    open(need, false, cx);
}

fn open(need: Need, reveal_main_on_close: bool, cx: &mut App) {
    if !cfg!(target_os = "macos") {
        return;
    }
    if let Some(handle) = cx.try_global::<HelpGlobal>().and_then(|help| help.handle) {
        let reused = handle
            .update(cx, |help, window, cx| {
                if help.need != need {
                    help.retarget(need.clone(), window, cx);
                }
                window.activate_window();
            })
            .is_ok();
        if reused {
            if reveal_main_on_close {
                cx.global_mut::<HelpGlobal>().reveal_main_on_close = true;
            }
            cx.activate(true);
            return;
        }
    }

    let settings_only = permissions::grants_are_settings_only();
    let rows = steps(
        &need,
        settings_only,
        permissions::granted_now(need.permission()),
    )
    .len() as f32;
    let height = 410. + rows * 36.;
    let bounds = Bounds::centered(None, size(px(WIDTH), px(height)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            kind: WindowKind::Floating,
            focus: true,
            show: true,
            is_resizable: false,
            is_minimizable: false,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        move |window, cx| {
            window.set_window_title("FrameEver");
            cx.new(|cx| PermissionHelp::new(need, settings_only, window, cx))
        },
    );
    match opened {
        Ok(handle) => {
            cx.set_global(HelpGlobal {
                handle: Some(handle),
                reveal_main_on_close,
            });
            cx.activate(true);
        }
        Err(error) => {
            tracing::warn!("permission help failed to open: {error:#}");
            if reveal_main_on_close {
                crate::app_windows::show_main_window(cx);
            }
        }
    }
}

#[cfg(debug_assertions)]
pub fn preview_from_env(cx: &mut App) {
    let Ok(spec) = std::env::var("FRAMEEVER_GPUI_PERMISSION_HELP_PREVIEW") else {
        return;
    };
    let need = match spec.split_once(':') {
        Some(("window", app)) => Need::WindowControl {
            app: Some(app.to_string()),
        },
        None if spec == "window" => Need::WindowControl { app: None },
        None if spec == "keystrokes" => Need::Keystrokes,
        _ => return,
    };
    show(need, cx);
}

pub struct PermissionHelp {
    theme: Theme,
    need: Need,
    permission: OSPermission,
    settings_only: bool,
    focus: FocusHandle,
    shown_at: Instant,
    opened_settings: bool,
    granted_at: Option<Instant>,
    needs_relaunch: bool,
    held_at_open: bool,
    poll: Option<Task<()>>,
}

async fn come_back_after_settings(cx: &mut gpui::AsyncWindowContext) {
    const TICK: Duration = Duration::from_millis(400);
    const GIVE_UP: Duration = Duration::from_secs(10 * 60);
    let started = Instant::now();
    let mut seen = false;
    while started.elapsed() < GIVE_UP {
        cx.background_executor().timer(TICK).await;
        let front = cx
            .background_executor()
            .spawn(async { crate::platform::system_settings_frontmost() })
            .await;
        if front {
            seen = true;
        } else if seen {
            cx.update(|window, cx| {
                window.activate_window();
                cx.activate(true);
            })
            .ok();
            return;
        }
    }
}

fn watches_grant(need: &Need, held_at_open: bool) -> bool {
    !(matches!(need, Need::WindowControl { .. }) && held_at_open)
}

impl PermissionHelp {
    fn new(need: Need, settings_only: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::theme::bind_window(window, cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        tracing::info!(?need, settings_only, "permission help: shown");
        let needs_relaunch = need == Need::Keystrokes && permissions::keystrokes_need_relaunch();
        let held_at_open = permissions::granted_now(need.permission());
        Self {
            theme: Theme::for_window(window, cx, true),
            permission: need.permission(),
            need,
            settings_only,
            focus,
            shown_at: Instant::now(),
            opened_settings: false,
            granted_at: needs_relaunch.then(Instant::now),
            needs_relaunch,
            held_at_open,
            poll: None,
        }
    }

    fn retarget(&mut self, need: Need, _window: &mut Window, cx: &mut Context<Self>) {
        self.permission = need.permission();
        self.need = need;
        self.shown_at = Instant::now();
        self.opened_settings = false;
        self.needs_relaunch =
            self.need == Need::Keystrokes && permissions::keystrokes_need_relaunch();
        self.granted_at = self.needs_relaunch.then(Instant::now);
        self.held_at_open = permissions::granted_now(self.permission);
        self.poll = None;
        cx.notify();
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let permission = self.permission;
        tracing::info!(?permission, "permission help: opening System Settings");
        self.opened_settings = true;
        permissions::open_permission_settings(permission);
        cx.defer(move |cx| crate::permission_guide::start(permission, cx));
        self.arm_poll(window, cx);
        cx.notify();
    }

    fn arm_poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.poll.is_some() {
            return;
        }
        if !watches_grant(&self.need, self.held_at_open) {
            self.poll = Some(cx.spawn_in(window, async move |this, cx| {
                come_back_after_settings(cx).await;
                this.update(cx, |this, _| this.poll = None).ok();
            }));
            return;
        }
        let permission = self.permission;
        self.poll = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let granted = cx
                    .background_executor()
                    .spawn(async move { permissions::granted_now(permission) })
                    .await;
                if granted {
                    break;
                }
            }
            tracing::info!(?permission, "permission help: granted");
            INVALIDATED.store(true, std::sync::atomic::Ordering::Release);
            let needs_relaunch = permission == OSPermission::InputMonitoring;
            this.update_in(cx, |this, window, cx| {
                this.granted_at = Some(Instant::now());
                this.needs_relaunch = needs_relaunch;
                crate::permission_guide::stop(cx);
                window.activate_window();
                cx.activate(true);
                cx.refresh_windows();
                cx.notify();
            })
            .ok();
            if needs_relaunch {
                return;
            }
            cx.background_executor().timer(CLOSE_AFTER_GRANT).await;
            this.update_in(cx, |this, window, cx| this.close(window, cx))
                .ok();
        }));
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.poll = None;
        if self.granted_at.is_none() {
            crate::permission_guide::stop(cx);
        }
        window.remove_window();
        cx.defer(closed);
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => self.close(window, cx),
            "enter" if self.needs_relaunch => permissions::relaunch(cx),
            "enter" if self.granted_at.is_some() || self.awaiting_user() => self.close(window, cx),
            "enter" => self.open_settings(window, cx),
            _ => {}
        }
    }

    fn awaiting_user(&self) -> bool {
        self.opened_settings && !watches_grant(&self.need, self.held_at_open)
    }

    fn copy(&self) -> (&'static Doodle, &'static str, &'static str, &'static str) {
        match self.need {
            Need::Keystrokes => (
                &art::KEYBOARD,
                t!("Show your keystrokes"),
                t!(
                    "macOS needs your OK before FrameEver can see the keys you press while recording."
                ),
                t!("Your next recording will include the keys you press."),
            ),
            Need::WindowControl { .. } => (
                &art::CURSOR,
                t!("Let FrameEver resize windows"),
                t!("macOS needs your OK before FrameEver can move or resize another app's window."),
                t!("FrameEver can resize windows now."),
            ),
        }
    }

    fn render_step(&self, paper: &Paper, index: usize, text: String, done: bool) -> gpui::Div {
        let still = self.shown_at.elapsed() > ANIMATE_FOR;
        let badge = div()
            .flex_none()
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border(px(1.5))
            .border_color(if done { paper.green } else { paper.track })
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(paper.ink);
        let badge = if done {
            badge.child(doodle::doodle(
                &art::MARK_GRANTED,
                paper.palette(),
                self.shown_at,
                still,
                18.,
                18.,
            ))
        } else {
            badge.child(SharedString::from((index + 1).to_string()))
        };
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(12.))
            .child(badge)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pt(px(2.))
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(if done { paper.soft } else { paper.ink })
                    .child(text),
            )
    }
}

impl Render for PermissionHelp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = Theme::for_window(window, cx, true);
        let paper = Paper::new(&self.theme);
        let still = self.shown_at.elapsed() > ANIMATE_FOR;
        let (art, title, body, done_body) = self.copy();
        let granted = self.granted_at.is_some();

        let mut card = div()
            .id("permission-help")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .px(px(32.))
            .pt(px(26.))
            .pb(px(26.))
            .rounded(px(22.))
            .border(px(1.5))
            .border_color(paper.track)
            .bg(paper.bg)
            .font_family(crate::theme::ui_font())
            .when(granted, |card| card.child(div().flex_1()))
            .child(doodle::doodle(
                if granted { &art::ALL_SET } else { art },
                paper.palette(),
                self.granted_at.unwrap_or(self.shown_at),
                still && !granted,
                132.,
                114.,
            ))
            .child(
                div()
                    .mt(px(16.))
                    .text_size(px(21.))
                    .line_height(px(27.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(paper.ink)
                    .text_center()
                    .child(if self.needs_relaunch {
                        t!("One restart and you're set")
                    } else if granted {
                        t!("You're all set")
                    } else {
                        title
                    }),
            )
            .child(
                div()
                    .mt(px(6.))
                    .max_w(px(340.))
                    .text_size(px(14.))
                    .line_height(px(21.))
                    .text_color(paper.soft)
                    .text_center()
                    .child(if self.needs_relaunch {
                        t!("macOS turns on keystrokes once FrameEver restarts.")
                    } else if granted {
                        done_body
                    } else {
                        body
                    }),
            );

        if !granted {
            let steps = steps(&self.need, self.settings_only, self.held_at_open);
            card = card.child(
                div()
                    .mt(px(20.))
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .px(px(18.))
                    .py(px(16.))
                    .rounded(px(16.))
                    .bg(paper.card)
                    .border(px(1.))
                    .border_color(paper.track)
                    .children(steps.into_iter().enumerate().map(|(index, text)| {
                        self.render_step(&paper, index, text, index == 0 && self.opened_settings)
                    })),
            );
            if self.opened_settings && !self.awaiting_user() {
                card = card.child(
                    div()
                        .mt(px(14.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(4.))
                        .child(doodle::doodle(
                            &art::WAITING,
                            paper.palette(),
                            self.shown_at,
                            false,
                            150.,
                            16.,
                        ))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(paper.soft)
                                .child(t!("Waiting for System Settings")),
                        ),
                );
            }
        }

        let buttons = if self.needs_relaunch {
            div()
                .flex()
                .flex_row()
                .gap(px(10.))
                .child(paper_button(
                    &paper,
                    "permission-help-relaunch",
                    t!("Relaunch FrameEver"),
                    true,
                    true,
                    cx.listener(|_, _, _, cx| permissions::relaunch(cx)),
                ))
                .child(paper_button(
                    &paper,
                    "permission-help-later",
                    t!("Not now"),
                    false,
                    true,
                    cx.listener(|this, _, window, cx| this.close(window, cx)),
                ))
        } else if self.awaiting_user() {
            div()
                .flex()
                .flex_row()
                .gap(px(10.))
                .child(paper_button(
                    &paper,
                    "permission-help-done",
                    t!("Done"),
                    true,
                    true,
                    cx.listener(|this, _, window, cx| this.close(window, cx)),
                ))
                .child(paper_button(
                    &paper,
                    "permission-help-open",
                    t!("Open System Settings again"),
                    false,
                    true,
                    cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                ))
        } else if granted {
            div().child(paper_button(
                &paper,
                "permission-help-done",
                t!("Done"),
                true,
                true,
                cx.listener(|this, _, window, cx| this.close(window, cx)),
            ))
        } else {
            div()
                .flex()
                .flex_row()
                .gap(px(10.))
                .child(paper_button(
                    &paper,
                    "permission-help-open",
                    if self.opened_settings {
                        t!("Open System Settings again")
                    } else {
                        t!("Open System Settings")
                    },
                    true,
                    true,
                    cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                ))
                .child(paper_button(
                    &paper,
                    "permission-help-later",
                    t!("Not now"),
                    false,
                    true,
                    cx.listener(|this, _, window, cx| this.close(window, cx)),
                ))
        };
        card.child(div().flex_1()).child(buttons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keystroke_steps_follow_the_macos_version() {
        let plus = steps(&Need::Keystrokes, true, false);
        assert_eq!(
            plus,
            vec![
                "Click Open System Settings below.",
                "Click + under Input Monitoring.",
                "Choose FrameEver and enter your Mac password.",
            ]
        );
        let before = steps(&Need::Keystrokes, false, false);
        assert_eq!(
            before,
            vec![
                "Click Open System Settings below.",
                "Find FrameEver under Accessibility and switch it on.",
            ]
        );
    }

    #[test]
    fn window_steps_name_the_app_when_known() {
        let named = steps(
            &Need::WindowControl {
                app: Some("Safari".into()),
            },
            true,
            false,
        );
        assert_eq!(named.len(), 4);
        assert_eq!(named[2], "Pick Choose Apps, then add Safari.");
        assert_eq!(
            named[3],
            "Set it to At Any Time so macOS doesn't ask again every day."
        );

        let unnamed = steps(&Need::WindowControl { app: None }, true, false);
        assert_eq!(
            unnamed[2],
            "Pick Choose Apps, then add the app you're recording."
        );

        let before = steps(&Need::WindowControl { app: None }, false, false);
        assert_eq!(before.len(), 2);
    }

    #[test]
    fn a_held_grant_starts_from_the_existing_row() {
        let held = steps(
            &Need::WindowControl {
                app: Some("Calculator".into()),
            },
            true,
            true,
        );
        assert_eq!(
            held,
            vec![
                "Click Open System Settings below.",
                "Click ⓘ next to FrameEver.",
                "Click + and add Calculator.",
                "Set it to At Any Time so macOS doesn't ask again every day.",
            ]
        );
        assert_eq!(steps(&Need::Keystrokes, true, true).len(), 3);
    }

    #[test]
    fn a_held_grant_leaves_the_window_choice_to_the_user() {
        let window = Need::WindowControl { app: None };
        assert!(!watches_grant(&window, true));
        assert!(watches_grant(&window, false));
        assert!(watches_grant(&Need::Keystrokes, true));
        assert!(watches_grant(&Need::Keystrokes, false));
    }

    #[test]
    fn window_control_always_means_accessibility() {
        assert_eq!(
            Need::WindowControl { app: None }.permission(),
            OSPermission::Accessibility
        );
    }
}
