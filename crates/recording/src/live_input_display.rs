use crate::{click_highlight::ChannelOrder, gamepad::GamepadWatch};
use frameever_project::{
    InputDisplay, InputDisplayStyle, XY,
    input_display::{
        BuiltinPresetId, DisplayState, Element, ElementKind, GamepadAxes, GamepadKind,
        GamepadSnapshot, Input, Preset, TRIGGER_FULL, display_rect, gamepad_buttons, key_code,
        preset_for_controller,
    },
};
use frameever_rendering::input_display::{LoadedPreset, atlas_scale_for, compose, resolve_preset};
use image::imageops;
use parking_lot::Mutex;
use std::{
    sync::{Arc, Weak},
    time::Duration,
};

pub const POLL_INTERVAL: Duration = Duration::from_millis(8);
const MOUSE_BUTTONS: std::ops::RangeInclusive<u8> = 1..=5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputDisplayCorner {
    TopLeft,
    TopRight,
    #[default]
    BottomLeft,
    BottomRight,
}

impl InputDisplayCorner {
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    fn position(self) -> XY<f64> {
        match self {
            Self::TopLeft => XY::new(0.0, 0.0),
            Self::TopRight => XY::new(1.0, 0.0),
            Self::BottomLeft => XY::new(0.0, 1.0),
            Self::BottomRight => XY::new(1.0, 1.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstantInputDisplay {
    pub preset: String,
    pub style: InputDisplayStyle,
    pub corner: InputDisplayCorner,
}

impl Default for InstantInputDisplay {
    fn default() -> Self {
        Self {
            preset: BuiltinPresetId::WasdMouse.id().to_string(),
            style: InputDisplayStyle::default(),
            corner: InputDisplayCorner::default(),
        }
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

pub fn live_sources() -> Vec<Box<dyn InputSource>> {
    let mut sources: Vec<Box<dyn InputSource>> = vec![Box::new(KeyboardAndMouse::new())];
    if let Some(watch) = GamepadWatch::connected() {
        sources.push(Box::new(Gamepad::new(watch)));
    }
    sources
}

struct Shared {
    settings: InstantInputDisplay,
    state: Mutex<DisplayState>,
}

#[derive(Clone)]
pub struct LiveInputDisplay(Arc<Shared>);

impl LiveInputDisplay {
    pub fn new(settings: InstantInputDisplay) -> Self {
        Self(Arc::new(Shared {
            settings,
            state: Mutex::new(DisplayState::default()),
        }))
    }

    pub fn show(&self, state: DisplayState) {
        *self.0.state.lock() = state;
    }

    fn state(&self) -> DisplayState {
        self.0.state.lock().clone()
    }

    pub fn watch(&self, recording: impl Fn() -> bool + Send + 'static) {
        self.watch_with(live_sources, recording);
    }

    pub fn watch_with(
        &self,
        sources: impl FnOnce() -> Vec<Box<dyn InputSource>> + Send + 'static,
        recording: impl Fn() -> bool + Send + 'static,
    ) {
        let shared = Arc::downgrade(&self.0);
        let spawned = std::thread::Builder::new()
            .name("live-input-display".into())
            .spawn(move || watch_sources(shared, sources(), recording));
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
    recording: impl Fn() -> bool,
) {
    loop {
        std::thread::sleep(POLL_INTERVAL);
        let Some(shared) = shared.upgrade() else {
            return;
        };
        if !recording() {
            continue;
        }
        let mut state = DisplayState::default();
        for source in &mut sources {
            source.read(&mut state);
        }
        LiveInputDisplay(shared).show(state);
    }
}

fn loaded_preset(
    settings: &InstantInputDisplay,
    controller: Option<GamepadKind>,
    frame: (u32, u32),
) -> Option<(LoadedPreset, Rect)> {
    let preset = preset_for_controller(&settings.preset, controller);
    let resolve = |scale| {
        resolve_preset(preset, settings.style, scale)
            .or_else(|| resolve_preset(BuiltinPresetId::WasdMouse.id(), settings.style, scale))
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
        let block = InputDisplay {
            enabled: true,
            preset: settings.preset.clone(),
            style: settings.style,
            position: settings.corner.position(),
            ..Default::default()
        };
        let rect = display_rect(
            &block,
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
    shown: DisplayState,
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
            shown: DisplayState::default(),
        };
        prepared.draw(&DisplayState::default());
        Some(prepared)
    }

    fn shows(&self, settings: &InstantInputDisplay, controller: Option<GamepadKind>) -> bool {
        preset_for_controller(&settings.preset, controller)
            == preset_for_controller(&settings.preset, self.controller)
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

    fn display(corner: InputDisplayCorner) -> LiveInputDisplay {
        LiveInputDisplay::new(InstantInputDisplay {
            corner,
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
        let display = display(InputDisplayCorner::BottomLeft);
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
        let display = display(InputDisplayCorner::BottomLeft);
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
        for corner in InputDisplayCorner::ALL {
            let (left, top, right, bottom) = painted(&captured(&display(corner)));
            let at_left = left.abs_diff(margin) <= 2 && right < FRAME.0 / 2;
            let at_right = right.abs_diff(FRAME.0 - 1 - margin) <= 2 && left > FRAME.0 / 2;
            let at_top = top.abs_diff(margin) <= 2 && bottom < FRAME.1 / 2;
            let at_bottom = bottom.abs_diff(FRAME.1 - 1 - margin) <= 2 && top > FRAME.1 / 2;
            let expected = match corner {
                InputDisplayCorner::TopLeft => at_left && at_top,
                InputDisplayCorner::TopRight => at_right && at_top,
                InputDisplayCorner::BottomLeft => at_left && at_bottom,
                InputDisplayCorner::BottomRight => at_right && at_bottom,
            };
            assert!(expected, "{corner:?}: {left}, {top}, {right}, {bottom}");
        }
    }

    #[test]
    fn a_still_screen_shows_a_key_going_down_and_up() {
        let display = display(InputDisplayCorner::BottomLeft);
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
        let display = display(InputDisplayCorner::TopLeft);
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
        let display = display(InputDisplayCorner::BottomLeft);
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
        assert!(
            differing(
                &fallback,
                &captured(&display(InputDisplayCorner::BottomLeft))
            )
            .is_empty()
        );
        assert_ne!(painted(&fallback), (u32::MAX, u32::MAX, 0, 0));
    }

    #[test]
    fn every_built_in_preset_can_be_chosen() {
        let pictures: Vec<_> = BuiltinPresetId::ALL
            .into_iter()
            .map(|preset| {
                painted(&captured(&LiveInputDisplay::new(InstantInputDisplay {
                    preset: preset.id().into(),
                    ..Default::default()
                })))
            })
            .collect();
        assert_ne!(pictures[0], pictures[1]);
    }

    fn gamepad_display() -> LiveInputDisplay {
        LiveInputDisplay::new(InstantInputDisplay {
            preset: BuiltinPresetId::Gamepad.id().into(),
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
        let wasd = display_with_controller(GamepadKind::PlayStation);

        assert!(differing(&first, &xbox).is_empty());
        assert!(differing(&playstation, &xbox).len() > 50);
        assert_eq!(painted(&playstation), painted(&xbox));
        assert!(differing(&wasd, &captured(&display(InputDisplayCorner::BottomLeft))).is_empty());
    }

    fn display_with_controller(controller: GamepadKind) -> ffmpeg::frame::Video {
        let display = display(InputDisplayCorner::BottomLeft);
        display.show(gamepad(controller, GamepadAxes::default(), &[]));
        captured(&display)
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
        let display = display(InputDisplayCorner::BottomLeft);
        let source = Scripted {
            reads: reads.clone(),
            down: down.clone(),
        };
        let recording = Arc::new(AtomicBool::new(false));
        let started = recording.clone();
        display.watch_with(
            move || vec![Box::new(source) as Box<dyn InputSource>],
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
            for preset in BuiltinPresetId::ALL {
                let display = LiveInputDisplay::new(InstantInputDisplay {
                    preset: preset.id().into(),
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
                    display.show(if down {
                        held(&[KEY_W, Input::MouseButton(1)])
                    } else {
                        DisplayState::default()
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
                    "{label} {}: display on, captured {held_frame:.2} us, captured with a key change {changing_frame:.2} us, still screen unchanged {still_idle:.2} us, still screen with a key change {still_changing:.2} us, one-time preparation {prepare:.1} ms",
                    preset.id()
                );
            }
        }
    }
}
