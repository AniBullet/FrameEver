use crate::{
    click_highlight::{ChannelOrder, ClickHighlights, Disc},
    live_input_display::{LiveInputDisplay, Painter},
};
use std::time::Instant;

type Bounds = (usize, usize, usize, usize);

#[derive(Clone, Default)]
pub struct FrameOverlays {
    pub click_highlight: Option<ClickHighlights>,
    pub input_display: Option<LiveInputDisplay>,
}

impl FrameOverlays {
    pub fn baker(&self) -> Option<FrameBaker> {
        (self.click_highlight.is_some() || self.input_display.is_some())
            .then(|| FrameBaker::new(self.click_highlight.clone(), self.input_display.clone()))
    }
}

#[derive(Default)]
struct Patch {
    left: usize,
    top: usize,
    width: usize,
    pixels: Vec<u8>,
}

impl Patch {
    fn save(pixels: &[u8], stride: usize, bounds: Bounds) -> Self {
        let mut patch = Self::default();
        patch.refill(pixels, stride, bounds);
        patch
    }

    fn refill(&mut self, pixels: &[u8], stride: usize, bounds: Bounds) {
        let (left, top, right, bottom) = bounds;
        self.left = left;
        self.top = top;
        self.width = (right - left) * 4;
        self.pixels.clear();
        for y in top..bottom {
            let start = y * stride + left * 4;
            self.pixels
                .extend_from_slice(&pixels[start..start + self.width]);
        }
    }

    fn restore(&self, pixels: &mut [u8], stride: usize) {
        if self.width == 0 {
            return;
        }
        for (row, saved) in self.pixels.chunks(self.width).enumerate() {
            let start = (self.top + row) * stride + self.left * 4;
            if let Some(target) = pixels.get_mut(start..start + self.width) {
                target.copy_from_slice(saved);
            }
        }
    }
}

pub struct FrameBaker {
    highlights: Option<ClickHighlights>,
    patches: Vec<Patch>,
    input_display: Option<Painter>,
    under_input_display: Patch,
}

fn channel_order(frame: &ffmpeg::frame::Video) -> Option<ChannelOrder> {
    match frame.format() {
        ffmpeg::format::Pixel::RGBA => Some(ChannelOrder::Rgba),
        ffmpeg::format::Pixel::BGRA => Some(ChannelOrder::Bgra),
        _ => None,
    }
}

impl FrameBaker {
    pub(crate) fn new(
        highlights: Option<ClickHighlights>,
        input_display: Option<LiveInputDisplay>,
    ) -> Self {
        Self {
            highlights,
            patches: Vec::new(),
            input_display: input_display.map(|display| display.painter()),
            under_input_display: Patch::default(),
        }
    }

    pub fn expect_frames_of(&mut self, size: (u32, u32)) {
        if let Some(painter) = &mut self.input_display {
            painter.prepare(size);
        }
    }

    fn discs(&self, now: Instant, size: (u32, u32)) -> Vec<Disc> {
        self.highlights
            .as_ref()
            .map(|highlights| highlights.discs(now, size))
            .unwrap_or_default()
    }

    pub fn captured(&mut self, frame: &mut ffmpeg::frame::Video, now: Instant) {
        self.patches.clear();
        let discs = self.discs(now, (frame.width(), frame.height()));
        self.paint_input_display(frame);
        self.paint_discs(frame, &discs);
    }

    pub fn repeated(
        &mut self,
        last: &ffmpeg::frame::Video,
        now: Instant,
    ) -> Option<ffmpeg::frame::Video> {
        let size = (last.width(), last.height());
        let discs = self.discs(now, size);
        let input_changed = self
            .input_display
            .as_ref()
            .is_some_and(|painter| painter.changed(size));
        if self.patches.is_empty() && discs.is_empty() && !input_changed {
            return None;
        }
        let mut frame = last.clone();
        let stride = frame.stride(0);
        let pixels = frame.data_mut(0);
        for patch in self.patches.drain(..) {
            patch.restore(pixels, stride);
        }
        if input_changed {
            self.under_input_display.restore(pixels, stride);
            self.paint_input_display(&mut frame);
        }
        self.paint_discs(&mut frame, &discs);
        Some(frame)
    }

    fn paint_input_display(&mut self, frame: &mut ffmpeg::frame::Video) {
        let (Some(painter), Some(order)) = (&mut self.input_display, channel_order(frame)) else {
            return;
        };
        let size = (frame.width(), frame.height());
        painter.prepare(size);
        let Some(bounds) = painter.bounds() else {
            return;
        };
        let stride = frame.stride(0);
        let pixels = frame.data_mut(0);
        self.under_input_display.refill(pixels, stride, bounds);
        painter.paint(pixels, stride, order);
    }

    fn paint_discs(&mut self, frame: &mut ffmpeg::frame::Video, discs: &[Disc]) {
        let Some(order) = channel_order(frame) else {
            return;
        };
        let size = (frame.width(), frame.height());
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
        for disc in discs {
            disc.paint(pixels, stride, size, order);
        }
    }
}
