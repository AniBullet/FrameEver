//! The app-wide recording session.
//!
//! Unit 1 kept the recording lifecycle on `MainWindow`; the controls bar makes
//! that untenable -- two windows now read and drive the same state. The session
//! is a plain gpui entity installed as a global, observed by both windows; all
//! engine work runs on tokio via `gpui_tokio` and lands back here with
//! `cx.notify()`.

use std::{
    sync::{Arc, Once},
    time::{Duration, Instant},
};

use cap_utils::disk_space::{DiskSpaceStatus, RecordingStorageMonitor};
use gpui::{App, AppContext as _, Context, Entity, Global, Task};

use crate::app_sounds::AppSound;
use crate::i18n::t;
use crate::recording::{self, ActiveRecording, StartConfig};

/// `Idle -> Starting -> Recording -> Stopping -> Idle`; pause is a flag on
/// `Recording`, matching the `recording`/`paused` variants of the Tauri bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Starting,
    Recording { paused: bool },
    Stopping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StudioEditorPresentation<I = gpui::WindowId> {
    Pending,
    Attempted,
    Opened(I),
    Dismissed,
}

impl<I: PartialEq> StudioEditorPresentation<I> {
    pub(crate) fn suppresses_completion_open(&self) -> bool {
        matches!(self, Self::Opened(_) | Self::Dismissed)
    }

    pub(crate) fn closed(&mut self, window: I) {
        if matches!(self, Self::Opened(current) if *current == window) {
            *self = Self::Dismissed;
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreparingStudioEditor {
    generation: u64,
    pub(crate) project_path: std::path::PathBuf,
    pub(crate) finalization: recording::StudioFinalization,
    pub(crate) capture_stopped: bool,
    pub(crate) presentation: StudioEditorPresentation,
}

fn prepares_studio_editor(
    mode: Option<recording::RecordingMode>,
    low_storage: bool,
    recording_failed: bool,
    editor_target: bool,
    open_editor: bool,
) -> bool {
    mode == Some(recording::RecordingMode::Studio)
        && !low_storage
        && !recording_failed
        && !editor_target
        && open_editor
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountdownAdvance {
    Ignore,
    Tick(u32),
    PlaySound,
    Start,
}

fn countdown_advance(
    phase: Phase,
    current_generation: u64,
    countdown_generation: u64,
    remaining: Option<u32>,
) -> CountdownAdvance {
    if phase != Phase::Starting || current_generation != countdown_generation {
        return CountdownAdvance::Ignore;
    }
    match remaining {
        Some(remaining) if remaining > 1 => CountdownAdvance::Tick(remaining - 1),
        Some(1) => CountdownAdvance::PlaySound,
        Some(0) => CountdownAdvance::Start,
        _ => CountdownAdvance::Ignore,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PauseControlTicket {
    generation: u64,
    sequence: u64,
    paused: bool,
}

struct PendingPauseControl {
    ticket: PauseControlTicket,
    abort: futures_util::future::AbortHandle,
}

#[derive(Default)]
struct PauseControlState {
    pending: Option<PendingPauseControl>,
    sequence: u64,
    uncertain: bool,
}

impl PauseControlState {
    fn begin(
        &mut self,
        generation: u64,
        paused: bool,
    ) -> Option<(PauseControlTicket, futures_util::future::AbortRegistration)> {
        if self.pending.is_some() || self.uncertain {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let ticket = PauseControlTicket {
            generation,
            sequence: self.sequence,
            paused,
        };
        let (abort, registration) = futures_util::future::AbortHandle::new_pair();
        self.pending = Some(PendingPauseControl {
            ticket: ticket.clone(),
            abort,
        });
        Some((ticket, registration))
    }

    fn invalidate(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.abort.abort();
        }
    }

    fn cancel_for_stop(&mut self) {
        self.uncertain |= self.pending.is_some();
        self.invalidate();
    }

    fn complete(
        &mut self,
        ticket: &PauseControlTicket,
        generation: u64,
        phase: Phase,
        paused: Option<bool>,
    ) -> bool {
        if generation != ticket.generation
            || !matches!(phase, Phase::Recording { .. })
            || self
                .pending
                .as_ref()
                .is_none_or(|pending| pending.ticket != *ticket)
        {
            return false;
        }
        self.pending = None;
        self.uncertain = paused.is_none();
        true
    }
}

struct PauseControlOutcome {
    paused: Option<bool>,
    error: Option<String>,
}

async fn run_pause_control(
    expected_paused: bool,
    control: impl Future<Output = anyhow::Result<()>>,
    query_paused: impl Future<Output = anyhow::Result<bool>>,
) -> PauseControlOutcome {
    let result = control.await;
    let state = tokio::time::timeout(Duration::from_secs(2), query_paused).await;
    let (paused, state_error) = match state {
        Ok(Ok(true)) => (Some(true), None),
        Ok(Ok(false)) if result.is_ok() => (Some(false), None),
        Ok(Ok(false)) => (None, None),
        Ok(Err(error)) => (
            None,
            Some(t!(
                "Could not confirm capture state: {error}",
                error = format!("{error:#}")
            )),
        ),
        Err(_) => (None, Some(t!("Timed out confirming capture state").into())),
    };
    let error = result
        .err()
        .map(|error| format!("{error:#}"))
        .or(state_error)
        .or_else(|| {
            (paused != Some(expected_paused))
                .then(|| t!("Capture did not acknowledge the requested pause state").into())
        });
    PauseControlOutcome { paused, error }
}

pub struct RecordingSession {
    pub phase: Phase,
    active: Option<ActiveRecording>,
    /// Why the last start attempt failed, for the main window to surface.
    pub error: Option<String>,
    pub storage_warning: bool,
    pub storage_notice: Option<String>,
    stopped_for_low_storage: bool,
    stopped_elapsed: Option<Duration>,
    /// The config of the live (or last) recording, kept for restart.
    last_config: Option<StartConfig>,
    /// True while the controls bar window is open, so the main window knows to
    /// fall back to its in-window overlay when the bar failed to open.
    pub controls_open: bool,
    /// Mirror of the recording-scoped mic mute flag (the engine zeroes the
    /// payloads; the flag itself lives on the live recording's mic lock and
    /// resets with every new session).
    pub mic_muted: bool,
    /// The project dir of the studio recording that just stopped cleanly.
    /// Taken by the phase observer to honour `postStudioRecordingBehaviour`
    /// (`openEditor` by default) the way the Tauri app does.
    pub finished_studio: Option<std::path::PathBuf>,
    preparing_studio_editor: Option<PreparingStudioEditor>,
    /// `EditorRecordingTarget` (`src-tauri/src/windows.rs:3679-3697`): the
    /// open editor project a "Record a new clip" capture must land back in.
    /// Set by the editor's record modal (`setEditorRecordingTarget`,
    /// `ClipsSidebar.tsx:444`), cleared when its picker is cancelled, and
    /// *taken* -- never merely read -- by the phase observer when the session
    /// comes back to rest, exactly the way `apply_post_studio_editor_behaviour`
    /// and the stop-cleanup fallback both `take()` it
    /// (`src-tauri/src/recording.rs:3231-3287`) so a stale target can never
    /// leak into the next recording.
    editor_recording_target: Option<std::path::PathBuf>,
    started_at: Option<Instant>,
    paused_accum: Duration,
    paused_since: Option<Instant>,
    storage_monitor: Option<Task<()>>,
    failure_monitor: Option<Task<()>>,
    recording_generation: u64,
    countdown_remaining: Option<u32>,
    countdown_cue_abort: Option<futures_util::future::AbortHandle>,
    start_gate: Option<cap_recording::RecordingStartGate>,
    start_cue: Option<crate::app_sounds::StartCue>,
    /// A capture pipeline that finished warming up while the countdown or the
    /// start cue was still running; activated the moment the cue ends.
    primed: Option<ActiveRecording>,
    /// Teardown of a cancelled primed start still releasing its devices; the
    /// next start waits for it before locking the mic and camera again.
    discarding: Option<Task<Result<(), tokio::task::JoinError>>>,
    pause_control: PauseControlState,
    pause_control_error: Option<String>,
    pipeline_failed: bool,
}

struct SessionGlobal(Entity<RecordingSession>);
impl Global for SessionGlobal {}

impl RecordingSession {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let session = cx.new(|_| Self {
            phase: Phase::Idle,
            active: None,
            error: None,
            storage_warning: false,
            storage_notice: None,
            stopped_for_low_storage: false,
            stopped_elapsed: None,
            last_config: None,
            controls_open: false,
            mic_muted: false,
            finished_studio: None,
            preparing_studio_editor: None,
            editor_recording_target: None,
            started_at: None,
            paused_accum: Duration::ZERO,
            paused_since: None,
            storage_monitor: None,
            failure_monitor: None,
            recording_generation: 0,
            countdown_remaining: None,
            countdown_cue_abort: None,
            start_gate: None,
            start_cue: None,
            primed: None,
            discarding: None,
            pause_control: PauseControlState::default(),
            pause_control_error: None,
            pipeline_failed: false,
        });
        cx.set_global(SessionGlobal(session.clone()));
        session
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<SessionGlobal>().0.clone()
    }

    /// True while a recording is in flight. Tolerates the global not being
    /// installed yet (reporting idle) so callers can gate on it from windows
    /// that may open before the session exists.
    pub fn recording_in_flight(cx: &App) -> bool {
        cx.has_global::<SessionGlobal>()
            && cx.global::<SessionGlobal>().0.read(cx).phase != Phase::Idle
    }

    pub(crate) fn work_in_flight(cx: &mut App) -> bool {
        Self::recording_in_flight(cx)
            || crate::app_windows::export_in_flight(cx)
            || crate::import::imports_in_flight(cx)
            || crate::transcription::work_in_flight()
    }

    /// Elapsed recording time, excluding paused stretches -- what the bar's
    /// timer shows.
    pub fn elapsed(&self) -> Duration {
        if let Some(elapsed) = self.stopped_elapsed {
            return elapsed;
        }
        let Some(started_at) = self.started_at else {
            return Duration::ZERO;
        };
        let paused = self.paused_accum
            + self
                .paused_since
                .map(|since| since.elapsed())
                .unwrap_or_default();
        started_at.elapsed().saturating_sub(paused)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.phase, Phase::Recording { paused: true })
    }

    pub fn pause_pending(&self) -> bool {
        self.pause_control.pending.is_some()
    }

    pub fn pause_uncertain(&self) -> bool {
        self.pause_control.uncertain
    }

    pub fn pause_unavailable(&self) -> bool {
        self.pause_pending() || self.pause_uncertain() || self.pipeline_failed
    }

    /// The live (or last) recording's mode -- the bar exposes mic mute for
    /// instant recordings only (studio records the mic as an editable track,
    /// where muted spans would silently bake zeros in).
    pub fn mode(&self) -> Option<crate::recording::RecordingMode> {
        self.last_config.as_ref().map(|config| config.mode)
    }

    /// Whether the live recording actually has a microphone attached.
    pub fn has_microphone(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.mic_mute.is_some())
    }

    /// Flip the recording-scoped mic mute. No-op without a live mic.
    pub fn toggle_mic_mute(&mut self, cx: &mut Context<Self>) {
        let Some(mute) = self
            .active
            .as_ref()
            .and_then(|active| active.mic_mute.clone())
        else {
            return;
        };
        self.mic_muted = !self.mic_muted;
        mute.store(self.mic_muted, std::sync::atomic::Ordering::Relaxed);
        cx.notify();
    }

    /// `set_editor_recording_target` (`src-tauri/src/lib.rs:3166-3172`):
    /// arm (or disarm) the editor project the next studio recording appends
    /// into. No phase guard here -- the guard lives at the call sites, the way
    /// the Tauri command is a bare state write.
    pub fn set_editor_recording_target(&mut self, target: Option<std::path::PathBuf>) {
        self.editor_recording_target = target;
    }

    /// `EditorRecordingTarget::current`.
    pub fn editor_recording_target(&self) -> Option<std::path::PathBuf> {
        self.editor_recording_target.clone()
    }

    /// `EditorRecordingTarget::take` -- the consuming read both finish paths
    /// use, so the target clears no matter how the recording ended.
    pub fn take_editor_recording_target(&mut self) -> Option<std::path::PathBuf> {
        self.editor_recording_target.take()
    }

    pub fn set_controls_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.controls_open != open {
            self.controls_open = open;
            cx.notify();
        }
    }

    pub fn countdown_remaining(&self) -> Option<u32> {
        self.countdown_remaining
    }

    pub(crate) fn preparing_studio_editor(&self) -> Option<&PreparingStudioEditor> {
        self.preparing_studio_editor
            .as_ref()
            .filter(|pending| pending.generation == self.recording_generation)
    }

    pub(crate) fn preparing_studio_editor_mut(&mut self) -> Option<&mut PreparingStudioEditor> {
        self.preparing_studio_editor
            .as_mut()
            .filter(|pending| pending.generation == self.recording_generation)
    }

    pub(crate) fn take_preparing_studio_editor(&mut self) -> Option<PreparingStudioEditor> {
        self.preparing_studio_editor.take()
    }

    pub(crate) fn capture_stopped_for_editor(&self) -> bool {
        self.phase == Phase::Stopping
            && self
                .preparing_studio_editor()
                .is_some_and(|pending| pending.capture_stopped)
    }

    pub fn start(&mut self, config: StartConfig, cx: &mut Context<Self>) {
        if self.phase != Phase::Idle {
            return;
        }
        if !crate::menus::recording_start_allowed(cx) {
            cx.notify();
            return;
        }
        self.storage_monitor = None;
        self.failure_monitor = None;
        self.recording_generation = self.recording_generation.wrapping_add(1);
        self.preparing_studio_editor = None;
        let generation = self.recording_generation;
        let countdown = crate::store::GeneralSettings::load()
            .recording_countdown
            .unwrap_or(0);
        let start_gate = cap_recording::RecordingStartGate::new();
        let config = StartConfig {
            start_gate: Some(start_gate.clone()),
            ..config
        };
        self.start_gate = Some(start_gate);
        self.start_cue = Some(crate::app_sounds::prime_recording_start_sound());
        self.discard_primed(cx);
        self.pause_control.invalidate();
        self.pause_control.uncertain = false;
        self.pause_control_error = None;
        self.pipeline_failed = false;
        self.phase = Phase::Starting;
        self.error = None;
        self.storage_warning = false;
        self.storage_notice = None;
        self.stopped_for_low_storage = false;
        self.stopped_elapsed = None;
        self.last_config = Some(config.clone());
        // `Some(0)` keeps the controls in their pre-start state while the cue plays.
        self.countdown_remaining = Some(countdown);
        cx.notify();

        // The pipeline warms up during the final countdown second so capture is
        // live when the cue ends; without a countdown it warms up under the cue.
        if countdown <= 1 {
            self.prime_engine(config.clone(), generation, cx);
        }
        let initial_cue = (countdown == 0).then(|| self.play_start_cue(cx));

        cx.spawn(async move |this, cx| {
            let mut pending_cue = initial_cue;
            loop {
                if let Some(cue) = pending_cue.take() {
                    cue.await;
                } else {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                }
                let keep_waiting = this
                    .update(cx, |this, cx| {
                        match countdown_advance(
                            this.phase,
                            this.recording_generation,
                            generation,
                            this.countdown_remaining,
                        ) {
                            CountdownAdvance::Ignore => false,
                            CountdownAdvance::Tick(remaining) => {
                                this.countdown_remaining = Some(remaining);
                                if remaining == 1 {
                                    this.prime_engine(config.clone(), generation, cx);
                                }
                                cx.notify();
                                true
                            }
                            CountdownAdvance::PlaySound => {
                                this.countdown_remaining = Some(0);
                                pending_cue = Some(this.play_start_cue(cx));
                                cx.notify();
                                true
                            }
                            CountdownAdvance::Start => {
                                this.countdown_remaining = None;
                                this.countdown_cue_abort = None;
                                this.activate_primed(cx);
                                cx.notify();
                                false
                            }
                        }
                    })
                    .unwrap_or(false);
                if !keep_waiting {
                    return;
                }
            }
        })
        .detach();
    }

    /// Plays the start cue; the returned future completes once the recording's
    /// start gate is armed, whether the cue played, failed or was aborted.
    fn play_start_cue(
        &mut self,
        cx: &mut Context<Self>,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()>>> {
        let gate = self.start_gate.clone().unwrap_or_default();
        let cue = self.start_cue.take();
        let (abort, registration) = futures_util::future::AbortHandle::new_pair();
        self.countdown_cue_abort = Some(abort);
        let task = gpui_tokio::Tokio::spawn(
            cx,
            futures_util::future::Abortable::new(
                async move {
                    match cue {
                        Some(cue) => crate::app_sounds::play_recording_start_sound(cue, gate).await,
                        None => {
                            gate.arm();
                        }
                    }
                },
                registration,
            ),
        );
        Box::pin(async move {
            let _ = task.await;
        })
    }

    fn abort_start_cue(&mut self) {
        if let Some(abort) = self.countdown_cue_abort.take() {
            abort.abort();
        }
        self.start_cue = None;
    }

    fn prime_engine(&mut self, config: StartConfig, generation: u64, cx: &mut Context<Self>) {
        if self.phase != Phase::Starting || self.recording_generation != generation {
            return;
        }
        if !crate::menus::recording_start_allowed(cx) {
            self.finish(cx);
            return;
        }

        let start = recording::start(config);
        let discarding = self.discarding.take();
        let task = gpui_tokio::Tokio::spawn(cx, async move {
            if let Some(discarding) = discarding
                && tokio::time::timeout(Duration::from_secs(5), discarding)
                    .await
                    .is_err()
            {
                tracing::warn!("previous recording start is still tearing down; starting anyway");
            }
            start.await
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.recording_generation != generation || this.phase != Phase::Starting {
                    if let Ok(Ok(active)) = result {
                        this.discard_stale_start(active, cx);
                    }
                    return;
                }
                match result {
                    Ok(Ok(active)) => {
                        if this.countdown_remaining.is_some() {
                            this.primed = Some(active);
                        } else {
                            this.activate(active, cx);
                        }
                    }
                    Ok(Err(error)) => {
                        tracing::error!("recording failed to start: {error:#}");
                        this.error = Some(format!("{error:#}"));
                        this.abort_start_cue();
                        this.countdown_remaining = None;
                        {
                            this.phase = Phase::Idle;
                        }
                    }
                    Err(join_error) => {
                        tracing::error!("recording start task died: {join_error}");
                        this.error = Some(t!("Recording task failed.").into());
                        this.abort_start_cue();
                        this.countdown_remaining = None;
                        {
                            this.phase = Phase::Idle;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn activate_primed(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = self.primed.take() {
            self.activate(active, cx);
        }
    }

    fn activate(&mut self, active: ActiveRecording, cx: &mut Context<Self>) {
        tracing::info!(dir = %active.project_dir.display(), "recording started");
        let project_dir = active.project_dir.clone();
        let done = active.done_fut();
        self.active = Some(active);
        self.phase = Phase::Recording { paused: false };
        // A fresh mic lock always starts unmuted.
        self.mic_muted = false;
        self.started_at = Some(Instant::now());
        self.paused_accum = Duration::ZERO;
        self.paused_since = None;
        self.start_cue = None;
        if self.mode() == Some(recording::RecordingMode::Studio) {
            crate::app_windows::prewarm_shared_wgpu_device(cx);
        }
        self.monitor_storage(project_dir, cx);
        self.monitor_recording_failure(done, cx);
        cx.notify();
    }

    fn discard_primed(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = self.primed.take() {
            self.discard_stale_start(active, cx);
        }
    }

    fn discard_stale_start(&mut self, active: ActiveRecording, cx: &mut Context<Self>) {
        let previous = self.discarding.take();
        self.discarding = Some(gpui_tokio::Tokio::spawn(cx, async move {
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            let discarded = active.cancel_and_delete().await;
            if let Err(error) = discarded {
                tracing::warn!(%error, "discarding a cancelled recording start failed");
            }
        }));
    }

    fn monitor_recording_failure(&mut self, done: cap_recording::DoneFut, cx: &mut Context<Self>) {
        let generation = self.recording_generation;
        let task = gpui_tokio::Tokio::spawn(cx, done);
        self.failure_monitor = Some(cx.spawn(async move |this, cx| {
            let error = match task.await {
                Ok(Ok(())) => t!("Recording ended unexpectedly.").to_string(),
                Ok(Err(error)) => error.to_string(),
                Err(error) => t!("Recording completion task failed: {error}", error = error),
            };
            this.update(cx, |this, cx| {
                if !recording_failure_is_current(this.phase, this.recording_generation, generation)
                    || this.active.is_none()
                {
                    return;
                }
                tracing::error!(%error, "recording pipeline failed");
                this.pipeline_failed = true;
                this.error = Some(error);
                this.stop(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    fn monitor_storage(&mut self, project_dir: std::path::PathBuf, cx: &mut Context<Self>) {
        self.storage_monitor = None;
        let task = cx.spawn(async move |this, cx| {
            let mut check_failed = false;
            let mut storage_monitor = RecordingStorageMonitor::default();
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let is_current = |session: &Self| {
                    matches!(session.phase, Phase::Recording { .. })
                        && session
                            .active
                            .as_ref()
                            .is_some_and(|active| active.project_dir == project_dir)
                };
                if !this.update(cx, |this, _| is_current(this)).unwrap_or(false) {
                    return;
                }
                let path = project_dir.clone();
                let (next_monitor, result) = cx
                    .background_executor()
                    .spawn(async move {
                        let result = storage_monitor.sample(&path);
                        (storage_monitor, result)
                    })
                    .await;
                storage_monitor = next_monitor;
                let storage = match result {
                    Ok(storage) => {
                        check_failed = false;
                        storage
                    }
                    Err(error) => {
                        if !check_failed {
                            tracing::warn!(%error, "Could not check recording storage");
                            check_failed = true;
                        }
                        continue;
                    }
                };
                if !this
                    .update(cx, |this, cx| {
                        if !is_current(this) {
                            return false;
                        }
                        match storage.status() {
                            DiskSpaceStatus::Exhausted => {
                                this.storage_warning = true;
                                this.stopped_for_low_storage = true;
                                this.storage_notice = Some(
                                    t!("Low storage. Stopping and saving your recording…").into(),
                                );
                                this.stop(cx);
                                false
                            }
                            status => {
                                let warning = status == DiskSpaceStatus::Low;
                                if warning != this.storage_warning {
                                    this.storage_warning = warning;
                                    cx.notify();
                                }
                                true
                            }
                        }
                    })
                    .unwrap_or(false)
                {
                    return;
                }
            }
        });
        self.storage_monitor = Some(task);
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if self.phase == Phase::Starting && self.countdown_remaining.is_some() {
            self.countdown_remaining = None;
            self.finish(cx);
            return;
        }
        if !matches!(self.phase, Phase::Recording { .. }) {
            return;
        }
        self.storage_monitor = None;
        #[cfg(not(windows))]
        {
            self.failure_monitor = None;
        }
        let Some(active) = self.active.as_ref() else {
            return;
        };
        let stop_generation = self.recording_generation;
        let phase_before_stop = self.phase;
        let low_storage = self.stopped_for_low_storage;
        let recording_failed = self.pipeline_failed;
        #[cfg(any(target_os = "macos", windows))]
        let original_failure = recording_failed.then(|| self.error.clone()).flatten();
        let studio_project_path = active.project_dir.clone();
        let camera_snapshot = self.last_config.as_ref().and_then(|config| {
            (config.mode == recording::RecordingMode::Studio && config.camera.is_some())
                .then(|| crate::app_windows::studio_camera_snapshot(&config.target, cx))
                .flatten()
        });
        let (studio_progress, studio_finalization) = if prepares_studio_editor(
            self.mode(),
            low_storage,
            recording_failed,
            self.editor_recording_target.is_some(),
            crate::store::GeneralSettings::load().post_studio_recording_behaviour
                == crate::store::PostStudioBehaviour::OpenEditor,
        ) {
            let (publisher, finalization) = recording::StudioFinalization::channel(
                studio_project_path.clone(),
                self.recording_generation,
            );
            publisher.set_camera_snapshot(camera_snapshot);
            (Some(publisher), Some(finalization))
        } else {
            (None, None)
        };
        #[cfg(windows)]
        let retained_stop = recording_failed
            .then(|| active.failed_stop_handle())
            .or_else(|| active.clean_studio_stop_handle(studio_progress, camera_snapshot));
        #[cfg(target_os = "macos")]
        let retained_stop = recording_failed
            .then(|| active.failed_stop_handle())
            .or_else(|| active.clean_studio_stop_handle(studio_progress, camera_snapshot));
        let retains_active = retained_stop.is_some();
        let stop_future: recording::CaptureStopFuture = match retained_stop {
            Some(future) => future,
            None => {
                let active = self.active.take().unwrap();
                Box::pin(async move { (true, active.stop(camera_snapshot).await) })
            }
        };
        self.stopped_elapsed = Some(self.elapsed());
        self.pause_control.cancel_for_stop();
        self.phase = Phase::Stopping;
        self.preparing_studio_editor =
            studio_finalization.map(|finalization| PreparingStudioEditor {
                generation: self.recording_generation,
                project_path: studio_project_path,
                finalization,
                capture_stopped: false,
                presentation: StudioEditorPresentation::Pending,
            });
        cx.notify();

        let stop_sound = Arc::new(Once::new());
        let task = gpui_tokio::Tokio::spawn(cx, stop_future);
        if let Some(pending) = self.preparing_studio_editor.clone() {
            let stop_sound = stop_sound.clone();
            let finalization = pending.finalization.clone();
            let captured =
                gpui_tokio::Tokio::spawn(cx, async move { finalization.wait_for_capture().await });
            cx.spawn(async move |this, cx| {
                if !matches!(captured.await, Ok(true)) {
                    return;
                }
                this.update(cx, |this, cx| {
                    if this.recording_generation != pending.generation || this.phase != Phase::Stopping { return; }
                    if let Some(current) = this.preparing_studio_editor_mut()
                        && current.finalization.same_job(&pending.finalization)
                    {
                        current.capture_stopped = true;
                        stop_sound.call_once(|| AppSound::StopRecording.play());
                        tracing::info!(path = %current.project_path.display(), "Studio capture stopped; editor preparation can begin");
                        cx.notify();
                    }
                }).ok();
            }).detach();
        }
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.recording_generation != stop_generation || this.phase != Phase::Stopping { return; }
                let capture_stopped = match &result {
                    Ok((stopped, _)) => *stopped,
                    Err(_) => !retains_active,
                };
                if !capture_stopped {
                    this.preparing_studio_editor = None;
                    let error = match result {
                        Ok((_, Err(error))) => format!("{error:#}"),
                        Err(error) => t!("Stop task failed: {error}", error = error),
                        Ok((_, Ok(_))) => t!("Capture stop was not acknowledged").into(),
                    };
                    #[cfg(any(target_os = "macos", windows))]
                    let error = match &original_failure {
                        Some(first) => t!("{first}; cleanup: {error}", first = first, error = error),
                        None => error,
                    };
                    tracing::error!(%error, "Capture cleanup is unconfirmed; retaining Stop control");
                    this.phase = phase_before_stop;
                    this.stopped_elapsed = None;
                    if matches!(this.phase, Phase::Recording { .. })
                        && let Some(active) = &this.active
                    {
                        this.monitor_storage(active.project_dir.clone(), cx);
                    }
                    #[cfg(windows)]
                    let message = if this.mode() == Some(recording::RecordingMode::Studio) {
                        t!("{error}. Studio shutdown remains unconfirmed; recording and files retained.", error = error)
                    } else {
                        t!("{error}. Capture cleanup is unconfirmed. Use Ctrl+Shift+F9 to retry Stop.", error = error)
                    };
                    #[cfg(target_os = "macos")]
                    let message = t!("{error}. Capture cleanup is unconfirmed. Use your recording Stop control to retry.", error = error);
                    this.error = Some(message);
                    cx.notify();
                    return;
                }
                let result = result.map(|(_, result)| result);
                #[cfg(any(target_os = "macos", windows))]
                let result = result.map(|result| result.map_err(|error| match &original_failure {
                    Some(first) => error.context(first.clone()),
                    None => error,
                }));
                if retains_active { this.active = None; }
                match result {
                    Ok(Ok(project_dir)) if recording_failed => {
                        tracing::warn!(dir = %project_dir.display(), "failed recording files preserved");
                    }
                    Ok(Ok(project_dir)) => {
                        // The completion affordance is Recents now: the main
                        // window comes back with this recording at the head of
                        // the carousel, thumbnail and all. Stop used to reveal
                        // the bundle in Finder as a stand-in; the Tauri app
                        // never does that, so it goes with the placeholder it
                        // was standing in for.
                        tracing::info!(dir = %project_dir.display(), "recording finished");
                        stop_sound.call_once(|| AppSound::StopRecording.play());
                        if low_storage {
                            this.storage_notice = Some(t!("Recording stopped because storage is low. Your recording was saved.").into());
                        }
                        if this.mode() == Some(crate::recording::RecordingMode::Studio) && !low_storage {
                            this.finished_studio = Some(project_dir);
                        }
                    }
                    Ok(Err(error)) => {
                        tracing::error!("recording failed to stop cleanly: {error:#}");
                        this.storage_notice = low_storage.then(|| t!("Recording stopped because storage is low. Your recording files were kept. Free up space, then recover the recording below.").into());
                        this.error = Some(format!("{error:#}"));
                    }
                    Err(join_error) => {
                        tracing::error!("recording stop task died: {join_error}");
                        this.storage_notice = low_storage.then(|| t!("Recording stopped because storage is low. Your recording files were kept. Free up space, then recover the recording below.").into());
                        this.error = Some(t!("Stop task failed.").into());
                    }
                }
                this.finish(cx);
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        let Phase::Recording { paused } = self.phase else {
            return;
        };
        let Some(active) = self.active.as_ref() else {
            return;
        };
        if self.pipeline_failed {
            return;
        }
        let Some((ticket, registration)) =
            self.pause_control.begin(self.recording_generation, !paused)
        else {
            return;
        };
        let control = if paused {
            active.resume_handle()
        } else {
            active.pause_handle()
        };
        let query_paused = active.is_paused_handle();
        cx.notify();
        let task = gpui_tokio::Tokio::spawn(
            cx,
            futures_util::future::Abortable::new(
                run_pause_control(ticket.paused, control, query_paused),
                registration,
            ),
        );
        cx.spawn(async move |this, cx| {
            let outcome = match task.await {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(_)) => return,
                Err(error) => PauseControlOutcome {
                    paused: None,
                    error: Some(t!("Capture control task failed: {error}", error = error)),
                },
            };
            this.update(cx, |this, cx| {
                if !this.pause_control.complete(
                    &ticket,
                    this.recording_generation,
                    this.phase,
                    outcome.paused,
                ) {
                    return;
                }
                if let Some(paused) = outcome.paused {
                    let was_paused = this.is_paused();
                    this.phase = Phase::Recording { paused };
                    if was_paused && !paused {
                        if let Some(since) = this.paused_since.take() {
                            this.paused_accum += since.elapsed();
                        }
                    } else if !was_paused && paused {
                        this.paused_since = Some(Instant::now());
                    }
                }
                if this.pause_control_error.is_some() && this.error == this.pause_control_error {
                    this.error = None;
                }
                this.pause_control_error = outcome.error.map(|error| {
                    t!(
                        "{error}. Use Stop if recording cannot continue safely.",
                        error = error
                    )
                });
                if let Some(error) = &this.pause_control_error {
                    tracing::error!(%error, "pause/resume failed");
                    this.error = Some(error.clone());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Cancel the live recording and delete its project directory (the bar's
    /// trash button).
    pub fn delete(&mut self, cx: &mut Context<Self>) {
        if self.pause_unavailable() {
            return;
        }
        if !matches!(self.phase, Phase::Recording { .. }) {
            return;
        }
        #[cfg(any(target_os = "macos", windows))]
        if self.mode() == Some(recording::RecordingMode::Studio) {
            self.discard_studio_after_stop(None, cx);
            return;
        }
        self.storage_monitor = None;
        let Some(active) = self.active.take() else {
            return;
        };
        self.phase = Phase::Stopping;
        cx.notify();

        let task = gpui_tokio::Tokio::spawn(cx, active.cancel_and_delete());
        cx.spawn(async move |this, cx| {
            if let Ok(Err(error)) = task.await {
                tracing::error!("delete recording: {error:#}");
            }
            this.update(cx, |this, cx| this.finish(cx)).ok();
        })
        .detach();
    }

    /// The bar's restart button: throw away the live recording and immediately
    /// start a new one with the same config.
    pub fn restart(&mut self, cx: &mut Context<Self>) {
        if !crate::menus::recording_start_allowed(cx) || self.pause_unavailable() {
            return;
        }
        if !matches!(self.phase, Phase::Recording { .. }) {
            return;
        }
        let Some(config) = self.last_config.clone() else {
            return;
        };
        #[cfg(any(target_os = "macos", windows))]
        if self.mode() == Some(recording::RecordingMode::Studio) {
            self.discard_studio_after_stop(Some(config), cx);
            return;
        }
        self.storage_monitor = None;
        let Some(active) = self.active.take() else {
            return;
        };
        #[cfg(not(windows))]
        {
            self.failure_monitor = None;
        }
        self.phase = Phase::Starting;
        self.started_at = None;
        cx.notify();

        let task = gpui_tokio::Tokio::spawn(cx, active.cancel_and_delete());
        cx.spawn(async move |this, cx| {
            if let Ok(Err(error)) = task.await {
                tracing::error!("restart: discarding old recording: {error:#}");
            }
            this.update(cx, |this, cx| {
                // Restart bypasses the Idle guard in `start` by construction:
                // we are in `Starting` and `start` would bail, so inline the
                // same transition.
                this.phase = Phase::Idle;
                this.start(config, cx);
            })
            .ok();
        })
        .detach();
    }

    #[cfg(any(target_os = "macos", windows))]
    fn discard_studio_after_stop(&mut self, restart: Option<StartConfig>, cx: &mut Context<Self>) {
        let Some(active) = self.active.as_ref() else {
            return;
        };
        let Some(future) = active.studio_delete_handle() else {
            return;
        };
        let directory = active.project_dir.clone();
        let generation = self.recording_generation;
        let phase = self.phase;
        self.storage_monitor = None;
        self.phase = Phase::Stopping;
        cx.notify();
        let task = gpui_tokio::Tokio::spawn(cx, future);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.recording_generation != generation
                    || this.phase != Phase::Stopping
                    || this
                        .active
                        .as_ref()
                        .is_none_or(|active| active.project_dir != directory)
                {
                    return;
                }
                match result {
                    Ok((true, Ok(_))) => {
                        this.active = None;
                        this.finish(cx);
                        if let Some(config) = restart {
                            this.start(config, cx);
                        }
                    }
                    Ok((true, Err(error))) => {
                        this.active = None;
                        this.finish(cx);
                        this.error = Some(t!(
                            "{error}; discard/restart stopped, local recording retained",
                            error = format!("{error:#}")
                        ));
                        cx.notify();
                    }
                    result => {
                        this.phase = phase;
                        if matches!(this.phase, Phase::Recording { .. }) {
                            this.monitor_storage(directory.clone(), cx);
                        }
                        this.error = Some(match result {
                            Ok((_, Err(error))) => t!(
                                "{error}; discard/restart stopped, local recording retained",
                                error = format!("{error:#}")
                            ),
                            Err(error) => t!(
                                "Studio discard task failed: {error}; cleanup is unconfirmed",
                                error = error
                            ),
                            _ => t!(
                                "Studio discard stop was not acknowledged; local recording retained"
                            )
                            .into(),
                        });
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        self.abort_start_cue();
        self.discard_primed(cx);
        self.start_gate = None;
        self.failure_monitor = None;
        self.pause_control.invalidate();
        self.pause_control.uncertain = false;
        self.pause_control_error = None;
        self.pipeline_failed = false;
        self.storage_monitor = None;
        self.countdown_remaining = None;
        self.phase = Phase::Idle;
        self.mic_muted = false;
        self.storage_warning = false;
        self.stopped_elapsed = None;
        self.started_at = None;
        self.paused_accum = Duration::ZERO;
        self.paused_since = None;
        cx.notify();
    }
}

fn recording_failure_is_current(phase: Phase, current: u64, completed: u64) -> bool {
    current == completed && matches!(phase, Phase::Recording { .. })
}

#[cfg(test)]
mod countdown_tests {
    use super::*;

    #[test]
    fn countdown_ticks_until_the_final_step_before_start() {
        assert_eq!(
            countdown_advance(Phase::Starting, 7, 7, Some(3)),
            CountdownAdvance::Tick(2)
        );
        assert_eq!(
            countdown_advance(Phase::Starting, 7, 7, Some(2)),
            CountdownAdvance::Tick(1)
        );
        assert_eq!(
            countdown_advance(Phase::Starting, 7, 7, Some(1)),
            CountdownAdvance::PlaySound
        );
        assert_eq!(
            countdown_advance(Phase::Starting, 7, 7, None),
            CountdownAdvance::Ignore
        );
    }

    #[test]
    fn cancelled_or_superseded_countdown_cannot_start_capture() {
        assert_eq!(
            countdown_advance(Phase::Idle, 7, 7, Some(1)),
            CountdownAdvance::Ignore
        );
        assert_eq!(
            countdown_advance(Phase::Starting, 8, 7, Some(1)),
            CountdownAdvance::Ignore
        );
        assert_eq!(
            countdown_advance(Phase::Recording { paused: false }, 7, 7, Some(1)),
            CountdownAdvance::Ignore
        );
    }

    #[test]
    fn completed_cue_can_only_start_its_own_pending_recording() {
        assert_eq!(
            countdown_advance(Phase::Starting, 7, 7, Some(0)),
            CountdownAdvance::Start
        );
        for (phase, current) in [(Phase::Idle, 7), (Phase::Starting, 8)] {
            assert_eq!(
                countdown_advance(phase, current, 7, Some(0)),
                CountdownAdvance::Ignore
            );
        }
    }

    #[test]
    fn sound_follows_every_countdown_step_and_precedes_capture() {
        for (remaining, expected) in [
            (3, CountdownAdvance::Tick(2)),
            (2, CountdownAdvance::Tick(1)),
            (1, CountdownAdvance::PlaySound),
            (0, CountdownAdvance::Start),
        ] {
            assert_eq!(
                countdown_advance(Phase::Starting, 7, 7, Some(remaining)),
                expected
            );
        }
    }
}

#[cfg(test)]
mod recording_failure_tests {
    use super::*;

    #[test]
    fn only_current_recording_failure_can_request_stop() {
        for paused in [false, true] {
            assert!(recording_failure_is_current(
                Phase::Recording { paused },
                4,
                4
            ));
            assert!(!recording_failure_is_current(
                Phase::Recording { paused },
                5,
                4
            ));
        }
    }

    #[test]
    fn late_failure_does_not_interrupt_start_stop_or_idle() {
        for phase in [Phase::Idle, Phase::Starting, Phase::Stopping] {
            assert!(!recording_failure_is_current(phase, 4, 4));
        }
    }
}

#[cfg(test)]
mod pause_control_tests {
    use super::*;

    #[test]
    fn rapid_toggle_does_not_dispatch_a_second_control() {
        let mut state = PauseControlState::default();
        let (ticket, _) = state.begin(1, true).unwrap();
        assert!(state.begin(1, false).is_none());
        assert!(state.complete(&ticket, 1, Phase::Recording { paused: false }, Some(true)));
        assert!(state.begin(1, false).is_some());
    }

    #[test]
    fn late_pause_acknowledgments_cannot_change_a_stopped_or_new_recording() {
        for phase in [Phase::Idle, Phase::Starting, Phase::Stopping] {
            let mut state = PauseControlState::default();
            let (ticket, _) = state.begin(1, true).unwrap();
            assert!(!state.complete(&ticket, 1, phase, Some(true)));
        }
        let mut state = PauseControlState::default();
        let (old, _) = state.begin(1, true).unwrap();
        state.invalidate();
        let (new, _) = state.begin(2, true).unwrap();
        assert!(!state.complete(&old, 2, Phase::Recording { paused: false }, Some(true)));
        assert!(state.complete(&new, 2, Phase::Recording { paused: false }, Some(true)));
    }

    #[test]
    fn a_stop_retry_does_not_accept_a_late_pause_or_resume_unconfirmed_capture() {
        let mut state = PauseControlState::default();
        let (old, _) = state.begin(1, true).unwrap();
        state.cancel_for_stop();
        assert!(state.uncertain);
        assert!(state.begin(1, false).is_none());
        assert!(!state.complete(&old, 1, Phase::Recording { paused: false }, Some(true)));
    }

    #[tokio::test]
    async fn pause_is_not_confirmed_before_the_actor_acknowledges_and_reports_it() {
        let (send, receive) = tokio::sync::oneshot::channel();
        let queried = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = queried.clone();
        let mut control = Box::pin(run_pause_control(
            true,
            async {
                receive.await?;
                Ok(())
            },
            async move {
                observed.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(true)
            },
        ));
        assert!(futures_util::poll!(&mut control).is_pending());
        assert!(!queried.load(std::sync::atomic::Ordering::SeqCst));
        send.send(()).unwrap();
        let outcome = control.await;
        assert_eq!(outcome.paused, Some(true));
        assert!(outcome.error.is_none());
    }

    #[tokio::test]
    async fn failed_pause_does_not_claim_capture_is_paused_or_allow_an_unsafe_retry() {
        let outcome = run_pause_control(
            true,
            async { anyhow::bail!("pipeline shutdown failed") },
            async { Ok(false) },
        )
        .await;
        assert_eq!(outcome.paused, None);
        assert!(outcome.error.unwrap().contains("pipeline shutdown failed"));
        let mut state = PauseControlState::default();
        let (ticket, _) = state.begin(1, true).unwrap();
        assert!(state.complete(
            &ticket,
            1,
            Phase::Recording { paused: false },
            outcome.paused
        ));
        assert!(state.uncertain);
        assert!(state.begin(1, true).is_none());
    }

    #[tokio::test]
    async fn failed_resume_preserves_a_confirmed_pause_and_reports_the_failure() {
        let outcome =
            run_pause_control(false, async { anyhow::bail!("input unavailable") }, async {
                Ok(true)
            })
            .await;
        assert_eq!(outcome.paused, Some(true));
        assert!(outcome.error.unwrap().contains("input unavailable"));
    }

    #[tokio::test]
    async fn stop_cancels_an_unstarted_pause_request() {
        let mut state = PauseControlState::default();
        let (_, registration) = state.begin(1, true).unwrap();
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = polled.clone();
        let control = futures_util::future::Abortable::new(
            async move { observed.store(true, std::sync::atomic::Ordering::SeqCst) },
            registration,
        );
        state.invalidate();
        assert!(control.await.is_err());
        assert!(!polled.load(std::sync::atomic::Ordering::SeqCst));
    }
}

#[cfg(test)]
mod studio_editor_presentation_tests {
    use super::*;

    #[test]
    fn early_editor_only_applies_to_normal_studio_stops_with_open_editor_preference() {
        for mode in [
            None,
            Some(recording::RecordingMode::Instant),
            Some(recording::RecordingMode::Studio),
        ] {
            for flags in 0..16 {
                let low_storage = flags & 1 != 0;
                let failed = flags & 2 != 0;
                let editor_target = flags & 4 != 0;
                let open_editor = flags & 8 != 0;
                let expected = mode == Some(recording::RecordingMode::Studio) && flags == 8;
                assert_eq!(
                    prepares_studio_editor(mode, low_storage, failed, editor_target, open_editor),
                    expected
                );
            }
        }
    }

    #[test]
    fn unsuccessful_or_raced_early_window_creation_keeps_normal_completion_open() {
        assert!(!StudioEditorPresentation::<u32>::Pending.suppresses_completion_open());
        assert!(!StudioEditorPresentation::<u32>::Attempted.suppresses_completion_open());
    }

    #[test]
    fn closing_a_preparing_editor_suppresses_completion_reopening() {
        let mut state = StudioEditorPresentation::Opened(1);
        assert!(state.suppresses_completion_open());
        state.closed(1);
        assert_eq!(state, StudioEditorPresentation::Dismissed);
        assert!(state.suppresses_completion_open());
    }

    #[test]
    fn explicit_reopening_retains_the_new_window_when_the_old_close_arrives() {
        let mut state = StudioEditorPresentation::Opened(1);
        state.closed(1);
        state = StudioEditorPresentation::Opened(2);
        state.closed(1);
        assert_eq!(state, StudioEditorPresentation::Opened(2));
        state.closed(2);
        assert_eq!(state, StudioEditorPresentation::Dismissed);
    }
}
