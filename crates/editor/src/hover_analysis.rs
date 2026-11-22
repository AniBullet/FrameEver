use std::{
    collections::VecDeque,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use frameever_project::CursorEvents;
use serde::{Deserialize, Serialize};

const DETECTOR_VERSION: u32 = 1;
const CACHE_EXTENSION: &str = "cursor-hover.json";
const DECODE_THREADS_THAT_KEEP_THE_CALLERS_PRIORITY: usize = 1;
const WAIT_POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Schedule {
    Run,
    Wait,
    Stop,
}

fn wait_for_turn(schedule: &dyn Fn() -> Schedule) -> bool {
    loop {
        match schedule() {
            Schedule::Run => return true,
            Schedule::Stop => return false,
            Schedule::Wait => std::thread::sleep(WAIT_POLL),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HoverCache {
    version: u32,
    display_bytes: u64,
    changes_ms: Vec<f64>,
}

fn cache_path(display: &Path) -> PathBuf {
    display.with_extension(CACHE_EXTENSION)
}

pub(crate) fn cached_changes(display: &Path) -> Option<Vec<f64>> {
    let file = std::fs::read(cache_path(display)).ok()?;
    let cache: HoverCache = serde_json::from_slice(&file).ok()?;
    let display_bytes = std::fs::metadata(display).ok()?.len();
    (cache.version == DETECTOR_VERSION && cache.display_bytes == display_bytes)
        .then_some(cache.changes_ms)
}

fn write_cache(display: &Path, changes_ms: &[f64]) -> Result<(), String> {
    let folder = display.parent().ok_or("display video has no folder")?;
    let cache = HoverCache {
        version: DETECTOR_VERSION,
        display_bytes: std::fs::metadata(display)
            .map_err(|error| format!("read display video size: {error}"))?
            .len(),
        changes_ms: changes_ms.to_vec(),
    };
    let json = serde_json::to_vec(&cache).map_err(|error| error.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(folder).map_err(|error| error.to_string())?;
    file.write_all(&json).map_err(|error| error.to_string())?;
    file.persist(cache_path(display))
        .map_err(|error| format!("save hover changes: {error}"))?;
    Ok(())
}

pub(crate) fn analyze(
    display: &Path,
    cursor: &CursorEvents,
    cursor_offset_ms: f64,
    schedule: &dyn Fn() -> Schedule,
) -> Result<Option<Vec<f64>>, String> {
    if !wait_for_turn(schedule) {
        return Ok(None);
    }
    let mut input = ffmpeg::format::input(display).map_err(|e| format!("open video: {e}"))?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or("no video stream")?;
    let stream_index = stream.index();
    let time_base = stream.time_base();
    let ms_per_tick = f64::from(time_base) * 1000.0;

    let mut context = ffmpeg::codec::Context::from_parameters(stream.parameters())
        .map_err(|e| format!("video parameters: {e}"))?;
    context.set_threading(ffmpeg::threading::Config::count(
        DECODE_THREADS_THAT_KEEP_THE_CALLERS_PRIORITY,
    ));
    let mut decoder = context
        .decoder()
        .video()
        .map_err(|e| format!("video decoder: {e}"))?;
    decoder.set_packet_time_base(time_base);

    let (source_width, source_height) = (decoder.width(), decoder.height());
    if source_width == 0 || source_height == 0 {
        return Err("display video has no size".into());
    }
    let (width, height) = analysis_size(source_width, source_height);
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        source_width,
        source_height,
        ffmpeg::format::Pixel::GRAY8,
        width as u32,
        height as u32,
        ffmpeg::software::scaling::flag::Flags::AREA,
    )
    .map_err(|e| format!("create scaler: {e}"))?;

    let pointer = PointerTrack::new(cursor);
    let mut detector = HoverDetector::new(width, height);
    let mut decoded = ffmpeg::frame::Video::empty();
    let mut gray = ffmpeg::frame::Video::empty();
    let mut drain = |decoder: &mut ffmpeg::decoder::Video,
                     detector: &mut HoverDetector|
     -> Result<bool, String> {
        while decoder.receive_frame(&mut decoded).is_ok() {
            if !wait_for_turn(schedule) {
                return Ok(false);
            }
            scaler
                .run(&decoded, &mut gray)
                .map_err(|e| format!("scale frame: {e}"))?;
            let ticks = decoded.timestamp().or_else(|| decoded.pts()).unwrap_or(0);
            let time_ms = ticks as f64 * ms_per_tick + cursor_offset_ms;
            let mut luma = Vec::with_capacity(width * height);
            for row in gray.data(0).chunks(gray.stride(0)).take(height) {
                luma.extend_from_slice(&row[..width]);
            }
            let position = pointer.position(time_ms).and_then(|(x, y)| {
                let (x, y) = (x * width as f64, y * height as f64);
                (x >= 0.0 && y >= 0.0 && x < width as f64 && y < height as f64)
                    .then_some((x as usize, y as usize))
            });
            detector.push(time_ms, luma, position, pointer.busy(time_ms));
        }
        Ok(true)
    };

    for (packet_stream, packet) in input.packets() {
        if !wait_for_turn(schedule) {
            return Ok(None);
        }
        if packet_stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(|e| format!("send packet: {e}"))?;
        if !drain(&mut decoder, &mut detector)? {
            return Ok(None);
        }
    }
    decoder.send_eof().map_err(|e| format!("send eof: {e}"))?;
    if !drain(&mut decoder, &mut detector)? || !wait_for_turn(schedule) {
        return Ok(None);
    }

    let changes = detector.finish();
    write_cache(display, &changes)?;
    Ok(Some(changes))
}

fn analysis_size(source_width: u32, source_height: u32) -> (usize, usize) {
    let width = ((source_width / 2).clamp(320, 720) / 2) * 2;
    let scaled_height = u64::from(source_height) * u64::from(width) / u64::from(source_width);
    (width as usize, (scaled_height.max(2) / 2 * 2) as usize)
}

struct PointerTrack {
    moves: Vec<(f64, f64, f64)>,
    clicks: Vec<f64>,
    held: Vec<(f64, f64)>,
}

const CLICK_GUARD_MS: f64 = 150.0;
const IDLE_GAP_MS: f64 = 66.0;

impl PointerTrack {
    fn new(cursor: &CursorEvents) -> Self {
        let mut moves: Vec<(f64, f64, f64)> =
            cursor.moves.iter().map(|m| (m.time_ms, m.x, m.y)).collect();
        moves.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut clicks: Vec<_> = cursor.clicks.iter().collect();
        clicks.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

        let mut held = Vec::new();
        let mut pressed = [false; 2];
        let mut first_press_ms = None;
        for click in &clicks {
            let button = usize::from(click.cursor_num);
            if button >= pressed.len() {
                continue;
            }
            if click.down {
                pressed[button] = true;
                first_press_ms.get_or_insert(click.time_ms);
            } else if std::mem::take(&mut pressed[button])
                && !pressed.contains(&true)
                && let Some(start) = first_press_ms.take()
            {
                held.push((start, click.time_ms));
            }
        }
        if let Some(start) = first_press_ms {
            held.push((start, f64::INFINITY));
        }
        Self {
            moves,
            clicks: clicks.iter().map(|click| click.time_ms).collect(),
            held,
        }
    }

    fn position(&self, time_ms: f64) -> Option<(f64, f64)> {
        let moves = &self.moves;
        let next = moves.partition_point(|m| m.0 <= time_ms);
        let (before, after) = match (next.checked_sub(1).map(|i| moves[i]), moves.get(next)) {
            (None, None) => return None,
            (None, Some(after)) => return Some((after.1, after.2)),
            (Some(before), None) => return Some((before.1, before.2)),
            (Some(before), Some(after)) => (before, *after),
        };
        if after.0 - before.0 > IDLE_GAP_MS {
            return Some((before.1, before.2));
        }
        let u = (time_ms - before.0) / (after.0 - before.0).max(1e-9);
        Some((
            before.1 + (after.1 - before.1) * u,
            before.2 + (after.2 - before.2) * u,
        ))
    }

    fn busy(&self, time_ms: f64) -> bool {
        let click = self
            .clicks
            .partition_point(|&click| click < time_ms - CLICK_GUARD_MS);
        let near_click = self
            .clicks
            .get(click)
            .is_some_and(|&click| click <= time_ms + CLICK_GUARD_MS);
        let hold = self.held.partition_point(|&(_, end)| end < time_ms);
        near_click
            || self
                .held
                .get(hold)
                .is_some_and(|&(start, _)| start <= time_ms)
    }
}

struct Frame {
    time_ms: f64,
    luma: Vec<u8>,
    pointer: Option<(usize, usize)>,
    busy: bool,
    screen_fraction_changed: f32,
}

const TUNING_WIDTH: f32 = 395.0;
const PROBE_RADIUS: f32 = 26.0;
const MAX_SCROLL_SHIFT: f32 = 60.0;
const STILL_POINTER: f32 = 3.0;
const PIXEL_THRESHOLD: u8 = 12;
const ACTIVE_FRACTION: f32 = 0.004;
const NET_CHANGE_FRACTION: f32 = 0.02;
const MAX_BURST_MS: f64 = 250.0;
const QUIET_MS: f64 = 100.0;
const MAX_SCREEN_FRACTION: f32 = 0.2;
const SCREEN_SAMPLE_STEP: usize = 7;
const SCROLL_RATIO: f32 = 0.5;
const MIN_SCROLLING_TEXTURE: f32 = 6.0;
const MERGE_MS: f64 = 120.0;
const RECENT_FRAMES: usize = 48;

#[derive(Clone, Copy)]
struct Burst {
    start: usize,
    end: usize,
    end_ms: f64,
    quiet_before: bool,
}

struct HoverDetector {
    width: usize,
    height: usize,
    probe_radius: usize,
    max_scroll_shift: i32,
    still_pointer: f32,
    frames: VecDeque<Frame>,
    first_index: usize,
    burst: Option<Burst>,
    last_active_ms: Option<f64>,
    changes: Vec<f64>,
}

impl HoverDetector {
    fn new(width: usize, height: usize) -> Self {
        let scale = width as f32 / TUNING_WIDTH;
        Self {
            width,
            height,
            probe_radius: (PROBE_RADIUS * scale).round() as usize,
            max_scroll_shift: (MAX_SCROLL_SHIFT * scale).round() as i32,
            still_pointer: STILL_POINTER * scale,
            frames: VecDeque::with_capacity(RECENT_FRAMES),
            first_index: 0,
            burst: None,
            last_active_ms: None,
            changes: Vec::new(),
        }
    }

    fn frame(&self, index: usize) -> Option<&Frame> {
        self.frames.get(index.checked_sub(self.first_index)?)
    }

    fn push(&mut self, time_ms: f64, luma: Vec<u8>, pointer: Option<(usize, usize)>, busy: bool) {
        let mut frame = Frame {
            time_ms,
            luma,
            pointer,
            busy,
            screen_fraction_changed: 0.0,
        };
        let mut active = false;
        if let Some(previous) = self.frames.back() {
            let activity = [frame.pointer, previous.pointer]
                .into_iter()
                .flatten()
                .map(|(x, y)| self.changed_fraction(&previous.luma, &frame.luma, x, y))
                .fold(0.0, f32::max);
            active = activity >= ACTIVE_FRACTION;
            frame.screen_fraction_changed = screen_fraction_changed(&previous.luma, &frame.luma);
        }

        let index = self.first_index + self.frames.len();
        if self.frames.len() == RECENT_FRAMES {
            self.frames.pop_front();
            self.first_index += 1;
        }
        self.frames.push_back(frame);

        if active {
            match self.burst {
                Some(burst) if time_ms - burst.end_ms < QUIET_MS => {
                    self.burst = Some(Burst {
                        end: index,
                        end_ms: time_ms,
                        ..burst
                    });
                }
                previous => {
                    if let Some(burst) = previous {
                        self.evaluate(burst, index - 1);
                    }
                    let quiet_before = self
                        .last_active_ms
                        .is_none_or(|last| time_ms - last >= QUIET_MS);
                    self.burst = Some(Burst {
                        start: index,
                        end: index,
                        end_ms: time_ms,
                        quiet_before,
                    });
                }
            }
            self.last_active_ms = Some(time_ms);
        } else if let Some(burst) = self.burst
            && time_ms - burst.end_ms >= QUIET_MS
        {
            self.burst = None;
            self.evaluate(burst, index);
        }
    }

    fn evaluate(&mut self, burst: Burst, after: usize) {
        let Burst {
            start,
            end,
            quiet_before,
            ..
        } = burst;
        if !quiet_before {
            return;
        }
        let (Some(first), Some(last), Some(before)) =
            (self.frame(start), self.frame(end), self.frame(start - 1))
        else {
            return;
        };
        if last.time_ms - first.time_ms > MAX_BURST_MS {
            return;
        }
        let Some((cx, cy)) = first.pointer.or(before.pointer) else {
            return;
        };
        let net_change = (start..=after)
            .filter_map(|index| self.frame(index))
            .flat_map(|frame| {
                [first.pointer, before.pointer]
                    .into_iter()
                    .flatten()
                    .map(|(x, y)| self.changed_fraction(&before.luma, &frame.luma, x, y))
            })
            .fold(0.0, f32::max);
        if net_change < NET_CHANGE_FRACTION {
            return;
        }
        let busy_or_screen_wide = (start..=end).any(|index| {
            self.frame(index).is_none_or(|frame| {
                frame.busy || frame.screen_fraction_changed > MAX_SCREEN_FRACTION
            })
        });
        if busy_or_screen_wide {
            return;
        }
        let scrolled = self.content_moved_past_pointer(start - 1, after, cx, cy)
            || (start..=end).any(|index| self.content_moved_past_pointer(index - 1, index, cx, cy));
        if scrolled {
            return;
        }
        let time_ms = first.time_ms;
        if self
            .changes
            .last()
            .is_none_or(|&last| time_ms - last > MERGE_MS)
        {
            self.changes.push(time_ms);
        }
    }

    fn finish(mut self) -> Vec<f64> {
        if let Some(burst) = self.burst.take() {
            let last = self.first_index + self.frames.len() - 1;
            self.evaluate(burst, last);
        }
        self.changes
    }

    fn probe_bounds(&self, cx: usize, cy: usize) -> (usize, usize, usize, usize) {
        let radius = self.probe_radius;
        (
            cx.saturating_sub(radius),
            (cx + radius).min(self.width - 1),
            cy.saturating_sub(radius),
            (cy + radius).min(self.height - 1),
        )
    }

    fn changed_fraction(&self, a: &[u8], b: &[u8], cx: usize, cy: usize) -> f32 {
        let (left, right, top, bottom) = self.probe_bounds(cx, cy);
        let mut changed = 0u32;
        for y in top..=bottom {
            let row = y * self.width;
            let span = row + left..=row + right;
            for (a, b) in a[span.clone()].iter().zip(&b[span]) {
                changed += u32::from(a.abs_diff(*b) > PIXEL_THRESHOLD);
            }
        }
        changed as f32 / ((right + 1 - left) * (bottom + 1 - top)) as f32
    }

    fn mean_difference(&self, a: &[u8], b: &[u8], cx: usize, cy: usize, dx: i32, dy: i32) -> f32 {
        let (left, right, top, bottom) = self.probe_bounds(cx, cy);
        let left = (left as i32).max(-dx);
        let right = (right as i32).min(self.width as i32 - 1 - dx);
        let top = (top as i32).max(-dy);
        let bottom = (bottom as i32).min(self.height as i32 - 1 - dy);
        if left > right || top > bottom {
            return f32::INFINITY;
        }
        let span = (right + 1 - left) as usize;
        let mut sum = 0u64;
        for y in top..=bottom {
            let shifted = (y + dy) as usize * self.width + (left + dx) as usize;
            let origin = y as usize * self.width + left as usize;
            let row_sum: u32 = a[shifted..shifted + span]
                .iter()
                .zip(&b[origin..origin + span])
                .map(|(a, b)| u32::from(a.abs_diff(*b)))
                .sum();
            sum += u64::from(row_sum);
        }
        sum as f32 / (span * (bottom + 1 - top) as usize) as f32
    }

    fn texture(&self, luma: &[u8], cx: usize, cy: usize) -> f32 {
        let (left, right, top, bottom) = self.probe_bounds(cx, cy);
        let mut sum = 0f32;
        let mut sum_of_squares = 0f32;
        for y in top..=bottom {
            for &value in &luma[y * self.width + left..=y * self.width + right] {
                let value = f32::from(value);
                sum += value;
                sum_of_squares += value * value;
            }
        }
        let count = ((right + 1 - left) * (bottom + 1 - top)) as f32;
        let mean = sum / count;
        (sum_of_squares / count - mean * mean).max(0.0).sqrt()
    }

    fn content_moved_past_pointer(&self, from: usize, to: usize, cx: usize, cy: usize) -> bool {
        let (Some(from), Some(to)) = (self.frame(from), self.frame(to)) else {
            return false;
        };
        let pointer_travel = match (from.pointer, to.pointer) {
            (Some(a), Some(b)) => (a.0 as f32 - b.0 as f32).hypot(a.1 as f32 - b.1 as f32),
            _ => 0.0,
        };
        if pointer_travel >= self.still_pointer
            || self.texture(&to.luma, cx, cy) < MIN_SCROLLING_TEXTURE
        {
            return false;
        }
        let unmoved = self.mean_difference(&from.luma, &to.luma, cx, cy, 0, 0);
        if unmoved < 0.5 {
            return false;
        }
        (1..=self.max_scroll_shift)
            .filter(|shift| *shift <= 8 || shift % 2 == 0)
            .flat_map(|shift| [(0, shift), (0, -shift), (shift, 0), (-shift, 0)])
            .any(|(dx, dy)| {
                self.mean_difference(&from.luma, &to.luma, cx, cy, dx, dy) < unmoved * SCROLL_RATIO
            })
    }
}

fn screen_fraction_changed(a: &[u8], b: &[u8]) -> f32 {
    let mut changed = 0u32;
    let mut total = 0u32;
    for (a, b) in a.iter().zip(b).step_by(SCREEN_SAMPLE_STEP) {
        total += 1;
        changed += u32::from(a.abs_diff(*b) > PIXEL_THRESHOLD);
    }
    changed as f32 / total.max(1) as f32
}

#[cfg(test)]
pub(crate) mod tests;
