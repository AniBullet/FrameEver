use gpui::{
    Animation, AnimationExt, Bounds, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point, Render, Size, Styled,
    Window, div, point, px,
};

use crate::{
    app_windows::AppWindows,
    i18n::t,
    session::{Phase, RecordingSession},
    theme::{Theme, motion},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CountdownFrame {
    Number(u32),
    Cue,
    Hidden,
}

fn countdown_frame(phase: Phase, remaining: Option<u32>, cue_started: bool) -> CountdownFrame {
    match (phase, remaining, cue_started) {
        (Phase::Starting, Some(0), true) => CountdownFrame::Cue,
        (Phase::Starting, Some(0), false) => CountdownFrame::Number(1),
        (Phase::Starting, Some(number), _) => CountdownFrame::Number(number),
        _ => CountdownFrame::Hidden,
    }
}

fn mark_origin(
    target: Bounds<Pixels>,
    viewport: Size<Pixels>,
    work_bottom: Pixels,
) -> Point<Pixels> {
    let bottom = target.bottom().min(work_bottom - px(70.)).max(target.top());
    point(
        (target.center().x - px(64.)).clamp(px(0.), (viewport.width - px(128.)).max(px(0.))),
        ((target.top() + bottom) / 2. - px(64.))
            .clamp(px(0.), (viewport.height - px(162.)).max(px(0.))),
    )
}

pub struct CountdownWindow {
    session: Entity<RecordingSession>,
    focus: FocusHandle,
    frame: CountdownFrame,
    target: Bounds<Pixels>,
    work_bottom: Pixels,
    interactive: bool,
    can_skip: bool,
}

impl CountdownWindow {
    pub fn new(
        session: Entity<RecordingSession>,
        initial: u32,
        target: Bounds<Pixels>,
        work_bottom: Pixels,
        interactive: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::theme::bind_window(window, cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.observe_in(&session, window, |this, session, window, cx| {
            let session = session.read(cx);
            this.frame = countdown_frame(
                session.phase,
                session.countdown_remaining(),
                session.countdown_cue_started(),
            );
            this.can_skip = session
                .countdown_remaining()
                .is_some_and(|remaining| remaining > 0);
            if this.frame == CountdownFrame::Hidden {
                if cx
                    .global::<AppWindows>()
                    .countdown
                    .is_some_and(|handle| handle.window_id() == window.window_handle().window_id())
                {
                    cx.global_mut::<AppWindows>().countdown = None;
                }
                window.remove_window();
            } else {
                window.request_animation_frame();
                cx.notify();
            }
        })
        .detach();
        Self {
            session,
            focus,
            frame: CountdownFrame::Number(initial),
            target,
            work_bottom,
            interactive,
            can_skip: true,
        }
    }
}

impl Render for CountdownWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window, cx, false);
        let viewport = window.viewport_size();
        let target = self.target.intersect(&Bounds {
            origin: point(px(0.), px(0.)),
            size: viewport,
        });
        let mut root = div()
            .id("record-countdown")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .font_family(crate::theme::ui_font())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.session.update(cx, |session, cx| session.stop(cx)),
                    "enter" => this
                        .session
                        .update(cx, |session, cx| session.skip_countdown(cx)),
                    _ => return,
                }
                cx.stop_propagation();
            }));
        for (x, y, width, height) in [
            (px(0.), px(0.), target.left(), viewport.height),
            (
                target.right(),
                px(0.),
                viewport.width - target.right(),
                viewport.height,
            ),
            (target.left(), px(0.), target.size.width, target.top()),
            (
                target.left(),
                target.bottom(),
                target.size.width,
                viewport.height - target.bottom(),
            ),
        ] {
            root = root.child(
                div()
                    .absolute()
                    .left(x)
                    .top(y)
                    .w(width.max(px(0.)))
                    .h(height.max(px(0.)))
                    .bg(theme.overlay),
            );
        }
        root = root.child(
            div()
                .absolute()
                .left(target.left())
                .top(target.top())
                .w(target.size.width)
                .h(target.size.height)
                .border_1()
                .border_color(theme.fixed_white),
        );
        let mut ring = div()
            .size(px(92.))
            .rounded_full()
            .border(px(6.))
            .border_color(theme.fixed_white)
            .bg(theme.record_gap)
            .flex()
            .items_center()
            .justify_center();
        match self.frame {
            CountdownFrame::Number(number) => {
                let number = div()
                    .text_size(px(48.))
                    .line_height(px(48.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.fixed_white)
                    .child(number.to_string())
                    .with_animation(
                        ("countdown-number", number),
                        Animation::new(motion::FAST).with_easing(motion::EASE_EFFECTS.curve()),
                        |element, progress| element.opacity(progress),
                    );
                ring = ring.child(number);
            }
            CountdownFrame::Cue => {
                ring = ring.child(div().size(px(48.)).rounded_full().bg(theme.recording));
            }
            CountdownFrame::Hidden => {}
        }
        let origin = mark_origin(target, viewport, self.work_bottom);
        let mark = div()
            .size(px(128.))
            .rounded(px(32.))
            .bg(theme.primary)
            .flex()
            .items_center()
            .justify_center()
            .child(ring);
        let mut cluster = div()
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .w(px(128.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .child(mark);
        if self.frame != CountdownFrame::Cue && self.interactive {
            let caption = if self.can_skip {
                format!("Esc  {}    Enter  {}", t!("Cancel"), t!("Start now"))
            } else {
                t!("Starting").to_owned()
            };
            cluster = cluster.child(
                div()
                    .flex_none()
                    .h(px(26.))
                    .px(px(8.))
                    .rounded(px(4.))
                    .bg(theme.badge)
                    .text_color(theme.fixed_white)
                    .text_size(px(12.))
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .child(caption),
            );
        }
        root.child(cluster)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_numbers_become_the_record_mark_for_the_cue_then_disappear() {
        assert_eq!(
            countdown_frame(Phase::Starting, Some(3), false),
            CountdownFrame::Number(3)
        );
        assert_eq!(
            countdown_frame(Phase::Starting, Some(1), false),
            CountdownFrame::Number(1)
        );
        assert_eq!(
            countdown_frame(Phase::Starting, Some(0), true),
            CountdownFrame::Cue
        );
        assert_eq!(
            countdown_frame(Phase::Starting, None, true),
            CountdownFrame::Hidden
        );
        assert_eq!(
            countdown_frame(Phase::Recording { paused: false }, None, true),
            CountdownFrame::Hidden
        );
        assert_eq!(
            countdown_frame(Phase::Idle, Some(3), true),
            CountdownFrame::Hidden
        );
    }

    #[test]
    fn countdown_keeps_the_last_number_until_the_cue_actually_starts() {
        assert_eq!(
            countdown_frame(Phase::Starting, Some(0), false),
            CountdownFrame::Number(1)
        );
        assert_eq!(
            countdown_frame(Phase::Starting, Some(0), true),
            CountdownFrame::Cue
        );
    }
}
