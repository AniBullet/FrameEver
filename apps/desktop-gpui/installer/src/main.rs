#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod runner;
mod system;
mod taskbar;

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use frameever_installer_app::{
    command_line::{Fallback, Mode, fallback, parse, relaunch_arguments, uninstaller_passthrough},
    flow::{
        self, Action, Button, Check, EXIT_BUSY, EXIT_FAILED, Effect, Flow, Frame, Installed, Job,
        Link, Screen, Setup, Silent, Style,
    },
    i18n::{self, tr},
    motion, payload,
    settings::language_setting,
    theme::Palette,
};
use frameever_utils::{brand::APP_NAME, storage};
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, AssetSource, Bounds, Context,
    FocusHandle, FontWeight, Image, ImageFormat, InteractiveElement, IntoElement, KeyBinding,
    KeyDownEvent, ParentElement, PathPromptOptions, Render, Rgba, SharedString,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowKind, WindowOptions, div, img, prelude::FluentBuilder,
    px, relative, size, svg, transparent_black,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const WIDTH: f32 = 440.;
const HEIGHT: f32 = 400.;
const TALL_HEIGHT: f32 = 600.;
const STRIP_WIDTH: f32 = 400.;
const CAPTION_HEIGHT: f32 = 32.;
const BODY_TEXT: f32 = 13.5;
const SMALL_TEXT: f32 = 12.;
const HEADING_TEXT: f32 = 20.;
const RADIUS_XS: f32 = 4.;
const RADIUS_SM: f32 = 8.;
const RADIUS_MD: f32 = 12.;
const POLL: Duration = Duration::from_millis(50);
const MONOSPACE: &str = "Consolas";
const KEY_CONTEXT: &str = "Setup";
const FALLBACK_ENV: &str = "FRAMEEVER_INSTALLER_FALLBACK";
const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/caption-minimize-windows.svg",
        include_bytes!("../../assets/icons/caption-minimize-windows.svg"),
    ),
    (
        "icons/caption-close-windows.svg",
        include_bytes!("../../assets/icons/caption-close-windows.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../../assets/icons/check.svg"),
    ),
    (
        "icons/chevron-down.svg",
        include_bytes!("../../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-up.svg",
        include_bytes!("../../assets/icons/chevron-up.svg"),
    ),
    (
        "icons/shield-check.svg",
        include_bytes!("../../assets/icons/shield-check.svg"),
    ),
];
const APP_ICON: &[u8] = include_bytes!("../../icons/128x128@2x.png");
const FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
];
const CHINESE_FONT: &[u8] =
    include_bytes!("../../assets/fonts/harmonyos-sans/HarmonyOS_Sans_SC.ttf");

static WINDOW_SHOWN: AtomicBool = AtomicBool::new(false);

gpui::actions!(setup, [FocusNext, FocusPrevious]);

#[derive(Default)]
struct Running {
    fraction: Option<f64>,
    outcome: Option<Result<(), String>>,
}

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_appearance(window.appearance());
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(RADIUS_XS))
            .bg(palette.popover)
            .border_1()
            .border_color(palette.border)
            .text_color(palette.popover_foreground)
            .text_size(px(SMALL_TEXT))
            .font_family(i18n::ui_font())
            .child(self.0.clone())
    }
}

struct SetupWindow {
    flow: Flow,
    app_data: PathBuf,
    focus: FocusHandle,
    primary: FocusHandle,
    icon: Arc<Image>,
    taskbar: Option<taskbar::Taskbar>,
    page: (flow::Page, bool, bool),
    epoch: usize,
    bar: (f32, f32),
    temp_copy: bool,
}

fn native_handle(window: &Window) -> Option<isize> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    Some(handle.hwnd.get())
}

fn window_size(screen: &Screen) -> gpui::Size<gpui::Pixels> {
    match &screen.frame {
        Frame::Main { tall, .. } => size(px(WIDTH), px(if *tall { TALL_HEIGHT } else { HEIGHT })),
        Frame::Strip => {
            let mut height = 14. + 40. + 14.;
            if screen.progress.is_some() {
                height += 10. + 4.;
            } else {
                height += 10. + 32.;
            }
            if screen.details.is_some() {
                height += 10. + 72.;
            }
            size(px(STRIP_WIDTH), px(height))
        }
    }
}

impl SetupWindow {
    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.flow.act(action);
        self.apply(effect, window, cx);
    }

    fn apply(&mut self, effect: Option<Effect>, window: &mut Window, cx: &mut Context<Self>) {
        match effect {
            Some(Effect::PickLocation(current)) => self.pick_location(current, window, cx),
            Some(Effect::Run(job)) => self.run(job, window, cx),
            Some(Effect::CloseApp) => self.close_app(window, cx),
            Some(Effect::Launch(executable)) => {
                let launched = runner::launch(&executable);
                runner::log(format_args!(
                    "launch {}: {launched:?}",
                    executable.display()
                ));
                let next = self.flow.launched(launched);
                self.apply(next, window, cx);
            }
            Some(Effect::OpenUrl(url)) => cx.open_url(&url),
            Some(Effect::OpenFolder(folder)) => cx.open_with_system(&folder),
            Some(Effect::ExportLogs) => self.export_logs(cx),
            Some(Effect::Quit) => {
                if self.temp_copy {
                    runner::remove_temp_copy();
                }
                cx.quit();
            }
            None => {}
        }
        self.sync(window, cx);
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let screen = self.flow.screen();
        let tall = matches!(screen.frame, Frame::Main { tall: true, .. });
        let key = (self.flow.page(), tall, screen.details.is_some());
        if key != self.page {
            let page_changed = key.0 != self.page.0;
            self.page = key;
            window.resize(window_size(&screen));
            if page_changed {
                self.epoch += 1;
                self.bar = (0., 0.);
                window.focus(&self.primary, cx);
            }
        }
        let percent = screen
            .progress
            .as_ref()
            .map_or(0., |it| f32::from(it.percent));
        if percent != self.bar.1 {
            self.bar = (self.bar.1, percent);
        }
        if let Some(taskbar) = &mut self.taskbar {
            taskbar.show(if screen.failed {
                taskbar::State::Failed
            } else if let Some(progress) = &screen.progress {
                taskbar::State::Running(progress.percent)
            } else {
                taskbar::State::Off
            });
        }
        cx.notify();
    }

    fn pick_location(&mut self, current: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        runner::log(format_args!("pick location from {}", current.display()));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                this.flow.location_picked(&folder);
                this.sync(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn export_logs(&mut self, cx: &mut Context<Self>) {
        let start = std::env::var_os("USERPROFILE")
            .map(|home| PathBuf::from(home).join("Desktop"))
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let target = cx.prompt_for_new_path(&start, Some("frameever-setup.log"));
        cx.spawn(async move |_, _| {
            let Ok(Ok(Some(target))) = target.await else {
                return;
            };
            if let Err(error) = runner::export_log(&target) {
                runner::log(format_args!("export logs: {error}"));
            }
        })
        .detach();
    }

    fn close_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app_data = self.app_data.clone();
        let dir = self.flow.location().to_path_buf();
        let closing = cx
            .background_executor()
            .spawn(async move { system::close_app(&app_data, &dir) });
        cx.spawn_in(window, async move |this, cx| {
            let closed = closing.await;
            runner::log(format_args!("close FrameEver: {closed:?}"));
            this.update_in(cx, |this, window, cx| {
                let next = this.flow.app_closed(closed);
                this.apply(next, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn run(&mut self, job: Job, window: &mut Window, cx: &mut Context<Self>) {
        let shared = Arc::new(Mutex::new(Running::default()));
        let writer = shared.clone();
        let installing = matches!(job, Job::Install { .. });
        std::thread::spawn(move || {
            let report = |fraction: f64| {
                if let Ok(mut running) = writer.lock() {
                    running.fraction = Some(fraction);
                }
            };
            let mut report = report;
            let outcome = match &job {
                Job::Install { .. } => runner::install(&job, payload::INSTALLED_BYTES, &mut report),
                Job::Uninstall { .. } => runner::uninstall(&job, &mut report),
            };
            runner::log(format_args!("job {job:?}: {outcome:?}"));
            if let Ok(mut running) = writer.lock() {
                running.outcome = Some(outcome);
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let (fraction, outcome) = match shared.lock() {
                    Ok(mut running) => (running.fraction.take(), running.outcome.take()),
                    Err(_) => return,
                };
                let done = outcome.is_some();
                let updated = this.update_in(cx, |this, window, cx| {
                    if let Some(fraction) = fraction {
                        this.flow.progress(fraction);
                    }
                    match outcome {
                        Some(outcome) => {
                            if installing && outcome.is_ok() {
                                runner::write_first_language(i18n::current().code());
                            }
                            let next = this.flow.finished(outcome);
                            this.apply(next, window, cx);
                        }
                        None => this.sync(window, cx),
                    }
                });
                if done || updated.is_err() {
                    return;
                }
            }
        })
        .detach();
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.modified() {
            return;
        }
        let action = match keystroke.key.as_str() {
            "escape" => self.flow.escape(),
            "enter" if self.focus.is_focused(window) || window.focused(cx).is_none() => {
                self.flow.primary()
            }
            _ => None,
        };
        if let Some(action) = action {
            self.act(action, window, cx);
        }
    }
}

fn focus_ring<E: InteractiveElement + Styled>(element: E, palette: Palette) -> E {
    element
        .border_1()
        .border_color(transparent_black())
        .focus_visible(move |style| style.border_color(palette.ring))
}

fn activate_keys<E: InteractiveElement>(
    element: E,
    action: Action,
    cx: &mut Context<SetupWindow>,
) -> E {
    element.on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
        let key = event.keystroke.key.as_str();
        if !event.keystroke.modifiers.modified() && matches!(key, "enter" | "space") {
            cx.stop_propagation();
            this.act(action, window, cx);
        }
    }))
}

fn button(
    button: Button,
    tall: bool,
    grow: bool,
    primary_focus: Option<&FocusHandle>,
    palette: Palette,
    cx: &mut Context<SetupWindow>,
) -> AnyElement {
    let action = button.action;
    let (bg, border, text, hover, active) = match button.style {
        Style::Primary => (
            palette.primary,
            palette.primary,
            palette.primary_foreground,
            palette.primary_hover,
            palette.primary_active,
        ),
        Style::Danger => (
            palette.danger,
            palette.danger,
            palette.danger_foreground,
            palette.danger_hover,
            palette.danger_hover,
        ),
        Style::Secondary => (
            palette.secondary,
            palette.border,
            palette.secondary_foreground,
            palette.secondary_hover,
            palette.secondary_active,
        ),
    };
    let element = div()
        .id(SharedString::from(format!("button-{action:?}")))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .when(grow, |this| this.flex_1())
        .flex_shrink_0()
        .h(px(if tall { 40. } else { 32. }))
        .px(px(14.))
        .rounded(px(if tall { RADIUS_MD } else { RADIUS_SM }))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(text)
        .text_size(px(BODY_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .tab_index(0)
        .hover(move |style| style.bg(hover))
        .active(move |style| style.bg(active))
        .focus_visible(move |style| style.border_color(palette.ring).border_2())
        .child(button.label)
        .on_click(cx.listener(move |this, _, window, cx| this.act(action, window, cx)));
    let element = activate_keys(element, action, cx);
    match primary_focus {
        Some(focus) if matches!(button.style, Style::Primary | Style::Danger) => {
            element.track_focus(focus).into_any_element()
        }
        _ => element.into_any_element(),
    }
}

fn link(
    link: Link,
    focus: Option<&FocusHandle>,
    palette: Palette,
    cx: &mut Context<SetupWindow>,
) -> AnyElement {
    let action = link.action;
    let element = focus_ring(
        div().id(SharedString::from(format!("link-{action:?}"))),
        palette,
    )
    .flex()
    .flex_row()
    .items_center()
    .gap(px(4.))
    .h(px(28.))
    .px(px(8.))
    .rounded(px(RADIUS_SM))
    .text_size(px(SMALL_TEXT))
    .text_color(palette.muted_foreground)
    .cursor_pointer()
    .tab_index(0)
    .hover(move |style| style.bg(palette.accent).text_color(palette.foreground))
    .child(link.label)
    .children(link.open.map(|open| {
        svg()
            .path(if open {
                "icons/chevron-up.svg"
            } else {
                "icons/chevron-down.svg"
            })
            .size(px(14.))
            .text_color(palette.muted_foreground)
    }))
    .on_click(cx.listener(move |this, _, window, cx| this.act(action, window, cx)));
    let element = activate_keys(element, action, cx);
    match focus {
        Some(focus) => element.track_focus(focus).into_any_element(),
        None => element.into_any_element(),
    }
}

fn check_row(check: Check, palette: Palette, cx: &mut Context<SetupWindow>) -> AnyElement {
    let action = Action::Toggle(check.choice);
    let on = check.on;
    let element = focus_ring(
        div().id(SharedString::from(format!("check-{:?}", check.choice))),
        palette,
    )
    .flex()
    .flex_row()
    .items_start()
    .gap(px(10.))
    .w_full()
    .px(px(4.))
    .py(px(2.))
    .rounded(px(RADIUS_SM))
    .cursor_pointer()
    .tab_index(0)
    .child(
        div()
            .mt(px(2.))
            .flex_shrink_0()
            .size(px(16.))
            .rounded(px(RADIUS_XS))
            .border_1()
            .flex()
            .items_center()
            .justify_center()
            .map(|this| {
                if on {
                    this.bg(palette.primary).border_color(palette.primary)
                } else {
                    this.bg(palette.background).border_color(palette.input)
                }
            })
            .when(on, |this| {
                this.child(
                    svg()
                        .path("icons/check.svg")
                        .size(px(12.))
                        .text_color(palette.primary_foreground),
                )
            }),
    )
    .child(
        div()
            .flex()
            .flex_col()
            .text_left()
            .child(check.label)
            .children(check.note.map(|note| {
                div()
                    .text_size(px(SMALL_TEXT))
                    .line_height(px(16.))
                    .text_color(palette.muted_foreground)
                    .child(note)
            })),
    )
    .on_click(cx.listener(move |this, _, window, cx| this.act(action, window, cx)));
    activate_keys(element, action, cx).into_any_element()
}

fn details_box(text: String, palette: Palette) -> impl IntoElement {
    div()
        .w_full()
        .px(px(12.))
        .py(px(10.))
        .rounded(px(RADIUS_MD))
        .bg(palette.card)
        .font_family(MONOSPACE)
        .text_size(px(11.5))
        .line_height(px(17.))
        .text_color(palette.muted_foreground)
        .text_left()
        .child(text)
}

fn caption(
    title: String,
    icon: Arc<Image>,
    locked: bool,
    palette: Palette,
    active: bool,
    cx: &mut Context<SetupWindow>,
) -> impl IntoElement + use<> {
    let glyph: Rgba = if active {
        palette.foreground
    } else {
        palette.muted_foreground
    };
    let control =
        move |id: &'static str, path: &'static str, height: f32, hover: Rgba, off: bool| {
            div()
                .id(id)
                .group(id)
                .w(px(46.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .occlude()
                .when(!off, |this| this.hover(move |style| style.bg(hover)))
                .child(
                    svg()
                        .path(path)
                        .w(px(10.))
                        .h(px(height))
                        .text_color(if off { palette.faint_foreground } else { glyph })
                        .when(id == "caption-close" && !off, |this| {
                            this.group_hover(id, |style| style.text_color(gpui::white()))
                        }),
                )
        };
    let tip: SharedString = tr("Can't be closed while files are being copied").into();
    div()
        .h(px(CAPTION_HEIGHT))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .pl(px(10.))
                .window_control_area(WindowControlArea::Drag)
                .child(img(icon).size(px(16.)).rounded(px(4.)))
                .child(
                    div()
                        .text_size(px(SMALL_TEXT))
                        .text_color(palette.muted_foreground)
                        .child(title),
                ),
        )
        .child(
            control(
                "caption-minimize",
                "icons/caption-minimize-windows.svg",
                1.,
                palette.accent,
                false,
            )
            .on_click(|_, window, _| window.minimize_window()),
        )
        .child(
            control(
                "caption-close",
                "icons/caption-close-windows.svg",
                10.,
                palette.danger,
                locked,
            )
            .when(locked, move |this| {
                let tip = tip.clone();
                this.tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            })
            .on_click(cx.listener(|this, _, window, cx| this.act(Action::Close, window, cx))),
        )
}

fn progress_bar(
    label: String,
    percent: u8,
    bar: (f32, f32),
    epoch: usize,
    palette: Palette,
) -> impl IntoElement {
    let (from, to) = bar;
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .gap(px(12.))
                .text_size(px(SMALL_TEXT))
                .text_color(palette.muted_foreground)
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.foreground)
                        .child(label),
                )
                .child(format!("{percent}%")),
        )
        .child(
            div()
                .h(px(4.))
                .w_full()
                .rounded(px(2.))
                .bg(palette.component.slider_bar)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .bg(palette.component.progress_bar)
                        .with_animation(
                            SharedString::from(format!("bar-{epoch}-{to}")),
                            Animation::new(motion::MOVE).with_easing(motion::EASE_EFFECTS.curve()),
                            move |this, delta| {
                                this.w(relative((from + (to - from) * delta) / 100.))
                            },
                        ),
                ),
        )
}

impl SetupWindow {
    fn render_main(
        &mut self,
        screen: Screen,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_buttons = !screen.buttons.is_empty();
        let buttons: Vec<_> = screen
            .buttons
            .into_iter()
            .map(|it| button(it, true, true, Some(&self.primary), palette, cx))
            .collect();
        let link_focus = (!has_buttons).then_some(&self.primary);
        let links: Vec<_> = screen
            .links
            .into_iter()
            .map(|it| link(it, link_focus, palette, cx))
            .collect();
        let checks: Vec<_> = screen
            .checks
            .into_iter()
            .map(|it| check_row(it, palette, cx))
            .collect();
        let location = screen.location.map(|location| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .w_full()
                .text_left()
                .child(
                    div()
                        .text_size(px(SMALL_TEXT))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.muted_foreground)
                        .child(location.label),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h(px(32.))
                                .px(px(10.))
                                .flex()
                                .items_center()
                                .rounded(px(RADIUS_SM))
                                .border_1()
                                .border_color(palette.input)
                                .bg(palette.background)
                                .text_size(px(SMALL_TEXT))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(location.path),
                        )
                        .child(button(location.change, false, false, None, palette, cx)),
                )
                .child(
                    div()
                        .text_size(px(SMALL_TEXT))
                        .line_height(px(16.))
                        .text_color(palette.muted_foreground)
                        .child(location.space),
                )
        });
        let keep = screen.keep.map(|text| {
            div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .w_full()
                .px(px(12.))
                .py(px(10.))
                .rounded(px(RADIUS_MD))
                .bg(palette.card)
                .text_size(px(SMALL_TEXT))
                .line_height(px(16.))
                .text_left()
                .child(
                    svg()
                        .path("icons/shield-check.svg")
                        .flex_shrink_0()
                        .mt(px(1.))
                        .size(px(14.))
                        .text_color(palette.success_text),
                )
                .child(div().flex_1().min_w_0().child(text))
        });
        let has_slot =
            location.is_some() || keep.is_some() || !checks.is_empty() || screen.details.is_some();
        let bottom = match screen.progress {
            Some(progress) => div()
                .w_full()
                .h(px(40.))
                .flex()
                .flex_col()
                .justify_end()
                .child(progress_bar(
                    progress.label,
                    progress.percent,
                    self.bar,
                    self.epoch,
                    palette,
                ))
                .into_any_element(),
            None => div()
                .w_full()
                .flex()
                .flex_row()
                .gap(px(8.))
                .children(buttons)
                .into_any_element(),
        };
        div()
            .id(SharedString::from(format!("page-{}", self.epoch)))
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .px(px(28.))
            .pt(px(20.))
            .pb(px(20.))
            .text_center()
            .child(
                img(self.icon.clone())
                    .size(px(if screen.hero { 72. } else { 40. }))
                    .rounded(px(if screen.hero { 16. } else { 9. })),
            )
            .child(
                div()
                    .mt(px(12.))
                    .text_size(px(HEADING_TEXT))
                    .line_height(px(26.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(screen.heading),
            )
            .when(!screen.sentence.is_empty(), |this| {
                this.child(
                    div()
                        .mt(px(4.))
                        .text_color(palette.muted_foreground)
                        .child(screen.sentence),
                )
            })
            .when(has_slot, |this| {
                this.child(
                    div()
                        .mt(px(16.))
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .children(screen.details.map(|text| details_box(text, palette)))
                        .children(location)
                        .children(keep)
                        .children(checks),
                )
            })
            .child(
                div()
                    .mt_auto()
                    .pt(px(16.))
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .child(bottom)
                    .children(links),
            )
            .with_animation(
                SharedString::from(format!("fade-{}", self.epoch)),
                Animation::new(motion::MID).with_easing(motion::EASE_EFFECTS.curve()),
                |this, delta| this.opacity(delta),
            )
            .into_any_element()
    }

    fn render_strip(
        &mut self,
        screen: Screen,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let buttons: Vec<_> = screen
            .buttons
            .into_iter()
            .map(|it| button(it, false, false, Some(&self.primary), palette, cx))
            .collect();
        let links: Vec<_> = screen
            .links
            .into_iter()
            .map(|it| link(it, None, palette, cx))
            .collect();
        let percent = screen.progress.as_ref().map(|it| it.percent);
        div()
            .id(SharedString::from(format!("strip-{}", self.epoch)))
            .size_full()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(16.))
            .py(px(14.))
            .border_1()
            .border_color(palette.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child(img(self.icon.clone()).size(px(32.)).rounded(px(7.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(FontWeight::MEDIUM).child(screen.heading))
                            .child(
                                div()
                                    .text_size(px(SMALL_TEXT))
                                    .line_height(px(16.))
                                    .text_color(palette.muted_foreground)
                                    .child(screen.meta.unwrap_or(screen.sentence)),
                            ),
                    )
                    .children(percent.map(|percent| {
                        div()
                            .text_size(px(SMALL_TEXT))
                            .text_color(palette.muted_foreground)
                            .child(format!("{percent}%"))
                    })),
            )
            .children(screen.details.map(|text| details_box(text, palette)))
            .map(|this| match screen.progress {
                Some(_) => this.child(
                    div()
                        .h(px(4.))
                        .w_full()
                        .rounded(px(2.))
                        .bg(palette.component.slider_bar)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .bg(palette.component.progress_bar)
                                .w(relative(self.bar.1 / 100.)),
                        ),
                ),
                None => this.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .children(links)
                        .child(div().flex_1())
                        .children(buttons),
                ),
            })
            .into_any_element()
    }
}

impl Render for SetupWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_appearance(window.appearance());
        let screen = self.flow.screen();
        let root = div()
            .id("setup")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.background)
            .text_color(palette.foreground)
            .text_size(px(BODY_TEXT))
            .line_height(px(20.))
            .font_family(i18n::ui_font());
        match screen.frame.clone() {
            Frame::Main { title, .. } => {
                let locked = screen.locked;
                let active = window.is_window_active();
                let caption = caption(title, self.icon.clone(), locked, palette, active, cx);
                let page = self.render_main(screen, palette, cx);
                root.child(caption).child(page)
            }
            Frame::Strip => {
                let strip = self.render_strip(screen, palette, cx);
                root.child(strip)
            }
        }
    }
}

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ICONS.iter().map(|(name, _)| (*name).into()).collect())
    }
}

fn run_fallback(setup: &Setup) -> i32 {
    let installed = setup.installed.as_ref().map(|it| it.dir.as_path());
    runner::log(format_args!("fallback for {:?}", setup.launch.mode));
    match fallback(&setup.launch, installed) {
        Some(Fallback::Installer(arguments)) => match runner::run_nsis(&arguments) {
            Ok(()) => 0,
            Err(error) => {
                runner::log(format_args!("fallback installer: {error}"));
                EXIT_FAILED
            }
        },
        Some(Fallback::Uninstaller(uninstaller)) => {
            match std::process::Command::new(&uninstaller).spawn() {
                Ok(_) => 0,
                Err(error) => {
                    runner::log(format_args!("fallback uninstaller: {error}"));
                    EXIT_FAILED
                }
            }
        }
        None => EXIT_FAILED,
    }
}

fn run_silent(setup: &Setup, app_data: &Path) -> i32 {
    let system = system::this_machine(app_data.to_path_buf());
    let job = match flow::silent(setup, &system) {
        Silent::Stop(code) => {
            runner::log(format_args!("silent run stopped with {code}"));
            return code;
        }
        Silent::Run(job) => job,
        Silent::CloseAppFirst(job) => {
            let dir = match &job {
                Job::Install { dir, .. } | Job::Uninstall { dir, .. } => dir.clone(),
            };
            match system::close_app(app_data, &dir) {
                flow::Closed::Gone => job,
                closed => {
                    runner::log(format_args!("silent run: FrameEver {closed:?}"));
                    return EXIT_BUSY;
                }
            }
        }
    };
    let outcome = match &job {
        Job::Install { .. } => runner::install(&job, payload::INSTALLED_BYTES, &mut |_| {}),
        Job::Uninstall { .. } => runner::uninstall(&job, &mut |_| {}),
    };
    runner::log(format_args!("silent {job:?}: {outcome:?}"));
    if setup.launch.mode == Mode::Uninstall {
        runner::remove_temp_copy();
    }
    if outcome.is_ok() { 0 } else { EXIT_FAILED }
}

fn open_window(setup: Setup, app_data: PathBuf, cx: &mut App) -> Result<(), String> {
    let update = setup.launch.mode == Mode::Update;
    let temp_copy = setup.launch.mode == Mode::Uninstall && setup.launch.from_temp;
    let flow = Flow::new(
        setup,
        system::this_machine(app_data.clone()),
        i18n::current(),
    );
    let first = flow.screen();
    let bounds = Bounds::centered(None, window_size(&first), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: (!update).then(|| TitlebarOptions {
            title: Some(tr("Install FrameEver").into()),
            appears_transparent: true,
            ..Default::default()
        }),
        focus: !update,
        show: true,
        kind: if update {
            WindowKind::PopUp
        } else {
            WindowKind::Normal
        },
        is_movable: !update,
        window_background: WindowBackgroundAppearance::Opaque,
        is_resizable: false,
        is_minimizable: !update,
        ..Default::default()
    };
    cx.open_window(options, move |window, cx| {
        let view = cx.new(|cx| {
            cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                .detach();
            let focus = cx.focus_handle();
            let primary = cx.focus_handle();
            window.focus(&primary, cx);
            let taskbar = native_handle(window).and_then(taskbar::Taskbar::new);
            let page = (flow.page(), false, false);
            SetupWindow {
                flow,
                app_data,
                focus,
                primary,
                icon: Arc::new(Image::from_bytes(ImageFormat::Png, APP_ICON.to_vec())),
                taskbar,
                page,
                epoch: 0,
                bar: (0., 0.),
                temp_copy,
            }
        });
        let watched = view.downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            watched
                .upgrade()
                .is_none_or(|view| !view.read(cx).flow.locked())
        });
        view.update(cx, |this, cx| {
            let effect = this.flow.start();
            this.apply(effect, window, cx);
        });
        WINDOW_SHOWN.store(true, Ordering::SeqCst);
        view
    })
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn installed_for(dir: Option<&Path>) -> Option<Installed> {
    system::installed().or_else(|| {
        let dir = dir?;
        dir.join(format!("{APP_NAME}.exe"))
            .is_file()
            .then(|| Installed {
                dir: dir.to_path_buf(),
                version: None,
            })
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some((uninstaller, arguments)) = uninstaller_passthrough(&args) {
        let outcome = runner::spawn_and_wait(&uninstaller, &arguments);
        runner::log(format_args!("nsis uninstaller passthrough: {outcome:?}"));
        std::process::exit(if outcome.is_ok() { 0 } else { EXIT_FAILED });
    }
    let launch = parse(args);
    runner::log(format_args!("start {launch:?}"));
    if launch.mode == Mode::Uninstall && !launch.from_temp {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        if let Some(dir) = dir
            && runner::relaunch_from_temp(&relaunch_arguments(&launch, &dir)).is_ok()
        {
            return;
        }
    }
    let app_data = storage::app_data_dir();
    let settings = std::fs::read_to_string(storage::settings_file()).ok();
    i18n::set(i18n::resolve(
        settings.as_deref().and_then(language_setting).as_deref(),
        i18n::system_language(),
    ));
    let installed = installed_for(launch.dir.as_deref());
    let setup = Setup {
        version: env!("CARGO_PKG_VERSION").to_string(),
        installed,
        default_dir: system::default_dir(),
        temp_dir: std::env::temp_dir(),
        storage_dir: runner::storage_folder(),
        installed_bytes: payload::INSTALLED_BYTES,
        payload_bytes: payload::NSIS.len() as u64,
        launch,
    };
    if setup.launch.silent {
        std::process::exit(run_silent(&setup, &app_data));
    }
    if std::env::var_os(FALLBACK_ENV).is_some_and(|value| value == "1") {
        std::process::exit(run_fallback(&setup));
    }
    let for_panic = setup.clone();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        runner::log(format_args!("panic: {info}"));
        if !WINDOW_SHOWN.load(Ordering::SeqCst) {
            std::process::exit(run_fallback(&for_panic));
        }
        default_hook(info);
    }));
    let for_failure = setup.clone();
    let app = gpui_platform::application().with_assets(Assets);
    app.run(move |cx: &mut App| {
        let mut fonts: Vec<Cow<'static, [u8]>> =
            FONTS.iter().map(|font| Cow::Borrowed(*font)).collect();
        if i18n::current() == i18n::Language::SimplifiedChinese {
            fonts.push(Cow::Borrowed(CHINESE_FONT));
        }
        if let Err(error) = cx.text_system().add_fonts(fonts) {
            runner::log(format_args!("fonts: {error:#}"));
        }
        cx.set_reduce_motion(system::reduce_motion());
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, Some(KEY_CONTEXT)),
            KeyBinding::new("shift-tab", FocusPrevious, Some(KEY_CONTEXT)),
        ]);
        if let Err(error) = open_window(setup, app_data, cx) {
            runner::log(format_args!("window: {error}"));
            std::process::exit(run_fallback(&for_failure));
        }
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
}
