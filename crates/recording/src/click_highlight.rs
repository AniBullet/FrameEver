use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};

const BASE_RADIUS: f64 = 18.0;
const AMBER: [u8; 3] = [0xF5, 0xAE, 0x39];
const START_OPACITY: f32 = 0.5;
pub const PULSE: Duration = Duration::from_millis(450);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureArea {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl CaptureArea {
    pub fn new(display: (f64, f64, f64, f64), crop: Option<(f64, f64, f64, f64)>) -> Self {
        let (x, y, width, height) = display;
        match crop {
            Some((crop_x, crop_y, crop_width, crop_height)) => Self {
                x: x + crop_x,
                y: y + crop_y,
                width: crop_width,
                height: crop_height,
            },
            None => Self {
                x,
                y,
                width,
                height,
            },
        }
    }

    pub fn radius(&self, scale_factor: f64, frame: (u32, u32)) -> f32 {
        (BASE_RADIUS * scale_factor * frame.0 as f64 / self.width.max(1.0)) as f32
    }

    pub fn frame_point(&self, cursor: (i32, i32), frame: (u32, u32)) -> (f32, f32) {
        let scale_x = frame.0 as f64 / self.width.max(1.0);
        let scale_y = frame.1 as f64 / self.height.max(1.0);
        (
            ((cursor.0 as f64 - self.x) * scale_x) as f32,
            ((cursor.1 as f64 - self.y) * scale_y) as f32,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pulse {
    size: f32,
    opacity: f32,
}

fn pulse(age: Duration) -> Option<Pulse> {
    if age >= PULSE {
        return None;
    }
    let progress = age.as_secs_f32() / PULSE.as_secs_f32();
    let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
    Some(Pulse {
        size: 0.55 + 0.45 * eased,
        opacity: START_OPACITY * (1.0 - progress),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelOrder {
    Rgba,
    Bgra,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Disc {
    center: (f32, f32),
    radius: f32,
    opacity: f32,
}

impl Disc {
    fn bounds(&self, frame: (u32, u32)) -> Option<(usize, usize, usize, usize)> {
        let reach = self.radius + 1.0;
        let left = (self.center.0 - reach).floor().max(0.0);
        let top = (self.center.1 - reach).floor().max(0.0);
        let right = (self.center.0 + reach).ceil().min(frame.0 as f32);
        let bottom = (self.center.1 + reach).ceil().min(frame.1 as f32);
        (left < right && top < bottom).then_some((
            left as usize,
            top as usize,
            right as usize,
            bottom as usize,
        ))
    }

    fn paint(&self, pixels: &mut [u8], stride: usize, frame: (u32, u32), order: ChannelOrder) {
        let Some((left, top, right, bottom)) = self.bounds(frame) else {
            return;
        };
        let color = match order {
            ChannelOrder::Rgba => AMBER,
            ChannelOrder::Bgra => [AMBER[2], AMBER[1], AMBER[0]],
        }
        .map(f32::from);
        for y in top..bottom {
            let dy = y as f32 + 0.5 - self.center.1;
            let row = &mut pixels[y * stride..];
            for x in left..right {
                let dx = x as f32 + 0.5 - self.center.0;
                let coverage = (self.radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
                if coverage <= 0.0 {
                    continue;
                }
                let weight = coverage * self.opacity;
                let pixel = &mut row[x * 4..x * 4 + 3];
                for (channel, target) in pixel.iter_mut().zip(color) {
                    let value = f32::from(*channel);
                    *channel = (value + (target - value) * weight).round() as u8;
                }
            }
        }
    }
}

struct Shared {
    clicks: Mutex<VecDeque<(Instant, (i32, i32))>>,
    area: CaptureArea,
    scale_factor: f64,
}

#[derive(Clone)]
pub struct ClickHighlights(Arc<Shared>);

impl ClickHighlights {
    pub fn new(area: CaptureArea, scale_factor: f64) -> Self {
        Self(Arc::new(Shared {
            clicks: Mutex::new(VecDeque::new()),
            area,
            scale_factor,
        }))
    }

    pub fn press(&self, at: Instant, cursor: (i32, i32)) {
        let mut clicks = self.0.clicks.lock();
        while clicks
            .front()
            .is_some_and(|(pressed, _)| at.saturating_duration_since(*pressed) >= PULSE)
        {
            clicks.pop_front();
        }
        clicks.push_back((at, cursor));
    }

    fn discs(&self, now: Instant, frame: (u32, u32)) -> Vec<Disc> {
        let mut clicks = self.0.clicks.lock();
        clicks.retain(|(pressed, _)| now.saturating_duration_since(*pressed) < PULSE);
        let radius = self.0.area.radius(self.0.scale_factor, frame);
        clicks
            .iter()
            .filter_map(|(pressed, cursor)| {
                let pulse = pulse(now.saturating_duration_since(*pressed))?;
                Some(Disc {
                    center: self.0.area.frame_point(*cursor, frame),
                    radius: radius * pulse.size,
                    opacity: pulse.opacity,
                })
            })
            .collect()
    }

    pub fn baker(&self) -> FrameBaker {
        FrameBaker {
            highlights: self.clone(),
            patches: Vec::new(),
        }
    }

    pub fn watch(&self) {
        let shared = Arc::downgrade(&self.0);
        let spawned = std::thread::Builder::new()
            .name("click-highlight".into())
            .spawn(move || watch_buttons(shared));
        if let Err(error) = spawned {
            tracing::warn!("click highlight watcher did not start: {error}");
        }
    }
}

fn watch_buttons(shared: Weak<Shared>) {
    use device_query::{DeviceQuery, DeviceState};
    let device = DeviceState::new();
    let mut previous = device.get_mouse().button_pressed;
    loop {
        std::thread::sleep(Duration::from_millis(8));
        let Some(shared) = shared.upgrade() else {
            return;
        };
        let mouse = device.get_mouse();
        let pressed = mouse
            .button_pressed
            .iter()
            .zip(&previous)
            .any(|(now, before)| *now && !*before);
        if pressed {
            ClickHighlights(shared).press(Instant::now(), mouse.coords);
        }
        previous = mouse.button_pressed;
    }
}

struct Patch {
    left: usize,
    top: usize,
    width: usize,
    pixels: Vec<u8>,
}

impl Patch {
    fn save(pixels: &[u8], stride: usize, bounds: (usize, usize, usize, usize)) -> Self {
        let (left, top, right, bottom) = bounds;
        let width = (right - left) * 4;
        let mut saved = Vec::with_capacity(width * (bottom - top));
        for y in top..bottom {
            let start = y * stride + left * 4;
            saved.extend_from_slice(&pixels[start..start + width]);
        }
        Self {
            left,
            top,
            width,
            pixels: saved,
        }
    }

    fn restore(&self, pixels: &mut [u8], stride: usize) {
        for (row, saved) in self.pixels.chunks(self.width).enumerate() {
            let start = (self.top + row) * stride + self.left * 4;
            if let Some(target) = pixels.get_mut(start..start + self.width) {
                target.copy_from_slice(saved);
            }
        }
    }
}

pub struct FrameBaker {
    highlights: ClickHighlights,
    patches: Vec<Patch>,
}

fn channel_order(frame: &ffmpeg::frame::Video) -> Option<ChannelOrder> {
    match frame.format() {
        ffmpeg::format::Pixel::RGBA => Some(ChannelOrder::Rgba),
        ffmpeg::format::Pixel::BGRA => Some(ChannelOrder::Bgra),
        _ => None,
    }
}

impl FrameBaker {
    pub fn captured(&mut self, frame: &mut ffmpeg::frame::Video, now: Instant) {
        self.patches.clear();
        self.paint(frame, now);
    }

    pub fn repeated(
        &mut self,
        last: &ffmpeg::frame::Video,
        now: Instant,
    ) -> Option<ffmpeg::frame::Video> {
        let size = (last.width(), last.height());
        if self.patches.is_empty() && self.highlights.discs(now, size).is_empty() {
            return None;
        }
        let mut frame = last.clone();
        let stride = frame.stride(0);
        let pixels = frame.data_mut(0);
        for patch in self.patches.drain(..) {
            patch.restore(pixels, stride);
        }
        self.paint(&mut frame, now);
        Some(frame)
    }

    fn paint(&mut self, frame: &mut ffmpeg::frame::Video, now: Instant) {
        let Some(order) = channel_order(frame) else {
            return;
        };
        let size = (frame.width(), frame.height());
        let discs = self.highlights.discs(now, size);
        if discs.is_empty() {
            return;
        }
        let stride = frame.stride(0);
        let pixels = frame.data_mut(0);
        self.patches.extend(
            discs
                .iter()
                .filter_map(|disc| disc.bounds(size))
                .map(|bounds| Patch::save(pixels, stride, bounds)),
        );
        for disc in &discs {
            disc.paint(pixels, stride, size, order);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn black_frame(width: u32, height: u32) -> ffmpeg::frame::Video {
        let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGBA, width, height);
        let stride = frame.stride(0);
        let length = frame.data(0).len();
        let pixels = black(width as usize, height as usize, stride);
        frame.data_mut(0).copy_from_slice(&pixels[..length]);
        frame
    }

    fn frame_pixel(frame: &ffmpeg::frame::Video, x: usize, y: usize) -> [u8; 4] {
        pixel(frame.data(0), frame.stride(0), x, y)
    }

    fn highlights() -> ClickHighlights {
        ClickHighlights::new(CaptureArea::new((1920.0, 0.0, 200.0, 200.0), None), 1.0)
    }

    #[test]
    fn a_captured_frame_shows_a_click_only_while_it_lasts() {
        let highlights = highlights();
        let mut baker = highlights.baker();
        let start = Instant::now();
        let mut idle = black_frame(200, 200);
        baker.captured(&mut idle, start);
        assert_eq!(frame_pixel(&idle, 100, 100), [0, 0, 0, 255]);
        highlights.press(start, (1920 + 100, 100));
        let mut clicked = black_frame(200, 200);
        baker.captured(&mut clicked, start + Duration::from_millis(10));
        assert_ne!(frame_pixel(&clicked, 100, 100), [0, 0, 0, 255]);
        assert_eq!(frame_pixel(&clicked, 0, 0), [0, 0, 0, 255]);
        let mut later = black_frame(200, 200);
        baker.captured(&mut later, start + PULSE);
        assert_eq!(frame_pixel(&later, 100, 100), [0, 0, 0, 255]);
    }

    #[test]
    fn a_still_screen_still_shows_the_click_and_then_clears_it() {
        let highlights = highlights();
        let mut baker = highlights.baker();
        let start = Instant::now();
        let mut last = black_frame(200, 200);
        baker.captured(&mut last, start);
        assert!(baker.repeated(&last, start).is_none());
        highlights.press(start, (1920 + 100, 100));
        let early = baker
            .repeated(&last, start + Duration::from_millis(10))
            .unwrap();
        assert_eq!(frame_pixel(&last, 100, 100), [0, 0, 0, 255]);
        let early_pixel = frame_pixel(&early, 100, 100);
        assert_ne!(early_pixel, [0, 0, 0, 255]);
        let middle = baker.repeated(&early, start + PULSE / 2).unwrap();
        let middle_pixel = frame_pixel(&middle, 100, 100);
        assert!(middle_pixel[0] < early_pixel[0], "{middle_pixel:?}");
        assert_ne!(middle_pixel, [0, 0, 0, 255]);
        let cleared = baker.repeated(&middle, start + PULSE).unwrap();
        for y in 70..130 {
            for x in 70..130 {
                assert_eq!(frame_pixel(&cleared, x, y), [0, 0, 0, 255]);
            }
        }
        assert!(baker.repeated(&cleared, start + PULSE * 2).is_none());
    }

    #[test]
    fn the_highlight_keeps_its_size_on_screen_at_every_scaling() {
        let area = CaptureArea::new((0.0, 0.0, 1920.0, 1080.0), None);
        assert_eq!(area.radius(1.0, (1920, 1080)), 18.0);
        let area = CaptureArea::new((0.0, 0.0, 2880.0, 1620.0), None);
        assert_eq!(area.radius(1.5, (2880, 1620)), 27.0);
        let area = CaptureArea::new((0.0, 0.0, 3840.0, 2160.0), None);
        assert_eq!(area.radius(2.0, (3840, 2160)), 36.0);
        assert_eq!(area.radius(2.0, (1920, 1080)), 18.0);
    }

    #[test]
    fn a_click_grows_and_fades_then_is_gone() {
        let start = pulse(Duration::ZERO).unwrap();
        let middle = pulse(PULSE / 2).unwrap();
        assert!(start.size < middle.size && middle.size < 1.0);
        assert!(start.opacity > middle.opacity && middle.opacity > 0.0);
        assert_eq!(start.opacity, 0.5);
        assert!(pulse(PULSE).is_none());
    }

    fn black(width: usize, height: usize, stride: usize) -> Vec<u8> {
        let mut pixels = vec![0u8; stride * height];
        for row in pixels.chunks_mut(stride) {
            for pixel in row[..width * 4].chunks_mut(4) {
                pixel[3] = 255;
            }
        }
        pixels
    }

    fn pixel(pixels: &[u8], stride: usize, x: usize, y: usize) -> [u8; 4] {
        let at = y * stride + x * 4;
        pixels[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn a_disc_blends_amber_inside_and_leaves_the_rest_alone() {
        let stride = 64 * 4 + 16;
        let mut pixels = black(64, 64, stride);
        let disc = Disc {
            center: (32.0, 32.0),
            radius: 10.0,
            opacity: 0.5,
        };
        disc.paint(&mut pixels, stride, (64, 64), ChannelOrder::Rgba);
        let center = pixel(&pixels, stride, 32, 32);
        for (channel, expected) in [(0, 123), (1, 87), (2, 29)] {
            assert!(center[channel].abs_diff(expected) <= 1, "{center:?}");
        }
        assert_eq!(center[3], 255);
        assert_eq!(pixel(&pixels, stride, 32, 44), [0, 0, 0, 255]);
        assert_eq!(pixel(&pixels, stride, 0, 0), [0, 0, 0, 255]);
        assert!(pixels[64 * 4..stride].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn a_disc_in_bgra_frames_swaps_red_and_blue() {
        let mut pixels = black(8, 8, 32);
        Disc {
            center: (4.0, 4.0),
            radius: 3.0,
            opacity: 1.0,
        }
        .paint(&mut pixels, 32, (8, 8), ChannelOrder::Bgra);
        assert_eq!(pixel(&pixels, 32, 4, 4), [57, 174, 245, 255]);
    }

    #[test]
    fn a_disc_at_the_edge_paints_only_what_is_in_the_frame() {
        let mut pixels = black(16, 16, 64);
        for center in [(0.0, 0.0), (15.5, 15.5), (-5.0, 8.0), (40.0, 40.0)] {
            Disc {
                center,
                radius: 8.0,
                opacity: 1.0,
            }
            .paint(&mut pixels, 64, (16, 16), ChannelOrder::Rgba);
        }
        assert_eq!(pixel(&pixels, 64, 0, 0), [245, 174, 57, 255]);
        assert_eq!(pixel(&pixels, 64, 15, 15), [245, 174, 57, 255]);
        assert_eq!(pixel(&pixels, 64, 15, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn a_click_lands_on_the_same_pixel_at_100_150_and_200_percent() {
        for (display, cursor, expected) in [
            ((0.0, 0.0, 1920.0, 1080.0), (960, 540), (960.0, 540.0)),
            ((0.0, 0.0, 2880.0, 1620.0), (1440, 810), (1440.0, 810.0)),
            ((0.0, 0.0, 3840.0, 2160.0), (100, 2000), (100.0, 2000.0)),
        ] {
            let area = CaptureArea::new(display, None);
            assert_eq!(
                area.frame_point(cursor, (display.2 as u32, display.3 as u32)),
                expected
            );
        }
    }

    #[test]
    fn a_second_display_left_of_or_above_the_main_one_is_measured_from_its_own_corner() {
        let left = CaptureArea::new((-2560.0, -200.0, 2560.0, 1440.0), None);
        assert_eq!(
            left.frame_point((-2460, -100), (2560, 1440)),
            (100.0, 100.0)
        );
        let right = CaptureArea::new((1920.0, 0.0, 3840.0, 2160.0), None);
        assert_eq!(
            right.frame_point((1920 + 3000, 1500), (3840, 2160)),
            (3000.0, 1500.0)
        );
    }

    #[test]
    fn an_area_or_window_is_measured_from_its_corner_on_its_display() {
        let area = CaptureArea::new(
            (1920.0, 0.0, 2880.0, 1620.0),
            Some((300.0, 150.0, 1200.0, 800.0)),
        );
        assert_eq!(area.frame_point((1920 + 300, 150), (1200, 800)), (0.0, 0.0));
        assert_eq!(
            area.frame_point((1920 + 900, 550), (1200, 800)),
            (600.0, 400.0)
        );
        assert_eq!(
            area.frame_point((1920 + 200, 100), (1200, 800)),
            (-100.0, -50.0)
        );
    }

    #[test]
    fn a_frame_smaller_than_the_area_scales_the_point_with_it() {
        let area = CaptureArea::new((0.0, 0.0, 3840.0, 2160.0), None);
        assert_eq!(area.frame_point((1920, 1080), (1920, 1080)), (960.0, 540.0));
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
    #[ignore = "timing run: cargo test -p frameever-recording --lib click_highlight -- --ignored --nocapture"]
    fn bake_cost_per_frame() {
        for (label, width, height, scale) in
            [("1080p", 1920u32, 1080u32, 1.0), ("4K", 3840, 2160, 2.0)]
        {
            let highlights = ClickHighlights::new(
                CaptureArea::new((0.0, 0.0, width as f64, height as f64), None),
                scale,
            );
            let mut baker = highlights.baker();
            let start = Instant::now();
            let mut frame = black_frame(width, height);
            let idle = median_micros(|| baker.captured(&mut frame, start));
            let repeat_idle = median_micros(|| {
                assert!(baker.repeated(&frame, start).is_none());
            });
            highlights.press(start, (width as i32 / 2, height as i32 / 2));
            let now = start + Duration::from_millis(1);
            let clicked = median_micros(|| baker.captured(&mut frame, now));
            let repeat_clicked = median_micros(|| {
                assert!(baker.repeated(&frame, now).is_some());
            });
            let copy = median_micros(|| {
                std::hint::black_box(frame.clone());
            });
            println!(
                "{label}: captured idle {idle:.2} us, captured with click {clicked:.2} us, repeated idle {repeat_idle:.2} us, repeated with click {repeat_clicked:.2} us, plain frame copy {copy:.2} us"
            );
        }
    }
}
