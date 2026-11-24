use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use frameever_automation::{
    Action, AutomationExportCompression, AutomationHost, AutomationRecordingMode, AutomationsStore,
    Capability, CaptureTargetKind, ClipboardSource, Condition, ExportDestination, ExportFormat,
    ExportProfile, PrecedingAction, Trigger, TriggerContext, apply_filename_template,
    notification_text,
};
use frameever_recording::sources::screen_capture::ScreenCaptureTarget;
use gpui::App;

const EDITOR_LOAD_WAIT: Duration = Duration::from_secs(60);
pub(crate) const EDITOR_LOADING: &str = "The editor is still opening this recording";

enum UiRequest {
    CopyImage(PathBuf),
    Reveal(PathBuf),
    Open(PathBuf),
    OpenEditor(PathBuf),
    OpenScreenshotEditor(PathBuf),
    DeleteScreenshot(PathBuf),
    DeleteRecording(PathBuf),
    ApplyPreset {
        project: PathBuf,
        config: serde_json::Value,
    },
}

struct UiMessage {
    request: UiRequest,
    reply: flume::Sender<Result<(), String>>,
}

enum UiOutcome {
    Done(Result<(), String>),
    Pending(gpui::Task<Result<(), String>>),
}

struct Runtime {
    handle: tokio::runtime::Handle,
    ui: flume::Sender<UiMessage>,
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn init(cx: &mut App) {
    let (ui, requests) = flume::unbounded::<UiMessage>();
    let runtime = Runtime {
        handle: gpui_tokio::Tokio::handle(cx),
        ui,
    };
    if RUNTIME.set(runtime).is_err() {
        return;
    }
    cx.spawn(async move |cx| {
        while let Ok(message) = requests.recv_async().await {
            let outcome = cx.update(|cx| handle_ui_request(message.request, cx));
            let result = match outcome {
                UiOutcome::Done(result) => result,
                UiOutcome::Pending(task) => task.await,
            };
            if message.reply.send(result).is_err() {
                tracing::debug!("an automation action stopped waiting for its answer");
            }
        }
    })
    .detach();
}

fn handle_ui_request(request: UiRequest, cx: &mut App) -> UiOutcome {
    match request {
        UiRequest::CopyImage(path) => {
            UiOutcome::Pending(crate::platform::read_image_into_clipboard(path, cx))
        }
        UiRequest::Reveal(path) => {
            crate::library::reveal_path(&path, cx);
            UiOutcome::Done(Ok(()))
        }
        UiRequest::Open(path) => {
            crate::library::open_with_system(&path, cx);
            UiOutcome::Done(Ok(()))
        }
        UiRequest::OpenEditor(path) => {
            crate::app_windows::open_editor(path, cx);
            UiOutcome::Done(Ok(()))
        }
        UiRequest::OpenScreenshotEditor(path) => {
            crate::app_windows::open_screenshot_editor(path, cx);
            UiOutcome::Done(Ok(()))
        }
        UiRequest::DeleteScreenshot(bundle) => UiOutcome::Done(
            crate::app_windows::delete_screenshot_for_automation(&bundle, cx),
        ),
        UiRequest::DeleteRecording(project) => UiOutcome::Pending(
            crate::app_windows::delete_recording_for_automation(&project, cx),
        ),
        UiRequest::ApplyPreset { project, config } => UiOutcome::Done(
            crate::app_windows::apply_preset_for_automation(&project, &config, cx),
        ),
    }
}

async fn ui(request: UiRequest) -> Result<(), String> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| "FrameEver is not ready to run this action".to_string())?;
    let (reply, response) = flume::bounded(1);
    runtime
        .ui
        .send(UiMessage { request, reply })
        .map_err(|_| "FrameEver is closing".to_string())?;
    response
        .recv_async()
        .await
        .map_err(|_| "FrameEver is closing".to_string())?
}

#[cfg(not(test))]
fn load_store() -> Option<AutomationsStore> {
    parse_store(crate::store::automations_value()?)
}

fn parse_store(value: serde_json::Value) -> Option<AutomationsStore> {
    match serde_json::from_value(value) {
        Ok(store) => Some(store),
        Err(error) => {
            tracing::error!("deserializing the automations store: {error}");
            None
        }
    }
}

#[cfg(test)]
fn load_store() -> Option<AutomationsStore> {
    None
}

fn store_for(trigger: Trigger) -> Option<AutomationsStore> {
    load_store().filter(|store| has_enabled_rule(store, trigger))
}

fn has_enabled_rule(store: &AutomationsStore, trigger: Trigger) -> bool {
    store
        .rules
        .iter()
        .any(|rule| rule.enabled && rule.trigger == trigger)
}

struct RunningEvents {
    events: Mutex<HashSet<(Trigger, String)>>,
}

impl RunningEvents {
    fn new() -> Self {
        Self {
            events: Mutex::new(HashSet::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<(Trigger, String)>> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn claim(&'static self, trigger: Trigger, key: String) -> Option<RunningEvent> {
        let claimed = self.lock().insert((trigger, key.clone()));
        claimed.then(|| RunningEvent {
            events: self,
            trigger,
            key,
        })
    }
}

struct RunningEvent {
    events: &'static RunningEvents,
    trigger: Trigger,
    key: String,
}

impl Drop for RunningEvent {
    fn drop(&mut self) {
        self.events
            .lock()
            .remove(&(self.trigger, std::mem::take(&mut self.key)));
    }
}

fn running_events() -> &'static RunningEvents {
    static RUNNING: OnceLock<RunningEvents> = OnceLock::new();
    RUNNING.get_or_init(RunningEvents::new)
}

fn event_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn spawn_run(
    trigger: Trigger,
    key: String,
    build: impl FnOnce(&AutomationsStore) -> TriggerContext + Send + 'static,
) -> Option<tokio::task::JoinHandle<()>> {
    let store = store_for(trigger)?;
    let Some(running) = running_events().claim(trigger, key) else {
        tracing::warn!(?trigger, "automations are already running for this event");
        return None;
    };
    let Some(runtime) = RUNTIME.get() else {
        tracing::warn!(?trigger, "automations are not ready; skipping");
        return None;
    };
    Some(runtime.handle.spawn(async move {
        let _running = running;
        let store = Arc::new(store);
        let context_store = store.clone();
        let ctx = match tokio::task::spawn_blocking(move || build(&context_store)).await {
            Ok(ctx) => ctx,
            Err(error) => {
                tracing::error!(?trigger, %error, "building the automation context failed");
                return;
            }
        };
        run_trigger(&store, trigger, ctx).await;
    }))
}

async fn run_trigger(store: &AutomationsStore, trigger: Trigger, ctx: TriggerContext) {
    let host = GpuiAutomationHost::for_trigger(trigger);
    let results = frameever_automation::run(&host, store, &trigger, &ctx).await;
    for result in &results {
        for action in &result.action_results {
            match &action.error {
                Some(error) => tracing::warn!(
                    rule_id = %result.rule_id,
                    action = action_name(&action.action),
                    %error,
                    "automation action did not complete"
                ),
                None => tracing::info!(
                    rule_id = %result.rule_id,
                    action = action_name(&action.action),
                    "automation action completed"
                ),
            }
        }
    }
}

fn action_name(action: &Action) -> &'static str {
    match action {
        Action::CopyToClipboard { .. } => "copyToClipboard",
        Action::SaveToLocation { .. } => "saveToLocation",
        Action::Export { .. } => "export",
        Action::RevealInFileManager => "revealInFileManager",
        Action::OpenFile => "openFile",
        Action::RunCommand { .. } => "runCommand",
        Action::RecognizeTextToClipboard => "recognizeTextToClipboard",
        Action::Notify { .. } => "notify",
        Action::OpenEditor => "openEditor",
        Action::SkipEditor => "skipEditor",
        Action::ApplyPreset { .. } => "applyPreset",
        Action::DeleteLocalFiles => "deleteLocalFiles",
    }
}

pub fn recording_started(project_dir: &Path) {
    spawn_run(Trigger::RecordingStarted, event_key(project_dir), |_| {
        TriggerContext::new()
    });
}

pub fn studio_recording_finished(project_dir: PathBuf, elapsed: Duration) {
    spawn_run(
        Trigger::StudioRecordingFinished,
        event_key(&project_dir),
        move |_| studio_context(&project_dir, elapsed),
    );
}

fn studio_context(project_dir: &Path, elapsed: Duration) -> TriggerContext {
    let ctx = TriggerContext::new()
        .with_project_path(project_dir.to_path_buf())
        .with_recording_mode(AutomationRecordingMode::Studio);
    if elapsed.is_zero() {
        ctx
    } else {
        ctx.with_duration(elapsed.as_secs_f64())
    }
}

fn instant_context(project_dir: &Path) -> TriggerContext {
    TriggerContext::new()
        .with_project_path(project_dir.to_path_buf())
        .with_recording_mode(AutomationRecordingMode::Instant)
}

pub fn instant_behaviour(
    project_dir: &Path,
    default: crate::store::PostInstantBehaviour,
) -> crate::store::PostInstantBehaviour {
    match store_for(Trigger::InstantRecordingFinished) {
        Some(store) => instant_behaviour_for(&store, project_dir, default),
        None => default,
    }
}

fn instant_behaviour_for(
    store: &AutomationsStore,
    project_dir: &Path,
    default: crate::store::PostInstantBehaviour,
) -> crate::store::PostInstantBehaviour {
    let matched = frameever_automation::evaluate(
        store,
        &Trigger::InstantRecordingFinished,
        &instant_context(project_dir),
    );
    if matched.iter().any(|(_, actions)| can_delete(actions)) {
        crate::store::PostInstantBehaviour::DoNothing
    } else {
        default
    }
}

pub fn instant_recording_finished(project_dir: PathBuf, hold: Option<InstantRulesHold>) {
    let run = spawn_run(
        Trigger::InstantRecordingFinished,
        event_key(&project_dir),
        move |_| instant_context(&project_dir),
    );
    if let (Some(run), Some(runtime)) = (run, RUNTIME.get()) {
        runtime.handle.spawn(async move {
            let _hold = hold;
            if let Err(error) = run.await {
                tracing::warn!(%error, "the Instant recording's automations did not finish");
            }
        });
    }
}

struct InstantRuleHolds {
    held: Mutex<HashMap<String, usize>>,
    released: tokio::sync::Notify,
}

impl InstantRuleHolds {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, usize>> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn is_held(&self, key: &str) -> bool {
        self.lock().contains_key(key)
    }
}

fn instant_rule_holds() -> &'static InstantRuleHolds {
    static HOLDS: OnceLock<InstantRuleHolds> = OnceLock::new();
    HOLDS.get_or_init(|| InstantRuleHolds {
        held: Mutex::new(HashMap::new()),
        released: tokio::sync::Notify::new(),
    })
}

pub struct InstantRulesHold {
    key: String,
}

fn hold_key(project_dir: &Path) -> String {
    let folder = match (project_dir.parent(), project_dir.file_name()) {
        (Some(parent), Some(name)) => parent
            .canonicalize()
            .map(|parent| parent.join(name))
            .unwrap_or_else(|_| project_dir.to_path_buf()),
        _ => project_dir.to_path_buf(),
    };
    event_key(&folder)
}

impl InstantRulesHold {
    fn take(project_dir: &Path) -> Self {
        let key = hold_key(project_dir);
        *instant_rule_holds().lock().entry(key.clone()).or_default() += 1;
        Self { key }
    }
}

impl Drop for InstantRulesHold {
    fn drop(&mut self) {
        let holds = instant_rule_holds();
        {
            let mut held = holds.lock();
            if let Some(count) = held.get_mut(&self.key) {
                *count -= 1;
                if *count == 0 {
                    held.remove(&self.key);
                }
            }
        }
        holds.released.notify_waiters();
    }
}

pub fn hold_instant_rules(project_dir: &Path) -> Option<InstantRulesHold> {
    store_for(Trigger::InstantRecordingFinished)?;
    Some(InstantRulesHold::take(project_dir))
}

pub async fn instant_rules_finished(project_dir: &Path) {
    let key = hold_key(project_dir);
    let holds = instant_rule_holds();
    loop {
        let mut released = std::pin::pin!(holds.released.notified());
        released.as_mut().enable();
        if !holds.is_held(&key) {
            return;
        }
        released.await;
    }
}

pub fn video_imported(project_dir: PathBuf) {
    spawn_run(Trigger::VideoImported, event_key(&project_dir), move |_| {
        TriggerContext::new().with_project_path(project_dir)
    });
}

pub fn recording_deleted(project_dir: PathBuf) {
    spawn_run(
        Trigger::RecordingDeleted,
        event_key(&project_dir),
        move |_| TriggerContext::new().with_project_path(project_dir),
    );
}

#[derive(Debug, Clone)]
pub struct ScreenshotTarget {
    kind: Option<CaptureTargetKind>,
    title: Option<String>,
}

fn capture_target_kind(target: &ScreenCaptureTarget) -> Option<CaptureTargetKind> {
    match target {
        ScreenCaptureTarget::Window { .. } => Some(CaptureTargetKind::Window),
        ScreenCaptureTarget::Display { .. } => Some(CaptureTargetKind::Display),
        ScreenCaptureTarget::Area { .. } => Some(CaptureTargetKind::Area),
        ScreenCaptureTarget::CameraOnly | ScreenCaptureTarget::AudioOnly => None,
    }
}

pub fn screenshot_target(target: &ScreenCaptureTarget) -> Option<ScreenshotTarget> {
    store_for(Trigger::ScreenshotTaken)?;
    Some(ScreenshotTarget {
        kind: capture_target_kind(target),
        title: target.title(),
    })
}

fn screenshot_context(image_path: PathBuf, target: &ScreenshotTarget) -> TriggerContext {
    let mut ctx = TriggerContext::new();
    if let Some(project) = image_path.parent() {
        ctx = ctx.with_project_path(project.to_path_buf());
    }
    ctx = ctx.with_image_path(image_path);
    if let Some(kind) = target.kind {
        ctx = ctx.with_capture_target(kind);
    }
    if let Some(title) = target.title.clone() {
        ctx = ctx.with_window_title(title);
    }
    ctx
}

#[derive(Default)]
pub struct ScreenshotRules {
    pub editor_choice: Option<bool>,
    pub deletes: bool,
}

pub fn screenshot_rules(target: Option<&ScreenshotTarget>, image_path: &Path) -> ScreenshotRules {
    let none = ScreenshotRules {
        editor_choice: None,
        deletes: false,
    };
    let (Some(target), Some(store)) = (target, store_for(Trigger::ScreenshotTaken)) else {
        return none;
    };
    let ctx = screenshot_context(image_path.to_path_buf(), target);
    ScreenshotRules {
        editor_choice: editor_choice(&store, Trigger::ScreenshotTaken, &ctx),
        deletes: rules_delete(&store, Trigger::ScreenshotTaken, &ctx),
    }
}

fn rules_delete(store: &AutomationsStore, trigger: Trigger, ctx: &TriggerContext) -> bool {
    frameever_automation::evaluate(store, &trigger, ctx)
        .iter()
        .any(|(_, actions)| can_delete(actions))
}

fn editor_choice(store: &AutomationsStore, trigger: Trigger, ctx: &TriggerContext) -> Option<bool> {
    let matched = frameever_automation::evaluate(store, &trigger, ctx);
    let any = |wanted: fn(&[Action]) -> bool| matched.iter().any(|(_, actions)| wanted(actions));
    if any(|actions| {
        actions
            .iter()
            .any(|action| matches!(action, Action::SkipEditor))
            || can_delete(actions)
    }) {
        Some(false)
    } else if any(|actions| {
        actions
            .iter()
            .any(|action| matches!(action, Action::OpenEditor))
    }) {
        Some(true)
    } else {
        None
    }
}

fn can_delete(actions: &[Action]) -> bool {
    actions.windows(2).any(|pair| {
        matches!(pair[1], Action::DeleteLocalFiles)
            && matches!(
                pair[0],
                Action::Export {
                    destination: ExportDestination::CustomPath { .. },
                    ..
                } | Action::SaveToLocation { .. }
            )
    })
}

pub fn screenshot_taken(image_path: PathBuf, target: ScreenshotTarget) {
    spawn_run(
        Trigger::ScreenshotTaken,
        event_key(&image_path),
        move |_| screenshot_context(image_path, &target),
    );
}

pub fn studio_editor_behaviour(
    project_dir: &Path,
    elapsed: Duration,
    default: crate::store::PostStudioBehaviour,
) -> crate::store::PostStudioBehaviour {
    match store_for(Trigger::StudioRecordingFinished) {
        Some(store) => studio_editor_behaviour_for(&store, project_dir, elapsed, default),
        None => default,
    }
}

fn studio_editor_behaviour_for(
    store: &AutomationsStore,
    project_dir: &Path,
    elapsed: Duration,
    default: crate::store::PostStudioBehaviour,
) -> crate::store::PostStudioBehaviour {
    let ctx = studio_context(project_dir, elapsed);
    match editor_choice(store, Trigger::StudioRecordingFinished, &ctx) {
        Some(false) => crate::store::PostStudioBehaviour::DoNothing,
        Some(true) => crate::store::PostStudioBehaviour::OpenEditor,
        None => default,
    }
}

#[derive(Default)]
struct GpuiAutomationHost {
    trigger: Option<Trigger>,
    last_output: Mutex<Option<Output>>,
}

#[derive(Debug, Clone, PartialEq)]
struct Output {
    path: PathBuf,
    written: Option<FileFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    inode: (u64, u64),
    #[cfg(windows)]
    file_id: (u32, u64),
}

fn file_fingerprint(path: &Path) -> std::io::Result<FileFingerprint> {
    let metadata = std::fs::symlink_metadata(path)?;
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(FileFingerprint {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        #[cfg(unix)]
        inode: (metadata.dev(), metadata.ino()),
        #[cfg(windows)]
        file_id: windows_file_id(path)?,
    })
}

#[cfg(windows)]
fn windows_file_id(path: &Path) -> std::io::Result<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let file = std::fs::File::open(path)?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((
        info.dwVolumeSerialNumber,
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}

impl GpuiAutomationHost {
    fn for_trigger(trigger: Trigger) -> Self {
        Self {
            trigger: Some(trigger),
            last_output: Mutex::new(None),
        }
    }

    fn set_last_output(&self, output: Option<Output>) {
        *self
            .last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = output;
    }

    fn remember_output(&self, path: PathBuf) {
        self.set_last_output(Some(Output {
            path,
            written: None,
        }));
    }

    fn editor_already_decided(&self) -> bool {
        matches!(
            self.trigger,
            Some(Trigger::StudioRecordingFinished | Trigger::ScreenshotTaken)
        )
    }

    fn output(&self) -> Option<Output> {
        self.last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    #[cfg(test)]
    fn last_output(&self) -> Option<PathBuf> {
        self.output().map(|output| output.path)
    }
}

impl AutomationHost for GpuiAutomationHost {
    fn capabilities(&self) -> &[Capability] {
        &[
            Capability::CopyToClipboard,
            Capability::SaveToLocation,
            Capability::Export,
            Capability::RevealInFileManager,
            Capability::OpenFile,
            Capability::RunCommand,
            Capability::Notify,
            Capability::OpenEditor,
            Capability::ApplyPreset,
            Capability::DeleteLocalFiles,
        ]
    }

    async fn copy_to_clipboard(
        &self,
        ctx: &TriggerContext,
        source: &ClipboardSource,
    ) -> Result<(), String> {
        ui(UiRequest::CopyImage(clipboard_image(ctx, *source)?)).await
    }

    async fn save_to_location(
        &self,
        ctx: &TriggerContext,
        dir: &str,
        filename_template: Option<&str>,
    ) -> Result<(), String> {
        self.set_last_output(None);
        let saved = save_to_location(ctx, dir, filename_template).await?;
        self.set_last_output(Some(saved));
        Ok(())
    }

    async fn export(
        &self,
        ctx: &TriggerContext,
        profile: &ExportProfile,
        destination: &ExportDestination,
    ) -> Result<(), String> {
        self.set_last_output(None);
        let project_path = existing_project(ctx, "No project path available for export")?;
        let output = render(project_path, profile, destination).await?;
        self.remember_output(output);
        Ok(())
    }

    async fn reveal_in_file_manager(&self, ctx: &TriggerContext) -> Result<(), String> {
        let path = ctx
            .image_path
            .as_ref()
            .or(ctx.output_path.as_ref())
            .or(ctx.project_path.as_ref())
            .ok_or("No path available to reveal")?;
        ui(UiRequest::Reveal(path.clone())).await
    }

    async fn open_file(&self, ctx: &TriggerContext) -> Result<(), String> {
        let path = ctx
            .image_path
            .as_ref()
            .or(ctx.output_path.as_ref())
            .ok_or("No file path available to open")?;
        ui(UiRequest::Open(path.clone())).await
    }

    async fn run_command(
        &self,
        ctx: &TriggerContext,
        program: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &HashMap<String, String>,
        use_shell: bool,
    ) -> Result<(), String> {
        tracing::info!(program, "automation: running command");
        frameever_automation::run_command(ctx, program, args, cwd, env, use_shell).await
    }

    async fn recognize_text_to_clipboard(&self, _ctx: &TriggerContext) -> Result<(), String> {
        Err("Recognizing text is not available in FrameEver yet".to_string())
    }

    async fn notify(
        &self,
        ctx: &TriggerContext,
        title_template: &str,
        body_template: &str,
    ) -> Result<(), String> {
        if !crate::notifications::enabled() {
            tracing::info!("automation: notifications are turned off in settings");
            return Ok(());
        }
        let (title, body) = notification_text(ctx, title_template, body_template);
        tokio::task::spawn_blocking(move || crate::notifications::show(&title, &body))
            .await
            .map_err(|error| format!("Failed to send notification: {error}"))?
    }

    async fn open_editor(&self, ctx: &TriggerContext) -> Result<(), String> {
        if self.editor_already_decided() {
            return Ok(());
        }
        ui(editor_request(ctx)?).await
    }

    async fn apply_preset(&self, ctx: &TriggerContext, name: &str) -> Result<(), String> {
        let project_path = existing_project(ctx, "No project path for preset")?;
        let config = preset_config(name)?;
        tracing::info!(preset = name, project = %project_path.display(), "automation: applying preset");
        let deadline = std::time::Instant::now() + EDITOR_LOAD_WAIT;
        loop {
            let request = UiRequest::ApplyPreset {
                project: project_path.clone(),
                config: config.clone(),
            };
            match ui(request).await {
                Err(error) if error == EDITOR_LOADING && std::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                result => return result,
            }
        }
    }

    async fn delete_local_files(&self, ctx: &TriggerContext) -> Result<(), String> {
        let project = ctx
            .project_path
            .clone()
            .ok_or("No project path for deletion")?;
        if ctx.image_path.is_some() {
            tracing::info!(path = %project.display(), "automation: deleting screenshot");
            return ui(UiRequest::DeleteScreenshot(project)).await;
        }
        tracing::info!(path = %project.display(), "automation: deleting local recording");
        ui(UiRequest::DeleteRecording(project)).await
    }

    async fn verify_before_delete(
        &self,
        ctx: &TriggerContext,
        preceding: Option<PrecedingAction<'_>>,
    ) -> Result<(), String> {
        let project = ctx
            .project_path
            .as_ref()
            .ok_or("No project path for deletion")?;
        let source = ctx.image_path.as_ref().or(ctx.output_path.as_ref());
        let output = self.output();
        verify_before_delete(
            project,
            preceding,
            output.as_ref().map(|output| output.path.as_path()),
            source.map(PathBuf::as_path),
            output.as_ref().and_then(|output| output.written.as_ref()),
        )
    }
}

fn editor_request(ctx: &TriggerContext) -> Result<UiRequest, String> {
    if let Some(image_path) = ctx.image_path.as_ref() {
        return Ok(UiRequest::OpenScreenshotEditor(image_path.clone()));
    }
    let path = existing_project(ctx, "No project path for editor")?;
    Ok(UiRequest::OpenEditor(path.clone()))
}

fn existing_project<'a>(ctx: &'a TriggerContext, missing: &str) -> Result<&'a PathBuf, String> {
    let project = ctx
        .project_path
        .as_ref()
        .ok_or_else(|| missing.to_string())?;
    if !project.is_dir() {
        return Err("The recording no longer exists".to_string());
    }
    Ok(project)
}

fn preset_config(name: &str) -> Result<serde_json::Value, String> {
    crate::presets::PresetsStore::load()
        .presets
        .into_iter()
        .find(|preset| preset.name == name)
        .map(|preset| preset.config)
        .ok_or_else(|| format!("Preset '{name}' not found"))
}

pub(crate) fn write_preset(project: &Path, config: &serde_json::Value) -> Result<(), String> {
    let current = match frameever_project::ProjectConfiguration::load(project) {
        Ok(current) => current,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(error) => return Err(format!("Failed to load project config: {error}")),
    };
    crate::presets::apply_preset(config, &current)
        .ok_or_else(|| "The preset could not be read".to_string())?
        .write(project)
        .map_err(|error| format!("Failed to write project config: {error}"))
}

async fn render(
    project_path: &Path,
    profile: &ExportProfile,
    destination: &ExportDestination,
) -> Result<PathBuf, String> {
    tracing::info!(project = %project_path.display(), format = ?profile.format, "automation: exporting");
    frameever_export::validate_output_path(project_path)?;
    let mut builder = frameever_export::ExporterBase::builder(project_path.to_path_buf());
    if let ExportDestination::CustomPath { dir } = destination {
        let folder = chosen_folder(dir)?;
        frameever_export::validate_output_path(&folder)?;
        tokio::fs::create_dir_all(&folder)
            .await
            .map_err(|error| format!("Failed to create directory: {error}"))?;
    }
    let reserved = match export_output_path(project_path, profile.format, destination) {
        Some(wanted) => Some(reserve_unused_path(&wanted).await?),
        None => None,
    };
    if let Some(output) = reserved.clone() {
        builder = builder.with_output_path(output);
    }
    let reservation = ReservedOutput(reserved);
    let base = builder.build().await.map_err(|error| format!("{error}"))?;
    let output = match profile.format {
        ExportFormat::Mp4 => {
            frameever_export::mp4::Mp4ExportSettings {
                codec: Default::default(),
                fps: profile.fps,
                resolution_base: profile.resolution_base,
                compression: export_compression(profile.compression),
                custom_bpp: None,
                force_ffmpeg_decoder: false,
                optimize_filesize: false,
            }
            .export(base, |_| true)
            .await
        }
        ExportFormat::Gif => {
            frameever_export::gif::GifExportSettings {
                fps: profile.fps,
                resolution_base: profile.resolution_base,
                quality: None,
            }
            .export(base, |_| true)
            .await
        }
        ExportFormat::Mov => {
            frameever_export::mov::MovExportSettings {
                fps: profile.fps,
                resolution_base: profile.resolution_base,
                cursor_only: false,
            }
            .export(base, |_| true)
            .await
        }
    }
    .map_err(|error| format!("Export failed: {error}"))?;
    reservation.keep();
    tracing::info!(output = %output.display(), "automation: export complete");
    Ok(output)
}

struct ReservedOutput(Option<PathBuf>);

impl ReservedOutput {
    fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for ReservedOutput {
    fn drop(&mut self) {
        if let Some(path) = self.0.take()
            && let Err(error) = std::fs::remove_file(&path)
        {
            tracing::debug!(path = %path.display(), %error, "no reserved export name to remove");
        }
    }
}

fn verify_before_delete(
    project: &Path,
    preceding: Option<PrecedingAction<'_>>,
    last_output: Option<&Path>,
    source: Option<&Path>,
    written: Option<&FileFingerprint>,
) -> Result<(), String> {
    let preceding = preceding.ok_or(
        "Delete local files needs an Export or Save to location before it; local files kept",
    )?;
    if !matches!(
        preceding.action,
        Action::Export { .. } | Action::SaveToLocation { .. }
    ) {
        return Err(format!(
            "Delete local files only follows an Export or Save to location, not {}; local files kept",
            action_name(preceding.action)
        ));
    }
    if !preceding.succeeded {
        return Err(format!(
            "{} did not succeed; local files kept",
            action_name(preceding.action)
        ));
    }
    let output =
        last_output.ok_or("The previous action left no file to check; local files kept")?;
    verify_output(output, project)?;
    match preceding.action {
        Action::SaveToLocation { .. } => verify_copy(output, source, written),
        _ => verify_playable(output),
    }
}

fn unchecked(path: &Path, error: impl std::fmt::Display) -> String {
    format!(
        "{} could not be checked ({error}); local files kept",
        path.display()
    )
}

fn verify_copy(
    output: &Path,
    source: Option<&Path>,
    written: Option<&FileFingerprint>,
) -> Result<(), String> {
    let source = source.ok_or("The saved file has no source to compare; local files kept")?;
    let written = written.ok_or_else(|| {
        format!(
            "{} was not written by this rule; local files kept",
            output.display()
        )
    })?;
    let current = file_fingerprint(output).map_err(|error| unchecked(output, error))?;
    if &current != written {
        return Err(format!(
            "{} changed after this rule saved it; local files kept",
            output.display()
        ));
    }
    let expected = std::fs::metadata(source)
        .map_err(|error| unchecked(source, error))?
        .len();
    let saved = std::fs::metadata(output)
        .map_err(|error| unchecked(output, error))?
        .len();
    if saved != expected {
        return Err(format!(
            "{} is {saved} bytes but the original is {expected}; local files kept",
            output.display()
        ));
    }
    Ok(())
}

fn verify_playable(output: &Path) -> Result<(), String> {
    let unreadable = |reason: String| {
        format!(
            "{} is not a playable video ({reason}); local files kept",
            output.display()
        )
    };
    let input = ffmpeg::format::input(output).map_err(|error| unreadable(error.to_string()))?;
    if input.streams().best(ffmpeg::media::Type::Video).is_none() {
        return Err(unreadable("no video track".to_string()));
    }
    if input.duration() <= 0 {
        return Err(unreadable("no duration".to_string()));
    }
    Ok(())
}

fn verify_output(output: &Path, project: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(output).map_err(|error| unchecked(output, error))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(format!(
            "{} is missing or empty; local files kept",
            output.display()
        ));
    }
    let resolve = |path: &Path| {
        path.canonicalize().map_err(|error| {
            format!(
                "{} could not be resolved ({error}); local files kept",
                path.display()
            )
        })
    };
    let (output, project) = (resolve(output)?, resolve(project)?);
    if output.starts_with(&project) {
        return Err(format!(
            "{} is inside the folder that would be deleted; local files kept",
            output.display()
        ));
    }
    Ok(())
}

fn clipboard_image(ctx: &TriggerContext, source: ClipboardSource) -> Result<PathBuf, String> {
    match source {
        ClipboardSource::Raw => ctx
            .image_path
            .as_ref()
            .or(ctx.output_path.as_ref())
            .cloned()
            .ok_or_else(|| "No image or output path available for clipboard copy".to_string()),
        ClipboardSource::Rendered => {
            if let Some(path) = ctx.output_path.clone() {
                return Ok(path);
            }
            if ctx.image_path.is_some() && ctx.project_path.is_some() {
                return Err(
                    "Copying the edited screenshot is not available in FrameEver yet; choose the original capture"
                        .to_string(),
                );
            }
            ctx.image_path
                .clone()
                .ok_or_else(|| "No rendered output available for clipboard copy".to_string())
        }
    }
}

async fn save_to_location(
    ctx: &TriggerContext,
    dir: &str,
    filename_template: Option<&str>,
) -> Result<Output, String> {
    let source = ctx
        .image_path
        .as_ref()
        .or(ctx.output_path.as_ref())
        .ok_or("No file path available for save")?;
    let filename = match filename_template {
        Some(template) => apply_filename_template(template, ctx),
        None => source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "capture.png".to_string()),
    };
    let wanted = chosen_folder(dir)?.join(&filename);
    if let Some(parent) = wanted.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Failed to create directory: {error}"))?;
    }
    if wanted.exists()
        && is_same_file(source, &wanted)
            .map_err(|error| format!("Failed to compare source and destination: {error}"))?
    {
        tracing::info!(dst = %wanted.display(), "automation: the file is already there");
        return Ok(Output {
            path: wanted,
            written: None,
        });
    }
    let destination = reserve_unused_path(&wanted).await?;
    tracing::info!(src = %source.display(), dst = %destination.display(), "automation: saving to location");
    let copied = tokio::fs::copy(source, &destination)
        .await
        .map_err(|error| format!("Failed to copy file: {error}"))
        .and_then(|_| {
            file_fingerprint(&destination)
                .map_err(|error| format!("Failed to check the saved file: {error}"))
        });
    match copied {
        Ok(written) => Ok(Output {
            path: destination,
            written: Some(written),
        }),
        Err(error) => {
            if let Err(remove_error) = tokio::fs::remove_file(&destination).await {
                tracing::debug!(%remove_error, "the unfinished copy was already gone");
            }
            Err(error)
        }
    }
}

const MAX_NAME_ATTEMPTS: u32 = 10_000;

async fn reserve_unused_path(wanted: &Path) -> Result<PathBuf, String> {
    for attempt in 0..MAX_NAME_ATTEMPTS {
        let candidate = numbered_path(wanted, attempt);
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .await
        {
            Ok(_) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) if tokio::fs::symlink_metadata(&candidate).await.is_ok() => continue,
            Err(error) => {
                return Err(format!("Failed to create {}: {error}", candidate.display()));
            }
        }
    }
    Err(format!(
        "No free file name left next to {}",
        wanted.display()
    ))
}

fn numbered_path(wanted: &Path, attempt: u32) -> PathBuf {
    if attempt == 0 {
        return wanted.to_path_buf();
    }
    let stem = wanted
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = match wanted.extension() {
        Some(extension) => format!("{stem} ({attempt}).{}", extension.to_string_lossy()),
        None => format!("{stem} ({attempt})"),
    };
    wanted.with_file_name(name)
}

fn chosen_folder(dir: &str) -> Result<PathBuf, String> {
    let folder = PathBuf::from(dir.trim());
    if dir.trim().is_empty() || !folder.is_absolute() {
        return Err("Choose a folder for this action".to_string());
    }
    Ok(folder)
}

fn is_same_file(source: &Path, destination: &Path) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let source = source.metadata()?;
        let destination = destination.metadata()?;
        Ok(source.dev() == destination.dev() && source.ino() == destination.ino())
    }
    #[cfg(windows)]
    {
        Ok(windows_file_id(source)? == windows_file_id(destination)?)
    }
}

fn export_output_path(
    project_path: &Path,
    format: ExportFormat,
    destination: &ExportDestination,
) -> Option<PathBuf> {
    match destination {
        ExportDestination::ProjectFolder => None,
        ExportDestination::CustomPath { dir } => {
            let extension = match format {
                ExportFormat::Mp4 => "mp4",
                ExportFormat::Gif => "gif",
                ExportFormat::Mov => "mov",
            };
            let name = project_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| "export".to_string());
            Some(PathBuf::from(dir.trim()).join(format!("{name}.{extension}")))
        }
    }
}

fn export_compression(
    compression: Option<AutomationExportCompression>,
) -> frameever_export::mp4::ExportCompression {
    match compression {
        Some(AutomationExportCompression::Maximum) => {
            frameever_export::mp4::ExportCompression::Maximum
        }
        Some(AutomationExportCompression::Social) => {
            frameever_export::mp4::ExportCompression::Social
        }
        Some(AutomationExportCompression::Web) | None => {
            frameever_export::mp4::ExportCompression::Web
        }
        Some(AutomationExportCompression::Potato) => {
            frameever_export::mp4::ExportCompression::Potato
        }
    }
}

#[cfg(test)]
mod tests;
