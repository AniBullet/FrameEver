mod model;

use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, FontWeight, HighlightStyle, InteractiveElement,
    IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Styled, StyledText, Window, div,
    prelude::FluentBuilder, px, svg,
};

use crate::{
    feeds::Feeds,
    i18n::{t, tc},
    platform,
    session::{Phase, RecordingSession},
    store::{self, TeleprompterAlign, TeleprompterScroll, TeleprompterState},
    teleprompter::count_words,
    theme::Theme,
    ui,
};

use model::{
    CaptureInputs, CaptureStatus, Edge, PlaybackState, ReadingKey, RecordingCue, VoiceGate,
};
pub use model::{edit_frame, opening_frame};

const SAVE_DEBOUNCE: Duration = Duration::from_millis(250);
const PLAYBACK_TICK: Duration = Duration::from_millis(16);
const MAX_TICK_SECONDS: f32 = 0.05;
const TITLE_HEIGHT: f32 = 32.;
const FOOT_HEIGHT: f32 = 44.;
const TOOLBAR_HEIGHT: f32 = 36.;
const EDGE_HIT: f32 = 6.;
const TEXT_PADDING: f32 = 32.;
pub const WINDOW_TITLE: &str = "FrameEver Teleprompter";

pub fn calculate_playback_speed(
    maximum_scroll: f32,
    word_count: usize,
    words_per_minute: u32,
) -> f32 {
    let duration_seconds = (word_count.max(1) as f32 / words_per_minute.max(1) as f32) * 60.;
    maximum_scroll.max(0.) / duration_seconds.max(1.)
}

pub fn advance_playback_position(
    position: f32,
    maximum_scroll: f32,
    pixels_per_second: f32,
    elapsed_seconds: f32,
) -> f32 {
    (position.max(0.) + pixels_per_second.max(0.) * elapsed_seconds.max(0.))
        .min(maximum_scroll.max(0.))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Read,
    Edit,
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    Move {
        pointer: Point<Pixels>,
        frame: [f32; 4],
    },
    Line {
        pointer_y: f32,
        band_top: f32,
    },
    Resize {
        edge: Edge,
        pointer: Point<Pixels>,
        frame: [f32; 4],
    },
}

#[derive(Default)]
struct PendingSave {
    state: TeleprompterState,
    keys: BTreeSet<&'static str>,
}

impl PendingSave {
    fn flush(&mut self) {
        for key in std::mem::take(&mut self.keys) {
            if !store::set_store_setting(store::TELEPROMPTER, key, self.state.value_for(key)) {
                tracing::warn!(key, "the store refused the teleprompter write");
            }
        }
    }
}

pub struct TeleprompterWindow {
    theme: Theme,
    state: TeleprompterState,
    view: View,
    options_open: bool,
    playback_state: PlaybackState,
    position: f32,
    ended: bool,
    text_height: Rc<Cell<f32>>,
    passed_until: Rc<Cell<usize>>,
    focus: FocusHandle,
    script_input: Entity<ui::TextInputState>,
    _script_events: gpui::Subscription,
    _recording_events: gpui::Subscription,
    save: Rc<RefCell<PendingSave>>,
    save_task: Option<gpui::Task<()>>,
    playback: Option<gpui::Task<()>>,
    hovered: bool,
    drag: Option<Drag>,
    voice: VoiceGate,
    speaking: bool,
    hiding_requested: bool,
    hidden_confirmed: Option<bool>,
    streamed: bool,
    frame: [f32; 4],
    read_frame: [f32; 4],
    full_screen: Option<[f32; 4]>,
    other_display: Option<[f32; 4]>,
    frame_generation: Arc<AtomicU64>,
}

fn frame_of(window: &Window) -> [f32; 4] {
    let bounds = window.bounds();
    [
        bounds.origin.x.into(),
        bounds.origin.y.into(),
        bounds.size.width.into(),
        bounds.size.height.into(),
    ]
}

fn centre(frame: [f32; 4]) -> (f32, f32) {
    (frame[0] + frame[2] / 2., frame[1] + frame[3] / 2.)
}

pub fn work_area_for(frame: [f32; 4], cx: &gpui::App) -> Option<[f32; 4]> {
    let (x, y) = centre(frame);
    let display = crate::camera_window::display_for_point(f64::from(x), f64::from(y));
    let work = crate::app_windows::display_work_area(display.as_ref(), cx)?;
    Some([
        work.origin.x.into(),
        work.origin.y.into(),
        work.size.width.into(),
        work.size.height.into(),
    ])
}

fn other_display_bounds(frame: [f32; 4]) -> Option<[f32; 4]> {
    let (x, y) = centre(frame);
    scap_targets::Display::list()
        .into_iter()
        .filter_map(|display| display.raw_handle().logical_bounds())
        .map(|bounds| {
            [
                bounds.position().x() as f32,
                bounds.position().y() as f32,
                bounds.size().width() as f32,
                bounds.size().height() as f32,
            ]
        })
        .find(|[left, top, width, height]| {
            !(x >= *left && x < left + width && y >= *top && y < top + height)
        })
}

impl TeleprompterWindow {
    pub fn new(read_frame: [f32; 4], window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state = TeleprompterState::load();
        state.line_height = model::nearest_line_spacing(state.line_height);
        state.font_size = model::step_font(state.font_size, 0.);
        state.reading_line = model::clamp_reading_line(state.reading_line);
        let save = Rc::new(RefCell::new(PendingSave {
            state: state.clone(),
            keys: BTreeSet::new(),
        }));
        let closing = save.clone();
        window.on_window_should_close(cx, move |_window, cx| {
            closing.borrow_mut().flush();
            cx.defer(crate::app_windows::teleprompter_closed);
            true
        });

        let script_input = cx.new(|cx| {
            let mut input = ui::TextInputState::multi_line(window, cx);
            input.set_text(state.script.clone(), cx);
            input.set_placeholder(t!("Paste or type your script…"));
            input
        });
        let script_events = cx.subscribe_in(
            &script_input,
            window,
            |this: &mut Self, input, event: &ui::TextInputEvent, window, cx| match event {
                ui::TextInputEvent::Changed => {
                    let script = input.read(cx).text().to_string();
                    this.edit_script(move |value| *value = script, window, cx);
                }
                ui::TextInputEvent::Cancelled => this.escape(window, cx),
                _ => {}
            },
        );

        let session = RecordingSession::global(cx);
        let mut playback_state = PlaybackState::default();
        playback_state.update_recording(
            session.read(cx).phase,
            session.read(cx).pause_uncertain(),
            false,
        );
        let recording_events = cx.observe_in(&session, window, |this, session, window, cx| {
            let (phase, uncertain) = {
                let session = session.read(cx);
                (session.phase, session.pause_uncertain())
            };
            let was_running = this.playback_state.running();
            let start_with =
                this.state.start_with_recording && this.view == View::Read && this.has_script();
            let cue = this
                .playback_state
                .update_recording(phase, uncertain, start_with);
            if cue == RecordingCue::StartFromTop {
                this.position = 0.;
                this.ended = false;
                this.voice.reset();
            }
            if !this.playback_state.running() {
                this.playback = None;
            } else if !was_running {
                this.start_playback(window, cx);
            }
            this.sync_capture_protection(false, window, cx);
            cx.notify();
            window.refresh();
        });

        crate::theme::bind_window(window, cx);
        let frame = frame_of(window);
        let view = if state.script.trim().is_empty() {
            View::Edit
        } else {
            View::Read
        };
        Self {
            theme: Theme::for_window(window, cx, false),
            state,
            view,
            options_open: false,
            playback_state,
            position: 0.,
            ended: false,
            text_height: Rc::new(Cell::new(0.)),
            passed_until: Rc::new(Cell::new(0)),
            focus: cx.focus_handle(),
            script_input,
            _script_events: script_events,
            _recording_events: recording_events,
            save,
            save_task: None,
            playback: None,
            hovered: false,
            drag: None,
            voice: VoiceGate::default(),
            speaking: false,
            hiding_requested: false,
            hidden_confirmed: None,
            streamed: false,
            frame,
            read_frame,
            full_screen: None,
            other_display: other_display_bounds(frame),
            frame_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn focus_root(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = match self.view {
            View::Edit => self.script_input.read(cx).focus_handle(),
            View::Read => self.focus.clone(),
        };
        window.focus(&focus, cx);
    }

    pub fn hides_from_sharing(&self) -> bool {
        self.state.hide_from_sharing
    }

    pub fn sync_capture_protection(
        &mut self,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let recording = RecordingSession::global(cx).read(cx).phase != Phase::Idle;
        let hidden = force || recording || self.state.hide_from_sharing;
        self.hiding_requested = hidden;
        let Some(native) = platform::native_window(window) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let readback = platform::set_window_capture_hidden(&native, hidden);
            #[cfg(target_os = "windows")]
            let streamed = platform::window_capture_exclusion_unavailable_reason().is_some();
            #[cfg(not(target_os = "windows"))]
            let streamed = false;
            tracing::info!(
                hidden,
                readback,
                streamed,
                "teleprompter capture protection"
            );
            this.update(cx, |this, cx| {
                if this.hiding_requested == hidden {
                    this.hidden_confirmed = Some(model::readback_hides(readback));
                }
                this.streamed = streamed;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn capture_status(&self, cx: &gpui::App) -> CaptureStatus {
        let session = RecordingSession::global(cx);
        let session = session.read(cx);
        model::capture_status(CaptureInputs {
            phase: session.phase,
            pause_uncertain: session.pause_uncertain(),
            hide_from_sharing: self.state.hide_from_sharing,
            hiding_requested: self.hiding_requested,
            hidden_confirmed: self.hidden_confirmed,
            streamed: self.streamed,
        })
    }

    fn touch(&mut self, key: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        {
            let mut save = self.save.borrow_mut();
            save.state = self.state.clone();
            save.keys.insert(key);
        }
        let save = self.save.clone();
        self.save_task = Some(cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            save.borrow_mut().flush();
        }));
    }

    fn has_script(&self) -> bool {
        !self.state.script.trim().is_empty()
    }

    fn edit_script(
        &mut self,
        edit: impl FnOnce(&mut String),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        edit(&mut self.state.script);
        if !self.has_script() {
            self.stop_playback();
        }
        self.position = 0.;
        self.ended = false;
        self.passed_until.set(0);
        self.touch(TeleprompterState::SCRIPT, window, cx);
        self.touch(TeleprompterState::CHARACTERS_PER_MINUTE, window, cx);
        cx.notify();
    }

    pub fn type_script(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut next = self.state.script.clone();
        next.push_str(text);
        self.script_input
            .update(cx, |input, cx| input.set_text(next.clone(), cx));
        self.edit_script(move |script| *script = next, window, cx);
    }

    fn change_font_size(&mut self, steps: f32, window: &mut Window, cx: &mut Context<Self>) {
        let next = model::step_font(self.state.font_size, steps);
        if next == self.state.font_size {
            return;
        }
        self.state.font_size = next;
        self.touch(TeleprompterState::FONT_SIZE, window, cx);
        cx.notify();
    }

    fn change_speed(&mut self, steps: i32, window: &mut Window, cx: &mut Context<Self>) {
        let next = model::step_speed(
            self.state.reading_speed(),
            steps,
            self.state.uses_chinese_speed(),
        );
        if next == self.state.reading_speed() {
            return;
        }
        let key = self.state.set_reading_speed(next);
        self.touch(key, window, cx);
        cx.notify();
    }

    fn maximum_scroll(&self) -> f32 {
        self.text_height.get().max(0.)
    }

    fn line_height(&self) -> f32 {
        (self.state.font_size * self.state.line_height).round()
    }

    fn total_seconds(&self) -> u32 {
        model::total_seconds(count_words(&self.state.script), self.state.reading_speed())
    }

    fn stop_playback(&mut self) {
        self.playback_state.requested = false;
        self.playback = None;
    }

    pub fn is_playing(&self) -> bool {
        self.playback_state.running()
    }

    pub fn toggle_playback(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == View::Edit {
            self.show_read(window, cx);
            if !self.has_script() {
                return;
            }
            self.position = 0.;
            self.ended = false;
        }
        if self.playback_state.requested {
            self.stop_playback();
            cx.notify();
            return;
        }
        if !self.has_script() {
            return;
        }
        if self.ended || self.position >= self.maximum_scroll() - 0.5 {
            self.position = 0.;
            self.ended = false;
            self.passed_until.set(0);
        }
        self.options_open = false;
        self.voice.reset();
        self.playback_state.requested = true;
        self.start_playback(window, cx);
        cx.notify();
    }

    fn start_playback(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.playback_state.running() {
            return;
        }
        self.playback = Some(cx.spawn_in(window, async move |this, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor().timer(PLAYBACK_TICK).await;
                let now = Instant::now();
                let elapsed = now
                    .saturating_duration_since(last)
                    .as_secs_f32()
                    .min(MAX_TICK_SECONDS);
                last = now;
                let running = this
                    .update_in(cx, |this, window, cx| {
                        let running = this.advance(elapsed, cx);
                        cx.notify();
                        window.refresh();
                        running
                    })
                    .unwrap_or(false);
                if !running {
                    return;
                }
            }
        }));
        cx.notify();
    }

    fn microphone_level(cx: &gpui::App) -> Option<f64> {
        let feeds = Feeds::global(cx);
        let feeds = feeds.read(cx);
        feeds.microphone.as_ref()?;
        Some(feeds.mic_level().read(cx).db())
    }

    fn advance(&mut self, elapsed: f32, cx: &gpui::App) -> bool {
        if !self.playback_state.running() || self.view != View::Read {
            return false;
        }
        let maximum = self.maximum_scroll();
        if maximum <= 1. {
            return true;
        }
        if self.state.scroll == TeleprompterScroll::Voice {
            self.speaking = match Self::microphone_level(cx) {
                Some(level) => self.voice.update(level, elapsed),
                None => false,
            };
            if !self.speaking {
                return true;
            }
        }
        let speed = calculate_playback_speed(
            maximum,
            count_words(&self.state.script),
            self.state.reading_speed(),
        );
        self.position = advance_playback_position(self.position, maximum, speed, elapsed);
        if self.position >= maximum - 0.5 {
            self.position = maximum;
            self.ended = true;
            self.playback_state.requested = false;
            return false;
        }
        true
    }

    fn jump_lines(&mut self, lines: f32, cx: &mut Context<Self>) {
        self.position = model::jump(
            self.position,
            lines,
            self.line_height(),
            self.maximum_scroll(),
        );
        self.ended = false;
        cx.notify();
    }

    fn to_top(&mut self, cx: &mut Context<Self>) {
        self.position = 0.;
        self.ended = false;
        self.passed_until.set(0);
        cx.notify();
    }

    fn apply_frame(&mut self, frame: [f32; 4], window: &mut Window, cx: &mut Context<Self>) {
        self.frame = frame;
        let Some(native) = platform::native_window(window) else {
            tracing::warn!("teleprompter move skipped because its native window is unavailable");
            return;
        };
        let generation = self.frame_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let generations = self.frame_generation.clone();
        let [x, y, width, height] = frame;
        #[cfg(target_os = "windows")]
        cx.spawn(async move |_, _| {
            if generations.load(Ordering::Acquire) == generation {
                platform::set_window_logical_frame(
                    &native,
                    x.into(),
                    y.into(),
                    width.into(),
                    height.into(),
                );
            }
        })
        .detach();
        #[cfg(not(target_os = "windows"))]
        {
            let old = window.bounds();
            let (frame_x, frame_y, _, frame_height) = platform::window_frame(&native);
            let native_x = frame_x + f64::from(x - f32::from(old.origin.x));
            let native_y =
                frame_y + frame_height - f64::from(y - f32::from(old.origin.y)) - f64::from(height);
            cx.spawn(async move |_, _| {
                if generations.load(Ordering::Acquire) == generation {
                    platform::set_window_frame(
                        &native,
                        native_x,
                        native_y,
                        width.into(),
                        height.into(),
                    );
                }
            })
            .detach();
        }
        cx.notify();
    }

    fn remember_read_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.full_screen.is_some() || self.view != View::Read {
            return;
        }
        self.read_frame = self.frame;
        self.state.frame = Some(self.frame);
        self.touch(TeleprompterState::FRAME, window, cx);
    }

    fn show_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == View::Edit {
            return;
        }
        self.leave_full_screen(window, cx);
        self.stop_playback();
        self.read_frame = self.frame;
        self.view = View::Edit;
        if let Some(work) = work_area_for(self.frame, cx) {
            self.apply_frame(model::edit_frame(self.read_frame, work), window, cx);
        }
        let focus = self.script_input.read(cx).focus_handle();
        window.focus(&focus, cx);
        cx.notify();
    }

    fn show_read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == View::Read {
            return;
        }
        self.view = View::Read;
        self.options_open = false;
        let [x, y, _, _] = self.frame;
        let [_, _, width, height] = self.read_frame;
        if let Some(work) = work_area_for(self.frame, cx) {
            let read = model::strip_frame([x, y, width, height], work, false);
            self.apply_frame(read, window, cx);
            self.remember_read_frame(window, cx);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn toggle_full_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.full_screen.is_some() {
            self.leave_full_screen(window, cx);
            return;
        }
        self.other_display = other_display_bounds(self.frame);
        let Some(display) = self.other_display else {
            return;
        };
        self.show_read(window, cx);
        self.full_screen = Some(self.frame);
        self.options_open = false;
        self.apply_frame(display, window, cx);
    }

    fn leave_full_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(previous) = self.full_screen.take() {
            self.apply_frame(previous, window, cx);
        }
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.options_open {
            self.options_open = false;
            cx.notify();
        } else if self.view == View::Edit {
            self.show_read(window, cx);
        } else {
            self.leave_full_screen(window, cx);
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_full_screen(window, cx);
        if self.view == View::Read {
            self.remember_read_frame(window, cx);
        } else {
            self.state.frame = Some([
                self.frame[0],
                self.frame[1],
                self.read_frame[2],
                self.read_frame[3],
            ]);
            self.touch(TeleprompterState::FRAME, window, cx);
        }
        self.save_task = None;
        self.save.borrow_mut().flush();
        window.remove_window();
        cx.defer(crate::app_windows::teleprompter_closed);
    }

    fn minimize(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(native) = platform::native_window(window) {
            platform::hide_native(&native);
        }
    }

    fn toggle_setting(
        &mut self,
        key: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        flip: impl FnOnce(&mut TeleprompterState),
    ) {
        flip(&mut self.state);
        self.touch(key, window, cx);
        if key == TeleprompterState::HIDE_FROM_SHARING {
            self.sync_capture_protection(false, window, cx);
        }
        cx.notify();
    }

    fn on_key(&mut self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if self.view == View::Edit {
            if keystroke.key == "escape" {
                self.escape(window, cx);
                cx.stop_propagation();
            }
            return;
        }
        let Some(key) = model::reading_key(
            keystroke.key.as_str(),
            keystroke.modifiers.control || keystroke.modifiers.platform,
            keystroke.modifiers.alt,
        ) else {
            if keystroke.key == "delete" {
                self.close(window, cx);
            }
            return;
        };
        match key {
            ReadingKey::TogglePlay => self.toggle_playback(window, cx),
            ReadingKey::Faster => self.change_speed(1, window, cx),
            ReadingKey::Slower => self.change_speed(-1, window, cx),
            ReadingKey::Larger => self.change_font_size(1., window, cx),
            ReadingKey::Smaller => self.change_font_size(-1., window, cx),
            ReadingKey::JumpDown => self.jump_lines(3., cx),
            ReadingKey::JumpUp => self.jump_lines(-3., cx),
            ReadingKey::Top => self.to_top(cx),
            ReadingKey::Edit => self.show_edit(window, cx),
            ReadingKey::FullScreen => self.toggle_full_screen(window, cx),
            ReadingKey::Leave => self.escape(window, cx),
        }
        cx.stop_propagation();
    }

    fn on_capture_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        if self.view == View::Edit
            && keystroke.key == "enter"
            && (keystroke.modifiers.control || keystroke.modifiers.platform)
        {
            self.toggle_playback(window, cx);
            cx.stop_propagation();
        }
    }

    fn pointer_global(window: &Window, point: Point<Pixels>) -> Point<Pixels> {
        window.bounds().origin + point
    }

    fn band_top(&self, window: &Window) -> f32 {
        model::band_top(
            f32::from(window.viewport_size().height),
            self.state.reading_line,
        )
    }

    fn begin_drag(
        &mut self,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.full_screen.is_some() || self.drag.is_some() {
            return;
        }
        let local_y = f32::from(event.position.y);
        let band = self.band_top(window);
        let on_band = self.view == View::Read
            && self.state.show_cue_markers
            && local_y >= band
            && local_y <= band + self.line_height();
        self.drag = Some(if on_band {
            Drag::Line {
                pointer_y: local_y,
                band_top: band,
            }
        } else {
            Drag::Move {
                pointer: Self::pointer_global(window, event.position),
                frame: self.frame,
            }
        });
        if self.view == View::Read {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn drag_to(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else {
            return;
        };
        match drag {
            Drag::Move { pointer, frame } => {
                let now = Self::pointer_global(window, event.position);
                let moved = [
                    frame[0] + f32::from(now.x - pointer.x),
                    frame[1] + f32::from(now.y - pointer.y),
                    frame[2],
                    frame[3],
                ];
                if let Some(work) = work_area_for(moved, cx) {
                    let next = model::strip_frame(moved, work, !event.modifiers.control);
                    if next != self.frame {
                        self.apply_frame(next, window, cx);
                    }
                }
            }
            Drag::Line {
                pointer_y,
                band_top,
            } => {
                let top = band_top + f32::from(event.position.y) - pointer_y;
                self.state.reading_line =
                    model::reading_line_for(top, f32::from(window.viewport_size().height));
                cx.notify();
            }
            Drag::Resize {
                edge,
                pointer,
                frame,
            } => {
                let now = Self::pointer_global(window, event.position);
                if let Some(work) = work_area_for(frame, cx) {
                    let next = model::resize_frame(
                        frame,
                        edge,
                        (f32::from(now.x - pointer.x), f32::from(now.y - pointer.y)),
                        work,
                    );
                    if next != self.frame {
                        self.apply_frame(next, window, cx);
                    }
                }
            }
        }
    }

    fn end_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        match drag {
            Drag::Line { .. } => self.touch(TeleprompterState::READING_LINE, window, cx),
            _ if self.view == View::Read => self.remember_read_frame(window, cx),
            _ => {}
        }
        cx.notify();
    }
}

impl Render for TeleprompterWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme.refresh(window, cx, false);
        let theme = self.theme;
        let attached = work_area_for(self.frame, cx)
            .is_some_and(|work| model::attached_to_top(self.frame, work))
            || self.full_screen.is_some();
        let radius = px(if self.full_screen.is_some() { 0. } else { 8. });

        div()
            .id("teleprompter-root")
            .track_focus(&self.focus)
            .key_context("Teleprompter")
            .on_key_down(cx.listener(Self::on_key))
            .capture_key_down(cx.listener(Self::on_capture_key))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if this.hovered != *hovered {
                    this.hovered = *hovered;
                    cx.notify();
                }
            }))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.card)
            .text_color(theme.foreground)
            .border_color(theme.border)
            .when(attached, |this| {
                this.border_l_1()
                    .border_r_1()
                    .border_b_1()
                    .rounded_b(radius)
            })
            .when(!attached, |this| this.border_1().rounded(radius))
            .font_family(crate::theme::ui_font())
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(13.))
            .when(self.drag.is_some(), |this| this.child(self.drag_layer(cx)))
            .map(|this| match self.view {
                View::Read => this.child(self.render_read(window, cx)),
                View::Edit => this
                    .child(self.render_title(cx))
                    .child(self.render_editor())
                    .child(self.render_foot(cx))
                    .children(self.render_options(cx)),
            })
            .when(
                self.view == View::Read && self.full_screen.is_none(),
                |this| this.child(self.render_resize_edges(attached, cx)),
            )
    }
}

impl TeleprompterWindow {
    fn drag_layer(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let moving = cx.entity().downgrade();
        let releasing = cx.entity().downgrade();
        gpui::canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let entity = moving.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != gpui::DispatchPhase::Bubble {
                        return;
                    }
                    entity
                        .update(cx, |this, cx| {
                            if event.dragging() {
                                this.drag_to(event, window, cx);
                            } else {
                                this.end_drag(window, cx);
                            }
                        })
                        .ok();
                });
                let entity = releasing.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase != gpui::DispatchPhase::Bubble || event.button != MouseButton::Left {
                        return;
                    }
                    entity.update(cx, |this, cx| this.end_drag(window, cx)).ok();
                });
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }

    fn icon_button(
        &self,
        id: &'static str,
        icon: &'static str,
        label: &'static str,
        on: bool,
        disabled: bool,
        cx: &mut Context<Self>,
        handler: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id(id)
            .flex_none()
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .text_color(theme.muted_foreground)
            .when(on, |this| {
                this.bg(theme.selection)
                    .text_color(theme.selection_foreground)
            })
            .when(disabled, |this| this.text_color(theme.faint_foreground))
            .when(!disabled, |this| {
                this.hover(|style| style.bg(theme.accent).text_color(theme.foreground))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        handler(this, window, cx);
                    }))
            })
            .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .child(svg().path(icon).size(px(16.)))
    }

    fn separator(&self) -> impl IntoElement {
        div()
            .flex_none()
            .w(px(1.))
            .h(px(20.))
            .mx(px(3.))
            .bg(self.theme.border)
    }

    fn caption(&self, text: impl Into<SharedString>, width: f32) -> impl IntoElement {
        div()
            .flex_none()
            .min_w(px(width))
            .px(px(6.))
            .text_center()
            .text_size(px(12.))
            .text_color(self.theme.muted_foreground)
            .whitespace_nowrap()
            .child(text.into())
    }

    fn status_line(&self, status: CaptureStatus) -> impl IntoElement {
        let theme = self.theme;
        let color = if status.warns() {
            theme.warning_text
        } else {
            theme.muted_foreground
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_size(px(12.))
            .text_color(color)
            .child(svg().path(status.icon()).size(px(14.)).flex_none())
            .child(status_label(status))
    }

    fn speed_label(&self) -> String {
        if self.state.uses_chinese_speed() {
            t!("{count} chars/min", count = self.state.reading_speed())
        } else {
            t!("{count} wpm", count = self.state.reading_speed())
        }
    }

    fn render_read(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let line = self.line_height();
        let band = self.band_top(window);
        let text_top = band - self.position;
        let script = self.state.script.clone();
        let passed = self.passed_until.get().min(script.len());
        let status = self.capture_status(cx);
        let playing = self.playback_state.requested;
        let show_controls = self.hovered || !playing || self.drag.is_some();

        let text: SharedString = if script.trim().is_empty() {
            t!("Press E to write your script").into()
        } else {
            script.clone().into()
        };
        let styled = if script.trim().is_empty() {
            StyledText::new(text).with_highlights(vec![(
                0..t!("Press E to write your script").len(),
                HighlightStyle {
                    color: Some(theme.muted_foreground.into()),
                    ..Default::default()
                },
            )])
        } else if passed > 0 && script.is_char_boundary(passed) {
            StyledText::new(text).with_highlights(vec![(
                0..passed,
                HighlightStyle {
                    color: Some(theme.faint_foreground.into()),
                    ..Default::default()
                },
            )])
        } else {
            StyledText::new(text)
        };
        let layout = styled.layout().clone();
        let ends = model::phrase_ends(&script);
        let height_cell = self.text_height.clone();
        let passed_cell = self.passed_until.clone();
        let entity = cx.entity().downgrade();
        let measure = gpui::canvas(
            move |bounds, _, cx| {
                height_cell.set(f32::from(bounds.size.height));
                let origin_y = f32::from(bounds.origin.y) - text_top;
                let tops: Vec<(usize, f32)> = ends
                    .iter()
                    .filter_map(|end| {
                        let point = layout.position_for_index(*end)?;
                        Some((*end, f32::from(point.y) - origin_y))
                    })
                    .collect();
                let next = model::passed_phrases(&tops, line, band);
                if next != passed_cell.get() {
                    passed_cell.set(next);
                    let entity = entity.clone();
                    cx.defer(move |cx| {
                        entity.update(cx, |_, cx| cx.notify()).ok();
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        div()
            .id("read")
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .cursor(gpui::CursorStyle::default())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.begin_drag(event, window, cx);
                }),
            )
            .when(self.state.show_cue_markers, |this| {
                this.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(band))
                        .h(px(line))
                        .bg(theme.selection)
                        .cursor(gpui::CursorStyle::ResizeUpDown),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(6.))
                        .top(px(band))
                        .h(px(line))
                        .flex()
                        .items_center()
                        .text_color(theme.muted_foreground)
                        .child(svg().path("icons/chevron-right.svg").size(px(14.))),
                )
                .child(
                    div()
                        .absolute()
                        .right(px(6.))
                        .top(px(band))
                        .h(px(line))
                        .flex()
                        .items_center()
                        .text_color(theme.muted_foreground)
                        .child(svg().path("icons/chevron-left.svg").size(px(14.))),
                )
            })
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(text_top))
                    .px(px(TEXT_PADDING))
                    .text_size(px(self.state.font_size))
                    .line_height(px(line))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .map(|this| match self.state.align {
                        TeleprompterAlign::Center => this.text_center(),
                        TeleprompterAlign::Left => this.text_left(),
                    })
                    .child(styled)
                    .child(measure),
            )
            .when(status.shows_notice(), |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(8.))
                        .bottom(px(8.))
                        .h(px(TOOLBAR_HEIGHT))
                        .max_w(px(290.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .rounded(px(20.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .child(self.status_line(status)),
                )
            })
            .when(show_controls, |this| this.child(self.render_toolbar(cx)))
            .when(self.full_screen.is_none(), |this| {
                this.child(
                    div()
                        .absolute()
                        .bottom(px(3.))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .w(px(32.))
                                .h(px(3.))
                                .rounded(px(2.))
                                .bg(theme.border_strong),
                        ),
                )
            })
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let has_script = self.has_script();
        let playing = self.playback_state.requested;
        let (play_icon, play_label) = if self.ended {
            ("icons/rotate-ccw.svg", t!("Start over  Space"))
        } else if playing {
            ("icons/pause.svg", t!("Pause  Space"))
        } else {
            ("icons/play.svg", t!("Play  Space"))
        };
        let left = model::seconds_left(self.total_seconds(), self.position, self.maximum_scroll());
        let voice_chip = (self.state.scroll == TeleprompterScroll::Voice && playing).then(|| {
            let label = if Self::microphone_level(cx).is_none() {
                t!("Choose a microphone")
            } else if self.speaking {
                t!("Following you")
            } else {
                t!("Waiting for you")
            };
            self.caption(label, 0.)
        });
        let full_screen_available = self.other_display.is_some() || self.full_screen.is_some();

        div()
            .id("toolbar")
            .absolute()
            .right(px(8.))
            .bottom(px(8.))
            .h(px(TOOLBAR_HEIGHT))
            .px(px(4.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.))
            .rounded(px(20.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.icon_button(
                "play",
                play_icon,
                play_label,
                false,
                !has_script,
                cx,
                |this, window, cx| this.toggle_playback(window, cx),
            ))
            .children(voice_chip)
            .child(self.icon_button(
                "slower",
                "icons/minus.svg",
                t!("Slower  ↓"),
                false,
                false,
                cx,
                |this, window, cx| this.change_speed(-1, window, cx),
            ))
            .child(self.caption(self.speed_label(), 56.))
            .child(self.icon_button(
                "faster",
                "icons/plus.svg",
                t!("Faster  ↑"),
                false,
                false,
                cx,
                |this, window, cx| this.change_speed(1, window, cx),
            ))
            .child(self.separator())
            .child(self.icon_button(
                "smaller",
                "icons/minus.svg",
                t!("Smaller text  -"),
                false,
                false,
                cx,
                |this, window, cx| this.change_font_size(-1., window, cx),
            ))
            .child(self.caption(format!("{}", self.state.font_size as i32), 24.))
            .child(self.icon_button(
                "larger",
                "icons/plus.svg",
                t!("Larger text  +"),
                false,
                false,
                cx,
                |this, window, cx| this.change_font_size(1., window, cx),
            ))
            .child(self.separator())
            .child(self.caption(model::format_clock(left), 0.))
            .when(full_screen_available, |this| {
                this.child(self.icon_button(
                    "full-screen",
                    if self.full_screen.is_some() {
                        "icons/minimize.svg"
                    } else {
                        "icons/maximize.svg"
                    },
                    if self.full_screen.is_some() {
                        t!("Exit full screen  F")
                    } else {
                        t!("Full screen on another display  F")
                    },
                    self.full_screen.is_some(),
                    false,
                    cx,
                    |this, window, cx| this.toggle_full_screen(window, cx),
                ))
            })
            .child(self.icon_button(
                "edit",
                "icons/pencil.svg",
                t!("Edit script  E"),
                false,
                false,
                cx,
                |this, window, cx| this.show_edit(window, cx),
            ))
            .child(self.icon_button(
                "close",
                "icons/x.svg",
                t!("Close"),
                false,
                false,
                cx,
                |this, window, cx| this.close(window, cx),
            ))
    }

    fn render_title(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let caption_button = |id: &'static str, icon: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex_none()
                .w(px(40.))
                .h(px(TITLE_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .hover(|style| style.bg(theme.accent).text_color(theme.foreground))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .child(svg().path(icon).size(px(14.)))
        };
        div()
            .id("title")
            .flex_none()
            .h(px(TITLE_HEIGHT))
            .pl(px(12.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.begin_drag(event, window, cx);
                }),
            )
            .child(self.status_line(self.capture_status(cx)))
            .child(div().flex_1())
            .child(
                caption_button("minimize", "icons/minus.svg", t!("Minimize"))
                    .on_click(cx.listener(|this, _, window, cx| this.minimize(window, cx))),
            )
            .child(
                caption_button("close-window", "icons/x.svg", t!("Close"))
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            )
    }

    fn render_editor(&self) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id("script-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(12.))
            .py(px(8.))
            .child(
                ui::TextInput::bare(&theme, "script", &self.script_input)
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(theme.foreground.into())
                    .placeholder_color(theme.muted_foreground.into())
                    .selection_color(theme.selection.into()),
            )
    }

    fn render_foot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let words = count_words(&self.state.script);
        let count = if self.state.uses_chinese_speed() {
            t!(
                "{count} chars · about {time}",
                count = words,
                time = model::format_clock(self.total_seconds())
            )
        } else {
            t!(
                "{count} words · about {time}",
                count = words,
                time = model::format_clock(self.total_seconds())
            )
        };
        div()
            .flex_none()
            .h(px(FOOT_HEIGHT))
            .px(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.))
            .child(self.icon_button(
                "read",
                "icons/play.svg",
                t!("Read  Ctrl Enter"),
                false,
                !self.has_script(),
                cx,
                |this, window, cx| this.toggle_playback(window, cx),
            ))
            .child(
                div()
                    .px(px(6.))
                    .text_size(px(12.))
                    .text_color(self.theme.muted_foreground)
                    .whitespace_nowrap()
                    .child(count),
            )
            .child(div().flex_1())
            .child(self.icon_button(
                "options",
                "icons/sliders-horizontal.svg",
                t!("Teleprompter options"),
                self.options_open,
                false,
                cx,
                |this, _, cx| {
                    this.options_open = !this.options_open;
                    this.other_display = other_display_bounds(this.frame);
                    cx.notify();
                },
            ))
    }

    fn option_row(&self, label: &'static str) -> gpui::Div {
        div()
            .min_h(px(30.))
            .px(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.))
                    .text_color(self.theme.popover_foreground)
                    .child(label),
            )
    }

    fn switch_row(
        &self,
        id: &'static str,
        label: &'static str,
        on: bool,
        key: &'static str,
        cx: &mut Context<Self>,
        flip: fn(&mut TeleprompterState),
    ) -> impl IntoElement {
        let theme = self.theme;
        self.option_row(label)
            .id(id)
            .rounded(px(6.))
            .hover(|style| style.bg(theme.accent))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_setting(key, window, cx, flip);
            }))
            .child(ui::Toggle::settings(
                &theme,
                SharedString::from(format!("{id}-switch")),
                on,
            ))
    }

    fn render_options(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.options_open {
            return None;
        }
        let theme = self.theme;
        let state = &self.state;
        let scroll = ui::SegmentedControl::settings(
            &theme,
            "scroll-mode",
            vec![
                ui::SegmentOption::new(
                    t!("Fixed speed"),
                    state.scroll == TeleprompterScroll::Fixed,
                ),
                ui::SegmentOption::new(
                    t!("When I speak"),
                    state.scroll == TeleprompterScroll::Voice,
                ),
            ],
        )
        .on_select(cx.listener(|this, index: &usize, window, cx| {
            this.state.scroll = if *index == 1 {
                TeleprompterScroll::Voice
            } else {
                TeleprompterScroll::Fixed
            };
            this.voice.reset();
            this.touch(TeleprompterState::SCROLL, window, cx);
            cx.notify();
        }));
        let spacing = ui::SegmentedControl::settings(
            &theme,
            "line-spacing",
            model::LINE_SPACINGS
                .iter()
                .map(|value| {
                    ui::SegmentOption::new(
                        format!("{value}"),
                        (state.line_height - value).abs() < 0.001,
                    )
                })
                .collect(),
        )
        .on_select(cx.listener(|this, index: &usize, window, cx| {
            if let Some(value) = model::LINE_SPACINGS.get(*index) {
                this.state.line_height = *value;
                this.touch(TeleprompterState::LINE_HEIGHT, window, cx);
                cx.notify();
            }
        }));
        let align = ui::SegmentedControl::settings(
            &theme,
            "align",
            vec![
                ui::SegmentOption::new(t!("Center"), state.align == TeleprompterAlign::Center),
                ui::SegmentOption::new(
                    tc!("text align", "Left"),
                    state.align == TeleprompterAlign::Left,
                ),
            ],
        )
        .on_select(cx.listener(|this, index: &usize, window, cx| {
            this.state.align = if *index == 1 {
                TeleprompterAlign::Left
            } else {
                TeleprompterAlign::Center
            };
            this.touch(TeleprompterState::ALIGN, window, cx);
            cx.notify();
        }));

        Some(
            div()
                .id("options")
                .absolute()
                .right(px(8.))
                .bottom(px(48.))
                .w(px(264.))
                .p(px(4.))
                .flex()
                .flex_col()
                .border_1()
                .border_color(theme.border)
                .rounded(px(8.))
                .bg(theme.popover)
                .text_color(theme.popover_foreground)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(self.option_row(t!("Scroll")).child(scroll))
                .child(self.option_row(t!("Line spacing")).child(spacing))
                .child(self.option_row(t!("Align")).child(align))
                .child(self.switch_row(
                    "start-with-recording",
                    t!("Start with recording"),
                    state.start_with_recording,
                    TeleprompterState::START_WITH_RECORDING,
                    cx,
                    |state| state.start_with_recording = !state.start_with_recording,
                ))
                .child(self.switch_row(
                    "reading-line",
                    t!("Reading line"),
                    state.show_cue_markers,
                    TeleprompterState::SHOW_CUE_MARKERS,
                    cx,
                    |state| state.show_cue_markers = !state.show_cue_markers,
                ))
                .child(self.switch_row(
                    "hide-from-sharing",
                    t!("Hide from screen sharing"),
                    state.hide_from_sharing,
                    TeleprompterState::HIDE_FROM_SHARING,
                    cx,
                    |state| state.hide_from_sharing = !state.hide_from_sharing,
                ))
                .when(self.other_display.is_some(), |this| {
                    this.child(
                        self.option_row(t!("Full screen on another display"))
                            .id("full-screen-option")
                            .rounded(px(6.))
                            .hover(|style| style.bg(theme.accent))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_full_screen(window, cx);
                            }))
                            .child(
                                svg()
                                    .path("icons/maximize.svg")
                                    .size(px(14.))
                                    .text_color(theme.muted_foreground),
                            ),
                    )
                })
                .into_any_element(),
        )
    }

    fn render_resize_edges(&self, attached: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let mut layer = div().absolute().inset_0().size_full();
        for edge in Edge::ALL {
            let id: &'static str = match edge {
                Edge::Left => "resize-left",
                Edge::Right => "resize-right",
                Edge::Bottom => "resize-bottom",
                Edge::BottomLeft => "resize-bottom-left",
                Edge::BottomRight => "resize-bottom-right",
            };
            let handle = div().id(id).absolute();
            let handle = match edge {
                Edge::Left => handle
                    .left_0()
                    .top(px(if attached { 0. } else { EDGE_HIT }))
                    .bottom(px(EDGE_HIT * 2.))
                    .w(px(EDGE_HIT))
                    .cursor(gpui::CursorStyle::ResizeLeftRight),
                Edge::Right => handle
                    .right_0()
                    .top(px(if attached { 0. } else { EDGE_HIT }))
                    .bottom(px(EDGE_HIT * 2.))
                    .w(px(EDGE_HIT))
                    .cursor(gpui::CursorStyle::ResizeLeftRight),
                Edge::Bottom => handle
                    .bottom_0()
                    .left(px(EDGE_HIT * 2.))
                    .right(px(EDGE_HIT * 2.))
                    .h(px(EDGE_HIT))
                    .cursor(gpui::CursorStyle::ResizeUpDown),
                Edge::BottomLeft => handle
                    .bottom_0()
                    .left_0()
                    .size(px(EDGE_HIT * 2.))
                    .cursor(gpui::CursorStyle::ResizeUpRightDownLeft),
                Edge::BottomRight => handle
                    .bottom_0()
                    .right_0()
                    .size(px(EDGE_HIT * 2.))
                    .cursor(gpui::CursorStyle::ResizeUpLeftDownRight),
            };
            layer = layer.child(handle.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.drag = Some(Drag::Resize {
                        edge,
                        pointer: Self::pointer_global(window, event.position),
                        frame: this.frame,
                    });
                    cx.notify();
                }),
            ));
        }
        layer
    }
}

fn status_label(status: CaptureStatus) -> &'static str {
    match status {
        CaptureStatus::CantHide => t!("Can't hide here, recordings may show this"),
        CaptureStatus::PausedWithRecording => t!("Paused with the recording"),
        CaptureStatus::HiddenFromThisRecording => t!("Hidden from this recording"),
        CaptureStatus::HiddenFromRecordingsAndSharing => t!("Hidden from recordings and sharing"),
        CaptureStatus::HiddenFromRecordings => t!("Hidden from recordings"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words_in_pasted_scripts() {
        assert_eq!(count_words("  One   two\nthree "), 3);
        assert_eq!(count_words(" "), 0);
    }

    #[test]
    fn chinese_script_scrolls_for_a_minute_at_260_characters_per_minute() {
        let state = TeleprompterState {
            script: "字".repeat(260),
            ..Default::default()
        };
        let speed =
            calculate_playback_speed(600., count_words(&state.script), state.reading_speed());
        assert_eq!(600. / speed, 60.);
    }

    #[test]
    fn mixed_scripts_count_han_characters_and_spaced_words() {
        assert_eq!(count_words("你好，FrameEver world！"), 4);
        assert_eq!(count_words("Hello中文world"), 4);
        assert_eq!(count_words("中文，。！？ ……"), 2);
        assert_eq!(count_words("  One   two\nthree don't re-record "), 5);
        assert_eq!(count_words("𠮷禰\u{e0100} café"), 3);
    }

    #[test]
    fn calculates_a_positive_scroll_speed_from_reading_duration() {
        assert_eq!(calculate_playback_speed(600., 300, 150), 5.);
        assert_eq!(calculate_playback_speed(-10., 300, 150), 0.);
    }

    #[test]
    fn retains_sub_pixel_movement_across_animation_frames() {
        let mut position = 0.;
        for _ in 0..60 {
            position = advance_playback_position(position, 100., 10., 1. / 60.);
        }
        assert!((position - 10.).abs() < 0.01, "position was {position}");
    }

    #[test]
    fn playback_is_bounded_at_both_ends() {
        assert_eq!(calculate_playback_speed(120., 3, 350), 120.);
        assert_eq!(advance_playback_position(90., 100., 1000., 1.), 100.);
        assert_eq!(advance_playback_position(-5., 100., 0., 1.), 0.);
    }

    #[test]
    fn every_status_has_a_label() {
        for status in [
            CaptureStatus::CantHide,
            CaptureStatus::PausedWithRecording,
            CaptureStatus::HiddenFromThisRecording,
            CaptureStatus::HiddenFromRecordingsAndSharing,
            CaptureStatus::HiddenFromRecordings,
        ] {
            assert!(!status_label(status).is_empty());
        }
    }
}
