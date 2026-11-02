use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{AsyncWindowContext, Bounds, Context, Pixels, RenderImage, Task, WeakEntity, Window};

use super::{MainWindow, TILE_PADDING};
use crate::library::{
    MediaKind,
    player::{
        self, Hold, HoldStop,
        strip::{self, Built, StripKey, Strips},
    },
    view::Entry,
};

const HOVER_WAIT: Duration = Duration::from_millis(200);
const TILE_BORDER: f32 = 1.;

struct Hover {
    path: PathBuf,
    line: f32,
}

struct Job {
    key: StripKey,
    stop: HoldStop,
    _task: Task<()>,
}

#[derive(Default)]
pub(super) struct ScrubState {
    strips: Strips<Arc<RenderImage>>,
    hover: Option<Hover>,
    wait: Option<Task<()>>,
    job: Option<Job>,
}

impl ScrubState {
    fn stop_job(&mut self) {
        self.wait = None;
        if let Some(job) = self.job.take() {
            job.stop.stop();
        }
    }
}

fn scrubbable(entry: &Entry) -> bool {
    entry.kind != MediaKind::Screenshot && entry.duration.is_some_and(|duration| duration > 0.)
}

fn thumb_fraction(x: Pixels, tile: Bounds<Pixels>) -> f32 {
    let inset = TILE_PADDING + TILE_BORDER;
    let width = (f32::from(tile.size.width) - inset * 2.).max(1.);
    (f32::from(x - tile.origin.x) - inset) / width
}

impl MainWindow {
    pub(super) fn scrub_library_tile(
        &mut self,
        path: &Path,
        x: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = &self.library;
        if page.marquee.is_some() || page.overlay.is_some() {
            return;
        }
        let Some(tile) = page.drawn.borrow().get(path).copied() else {
            return;
        };
        let Some(entry) = page.entry(path).filter(|entry| scrubbable(entry)) else {
            return;
        };
        let key = StripKey::of(entry);
        let line = strip::line_at(thumb_fraction(x, tile));
        let hover = &page.scrub.hover;
        let arrived = hover.as_ref().is_none_or(|hover| hover.path != path);
        let moved = hover.as_ref().is_none_or(|hover| hover.line != line);
        self.library.scrub.hover = Some(Hover {
            path: path.to_path_buf(),
            line,
        });
        if arrived {
            self.queue_library_strip(key, window, cx);
        }
        if moved {
            cx.notify();
        }
    }

    fn queue_library_strip(&mut self, key: StripKey, window: &mut Window, cx: &mut Context<Self>) {
        let scrub = &mut self.library.scrub;
        scrub.stop_job();
        if !scrub.strips.needs_building(&key) {
            return;
        }
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(HOVER_WAIT).await;
            this.update_in(cx, |this, window, cx| {
                this.start_library_strip(key, window, cx);
            })
            .ok();
        });
        self.library.scrub.wait = Some(task);
    }

    fn start_library_strip(&mut self, key: StripKey, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.read(cx).phase != crate::main_window::Phase::Idle {
            return;
        }
        let page = &mut self.library;
        let Some(entry) = page
            .scrub
            .hover
            .as_ref()
            .and_then(|hover| page.entry(&hover.path))
            .filter(|entry| StripKey::of(entry) == key)
            .cloned()
        else {
            return;
        };
        let hold = Hold::on(&entry.path);
        let stop = hold.stopper();
        let done = key.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            let built = cx
                .background_executor()
                .spawn(async move {
                    match player::segments_of(&entry) {
                        Ok(segments) => strip::build(&segments, &hold),
                        Err(_) => Built::Failed,
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.finish_library_strip(done, built, window, cx);
            })
            .ok();
        });
        page.scrub.job = Some(Job {
            key,
            stop,
            _task: task,
        });
    }

    fn finish_library_strip(
        &mut self,
        key: StripKey,
        built: Built,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scrub = &mut self.library.scrub;
        if scrub.job.as_ref().is_some_and(|job| job.key == key) {
            scrub.job = None;
        }
        match built {
            Built::Frames(frames) if self.library.entries.is_some() => {
                let retired = self.library.scrub.strips.insert(key, frames);
                drop_frames(retired, window);
                cx.notify();
            }
            Built::Frames(frames) => drop_frames(frames, window),
            Built::Failed => self.library.scrub.strips.fail(key),
            Built::Stopped => {}
        }
    }

    pub(super) fn leave_library_tile(&mut self, path: &Path, cx: &mut Context<Self>) {
        let scrub = &self.library.scrub;
        if scrub.hover.as_ref().is_some_and(|hover| hover.path == path) {
            self.clear_library_scrub(cx);
        }
    }

    pub(in crate::main_window) fn clear_library_scrub(&mut self, cx: &mut Context<Self>) {
        let scrub = &mut self.library.scrub;
        scrub.stop_job();
        if scrub.hover.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn drop_library_strips(&mut self, window: &mut Window) {
        let scrub = &mut self.library.scrub;
        scrub.stop_job();
        scrub.hover = None;
        drop_frames(scrub.strips.clear(), window);
    }

    pub(super) fn library_scrub_of(
        &self,
        entry: &Entry,
    ) -> Option<(Option<Arc<RenderImage>>, f32)> {
        let scrub = &self.library.scrub;
        let hover = scrub
            .hover
            .as_ref()
            .filter(|hover| hover.path == entry.path)?;
        let frame = scrub.strips.get(&StripKey::of(entry)).and_then(|frames| {
            frames
                .get(strip::frame_at(hover.line, frames.len()))
                .cloned()
        });
        Some((frame, hover.line))
    }
}

pub(super) async fn sweep_library_tile(
    this: WeakEntity<MainWindow>,
    path: PathBuf,
    seconds: f64,
    cx: &mut AsyncWindowContext,
) {
    let poll = Duration::from_millis(20);
    let tile = loop {
        let tile = this.update(cx, |this, _| {
            this.library.drawn.borrow().get(&path).copied()
        });
        match tile {
            Ok(Some(tile)) => break tile,
            Ok(None) => cx.background_executor().timer(poll).await,
            Err(_) => return,
        }
    };
    let left = f32::from(tile.origin.x) + TILE_PADDING + TILE_BORDER;
    let width = f32::from(tile.size.width) - (TILE_PADDING + TILE_BORDER) * 2.;
    let hovered = Instant::now();
    let ready = loop {
        let ready = this.update_in(cx, |this, window, cx| {
            this.scrub_library_tile(&path, gpui::px(left), window, cx);
            let page = &this.library;
            let entry = page.entry(&path)?;
            page.scrub.strips.get(&StripKey::of(entry)).map(|_| ())
        });
        match ready {
            Ok(Some(())) => break hovered.elapsed(),
            Ok(None) if hovered.elapsed().as_secs() < 10 => {
                cx.background_executor().timer(poll).await;
            }
            _ => return,
        }
    };
    let started = Instant::now();
    let (mut frames, mut longest, mut last) = (0u32, Duration::ZERO, started);
    while started.elapsed().as_secs_f64() < seconds {
        let across = (frames % strip::LINE_STEPS as u32) as f32 / strip::LINE_STEPS;
        let (next, drawn) = flume::bounded(1);
        let stepped = this.update_in(cx, |this, window, cx| {
            this.scrub_library_tile(&path, gpui::px(left + width * across), window, cx);
            window.on_next_frame(move |_, _| {
                next.send(()).ok();
            });
            cx.notify();
        });
        if stepped.is_err() || drawn.recv_async().await.is_err() {
            return;
        }
        let now = Instant::now();
        longest = longest.max(now.saturating_duration_since(last));
        last = now;
        frames += 1;
    }
    tracing::info!(
        ready_ms = ready.as_secs_f64() * 1000.,
        frames,
        seconds = started.elapsed().as_secs_f64(),
        longest_gap_ms = longest.as_secs_f64() * 1000.,
        "library scrub sweep"
    );
}

fn drop_frames(frames: Vec<Arc<RenderImage>>, window: &mut Window) {
    for frame in frames {
        if let Err(error) = window.drop_image(frame) {
            tracing::debug!("Could not drop a scrub frame: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{point, px, size};

    use super::*;

    #[test]
    fn the_fraction_runs_across_the_thumbnail_not_the_tile() {
        let tile = Bounds {
            origin: point(px(100.), px(0.)),
            size: size(px(214.), px(160.)),
        };
        assert_eq!(thumb_fraction(px(107.), tile), 0.);
        assert_eq!(thumb_fraction(px(207.), tile), 0.5);
        assert_eq!(thumb_fraction(px(307.), tile), 1.);
    }
}
