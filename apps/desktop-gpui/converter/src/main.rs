#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{
    borrow::Cow,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use frameever_converter::{Progress, Report, Target};
use frameever_converter_app::{
    i18n, system,
    theme::Palette,
    wizard::{
        Action, Button, Details, Effect, Empty, Icon, Meter, Row, Screen, StepState, Style,
        Warning, Wizard,
    },
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, AssetSource, Bounds, Context,
    FocusHandle, FontWeight, Image, ImageFormat, InteractiveElement, IntoElement, KeyBinding,
    KeyDownEvent, ParentElement, PathBuilder, PathPromptOptions, Render, Rgba, SharedString,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowOptions, canvas, div, img, point,
    prelude::FluentBuilder, px, size, svg, transparent_black,
};
use lyon_tessellation::{LineCap, LineJoin, StrokeOptions};

const WINDOW_WIDTH: f32 = 560.;
const WINDOW_HEIGHT: f32 = 560.;
const CAPTION_HEIGHT: f32 = 40.;
const CONTROL_HEIGHT: f32 = 32.;
const BODY_TEXT: f32 = 13.5;
const SMALL_TEXT: f32 = 12.;
const HEADING_TEXT: f32 = 20.;
const RADIUS_XS: f32 = 4.;
const RADIUS_SM: f32 = 8.;
const RADIUS_MD: f32 = 12.;
const PROGRESS_POLL: Duration = Duration::from_millis(50);
const WINDOW_TITLE: &str = "FrameEver Converter";
const MONOSPACE: &str = "Consolas";
const KEY_CONTEXT: &str = "Converter";
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
        "icons/chevron-left.svg",
        include_bytes!("../../assets/icons/chevron-left.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../../assets/icons/folder.svg"),
    ),
    (
        "icons/folder-open.svg",
        include_bytes!("../../assets/icons/folder-open.svg"),
    ),
    (
        "icons/video.svg",
        include_bytes!("../../assets/icons/video.svg"),
    ),
    (
        "icons/image.svg",
        include_bytes!("../../assets/icons/image.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../../assets/icons/settings.svg"),
    ),
    (
        "icons/triangle-alert.svg",
        include_bytes!("../../assets/icons/triangle-alert.svg"),
    ),
];
const APP_ICON: &[u8] = include_bytes!("../../assets/dock-icon.png");
const FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
];

gpui::actions!(converter, [FocusNext, FocusPrevious]);

fn icon_path(icon: Icon) -> &'static str {
    match icon {
        Icon::Folder => "icons/folder.svg",
        Icon::FolderOpen => "icons/folder-open.svg",
        Icon::Video => "icons/video.svg",
        Icon::Image => "icons/image.svg",
        Icon::Settings => "icons/settings.svg",
        Icon::Alert => "icons/triangle-alert.svg",
        Icon::Back => "icons/chevron-left.svg",
    }
}

#[derive(Default)]
struct Running {
    progress: Option<Progress>,
    report: Option<Report>,
}

struct ConverterWindow {
    wizard: Wizard,
    icon: Arc<Image>,
    focus: FocusHandle,
}

fn current_step(screen: &Screen) -> Option<usize> {
    screen
        .steps
        .iter()
        .position(|(_, state)| *state == StepState::Current)
}

impl ConverterWindow {
    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let before = current_step(&self.wizard.screen());
        let effect = self.wizard.act(action);
        match effect {
            Some(Effect::PickSource) => self.pick_folder(false, cx),
            Some(Effect::PickDestination) => self.pick_folder(true, cx),
            Some(Effect::Run(job)) => self.run(job, cx),
            Some(Effect::Open(path)) => cx.open_with_system(&path),
            Some(Effect::Launch(path)) => {
                cx.open_with_system(&path);
                cx.quit();
            }
            Some(Effect::Quit) => cx.quit(),
            None => {}
        }
        if current_step(&self.wizard.screen()) != before {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn pick_folder(&mut self, destination: bool, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(folder): Option<PathBuf> = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                if destination {
                    this.wizard.change_destination(&folder);
                } else {
                    this.wizard.choose_folder(&folder);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn run(&mut self, job: frameever_converter_app::wizard::Job, cx: &mut Context<Self>) {
        let shared = Arc::new(Mutex::new(Running::default()));
        let writer = shared.clone();
        cx.background_executor()
            .spawn(async move {
                let report = job.run(|progress| {
                    if let Ok(mut running) = writer.lock() {
                        running.progress = Some(progress.clone());
                    }
                });
                if let Ok(mut running) = writer.lock() {
                    running.report = Some(report);
                }
            })
            .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PROGRESS_POLL).await;
                let (progress, report) = match shared.lock() {
                    Ok(mut running) => (running.progress.take(), running.report.take()),
                    Err(_) => return,
                };
                let finished = report.is_some();
                let updated = this.update(cx, |this, cx| {
                    if let Some(progress) = progress {
                        this.wizard.progress(progress);
                    }
                    if let Some(report) = report {
                        this.wizard.finish(report);
                    }
                    cx.notify();
                });
                if finished || updated.is_err() {
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
            "escape" => self.wizard.escape(),
            "enter" if self.focus.is_focused(window) || window.focused(cx).is_none() => {
                self.wizard.primary()
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

fn button_id(action: Action) -> SharedString {
    format!("button-{action:?}").into()
}

fn button(
    button: Button,
    small: bool,
    palette: Palette,
    cx: &mut Context<ConverterWindow>,
) -> AnyElement {
    let action = button.action;
    let (bg, border, text, hover, active) = match (button.enabled, button.style) {
        (false, _) => (
            palette.muted,
            palette.muted,
            palette.faint_foreground,
            palette.muted,
            palette.muted,
        ),
        (true, Style::Primary) => (
            palette.primary,
            palette.primary,
            palette.primary_foreground,
            palette.primary_hover,
            palette.primary_active,
        ),
        (true, Style::Secondary) => (
            palette.secondary,
            palette.border,
            palette.secondary_foreground,
            palette.secondary_hover,
            palette.secondary_active,
        ),
        (true, Style::Ghost) => (
            transparent_black().into(),
            transparent_black().into(),
            palette.foreground,
            palette.accent,
            palette.secondary_active,
        ),
    };
    let element = div()
        .id(button_id(action))
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(if small { 24. } else { CONTROL_HEIGHT }))
        .px(px(if small { 8. } else { 12. }))
        .rounded(px(if small { RADIUS_SM } else { RADIUS_MD }))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(text)
        .text_size(px(if small { SMALL_TEXT } else { BODY_TEXT }))
        .font_weight(FontWeight::MEDIUM)
        .children(
            button
                .icon
                .map(|icon| svg().path(icon_path(icon)).size(px(14.)).text_color(text)),
        )
        .child(button.label);
    if !button.enabled {
        return element.into_any_element();
    }
    element
        .tab_index(0)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .active(move |style| style.bg(active))
        .focus_visible(move |style| style.border_color(palette.ring))
        .on_click(cx.listener(move |this, _, window, cx| this.act(action, window, cx)))
        .into_any_element()
}

fn details_link(
    id: SharedString,
    details: Details,
    palette: Palette,
    cx: &mut Context<ConverterWindow>,
) -> impl IntoElement + use<> {
    let toggle = details.toggle;
    div()
        .flex()
        .flex_col()
        .items_start()
        .child(
            focus_ring(div().id(id), palette)
                .tab_index(0)
                .rounded(px(RADIUS_XS))
                .text_size(px(SMALL_TEXT))
                .text_color(palette.link)
                .cursor_pointer()
                .child(details.label)
                .on_click(cx.listener(move |this, _, window, cx| this.act(toggle, window, cx))),
        )
        .when(details.open, |this| {
            this.child(
                div()
                    .mt(px(4.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(RADIUS_XS))
                    .bg(palette.muted)
                    .font_family(MONOSPACE)
                    .text_size(px(11.))
                    .text_color(palette.muted_foreground)
                    .child(details.raw),
            )
        })
}

fn warning(
    index: usize,
    warning: Warning,
    palette: Palette,
    cx: &mut Context<ConverterWindow>,
) -> impl IntoElement + use<> {
    div()
        .id(("warning", index))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(RADIUS_SM))
        .bg(palette.warning_tint)
        .text_color(palette.warning_text)
        .text_size(px(SMALL_TEXT))
        .line_height(px(16.))
        .child(
            svg()
                .path(icon_path(Icon::Alert))
                .flex_shrink_0()
                .mt(px(1.))
                .size(px(14.))
                .text_color(palette.warning_text),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(warning.text)
                .children(warning.details.map(|details| {
                    details_link(
                        format!("warning-details-{index}").into(),
                        details,
                        palette,
                        cx,
                    )
                })),
        )
        .children(warning.button.map(|it| button(it, true, palette, cx)))
}

fn check_box(
    id: SharedString,
    on: bool,
    action: Action,
    palette: Palette,
    cx: &mut Context<ConverterWindow>,
) -> impl IntoElement + use<> {
    div()
        .id(id)
        .tab_index(0)
        .flex_shrink_0()
        .size(px(16.))
        .rounded(px(RADIUS_XS))
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .map(|this| {
            if on {
                this.bg(palette.primary).border_color(palette.primary)
            } else {
                this.bg(palette.background).border_color(palette.input)
            }
        })
        .focus_visible(move |style| style.border_color(palette.ring).border_2())
        .when(on, |this| {
            this.child(
                svg()
                    .path("icons/check.svg")
                    .size(px(12.))
                    .text_color(palette.primary_foreground),
            )
        })
        .on_click(cx.listener(move |this, _, window, cx| this.act(action, window, cx)))
}

fn row(
    group: usize,
    index: usize,
    row: Row,
    palette: Palette,
    cx: &mut Context<ConverterWindow>,
) -> AnyElement {
    let id: SharedString = format!("row-{group}-{index}").into();
    let title_color = if row.off {
        palette.faint_foreground
    } else {
        palette.foreground
    };
    let detail_color = if row.off {
        palette.faint_foreground
    } else {
        palette.muted_foreground
    };
    let element = div()
        .id(id.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .w_full()
        .min_h(px(44.))
        .px(px(12.))
        .py(px(8.))
        .when(index > 0, |this| {
            this.border_t_1().border_color(palette.border)
        })
        .children(row.check.map(|(kind, on)| {
            check_box(
                format!("{id}-check").into(),
                on,
                Action::Toggle(kind),
                palette,
                cx,
            )
        }))
        .children(row.icon.map(|icon| {
            svg()
                .path(icon_path(icon))
                .flex_shrink_0()
                .size(px(16.))
                .text_color(if row.warning {
                    palette.warning_text
                } else {
                    title_color
                })
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(title_color)
                        .child(row.title),
                )
                .when(!row.detail.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(SMALL_TEXT))
                            .line_height(px(16.))
                            .text_color(detail_color)
                            .child(row.detail),
                    )
                })
                .children(row.path.map(|path| {
                    div()
                        .font_family(MONOSPACE)
                        .text_size(px(11.5))
                        .text_color(palette.muted_foreground)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(path)
                }))
                .children(row.details.map(|details| {
                    details_link(format!("{id}-details").into(), details, palette, cx)
                })),
        )
        .when(!row.trailing.is_empty(), |this| {
            this.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(SMALL_TEXT))
                    .text_color(palette.muted_foreground)
                    .whitespace_nowrap()
                    .child(row.trailing),
            )
        })
        .children(row.button.map(|it| button(it, false, palette, cx)));
    match row.action {
        Some(action) => focus_ring(element, palette)
            .tab_index(0)
            .cursor_pointer()
            .hover(move |style| style.bg(palette.accent))
            .child(
                svg()
                    .path("icons/chevron-right.svg")
                    .flex_shrink_0()
                    .size(px(14.))
                    .text_color(palette.muted_foreground),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.act(action, window, cx)))
            .into_any_element(),
        None => element.into_any_element(),
    }
}

fn empty(
    empty: Empty,
    palette: Palette,
    cx: &mut Context<ConverterWindow>,
) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .p(px(24.))
        .text_center()
        .text_color(palette.muted_foreground)
        .child(
            svg()
                .path(icon_path(Icon::Folder))
                .size(px(24.))
                .text_color(palette.muted_foreground),
        )
        .child(empty.text)
        .child(button(empty.button, false, palette, cx))
}

fn meter(meter: Meter, palette: Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(32.))
                .line_height(px(40.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{}%", meter.percent)),
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
                        .w(gpui::relative(f32::from(meter.percent) / 100.))
                        .bg(palette.component.progress_bar),
                ),
        )
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
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(meter.line),
                )
                .child(div().flex_shrink_0().child(meter.left)),
        )
}

type Segment = [(f32, f32); 4];

const TICK_RING: &[Segment] = &[
    [(60., 12.), (84., 12.), (102., 30.), (102., 52.)],
    [(102., 52.), (102., 75.), (84., 92.), (60., 92.)],
    [(60., 92.), (35., 92.), (18., 74.), (18., 52.)],
    [(18., 52.), (18., 31.), (34., 14.), (56., 12.5)],
    [(56., 12.5), (62., 12.), (68., 13.), (73., 15.)],
];
const TICK_MARK: &[(f32, f32)] = &[(38., 54.), (54., 70.), (84., 34.)];
const TICK_VIEW: (f32, f32) = (120., 104.);
const TICK_WIDTH: f32 = 72.;
const TICK_TOTAL_MS: f32 = 1200.;
const RING_TIMING: (f32, f32) = (450., 750.);
const MARK_TIMING: (f32, f32) = (80., 500.);

fn flatten(segments: &[Segment]) -> Vec<(f32, f32)> {
    let mut points = vec![segments[0][0]];
    for [a, b, c, d] in segments {
        for step in 1..=24 {
            let t = step as f32 / 24.;
            let u = 1. - t;
            let blend = |p: f32, q: f32, r: f32, s: f32| {
                u * u * u * p + 3. * u * u * t * q + 3. * u * t * t * r + t * t * t * s
            };
            points.push((blend(a.0, b.0, c.0, d.0), blend(a.1, b.1, c.1, d.1)));
        }
    }
    points
}

fn trimmed(points: &[(f32, f32)], fraction: f32) -> Vec<(f32, f32)> {
    let lengths: Vec<f32> = points
        .windows(2)
        .map(|pair| ((pair[1].0 - pair[0].0).powi(2) + (pair[1].1 - pair[0].1).powi(2)).sqrt())
        .collect();
    let mut allowed = lengths.iter().sum::<f32>() * fraction.clamp(0., 1.);
    let mut out = vec![points[0]];
    for (pair, length) in points.windows(2).zip(lengths) {
        if allowed <= 0. {
            break;
        }
        if length <= allowed {
            out.push(pair[1]);
            allowed -= length;
        } else {
            let t = allowed / length;
            out.push((
                pair[0].0 + (pair[1].0 - pair[0].0) * t,
                pair[0].1 + (pair[1].1 - pair[0].1) * t,
            ));
            break;
        }
    }
    out
}

fn stroke_fraction(elapsed_ms: f32, (start, duration): (f32, f32)) -> f32 {
    let t = ((elapsed_ms - start) / duration).clamp(0., 1.);
    1. - (1. - t).powi(3)
}

fn tick(palette: Palette) -> impl IntoElement {
    let scale = TICK_WIDTH / TICK_VIEW.0;
    let height = TICK_VIEW.1 * scale;
    let ring = flatten(TICK_RING);
    div().flex().justify_center().pt(px(20.)).pb(px(8.)).child(
        div().w(px(TICK_WIDTH)).h(px(height)).with_animation(
            "all-set",
            Animation::new(Duration::from_millis(TICK_TOTAL_MS as u64)),
            move |this, delta| {
                let elapsed = delta * TICK_TOTAL_MS;
                let strokes = [
                    (
                        trimmed(&ring, stroke_fraction(elapsed, RING_TIMING)),
                        3. * scale,
                        palette.muted_foreground,
                    ),
                    (
                        trimmed(TICK_MARK, stroke_fraction(elapsed, MARK_TIMING)),
                        5.5 * scale,
                        palette.success_text,
                    ),
                ];
                this.child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, (), window, _| {
                            for (points, width, color) in strokes {
                                if points.len() < 2 {
                                    continue;
                                }
                                let mut options = StrokeOptions::default().with_line_width(width);
                                options.start_cap = LineCap::Round;
                                options.end_cap = LineCap::Round;
                                options.line_join = LineJoin::Round;
                                let mut path = PathBuilder::stroke(px(width))
                                    .with_style(gpui::PathStyle::Stroke(options));
                                let at = |(x, y): (f32, f32)| {
                                    point(
                                        bounds.origin.x + px(x * scale),
                                        bounds.origin.y + px(y * scale),
                                    )
                                };
                                path.move_to(at(points[0]));
                                for point in &points[1..] {
                                    path.line_to(at(*point));
                                }
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, color);
                                }
                            }
                        },
                    )
                    .size_full(),
                )
            },
        ),
    )
}

impl Render for ConverterWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_appearance(window.appearance());
        let screen = self.wizard.screen();
        let converting = self.wizard.is_converting();
        let head = (!screen.title.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .when(screen.tick, |this| this.items_center().text_center())
                .child(
                    div()
                        .text_size(px(HEADING_TEXT))
                        .line_height(px(26.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(screen.title.clone()),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_color(palette.muted_foreground)
                        .child(screen.subtitle.clone()),
                )
        });
        let warnings: Vec<_> = screen
            .warnings
            .into_iter()
            .enumerate()
            .map(|(index, it)| warning(index, it, palette, cx))
            .collect();
        let mut groups = Vec::new();
        for (group, rows) in screen.groups.into_iter().enumerate() {
            let mut drawn = Vec::new();
            for (index, it) in rows.into_iter().enumerate() {
                drawn.push(row(group, index, it, palette, cx));
            }
            groups.push(
                div()
                    .flex()
                    .flex_col()
                    .rounded(px(RADIUS_MD))
                    .bg(palette.card)
                    .overflow_hidden()
                    .children(drawn),
            );
        }
        let start: Vec<_> = screen
            .start
            .into_iter()
            .map(|it| button(it, false, palette, cx))
            .collect();
        let end: Vec<_> = screen
            .end
            .into_iter()
            .map(|it| button(it, false, palette, cx))
            .collect();
        let empty_state = screen.empty.map(|it| empty(it, palette, cx));
        div()
            .id("converter")
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
            .font_family(i18n::ui_font())
            .child(caption(
                self.icon.clone(),
                palette,
                window.is_window_active(),
                converting,
                cx,
            ))
            .child(steps(&screen.steps, palette))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("page")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .px(px(24.))
                            .py(px(16.))
                            .when(screen.tick, |this| this.child(tick(palette)))
                            .children(head)
                            .children(warnings)
                            .children(empty_state)
                            .children(groups)
                            .children(screen.meter.map(|it| meter(it, palette)))
                            .children(screen.notes.into_iter().map(|note| {
                                div()
                                    .text_size(px(SMALL_TEXT))
                                    .line_height(px(16.))
                                    .text_color(palette.muted_foreground)
                                    .child(note)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(8.))
                            .px(px(24.))
                            .pt(px(12.))
                            .pb(px(16.))
                            .children(start)
                            .child(div().flex_1().min_w_0())
                            .children(end),
                    ),
            )
    }
}

fn steps(steps: &[(String, StepState)], palette: Palette) -> impl IntoElement {
    let mut line = div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .gap(px(6.))
        .px(px(24.))
        .pt(px(4.))
        .text_size(px(SMALL_TEXT));
    for (index, (label, state)) in steps.iter().enumerate() {
        if index > 0 {
            line = line.child(div().text_color(palette.border_strong).child("›"));
        }
        let text = div().child(label.clone());
        line = line.child(match state {
            StepState::Current => text
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.foreground),
            StepState::Done => text.text_color(palette.muted_foreground),
            StepState::Ahead => text.text_color(palette.faint_foreground),
        });
    }
    line
}

fn caption(
    icon: Arc<Image>,
    palette: Palette,
    active: bool,
    converting: bool,
    cx: &mut Context<ConverterWindow>,
) -> impl IntoElement + use<> {
    let glyph: Rgba = if active {
        palette.foreground
    } else {
        palette.muted_foreground
    };
    let control = move |id: &'static str, path: &'static str, height: f32, hover: Rgba| {
        focus_ring(div().id(id).group(id), palette)
            .tab_index(0)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .hover(move |style| style.bg(hover))
            .child(
                svg()
                    .path(path)
                    .w(px(10.))
                    .h(px(height))
                    .text_color(glyph)
                    .when(id == "caption-close", |this| {
                        this.group_hover(id, |style| style.text_color(gpui::white()))
                    }),
            )
    };
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
                .pl(px(16.))
                .window_control_area(WindowControlArea::Drag)
                .child(img(icon).size(px(20.)).rounded(px(5.)))
                .child(
                    div()
                        .text_size(px(SMALL_TEXT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(WINDOW_TITLE),
                ),
        )
        .child(
            control(
                "caption-minimize",
                "icons/caption-minimize-windows.svg",
                1.,
                palette.accent,
            )
            .on_click(|_, window, _| window.minimize_window()),
        )
        .child(
            control(
                "caption-close",
                "icons/caption-close-windows.svg",
                10.,
                palette.danger,
            )
            .when(converting, |this| this.cursor_default())
            .on_click(cx.listener(|this, _, window, cx| this.act(Action::Close, window, cx))),
        )
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

fn main() {
    let target = Target::from_environment();
    i18n::set(i18n::resolve(
        i18n::language_setting(&target.settings_file).as_deref(),
    ));
    let app = gpui_platform::application().with_assets(Assets);
    app.run(move |cx: &mut App| {
        if let Err(error) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
        {
            eprintln!("failed to load fonts: {error:#}");
        }
        cx.set_reduce_motion(system::reduce_motion());
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, Some(KEY_CONTEXT)),
            KeyBinding::new("shift-tab", FocusPrevious, Some(KEY_CONTEXT)),
        ]);
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(WINDOW_TITLE.into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Opaque,
                is_resizable: false,
                is_minimizable: true,
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| {
                    cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                        .detach();
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    ConverterWindow {
                        wizard: Wizard::new(target, system::this_machine()),
                        icon: Arc::new(Image::from_bytes(ImageFormat::Png, APP_ICON.to_vec())),
                        focus,
                    }
                });
                let watched = view.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    let Some(view) = watched.upgrade() else {
                        return true;
                    };
                    if !view.read(cx).wizard.is_converting() {
                        return true;
                    }
                    view.update(cx, |this, cx| this.act(Action::Close, window, cx));
                    false
                });
                view
            },
        );
        if let Err(error) = opened {
            eprintln!("failed to open the converter window: {error:#}");
            cx.quit();
        }
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
}
