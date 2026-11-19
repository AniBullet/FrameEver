use crate::{click_highlight::ChannelOrder, gamepad::GamepadWatch, input_reader::TargetFocus};
use frameever_project::{
    InputDisplay, InputDisplayStyle, XY,
    input_display::{
        BuiltinPresetId, DisplayState, Element, ElementKind, GamepadAxes, GamepadKind,
        GamepadSnapshot, INPUT_DISPLAY_DEFAULT_SIZE, INPUT_DISPLAY_OPACITY_RANGE, Input, Preset,
        TRIGGER_FULL, WindowRule, display_rect, gamepad_buttons, key_code,
    },
};
use frameever_rendering::input_display::{
    LoadedPreset, atlas_scale_for, compose, preset_for_controller, resolve_preset,
};
use image::imageops;
use parking_lot::Mutex;
use std::borrow::Cow;
use std::{
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub const POLL_INTERVAL: Duration = Duration::from_millis(8);
const MOUSE_BUTTONS: std::ops::RangeInclusive<u8> = 1..=5;

pub const INSTANT_INPUT_DEFAULT_OPACITY: f64 = 80.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputDevice {
    #[default]
    Auto,
    KeyboardAndMouse,
    Gamepad,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InstantInputDisplay {
    pub preset: String,
    pub gamepad_preset: String,
    pub style: InputDisplayStyle,
    pub device: InputDevice,
    pub position: XY<f64>,
    pub size: f64,
    pub opacity: f64,
}

impl Default for InstantInputDisplay {
    fn default() -> Self {
        Self {
            preset: BuiltinPresetId::WasdMouse.id().to_string(),
            gamepad_preset: BuiltinPresetId::Gamepad.id().to_string(),
            style: InputDisplayStyle::default(),
            device: InputDevice::default(),
            position: XY::new(0.0, 1.0),
            size: INPUT_DISPLAY_DEFAULT_SIZE,
            opacity: INSTANT_INPUT_DEFAULT_OPACITY,
        }
    }
}

impl InstantInputDisplay {
    fn block(&self) -> InputDisplay {
        InputDisplay {
            enabled: true,
            preset: self.preset.clone(),
            style: self.style,
            position: self.position,
            size: self.size,
            opacity: self
                .opacity
                .clamp(INPUT_DISPLAY_OPACITY_RANGE.0, INPUT_DISPLAY_OPACITY_RANGE.1),
            ..Default::default()
        }
    }

    fn keyboard_preset(&self) -> &str {
        let draws_a_gamepad = BuiltinPresetId::from_id(&self.preset)
            .is_some_and(|preset| preset.controller().is_some());
        if draws_a_gamepad {
            BuiltinPresetId::WasdMouse.id()
        } else {
            &self.preset
        }
    }

    fn draws_the_gamepad(&self, gamepad_in_use: Option<GamepadKind>) -> bool {
        match (self.device, gamepad_in_use) {
            (InputDevice::Gamepad, _) | (InputDevice::Auto, Some(_)) => true,
            (InputDevice::KeyboardAndMouse, _) | (InputDevice::Auto, None) => false,
        }
    }

    fn shown_preset(&self, gamepad_in_use: Option<GamepadKind>) -> Cow<'_, str> {
        if self.draws_the_gamepad(gamepad_in_use) {
            preset_for_controller(&self.gamepad_preset, gamepad_in_use)
        } else {
            self.keyboard_preset().into()
        }
    }

    fn built_in_preset(&self, gamepad_in_use: Option<GamepadKind>) -> &'static str {
        if self.draws_the_gamepad(gamepad_in_use) {
            BuiltinPresetId::Gamepad.for_controller(gamepad_in_use).id()
        } else {
            BuiltinPresetId::WasdMouse.id()
        }
    }
}

#[derive(Default)]
struct DeviceInUse {
    gamepad: bool,
}

impl DeviceInUse {
    fn follow(&mut self, device: InputDevice, mut state: DisplayState) -> DisplayState {
        let on_the_gamepad = |input: Input| matches!(input, Input::GamepadButton(_));
        let gamepad = state.axes != GamepadAxes::default() || state.held().any(on_the_gamepad);
        let keys = state.held().any(|input| !on_the_gamepad(input));
        self.gamepad = match device {
            InputDevice::Auto if gamepad != keys => gamepad,
            InputDevice::Auto => self.gamepad,
            InputDevice::KeyboardAndMouse => false,
            InputDevice::Gamepad => true,
        };
        if !self.gamepad {
            state.controller = None;
        }
        state
    }
}

pub trait InputSource {
    fn read(&mut self, state: &mut DisplayState);
}

pub struct KeyboardAndMouse(device_query::DeviceState);

impl KeyboardAndMouse {
    pub fn new() -> Self {
        Self(device_query::DeviceState::new())
    }
}

impl Default for KeyboardAndMouse {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSource for KeyboardAndMouse {
    fn read(&mut self, state: &mut DisplayState) {
        use device_query::DeviceQuery;
        hold(
            state,
            &self.0.get_keys(),
            &self.0.get_mouse().button_pressed,
        );
    }
}

fn hold(state: &mut DisplayState, keys: &[device_query::Keycode], buttons: &[bool]) {
    for key in keys {
        if let Some(code) = key_code(&crate::cursor::keycode_to_string(key).1) {
            state.apply(Input::Key(code), true);
        }
    }
    for (button, pressed) in buttons.iter().enumerate() {
        if let (true, Ok(button)) = (*pressed, u8::try_from(button)) {
            state.apply(Input::MouseButton(button), true);
        }
    }
}

pub struct Gamepad(GamepadWatch);

impl Gamepad {
    pub fn new(watch: GamepadWatch) -> Self {
        Self(watch)
    }
}

impl InputSource for Gamepad {
    fn read(&mut self, state: &mut DisplayState) {
        if let Some(snapshot) = self.0.read() {
            state.show_gamepad(&snapshot);
        }
        state.controller = self.0.controller();
    }
}

pub fn live_sources(device: InputDevice) -> Vec<Box<dyn InputSource>> {
    let mut sources: Vec<Box<dyn InputSource>> = Vec::new();
    if device != InputDevice::Gamepad {
        sources.push(Box::new(KeyboardAndMouse::new()));
    }
    if device != InputDevice::KeyboardAndMouse
        && let Some(watch) = GamepadWatch::connected()
    {
        sources.push(Box::new(Gamepad::new(watch)));
    }
    sources
}

struct Shared {
    settings: InstantInputDisplay,
    state: Mutex<DisplayState>,
    paused: AtomicBool,
}

#[derive(Clone)]
pub struct LiveInputDisplay(Arc<Shared>);

impl LiveInputDisplay {
    pub fn new(settings: InstantInputDisplay) -> Self {
        Self(Arc::new(Shared {
            settings,
            state: Mutex::new(DisplayState::default()),
            paused: AtomicBool::new(false),
        }))
    }

    pub fn set_paused(&self, paused: bool) {
        let mut state = self.0.state.lock();
        self.0.paused.store(paused, Ordering::SeqCst);
        if paused {
            *state = DisplayState::default();
        }
    }

    pub fn show(&self, state: DisplayState) {
        *self.0.state.lock() = state;
    }

    fn state(&self) -> DisplayState {
        self.0.state.lock().clone()
    }

    pub fn watch(
        &self,
        target_window: Option<scap_targets::WindowId>,
        recording: impl Fn() -> bool + Send + 'static,
    ) {
        let device = self.0.settings.device;
        self.watch_with(
            move || live_sources(device),
            move || TargetFocus::of(target_window.as_ref()),
            recording,
        );
    }

    pub fn watch_with(
        &self,
        sources: impl FnOnce() -> Vec<Box<dyn InputSource>> + Send + 'static,
        target: impl FnOnce() -> TargetFocus + Send + 'static,
        recording: impl Fn() -> bool + Send + 'static,
    ) {
        let shared = Arc::downgrade(&self.0);
        let spawned = std::thread::Builder::new()
            .name("live-input-display".into())
            .spawn(move || watch_sources(shared, sources(), target(), recording));
        if let Err(error) = spawned {
            tracing::warn!("input display watcher did not start: {error}");
        }
    }

    pub(crate) fn painter(&self) -> Painter {
        Painter {
            display: self.clone(),
            prepared_for: None,
            prepared: None,
        }
    }
}

fn watch_sources(
    shared: Weak<Shared>,
    mut sources: Vec<Box<dyn InputSource>>,
    mut target: TargetFocus,
    recording: impl Fn() -> bool,
) {
    let mut rule = WindowRule::default();
    let mut in_use = DeviceInUse::default();
    loop {
        std::thread::sleep(POLL_INTERVAL);
        let Some(shared) = shared.upgrade() else {
            return;
        };
        if shared.paused.load(Ordering::SeqCst) || !recording() {
            continue;
        }
        let mut polled = DisplayState::default();
        for source in &mut sources {
            source.read(&mut polled);
        }
        let shown = in_use.follow(
            shared.settings.device,
            rule.follow(&polled, target.read().unwrap_or(true)),
        );
        let mut state = shared.state.lock();
        if !shared.paused.load(Ordering::SeqCst) {
            *state = shown;
        }
    }
}

fn loaded_preset(
    settings: &InstantInputDisplay,
    controller: Option<GamepadKind>,
    frame: (u32, u32),
) -> Option<(LoadedPreset, Rect)> {
    let preset = settings.shown_preset(controller);
    let resolve = |scale| {
        resolve_preset(&preset, settings.style, scale)
            .or_else(|| resolve_preset(settings.built_in_preset(controller), settings.style, scale))
    };
    let loaded = resolve(1)?;
    let rect = Rect::in_frame(settings, &loaded, frame)?;
    let wanted = atlas_scale_for(loaded.preset.width, rect.width as f64);
    let loaded = if loaded.scalable && wanted != loaded.atlas_scale {
        resolve(wanted)?
    } else {
        loaded
    };
    Some((loaded, rect))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
}

impl Rect {
    fn in_frame(
        settings: &InstantInputDisplay,
        loaded: &LoadedPreset,
        frame: (u32, u32),
    ) -> Option<Self> {
        let rect = display_rect(
            &settings.block(),
            (loaded.preset.width, loaded.preset.height),
            loaded.frame_share,
            frame,
        );
        let (frame_width, frame_height) = (frame.0 as usize, frame.1 as usize);
        let width = (rect.width.round() as usize).min(frame_width);
        let height = (rect.height.round() as usize).min(frame_height);
        (width > 0 && height > 0).then(|| Self {
            left: (rect.x.round().max(0.0) as usize).min(frame_width - width),
            top: (rect.y.round().max(0.0) as usize).min(frame_height - height),
            width,
            height,
        })
    }
}

fn every_input(preset: &Preset) -> DisplayState {
    let mut state = DisplayState::default();
    for element in &preset.elements {
        if let ElementKind::KeyboardKey { code } = element.kind {
            state.apply(Input::Key(code), true);
        }
    }
    for button in MOUSE_BUTTONS {
        state.apply(Input::MouseButton(button), true);
    }
    state.show_gamepad(&GamepadSnapshot {
        buttons: (1 << gamepad_buttons::COUNT) - 1,
        axes: GamepadAxes {
            left_trigger: TRIGGER_FULL,
            right_trigger: TRIGGER_FULL,
            ..Default::default()
        },
        ..Default::default()
    });
    state.controller = None;
    state
}

fn fitted(loaded: &LoadedPreset, state: &DisplayState, rect: Rect) -> Vec<u8> {
    let composed = compose(loaded, state);
    let (width, height) = (rect.width as u32, rect.height as u32);
    if composed.dimensions() == (width, height) {
        return composed.into_raw();
    }
    imageops::resize(&composed, width, height, imageops::FilterType::Triangle).into_raw()
}

struct Knobs {
    sticks: Vec<Element>,
    released: Vec<u8>,
    pressed: Vec<u8>,
}

struct Prepared {
    frame: (u32, u32),
    rect: Rect,
    controller: Option<GamepadKind>,
    preset: Preset,
    released: Vec<u8>,
    pressed: Vec<u8>,
    knobs: Option<Knobs>,
    picture: Vec<u8>,
    opacity: u32,
    shown: DisplayState,
}

fn fade(picture: &mut [u8], opacity: u32) {
    if opacity >= 255 {
        return;
    }
    for channel in picture {
        *channel = ((u32::from(*channel) * opacity + 127) / 255) as u8;
    }
}

fn is_stick(element: &Element) -> bool {
    matches!(element.kind, ElementKind::AnalogStick { .. })
}

impl Prepared {
    fn new(
        settings: &InstantInputDisplay,
        controller: Option<GamepadKind>,
        frame: (u32, u32),
    ) -> Option<Self> {
        let (mut loaded, rect) = loaded_preset(settings, controller, frame)?;
        let everything = every_input(&loaded.preset);
        let (sticks, still): (Vec<Element>, Vec<Element>) =
            std::mem::take(&mut loaded.preset.elements)
                .into_iter()
                .partition(is_stick);
        let knobs = (!sticks.is_empty()).then(|| {
            loaded.preset.elements = sticks.clone();
            Knobs {
                released: fitted(&loaded, &DisplayState::default(), rect),
                pressed: fitted(&loaded, &everything, rect),
                sticks,
            }
        });
        loaded.preset.elements = still;
        let released = fitted(&loaded, &DisplayState::default(), rect);
        let pressed = fitted(&loaded, &everything, rect);
        let mut prepared = Self {
            frame,
            rect,
            controller,
            picture: released.clone(),
            released,
            pressed,
            knobs,
            preset: loaded.preset,
            opacity: (settings.block().opacity_fraction() * 255.0).round() as u32,
            shown: DisplayState::default(),
        };
        prepared.draw(&DisplayState::default());
        Some(prepared)
    }

    fn shows(&self, settings: &InstantInputDisplay, controller: Option<GamepadKind>) -> bool {
        settings.shown_preset(controller) == settings.shown_preset(self.controller)
    }

    fn show(&mut self, state: DisplayState) {
        if state == self.shown {
            return;
        }
        self.draw(&state);
        self.shown = state;
    }

    fn scale(&self) -> (f64, f64) {
        (
            self.rect.width as f64 / f64::from(self.preset.width.max(1)),
            self.rect.height as f64 / f64::from(self.preset.height.max(1)),
        )
    }

    fn draw(&mut self, state: &DisplayState) {
        self.compose(state);
        fade(&mut self.picture, self.opacity);
    }

    fn compose(&mut self, state: &DisplayState) {
        self.picture.copy_from_slice(&self.released);
        let resting = self.preset.sprites(&DisplayState::default());
        let lit = self.preset.sprites(state);
        let (across, down) = self.scale();
        let span = |start: u32, length: u32, scale: f64, limit: usize| {
            let from = ((f64::from(start) * scale).floor() as usize).min(limit);
            let to = ((f64::from(start + length) * scale).ceil() as usize).min(limit);
            from..to
        };
        for lit in lit.iter().filter(|lit| !resting.contains(lit)) {
            let columns = span(lit.x, lit.source.width, across, self.rect.width);
            for row in span(lit.y, lit.source.height, down, self.rect.height) {
                let line = row * self.rect.width * 4;
                let range = line + columns.start * 4..line + columns.end * 4;
                self.picture[range.clone()].copy_from_slice(&self.pressed[range]);
            }
        }
        let Some(knobs) = &self.knobs else {
            return;
        };
        for stick in &knobs.sticks {
            let ElementKind::AnalogStick { right, .. } = stick.kind else {
                continue;
            };
            let click = if right {
                gamepad_buttons::RIGHT_STICK
            } else {
                gamepad_buttons::LEFT_STICK
            };
            let layer = if state.is_held(Input::GamepadButton(click)) {
                &knobs.pressed
            } else {
                &knobs.released
            };
            let (dx, dy) = stick.stick_offset(state).unwrap_or_default();
            let shift = (
                (f64::from(dx) * across).round() as isize,
                (f64::from(dy) * down).round() as isize,
            );
            let columns = span(
                stick.x.saturating_sub(1),
                stick.mapping.width + 2,
                across,
                self.rect.width,
            );
            let rows = span(
                stick.y.saturating_sub(1),
                stick.mapping.height + 2,
                down,
                self.rect.height,
            );
            lay_over(
                &mut self.picture,
                layer,
                (self.rect.width, self.rect.height),
                (columns, rows),
                shift,
            );
        }
    }
}

fn lay_over(
    picture: &mut [u8],
    layer: &[u8],
    (width, height): (usize, usize),
    (columns, rows): (std::ops::Range<usize>, std::ops::Range<usize>),
    (shift_x, shift_y): (isize, isize),
) {
    for row in rows {
        let Some(target_row) = row.checked_add_signed(shift_y).filter(|row| *row < height) else {
            continue;
        };
        for column in columns.clone() {
            let Some(target_column) = column
                .checked_add_signed(shift_x)
                .filter(|column| *column < width)
            else {
                continue;
            };
            let source = &layer[(row * width + column) * 4..][..4];
            let keep = 255 - u32::from(source[3]);
            if keep == 255 {
                continue;
            }
            let target = &mut picture[(target_row * width + target_column) * 4..][..4];
            for (target, source) in target.iter_mut().zip(source) {
                *target =
                    (u32::from(*source) + (u32::from(*target) * keep + 127) / 255).min(255) as u8;
            }
        }
    }
}

pub(crate) struct Painter {
    display: LiveInputDisplay,
    prepared_for: Option<(u32, u32)>,
    prepared: Option<Prepared>,
}

impl Painter {
    pub(crate) fn prepare(&mut self, frame: (u32, u32)) {
        if self.prepared_for != Some(frame) {
            self.prepared_for = Some(frame);
            self.prepared = Prepared::new(&self.display.0.settings, None, frame);
        }
    }

    pub(crate) fn changed(&self, frame: (u32, u32)) -> bool {
        self.prepared.as_ref().is_some_and(|prepared| {
            prepared.frame == frame && *self.display.0.state.lock() != prepared.shown
        })
    }

    pub(crate) fn bounds(&self) -> Option<(usize, usize, usize, usize)> {
        self.prepared.as_ref().map(|prepared| {
            let rect = prepared.rect;
            (
                rect.left,
                rect.top,
                rect.left + rect.width,
                rect.top + rect.height,
            )
        })
    }

    pub(crate) fn paint(&mut self, pixels: &mut [u8], stride: usize, order: ChannelOrder) {
        let state = self.display.state();
        let settings = &self.display.0.settings;
        if let Some(stale) = self
            .prepared
            .as_ref()
            .filter(|prepared| !prepared.shows(settings, state.controller))
        {
            self.prepared = Prepared::new(settings, state.controller, stale.frame);
        }
        let Some(prepared) = &mut self.prepared else {
            return;
        };
        prepared.show(state);
        blend(&prepared.picture, prepared.rect, pixels, stride, order);
    }
}

fn blend(picture: &[u8], rect: Rect, pixels: &mut [u8], stride: usize, order: ChannelOrder) {
    let channels: [usize; 3] = match order {
        ChannelOrder::Rgba => [0, 1, 2],
        ChannelOrder::Bgra => [2, 1, 0],
    };
    for (row, source) in picture.chunks_exact(rect.width * 4).enumerate() {
        let start = (rect.top + row) * stride + rect.left * 4;
        let Some(target) = pixels.get_mut(start..start + rect.width * 4) else {
            return;
        };
        for (target, source) in target.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
            match source[3] {
                0 => {}
                255 => {
                    for (channel, from) in channels.into_iter().enumerate() {
                        target[channel] = source[from];
                    }
                }
                alpha => {
                    let keep = 255 - u32::from(alpha);
                    for (channel, from) in channels.into_iter().enumerate() {
                        let under = (u32::from(target[channel]) * keep + 127) / 255;
                        target[channel] = (u32::from(source[from]) + under).min(255) as u8;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        click_highlight::{CaptureArea, ClickHighlights, PULSE},
        frame_overlays::FrameOverlays,
    };
    use frameever_project::input_display::key_codes;
    use std::{
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        time::Instant,
    };

    const FRAME: (u32, u32) = (640, 360);
    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const KEY_W: Input = Input::Key(key_codes::letter('W'));

    fn black_frame(format: ffmpeg::format::Pixel) -> ffmpeg::frame::Video {
        let mut frame = ffmpeg::frame::Video::new(format, FRAME.0, FRAME.1);
        for pixel in frame.data_mut(0).chunks_exact_mut(4) {
            pixel.copy_from_slice(&BLACK);
        }
        frame
    }

    fn pixel(frame: &ffmpeg::frame::Video, x: u32, y: u32) -> [u8; 4] {
        let at = y as usize * frame.stride(0) + x as usize * 4;
        frame.data(0)[at..at + 4].try_into().unwrap()
    }

    fn held(inputs: &[Input]) -> DisplayState {
        let mut state = DisplayState::default();
        for input in inputs {
            state.apply(*input, true);
        }
        state
    }

    const TOP_LEFT: XY<f64> = XY { x: 0.0, y: 0.0 };
    const TOP_RIGHT: XY<f64> = XY { x: 1.0, y: 0.0 };
    const BOTTOM_LEFT: XY<f64> = XY { x: 0.0, y: 1.0 };
    const BOTTOM_RIGHT: XY<f64> = XY { x: 1.0, y: 1.0 };

    fn display(position: XY<f64>) -> LiveInputDisplay {
        LiveInputDisplay::new(InstantInputDisplay {
            position,
            ..Default::default()
        })
    }

    fn overlays(display: &LiveInputDisplay) -> FrameOverlays {
        FrameOverlays {
            click_highlight: None,
            input_display: Some(display.clone()),
        }
    }

    fn captured(display: &LiveInputDisplay) -> ffmpeg::frame::Video {
        let mut frame = black_frame(ffmpeg::format::Pixel::RGBA);
        overlays(display)
            .baker()
            .unwrap()
            .captured(&mut frame, Instant::now());
        frame
    }

    fn painted(frame: &ffmpeg::frame::Video) -> (u32, u32, u32, u32) {
        let mut bounds = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..FRAME.1 {
            for x in 0..FRAME.0 {
                if pixel(frame, x, y) != BLACK {
                    bounds = (
                        bounds.0.min(x),
                        bounds.1.min(y),
                        bounds.2.max(x),
                        bounds.3.max(y),
                    );
                }
            }
        }
        bounds
    }

    fn differing(a: &ffmpeg::frame::Video, b: &ffmpeg::frame::Video) -> Vec<(u32, u32)> {
        (0..FRAME.1)
            .flat_map(|y| (0..FRAME.0).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(a, *x, *y) != pixel(b, *x, *y))
            .collect()
    }

    fn key_w_in_the_bottom_left_corner() -> (f64, f64, f64, f64) {
        let loaded = resolve_preset("wasd-mouse", InputDisplayStyle::default(), 1).unwrap();
        let key = loaded
            .preset
            .elements
            .iter()
            .find(|element| {
                element.kind
                    == ElementKind::KeyboardKey {
                        code: key_codes::letter('W'),
                    }
            })
            .unwrap();
        let width = f64::from(FRAME.0) * 0.30;
        let scale = width / f64::from(loaded.preset.width);
        let height = f64::from(loaded.preset.height) * scale;
        let margin = f64::from(FRAME.0) * 0.03;
        let top = f64::from(FRAME.1) - margin - height;
        (
            margin + f64::from(key.x) * scale,
            top + f64::from(key.y) * scale,
            margin + f64::from(key.x + key.mapping.width) * scale,
            top + f64::from(key.y + key.mapping.height) * scale,
        )
    }

    #[test]
    fn a_held_key_lights_up_in_the_recorded_frame() {
        let display = display(BOTTOM_LEFT);
        let resting = captured(&display);
        display.show(held(&[KEY_W]));
        let pressed = captured(&display);
        let changed = differing(&resting, &pressed);
        let (left, top, right, bottom) = key_w_in_the_bottom_left_corner();
        assert!(changed.len() > 100, "{}", changed.len());
        for (x, y) in &changed {
            assert!(
                f64::from(*x) >= left - 2.0
                    && f64::from(*x) <= right + 2.0
                    && f64::from(*y) >= top - 2.0
                    && f64::from(*y) <= bottom + 2.0,
                "({x}, {y}) is outside the W key ({left}, {top}, {right}, {bottom})"
            );
        }
        let centre = (((left + right) / 2.0) as u32, ((top + bottom) / 2.0) as u32);
        assert!(changed.contains(&centre));
        display.show(DisplayState::default());
        assert!(differing(&resting, &captured(&display)).is_empty());
    }

    #[test]
    fn a_held_mouse_button_lights_up_too() {
        let display = display(BOTTOM_LEFT);
        let resting = captured(&display);
        display.show(held(&[Input::MouseButton(1)]));
        let pressed = captured(&display);
        let changed = differing(&resting, &pressed);
        let key = key_w_in_the_bottom_left_corner();
        assert!(!changed.is_empty());
        assert!(changed.iter().all(|(x, _)| f64::from(*x) > key.2));
    }

    #[test]
    fn the_display_sits_in_the_chosen_corner_inside_the_margin() {
        let margin = 19;
        for corner in [TOP_LEFT, TOP_RIGHT, BOTTOM_LEFT, BOTTOM_RIGHT] {
            let (left, top, right, bottom) = painted(&captured(&display(corner)));
            let at_left = left.abs_diff(margin) <= 2 && right < FRAME.0 / 2;
            let at_right = right.abs_diff(FRAME.0 - 1 - margin) <= 2 && left > FRAME.0 / 2;
            let at_top = top.abs_diff(margin) <= 2 && bottom < FRAME.1 / 2;
            let at_bottom = bottom.abs_diff(FRAME.1 - 1 - margin) <= 2 && top > FRAME.1 / 2;
            let across = if corner.x == 0.0 { at_left } else { at_right };
            let down = if corner.y == 0.0 { at_top } else { at_bottom };
            assert!(
                across && down,
                "{corner:?}: {left}, {top}, {right}, {bottom}"
            );
        }
    }

    #[test]
    fn a_still_screen_shows_a_key_going_down_and_up() {
        let display = display(BOTTOM_LEFT);
        let mut baker = overlays(&display).baker().unwrap();
        let now = Instant::now();
        let mut resting = black_frame(ffmpeg::format::Pixel::RGBA);
        baker.captured(&mut resting, now);
        assert!(baker.repeated(&resting, now).is_none());
        display.show(held(&[KEY_W]));
        let pressed = baker.repeated(&resting, now).unwrap();
        display.show(held(&[KEY_W]));
        assert!(baker.repeated(&pressed, now).is_none());
        assert!(differing(&pressed, &captured(&display)).is_empty());
        display.show(DisplayState::default());
        let released = baker.repeated(&pressed, now).unwrap();
        assert!(differing(&released, &resting).is_empty());
        assert!(baker.repeated(&released, now).is_none());
    }

    #[test]
    fn a_click_highlight_over_the_display_fades_back_to_the_display() {
        let display = display(TOP_LEFT);
        let highlights = ClickHighlights::new(
            CaptureArea::new((0.0, 0.0, f64::from(FRAME.0), f64::from(FRAME.1)), None),
            1.0,
        );
        let mut baker = FrameOverlays {
            click_highlight: Some(highlights.clone()),
            input_display: Some(display.clone()),
        }
        .baker()
        .unwrap();
        let start = Instant::now();
        let mut resting = black_frame(ffmpeg::format::Pixel::RGBA);
        baker.captured(&mut resting, start);
        highlights.press(start, (60, 60));
        let clicked = baker
            .repeated(&resting, start + Duration::from_millis(10))
            .unwrap();
        assert!(!differing(&clicked, &resting).is_empty());
        display.show(held(&[KEY_W]));
        let pressed = baker.repeated(&clicked, start + PULSE / 2).unwrap();
        let after = baker.repeated(&pressed, start + PULSE).unwrap();
        assert!(differing(&after, &captured(&display)).is_empty());
    }

    #[test]
    fn the_picture_is_laid_over_the_frame_in_the_frames_channel_order() {
        let picture = [[255, 0, 0, 255], [0, 0, 0, 0], [100, 50, 0, 128]].concat();
        let rect = Rect {
            left: 1,
            top: 1,
            width: 3,
            height: 1,
        };
        let stride = 5 * 4 + 4;
        let under = [10, 20, 30, 255];
        let frame = || [under; 12].concat();
        let mut rgba = frame();
        blend(&picture, rect, &mut rgba, stride, ChannelOrder::Rgba);
        assert_eq!(rgba[..stride], frame()[..stride]);
        assert_eq!(rgba[stride..stride + 4], under);
        assert_eq!(rgba[stride + 4..stride + 8], [255, 0, 0, 255]);
        assert_eq!(rgba[stride + 8..stride + 12], under);
        assert_eq!(rgba[stride + 12..stride + 16], [105, 60, 15, 255]);
        assert_eq!(rgba[stride + 16..], frame()[stride + 16..]);
        let mut bgra = frame();
        blend(&picture, rect, &mut bgra, stride, ChannelOrder::Bgra);
        assert_eq!(bgra[stride + 4..stride + 8], [0, 0, 255, 255]);
        assert_eq!(bgra[stride + 12..stride + 16], [5, 60, 115, 255]);
    }

    #[test]
    fn a_bgra_frame_gets_the_display_too() {
        let display = display(BOTTOM_LEFT);
        let mut bgra = black_frame(ffmpeg::format::Pixel::BGRA);
        overlays(&display)
            .baker()
            .unwrap()
            .captured(&mut bgra, Instant::now());
        assert!(differing(&bgra, &captured(&display)).is_empty());
        assert_ne!(painted(&bgra), (u32::MAX, u32::MAX, 0, 0));
    }

    #[test]
    fn a_preset_that_no_longer_exists_falls_back_to_wasd_and_mouse() {
        let missing = LiveInputDisplay::new(InstantInputDisplay {
            preset: "deleted-by-the-user".into(),
            ..Default::default()
        });
        let fallback = captured(&missing);
        assert!(differing(&fallback, &captured(&display(BOTTOM_LEFT))).is_empty());
        assert_ne!(painted(&fallback), (u32::MAX, u32::MAX, 0, 0));
    }

    #[test]
    fn every_built_in_preset_can_be_chosen() {
        let pictures: Vec<_> = BuiltinPresetId::ALL
            .into_iter()
            .map(|preset| {
                painted(&captured(&LiveInputDisplay::new(InstantInputDisplay {
                    preset: preset.id().into(),
                    device: device_of(preset),
                    ..Default::default()
                })))
            })
            .collect();
        assert_ne!(pictures[0], pictures[1]);
    }

    fn device_of(preset: BuiltinPresetId) -> InputDevice {
        match preset.controller() {
            Some(_) => InputDevice::Gamepad,
            None => InputDevice::KeyboardAndMouse,
        }
    }

    fn gamepad_display() -> LiveInputDisplay {
        LiveInputDisplay::new(InstantInputDisplay {
            device: InputDevice::Gamepad,
            ..Default::default()
        })
    }

    fn gamepad(kind: GamepadKind, axes: GamepadAxes, buttons: &[u16]) -> DisplayState {
        let mut state = DisplayState::default();
        state.show_gamepad(&buttons.iter().fold(
            GamepadSnapshot {
                kind,
                axes,
                ..Default::default()
            },
            |snapshot, button| snapshot.with_button(*button),
        ));
        state
    }

    fn part(id: &str, grow: f64) -> (f64, f64, f64, f64) {
        let loaded = resolve_preset("gamepad", InputDisplayStyle::default(), 1).unwrap();
        let element = loaded
            .preset
            .elements
            .iter()
            .find(|element| element.id == id)
            .unwrap();
        let width = f64::from(FRAME.0) * BuiltinPresetId::Gamepad.frame_share();
        let scale = width / f64::from(loaded.preset.width);
        let margin = f64::from(FRAME.0) * 0.03;
        let top = f64::from(FRAME.1) - margin - f64::from(loaded.preset.height) * scale;
        (
            margin + (f64::from(element.x) - grow) * scale - 2.0,
            top + (f64::from(element.y) - grow) * scale - 2.0,
            margin + (f64::from(element.x + element.mapping.width) + grow) * scale + 2.0,
            top + (f64::from(element.y + element.mapping.height) + grow) * scale + 2.0,
        )
    }

    fn inside(points: &[(u32, u32)], (left, top, right, bottom): (f64, f64, f64, f64)) -> bool {
        points.iter().all(|(x, y)| {
            f64::from(*x) >= left
                && f64::from(*x) <= right
                && f64::from(*y) >= top
                && f64::from(*y) <= bottom
        })
    }

    #[test]
    fn a_pushed_stick_moves_in_the_recorded_frame_and_comes_back() {
        let display = gamepad_display();
        let resting = captured(&display);
        let pushed = |left_stick| {
            display.show(gamepad(
                GamepadKind::Xbox,
                GamepadAxes {
                    left_stick,
                    ..Default::default()
                },
                &[],
            ));
            captured(&display)
        };
        let right = differing(&resting, &pushed([127, 0]));
        let left = differing(&resting, &pushed([-127, 0]));
        let centre = |points: &[(u32, u32)]| {
            points.iter().map(|(x, _)| f64::from(*x)).sum::<f64>() / points.len() as f64
        };

        assert!(right.len() > 20, "{}", right.len());
        assert!(inside(&right, part("stick-left", 9.0)));
        assert!(inside(&left, part("stick-left", 9.0)));
        assert_ne!(right, left);
        assert!(differing(&pushed([127, 0]), &pushed([-127, 0])).len() > 20);
        assert!(centre(&differing(&pushed([0, 0]), &pushed([127, 0]))) > centre(&left) - 20.0);
        assert!(differing(&resting, &pushed([0, 0])).is_empty());
    }

    #[test]
    fn a_trigger_fills_further_the_further_it_is_pulled() {
        let display = gamepad_display();
        let resting = captured(&display);
        let pulled = |right_trigger| {
            display.show(gamepad(
                GamepadKind::Xbox,
                GamepadAxes {
                    right_trigger,
                    ..Default::default()
                },
                &[],
            ));
            differing(&resting, &captured(&display))
        };
        let (half, full) = (pulled(128), pulled(255));
        let reach = |points: &[(u32, u32)]| points.iter().map(|(x, _)| *x).max().unwrap();

        assert!(!half.is_empty());
        assert!(full.len() > half.len() + half.len() / 2);
        assert!(reach(&full) > reach(&half) + 5);
        assert!(inside(&full, part("trigger-right", 0.0)));
        assert!(pulled(0).is_empty());
    }

    #[test]
    fn gamepad_buttons_light_and_a_clicked_stick_lights_where_it_is() {
        let display = gamepad_display();
        let resting = captured(&display);
        display.show(gamepad(
            GamepadKind::Xbox,
            GamepadAxes::default(),
            &[gamepad_buttons::SOUTH],
        ));
        let south = differing(&resting, &captured(&display));
        display.show(gamepad(
            GamepadKind::Xbox,
            GamepadAxes::default(),
            &[gamepad_buttons::RIGHT_STICK],
        ));
        let clicked = differing(&resting, &captured(&display));

        assert!(south.len() > 10);
        assert!(inside(&south, part("face-south", 0.0)));
        assert!(clicked.len() > 20);
        assert!(inside(&clicked, part("stick-right", 0.0)));
    }

    #[test]
    fn the_picture_takes_the_layout_of_the_controller_in_use() {
        let xbox = captured(&gamepad_display());
        let shown = gamepad_display();
        let mut baker = overlays(&shown).baker().unwrap();
        let now = Instant::now();
        let mut first = black_frame(ffmpeg::format::Pixel::RGBA);
        baker.captured(&mut first, now);
        shown.show(gamepad(
            GamepadKind::PlayStation,
            GamepadAxes::default(),
            &[],
        ));
        let playstation = baker.repeated(&first, now).unwrap();

        assert!(differing(&first, &xbox).is_empty());
        assert!(differing(&playstation, &xbox).len() > 50);
        assert_eq!(painted(&playstation), painted(&xbox));
    }

    fn with_settings(settings: InstantInputDisplay, state: DisplayState) -> ffmpeg::frame::Video {
        let display = LiveInputDisplay::new(settings);
        display.show(state);
        captured(&display)
    }

    fn of_device(device: InputDevice) -> InstantInputDisplay {
        InstantInputDisplay {
            device,
            ..Default::default()
        }
    }

    #[test]
    fn the_chosen_device_decides_which_picture_is_drawn() {
        let playstation = || gamepad(GamepadKind::PlayStation, GamepadAxes::default(), &[]);
        let wasd = captured(&display(BOTTOM_LEFT));
        let xbox_pad = captured(&gamepad_display());
        let playstation_pad = with_settings(of_device(InputDevice::Gamepad), playstation());
        let same = |a: &ffmpeg::frame::Video, b: &ffmpeg::frame::Video| differing(a, b).is_empty();

        assert!(!same(&wasd, &xbox_pad));
        assert!(!same(&playstation_pad, &xbox_pad));
        assert!(same(
            &with_settings(of_device(InputDevice::KeyboardAndMouse), playstation()),
            &wasd
        ));
        assert!(same(
            &with_settings(of_device(InputDevice::Auto), playstation()),
            &playstation_pad
        ));
        assert!(same(
            &with_settings(of_device(InputDevice::Auto), held(&[KEY_W])),
            &with_settings(of_device(InputDevice::KeyboardAndMouse), held(&[KEY_W]))
        ));
        let stored_before_the_device_choice = InstantInputDisplay {
            preset: BuiltinPresetId::Gamepad.id().into(),
            device: InputDevice::KeyboardAndMouse,
            ..Default::default()
        };
        assert!(same(
            &with_settings(stored_before_the_device_choice, DisplayState::default()),
            &wasd
        ));
    }

    #[test]
    fn a_made_gamepad_preset_is_drawn_when_the_gamepad_is_and_the_built_in_pad_when_it_is_gone() {
        use frameever_project::input_display::{GamepadParts, KeyLayout};
        use frameever_rendering::input_presets::{remove, save_layout, use_presets_dir};
        let dir = std::env::temp_dir().join(format!(
            "frameever-instant-gamepad-preset-{}",
            std::process::id()
        ));
        use_presets_dir(dir.clone());
        let made = save_layout(
            &dir,
            None,
            "No triggers",
            &KeyLayout {
                gamepad: Some(GamepadParts {
                    triggers: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
        let chosen = |device| InstantInputDisplay {
            device,
            gamepad_preset: made.clone(),
            ..Default::default()
        };
        let playstation = || gamepad(GamepadKind::PlayStation, GamepadAxes::default(), &[]);
        let same = |a: &ffmpeg::frame::Video, b: &ffmpeg::frame::Video| differing(a, b).is_empty();
        let built_in = with_settings(of_device(InputDevice::Gamepad), DisplayState::default());
        let built_in_playstation = with_settings(of_device(InputDevice::Gamepad), playstation());
        let without_triggers = with_settings(chosen(InputDevice::Gamepad), DisplayState::default());

        assert!(!same(&without_triggers, &built_in));
        assert_eq!(painted(&without_triggers).2, painted(&built_in).2);
        assert!(
            differing(&without_triggers, &built_in)
                .iter()
                .all(|(_, y)| *y < painted(&built_in).1 + 14)
        );
        let on_a_playstation_pad = with_settings(chosen(InputDevice::Auto), playstation());
        assert!(!same(&on_a_playstation_pad, &without_triggers));
        assert!(!same(&on_a_playstation_pad, &built_in_playstation));
        assert!(same(
            &with_settings(chosen(InputDevice::Auto), held(&[KEY_W])),
            &with_settings(of_device(InputDevice::KeyboardAndMouse), held(&[KEY_W]))
        ));
        assert!(same(
            &with_settings(chosen(InputDevice::KeyboardAndMouse), playstation()),
            &captured(&display(BOTTOM_LEFT))
        ));

        remove(&dir, &made).unwrap();
        assert!(same(
            &with_settings(chosen(InputDevice::Gamepad), playstation()),
            &built_in_playstation
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_device_in_use_is_the_one_that_last_produced_input() {
        let pad_connected = || gamepad(GamepadKind::Switch, GamepadAxes::default(), &[]);
        let pad_pressed = || {
            gamepad(
                GamepadKind::Switch,
                GamepadAxes::default(),
                &[gamepad_buttons::SOUTH],
            )
        };
        let stick_pushed = || {
            gamepad(
                GamepadKind::Switch,
                GamepadAxes {
                    left_stick: [90, 0],
                    ..Default::default()
                },
                &[],
            )
        };
        let key_held_with_pad_connected = || {
            let mut state = pad_connected();
            state.apply(KEY_W, true);
            state
        };
        let mut in_use = DeviceInUse::default();
        let mut shown =
            |device, state: DisplayState| in_use.follow(device, state).controller.is_some();

        assert!(!shown(InputDevice::Auto, pad_connected()));
        assert!(shown(InputDevice::Auto, pad_pressed()));
        assert!(shown(InputDevice::Auto, pad_connected()));
        assert!(!shown(InputDevice::Auto, key_held_with_pad_connected()));
        assert!(!shown(InputDevice::Auto, pad_connected()));
        assert!(shown(InputDevice::Auto, stick_pushed()));
        assert!(!shown(InputDevice::KeyboardAndMouse, pad_pressed()));
        assert!(shown(InputDevice::Gamepad, key_held_with_pad_connected()));
        assert!(!shown(InputDevice::Gamepad, held(&[KEY_W])));
    }

    #[test]
    fn a_device_that_was_not_chosen_is_not_read() {
        assert_eq!(live_sources(InputDevice::KeyboardAndMouse).len(), 1);
        assert!(live_sources(InputDevice::Gamepad).len() <= 1);
    }

    fn painted_size(settings: InstantInputDisplay) -> (u32, u32) {
        let (left, top, right, bottom) = painted(&captured(&LiveInputDisplay::new(settings)));
        (right - left + 1, bottom - top + 1)
    }

    #[test]
    fn size_scales_the_display_like_the_editor_does() {
        let sized = |size| {
            painted_size(InstantInputDisplay {
                size,
                opacity: 100.0,
                ..Default::default()
            })
        };
        let (normal, large, small) = (sized(60.0), sized(120.0), sized(30.0));

        assert!(normal.0.abs_diff(192) <= 3, "{normal:?}");
        assert!(large.0.abs_diff(normal.0 * 2) <= 4, "{large:?}");
        assert!(large.1.abs_diff(normal.1 * 2) <= 4, "{large:?}");
        assert!(small.0.abs_diff(normal.0 / 2) <= 4, "{small:?}");
        assert_eq!(sized(5000.0), large);
    }

    #[test]
    fn the_display_can_sit_anywhere_inside_the_margin() {
        let placed = |x, y| {
            painted(&captured(&LiveInputDisplay::new(InstantInputDisplay {
                position: XY::new(x, y),
                ..Default::default()
            })))
        };
        let (left, top, right, bottom) = placed(0.5, 0.5);
        assert!((left + right).abs_diff(FRAME.0 - 1) <= 3, "{left} {right}");
        assert!((top + bottom).abs_diff(FRAME.1 - 1) <= 3, "{top} {bottom}");

        let corner = placed(0.0, 0.0);
        let far_corner = placed(1.0, 1.0);
        let quarter = placed(0.25, 0.75);
        let between = |from: u32, to: u32, share: f64| {
            (f64::from(from) + f64::from(to - from) * share).round() as u32
        };
        assert!(
            quarter.0.abs_diff(between(corner.0, far_corner.0, 0.25)) <= 2,
            "{quarter:?}"
        );
        assert!(
            quarter.1.abs_diff(between(corner.1, far_corner.1, 0.75)) <= 2,
            "{quarter:?}"
        );
    }

    #[test]
    fn the_default_display_is_not_fully_opaque() {
        let settings = InstantInputDisplay::default();
        assert!(settings.opacity < 100.0);
        assert!(settings.opacity >= 50.0);
    }

    #[test]
    fn opacity_fades_the_whole_picture_evenly_whatever_is_pressed() {
        let pushed = gamepad(
            GamepadKind::Xbox,
            GamepadAxes {
                left_stick: [127, 0],
                ..Default::default()
            },
            &[gamepad_buttons::LEFT_STICK],
        );
        for (device, state) in [
            (InputDevice::KeyboardAndMouse, DisplayState::default()),
            (InputDevice::KeyboardAndMouse, held(&[KEY_W])),
            (InputDevice::Gamepad, pushed),
        ] {
            let drawn = |opacity| {
                with_settings(
                    InstantInputDisplay {
                        device,
                        opacity,
                        ..Default::default()
                    },
                    state.clone(),
                )
            };
            let (opaque, half) = (drawn(100.0), drawn(50.0));
            let mut seen = 0;
            for y in 0..FRAME.1 {
                for x in 0..FRAME.0 {
                    let (full, faded) = (pixel(&opaque, x, y), pixel(&half, x, y));
                    for channel in 0..3 {
                        let expected = (u32::from(full[channel]) * 128 + 127) / 255;
                        assert!(
                            u32::from(faded[channel]).abs_diff(expected) <= 1,
                            "({x}, {y}): {full:?} became {faded:?}"
                        );
                    }
                    seen += usize::from(full != BLACK);
                }
            }
            assert!(seen > 1000, "{seen}");
        }
    }

    #[test]
    fn a_see_through_display_lets_the_recording_show_through() {
        let over_white = |opacity| {
            let display = LiveInputDisplay::new(InstantInputDisplay {
                opacity,
                ..Default::default()
            });
            let mut frame = black_frame(ffmpeg::format::Pixel::RGBA);
            for pixel in frame.data_mut(0).chunks_exact_mut(4) {
                pixel.copy_from_slice(&[255, 255, 255, 255]);
            }
            overlays(&display)
                .baker()
                .unwrap()
                .captured(&mut frame, Instant::now());
            frame
        };
        let (opaque, faint) = (over_white(100.0), over_white(20.0));
        let darkest = |frame: &ffmpeg::frame::Video| {
            (0..FRAME.1)
                .flat_map(|y| (0..FRAME.0).map(move |x| (x, y)))
                .map(|(x, y)| pixel(frame, x, y)[0])
                .min()
                .unwrap()
        };
        assert!(darkest(&opaque) < 80, "{}", darkest(&opaque));
        assert!(darkest(&faint) > 190, "{}", darkest(&faint));
        assert!(differing(&over_white(1.0), &faint).is_empty());
    }

    struct PadAndKey {
        pad_down: Arc<AtomicBool>,
        key_down: Arc<AtomicBool>,
    }

    impl InputSource for PadAndKey {
        fn read(&mut self, state: &mut DisplayState) {
            let pad = GamepadSnapshot {
                kind: GamepadKind::PlayStation,
                ..Default::default()
            };
            state.show_gamepad(&if self.pad_down.load(Ordering::SeqCst) {
                pad.with_button(gamepad_buttons::SOUTH)
            } else {
                pad
            });
            if self.key_down.load(Ordering::SeqCst) {
                state.apply(KEY_W, true);
            }
        }
    }

    #[test]
    fn a_gamepad_being_used_takes_over_the_picture_and_a_key_takes_it_back() {
        let pad_down = Arc::new(AtomicBool::new(false));
        let key_down = Arc::new(AtomicBool::new(false));
        let display = display(BOTTOM_LEFT);
        let source = PadAndKey {
            pad_down: pad_down.clone(),
            key_down: key_down.clone(),
        };
        display.watch_with(
            move || vec![Box::new(source) as Box<dyn InputSource>],
            TargetFocus::none,
            || true,
        );
        let controller = || display.state().controller;

        std::thread::sleep(POLL_INTERVAL * 6);
        assert_eq!(controller(), None);
        pad_down.store(true, Ordering::SeqCst);
        assert!(eventually(|| controller() == Some(GamepadKind::PlayStation)));
        pad_down.store(false, Ordering::SeqCst);
        std::thread::sleep(POLL_INTERVAL * 6);
        assert_eq!(controller(), Some(GamepadKind::PlayStation));
        key_down.store(true, Ordering::SeqCst);
        assert!(eventually(|| controller().is_none()));
    }

    #[test]
    fn a_gamepad_that_is_read_shows_in_the_display_state() {
        use crate::gamepad::testing::Scripted;
        let pushed = GamepadSnapshot {
            kind: GamepadKind::Switch,
            axes: GamepadAxes {
                right_stick: [0, -127],
                ..Default::default()
            },
            ..Default::default()
        }
        .with_button(gamepad_buttons::LEFT_SHOULDER);
        let mut source = Gamepad::new(GamepadWatch::new(
            Scripted::default()
                .then(&[(1, GamepadSnapshot::default())])
                .then(&[(1, pushed)]),
        ));
        let mut before = DisplayState::default();
        source.read(&mut before);
        let mut after = DisplayState::default();
        source.read(&mut after);

        assert_eq!(before.held().count(), 0);
        assert_eq!(before.controller, Some(GamepadKind::Xbox));
        assert_eq!(
            after.held().collect::<Vec<_>>(),
            vec![Input::GamepadButton(gamepad_buttons::LEFT_SHOULDER)]
        );
        assert_eq!(after.axes, pushed.axes);
        assert_eq!(after.controller, Some(GamepadKind::Switch));
    }

    #[test]
    fn frames_without_overlays_get_no_baker() {
        assert!(FrameOverlays::default().baker().is_none());
    }

    #[test]
    fn keys_and_buttons_read_from_the_devices_become_held_input() {
        use device_query::Keycode;
        let mut state = DisplayState::default();
        hold(
            &mut state,
            &[Keycode::W, Keycode::LShift],
            &[false, true, false, false, true],
        );
        assert_eq!(
            state.held().collect::<Vec<_>>(),
            vec![
                KEY_W,
                Input::Key(key_codes::SHIFT_LEFT),
                Input::MouseButton(1),
                Input::MouseButton(4),
            ]
        );
    }

    struct Scripted {
        reads: Arc<AtomicUsize>,
        down: Arc<AtomicBool>,
    }

    impl InputSource for Scripted {
        fn read(&mut self, state: &mut DisplayState) {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.down.load(Ordering::SeqCst) {
                state.apply(KEY_W, true);
            }
        }
    }

    fn eventually(mut check: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if check() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        false
    }

    #[test]
    fn input_is_read_only_while_the_display_is_watched_and_alive() {
        let reads = Arc::new(AtomicUsize::new(0));
        let down = Arc::new(AtomicBool::new(false));
        let display = display(BOTTOM_LEFT);
        let source = Scripted {
            reads: reads.clone(),
            down: down.clone(),
        };
        let recording = Arc::new(AtomicBool::new(false));
        let started = recording.clone();
        display.watch_with(
            move || vec![Box::new(source) as Box<dyn InputSource>],
            TargetFocus::none,
            move || started.load(Ordering::SeqCst),
        );
        down.store(true, Ordering::SeqCst);
        std::thread::sleep(POLL_INTERVAL * 6);
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(display.state(), DisplayState::default());
        recording.store(true, Ordering::SeqCst);
        assert!(eventually(|| display.state() == held(&[KEY_W])));
        down.store(false, Ordering::SeqCst);
        assert!(eventually(|| display.state() == DisplayState::default()));
        drop(display);
        let mut last = usize::MAX;
        assert!(eventually(|| {
            let now = reads.load(Ordering::SeqCst);
            let stopped = now == last;
            last = now;
            std::thread::sleep(POLL_INTERVAL * 6);
            stopped
        }));
    }

    fn settled(reads: &AtomicUsize) -> usize {
        let mut last = usize::MAX;
        assert!(eventually(|| {
            let now = reads.load(Ordering::SeqCst);
            let stopped = now == last;
            last = now;
            std::thread::sleep(POLL_INTERVAL * 6);
            stopped
        }));
        last
    }

    #[test]
    fn nothing_is_read_while_the_recording_is_paused() {
        let reads = Arc::new(AtomicUsize::new(0));
        let down = Arc::new(AtomicBool::new(true));
        let display = display(BOTTOM_LEFT);
        let source = Scripted {
            reads: reads.clone(),
            down: down.clone(),
        };
        display.watch_with(
            move || vec![Box::new(source) as Box<dyn InputSource>],
            TargetFocus::none,
            || true,
        );
        assert!(eventually(|| display.state() == held(&[KEY_W])));

        display.set_paused(true);
        let while_paused = settled(&reads);
        assert_eq!(display.state(), DisplayState::default());

        display.set_paused(false);
        assert!(eventually(|| display.state() == held(&[KEY_W])));
        assert!(reads.load(Ordering::SeqCst) > while_paused);
    }

    #[test]
    fn input_pressed_while_another_window_is_in_front_is_not_drawn() {
        let down = Arc::new(AtomicBool::new(false));
        let in_front = Arc::new(AtomicBool::new(false));
        let display = display(BOTTOM_LEFT);
        let source = Scripted {
            reads: Arc::new(AtomicUsize::new(0)),
            down: down.clone(),
        };
        let front = in_front.clone();
        display.watch_with(
            move || vec![Box::new(source) as Box<dyn InputSource>],
            move || TargetFocus::scripted(move || Some(front.load(Ordering::SeqCst))),
            || true,
        );
        let stays_empty = || {
            std::thread::sleep(POLL_INTERVAL * 6);
            display.state() == DisplayState::default()
        };

        down.store(true, Ordering::SeqCst);
        assert!(stays_empty());
        in_front.store(true, Ordering::SeqCst);
        assert!(stays_empty());
        down.store(false, Ordering::SeqCst);
        std::thread::sleep(POLL_INTERVAL * 6);
        down.store(true, Ordering::SeqCst);
        assert!(eventually(|| display.state() == held(&[KEY_W])));
        in_front.store(false, Ordering::SeqCst);
        assert!(eventually(|| display.state() == DisplayState::default()));
        in_front.store(true, Ordering::SeqCst);
        assert!(eventually(|| display.state() == held(&[KEY_W])));
    }

    fn median_micros(mut run: impl FnMut()) -> f64 {
        let mut samples: Vec<f64> = (0..300)
            .map(|_| {
                let started = Instant::now();
                run();
                started.elapsed().as_secs_f64() * 1_000_000.0
            })
            .collect();
        samples.sort_by(f64::total_cmp);
        samples[samples.len() / 2]
    }

    #[test]
    #[ignore = "timing run: pnpm fe test --crate frameever-recording live_input_display --ignored --optimized"]
    fn bake_cost_per_frame() {
        for (label, width, height) in [("1080p", 1920u32, 1080u32), ("4K", 3840, 2160)] {
            let now = Instant::now();
            let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::BGRA, width, height);
            let copy = median_micros(|| {
                std::hint::black_box(frame.clone());
            });
            println!("{label}: display off 0.00 us (no baker), plain frame copy {copy:.2} us");
            for (preset, opacity) in BuiltinPresetId::ALL
                .into_iter()
                .flat_map(|preset| [(preset, 100.0), (preset, INSTANT_INPUT_DEFAULT_OPACITY)])
            {
                let display = LiveInputDisplay::new(InstantInputDisplay {
                    preset: preset.id().into(),
                    device: device_of(preset),
                    opacity,
                    ..Default::default()
                });
                let mut baker = overlays(&display).baker().unwrap();
                let started = Instant::now();
                baker.expect_frames_of((width, height));
                let prepare = started.elapsed().as_secs_f64() * 1000.0;
                let held_frame = median_micros(|| baker.captured(&mut frame, now));
                let mut down = false;
                let changing_frame = median_micros(|| {
                    down = !down;
                    display.show(match (down, preset.controller()) {
                        (true, Some(controller)) => gamepad(
                            controller,
                            GamepadAxes {
                                left_stick: [127, -60],
                                right_trigger: 200,
                                ..Default::default()
                            },
                            &[gamepad_buttons::SOUTH],
                        ),
                        (true, None) => held(&[KEY_W, Input::MouseButton(1)]),
                        (false, _) => DisplayState::default(),
                    });
                    baker.captured(&mut frame, now);
                });
                let still_idle = median_micros(|| {
                    assert!(baker.repeated(&frame, now).is_none());
                });
                let still_changing = median_micros(|| {
                    down = !down;
                    display.show(if down {
                        held(&[KEY_W])
                    } else {
                        DisplayState::default()
                    });
                    assert!(baker.repeated(&frame, now).is_some());
                });
                println!(
                    "{label} {} at {opacity}% opacity: display on, captured {held_frame:.2} us, captured with a key change {changing_frame:.2} us, still screen unchanged {still_idle:.2} us, still screen with a key change {still_changing:.2} us, one-time preparation {prepare:.1} ms",
                    preset.id()
                );
            }
        }
    }
}
