use gpui::{
    App, AppContext as _, Context, CursorStyle, Entity, EventEmitter, FocusHandle, Global, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement as _, PathBuilder, PathStyle, Render, Styled, Window, WindowHandle,
    canvas, div, point, prelude::FluentBuilder as _, px,
};
use lyon_tessellation::{LineCap, LineJoin, StrokeOptions};

use crate::{
    session::{Phase, RecordingSession},
    theme::Theme,
};

pub const INK_COLOURS: usize = 3;
const MIN_STEP: f32 = 1.5;
const MIN_ARROW: f32 = 6.;
const ARROW_HEAD: f32 = 14.;
const ARROW_SPREAD: f32 = 0.49;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkTool {
    Pen,
    Highlighter,
    Arrow,
}

impl InkTool {
    fn width(self) -> f32 {
        match self {
            InkTool::Highlighter => 14.,
            InkTool::Pen | InkTool::Arrow => 3.,
        }
    }

    fn opacity(self) -> f32 {
        match self {
            InkTool::Highlighter => 0.4,
            InkTool::Pen | InkTool::Arrow => 1.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkAction {
    Tool(InkTool),
    Colour(usize),
    Undo,
    Clear,
    Leave,
}

pub fn ink_action_for_key(key: &str, control: bool) -> Option<InkAction> {
    match (key, control) {
        ("p", false) => Some(InkAction::Tool(InkTool::Pen)),
        ("h", false) => Some(InkAction::Tool(InkTool::Highlighter)),
        ("a", false) => Some(InkAction::Tool(InkTool::Arrow)),
        ("1", false) => Some(InkAction::Colour(0)),
        ("2", false) => Some(InkAction::Colour(1)),
        ("3", false) => Some(InkAction::Colour(2)),
        ("z", true) => Some(InkAction::Undo),
        ("delete" | "backspace", false) => Some(InkAction::Clear),
        ("escape", false) => Some(InkAction::Leave),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub display: usize,
    pub tool: InkTool,
    pub colour: usize,
    pub points: Vec<(f32, f32)>,
}

#[derive(Debug)]
pub struct Ink {
    tool: InkTool,
    colour: usize,
    strokes: Vec<Stroke>,
    drawing: bool,
}

impl Default for Ink {
    fn default() -> Self {
        Self {
            tool: InkTool::Pen,
            colour: 0,
            strokes: Vec::new(),
            drawing: false,
        }
    }
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Ink {
    pub fn tool(&self) -> InkTool {
        self.tool
    }

    pub fn colour(&self) -> usize {
        self.colour
    }

    pub fn set_tool(&mut self, tool: InkTool) {
        self.tool = tool;
    }

    pub fn set_colour(&mut self, colour: usize) {
        if colour < INK_COLOURS {
            self.colour = colour;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    pub fn strokes_on(&self, display: usize) -> impl Iterator<Item = &Stroke> {
        self.strokes
            .iter()
            .filter(move |stroke| stroke.display == display)
    }

    pub fn begin(&mut self, display: usize, at: (f32, f32)) {
        self.finish();
        self.strokes.push(Stroke {
            display,
            tool: self.tool,
            colour: self.colour,
            points: vec![at],
        });
        self.drawing = true;
    }

    pub fn extend(&mut self, at: (f32, f32)) -> bool {
        if !self.drawing {
            return false;
        }
        let Some(stroke) = self.strokes.last_mut() else {
            return false;
        };
        let Some(last) = stroke.points.last().copied() else {
            return false;
        };
        if distance(last, at) < MIN_STEP {
            return false;
        }
        if stroke.tool == InkTool::Arrow {
            stroke.points.truncate(1);
        }
        stroke.points.push(at);
        true
    }

    pub fn finish(&mut self) -> Option<usize> {
        if !std::mem::take(&mut self.drawing) {
            return None;
        }
        let stroke = self.strokes.last()?;
        let too_short = stroke.tool == InkTool::Arrow
            && stroke
                .points
                .first()
                .zip(stroke.points.last())
                .is_none_or(|(start, end)| distance(*start, *end) < MIN_ARROW);
        if too_short {
            return self.strokes.pop().map(|stroke| stroke.display);
        }
        None
    }

    pub fn undo(&mut self) -> Option<usize> {
        self.drawing = false;
        self.strokes.pop().map(|stroke| stroke.display)
    }

    pub fn clear(&mut self) -> bool {
        self.drawing = false;
        let had = !self.strokes.is_empty();
        self.strokes.clear();
        had
    }
}

pub fn arrow_head(start: (f32, f32), end: (f32, f32)) -> [(f32, f32); 2] {
    let angle = (end.1 - start.1).atan2(end.0 - start.0);
    let length = ARROW_HEAD.min(distance(start, end));
    [angle + ARROW_SPREAD, angle - ARROW_SPREAD]
        .map(|wing| (end.0 - length * wing.cos(), end.1 - length * wing.sin()))
}

pub fn can_draw(phase: Phase) -> bool {
    matches!(phase, Phase::Recording { .. })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InkChanged {
    pub display: Option<usize>,
}

pub struct ScreenInk {
    ink: Ink,
    active: bool,
    windows: Vec<WindowHandle<InkOverlay>>,
    previous_foreground: Option<isize>,
}

impl EventEmitter<InkChanged> for ScreenInk {}

struct ScreenInkGlobal(Entity<ScreenInk>);

impl Global for ScreenInkGlobal {}

impl ScreenInk {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let ink = cx.new(|_| Self {
            ink: Ink::default(),
            active: false,
            windows: Vec::new(),
            previous_foreground: None,
        });
        let session = RecordingSession::global(cx);
        let observed = ink.clone();
        cx.observe(&session, move |session, cx| {
            if can_draw(session.read(cx).phase) {
                return;
            }
            let idle = {
                let ink = observed.read(cx);
                !ink.active && ink.ink.is_empty() && ink.windows.is_empty()
            };
            if !idle {
                end(cx);
            }
        })
        .detach();
        cx.set_global(ScreenInkGlobal(ink.clone()));
        ink
    }

    pub fn try_global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<ScreenInkGlobal>()
            .map(|global| global.0.clone())
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn ink(&self) -> &Ink {
        &self.ink
    }

    pub fn apply(&mut self, action: InkAction, cx: &mut Context<Self>) {
        match action {
            InkAction::Tool(tool) => {
                self.ink.set_tool(tool);
                cx.notify();
            }
            InkAction::Colour(colour) => {
                self.ink.set_colour(colour);
                cx.notify();
            }
            InkAction::Undo => {
                if let Some(display) = self.ink.undo() {
                    cx.emit(InkChanged {
                        display: Some(display),
                    });
                }
            }
            InkAction::Clear => {
                if self.ink.clear() {
                    cx.emit(InkChanged { display: None });
                }
            }
            InkAction::Leave => cx.defer(|cx| set_active(false, cx)),
        }
    }

    fn begin(&mut self, display: usize, at: (f32, f32), cx: &mut Context<Self>) {
        if !self.active {
            return;
        }
        self.ink.begin(display, at);
        cx.emit(InkChanged {
            display: Some(display),
        });
    }

    fn extend(&mut self, display: usize, at: (f32, f32), cx: &mut Context<Self>) {
        if self.ink.extend(at) {
            cx.emit(InkChanged {
                display: Some(display),
            });
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        if let Some(display) = self.ink.finish() {
            cx.emit(InkChanged {
                display: Some(display),
            });
        }
    }
}

pub fn toggle(cx: &mut App) {
    let Some(ink) = ScreenInk::try_global(cx) else {
        return;
    };
    let active = ink.read(cx).active;
    set_active(!active, cx);
}

pub fn set_active(active: bool, cx: &mut App) {
    let Some(ink) = ScreenInk::try_global(cx) else {
        return;
    };
    if active && !can_draw(RecordingSession::global(cx).read(cx).phase) {
        return;
    }
    if ink.read(cx).active == active {
        return;
    }
    if active && ink.read(cx).windows.is_empty() {
        let windows = crate::app_windows::open_ink_overlays(ink.clone(), cx);
        ink.update(cx, |ink, _| ink.windows = windows);
    }
    let windows = ink.read(cx).windows.clone();
    let previous = if active {
        crate::platform::foreground_window()
    } else {
        ink.read(cx).previous_foreground
    };
    ink.update(cx, |ink, cx| {
        ink.active = active;
        ink.finish(cx);
        ink.previous_foreground = active.then_some(previous).flatten();
        cx.notify();
        cx.emit(InkChanged { display: None });
    });
    crate::app_windows::set_ink_overlays_interactive(&windows, active, cx);
    if !active
        && !windows.is_empty()
        && let Some(previous) = previous
    {
        crate::platform::restore_foreground_window(previous);
    }
}

fn end(cx: &mut App) {
    let Some(ink) = ScreenInk::try_global(cx) else {
        return;
    };
    let windows = ink.update(cx, |ink, cx| {
        ink.active = false;
        ink.previous_foreground = None;
        ink.ink = Ink {
            tool: ink.ink.tool(),
            colour: ink.ink.colour(),
            ..Ink::default()
        };
        cx.notify();
        std::mem::take(&mut ink.windows)
    });
    cx.defer(move |cx| {
        for window in windows {
            window
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
    });
}

pub struct InkOverlay {
    display: usize,
    ink: Entity<ScreenInk>,
    focus: FocusHandle,
}

impl InkOverlay {
    pub fn new(
        display: usize,
        ink: Entity<ScreenInk>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::theme::bind_window(window, cx);
        cx.subscribe(&ink, |this: &mut Self, _, change: &InkChanged, cx| {
            if change.display.is_none_or(|display| display == this.display) {
                cx.notify();
            }
        })
        .detach();
        Self {
            display,
            ink,
            focus: cx.focus_handle(),
        }
    }

    pub fn take_keyboard(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        window.activate_window();
    }
}

fn ink_colour(theme: &Theme, colour: usize) -> Hsla {
    match colour {
        1 => theme.danger.into(),
        2 => theme.warning.into(),
        _ => theme.primary.into(),
    }
}

pub fn ink_swatch(theme: &Theme, colour: usize) -> Hsla {
    ink_colour(theme, colour)
}

fn paint_stroke(stroke: &Stroke, theme: &Theme, window: &mut Window) {
    let Some(start) = stroke.points.first().copied() else {
        return;
    };
    let end = stroke.points.last().copied().unwrap_or(start);
    let width = stroke.tool.width();
    let mut colour = ink_colour(theme, stroke.colour);
    colour.a = stroke.tool.opacity();
    let mut builder = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(
        StrokeOptions::default()
            .with_line_width(width)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round),
    ));
    builder.move_to(point(px(start.0), px(start.1)));
    if stroke.points.len() == 1 {
        builder.line_to(point(px(start.0 + 0.1), px(start.1)));
    }
    for at in &stroke.points[1..] {
        builder.line_to(point(px(at.0), px(at.1)));
    }
    if stroke.tool == InkTool::Arrow && stroke.points.len() > 1 {
        let [left, right] = arrow_head(start, end);
        builder.move_to(point(px(left.0), px(left.1)));
        builder.line_to(point(px(end.0), px(end.1)));
        builder.line_to(point(px(right.0), px(right.1)));
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

impl Render for InkOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window, cx, false);
        let display = self.display;
        let state = self.ink.read(cx);
        let active = state.active;
        let strokes: Vec<Stroke> = state.ink.strokes_on(display).cloned().collect();
        div()
            .id("screen-ink")
            .track_focus(&self.focus)
            .size_full()
            .when(active, |this| {
                this.cursor(CursorStyle::Crosshair)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus, cx);
                            let at = (f32::from(event.position.x), f32::from(event.position.y));
                            this.ink
                                .update(cx, |ink, cx| ink.begin(this.display, at, cx));
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        if event.pressed_button != Some(MouseButton::Left) {
                            return;
                        }
                        let at = (f32::from(event.position.x), f32::from(event.position.y));
                        this.ink
                            .update(cx, |ink, cx| ink.extend(this.display, at, cx));
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.ink.update(cx, |ink, cx| ink.finish(cx));
                        }),
                    )
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        let keystroke = &event.keystroke;
                        let Some(action) = ink_action_for_key(
                            keystroke.key.as_str(),
                            keystroke.modifiers.control || keystroke.modifiers.platform,
                        ) else {
                            return;
                        };
                        this.ink.update(cx, |ink, cx| ink.apply(action, cx));
                        cx.stop_propagation();
                    }))
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, (), window, _| {
                        for stroke in &strokes {
                            paint_stroke(stroke, &theme, window);
                        }
                    },
                )
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(ink: &mut Ink, display: usize, points: &[(f32, f32)]) {
        ink.begin(display, points[0]);
        for at in &points[1..] {
            ink.extend(*at);
        }
        ink.finish();
    }

    #[test]
    fn screen_ink_keeps_each_stroke_on_the_display_it_was_drawn_on() {
        let mut ink = Ink::default();
        drawn(&mut ink, 0, &[(10., 10.), (20., 20.)]);
        drawn(&mut ink, 1, &[(5., 5.), (50., 5.)]);
        assert_eq!(ink.strokes_on(0).count(), 1);
        assert_eq!(
            ink.strokes_on(1).next().unwrap().points,
            vec![(5., 5.), (50., 5.)]
        );
        assert_eq!(ink.strokes_on(2).count(), 0);
    }

    #[test]
    fn screen_ink_strokes_keep_the_tool_and_colour_they_were_drawn_with() {
        let mut ink = Ink::default();
        ink.set_tool(InkTool::Highlighter);
        ink.set_colour(2);
        drawn(&mut ink, 0, &[(0., 0.), (30., 0.)]);
        ink.set_tool(InkTool::Pen);
        ink.set_colour(7);
        assert_eq!(ink.colour(), 2);
        ink.set_colour(1);
        let stroke = ink.strokes_on(0).next().unwrap();
        assert_eq!((stroke.tool, stroke.colour), (InkTool::Highlighter, 2));
    }

    #[test]
    fn screen_ink_ignores_pointer_jitter_and_moves_without_a_stroke() {
        let mut ink = Ink::default();
        assert!(!ink.extend((4., 4.)));
        ink.begin(0, (10., 10.));
        assert!(!ink.extend((10.5, 10.5)));
        assert!(ink.extend((13., 10.)));
        ink.finish();
        assert!(!ink.extend((40., 40.)));
        assert_eq!(
            ink.strokes_on(0).next().unwrap().points,
            vec![(10., 10.), (13., 10.)]
        );
    }

    #[test]
    fn screen_ink_arrow_is_one_segment_and_a_click_leaves_none() {
        let mut ink = Ink::default();
        ink.set_tool(InkTool::Arrow);
        drawn(&mut ink, 0, &[(0., 0.), (20., 5.), (60., 40.), (100., 0.)]);
        assert_eq!(
            ink.strokes_on(0).next().unwrap().points,
            vec![(0., 0.), (100., 0.)]
        );
        drawn(&mut ink, 0, &[(200., 200.), (202., 201.)]);
        assert_eq!(ink.strokes_on(0).count(), 1);
    }

    #[test]
    fn screen_ink_undo_removes_the_latest_stroke_and_clear_removes_all() {
        let mut ink = Ink::default();
        drawn(&mut ink, 0, &[(0., 0.), (9., 9.)]);
        drawn(&mut ink, 1, &[(0., 0.), (9., 9.)]);
        drawn(&mut ink, 0, &[(20., 0.), (29., 9.)]);
        assert_eq!(ink.undo(), Some(0));
        assert_eq!(ink.undo(), Some(1));
        assert_eq!(ink.strokes_on(0).count(), 1);
        assert!(ink.clear());
        assert!(ink.is_empty());
        assert!(!ink.clear());
        assert_eq!(ink.undo(), None);
    }

    #[test]
    fn screen_ink_arrow_head_opens_back_from_the_tip() {
        let [left, right] = arrow_head((0., 0.), (100., 0.));
        assert!((left.0 - 87.65).abs() < 0.05, "{left:?}");
        assert!((left.1 + 6.59).abs() < 0.05, "{left:?}");
        assert!((right.0 - 87.65).abs() < 0.05, "{right:?}");
        assert!((right.1 - 6.59).abs() < 0.05, "{right:?}");
        let [up, _] = arrow_head((0., 0.), (0., 5.));
        assert!(up.1 >= 0. && up.1 < 5.);
    }

    #[test]
    fn screen_ink_every_action_has_a_key() {
        assert_eq!(
            ink_action_for_key("p", false),
            Some(InkAction::Tool(InkTool::Pen))
        );
        assert_eq!(
            ink_action_for_key("h", false),
            Some(InkAction::Tool(InkTool::Highlighter))
        );
        assert_eq!(
            ink_action_for_key("a", false),
            Some(InkAction::Tool(InkTool::Arrow))
        );
        assert_eq!(ink_action_for_key("1", false), Some(InkAction::Colour(0)));
        assert_eq!(ink_action_for_key("3", false), Some(InkAction::Colour(2)));
        assert_eq!(ink_action_for_key("z", true), Some(InkAction::Undo));
        assert_eq!(ink_action_for_key("delete", false), Some(InkAction::Clear));
        assert_eq!(ink_action_for_key("escape", false), Some(InkAction::Leave));
        assert_eq!(ink_action_for_key("z", false), None);
        assert_eq!(ink_action_for_key("p", true), None);
    }

    fn recording(cx: &mut gpui::TestAppContext, phase: Phase) -> Entity<ScreenInk> {
        cx.update(|cx| {
            crate::feeds::Feeds::init(cx);
            let session = RecordingSession::init(cx);
            session.update(cx, |session, _| session.phase = phase);
            ScreenInk::init(cx)
        })
    }

    #[gpui::test]
    fn screen_ink_draw_mode_only_starts_while_recording(cx: &mut gpui::TestAppContext) {
        let ink = recording(cx, Phase::Starting);
        cx.update(|cx| toggle(cx));
        assert!(!cx.read(|cx| ink.read(cx).active()));
        cx.update(|cx| {
            RecordingSession::global(cx).update(cx, |session, cx| {
                session.phase = Phase::Recording { paused: true };
                cx.notify();
            });
            toggle(cx);
        });
        assert!(cx.read(|cx| ink.read(cx).active()));
        cx.update(|cx| toggle(cx));
        assert!(!cx.read(|cx| ink.read(cx).active()));
    }

    #[gpui::test]
    fn screen_ink_strokes_stay_after_leaving_draw_mode_and_go_when_the_recording_ends(
        cx: &mut gpui::TestAppContext,
    ) {
        let ink = recording(cx, Phase::Recording { paused: false });
        cx.update(|cx| {
            set_active(true, cx);
            ink.update(cx, |ink, cx| {
                ink.begin(0, (1., 1.), cx);
                ink.extend(0, (40., 40.), cx);
                ink.finish(cx);
                ink.apply(InkAction::Leave, cx);
            });
        });
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(!ink.read(cx).active());
            assert_eq!(ink.read(cx).ink().strokes_on(0).count(), 1);
        });
        cx.update(|cx| {
            ink.update(cx, |ink, cx| ink.begin(0, (5., 5.), cx));
            assert_eq!(ink.read(cx).ink().strokes_on(0).count(), 1);
            set_active(true, cx);
            RecordingSession::global(cx).update(cx, |session, cx| {
                session.phase = Phase::Stopping;
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(!ink.read(cx).active());
            assert!(ink.read(cx).ink().is_empty());
        });
    }

    #[gpui::test]
    fn screen_ink_only_the_display_drawn_on_is_asked_to_repaint(cx: &mut gpui::TestAppContext) {
        let ink = recording(cx, Phase::Recording { paused: false });
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update(|cx| {
            set_active(true, cx);
            let seen = seen.clone();
            cx.subscribe(&ink, move |_, change: &InkChanged, _| {
                seen.borrow_mut().push(change.display)
            })
            .detach();
            ink.update(cx, |ink, cx| {
                ink.begin(1, (1., 1.), cx);
                ink.extend(1, (1.2, 1.2), cx);
                ink.extend(1, (30., 30.), cx);
                ink.finish(cx);
                ink.apply(InkAction::Tool(InkTool::Arrow), cx);
                ink.apply(InkAction::Colour(1), cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(*seen.borrow(), vec![Some(1), Some(1)]);
    }
}
