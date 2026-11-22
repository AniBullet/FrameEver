use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use frameever_project::{CursorEvents, ProjectConfiguration, RecordingMeta, StudioRecordingMeta};
use tokio::sync::Notify;

use crate::{
    MediaActivity, SegmentMedia,
    hover_analysis::{self, Schedule},
};

const EDITOR_IDLE: Duration = Duration::from_secs(1);

#[derive(Clone, Default)]
pub struct HoverChanges(Arc<Mutex<Arc<[f64]>>>);

impl HoverChanges {
    pub(crate) fn saved(display: &std::path::Path) -> Self {
        let saved = hover_analysis::cached_changes(display).unwrap_or_default();
        Self(Arc::new(Mutex::new(saved.into())))
    }

    pub fn times_ms(&self) -> Arc<[f64]> {
        Arc::clone(&self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn replace(&self, times_ms: Arc<[f64]>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = times_ms;
    }
}

struct HoverJob {
    display: PathBuf,
    cursor: Arc<CursorEvents>,
    cursor_offset_ms: f64,
    changes: HoverChanges,
}

fn hover_jobs(meta: &RecordingMeta, segments: &[SegmentMedia]) -> Vec<HoverJob> {
    let Some(studio) = meta.studio_meta() else {
        return Vec::new();
    };
    let displays: Vec<PathBuf> = match studio {
        StudioRecordingMeta::SingleSegment { segment } => vec![meta.path(&segment.display.path)],
        StudioRecordingMeta::MultipleSegments { inner } => inner
            .segments
            .iter()
            .map(|segment| meta.path(&segment.display.path))
            .collect(),
    };
    displays
        .into_iter()
        .zip(segments)
        .enumerate()
        .filter(|(_, (_, media))| !media.cursor.moves.is_empty())
        .map(|(index, (display, media))| HoverJob {
            display,
            cursor: media.cursor.clone(),
            cursor_offset_ms: frameever_rendering::display_cursor_offset_secs(studio, index)
                * 1000.0,
            changes: media.hover_changes.clone(),
        })
        .collect()
}

fn wants_hover_analysis(project: &ProjectConfiguration) -> bool {
    let wants = |cursor: &frameever_project::CursorConfiguration| {
        cursor.follows_hover_highlights() && !cursor.raw && !cursor.hide
    };
    wants(&project.cursor)
        || project.timeline.as_ref().is_some_and(|timeline| {
            timeline
                .style_segments
                .iter()
                .filter(|style| style.is_active_at(style.start))
                .filter_map(|style| style.overrides.cursor.as_ref())
                .any(wants)
        })
}

struct FollowState {
    wanted: AtomicBool,
    started: AtomicBool,
    stopped: AtomicBool,
    opened: Instant,
    last_preview_ms: AtomicU64,
    export_preview_active: Arc<AtomicBool>,
    found: Notify,
}

impl FollowState {
    fn schedule(&self) -> Schedule {
        if self.stopped.load(Ordering::Acquire) {
            return Schedule::Stop;
        }
        let last_preview = Duration::from_millis(self.last_preview_ms.load(Ordering::Acquire));
        let waits = !self.wanted.load(Ordering::Acquire)
            || MediaActivity::any_running()
            || self.export_preview_active.load(Ordering::Acquire)
            || self.opened.elapsed().saturating_sub(last_preview) < EDITOR_IDLE;
        if waits { Schedule::Wait } else { Schedule::Run }
    }

    fn analyse(&self, jobs: Vec<HoverJob>) {
        lower_thread_priority();
        for (index, job) in jobs.into_iter().enumerate() {
            if !job.display.is_file() {
                continue;
            }
            let started = Instant::now();
            let found = match hover_analysis::cached_changes(&job.display) {
                Some(changes) => Ok(Some(changes)),
                None => hover_analysis::analyze(
                    &job.display,
                    &job.cursor,
                    job.cursor_offset_ms,
                    &|| self.schedule(),
                ),
            };
            match found {
                Ok(Some(changes)) => {
                    if *job.changes.times_ms() == *changes {
                        continue;
                    }
                    tracing::info!(
                        segment = index,
                        changes = changes.len(),
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "Hover analysis finished"
                    );
                    job.changes.replace(changes.into());
                    self.found.notify_one();
                }
                Ok(None) => return,
                Err(error) => {
                    tracing::warn!(segment = index, %error, "Hover analysis failed");
                }
            }
        }
    }
}

pub(crate) struct HoverFollow {
    state: Arc<FollowState>,
}

impl HoverFollow {
    pub(crate) fn new(export_preview_active: Arc<AtomicBool>) -> Self {
        Self {
            state: Arc::new(FollowState {
                wanted: AtomicBool::new(false),
                started: AtomicBool::new(false),
                stopped: AtomicBool::new(false),
                opened: Instant::now(),
                last_preview_ms: AtomicU64::new(0),
                export_preview_active,
                found: Notify::new(),
            }),
        }
    }

    pub(crate) fn preview_requested(&self) {
        let since_open = self.state.opened.elapsed().as_millis() as u64;
        self.state
            .last_preview_ms
            .store(since_open, Ordering::Release);
    }

    pub(crate) fn project_changed(
        &self,
        project: &ProjectConfiguration,
        meta: &RecordingMeta,
        segments: &[SegmentMedia],
    ) {
        let wanted = wants_hover_analysis(project);
        self.state.wanted.store(wanted, Ordering::Release);
        if !wanted || self.state.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let jobs = hover_jobs(meta, segments);
        if jobs.is_empty() {
            return;
        }
        let state = Arc::clone(&self.state);
        if let Err(error) = std::thread::Builder::new()
            .name("cursor-hover-analysis".into())
            .spawn(move || state.analyse(jobs))
        {
            tracing::warn!(%error, "Could not start hover analysis");
        }
    }

    pub(crate) async fn found(&self) {
        self.state.found.notified().await;
    }

    pub(crate) fn stop(&self) {
        self.state.stopped.store(true, Ordering::Release);
    }
}

impl Drop for HoverFollow {
    fn drop(&mut self) {
        self.stop();
    }
}

fn lower_thread_priority() {
    #[cfg(target_os = "macos")]
    {
        const QOS_CLASS_BACKGROUND: u32 = 0x09;
        unsafe extern "C" {
            fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
        }
        let status = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_BACKGROUND, 0) };
        if status != 0 {
            tracing::warn!(status, "Hover analysis kept its normal priority");
        }
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
        };
        let lowered =
            unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
        if let Err(error) = lowered {
            tracing::warn!(%error, "Hover analysis kept its normal priority");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_project::ProjectConfiguration;

    #[test]
    fn analysis_is_wanted_by_the_base_cursor_or_an_active_cursor_style() {
        let mut project = ProjectConfiguration::default();
        assert!(wants_hover_analysis(&project));
        project.cursor.hover_anchors = false;
        assert!(!wants_hover_analysis(&project));
        project.cursor.hover_anchors = true;
        project.cursor.interaction_anchors = false;
        assert!(!wants_hover_analysis(&project));

        let mut raw = ProjectConfiguration::default();
        raw.cursor.raw = true;
        assert!(!wants_hover_analysis(&raw));
        let mut hidden = ProjectConfiguration::default();
        hidden.cursor.hide = true;
        assert!(!wants_hover_analysis(&hidden));

        let styled: ProjectConfiguration = serde_json::from_value(serde_json::json!({
            "cursor": { "interactionAnchors": true },
            "timeline": { "segments": [], "zoomSegments": [], "styleSegments": [{
                "start": 1.0, "end": 2.0,
                "overrides": { "cursor": { "interactionAnchors": true, "hoverAnchors": true } }
            }] }
        }))
        .unwrap();
        assert!(!styled.cursor.hover_anchors);
        assert!(wants_hover_analysis(&styled));

        let mut disabled = styled.clone();
        disabled.timeline.as_mut().unwrap().style_segments[0].enabled = false;
        assert!(!wants_hover_analysis(&disabled));
        let mut empty = styled;
        empty.timeline.as_mut().unwrap().style_segments[0].end = 1.0;
        assert!(!wants_hover_analysis(&empty));
    }
}
