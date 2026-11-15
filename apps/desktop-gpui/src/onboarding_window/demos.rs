use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, Div, FontWeight, Hsla, IntoElement, ObjectFit,
    ParentElement, SharedString, Styled, StyledImage, div, img, prelude::FluentBuilder, px,
    relative,
};

use super::{OnboardingWindow, Step, copy, ease, glyph};
use crate::i18n::{n, t, tr};

const TILE_NAMES: [&str; 4] = [n!("Bug report"), n!("Demo"), n!("Login page"), n!("Weekly")];
const TILE_ICONS: [&str; 4] = [
    "icons/zap.svg",
    "icons/clapperboard.svg",
    "icons/image.svg",
    "icons/zap.svg",
];

#[derive(Clone, Copy)]
struct Spot {
    x: f32,
    y: f32,
}

const fn at(x: f32, y: f32) -> Spot {
    Spot { x, y }
}

impl OnboardingWindow {
    fn phase_key(&self, name: &'static str) -> (&'static str, usize) {
        (
            name,
            self.demo_cycle * 64 + self.demo_phase + self.step_revision * 4096,
        )
    }

    fn motion<E: IntoElement + Styled + 'static>(
        &self,
        element: E,
        name: &'static str,
        millis: u64,
        apply: impl Fn(E, f32) -> E + 'static,
    ) -> AnyElement {
        if !self.animated {
            return apply(element, 1.).into_any_element();
        }
        element
            .with_animation(
                self.phase_key(name),
                Animation::new(Duration::from_millis(millis)),
                move |el, t| apply(el, ease(t)),
            )
            .into_any_element()
    }

    fn fade_in(&self, element: Div, name: &'static str, millis: u64, fresh: bool) -> AnyElement {
        if !fresh {
            return element.into_any_element();
        }
        self.motion(element, name, millis, |el, t| el.opacity(t))
    }

    fn rise_in(&self, element: Div, name: &'static str, millis: u64, fresh: bool) -> AnyElement {
        if !fresh {
            return element.into_any_element();
        }
        self.motion(element, name, millis, |el, t| {
            el.top(px(16. * (1. - t))).opacity(t)
        })
    }

    fn desktop(&self, padding: f32) -> Div {
        let theme = self.theme;
        div()
            .absolute()
            .inset_0()
            .p(px(padding))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children([0.70, 0.54, 0.82, 0.46, 0.64, 0.38].map(|width| {
                div()
                    .h(px(7.))
                    .w(relative(width))
                    .rounded(px(4.))
                    .bg(theme.border)
            }))
    }

    fn bottom_center(&self, bottom: f32, child: impl IntoElement) -> Div {
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(bottom))
            .flex()
            .justify_center()
            .child(child)
    }

    fn corner(&self, child: impl IntoElement) -> Div {
        div().absolute().right(px(14.)).bottom(px(14.)).child(child)
    }

    fn chip(&self, icon: Option<(&'static str, Hsla)>, text: impl Into<SharedString>) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .py(px(5.))
            .rounded(px(8.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .text_size(px(11.))
            .text_color(theme.foreground)
            .whitespace_nowrap()
            .when_some(icon, |el, (path, color)| el.child(glyph(path, 14., color)))
            .child(text.into())
    }

    fn recording_bar(&self, phase: usize, max_seconds: usize) -> Div {
        let theme = self.theme;
        let bar = div()
            .h(px(30.))
            .pl(px(10.))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(8.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .text_size(px(11.))
            .text_color(theme.foreground);
        if phase == 0 {
            return bar.pl(px(6.)).child(
                div()
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(5.))
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .child(t!("Start recording")),
            );
        }
        bar.child(div().size(px(8.)).rounded_full().bg(theme.recording))
            .child(format!("0:0{}", (phase - 1).min(max_seconds)))
            .child(glyph("icons/pause.svg", 14., theme.muted_foreground))
            .child(
                div()
                    .size(px(20.))
                    .rounded(px(5.))
                    .bg(theme.primary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().size(px(7.)).rounded(px(1.5)).bg(theme.recording)),
            )
    }

    fn cursor(&self, from: Spot, to: Spot, name: &'static str) -> AnyElement {
        let windows = cfg!(target_os = "windows");
        let arrow = img(if windows {
            "onboarding/cursor-windows.svg"
        } else {
            "onboarding/cursor-macos.svg"
        })
        .w(px(14.))
        .h(px(20.))
        .object_fit(ObjectFit::Contain);
        let cursor = div().absolute().child(arrow);
        self.motion(cursor, name, 600, move |el, t| {
            el.left(relative(from.x + (to.x - from.x) * t))
                .top(relative(from.y + (to.y - from.y) * t))
        })
    }

    fn tabs(&self) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .gap(px(4.))
            .px(px(8.))
            .py(px(6.))
            .border_b_1()
            .border_color(theme.border)
            .text_size(px(10.))
            .text_color(theme.muted_foreground)
            .whitespace_nowrap()
            .children(
                [t!("All"), t!("Studio"), t!("Instant"), t!("Screenshots")]
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| {
                        div()
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(4.))
                            .when(index == 0, |el| {
                                el.bg(theme.secondary).text_color(theme.foreground)
                            })
                            .child(label)
                    }),
            )
    }

    fn tile(&self, index: usize, fresh: bool, selected: bool) -> Div {
        let theme = self.theme;
        let name: SharedString = if index == 0 {
            t!("Instant 11-15").into()
        } else {
            tr(TILE_NAMES[index % 4]).into()
        };
        let thumb = div()
            .relative()
            .h(px(36.))
            .rounded(px(4.))
            .bg(theme.secondary)
            .flex()
            .items_center()
            .justify_center()
            .when(fresh || selected, |el| {
                el.border_2().border_color(theme.ring)
            })
            .child(glyph(TILE_ICONS[index % 4], 14., theme.faint_foreground))
            .when(fresh, |el| {
                el.child(
                    div()
                        .absolute()
                        .top(px(3.))
                        .right(px(3.))
                        .size(px(10.))
                        .rounded_full()
                        .bg(theme.success),
                )
            });
        div()
            .flex()
            .flex_col()
            .gap(px(3.))
            .text_size(px(9.))
            .text_color(theme.muted_foreground)
            .child(thumb)
            .child(div().truncate().child(name))
    }

    fn library(&self, fresh: bool, selected: bool, dragging: bool) -> Div {
        let first = self.tile(0, fresh, selected);
        let first = if dragging {
            self.motion(first, "demo-drag", 900, |el, t| {
                el.relative()
                    .left(px(150. * t))
                    .top(px(40. * t))
                    .opacity(1. - 0.15 * t)
            })
        } else {
            first.into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(self.theme.background)
            .border_1()
            .border_color(self.theme.border)
            .rounded(px(6.))
            .overflow_hidden()
            .child(self.tabs())
            .child(
                div()
                    .flex_1()
                    .p(px(8.))
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(div().w(relative(0.22)).child(first))
                    .children((1..8).map(|index| {
                        div()
                            .w(relative(0.22))
                            .child(self.tile(index, false, false))
                    })),
            )
    }

    fn demo_studio(&self, phase: usize) -> Div {
        let theme = self.theme;
        let mut screen = div().child(self.desktop(16.));
        if phase <= 4 {
            screen = screen.child(self.bottom_center(12., self.recording_bar(phase, 3)));
        }
        if phase == 3 || phase == 4 {
            screen = screen.child(self.bottom_center(
                52.,
                self.rise_in(
                    self.chip(None, t!("Chapter marked")),
                    "demo-chapter",
                    300,
                    phase == 3,
                ),
            ));
        }
        if phase >= 5 {
            let zoom = phase >= 7;
            let shot = div().rounded(px(5.)).bg(theme.card);
            let shot = self.motion(shot, "demo-zoom", 600, move |el, t| {
                let t = if zoom { t } else { 0. };
                el.w(relative(0.62 + 0.22 * t)).h(relative(0.7 + 0.24 * t))
            });
            let preview = div()
                .relative()
                .flex_1()
                .mx(px(8.))
                .mt(px(8.))
                .rounded(px(6.))
                .bg(theme.preview)
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .child(shot)
                .when(phase >= 6, |el| {
                    el.child(
                        self.bottom_center(
                            8.,
                            self.fade_in(
                                div()
                                    .px(px(8.))
                                    .py(px(3.))
                                    .rounded(px(4.))
                                    .bg(theme.overlay)
                                    .text_color(theme.primary_foreground)
                                    .text_size(px(10.))
                                    .whitespace_nowrap()
                                    .child(t!("Today we'll look at the new export page")),
                                "demo-caption",
                                300,
                                phase == 6,
                            ),
                        ),
                    )
                });
            let track = || {
                div()
                    .relative()
                    .h(px(12.))
                    .rounded(px(3.))
                    .bg(theme.secondary)
            };
            let segment = |left: f32, width: f32, color: Hsla| {
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(left))
                    .w(relative(width))
                    .rounded(px(3.))
                    .bg(color)
            };
            let head = div()
                .absolute()
                .top(px(-4.))
                .bottom(px(-4.))
                .w(px(2.))
                .bg(theme.foreground);
            let head = if phase == 6 {
                self.motion(head, "demo-head", 1000, |el, t| el.left(relative(0.58 * t)))
            } else {
                head.left(relative(if phase == 5 { 0. } else { 0.58 }))
                    .into_any_element()
            };
            let tracks = div()
                .relative()
                .h(px(64.))
                .m(px(8.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    track()
                        .child(segment(0., 1., theme.track_scene.into()))
                        .child(
                            div()
                                .absolute()
                                .top(px(-2.))
                                .left(relative(0.42))
                                .w(px(2.))
                                .h(px(16.))
                                .bg(theme.foreground),
                        ),
                )
                .child(track().child(segment(0.38, 0.26, theme.track_zoom.into())))
                .child(track().child(segment(0.12, 0.5, theme.track_text.into())))
                .child(head);
            screen = screen.child(
                self.fade_in(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_col()
                        .bg(theme.background)
                        .child(preview)
                        .child(tracks),
                    "demo-editor",
                    400,
                    phase == 5,
                ),
            );
        }
        if phase >= 8 {
            let done = phase >= 10;
            let fill = div().h_full().bg(theme.component.progress_bar);
            let fill = match phase {
                8 => fill.w(relative(0.)).into_any_element(),
                9 => self.motion(fill, "demo-fill", 1700, |el, t| el.w(relative(t))),
                _ => fill.w(relative(1.)).into_any_element(),
            };
            let percent = match phase {
                8 => "0%",
                9 => "",
                _ => "100%",
            };
            let dialog = div()
                .w(px(220.))
                .p(px(14.))
                .rounded(px(10.))
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap(px(10.))
                .text_size(px(12.))
                .text_color(theme.foreground)
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t!("Export")))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_color(theme.muted_foreground)
                        .child("MP4 · 1080p")
                        .child(percent),
                )
                .child(
                    div()
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(theme.secondary)
                        .overflow_hidden()
                        .child(fill),
                )
                .when(done, |el| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(theme.success_text)
                            .child(glyph("icons/circle-check.svg", 14., theme.success_text))
                            .child(t!("Exported")),
                    )
                });
            screen = screen.child(
                self.fade_in(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(theme.overlay)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(dialog),
                    "demo-export",
                    300,
                    phase == 8,
                ),
            );
        }
        screen
    }

    fn demo_instant(&self, phase: usize) -> Div {
        let theme = self.theme;
        let mut screen = div().child(self.desktop(16.));
        if phase <= 4 {
            screen = screen.child(self.bottom_center(12., self.recording_bar(phase, 2)));
        }
        if phase >= 5 {
            let library = div()
                .absolute()
                .left(px(8.))
                .top(px(8.))
                .right(px(8.))
                .bottom(px(8.))
                .child(self.library(true, phase >= 6, phase == 6));
            screen = screen.child(self.rise_in(library, "demo-library", 450, phase == 5));
        }
        if phase >= 6 {
            screen = screen.child(self.corner(self.chip(
                Some(("icons/copy.svg", theme.foreground.into())),
                t!("Drop it into any app"),
            )));
        }
        match phase {
            3 => screen.child(self.cursor(at(0.7, 0.4), at(0.7, 0.4), "demo-cursor")),
            4 => screen.child(self.cursor(at(0.7, 0.4), at(0.61, 0.84), "demo-cursor")),
            6 => screen.child(self.cursor(at(0.14, 0.3), at(0.52, 0.52), "demo-cursor")),
            _ => screen,
        }
    }

    fn demo_screenshot(&self, phase: usize) -> Div {
        let theme = self.theme;
        let mut screen = div().child(self.desktop(16.));
        if phase <= 1 {
            let link: Hsla = theme.link.into();
            let full = phase == 1;
            let area = div()
                .absolute()
                .left(relative(0.22))
                .top(relative(0.18))
                .border(px(1.5))
                .border_dashed()
                .border_color(link)
                .bg(link.opacity(0.12));
            let area = self.motion(area, "demo-area", 700, move |el, t| {
                let t = if full { t } else { 0. };
                el.w(relative(0.52 * t)).h(relative(0.56 * t))
            });
            let loupe = div()
                .absolute()
                .size(px(46.))
                .rounded_full()
                .border_2()
                .border_color(theme.foreground)
                .bg(theme.muted);
            let loupe = self.motion(loupe, "demo-loupe", 700, move |el, t| {
                let t = if full { t } else { 0. };
                el.left(relative(0.16 + 0.52 * t))
                    .top(relative(0.08 + 0.58 * t))
            });
            screen = screen.child(area).child(loupe);
            if full {
                screen = screen.child(
                    div()
                        .absolute()
                        .left(relative(0.62))
                        .top(relative(0.76))
                        .px(px(4.))
                        .py(px(1.))
                        .rounded(px(3.))
                        .bg(theme.overlay)
                        .text_color(theme.primary_foreground)
                        .text_size(px(9.))
                        .child("1280 × 720"),
                );
            }
        }
        if phase >= 2 {
            let numbering = (3..5).contains(&phase);
            let rail = div()
                .w(px(30.))
                .py(px(6.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .border_r_1()
                .border_color(theme.border)
                .child(glyph(
                    "icons/arrow-up-right.svg",
                    14.,
                    theme.muted_foreground,
                ))
                .child({
                    let color: Hsla = if numbering {
                        theme.foreground.into()
                    } else {
                        theme.muted_foreground.into()
                    };
                    div()
                        .size(px(14.))
                        .rounded_full()
                        .border(px(1.5))
                        .border_color(color)
                        .text_color(color)
                        .text_size(px(8.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child("1")
                })
                .children(
                    [
                        "icons/highlighter.svg",
                        "icons/type.svg",
                        "icons/eye-off.svg",
                        "icons/search.svg",
                    ]
                    .map(|icon| glyph(icon, 14., theme.muted_foreground)),
                );
            let badge = |number: &'static str, left: f32, top: f32, fresh: bool| {
                let badge = div()
                    .absolute()
                    .left(relative(left))
                    .top(relative(top))
                    .size(px(18.))
                    .rounded_full()
                    .bg(theme.recording)
                    .text_color(theme.primary_foreground)
                    .text_size(px(10.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(number);
                self.fade_in(badge, badge_animation_id(number), 400, fresh)
            };
            let mut image = div()
                .relative()
                .w(relative(0.72))
                .h(relative(0.72))
                .rounded(px(8.))
                .bg(theme.card)
                .overflow_hidden()
                .child(self.desktop(12.));
            if phase >= 5 {
                image = image.child(
                    self.fade_in(
                        div().absolute().inset_0().bg(theme.overlay).child(
                            div()
                                .absolute()
                                .left(relative(0.14))
                                .top(relative(0.22))
                                .w(relative(0.42))
                                .h(relative(0.34))
                                .rounded_full()
                                .bg(theme.card),
                        ),
                        "demo-spot",
                        400,
                        phase == 5,
                    ),
                );
            }
            if phase >= 3 {
                image = image.child(badge("1", 0.22, 0.26, phase == 3));
            }
            if phase >= 4 {
                image = image.child(badge("2", 0.58, 0.58, phase == 4));
            }
            if phase >= 6 {
                image = image.child(
                    self.fade_in(
                        div()
                            .absolute()
                            .right(relative(0.1))
                            .top(relative(0.16))
                            .px(px(8.))
                            .py(px(3.))
                            .rounded(px(8.))
                            .bg(theme.foreground)
                            .text_color(theme.background)
                            .text_size(px(9.))
                            .child(t!("Click here")),
                        "demo-bubble",
                        400,
                        phase == 6,
                    ),
                );
            }
            let editor = div()
                .absolute()
                .inset_0()
                .flex()
                .bg(theme.background)
                .child(rail)
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.preview)
                        .child(image),
                );
            screen = screen.child(self.fade_in(editor, "demo-shot-editor", 350, phase == 2));
            if phase == 2 && self.animated {
                screen = screen.child(self.motion(
                    div().absolute().inset_0().bg(gpui::white()),
                    "demo-shutter",
                    300,
                    |el, t| el.opacity(0.6 * (1. - t)),
                ));
            }
        }
        if phase >= 7 {
            screen = screen.child(self.corner(self.rise_in(
                self.chip(
                    Some(("icons/check.svg", theme.success_text.into())),
                    t!("Screenshot copied"),
                ),
                "demo-copied",
                300,
                phase == 7,
            )));
        }
        screen
    }

    fn demo_library(&self, phase: usize) -> Div {
        let theme = self.theme;
        let mut screen = div().child(self.desktop(16.));
        if phase == 0 {
            let compact = div()
                .absolute()
                .left(px(16.))
                .top(px(16.))
                .w(px(132.))
                .h(px(176.))
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .bg(theme.background)
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.))
                .child(
                    div()
                        .flex()
                        .gap(px(2.))
                        .p(px(2.))
                        .rounded(px(5.))
                        .bg(theme.secondary)
                        .children((0..3).map(|index| {
                            div()
                                .flex_1()
                                .h(px(12.))
                                .rounded(px(4.))
                                .when(index == 0, |el| el.bg(theme.background))
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .children((0..4).map(|_| {
                            div()
                                .w(px(53.))
                                .h(px(22.))
                                .rounded(px(4.))
                                .bg(theme.secondary)
                        })),
                )
                .child(div().flex_1())
                .child(div().h(px(18.)).rounded(px(4.)).bg(theme.primary))
                .child(
                    div().flex().justify_end().text_size(px(9.)).child(
                        div()
                            .px(px(3.))
                            .rounded(px(3.))
                            .border(px(1.5))
                            .border_color(theme.ring)
                            .text_color(theme.foreground)
                            .child(format!("{} ›", t!("Library"))),
                    ),
                );
            screen = screen.child(compact).child(self.cursor(
                at(0.4, 0.6),
                at(0.22, 0.6),
                "demo-cursor",
            ));
        } else {
            let library = div()
                .size_full()
                .child(self.library(false, phase >= 2, phase == 4));
            let library = if phase == 1 {
                self.motion(library, "demo-expand", 550, |el, t| {
                    el.w(relative(0.22 + 0.78 * t)).h(relative(0.62 + 0.38 * t))
                })
            } else {
                library.into_any_element()
            };
            screen = screen.child(
                div()
                    .absolute()
                    .left(px(8.))
                    .top(px(8.))
                    .right(px(8.))
                    .bottom(px(8.))
                    .child(library),
            );
        }
        if phase == 3 {
            screen = screen.child(
                self.fade_in(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(theme.overlay)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(relative(0.62))
                                .h(relative(0.62))
                                .rounded(px(6.))
                                .bg(theme.preview)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(glyph("icons/play.svg", 20., theme.primary_foreground)),
                        ),
                    "demo-viewer",
                    250,
                    true,
                ),
            );
        }
        if phase == 4 {
            screen = screen.child(self.corner(self.chip(
                Some(("icons/copy.svg", theme.foreground.into())),
                t!("The file itself, not a link"),
            )));
        }
        if phase >= 5 {
            let typed = phase >= 6;
            let rows: Vec<&'static str> = if typed {
                vec![t!("Compress selected"), t!("Automatic compression")]
            } else {
                vec![
                    t!("Open settings"),
                    t!("Show storage folder"),
                    t!("Start recording"),
                ]
            };
            let palette = div()
                .w(px(300.))
                .rounded(px(10.))
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .text_size(px(11.))
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .py(px(8.))
                        .border_b_1()
                        .border_color(theme.border)
                        .text_color(theme.foreground)
                        .child(glyph("icons/search.svg", 14., theme.muted_foreground))
                        .when(typed, |el| el.child(t!("compress")))
                        .child(div().w(px(1.)).h(px(12.)).bg(theme.foreground)),
                )
                .children(rows.into_iter().enumerate().map(|(index, label)| {
                    div()
                        .flex()
                        .justify_between()
                        .px(px(10.))
                        .py(px(6.))
                        .text_color(theme.muted_foreground)
                        .when(index == 0, |el| {
                            el.bg(theme.selection)
                                .text_color(theme.selection_foreground)
                                .child(label)
                                .child("Enter")
                        })
                        .when(index > 0, |el| el.child(label))
                }));
            screen = screen.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(22.))
                    .flex()
                    .justify_center()
                    .child(self.rise_in(palette, "demo-palette", 250, phase == 5)),
            );
        }
        screen
    }

    pub(super) fn demo_screen(&self) -> AnyElement {
        let phase = self.demo_phase;
        let content = match self.step {
            Step::Studio => self.demo_studio(phase),
            Step::Instant => self.demo_instant(phase),
            Step::Screenshot => self.demo_screenshot(phase),
            _ => self.demo_library(phase),
        };
        content
            .relative()
            .flex_1()
            .min_h_0()
            .rounded(px(10.))
            .overflow_hidden()
            .bg(self.theme.muted)
            .into_any_element()
    }

    pub(super) fn demo_labels(&self) -> Div {
        let theme = self.theme;
        let phase = self.demo_phase;
        let (labels, active): (Vec<&'static str>, usize) = match self.step {
            Step::Studio => (
                vec![t!("Record"), t!("Edit"), t!("Export")],
                if phase >= 8 {
                    2
                } else if phase >= 5 {
                    1
                } else {
                    0
                },
            ),
            Step::Instant => (
                vec![t!("Record"), t!("Stop"), t!("In the library")],
                if phase >= 5 {
                    2
                } else if phase == 4 {
                    1
                } else {
                    0
                },
            ),
            Step::Screenshot => (
                vec![t!("Select area"), t!("Annotate"), t!("Copy")],
                if phase >= 7 {
                    2
                } else if phase >= 2 {
                    1
                } else {
                    0
                },
            ),
            _ => (
                vec![t!("Expand"), t!("Preview"), t!("Drag out"), t!("Find")],
                if phase >= 5 {
                    3
                } else if phase == 4 {
                    2
                } else if phase >= 2 {
                    1
                } else {
                    0
                },
            ),
        };
        div()
            .flex()
            .justify_center()
            .gap(px(6.))
            .text_size(px(11.))
            .text_color(theme.faint_foreground)
            .children(labels.into_iter().enumerate().flat_map(|(index, label)| {
                let dot =
                    (index > 0).then(|| copy("·", 11., theme.faint_foreground).into_any_element());
                let pill = div()
                    .px(px(8.))
                    .py(px(2.))
                    .rounded_full()
                    .when(index == active, |el| {
                        el.bg(theme.secondary).text_color(theme.foreground)
                    })
                    .child(label)
                    .into_any_element();
                dot.into_iter().chain([pill])
            }))
    }
}

fn badge_animation_id(number: &str) -> &'static str {
    if number == "1" {
        "demo-badge-1"
    } else {
        "demo-badge-2"
    }
}
