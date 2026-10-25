//! The settings pages beyond General/Recordings/Screenshots -- Shortcuts,
//! CLI, Automations, Transcription, Integrations, License, Experimental,
//! Feedback and Changelog -- kept out of `settings_window.rs` so the shell
//! stays readable. Every renderer here is an `impl SettingsWindow` extension.
//!
//! Transcribed from the routes under
//! Tauri `src/routes/(window-chrome)/settings/`, with every store key
//! and HTTP endpoint spelled exactly the way the Tauri app spells it so the
//! two apps keep reading each other's state. Web calls run on the gpui_tokio
//! runtime (reqwest needs tokio); persistence goes through [`crate::store`].

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr as _,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use frameever_cli_install as cli_install;
use gpui::{
    AppContext as _, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    MouseButton, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, rgb, svg,
};
use serde_json::{Map, Value};

use crate::{
    diagnostics,
    i18n::{n, t, tr},
    settings_window::{MenuKind, Page, SettingsWindow},
    store::{
        self, Action, AutomationExportCompression, AutomationRecordingMode, AutomationRule,
        AutomationsStore, CaptureTargetKind, ClipboardSource, Condition, ExportDestination,
        ExportFormat, ExportProfile, GENERAL_SETTINGS, Hotkey, MatchMode, Trigger,
    },
    theme::Theme,
    ui,
};

/// `font-mono` in the TSX resolves to the platform mono stack; the closest
/// single face per OS, since no mono font is bundled.
const MONO_FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "monospace"
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Which text field an event came from, so one dispatcher serves every static
/// input these pages own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageField {
    Hint,
}

/// `saveState` in transcription.tsx.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HintSave {
    Idle,
    Saving,
    Saved,
}

/// The Diagnostic Report's phases. `Running` carries the CLI's own stage name
/// and leg so the label is built at render time, not at send time.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DiagnosticStatus {
    Idle,
    Running {
        stage: String,
        mode: Option<String>,
    },
    Done {
        verdict: Option<String>,
        summary: Option<String>,
        report_path: Option<PathBuf>,
        /// Set when the sync test could not run at all; the environment half
        /// of the report is still there.
        sync_test_error: Option<String>,
    },
    Error(String),
}

struct ListeningHotkey {
    action: &'static str,
    modifiers: gpui::Modifiers,
    error: Option<&'static str>,
}

struct SavedShortcut {
    action: &'static str,
    moved_from: Option<&'static str>,
}

/// The per-field text inputs for one expanded rule's editor. Rebuilt whenever
/// the rule's structure (conditions/actions lists, types) changes, because the
/// fields are positional.
struct RuleEditor {
    rule_id: String,
    fields: Vec<(AutoField, Entity<ui::TextInputState>)>,
    _subscriptions: Vec<gpui::Subscription>,
}

/// Which rule field a text input edits; the payload is the condition/action
/// index within the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoField {
    Name,
    ConditionSecs(usize),
    ConditionPattern(usize),
    ActionDir(usize),
    ActionFilename(usize),
    ActionProgram(usize),
    ActionArgs(usize),
    ActionNotifyTitle(usize),
    ActionNotifyBody(usize),
    ActionExportDir(usize),
}

/// Everything the pages here own. Lives on [`SettingsWindow`] as one field so
/// the shell's struct stays readable.
pub(crate) struct PagesState {
    // Shortcuts (hotkeys.tsx)
    hotkeys: Map<String, Value>,
    listening: Option<ListeningHotkey>,
    shortcut_saved: Option<SavedShortcut>,
    shortcut_saved_reset: Option<gpui::Task<()>>,

    // CLI (cli.tsx). `None` while the status fetch is in flight.
    cli_status: Option<Result<cli_install::CliInstallStatus, String>>,
    cli_installing: bool,
    cli_uninstalling: bool,
    /// Stand-in for cli.tsx's `toast.error` -- there is no toast layer here.
    cli_error: Option<String>,
    cli_copied: bool,
    cli_copied_reset: Option<gpui::Task<()>>,

    // Transcription (transcription.tsx)
    hints: Vec<String>,
    hint_input: Entity<ui::TextInputState>,
    hint_draft: String,
    hint_save: HintSave,
    hint_save_task: Option<gpui::Task<()>>,

    /// Best-effort local stand-in for `commands.getSystemDiagnostics`.
    os_version: Option<Option<String>>,

    // Diagnostic Report (feedback.tsx's diagnostic section)
    diagnostic: DiagnosticStatus,
    diagnostic_mode: diagnostics::SyncMode,
    diagnostic_mic: bool,
    /// Set for the length of a run; the Cancel button flips it and the runner's
    /// watchdog kills the self-test subprocess. It stays readable after Cancel
    /// so the run's later stages can still see it.
    diagnostic_cancel: Option<Arc<AtomicBool>>,
    /// Cancel has been pressed and the run has not resolved yet: the label
    /// stays on "Cancelling..." and no later stage walks it back.
    diagnostic_cancelling: bool,
    /// `None` until the Feedback page resolves it once; `Some(None)` means no
    /// `cap` sidecar was found and the run will be environment-only.
    selftest_binary: Option<Option<PathBuf>>,

    // Experimental (experimental.tsx). The Developer section's hand-back takes the
    // whole settings window over, so its clock and the ticker that repaints the
    // sequence live here and the overlay renders from the window root.
    switch_back: Option<SwitchBack>,
    switch_back_ticker: Option<gpui::Task<()>>,

    // Automations (automations.tsx)
    automations: AutomationsStore,
    automations_loaded: bool,
    automations_save_failed: bool,
    expanded_rule: Option<String>,
    rule_editor: Option<RuleEditor>,
    compatibility_checks: HashSet<String>,

    _subscriptions: Vec<gpui::Subscription>,
}

impl PagesState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<SettingsWindow>) -> Self {
        let hint_input = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_placeholder(t!("Add a term"));
            input
        });

        let mut subscriptions = vec![cx.subscribe(&hint_input, |this, input, event, cx| {
            this.page_field_event(PageField::Hint, input, event, cx)
        })];
        // Keys stop arriving once the window loses focus, so a recording row
        // would otherwise keep every global shortcut paused.
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.shortcuts_cancel(cx);
            }
        }));

        Self {
            hotkeys: store::hotkeys_raw(),
            listening: None,
            shortcut_saved: None,
            shortcut_saved_reset: None,
            cli_status: None,
            cli_installing: false,
            cli_uninstalling: false,
            cli_error: None,
            cli_copied: false,
            cli_copied_reset: None,
            hints: store::transcription_hints(),
            hint_input,
            hint_draft: String::new(),
            hint_save: HintSave::Idle,
            hint_save_task: None,
            os_version: None,
            diagnostic: DiagnosticStatus::Idle,
            diagnostic_mode: diagnostics::SyncMode::Both,
            diagnostic_mic: false,
            diagnostic_cancel: None,
            diagnostic_cancelling: false,
            selftest_binary: None,
            switch_back: None,
            switch_back_ticker: None,
            automations: AutomationsStore::default(),
            automations_loaded: false,
            automations_save_failed: false,
            expanded_rule: None,
            rule_editor: None,
            compatibility_checks: HashSet::new(),
            _subscriptions: subscriptions,
        }
    }
}

// ---------------------------------------------------------------------------
// Shell hooks
// ---------------------------------------------------------------------------

impl SettingsWindow {
    /// Per-page fetch/reset on navigation -- what each route's mount effects
    /// do. Called from [`SettingsWindow::page_shown`].
    pub(crate) fn pages_shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.page {
            Page::Shortcuts => {
                self.pages.hotkeys = store::hotkeys_raw();
                self.pages.shortcut_saved = None;
                self.pages.shortcut_saved_reset = None;
            }
            Page::Cli => self.cli_refresh(window, cx),
            Page::Transcription => {
                self.pages.hints = store::transcription_hints();
                self.pages.hint_draft.clear();
                self.pages.hint_save = HintSave::Idle;
                self.pages.hint_save_task = None;
                let input = self.pages.hint_input.clone();
                input.update(cx, |input, cx| input.set_text("", cx));
            }
            Page::Feedback => self.feedback_load_os_version(window, cx),
            Page::Automations => {
                self.pages.automations = store::automations();
                self.pages.automations_loaded = true;
                self.pages.automations_save_failed = false;
                self.pages.expanded_rule = None;
                self.pages.rule_editor = None;
                self.pages.compatibility_checks.clear();
            }
            _ => {}
        }
    }

    /// The static text fields' events, dispatched from the subscriptions in
    /// [`PagesState::new`].
    fn page_field_event(
        &mut self,
        field: PageField,
        input: Entity<ui::TextInputState>,
        event: &ui::TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ui::TextInputEvent::Changed => {
                let value = input.read(cx).text().to_string();
                match field {
                    PageField::Hint => self.pages.hint_draft = value,
                }
                cx.notify();
            }
            // `onKeyDown` in transcription.tsx: Enter adds the pending term.
            ui::TextInputEvent::Confirmed if field == PageField::Hint => {
                self.transcription_add_hint(cx);
            }
            _ => {}
        }
    }

    /// Option lists for the [`MenuKind`] variants owned by these pages.
    pub(crate) fn pages_menu_items(&self, kind: MenuKind) -> Vec<ui::MenuItem> {
        match kind {
            MenuKind::AutomationTrigger(rule) => {
                let current = self.rule_at(rule).map(|rule| rule.trigger);
                ALL_TRIGGERS
                    .iter()
                    .map(|trigger| {
                        ui::MenuItem::new(trigger_label(*trigger), Some(*trigger) == current)
                    })
                    .collect()
            }
            MenuKind::AutomationMatchMode(rule) => {
                let current = self.rule_at(rule).map(|rule| rule.match_mode);
                [
                    (MatchMode::All, t!("Match all")),
                    (MatchMode::Any, t!("Match any")),
                ]
                .iter()
                .map(|(mode, label)| ui::MenuItem::new(tr(label), Some(*mode) == current))
                .collect()
            }
            MenuKind::AutomationConditionType(rule, index) => {
                let current = self
                    .rule_at(rule)
                    .and_then(|rule| rule.conditions.get(index))
                    .map(condition_type_of);
                ALL_CONDITION_TYPES
                    .iter()
                    .map(|kind| ui::MenuItem::new(condition_label(*kind), Some(*kind) == current))
                    .collect()
            }
            MenuKind::AutomationConditionTarget(rule, index) => {
                let current = match self
                    .rule_at(rule)
                    .and_then(|rule| rule.conditions.get(index))
                {
                    Some(Condition::CaptureTargetIs { target }) => Some(*target),
                    _ => None,
                };
                CAPTURE_TARGETS
                    .iter()
                    .map(|(target, label)| ui::MenuItem::new(tr(label), Some(*target) == current))
                    .collect()
            }
            MenuKind::AutomationConditionMode(rule, index) => {
                let current = match self
                    .rule_at(rule)
                    .and_then(|rule| rule.conditions.get(index))
                {
                    Some(Condition::RecordingModeIs { mode }) => Some(*mode),
                    _ => None,
                };
                RECORDING_MODES
                    .iter()
                    .map(|(mode, label)| ui::MenuItem::new(tr(label), Some(*mode) == current))
                    .collect()
            }
            MenuKind::AutomationActionType(rule, index) => {
                let current = self
                    .rule_at(rule)
                    .and_then(|rule| rule.actions.get(index))
                    .map(action_type_of);
                ALL_ACTION_TYPES
                    .iter()
                    .map(|kind| ui::MenuItem::new(action_label(*kind), Some(*kind) == current))
                    .collect()
            }
            MenuKind::AutomationClipboardSource(rule, index) => {
                let current = match self.rule_at(rule).and_then(|rule| rule.actions.get(index)) {
                    Some(Action::CopyToClipboard { source }) => Some(*source),
                    _ => None,
                };
                CLIPBOARD_SOURCES
                    .iter()
                    .map(|(source, label)| ui::MenuItem::new(tr(label), Some(*source) == current))
                    .collect()
            }
            MenuKind::AutomationExportFormat(rule, index) => {
                let current = self
                    .export_profile_at(rule, index)
                    .map(|profile| profile.format);
                EXPORT_FORMATS
                    .iter()
                    .map(|(format, label)| ui::MenuItem::new(*label, Some(*format) == current))
                    .collect()
            }
            MenuKind::AutomationExportResolution(rule, index) => {
                let current = self
                    .export_profile_at(rule, index)
                    .map(resolution_value)
                    .unwrap_or("1080p");
                RESOLUTION_PRESETS
                    .iter()
                    .map(|(value, label, ..)| ui::MenuItem::new(*label, *value == current))
                    .collect()
            }
            MenuKind::AutomationExportFps(rule, index) => {
                let current = self
                    .export_profile_at(rule, index)
                    .map(|profile| profile.fps);
                FPS_PRESETS
                    .iter()
                    .map(|fps| ui::MenuItem::new(format!("{fps} FPS"), Some(*fps) == current))
                    .collect()
            }
            MenuKind::AutomationExportCompression(rule, index) => {
                let current = self
                    .export_profile_at(rule, index)
                    .and_then(|profile| profile.compression)
                    .unwrap_or(AutomationExportCompression::Web);
                COMPRESSIONS
                    .iter()
                    .map(|(compression, label)| {
                        ui::MenuItem::new(tr(label), *compression == current)
                    })
                    .collect()
            }
            MenuKind::AutomationPreset(rule, index) => {
                let current = match self.rule_at(rule).and_then(|rule| rule.actions.get(index)) {
                    Some(Action::ApplyPreset { name }) => Some(name.clone()),
                    _ => None,
                };
                store::preset_names()
                    .into_iter()
                    .map(|name| {
                        let checked = current.as_deref() == Some(name.as_str());
                        ui::MenuItem::new(name, checked)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Commit for the pages' menus -- the other half of `pages_menu_items`.
    pub(crate) fn pages_choose(
        &mut self,
        kind: MenuKind,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match kind {
            MenuKind::AutomationTrigger(rule) => {
                if let Some(trigger) = ALL_TRIGGERS.get(index).copied() {
                    // Rebuilt because the name field's placeholder is derived
                    // from the trigger.
                    self.automation_mutate(rule, true, window, cx, |rule| rule.trigger = trigger);
                }
            }
            MenuKind::AutomationMatchMode(rule) => {
                let mode = if index == 0 {
                    MatchMode::All
                } else {
                    MatchMode::Any
                };
                self.automation_mutate(rule, false, window, cx, |rule| rule.match_mode = mode);
            }
            MenuKind::AutomationConditionType(rule, condition) => {
                if let Some(kind) = ALL_CONDITION_TYPES.get(index).copied() {
                    // `onReplace(defaultConditionForType(t))`.
                    self.automation_mutate(rule, true, window, cx, move |rule| {
                        if let Some(slot) = rule.conditions.get_mut(condition) {
                            *slot = default_condition(kind);
                        }
                    });
                }
            }
            MenuKind::AutomationConditionTarget(rule, condition) => {
                if let Some((target, _)) = CAPTURE_TARGETS.get(index).copied() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Condition::CaptureTargetIs { target: slot }) =
                            rule.conditions.get_mut(condition)
                        {
                            *slot = target;
                        }
                    });
                }
            }
            MenuKind::AutomationConditionMode(rule, condition) => {
                if let Some((mode, _)) = RECORDING_MODES.get(index).copied() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Condition::RecordingModeIs { mode: slot }) =
                            rule.conditions.get_mut(condition)
                        {
                            *slot = mode;
                        }
                    });
                }
            }
            MenuKind::AutomationActionType(rule, action) => {
                if let Some(kind) = ALL_ACTION_TYPES.get(index).copied() {
                    // `onReplace(defaultActionForType(t))`.
                    self.automation_mutate(rule, true, window, cx, move |rule| {
                        if let Some(slot) = rule.actions.get_mut(action) {
                            *slot = default_action(kind);
                        }
                    });
                }
            }
            MenuKind::AutomationClipboardSource(rule, action) => {
                if let Some((source, _)) = CLIPBOARD_SOURCES.get(index).copied() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Action::CopyToClipboard { source: slot }) =
                            rule.actions.get_mut(action)
                        {
                            *slot = source;
                        }
                    });
                }
            }
            MenuKind::AutomationExportFormat(rule, action) => {
                if let Some((format, _)) = EXPORT_FORMATS.get(index).copied() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Action::Export { profile, .. }) = rule.actions.get_mut(action) {
                            profile.format = format;
                        }
                    });
                }
            }
            MenuKind::AutomationExportResolution(rule, action) => {
                if let Some((_, _, x, y)) = RESOLUTION_PRESETS.get(index).copied() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Action::Export { profile, .. }) = rule.actions.get_mut(action) {
                            profile.resolution_base = frameever_project::XY { x, y };
                        }
                    });
                }
            }
            MenuKind::AutomationExportFps(rule, action) => {
                if let Some(fps) = FPS_PRESETS.get(index).copied() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Action::Export { profile, .. }) = rule.actions.get_mut(action) {
                            profile.fps = fps;
                        }
                    });
                }
            }
            MenuKind::AutomationExportCompression(rule, action) => {
                if let Some((compression, _)) = COMPRESSIONS.get(index).copied() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Action::Export { profile, .. }) = rule.actions.get_mut(action) {
                            profile.compression = Some(compression);
                        }
                    });
                }
            }
            MenuKind::AutomationPreset(rule, action) => {
                if let Some(name) = store::preset_names().get(index).cloned() {
                    self.automation_mutate(rule, false, window, cx, move |rule| {
                        if let Some(Action::ApplyPreset { name: slot }) =
                            rule.actions.get_mut(action)
                        {
                            *slot = name;
                        }
                    });
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// A `ui::Select`-shaped trigger that opens one of these pages' menus.
    fn pages_select(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        kind: MenuKind,
        cx: &mut Context<Self>,
    ) -> ui::Select {
        ui::Select::settings(&self.theme, id, label).on_click(cx.listener(
            move |this, event: &gpui::ClickEvent, window, cx| {
                this.open_menu(kind, event.position(), window, cx);
            },
        ))
    }

    /// A page-owned single-line input on the settings surface.
    fn pages_input(
        &self,
        id: impl Into<gpui::ElementId>,
        input: &Entity<ui::TextInputState>,
    ) -> ui::TextInput {
        ui::TextInput::settings(&self.theme, id, input)
    }
}

// ---------------------------------------------------------------------------
// Shortcuts
// ---------------------------------------------------------------------------

const SCREENSHOT_SHORTCUTS: [&str; 3] = ["screenshotArea", "screenshotWindow", "screenshotDisplay"];

const RECORDING_SHORTCUTS: [&str; 7] = [
    "startStudioRecording",
    "startInstantRecording",
    "openRecordingPickerArea",
    "openRecordingPickerWindow",
    "openRecordingPickerDisplay",
    "openRecordingPicker",
    "cycleRecordingMode",
];

const WHILE_RECORDING_SHORTCUTS: [&str; 3] =
    ["stopRecording", "togglePauseRecording", "restartRecording"];

const SHORTCUT_SAVED_NOTICE: std::time::Duration = std::time::Duration::from_secs(6);

#[derive(Clone, Copy)]
struct OutcomeOption {
    icon: &'static str,
    label: &'static str,
    hint: &'static str,
}

const SCREENSHOT_OUTCOMES: [(store::PostScreenshotBehaviour, OutcomeOption); 4] = [
    (
        store::PostScreenshotBehaviour::OpenEditor,
        OutcomeOption {
            icon: "icons/edit.svg",
            label: n!("Open editor"),
            hint: n!("Annotate first"),
        },
    ),
    (
        store::PostScreenshotBehaviour::Copy,
        OutcomeOption {
            icon: "icons/copy.svg",
            label: n!("Copy"),
            hint: n!("Paste anywhere"),
        },
    ),
    (
        store::PostScreenshotBehaviour::CopyAndOpenEditor,
        OutcomeOption {
            icon: "icons/layers.svg",
            label: n!("Copy and edit"),
            hint: n!("Both at once"),
        },
    ),
    (
        store::PostScreenshotBehaviour::Save,
        OutcomeOption {
            icon: "icons/folder-down.svg",
            label: n!("Do nothing"),
            hint: n!("Saved to library"),
        },
    ),
];

const STUDIO_OUTCOMES: [(store::PostStudioBehaviour, OutcomeOption); 3] = [
    (
        store::PostStudioBehaviour::OpenEditor,
        OutcomeOption {
            icon: "icons/edit.svg",
            label: n!("Open editor"),
            hint: n!("Edit right away"),
        },
    ),
    (
        store::PostStudioBehaviour::ShowOverlay,
        OutcomeOption {
            icon: "icons/app-window.svg",
            label: n!("Show FrameEver"),
            hint: n!("Back to the recorder"),
        },
    ),
    (
        store::PostStudioBehaviour::DoNothing,
        OutcomeOption {
            icon: "icons/folder-down.svg",
            label: n!("Do nothing"),
            hint: n!("Saved to library"),
        },
    ),
];

/// `disableAutoOpenLinks`, phrased as what an Instant recording does.
const INSTANT_OUTCOMES: [(bool, OutcomeOption); 2] = [
    (
        false,
        OutcomeOption {
            icon: "icons/globe.svg",
            label: n!("Open link"),
            hint: n!("In your browser"),
        },
    ),
    (
        true,
        OutcomeOption {
            icon: "icons/link.svg",
            label: n!("Copy link only"),
            hint: n!("Nothing opens"),
        },
    ),
];

fn shortcut_label(action: &str) -> &'static str {
    match action {
        "screenshotArea" => t!("Screenshot an area"),
        "screenshotWindow" => t!("Screenshot a window"),
        "screenshotDisplay" => t!("Screenshot the display"),
        "startStudioRecording" => t!("Start Studio recording"),
        "startInstantRecording" => t!("Start Instant recording"),
        "openRecordingPickerArea" => t!("Record an area"),
        "openRecordingPickerWindow" => t!("Record a window"),
        "openRecordingPickerDisplay" => t!("Record a display"),
        "openRecordingPicker" => t!("Open recording picker"),
        "cycleRecordingMode" => t!("Cycle recording mode"),
        "stopRecording" => t!("Stop recording"),
        "togglePauseRecording" => t!("Pause or resume"),
        "restartRecording" => t!("Restart recording"),
        _ => t!("Shortcut"),
    }
}

fn screenshot_outcome(behaviour: store::PostScreenshotBehaviour) -> &'static str {
    match behaviour {
        store::PostScreenshotBehaviour::OpenEditor => t!("open it in the editor"),
        store::PostScreenshotBehaviour::Copy => t!("copy it to your clipboard"),
        store::PostScreenshotBehaviour::CopyAndOpenEditor => t!("copy it and open the editor"),
        store::PostScreenshotBehaviour::Save => {
            t!("save it to your library without opening anything")
        }
    }
}

fn picker_outcome(mode: crate::main_window::Mode) -> &'static str {
    match mode {
        crate::main_window::Mode::Studio => t!("record it in Studio mode"),
        crate::main_window::Mode::Instant => t!("record it in Instant mode"),
        crate::main_window::Mode::Screenshot => t!("take a screenshot of it"),
    }
}

/// What pressing the shortcut does, as a phrase that completes "Press ⌘⇧4
/// to ...". It follows the after-capture settings and the current mode, so
/// the row always describes what will actually happen.
fn shortcut_effect(
    action: &str,
    settings: &store::GeneralSettings,
    mode: crate::main_window::Mode,
) -> String {
    let screenshot = screenshot_outcome(settings.post_screenshot_behaviour);
    let studio_stop = match settings.post_studio_recording_behaviour {
        store::PostStudioBehaviour::OpenEditor => t!(", then open the editor when you stop"),
        store::PostStudioBehaviour::ShowOverlay | store::PostStudioBehaviour::DoNothing => "",
    };
    let instant_stop = if settings.disable_auto_open_links {
        t!(", then copy its share link when you stop")
    } else {
        t!(", then copy and open its share link when you stop")
    };
    match action {
        "screenshotArea" => t!("select an area and {screenshot}", screenshot = screenshot),
        "screenshotWindow" => t!(
            "capture the window under your pointer and {screenshot}",
            screenshot = screenshot
        ),
        "screenshotDisplay" => t!(
            "capture the display under your pointer and {screenshot}",
            screenshot = screenshot
        ),
        "startStudioRecording" => t!(
            "record your selected screen or window in Studio mode{studio_stop}",
            studio_stop = studio_stop
        ),
        "startInstantRecording" => t!(
            "record your selected screen or window in Instant mode{instant_stop}",
            instant_stop = instant_stop
        ),
        "openRecordingPickerArea" => t!(
            "select an area, then {outcome}",
            outcome = picker_outcome(mode)
        ),
        "openRecordingPickerWindow" => t!(
            "choose a window, then {outcome}",
            outcome = picker_outcome(mode)
        ),
        "openRecordingPickerDisplay" => t!(
            "choose a display, then {outcome}",
            outcome = picker_outcome(mode)
        ),
        "openRecordingPicker" => t!("open FrameEver so you can choose what to record").into(),
        "cycleRecordingMode" => t!("switch between Studio, Instant and Screenshot mode").into(),
        "stopRecording" => t!("stop the current recording").into(),
        "togglePauseRecording" => t!("pause the current recording, or resume it").into(),
        "restartRecording" => t!("throw away the current take and start again").into(),
        _ => String::new(),
    }
}

fn description_follows_outcome(action: &str) -> bool {
    SCREENSHOT_SHORTCUTS.contains(&action)
        || matches!(action, "startStudioRecording" | "startInstantRecording")
}

fn capitalized_sentence(phrase: &str) -> String {
    let mut chars = phrase.chars();
    match chars.next() {
        Some(first) => t!(
            "{sentence}.",
            sentence = format!("{}{}", first.to_uppercase(), chars.as_str())
        ),
        None => String::new(),
    }
}

/// The gpui keystroke name for a key, as the W3C `KeyboardEvent.code` string
/// the Tauri app stores (`e.code` in hotkeys.tsx, `global_hotkey::Code` in
/// hotkeys.rs). `None` for keys neither side can bind.
fn hotkey_code_for_key(key: &str) -> Option<String> {
    if let Some(rest) = key.strip_prefix('f')
        && !rest.is_empty()
        && rest.len() <= 2
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return Some(format!("F{rest}"));
    }
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return match c {
            'a'..='z' => Some(format!("Key{}", c.to_ascii_uppercase())),
            'A'..='Z' => Some(format!("Key{c}")),
            '0'..='9' => Some(format!("Digit{c}")),
            ',' => Some("Comma".into()),
            '.' => Some("Period".into()),
            '/' => Some("Slash".into()),
            ';' => Some("Semicolon".into()),
            '\'' => Some("Quote".into()),
            '[' => Some("BracketLeft".into()),
            ']' => Some("BracketRight".into()),
            '\\' => Some("Backslash".into()),
            '-' => Some("Minus".into()),
            '=' => Some("Equal".into()),
            '`' => Some("Backquote".into()),
            _ => None,
        };
    }
    let code = match key {
        "space" => "Space",
        "enter" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        _ => return None,
    };
    Some(code.to_string())
}

fn is_function_key(code: &str) -> bool {
    code.strip_prefix('F')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

/// Bare letters, digits and Shift-only combos would fire while typing in any
/// app, so a shortcut needs ⌘, ⌃ or ⌥ unless it is a function key.
fn shortcut_needs_modifier(hotkey: &Hotkey) -> bool {
    !(hotkey.meta || hotkey.ctrl || hotkey.alt) && !is_function_key(&hotkey.code)
}

const NEEDS_MODIFIER: &str = if cfg!(target_os = "macos") {
    n!("Include ⌘, ⌃ or ⌥ so the shortcut doesn't get in the way of typing.")
} else {
    n!("Include Ctrl, Alt or Win so the shortcut doesn't get in the way of typing.")
};

const UNSUPPORTED_KEY: &str =
    n!("That key can't be used in a shortcut. Try a letter, number or F-key.");

const RECORDING_HINT: &str = n!("Press the keys you want to use. Esc cancels, Delete removes it.");

const UNAVAILABLE: &str =
    n!("Another app or the system already uses this shortcut. Click to choose a different one.");

/// `HotkeyText`'s key list: modifiers in ⌘⌃⌥⇧ order, then the main key.
/// [`ui::kbd_symbol`] does the per-OS glyph mapping.
fn hotkey_display_keys(hotkey: &Hotkey) -> Vec<String> {
    let mut keys = Vec::new();
    if hotkey.meta {
        keys.push("meta".to_string());
    }
    if hotkey.ctrl {
        keys.push("ctrl".to_string());
    }
    if hotkey.alt {
        keys.push("alt".to_string());
    }
    if hotkey.shift {
        keys.push("shift".to_string());
    }
    keys.push(hotkey.code.clone());
    keys
}

fn held_modifier_keys(modifiers: gpui::Modifiers) -> Vec<String> {
    let mut keys = Vec::new();
    if modifiers.platform {
        keys.push("meta".to_string());
    }
    if modifiers.control {
        keys.push("ctrl".to_string());
    }
    if modifiers.alt {
        keys.push("alt".to_string());
    }
    if modifiers.shift {
        keys.push("shift".to_string());
    }
    keys
}

fn hotkey_inline_text(hotkey: &Hotkey) -> String {
    let separator = if cfg!(target_os = "macos") { "" } else { "+" };
    hotkey_display_keys(hotkey)
        .iter()
        .map(|key| hotkey_key_text(key))
        .collect::<Vec<_>>()
        .join(separator)
}

fn hotkey_key_text(key: &str) -> SharedString {
    if let Some(digit) = key.strip_prefix("Digit") {
        return SharedString::from(digit.to_string());
    }
    let named = match key {
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Semicolon" => ";",
        "Quote" => "'",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Minus" => "-",
        "Equal" => "=",
        "Backquote" => "`",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Tab" => "⇥",
        "Delete" => "⌦",
        _ => return ui::kbd_symbol(key),
    };
    SharedString::from(named)
}

fn shortcut_key_cap(theme: &Theme, key: &str) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .h(px(20.))
        .min_w(px(20.))
        .px(px(5.))
        .rounded(px(5.))
        .bg(theme.settings_card_bg())
        .border_1()
        .border_color(theme.settings_border())
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.settings_text())
        .child(hotkey_key_text(key))
}

impl SettingsWindow {
    fn shortcut_binding(&self, action: &str) -> Option<Hotkey> {
        self.pages
            .hotkeys
            .get(action)
            .and_then(store::hotkey_from_value)
    }

    fn shortcuts_start_listening(
        &mut self,
        action: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pages.shortcut_saved = None;
        self.pages.shortcut_saved_reset = None;
        crate::hotkeys::pause(cx);
        self.pages.listening = Some(ListeningHotkey {
            action,
            modifiers: window.modifiers(),
            error: None,
        });
        // Keys must reach the root's on_key_down, not a focused field.
        self.focus_root(window, cx);
        cx.notify();
    }

    /// Leave recording mode without touching the binding. Safe to call when
    /// nothing is recording; every exit path (Escape, a click elsewhere,
    /// another page, the window losing focus or closing) lands here.
    pub(crate) fn shortcuts_cancel(&mut self, cx: &mut Context<Self>) {
        if self.pages.listening.take().is_none() {
            return;
        }
        crate::hotkeys::resume(cx);
        cx.notify();
    }

    pub(crate) fn shortcuts_modifiers_changed(
        &mut self,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        if let Some(listening) = self.pages.listening.as_mut() {
            listening.modifiers = modifiers;
            cx.notify();
        }
    }

    /// The window keydown listener while a row is recording. Returns whether
    /// the key was consumed. Modifier-only presses never arrive as
    /// `KeyDownEvent`s in gpui; they come through
    /// [`Self::shortcuts_modifiers_changed`].
    pub(crate) fn shortcuts_capture_key(
        &mut self,
        keystroke: &gpui::Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.page != Page::Shortcuts {
            return false;
        }
        let Some(listening) = self.pages.listening.as_mut() else {
            return false;
        };
        let action = listening.action;
        let modifiers = keystroke.modifiers;
        let bare = !modifiers.platform && !modifiers.control && !modifiers.alt && !modifiers.shift;

        // Escape is the picker's cancel key, so it is never a binding.
        if keystroke.key == "escape" {
            self.shortcuts_cancel(cx);
            return true;
        }
        if bare && matches!(keystroke.key.as_str(), "backspace" | "delete") {
            self.pages.listening = None;
            self.pages.hotkeys.remove(action);
            self.shortcuts_commit(cx);
            return true;
        }

        // A code the Tauri app cannot parse must never reach the store: its
        // `HotkeysStore` deserializes the whole `hotkeys` map strictly
        // (`serde_json::from_value` in `HotkeysStore::get`), so one bad entry
        // would silently drop every binding over there.
        let Some(code) = hotkey_code_for_key(&keystroke.key)
            .filter(|code| global_hotkey::hotkey::Code::from_str(code).is_ok())
        else {
            listening.error = Some(UNSUPPORTED_KEY);
            cx.notify();
            return true;
        };
        let hotkey = Hotkey {
            code,
            meta: modifiers.platform,
            ctrl: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
        };
        if shortcut_needs_modifier(&hotkey) {
            listening.error = Some(NEEDS_MODIFIER);
            cx.notify();
            return true;
        }

        let mut moved_from = None;
        for other in SCREENSHOT_SHORTCUTS
            .iter()
            .chain(RECORDING_SHORTCUTS.iter())
            .chain(WHILE_RECORDING_SHORTCUTS.iter())
        {
            if *other != action && self.shortcut_binding(other).as_ref() == Some(&hotkey) {
                self.pages.hotkeys.remove(*other);
                moved_from.get_or_insert(*other);
            }
        }

        let value = serde_json::to_value(&hotkey).unwrap_or(Value::Null);
        self.pages.hotkeys.insert(action.to_string(), value);
        self.pages.listening = None;
        self.shortcuts_commit(cx);
        self.pages.shortcut_saved = Some(SavedShortcut { action, moved_from });
        self.pages.shortcut_saved_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SHORTCUT_SAVED_NOTICE).await;
            this.update(cx, |this, cx| {
                if this
                    .pages
                    .shortcut_saved
                    .as_ref()
                    .is_some_and(|saved| saved.action == action)
                {
                    this.pages.shortcut_saved = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        true
    }

    /// Persist the map and swap the OS registrations. Deferred because the
    /// registry swap reads the store this just wrote; resuming re-registers
    /// from the store, and so does a reload when nothing was paused.
    fn shortcuts_commit(&mut self, cx: &mut Context<Self>) {
        if !store::set_hotkeys_raw(&self.pages.hotkeys) {
            tracing::warn!("saving the hotkeys store failed");
        }
        cx.defer(|cx| {
            if !crate::hotkeys::resume(cx) {
                crate::hotkeys::reload(cx);
            }
            cx.refresh_windows();
        });
        cx.notify();
    }

    pub(crate) fn render_shortcuts(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let unavailable = crate::hotkeys::unavailable_actions(cx);
        let mode = crate::main_window::Mode::from_store();
        let rows_for = |actions: &[&'static str], cx: &mut Context<Self>| {
            actions
                .iter()
                .map(|action| {
                    self.render_shortcut_row(action, mode, unavailable.contains(action), cx)
                })
                .collect::<Vec<_>>()
        };

        let mut screenshots = rows_for(&SCREENSHOT_SHORTCUTS, cx);
        screenshots.push(
            self.render_outcome_panel(
                "outcome-screenshot",
                t!("After a screenshot"),
                t!("For every screenshot, from a shortcut or from FrameEver"),
                &SCREENSHOT_OUTCOMES
                    .iter()
                    .map(|(value, option)| {
                        (*option, *value == self.settings.post_screenshot_behaviour)
                    })
                    .collect::<Vec<_>>(),
                |this, index, cx| {
                    if let Some((value, _)) = SCREENSHOT_OUTCOMES.get(index) {
                        this.settings.post_screenshot_behaviour = *value;
                        this.write_enum("postScreenshotBehaviour", *value, cx);
                    }
                },
                cx,
            ),
        );

        let mut recording = rows_for(&RECORDING_SHORTCUTS, cx);
        recording.push(
            self.render_outcome_panel(
                "outcome-studio",
                t!("After a Studio recording"),
                t!("When you stop"),
                &STUDIO_OUTCOMES
                    .iter()
                    .map(|(value, option)| {
                        (
                            *option,
                            *value == self.settings.post_studio_recording_behaviour,
                        )
                    })
                    .collect::<Vec<_>>(),
                |this, index, cx| {
                    if let Some((value, _)) = STUDIO_OUTCOMES.get(index) {
                        this.settings.post_studio_recording_behaviour = *value;
                        this.write_enum("postStudioRecordingBehaviour", *value, cx);
                    }
                },
                cx,
            ),
        );
        recording.push(
            self.render_outcome_panel(
                "outcome-instant",
                t!("After an Instant recording"),
                t!("The share link is always copied"),
                &INSTANT_OUTCOMES
                    .iter()
                    .map(|(disabled, option)| {
                        (*option, *disabled == self.settings.disable_auto_open_links)
                    })
                    .collect::<Vec<_>>(),
                |this, index, cx| {
                    if let Some((disabled, _)) = INSTANT_OUTCOMES.get(index) {
                        this.settings.disable_auto_open_links = *disabled;
                        this.write_bool("disableAutoOpenLinks", *disabled, cx);
                    }
                },
                cx,
            ),
        );

        let while_recording = rows_for(&WHILE_RECORDING_SHORTCUTS, cx);

        let listening_any = self.pages.listening.is_some();
        let card = |id: &'static str, rows: Vec<gpui::AnyElement>, cx: &mut Context<Self>| {
            self.rows(rows)
                .id(id)
                .when(listening_any, |this| {
                    this.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| this.shortcuts_cancel(cx)),
                    )
                })
                .into_any_element()
        };

        vec![
            self.section(
                t!("Screenshots"),
                Some(t!("Shortcuts work from any app. Click one to change it.")),
                None,
                vec![card("shortcuts-screenshots", screenshots, cx)],
            )
            .into_any_element(),
            self.section(
                t!("Recording"),
                Some(t!("Start a recording without opening FrameEver first.")),
                None,
                vec![card("shortcuts-recording", recording, cx)],
            )
            .into_any_element(),
            self.section(
                t!("While recording"),
                None,
                None,
                vec![card("shortcuts-while-recording", while_recording, cx)],
            )
            .into_any_element(),
        ]
    }

    fn render_outcome_panel(
        &self,
        id: &'static str,
        title: &'static str,
        hint: &'static str,
        options: &[(OutcomeOption, bool)],
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let ink = theme.settings_text();
        // The card tokens are translucent, so on this tinted panel they read
        // as gray; the chosen option needs a solid, lighter surface.
        let raised = if theme.is_dark() {
            gpui::hsla(0., 0., 1., 0.1)
        } else {
            gpui::white()
        };
        let on_select = std::rc::Rc::new(on_select);

        let cards =
            options
                .iter()
                .enumerate()
                .map(|(index, (option, selected))| {
                    let selected = *selected;
                    let on_select = on_select.clone();
                    div()
                        .id((id, index))
                        .relative()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .flex_1()
                        .min_w_0()
                        .p(px(10.))
                        .rounded(px(10.))
                        .border_1()
                        .when(selected, |this| this.border_color(ink).bg(raised))
                        .when(!selected, |this| {
                            this.border_color(theme.settings_border())
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.settings_hover()))
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(28.))
                                .rounded(px(7.))
                                .bg(theme.settings_fill())
                                .child(svg().path(option.icon).size(px(16.)).text_color(
                                    if selected {
                                        ink
                                    } else {
                                        theme.settings_muted()
                                    },
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(ink)
                                        .child(tr(option.label)),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .line_height(px(14.))
                                        .text_color(theme.settings_muted())
                                        .child(tr(option.hint)),
                                ),
                        )
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.shortcuts_cancel(cx);
                            on_select(this, index, cx);
                            cx.notify();
                        }))
                });

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .bg(theme.settings_fill())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.settings_muted())
                            .child(hint),
                    ),
            )
            .child(div().flex().flex_row().gap(px(8.)).children(cards))
            .into_any_element()
    }

    fn render_shortcut_row(
        &self,
        action: &'static str,
        mode: crate::main_window::Mode,
        unavailable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let binding = self.shortcut_binding(action);
        let listening = self
            .pages
            .listening
            .as_ref()
            .filter(|listening| listening.action == action);
        let saved = self
            .pages
            .shortcut_saved
            .as_ref()
            .filter(|saved| saved.action == action);
        let effect = shortcut_effect(action, &self.settings, mode);
        let red: Hsla = theme.red_11.into();
        let amber: Hsla = theme.amber_11.into();

        let (detail, detail_color, detail_icon): (String, Hsla, Option<(&str, Hsla)>) =
            if let Some(listening) = listening {
                match listening.error {
                    Some(error) => (tr(error).into(), red, None),
                    None => (tr(RECORDING_HINT).into(), theme.settings_muted(), None),
                }
            } else if unavailable && binding.is_some() {
                (tr(UNAVAILABLE).into(), amber, None)
            } else if let (Some(saved), Some(binding)) = (saved, binding.as_ref()) {
                let mut text = t!(
                    "Saved. Press {keys} in any app to {effect}.",
                    keys = hotkey_inline_text(binding),
                    effect = effect
                );
                if let Some(from) = saved.moved_from {
                    text.push_str(&t!(
                        " Removed from {shortcut}.",
                        shortcut = shortcut_label(from)
                    ));
                }
                (
                    text,
                    theme.settings_text(),
                    Some(("icons/circle-check.svg", theme.settings_text())),
                )
            } else {
                (capitalized_sentence(&effect), theme.settings_muted(), None)
            };

        let group: SharedString = format!("shortcut-row-{action}").into();

        let field = self.render_shortcut_field(action, binding.as_ref(), listening, cx);
        let clear = div()
            .id(SharedString::from(format!("shortcut-clear-{action}")))
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.))
            .rounded(px(6.))
            .cursor_pointer()
            .invisible()
            .when(binding.is_some() && listening.is_none(), |this| {
                this.group_hover(group.clone(), |style| style.visible())
            })
            .hover(|style| style.bg(theme.settings_hover()))
            .child(
                svg()
                    .path("icons/x.svg")
                    .size(px(12.))
                    .text_color(theme.settings_muted()),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.shortcuts_cancel(cx);
                this.pages.hotkeys.remove(action);
                if this
                    .pages
                    .shortcut_saved
                    .as_ref()
                    .is_some_and(|saved| saved.action == action)
                {
                    this.pages.shortcut_saved = None;
                }
                this.shortcuts_commit(cx);
            }));

        div()
            .id(SharedString::from(format!("shortcut-{action}")))
            .group(group)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .min_h(px(56.))
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().text_size(px(13.)).child(shortcut_label(action)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(5.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(detail_color)
                            // Picking an "After ..." option rewrites this text,
                            // and a one-line/two-line swap moved the panel out
                            // from under the pointer. Two lines are reserved
                            // for every description that can change that way.
                            .when(description_follows_outcome(action), |this| {
                                this.min_h(px(32.))
                            })
                            .when_some(detail_icon, |this, (path, color)| {
                                this.child(
                                    svg()
                                        .path(path)
                                        .flex_shrink_0()
                                        .mt(px(2.))
                                        .size(px(12.))
                                        .text_color(color),
                                )
                            })
                            .child(div().min_w_0().child(detail)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(4.))
                    .flex_shrink_0()
                    .child(clear)
                    .child(field),
            )
            .into_any_element()
    }

    fn render_shortcut_field(
        &self,
        action: &'static str,
        binding: Option<&Hotkey>,
        listening: Option<&ListeningHotkey>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;

        let base = div()
            .id(SharedString::from(format!("shortcut-field-{action}")))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(3.))
            .h(px(30.))
            .min_w(px(132.))
            .px(px(5.))
            .rounded(px(8.))
            .border_1()
            .cursor_pointer();

        let field = match (listening, binding) {
            (Some(listening), _) => {
                let held = held_modifier_keys(listening.modifiers);
                let mut field = base
                    .border_color(theme.settings_text())
                    .bg(theme.settings_card_bg())
                    .children(held.iter().map(|key| shortcut_key_cap(&theme, key)));
                if held.is_empty() {
                    field = field.child(
                        div()
                            .px(px(4.))
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child(t!("Type shortcut…")),
                    );
                } else {
                    field = field.child(
                        div()
                            .px(px(2.))
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child("…"),
                    );
                }
                field.into_any_element()
            }
            (None, Some(hotkey)) => base
                .border_color(theme.settings_border())
                .bg(theme.settings_fill())
                .hover(|style| style.bg(theme.settings_hover()))
                .children(
                    hotkey_display_keys(hotkey)
                        .iter()
                        .map(|key| shortcut_key_cap(&theme, key)),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.shortcuts_start_listening(action, window, cx);
                }))
                .into_any_element(),
            (None, None) => base
                .border_color(theme.settings_border())
                .border_dashed()
                .hover(|style| style.bg(theme.settings_hover()))
                .child(
                    div()
                        .px(px(4.))
                        .text_size(px(12.))
                        .text_color(theme.settings_muted())
                        .child(t!("Record shortcut")),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.shortcuts_start_listening(action, window, cx);
                }))
                .into_any_element(),
        };
        div().child(field).into_any_element()
    }
}

// ---------------------------------------------------------------------------
// CLI (cli.tsx)
// ---------------------------------------------------------------------------

impl SettingsWindow {
    fn cli_refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pages.cli_status = None;
        cx.spawn_in(window, async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async { cli_install::status() })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.pages.cli_status = Some(status);
                cx.notify();
                window.refresh();
            })
            .ok();
        })
        .detach();
    }

    fn cli_run(&mut self, uninstall: bool, window: &mut Window, cx: &mut Context<Self>) {
        if uninstall {
            self.pages.cli_uninstalling = true;
        } else {
            self.pages.cli_installing = true;
        }
        self.pages.cli_error = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if uninstall {
                        cli_install::uninstall()
                    } else {
                        cli_install::install()
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.pages.cli_installing = false;
                this.pages.cli_uninstalling = false;
                match result {
                    Ok(status) => this.pages.cli_status = Some(Ok(status)),
                    Err(error) => {
                        // `toast.error(..)` then `refetch()` in cli.tsx.
                        this.pages.cli_error = Some(error);
                        this.cli_refresh(window, cx);
                    }
                }
                cx.notify();
                window.refresh();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn render_cli(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;

        let body: gpui::AnyElement = match &self.pages.cli_status {
            // `h-20 rounded-lg bg-gray-3 animate-pulse` (static here -- no
            // keyframe ticker for a placeholder).
            None => div()
                .h(px(80.))
                .rounded(px(8.))
                .bg(theme.settings_fill())
                .into_any_element(),
            Some(Err(error)) => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(Hsla::from(theme.red_9))
                        .child(t!("Couldn't load CLI status: {error}", error = error)),
                )
                .child(div().child(self.button(
                    "cli-retry",
                    (ui::ButtonVariant::Gray, None),
                    t!("Retry"),
                    false,
                    cx,
                    |this, window, cx| this.cli_refresh(window, cx),
                )))
                .into_any_element(),
            Some(Ok(status)) => self
                .render_cli_status(status.clone(), cx)
                .into_any_element(),
        };

        vec![
            self.section(
                t!("Command Line"),
                Some(
                    t!("Install the FrameEver command for terminals, agents, scripts, and local automation."),
                ),
                None,
                vec![self.card(true).child(body).into_any_element()],
            )
            .into_any_element(),
        ]
    }

    fn render_cli_status(
        &self,
        status: cli_install::CliInstallStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let installing = self.pages.cli_installing;
        let uninstalling = self.pages.cli_uninstalling;

        let install_label = if installing {
            if status.installed {
                t!("Repairing...")
            } else {
                t!("Installing...")
            }
        } else if status.installed {
            t!("Repair")
        } else {
            t!("Install CLI")
        };

        // `<code class="font-mono text-gray-12">`.
        let code_span = |text: String| {
            div()
                .font_family(MONO_FONT)
                .text_color(theme.settings_text())
                .child(text)
        };

        let header = div()
            .flex()
            .flex_row()
            .items_start()
            .justify_between()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .min_w_0()
                    .child(div().text_size(px(13.)).child(if status.installed {
                        t!("Installed")
                    } else {
                        t!("Not installed")
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(theme.settings_muted())
                            .child(t!("The desktop app installs a local\u{a0}"))
                            .child(code_span(crate::brand::CLI_COMMAND.into()).text_size(px(12.)))
                            .child(t!("\u{a0}command that points back to the bundled CLI.")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_shrink_0()
                    .gap(px(8.))
                    .when(status.installed, |this| {
                        this.child(self.button(
                            "cli-remove",
                            (ui::ButtonVariant::Gray, None),
                            if uninstalling {
                                t!("Removing...")
                            } else {
                                t!("Remove")
                            },
                            uninstalling,
                            cx,
                            |this, window, cx| this.cli_run(true, window, cx),
                        ))
                    })
                    .child(self.button(
                        "cli-install",
                        (ui::ButtonVariant::Dark, None),
                        install_label,
                        installing,
                        cx,
                        |this, window, cx| this.cli_run(false, window, cx),
                    )),
            );

        // `<PathRow>`: `w-16` label, mono value chip.
        let path_row = |label: &'static str, value: String| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .min_w_0()
                .child(
                    div()
                        .w(px(64.))
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .text_color(theme.settings_muted())
                        .child(label),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(6.))
                        .bg(theme.settings_fill())
                        .font_family(MONO_FONT)
                        .text_size(px(11.))
                        .text_color(theme.settings_text())
                        .child(value),
                )
        };

        let shell_command = status.shell_command.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(path_row(t!("Command"), status.shim_path.clone()))
                    .child(path_row(t!("Target"), status.target_path.clone())),
            )
            .when_some(status.conflict.clone(), |this, conflict| {
                // `rounded-lg border border-red-300/40 bg-red-500/10 px-3 py-2
                //  text-xs text-red-11`.
                this.child(
                    div()
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Theme::with_alpha(theme.red_9, 0.4))
                        .bg(Theme::with_alpha(theme.red_9, 0.1))
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(Hsla::from(theme.red_9))
                        .child(conflict),
                )
            })
            .when_some(self.pages.cli_error.clone(), |this, error| {
                // cli.tsx surfaces install/remove failures as toasts; inline
                // red text is the stand-in.
                this.child(
                    div()
                        .text_size(px(12.))
                        .text_color(Hsla::from(theme.red_9))
                        .child(error),
                )
            })
            .when(status.installed && !status.on_path, |this| {
                let help: gpui::AnyElement = if status.path_configured {
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(theme.settings_muted())
                        .child(t!(
                            "Added {command} to your PATH. Restart your terminal to use it, or \
                             run this now:",
                            command = crate::brand::CLI_COMMAND
                        ))
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(theme.settings_muted())
                        .child(t!(
                            "Add {path} to your PATH to use {command} from a new terminal.",
                            path = status.path_entry,
                            command = crate::brand::CLI_COMMAND
                        ))
                        .into_any_element()
                };
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(12.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.settings_border())
                        .bg(theme.settings_fill())
                        .child(help)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(6.))
                                        .bg(theme.settings_card_bg())
                                        .font_family(MONO_FONT)
                                        .text_size(px(12.))
                                        .text_color(theme.settings_text())
                                        .child(status.shell_command.clone()),
                                )
                                .child(self.button(
                                    "cli-copy-path",
                                    (ui::ButtonVariant::Gray, None),
                                    if self.pages.cli_copied {
                                        t!("Copied")
                                    } else {
                                        t!("Copy")
                                    },
                                    false,
                                    cx,
                                    move |this, window, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            shell_command.clone(),
                                        ));
                                        this.pages.cli_copied = true;
                                        this.pages.cli_copied_reset =
                                            Some(cx.spawn_in(window, async move |this, cx| {
                                                cx.background_executor()
                                                    .timer(Duration::from_millis(1500))
                                                    .await;
                                                this.update(cx, |this, cx| {
                                                    this.pages.cli_copied = false;
                                                    cx.notify();
                                                })
                                                .ok();
                                            }));
                                        cx.notify();
                                    },
                                )),
                        ),
                )
            })
    }
}

// ---------------------------------------------------------------------------
// Transcription (transcription.tsx)
// ---------------------------------------------------------------------------

impl SettingsWindow {
    /// `addHint()`.
    fn transcription_add_hint(&mut self, cx: &mut Context<Self>) {
        let value = self.pages.hint_draft.replace('\0', "");
        let value = value.trim().to_string();
        if value.is_empty() {
            return;
        }

        let mut next = self.pages.hints.clone();
        next.push(value);
        let next = store::normalize_transcription_hints(next);
        let input = self.pages.hint_input.clone();
        input.update(cx, |input, cx| input.set_text("", cx));
        self.pages.hint_draft.clear();
        if next.len() == self.pages.hints.len() {
            cx.notify();
            return;
        }
        self.pages.hints = next.clone();
        self.transcription_persist(next, cx);
    }

    /// `persist()`: 250ms debounce, then the store write, then the 1.2s
    /// "Saved" hold. Assigning over the task drops the pending write, which
    /// is `clearTimeout`.
    fn transcription_persist(&mut self, hints: Vec<String>, cx: &mut Context<Self>) {
        let normalized = store::normalize_transcription_hints(hints);
        self.pages.hint_save = HintSave::Saving;
        self.pages.hint_save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let ok = this
                .update(cx, |this, cx| {
                    let ok = store::set_store_setting(
                        GENERAL_SETTINGS,
                        "transcriptionHints",
                        Value::Array(normalized.iter().cloned().map(Value::String).collect()),
                    );
                    this.pages.hint_save = if ok { HintSave::Saved } else { HintSave::Idle };
                    cx.notify();
                    ok
                })
                .unwrap_or(false);
            if !ok {
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            this.update(cx, |this, cx| {
                if this.pages.hint_save == HintSave::Saved {
                    this.pages.hint_save = HintSave::Idle;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn transcription_remove_hint(&mut self, hint: String, cx: &mut Context<Self>) {
        self.pages.hints.retain(|existing| *existing != hint);
        let next = self.pages.hints.clone();
        self.transcription_persist(next, cx);
    }

    pub(crate) fn render_transcription(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;
        let hints = self.pages.hints.clone();
        let save_label = match self.pages.hint_save {
            HintSave::Saving => t!("Saving..."),
            HintSave::Saved => t!("Saved"),
            HintSave::Idle => "",
        };

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .min_w_0()
                    .child(div().text_size(px(13.)).child(t!("Remembered terms")))
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(theme.settings_muted())
                            .child(t!(
                                "Add one term at a time to reduce typos and formatting mistakes."
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .when(!hints.is_empty(), |this| {
                        this.child(self.button(
                            "hints-clear",
                            (ui::ButtonVariant::Gray, None),
                            t!("Clear"),
                            false,
                            cx,
                            |this, _window, cx| {
                                this.pages.hints.clear();
                                this.transcription_persist(Vec::new(), cx);
                            },
                        ))
                    })
                    .child(
                        // `min-w-15 text-right text-xs text-gray-11`.
                        div()
                            .min_w(px(60.))
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child(save_label),
                    ),
            );

        let add_disabled = self.pages.hint_draft.trim().is_empty();
        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.pages_input("hint-input", &self.pages.hint_input)),
            )
            .child(self.button(
                "hints-add",
                (ui::ButtonVariant::Primary, Some("icons/plus.svg")),
                t!("Add"),
                add_disabled,
                cx,
                |this, _window, cx| this.transcription_add_hint(cx),
            ));

        let card = self
            .card(true)
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(header)
            .child(input_row)
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(theme.settings_muted())
                    .child(t!(
                        "These hints are applied when you generate captions in the editor."
                    )),
            );

        let mut sections = vec![
            self.section(
                t!("Transcription"),
                Some(t!(
                    "Add names, spellings, domains, and capitalization preferences that caption \
                     generation should keep in mind."
                )),
                None,
                vec![card.into_any_element()],
            )
            .into_any_element(),
        ];

        if !hints.is_empty() {
            let count = div()
                .text_size(px(12.))
                .text_color(theme.settings_muted())
                .child(if hints.len() == 1 {
                    t!("1 item").to_string()
                } else {
                    t!("{count} items", count = hints.len())
                })
                .into_any_element();
            let chips = div().flex().flex_row().flex_wrap().gap(px(8.)).children(
                hints.into_iter().enumerate().map(|(index, hint)| {
                    let remove = hint.clone();
                    // `px-2.5 py-1 rounded-full text-xs bg-gray-3 border
                    //  border-gray-4 hover:bg-gray-4`.
                    div()
                        .id(("hint-chip", index))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .py(px(4.))
                        .rounded_full()
                        .bg(theme.settings_fill())
                        .border_1()
                        .border_color(theme.settings_border())
                        .text_size(px(12.))
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.settings_selection()))
                        .child(hint)
                        .child(
                            svg()
                                .path("icons/x.svg")
                                .size(px(12.))
                                .text_color(theme.settings_muted()),
                        )
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.transcription_remove_hint(remove.clone(), cx);
                        }))
                }),
            );

            sections.push(
                self.section(
                    t!("Active hints"),
                    None,
                    Some(count),
                    vec![self.card(true).child(chips).into_any_element()],
                )
                .into_any_element(),
            );
        }

        sections
    }
}

// ---------------------------------------------------------------------------
// Experimental (experimental.tsx)
// ---------------------------------------------------------------------------

pub(crate) enum SwitchBack {
    Running(std::time::Instant),
    WaitingForClassic,
    Failed(String),
}

/// A cold dev build can genuinely take this long; past it, assume the build
/// failed and hand the user back their app with a pointer at the terminal.
const CLASSIC_WAIT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

const SWITCH_SENTENCES: &[&str] = &[
    n!("Switching back to the classic app."),
    n!("Your recordings and settings stay exactly where they are."),
];
const SWITCH_SENTENCE_MS: u64 = 2000;
const SWITCH_FADE_MS: f32 = 400.;
const SWITCH_COUNTDOWN_FROM: u32 = 5;
const SWITCH_TAKEOVER_MS: u64 = SWITCH_COUNTDOWN_FROM as u64 * 1000;

/// The overlay's whole appearance at one instant: which sentence, how opaque,
/// and the number.
fn takeover_frame(elapsed_ms: f32) -> (usize, f32, u32) {
    let last = SWITCH_SENTENCES.len() - 1;
    let index = ((elapsed_ms / SWITCH_SENTENCE_MS as f32) as usize).min(last);
    let within = elapsed_ms - index as f32 * SWITCH_SENTENCE_MS as f32;
    let fade_in = (within / SWITCH_FADE_MS).clamp(0., 1.);
    // The last line holds rather than fading out; blanking the column while the
    // countdown finishes would read as a stall.
    let alpha = if index == last {
        fade_in
    } else {
        fade_in.min(((SWITCH_SENTENCE_MS as f32 - within) / SWITCH_FADE_MS).clamp(0., 1.))
    };
    // Five on the first frame down to one in the last second; the switch fires
    // rather than ever showing zero.
    let left = SWITCH_TAKEOVER_MS as f32 - elapsed_ms;
    let remaining = (left / 1000.)
        .ceil()
        .clamp(1., SWITCH_COUNTDOWN_FROM as f32) as u32;
    (index, alpha, remaining)
}

/// Cap ships as this app alone, so the classic Tauri app only exists inside
/// the `tauri dev` harness: a debug cargo build can hand the session back to
/// it through the dev-session supervisor (`store::request_classic_reopen`),
/// and nothing else has a classic app to switch to.
fn dev_supervisor_available_for(exe: Option<&std::path::Path>, debug_build: bool) -> bool {
    debug_build
        && exe.is_some_and(|exe| {
            exe.components()
                .any(|component| component.as_os_str() == "target")
        })
}

fn dev_supervisor_available() -> bool {
    dev_supervisor_available_for(
        std::env::current_exe().ok().as_deref(),
        cfg!(debug_assertions),
    )
}

impl SettingsWindow {
    pub(crate) fn render_experimental(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        // No "Native camera preview" toggle here: in this app the native
        // camera path is the only implementation, so the Tauri page's
        // experimental switch has nothing to switch. The store key
        // (`enableNativeCameraPreview`) stays readable in `store.rs` and is
        // left untouched in the shared store -- the Tauri app still uses it.
        let mut sections = vec![
            self.section(
                t!("Reliability"),
                None,
                None,
                vec![
                    self.rows(vec![
                        self.setting_row(
                            t!("Out-of-process muxer"),
                            Some(t!(
                                "Run the fragmented-MP4 muxer in an isolated subprocess so muxer \
                             crashes can't take down your recording. Requires the bundled \
                             frameever-muxer binary."
                            )),
                            self.toggle(
                                "out-of-process-muxer",
                                self.settings.out_of_process_muxer,
                                cx,
                                |this, cx| {
                                    this.settings.out_of_process_muxer =
                                        !this.settings.out_of_process_muxer;
                                    let value = this.settings.out_of_process_muxer;
                                    this.write_bool("outOfProcessMuxer", value, cx);
                                },
                            )
                            .into_any_element(),
                        ),
                    ])
                    .into_any_element(),
                ],
            )
            .into_any_element(),
        ];
        if dev_supervisor_available() {
            sections.push(
                self.section(
                    t!("Developer"),
                    Some(t!("Only shown in development builds.")),
                    None,
                    vec![self.rows(vec![self.classic_app_row(cx)]).into_any_element()],
                )
                .into_any_element(),
            );
        }
        sections
    }

    /// The dev harness's way back to the classic app: being in this app is what
    /// the toggle means, and turning it off reopens the classic app under
    /// `tauri dev`.
    fn classic_app_row(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let checked = !matches!(self.pages.switch_back, Some(SwitchBack::Running(_)));
        self.setting_row(
            t!("Use native app"),
            Some(t!(
                "Turn off to close this app and reopen the classic Tauri app in the dev harness. \
                 Recordings and settings are shared."
            )),
            self.toggle("enable-gpui-app", checked, cx, |this, cx| {
                match this.pages.switch_back {
                    Some(SwitchBack::Running(_)) => this.cancel_switch_back(cx),
                    // Committed: the classic app is already being brought up.
                    Some(SwitchBack::WaitingForClassic) => {}
                    _ => this.start_switch_back(cx),
                }
            })
            .into_any_element(),
        )
    }

    /// The full-window takeover, painted from `SettingsWindow::render` so it
    /// covers the sidebar and the menus too.
    pub(crate) fn render_switch_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let switch = self.pages.switch_back.as_ref()?;
        let white = gpui::white();
        let mut column = div()
            .absolute()
            .inset_0()
            .when(cfg!(target_os = "windows"), |overlay| overlay.top(px(36.)))
            // Nothing behind the takeover is clickable while it runs.
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(28.))
            .bg(gpui::hsla(0., 0., 0., 0.92));

        match switch {
            SwitchBack::Running(started) => {
                let (index, alpha, remaining) =
                    takeover_frame(started.elapsed().as_secs_f32() * 1000.);
                column = column
                    .child(
                        div()
                            .text_size(px(72.))
                            .line_height(px(76.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(white)
                            .child(SharedString::from(remaining.to_string())),
                    )
                    .child(
                        div()
                            .h(px(40.))
                            .max_w(px(380.))
                            .flex()
                            .items_center()
                            .text_size(px(15.))
                            .text_center()
                            .text_color(gpui::hsla(0., 0., 1., alpha))
                            .child(tr(SWITCH_SENTENCES[index])),
                    );
            }
            SwitchBack::WaitingForClassic => {
                column = column
                    .child(
                        div()
                            .max_w(px(380.))
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_center()
                            .text_color(white)
                            .child(t!("Opening the classic app")),
                    )
                    .child(
                        div()
                            .max_w(px(380.))
                            .text_size(px(14.))
                            .text_center()
                            .text_color(gpui::hsla(0., 0., 1., 0.7))
                            .child(t!("This window will close when the classic app is ready.")),
                    );
            }
            SwitchBack::Failed(error) => {
                column = column.child(
                    div()
                        .max_w(px(380.))
                        .text_size(px(14.))
                        .text_center()
                        .text_color(rgb(0xf87171))
                        .child(SharedString::from(error.clone())),
                );
            }
        }

        // Committed states have nothing to cancel.
        if matches!(switch, SwitchBack::WaitingForClassic) {
            return Some(column.into_any_element());
        }

        Some(
            column
                .child(
                    div()
                        .id("switch-back-cancel")
                        .flex()
                        .items_center()
                        .px(px(16.))
                        .h(px(32.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(gpui::hsla(0., 0., 1., 0.25))
                        .text_size(px(13.))
                        .text_color(white)
                        .cursor_pointer()
                        .hover(|style| style.bg(gpui::hsla(0., 0., 1., 0.12)))
                        .on_click(cx.listener(|this, _, _window, cx| this.cancel_switch_back(cx)))
                        .child(if matches!(switch, SwitchBack::Failed(_)) {
                            t!("Close")
                        } else {
                            t!("Cancel")
                        }),
                )
                .into_any_element(),
        )
    }

    fn start_switch_back(&mut self, cx: &mut Context<Self>) {
        if self.switch_back_blocked(cx) {
            return;
        }

        self.pages.switch_back = Some(SwitchBack::Running(std::time::Instant::now()));
        // gpui only renders on invalidation, so the fades and the countdown
        // need a pulse to run at all -- the `toggle_placeholders` shape.
        // Assigning over the previous task drops it, which is how Cancel and a
        // restart stop the one already in flight.
        self.pages.switch_back_ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let running = this
                    .update(cx, |this, cx| {
                        let started = match &this.pages.switch_back {
                            Some(SwitchBack::Running(started)) => *started,
                            _ => return false,
                        };
                        if started.elapsed() >= Duration::from_millis(SWITCH_TAKEOVER_MS) {
                            this.finish_switch_back(cx);
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !running {
                    return;
                }
            }
        }));
        cx.notify();
    }

    fn cancel_switch_back(&mut self, cx: &mut Context<Self>) {
        self.pages.switch_back = None;
        self.pages.switch_back_ticker = None;
        cx.notify();
    }

    fn switch_back_blocked(&mut self, cx: &mut Context<Self>) -> bool {
        if !crate::session::RecordingSession::work_in_flight(cx) {
            return false;
        }

        self.pages.switch_back = Some(SwitchBack::Failed(
            t!("Finish your recording, export, import, or transcription task before switching to the classic app.")
                .to_string(),
        ));
        cx.notify();
        true
    }

    /// Hand the session (`enableGpuiApp`) to the classic dev app, ask the dev
    /// supervisor to start it, then quit once it is visible.
    fn finish_switch_back(&mut self, cx: &mut Context<Self>) {
        if self.switch_back_blocked(cx) {
            return;
        }
        if !dev_supervisor_available() {
            self.pages.switch_back = Some(SwitchBack::Failed(
                t!("Couldn't find the classic app to switch back to.").to_string(),
            ));
            cx.notify();
            return;
        }
        if let Err(error) = crate::app_windows::flush_pending_editor_saves(cx) {
            self.pages.switch_back = Some(SwitchBack::Failed(error));
            cx.notify();
            return;
        }

        if !store::set_store_setting(GENERAL_SETTINGS, "enableGpuiApp", Value::Bool(false)) {
            self.pages.switch_back = Some(SwitchBack::Failed(
                t!("Couldn't save your app preference. FrameEver is still open.").to_string(),
            ));
            cx.notify();
            return;
        }
        let pending = store::mark_classic_pending();
        self.pages.switch_back = Some(SwitchBack::WaitingForClassic);
        self.pages.switch_back_ticker = None;
        cx.notify();

        // The handoff must finish even if its settings window closes.
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { pending.and_then(|()| store::request_classic_reopen()) })
                .await;
            let mut failure = result
                .err()
                .map(|error| t!("Couldn't open the classic app: {error}", error = error));
            let started = std::time::Instant::now();
            while failure.is_none() {
                match store::classic_pending_path().try_exists() {
                    Ok(false) => {
                        tracing::info!("classic app is visible; quitting GPUI");
                        cx.update(crate::menus::quit);
                        return;
                    }
                    Ok(true) => {}
                    Err(error) => {
                        failure = Some(t!(
                            "Couldn't check whether the classic app opened: {error}",
                            error = error
                        ));
                        break;
                    }
                }
                if started.elapsed() >= CLASSIC_WAIT_TIMEOUT {
                    failure = Some(
                        t!("The classic app hasn't opened. Check the dev terminal for build errors, then try again.")
                            .to_string(),
                    );
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
            }
            let message =
                failure.unwrap_or_else(|| t!("Couldn't open the classic app.").to_string());
            tracing::error!("{message}");
            store::clear_classic_pending();
            store::set_store_setting(GENERAL_SETTINGS, "enableGpuiApp", Value::Bool(true));
            this.update(cx, |this, cx| {
                this.pages.switch_back = Some(SwitchBack::Failed(message));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

// ---------------------------------------------------------------------------
// Feedback (feedback.tsx)
// ---------------------------------------------------------------------------

impl SettingsWindow {
    /// The page's two one-shot probes: the OS string and whether there is a
    /// `cap` sidecar to run the sync test with. Both stat the filesystem, so
    /// neither runs on the UI thread and neither is repeated per frame.
    fn feedback_load_os_version(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pages.os_version.is_some() {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let version = cx.background_executor().spawn(async { os_version() }).await;
            let selftest = cx
                .background_executor()
                .spawn(async { diagnostics::resolve_selftest_binary() })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.pages.os_version = Some(version);
                this.pages.selftest_binary = Some(selftest);
                cx.notify();
                window.refresh();
            })
            .ok();
        })
        .detach();
    }

    fn export_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let suggested = format!(
            "frameever-logs-{}.txt",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        cx.spawn_in(window, async move |_, cx| {
            let Some(path) = crate::platform::save_file_panel_async(&suggested, &["txt"], cx).await
            else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { diagnostics::export_logs(&path).map(|()| path) })
                .await;
            match result {
                Ok(path) => crate::library::reveal_in_folder(&path),
                Err(error) => crate::platform::alert_dialog(t!("Could not export logs"), &error),
            }
        })
        .detach();
    }

    fn diagnostic_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.pages.diagnostic, DiagnosticStatus::Running { .. }) {
            return;
        }

        // The test starts its own recording and flashes the screen, which
        // would land in whatever the user is currently recording.
        if crate::session::RecordingSession::recording_in_flight(cx) {
            self.pages.diagnostic = DiagnosticStatus::Error(
                t!("Stop the current recording before running a diagnostic.").to_string(),
            );
            cx.notify();
            return;
        }

        // A stray click costs a couple of minutes of hijacked screen and loud
        // beeps, so it is confirmed the way the Tauri app confirms it.
        let message = t!(
            "FrameEver will take over your screen with a flashing pattern and play loud beeps for \
             about {seconds} seconds per pipeline. Take your headphones off, leave the volume \
             audible, and leave the machine alone until it finishes.",
            seconds = diagnostics::DEFAULT_DURATION_SECS
        );
        cx.spawn_in(window, async move |this, cx| {
            if !crate::platform::confirm_dialog(
                t!("Run diagnostic?"),
                &message,
                t!("Run Diagnostic"),
                t!("Cancel"),
                true,
            ) {
                return;
            }
            this.update_in(cx, |this, window, cx| this.diagnostic_start(window, cx))
                .ok();
        })
        .detach();
    }

    fn diagnostic_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Process-global, not per-page: Settings builds a fresh state every
        // time it opens while the run is a detached task that outlives the
        // window, so `self.pages.diagnostic` alone cannot see the run started
        // by a previous Settings window.
        let guard = match diagnostics::RunGuard::acquire() {
            Ok(guard) => guard,
            Err(error) => {
                self.pages.diagnostic = DiagnosticStatus::Error(error);
                cx.notify();
                return;
            }
        };

        let options = diagnostics::SyncTestOptions {
            mode: self.pages.diagnostic_mode,
            microphone: self.pages.diagnostic_mic,
            duration_secs: diagnostics::DEFAULT_DURATION_SECS,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.pages.diagnostic_cancel = Some(cancel.clone());
        self.pages.diagnostic_cancelling = false;
        self.pages.diagnostic = DiagnosticStatus::Running {
            stage: diagnostics::START_STAGE.to_string(),
            mode: None,
        };
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let (stage_tx, stage_rx) = flume::unbounded::<diagnostics::Stage>();
            let run = cx.background_executor().spawn({
                let cancel = cancel.clone();
                async move {
                    // Released on every exit path below, and on the drop of the
                    // whole future if the run is ever abandoned.
                    let _guard = guard;
                    let stages = stage_tx.clone();
                    let (sync_test, sync_test_error) = match diagnostics::resolve_selftest_binary()
                    {
                        Some(binary) => {
                            match diagnostics::run_sync_test(&binary, &options, &cancel, |stage| {
                                let _ = stages.send(stage);
                            }) {
                                Ok(report) => (Some(report), None),
                                Err(error) => (None, Some(error)),
                            }
                        }
                        // No sidecar: the environment half of the report is
                        // still worth having, with the reason recorded in it.
                        None => (
                            None,
                            Some(
                                t!("The FrameEver command-line tool was not found next to this build, so \
                                 the sync test could not run.")
                                    .to_string(),
                            ),
                        ),
                    };
                    if cancel.load(Ordering::Relaxed) {
                        return Err(diagnostics::CANCELLED.to_string());
                    }

                    let _ = stage_tx.send(diagnostics::Stage {
                        stage: diagnostics::COLLECT_STAGE.to_string(),
                        mode: None,
                    });
                    let report = diagnostics::collect_report(sync_test, sync_test_error.clone());
                    // `collect_report` blocks for tens of seconds and cannot be
                    // interrupted, so the flag is read again on the far side of
                    // it: a cancelled run must not still write a report.
                    if cancel.load(Ordering::Relaxed) {
                        return Err(diagnostics::CANCELLED.to_string());
                    }
                    let path = diagnostics::write_report(&report)
                        .map_err(|error| tracing::warn!("writing the diagnostic report: {error}"))
                        .ok();
                    let sync = report.get("syncTest").cloned().unwrap_or(Value::Null);
                    Ok((
                        sync.get("verdict")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        sync.get("summary")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        path,
                        sync_test_error,
                    ))
                }
            });

            // Poll the stage channel until the run drops its sender, the same
            // shape `editor_export` uses; only the newest stage is worth a
            // repaint.
            loop {
                let mut latest = None;
                while let Ok(stage) = stage_rx.try_recv() {
                    latest = Some(stage);
                }
                if let Some(stage) = latest {
                    this.update_in(cx, |this, window, cx| {
                        // A stage still in flight when Cancel was pressed must
                        // not walk the label back off "Cancelling".
                        if this.pages.diagnostic_cancelling
                            || this.pages.diagnostic_cancel.is_none()
                        {
                            return;
                        }
                        this.pages.diagnostic = DiagnosticStatus::Running {
                            stage: stage.stage,
                            mode: stage.mode,
                        };
                        cx.notify();
                        window.refresh();
                    })
                    .ok();
                }
                if stage_rx.is_disconnected() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }

            let outcome = run.await;
            this.update_in(cx, |this, window, cx| {
                this.pages.diagnostic_cancel = None;
                this.pages.diagnostic_cancelling = false;
                this.pages.diagnostic = match outcome {
                    Ok((verdict, summary, report_path, sync_test_error)) => {
                        DiagnosticStatus::Done {
                            verdict,
                            summary,
                            report_path,
                            sync_test_error,
                        }
                    }
                    Err(error) if error == diagnostics::CANCELLED => DiagnosticStatus::Idle,
                    Err(error) => DiagnosticStatus::Error(error),
                };
                cx.notify();
                window.refresh();
            })
            .ok();
        })
        .detach();
    }

    fn diagnostic_stop(&mut self, cx: &mut Context<Self>) {
        // The flag stays in place rather than being taken: the run reads it
        // again after `collect_report`, which is the whole second half of a
        // run, and a taken flag left that half uncancellable.
        let Some(cancel) = self.pages.diagnostic_cancel.clone() else {
            return;
        };
        cancel.store(true, Ordering::Relaxed);
        self.pages.diagnostic_cancelling = true;
        if let DiagnosticStatus::Running { stage, .. } = &mut self.pages.diagnostic {
            diagnostics::CANCEL_STAGE.clone_into(stage);
        }
        cx.notify();
    }

    fn render_diagnostic(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let running = matches!(self.pages.diagnostic, DiagnosticStatus::Running { .. });

        let warning = self
            .note_box()
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(theme.settings_text())
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!("The test takes over your screen and makes noise.")),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(theme.settings_muted())
                    .child(t!(
                        "A fullscreen window flashes black and white while 1kHz beeps play \
                         through your speakers, and FrameEver records it and measures how far the \
                         picture and the sound have drifted apart. Leave the machine alone \
                         while it runs. Both modes together take a couple of minutes."
                    )),
            );

        let mode = self.pages.diagnostic_mode;
        let mode_control = ui::SegmentedControl::settings(
            &theme,
            "diagnostic-mode",
            diagnostics::SyncMode::ALL
                .iter()
                .map(|value| {
                    ui::SegmentOption::new(value.label(), *value == mode).disabled(running)
                })
                .collect(),
        )
        .on_select(cx.listener(|this, index: &usize, _window, cx| {
            if let Some(value) = ui::option_at(diagnostics::SyncMode::ALL, *index) {
                this.pages.diagnostic_mode = value;
                cx.notify();
            }
        }));

        let options = self.rows(vec![
            self.setting_row(
                t!("Recording mode"),
                Some(t!(
                    "Which pipeline to test. Both runs the studio leg and then the instant one."
                )),
                mode_control.into_any_element(),
            ),
            self.setting_row(
                t!("Test microphone"),
                Some(t!(
                    "Also record your microphone and check that it hears the beeps in time. \
                     Needs speakers the mic can actually hear."
                )),
                self.toggle(
                    "diagnostic-mic",
                    self.pages.diagnostic_mic,
                    cx,
                    |this, cx| {
                        this.pages.diagnostic_mic = !this.pages.diagnostic_mic;
                        cx.notify();
                    },
                )
                .into_any_element(),
            ),
        ]);

        let mut body = div().flex().flex_col().gap(px(8.));
        match &self.pages.diagnostic {
            DiagnosticStatus::Idle => {
                body = body.child(
                    div().child(
                        ui::Button::settings(
                            &theme,
                            "diagnostic-run",
                            ui::ButtonVariant::Dark,
                            ui::ButtonSize::Md,
                        )
                        .label(t!("Run Diagnostic"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.diagnostic_run(window, cx)),
                        ),
                    ),
                );
                // Resolved once when the page opens; a build with no sidecar
                // still produces the environment half of the report.
                if let Some(None) = &self.pages.selftest_binary {
                    body = body.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.settings_muted())
                            .child(
                                t!("The FrameEver command-line tool was not found next to this build, so \
                                 the report will cover your environment only."),
                            ),
                    );
                }
            }
            DiagnosticStatus::Running { stage, mode } => {
                body = body
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.settings_text())
                            .child(diagnostics::stage_label(stage, mode.as_deref())),
                    )
                    .child(
                        div().child(
                            ui::Button::settings(
                                &theme,
                                "diagnostic-cancel",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Md,
                            )
                            .label(t!("Cancel"))
                            .disabled_settings(
                                &theme,
                                self.pages.diagnostic_cancel.is_none()
                                    || self.pages.diagnostic_cancelling,
                            )
                            .on_click(cx.listener(|this, _, _window, cx| this.diagnostic_stop(cx))),
                        ),
                    );
            }
            DiagnosticStatus::Done {
                verdict,
                summary,
                report_path,
                sync_test_error,
            } => {
                body = body.child(self.verdict_chip(verdict.as_deref()));
                if let Some(summary) = summary {
                    body = body.child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(theme.settings_text())
                            .child(summary.clone()),
                    );
                }
                if let Some(error) = sync_test_error {
                    body = body.child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .text_color(Hsla::from(theme.red_9))
                            .child(error.clone()),
                    );
                }

                let mut buttons = div().flex().flex_row().gap(px(8.));
                if let Some(path) = report_path {
                    let path = path.clone();
                    buttons = buttons.child(
                        ui::Button::settings(
                            &theme,
                            "diagnostic-reveal",
                            ui::ButtonVariant::Gray,
                            ui::ButtonSize::Md,
                        )
                        .label(t!("Show File"))
                        .on_click(move |_, _, _| crate::library::reveal_in_folder(&path)),
                    );
                }
                buttons = buttons.child(
                    ui::Button::settings(
                        &theme,
                        "diagnostic-rerun",
                        ui::ButtonVariant::Gray,
                        ui::ButtonSize::Md,
                    )
                    .label(t!("Run Again"))
                    .on_click(cx.listener(|this, _, window, cx| this.diagnostic_run(window, cx))),
                );
                body = body.child(buttons);
            }
            DiagnosticStatus::Error(error) => {
                body = body
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(Hsla::from(theme.red_9))
                            .child(error.clone()),
                    )
                    .child(
                        div().child(
                            ui::Button::settings(
                                &theme,
                                "diagnostic-retry",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Md,
                            )
                            .label(t!("Try Again"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.diagnostic_run(window, cx)),
                            ),
                        ),
                    );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(warning)
            .child(options)
            .child(body)
            .into_any_element()
    }

    /// PASS / WARN / FAIL / INCONCLUSIVE, in the same pill shape the System
    /// Information block's capture-support chip uses.
    fn verdict_chip(&self, verdict: Option<&str>) -> gpui::AnyElement {
        let theme = self.theme;
        // The same green/red pair the capture-support chip below uses, plus an
        // amber for `warn`; `inconclusive` and "no sync test at all" are not
        // failures, so they stay in the muted grey.
        let (label, fill, text): (&str, Hsla, Hsla) = match verdict {
            Some("pass") => (
                t!("PASS"),
                gpui::rgb(0x22c55e).into(),
                gpui::rgb(0x4ade80).into(),
            ),
            Some("warn") => (
                t!("WARN"),
                gpui::rgb(0xf59e0b).into(),
                gpui::rgb(0xfbbf24).into(),
            ),
            Some("fail") => (t!("FAIL"), theme.red_9.into(), theme.red_9.into()),
            Some("inconclusive") => (
                t!("INCONCLUSIVE"),
                theme.settings_muted(),
                theme.settings_muted(),
            ),
            // No sync test in the report: the environment half still ran.
            _ => (
                t!("ENVIRONMENT ONLY"),
                theme.settings_muted(),
                theme.settings_muted(),
            ),
        };
        let mut fill = fill;
        fill.a = 0.2;
        div()
            .flex()
            .flex_row()
            .child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .bg(fill)
                    .text_color(text)
                    .child(label),
            )
            .into_any_element()
    }

    pub(crate) fn render_feedback(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;

        let open_logs = div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .child(
                ui::Button::settings(
                    &theme,
                    "diagnostics-export-logs",
                    ui::ButtonVariant::Dark,
                    ui::ButtonSize::Md,
                )
                .label(t!("Export Logs…"))
                .on_click(cx.listener(|this, _, window, cx| this.export_logs(window, cx))),
            )
            .child(
                ui::Button::settings(
                    &theme,
                    "diagnostics-open-logs",
                    ui::ButtonVariant::Gray,
                    ui::ButtonSize::Md,
                )
                .label(t!("Open Logs Folder"))
                .on_click(|_, _, _| crate::library::open_path(&diagnostics::logs_dir())),
            );

        let report_issue = div().child(
            ui::Button::settings(
                &theme,
                "diagnostics-report-issue",
                ui::ButtonVariant::Gray,
                ui::ButtonSize::Md,
            )
            .label(t!("Report an Issue on GitHub"))
            .on_click(|_, _, cx| cx.open_url(&diagnostics::issue_report_url())),
        );

        let system_info: gpui::AnyElement = match &self.pages.os_version {
            None => div()
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(theme.settings_muted())
                .child(t!("Loading system information..."))
                .into_any_element(),
            Some(version) => {
                let info_block = |label: &'static str, value: String| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.settings_muted())
                                .child(label),
                        )
                        .child(
                            div()
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .bg(theme.settings_card_bg())
                                .font_family(MONO_FONT)
                                .text_size(px(12.))
                                .text_color(theme.settings_muted())
                                .child(value),
                        )
                };
                // ScreenCaptureKit / Windows Graphics Capture are both a given
                // on the OS versions this app runs on.
                let capture_supported = cfg!(any(target_os = "macos", target_os = "windows"));
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .children(
                        version
                            .clone()
                            .map(|version| info_block(t!("Operating System"), version)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.settings_muted())
                                    .child(t!("Capture Support")),
                            )
                            .child(
                                div().flex().flex_row().child(
                                    div()
                                        .px(px(8.))
                                        .py(px(4.))
                                        .rounded(px(4.))
                                        .text_size(px(12.))
                                        .bg(Theme::with_alpha(
                                            if capture_supported {
                                                gpui::rgb(0x22c55e)
                                            } else {
                                                theme.red_9
                                            },
                                            0.2,
                                        ))
                                        .text_color(Hsla::from(if capture_supported {
                                            gpui::rgb(0x4ade80)
                                        } else {
                                            theme.red_9
                                        }))
                                        .child(t!(
                                            "Screen Capture: {support}",
                                            support = if capture_supported {
                                                t!("Supported")
                                            } else {
                                                t!("Not Supported")
                                            }
                                        )),
                                ),
                            ),
                    )
                    .child(info_block(
                        t!("App Version"),
                        format!("v{}", env!("CARGO_PKG_VERSION")),
                    ))
                    .into_any_element()
            }
        };

        vec![
            self.section(
                t!("Diagnostic Report"),
                Some(
                    t!("Records a short flashing/beeping test pattern, measures how far the picture and sound drift apart, and snapshots your displays, cameras, microphones, disk space and recording settings. The report is saved as a file on this computer and is never sent anywhere."),
                ),
                None,
                vec![self.render_diagnostic(cx)],
            )
            .into_any_element(),
            self.section(
                t!("Logs"),
                Some(
                    t!("Logs stay on this computer. Export them to a file or open the folder, review them, and attach them to a bug report yourself."),
                ),
                None,
                vec![open_logs.into_any_element()],
            )
            .into_any_element(),
            self.section(
                t!("Report an Issue"),
                Some(
                    t!("Opens a new GitHub issue in your browser with a template and the app version. Nothing else is filled in, and nothing is sent until you submit it."),
                ),
                None,
                vec![report_issue.into_any_element()],
            )
            .into_any_element(),
            self.section(t!("System Information"), None, None, vec![system_info])
                .into_any_element(),
        ]
    }
}

/// Best-effort `SystemDiagnostics.macosVersion.displayName`.
fn os_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let read = |arg: &str| {
            std::process::Command::new("sw_vers")
                .arg(arg)
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let version = read("-productVersion")?;
        Some(match read("-buildVersion") {
            Some(build) => format!("macOS {version} ({build})"),
            None => format!("macOS {version}"),
        })
    }
    #[cfg(target_os = "windows")]
    {
        None
    }
}

const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

struct ChangelogEntry {
    version: String,
    date: String,
    content: String,
}

fn parse_changelog(text: &str) -> Vec<ChangelogEntry> {
    let mut entries: Vec<ChangelogEntry> = Vec::new();
    for line in text.replace("\r\n", "\n").lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let (version, date) = match heading.split_once('—') {
                Some((version, date)) => (version, date),
                None => (heading, ""),
            };
            entries.push(ChangelogEntry {
                version: version.trim().trim_matches(['[', ']']).to_string(),
                date: date.trim().to_string(),
                content: String::new(),
            });
        } else if let Some(entry) = entries.last_mut()
            && line.trim() != "---"
        {
            entry.content.push_str(line);
            entry.content.push('\n');
        }
    }
    entries
}

struct MarkdownParagraph {
    text: String,
    heading: bool,
}

fn markdown_paragraphs(content: &str) -> Vec<MarkdownParagraph> {
    content
        .replace("\r\n", "\n")
        .split("\n\n")
        .flat_map(|block| block.split('\n'))
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (line, heading) = match line.trim_start_matches('#') {
                stripped if stripped.len() != line.len() => (stripped.trim_start(), true),
                _ => (line, false),
            };
            if line.starts_with("![") {
                return None;
            }
            let mut text = line.replace("**", "").replace('`', "");
            while let (Some(open), Some(mid)) = (text.find('['), text.find("](")) {
                let Some(close) = text[mid..].find(')').map(|offset| mid + offset) else {
                    break;
                };
                if open < mid {
                    let label = text[open + 1..mid].to_string();
                    text.replace_range(open..=close, &label);
                } else {
                    break;
                }
            }
            Some(MarkdownParagraph { text, heading })
        })
        .collect()
}

impl SettingsWindow {
    pub(crate) fn render_changelog(&self, _cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;
        let entries = parse_changelog(CHANGELOG);
        let last = entries.len().saturating_sub(1);
        vec![
            div()
                .flex()
                .flex_col()
                .gap(px(32.))
                .children(entries.into_iter().enumerate().map(|(index, entry)| {
                    div()
                        .flex()
                        .flex_col()
                        .when(index != last, |this| {
                            this.pb(px(32.))
                                .border_b_2()
                                .border_color(theme.settings_border())
                        })
                        .when(index == 0, |this| {
                            this.child(
                                div().flex().flex_row().mb(px(8.)).child(
                                    div()
                                        .px(px(8.))
                                        .py(px(4.))
                                        .rounded(px(6.))
                                        .bg(Hsla::from(theme.blue_9))
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(gpui::white())
                                        .child(t!("NEW")),
                                ),
                            )
                        })
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .mb(px(8.))
                                .child(t!("Version {version}", version = entry.version)),
                        )
                        .when(!entry.date.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_size(px(12.))
                                    .line_height(px(18.))
                                    .text_color(theme.settings_muted())
                                    .mb(px(16.))
                                    .child(entry.date.clone()),
                            )
                        })
                        .child(
                            div().flex().flex_col().gap(px(10.)).children(
                                markdown_paragraphs(&entry.content)
                                    .into_iter()
                                    .map(|paragraph| {
                                        div()
                                            .text_size(px(13.))
                                            .line_height(px(20.))
                                            .text_color(theme.settings_muted())
                                            .when(paragraph.heading, |this| {
                                                this.font_weight(FontWeight::BOLD)
                                                    .text_color(theme.settings_text())
                                            })
                                            .child(paragraph.text)
                                    }),
                            ),
                        )
                }))
                .into_any_element(),
        ]
    }
}

// ---------------------------------------------------------------------------
// Automations (automations.tsx)
// ---------------------------------------------------------------------------

const ALL_TRIGGERS: [Trigger; 6] = [
    Trigger::ScreenshotTaken,
    Trigger::StudioRecordingFinished,
    Trigger::InstantRecordingFinished,
    Trigger::RecordingStarted,
    Trigger::VideoImported,
    Trigger::RecordingDeleted,
];

/// The type half of [`Action`], for the type-select menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionType {
    CopyToClipboard,
    SaveToLocation,
    Export,
    Upload,
    RevealInFileManager,
    OpenFile,
    RecognizeTextToClipboard,
    Notify,
    OpenEditor,
    SkipEditor,
    ApplyPreset,
    RunCommand,
    Webhook,
    DeleteLocalFiles,
}

/// `ALL_ACTION_TYPES` order in automations.tsx.
const ALL_ACTION_TYPES: [ActionType; 12] = [
    ActionType::CopyToClipboard,
    ActionType::SaveToLocation,
    ActionType::Export,
    ActionType::RevealInFileManager,
    ActionType::OpenFile,
    ActionType::RecognizeTextToClipboard,
    ActionType::Notify,
    ActionType::OpenEditor,
    ActionType::SkipEditor,
    ActionType::ApplyPreset,
    ActionType::RunCommand,
    ActionType::DeleteLocalFiles,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConditionType {
    CaptureTargetIs,
    RecordingModeIs,
    DurationAtLeast,
    DurationAtMost,
    WindowTitleContains,
    OrganizationIs,
}

const ALL_CONDITION_TYPES: [ConditionType; 5] = [
    ConditionType::CaptureTargetIs,
    ConditionType::RecordingModeIs,
    ConditionType::DurationAtLeast,
    ConditionType::DurationAtMost,
    ConditionType::WindowTitleContains,
];

const CAPTURE_TARGETS: [(CaptureTargetKind, &str); 3] = [
    (CaptureTargetKind::Display, n!("Display")),
    (CaptureTargetKind::Window, n!("Window")),
    (CaptureTargetKind::Area, n!("Area")),
];

const RECORDING_MODES: [(AutomationRecordingMode, &str); 2] = [
    (AutomationRecordingMode::Studio, n!("Studio")),
    (AutomationRecordingMode::Instant, n!("Instant")),
];

const CLIPBOARD_SOURCES: [(ClipboardSource, &str); 2] = [
    (ClipboardSource::Raw, n!("Original capture")),
    (ClipboardSource::Rendered, n!("Edited / rendered")),
];

const EXPORT_FORMATS: [(ExportFormat, &str); 3] = [
    (ExportFormat::Mp4, "MP4"),
    (ExportFormat::Gif, "GIF"),
    (ExportFormat::Mov, "MOV"),
];

const FPS_PRESETS: [u32; 3] = [15, 30, 60];

const RESOLUTION_PRESETS: [(&str, &str, u32, u32); 4] = [
    ("720p", "720p", 1280, 720),
    ("1080p", "1080p", 1920, 1080),
    ("1440p", "1440p", 2560, 1440),
    ("4k", "4K", 3840, 2160),
];

const COMPRESSIONS: [(AutomationExportCompression, &str); 4] = [
    (AutomationExportCompression::Maximum, n!("Maximum")),
    (AutomationExportCompression::Social, n!("Social")),
    (AutomationExportCompression::Web, n!("Web")),
    (AutomationExportCompression::Potato, n!("Potato")),
];

/// `TRIGGER_LABELS` (`utils/automations.ts`).
fn trigger_label(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::ScreenshotTaken => t!("On screenshot taken"),
        Trigger::StudioRecordingFinished => t!("On studio recording finished"),
        Trigger::InstantRecordingFinished => t!("On instant recording finished"),
        Trigger::RecordingStarted => t!("On recording started"),
        Trigger::UploadCompleted => t!("On upload completed"),
        Trigger::VideoImported => t!("On video imported"),
        Trigger::RecordingDeleted => t!("On recording deleted"),
    }
}

/// `TRIGGER_PHRASE`.
fn trigger_phrase(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::ScreenshotTaken => t!("Screenshot taken"),
        Trigger::StudioRecordingFinished => t!("Studio recording ends"),
        Trigger::InstantRecordingFinished => t!("Instant recording ends"),
        Trigger::RecordingStarted => t!("Recording starts"),
        Trigger::UploadCompleted => t!("Upload completes"),
        Trigger::VideoImported => t!("Video imported"),
        Trigger::RecordingDeleted => t!("Recording deleted"),
    }
}

/// `TRIGGER_NOUN`.
fn trigger_noun(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::ScreenshotTaken => t!("Screenshot"),
        Trigger::StudioRecordingFinished => t!("Studio recording"),
        Trigger::InstantRecordingFinished => t!("Instant recording"),
        Trigger::RecordingStarted => t!("Recording start"),
        Trigger::UploadCompleted => t!("Upload"),
        Trigger::VideoImported => t!("Import"),
        Trigger::RecordingDeleted => t!("Deletion"),
    }
}

/// `TRIGGER_ICONS`.
fn trigger_icon(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::ScreenshotTaken => "icons/image.svg",
        Trigger::StudioRecordingFinished => "icons/clapperboard.svg",
        Trigger::InstantRecordingFinished => "icons/zap.svg",
        Trigger::RecordingStarted => "icons/play-circle.svg",
        Trigger::UploadCompleted => "icons/cloud-upload.svg",
        Trigger::VideoImported => "icons/import.svg",
        Trigger::RecordingDeleted => "icons/trash.svg",
    }
}

/// `ACTION_LABELS`.
fn action_label(kind: ActionType) -> &'static str {
    match kind {
        ActionType::CopyToClipboard => t!("Copy to clipboard"),
        ActionType::SaveToLocation => t!("Save to location"),
        ActionType::Export => t!("Export with profile"),
        ActionType::Upload => t!("Upload + copy link"),
        ActionType::RevealInFileManager => t!("Reveal in file manager"),
        ActionType::OpenFile => t!("Open file"),
        ActionType::RunCommand => t!("Run command"),
        ActionType::Webhook => t!("Send webhook"),
        ActionType::RecognizeTextToClipboard => t!("Recognize text (OCR) to clipboard"),
        ActionType::Notify => t!("Show notification"),
        ActionType::OpenEditor => t!("Open editor"),
        ActionType::SkipEditor => t!("Skip editor (headless)"),
        ActionType::ApplyPreset => t!("Apply editor preset"),
        ActionType::DeleteLocalFiles => t!("Delete local files"),
    }
}

/// `ACTION_SHORT`.
fn action_short(kind: ActionType) -> &'static str {
    match kind {
        ActionType::CopyToClipboard => t!("Copy to clipboard"),
        ActionType::SaveToLocation => t!("Save to folder"),
        ActionType::Export => t!("Export"),
        ActionType::Upload => t!("Upload & copy link"),
        ActionType::RevealInFileManager => t!("Reveal in file manager"),
        ActionType::OpenFile => t!("Open file"),
        ActionType::RecognizeTextToClipboard => t!("Copy text (OCR)"),
        ActionType::Notify => t!("Notify"),
        ActionType::OpenEditor => t!("Open editor"),
        ActionType::SkipEditor => t!("Skip editor"),
        ActionType::ApplyPreset => t!("Apply preset"),
        ActionType::RunCommand => t!("Run command"),
        ActionType::Webhook => t!("Send webhook"),
        ActionType::DeleteLocalFiles => t!("Delete local files"),
    }
}

/// `ACTION_NOUN`.
fn action_noun(kind: ActionType) -> &'static str {
    match kind {
        ActionType::CopyToClipboard => t!("Clipboard"),
        ActionType::SaveToLocation => t!("Folder"),
        ActionType::Export => t!("Export"),
        ActionType::Upload => t!("Upload"),
        ActionType::RevealInFileManager => t!("Reveal"),
        ActionType::OpenFile => t!("Open"),
        ActionType::RecognizeTextToClipboard => t!("Text"),
        ActionType::Notify => t!("Notify"),
        ActionType::OpenEditor => t!("Editor"),
        ActionType::SkipEditor => t!("Skip editor"),
        ActionType::ApplyPreset => t!("Preset"),
        ActionType::RunCommand => t!("Command"),
        ActionType::Webhook => t!("Webhook"),
        ActionType::DeleteLocalFiles => t!("Delete"),
    }
}

/// `CONDITION_LABELS`.
fn condition_label(kind: ConditionType) -> &'static str {
    match kind {
        ConditionType::CaptureTargetIs => t!("Capture target is"),
        ConditionType::RecordingModeIs => t!("Recording mode is"),
        ConditionType::DurationAtLeast => t!("Duration at least (seconds)"),
        ConditionType::DurationAtMost => t!("Duration at most (seconds)"),
        ConditionType::WindowTitleContains => t!("Window title contains"),
        ConditionType::OrganizationIs => t!("Organization is"),
    }
}

fn action_type_of(action: &Action) -> ActionType {
    match action {
        Action::CopyToClipboard { .. } => ActionType::CopyToClipboard,
        Action::SaveToLocation { .. } => ActionType::SaveToLocation,
        Action::Export { .. } => ActionType::Export,
        Action::Upload { .. } => ActionType::Upload,
        Action::RevealInFileManager => ActionType::RevealInFileManager,
        Action::OpenFile => ActionType::OpenFile,
        Action::RunCommand { .. } => ActionType::RunCommand,
        Action::Webhook { .. } => ActionType::Webhook,
        Action::RecognizeTextToClipboard => ActionType::RecognizeTextToClipboard,
        Action::Notify { .. } => ActionType::Notify,
        Action::OpenEditor => ActionType::OpenEditor,
        Action::SkipEditor => ActionType::SkipEditor,
        Action::ApplyPreset { .. } => ActionType::ApplyPreset,
        Action::DeleteLocalFiles => ActionType::DeleteLocalFiles,
    }
}

fn condition_type_of(condition: &Condition) -> ConditionType {
    match condition {
        Condition::CaptureTargetIs { .. } => ConditionType::CaptureTargetIs,
        Condition::RecordingModeIs { .. } => ConditionType::RecordingModeIs,
        Condition::DurationAtLeast { .. } => ConditionType::DurationAtLeast,
        Condition::DurationAtMost { .. } => ConditionType::DurationAtMost,
        Condition::WindowTitleContains { .. } => ConditionType::WindowTitleContains,
        Condition::OrganizationIs { .. } => ConditionType::OrganizationIs,
    }
}

/// `defaultActionForType`.
fn default_action(kind: ActionType) -> Action {
    match kind {
        ActionType::CopyToClipboard => Action::CopyToClipboard {
            source: ClipboardSource::Raw,
        },
        ActionType::SaveToLocation => Action::SaveToLocation {
            dir: String::new(),
            filename_template: None,
        },
        ActionType::Export => Action::Export {
            profile: ExportProfile {
                format: ExportFormat::Mp4,
                fps: 30,
                resolution_base: frameever_project::XY { x: 1920, y: 1080 },
                compression: Some(AutomationExportCompression::Web),
                preset_name: None,
            },
            destination: ExportDestination::ProjectFolder,
        },
        ActionType::Upload => Action::Upload {
            organization_id: None,
            copy_link: true,
            open_in_browser: false,
        },
        ActionType::RevealInFileManager => Action::RevealInFileManager,
        ActionType::OpenFile => Action::OpenFile,
        ActionType::RunCommand => Action::RunCommand {
            program: String::new(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            use_shell: false,
        },
        ActionType::Webhook => Action::Webhook {
            url: String::new(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body_template: None,
        },
        ActionType::RecognizeTextToClipboard => Action::RecognizeTextToClipboard,
        ActionType::Notify => Action::Notify {
            title_template: "FrameEver".to_string(),
            body_template: String::new(),
        },
        ActionType::OpenEditor => Action::OpenEditor,
        ActionType::SkipEditor => Action::SkipEditor,
        ActionType::ApplyPreset => Action::ApplyPreset {
            name: String::new(),
        },
        ActionType::DeleteLocalFiles => Action::DeleteLocalFiles,
    }
}

/// `defaultConditionForType`.
fn default_condition(kind: ConditionType) -> Condition {
    match kind {
        ConditionType::CaptureTargetIs => Condition::CaptureTargetIs {
            target: CaptureTargetKind::Window,
        },
        ConditionType::RecordingModeIs => Condition::RecordingModeIs {
            mode: AutomationRecordingMode::Studio,
        },
        ConditionType::DurationAtLeast => Condition::DurationAtLeast { secs: 5. },
        ConditionType::DurationAtMost => Condition::DurationAtMost { secs: 300. },
        ConditionType::WindowTitleContains => Condition::WindowTitleContains {
            pattern: String::new(),
        },
        ConditionType::OrganizationIs => Condition::OrganizationIs { id: String::new() },
    }
}

/// `createEmptyRule`.
fn create_empty_rule() -> AutomationRule {
    AutomationRule {
        id: store::new_uuid_v4(),
        name: String::new(),
        enabled: true,
        trigger: Trigger::ScreenshotTaken,
        match_mode: MatchMode::All,
        conditions: Vec::new(),
        actions: vec![Action::CopyToClipboard {
            source: ClipboardSource::Raw,
        }],
    }
}

/// The contextual data each trigger provides at runtime -- `TRIGGER_CONTEXT`
/// in `utils/automations.ts`, mirroring the Rust `TriggerContext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerContextField {
    CaptureTarget,
    WindowTitle,
    RecordingMode,
    Duration,
    ProjectPath,
    FilePath,
    ShareLink,
}

fn trigger_context(trigger: Trigger) -> &'static [TriggerContextField] {
    use TriggerContextField::*;
    match trigger {
        Trigger::ScreenshotTaken => &[CaptureTarget, WindowTitle, ProjectPath, FilePath],
        Trigger::StudioRecordingFinished => &[RecordingMode, Duration, ProjectPath],
        Trigger::InstantRecordingFinished => &[RecordingMode, ProjectPath, ShareLink],
        Trigger::RecordingStarted => &[],
        Trigger::UploadCompleted => &[ProjectPath, ShareLink],
        Trigger::VideoImported => &[ProjectPath],
        Trigger::RecordingDeleted => &[ProjectPath],
    }
}

/// `conditionAppliesToTrigger`.
fn condition_applies_to_trigger(kind: ConditionType, trigger: Trigger) -> bool {
    use TriggerContextField::*;
    let required = match kind {
        ConditionType::CaptureTargetIs => CaptureTarget,
        ConditionType::RecordingModeIs => RecordingMode,
        ConditionType::DurationAtLeast | ConditionType::DurationAtMost => Duration,
        ConditionType::WindowTitleContains => WindowTitle,
        // `organizationIs: null` -- it never matches any trigger today.
        ConditionType::OrganizationIs => return false,
    };
    trigger_context(trigger).contains(&required)
}

/// `actionAppliesToTrigger`.
fn action_applies_to_trigger(kind: ActionType, trigger: Trigger) -> bool {
    use TriggerContextField::*;
    if kind == ActionType::SkipEditor {
        return matches!(
            trigger,
            Trigger::ScreenshotTaken | Trigger::StudioRecordingFinished
        );
    }
    let required: &[TriggerContextField] = match kind {
        ActionType::CopyToClipboard
        | ActionType::SaveToLocation
        | ActionType::OpenFile
        | ActionType::RecognizeTextToClipboard => &[FilePath],
        ActionType::Export | ActionType::ApplyPreset | ActionType::DeleteLocalFiles => {
            &[ProjectPath]
        }
        ActionType::RevealInFileManager | ActionType::OpenEditor | ActionType::Upload => {
            &[FilePath, ProjectPath]
        }
        _ => return true,
    };
    let provided = trigger_context(trigger);
    required.iter().any(|field| provided.contains(field))
}

/// `DANGEROUS_ACTIONS`.
fn action_is_dangerous(kind: ActionType) -> bool {
    kind == ActionType::RunCommand
}

fn trigger_is_removed(trigger: Trigger) -> bool {
    trigger == Trigger::UploadCompleted
}

fn condition_is_removed(kind: ConditionType) -> bool {
    kind == ConditionType::OrganizationIs
}

fn action_is_removed(kind: ActionType) -> bool {
    matches!(kind, ActionType::Upload | ActionType::Webhook)
}

/// `ruleSummary`.
fn rule_summary(rule: &AutomationRule) -> String {
    let trigger = trigger_phrase(rule.trigger);
    if rule.actions.is_empty() {
        return t!("{trigger} → no actions yet", trigger = trigger);
    }
    let actions = rule
        .actions
        .iter()
        .map(|action| action_short(action_type_of(action)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{trigger} → {actions}")
}

/// `autoRuleName`.
fn auto_rule_name(rule: &AutomationRule) -> String {
    let trigger = trigger_noun(rule.trigger);
    match rule.actions.first() {
        Some(action) => format!("{trigger} → {}", action_noun(action_type_of(action))),
        None => t!("{trigger} automation", trigger = trigger),
    }
}

/// `ruleDisplayName`.
fn rule_display_name(rule: &AutomationRule) -> String {
    let name = rule.name.trim();
    if name.is_empty() {
        auto_rule_name(rule)
    } else {
        name.to_string()
    }
}

/// `resolutionValue()`: the preset matching the profile, defaulting to 1080p.
fn resolution_value(profile: &ExportProfile) -> &'static str {
    RESOLUTION_PRESETS
        .iter()
        .find(|(_, _, x, y)| *x == profile.resolution_base.x && *y == profile.resolution_base.y)
        .map(|(value, ..)| *value)
        .unwrap_or("1080p")
}

/// `value={c.secs}` on a number input: integers without the trailing `.0`.
fn format_secs(secs: f64) -> String {
    if secs.fract() == 0. && secs.abs() < 1e15 {
        format!("{}", secs as i64)
    } else {
        format!("{secs}")
    }
}

/// One of `TEMPLATES` -- id, name, description, icon, and the built rule.
struct Template {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    build: fn() -> AutomationRule,
}

fn template_rule(name: &str, trigger: Trigger, actions: Vec<Action>) -> AutomationRule {
    AutomationRule {
        id: store::new_uuid_v4(),
        name: name.to_string(),
        enabled: true,
        trigger,
        match_mode: MatchMode::All,
        conditions: Vec::new(),
        actions,
    }
}

const TEMPLATES: [Template; 5] = [
    Template {
        id: "copy-screenshot",
        name: n!("Auto-copy new screenshots to clipboard"),
        description: n!("Snap a screenshot and it's right there, ready to paste."),
        icon: "icons/copy.svg",
        build: || {
            template_rule(
                t!("Auto-copy new screenshots to clipboard"),
                Trigger::ScreenshotTaken,
                vec![Action::CopyToClipboard {
                    source: ClipboardSource::Raw,
                }],
            )
        },
    },
    Template {
        id: "ocr-screenshot",
        name: n!("Pull the text out of screenshots"),
        description: n!("FrameEver reads the text in your screenshot and copies it for you."),
        icon: "icons/scan-text.svg",
        build: || {
            template_rule(
                t!("Pull the text out of screenshots"),
                Trigger::ScreenshotTaken,
                vec![Action::RecognizeTextToClipboard],
            )
        },
    },
    Template {
        id: "save-screenshot",
        name: n!("Tuck screenshots into a folder"),
        description: n!("Send every new screenshot straight to a folder you pick."),
        icon: "icons/folder-down.svg",
        build: || {
            template_rule(
                t!("Tuck screenshots into a folder"),
                Trigger::ScreenshotTaken,
                vec![default_action(ActionType::SaveToLocation)],
            )
        },
    },
    Template {
        id: "reveal-screenshot",
        name: n!("Jump to each new screenshot"),
        description: n!("Pop open every screenshot in Finder the moment you take it."),
        icon: "icons/folder-open.svg",
        build: || {
            template_rule(
                t!("Jump to each new screenshot"),
                Trigger::ScreenshotTaken,
                vec![Action::RevealInFileManager],
            )
        },
    },
    Template {
        id: "export-studio",
        name: n!("Auto-export when you finish recording"),
        description: n!("Render an MP4 the second a Studio recording wraps up."),
        icon: "icons/film.svg",
        build: || {
            template_rule(
                t!("Auto-export when you finish recording"),
                Trigger::StudioRecordingFinished,
                vec![default_action(ActionType::Export)],
            )
        },
    },
];

impl SettingsWindow {
    fn rule_at(&self, index: usize) -> Option<&AutomationRule> {
        self.pages.automations.rules.get(index)
    }

    fn export_profile_at(&self, rule: usize, action: usize) -> Option<&ExportProfile> {
        match self.rule_at(rule).and_then(|rule| rule.actions.get(action)) {
            Some(Action::Export { profile, .. }) => Some(profile),
            _ => None,
        }
    }

    /// `mutate()`: apply, persist, and rebuild the expanded editor when the
    /// change was structural (field set changed).
    fn automation_mutate(
        &mut self,
        rule: usize,
        structural: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut AutomationRule),
    ) {
        let Some(slot) = self.pages.automations.rules.get_mut(rule) else {
            return;
        };
        mutate(slot);
        self.automations_persist();
        if structural {
            self.rebuild_rule_editor(window, cx);
        }
        cx.notify();
    }

    fn automations_persist(&mut self) {
        self.pages.compatibility_checks.clear();
        self.pages.automations_save_failed = !store::set_automations(&self.pages.automations);
        if self.pages.automations_save_failed {
            tracing::warn!("saving the automations store failed");
        }
    }

    /// `addRule`: append, persist, and expand.
    fn automation_add_rule(
        &mut self,
        rule: AutomationRule,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = rule.id.clone();
        self.pages.automations.rules.push(rule);
        self.automations_persist();
        self.pages.expanded_rule = Some(id);
        self.rebuild_rule_editor(window, cx);
        cx.notify();
    }

    fn automation_remove_rule(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.pages.automations.rules.len() {
            return;
        }
        let removed = self.pages.automations.rules.remove(index);
        self.automations_persist();
        if self.pages.expanded_rule.as_deref() == Some(removed.id.as_str()) {
            self.pages.expanded_rule = None;
            self.rebuild_rule_editor(window, cx);
        }
        cx.notify();
    }

    fn automation_test(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(rule) = self.pages.automations.rules.get(index) else {
            return;
        };
        self.pages.compatibility_checks.insert(rule.id.clone());
        cx.notify();
    }

    /// Build (or drop) the text-input entities behind the expanded rule's
    /// editor. Positional fields, so any structural change rebuilds.
    fn rebuild_rule_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rule_id) = self.pages.expanded_rule.clone() else {
            self.pages.rule_editor = None;
            return;
        };
        let Some(rule) = self
            .pages
            .automations
            .rules
            .iter()
            .find(|rule| rule.id == rule_id)
        else {
            self.pages.rule_editor = None;
            return;
        };

        let mut specs: Vec<(AutoField, String, String)> =
            vec![(AutoField::Name, rule.name.clone(), auto_rule_name(rule))];
        for (index, condition) in rule.conditions.iter().enumerate() {
            match condition {
                Condition::DurationAtLeast { secs } | Condition::DurationAtMost { secs } => {
                    specs.push((
                        AutoField::ConditionSecs(index),
                        format_secs(*secs),
                        String::new(),
                    ));
                }
                Condition::WindowTitleContains { pattern } => {
                    specs.push((
                        AutoField::ConditionPattern(index),
                        pattern.clone(),
                        t!("e.g. Slack").to_string(),
                    ));
                }
                _ => {}
            }
        }
        for (index, action) in rule.actions.iter().enumerate() {
            match action {
                Action::SaveToLocation {
                    dir,
                    filename_template,
                } => {
                    specs.push((
                        AutoField::ActionDir(index),
                        dir.clone(),
                        "/Users/you/Screenshots".to_string(),
                    ));
                    specs.push((
                        AutoField::ActionFilename(index),
                        filename_template.clone().unwrap_or_default(),
                        "{date}-{window}".to_string(),
                    ));
                }
                Action::Export { destination, .. } => {
                    let dir = match destination {
                        ExportDestination::ProjectFolder => String::new(),
                        ExportDestination::CustomPath { dir } => dir.clone(),
                    };
                    specs.push((
                        AutoField::ActionExportDir(index),
                        dir,
                        t!("Project folder").to_string(),
                    ));
                }
                Action::RunCommand { program, args, .. } => {
                    specs.push((
                        AutoField::ActionProgram(index),
                        program.clone(),
                        "/usr/local/bin/my-script".to_string(),
                    ));
                    specs.push((AutoField::ActionArgs(index), args.join(" "), String::new()));
                }
                Action::Notify {
                    title_template,
                    body_template,
                } => {
                    specs.push((
                        AutoField::ActionNotifyTitle(index),
                        title_template.clone(),
                        String::new(),
                    ));
                    specs.push((
                        AutoField::ActionNotifyBody(index),
                        body_template.clone(),
                        String::new(),
                    ));
                }
                _ => {}
            }
        }

        let mut fields = Vec::with_capacity(specs.len());
        let mut subscriptions = Vec::with_capacity(specs.len());
        for (field, text, placeholder) in specs {
            let input = cx.new(|cx| {
                let mut input = ui::TextInputState::single_line(window, cx);
                if !placeholder.is_empty() {
                    input.set_placeholder(placeholder);
                }
                input.set_text(text, cx);
                input
            });
            subscriptions.push(cx.subscribe(&input, move |this, input, event, cx| {
                this.rule_editor_event(field, input, event, cx)
            }));
            fields.push((field, input));
        }
        self.pages.rule_editor = Some(RuleEditor {
            rule_id,
            fields,
            _subscriptions: subscriptions,
        });
    }

    /// One editor field changed: apply it to the rule and persist -- every
    /// `onInput` in automations.tsx mutates and saves.
    fn rule_editor_event(
        &mut self,
        field: AutoField,
        input: Entity<ui::TextInputState>,
        event: &ui::TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        if *event != ui::TextInputEvent::Changed {
            return;
        }
        let text = input.read(cx).text().to_string();
        let Some(rule_id) = self
            .pages
            .rule_editor
            .as_ref()
            .map(|editor| editor.rule_id.clone())
        else {
            return;
        };
        let Some(rule) = self
            .pages
            .automations
            .rules
            .iter_mut()
            .find(|rule| rule.id == rule_id)
        else {
            return;
        };

        match field {
            AutoField::Name => rule.name = text,
            AutoField::ConditionSecs(index) => {
                // `Number(v) || 0`.
                let secs = text.trim().parse::<f64>().ok().filter(|v| v.is_finite());
                match rule.conditions.get_mut(index) {
                    Some(
                        Condition::DurationAtLeast { secs: slot }
                        | Condition::DurationAtMost { secs: slot },
                    ) => *slot = secs.unwrap_or(0.),
                    _ => return,
                }
            }
            AutoField::ConditionPattern(index) => match rule.conditions.get_mut(index) {
                Some(Condition::WindowTitleContains { pattern }) => *pattern = text,
                _ => return,
            },
            AutoField::ActionDir(index) => match rule.actions.get_mut(index) {
                Some(Action::SaveToLocation { dir, .. }) => *dir = text,
                _ => return,
            },
            AutoField::ActionFilename(index) => match rule.actions.get_mut(index) {
                Some(Action::SaveToLocation {
                    filename_template, ..
                }) => *filename_template = (!text.is_empty()).then_some(text),
                _ => return,
            },
            AutoField::ActionProgram(index) => match rule.actions.get_mut(index) {
                Some(Action::RunCommand { program, .. }) => *program = text,
                _ => return,
            },
            AutoField::ActionArgs(index) => match rule.actions.get_mut(index) {
                // `v.split(" ")` -- a plain space split, as shipped.
                Some(Action::RunCommand { args, .. }) => {
                    *args = if text.is_empty() {
                        Vec::new()
                    } else {
                        text.split(' ').map(str::to_string).collect()
                    }
                }
                _ => return,
            },
            AutoField::ActionNotifyTitle(index) => match rule.actions.get_mut(index) {
                Some(Action::Notify { title_template, .. }) => *title_template = text,
                _ => return,
            },
            AutoField::ActionNotifyBody(index) => match rule.actions.get_mut(index) {
                Some(Action::Notify { body_template, .. }) => *body_template = text,
                _ => return,
            },
            AutoField::ActionExportDir(index) => match rule.actions.get_mut(index) {
                Some(Action::Export { destination, .. }) => {
                    *destination = if text.is_empty() {
                        ExportDestination::ProjectFolder
                    } else {
                        ExportDestination::CustomPath { dir: text }
                    }
                }
                _ => return,
            },
        }
        self.automations_persist();
        cx.notify();
    }

    /// A directory picker for the two Browse buttons, writing both the rule
    /// and the editor field's text.
    fn automation_pick_dir(
        &mut self,
        rule: usize,
        field: AutoField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(dir) = paths.into_iter().next() else {
                return;
            };
            let dir = dir.to_string_lossy().to_string();
            this.update_in(cx, |this, window, cx| {
                let value = dir.clone();
                this.automation_mutate(rule, false, window, cx, move |rule| match field {
                    AutoField::ActionDir(index) => {
                        if let Some(Action::SaveToLocation { dir, .. }) =
                            rule.actions.get_mut(index)
                        {
                            *dir = value;
                        }
                    }
                    AutoField::ActionExportDir(index) => {
                        if let Some(Action::Export { destination, .. }) =
                            rule.actions.get_mut(index)
                        {
                            *destination = ExportDestination::CustomPath { dir: value };
                        }
                    }
                    _ => {}
                });
                if let Some(input) = this.rule_editor_input(field) {
                    input.update(cx, |input, cx| input.set_text(dir, cx));
                }
            })
            .ok();
        })
        .detach();
    }

    fn rule_editor_input(&self, field: AutoField) -> Option<Entity<ui::TextInputState>> {
        self.pages
            .rule_editor
            .as_ref()?
            .fields
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .map(|(_, input)| input.clone())
    }

    pub(crate) fn render_automations(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;
        let loaded = self.pages.automations_loaded;

        let rules_body: gpui::AnyElement = if !loaded {
            // The `Suspense` fallback.
            div()
                .h(px(96.))
                .rounded(px(10.))
                .bg(theme.settings_fill())
                .into_any_element()
        } else if self.pages.automations.rules.is_empty() {
            self.render_automations_empty(cx).into_any_element()
        } else {
            let mut children: Vec<gpui::AnyElement> = Vec::new();
            for index in 0..self.pages.automations.rules.len() {
                children.push(self.render_rule_card(index, cx));
            }
            children.push(self.render_add_rule_button(cx).into_any_element());
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .children(children)
                .into_any_element()
        };

        let mut rule_content = Vec::new();
        if self.pages.automations_save_failed {
            rule_content.push(
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(8.))
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(Hsla::from(theme.amber_11))
                    .child(t!(
                        "Failed to save automations. Your changes have not been saved."
                    ))
                    .child(
                        ui::Button::settings(
                            &theme,
                            "automations-retry-save",
                            ui::ButtonVariant::Gray,
                            ui::ButtonSize::Xs,
                        )
                        .label(t!("Retry save"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.automations_persist();
                            cx.notify();
                        })),
                    )
                    .into_any_element(),
            );
        }
        rule_content.push(rules_body);

        // `grid grid-cols-2 gap-2.5`, as rows of two.
        let mut template_rows: Vec<gpui::AnyElement> = Vec::new();
        for pair in TEMPLATES.chunks(2) {
            template_rows.push(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(10.))
                    .children(
                        pair.iter()
                            .map(|template| self.render_template_card(template, cx)),
                    )
                    .into_any_element(),
            );
        }
        let templates = div().flex().flex_col().gap(px(10.)).children(template_rows);

        vec![
            self.section(
                t!("Automations"),
                Some(
                    t!("Rules are shared with the FrameEver CLI. FrameEver saves these rules but does not \
                     run automations yet."),
                ),
                None,
                rule_content,
            )
            .into_any_element(),
            self.section(
                t!("Templates"),
                Some(t!("One click to add a ready-made automation. Tweak anything afterwards.")),
                None,
                vec![templates.into_any_element()],
            )
            .into_any_element(),
        ]
    }

    /// `<EmptyState>`.
    fn render_automations_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        self.card(true).child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .py(px(24.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(44.))
                        .mb(px(4.))
                        .rounded_full()
                        .bg(theme.settings_fill())
                        .child(
                            svg()
                                .path("icons/zap.svg")
                                .size(px(20.))
                                .text_color(theme.settings_muted()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(t!("No automations yet")),
                )
                .child(
                    div()
                        .max_w(px(320.))
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_center()
                        .text_color(theme.settings_muted())
                        .child(t!(
                            "Pick a template below to get started in one click, or build your \
                             own from scratch."
                        )),
                )
                .child(div().mt(px(4.)).child(self.button(
                    "automations-scratch",
                    (ui::ButtonVariant::Gray, Some("icons/plus.svg")),
                    t!("Start from scratch"),
                    false,
                    cx,
                    |this, window, cx| {
                        this.automation_add_rule(create_empty_rule(), window, cx);
                    },
                ))),
        )
    }

    /// `<AddRuleButton>`.
    fn render_add_rule_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id("automations-add")
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .py(px(10.))
            .w_full()
            .rounded(px(10.))
            .border_1()
            .border_dashed()
            .border_color(theme.settings_border())
            .text_size(px(13.))
            .text_color(theme.settings_muted())
            .cursor_pointer()
            .hover(|style| {
                style
                    .text_color(theme.settings_text())
                    .bg(theme.settings_card_bg())
            })
            .child(
                svg()
                    .path("icons/plus.svg")
                    .size(px(16.))
                    .text_color(theme.settings_muted()),
            )
            .child(t!("New automation"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.automation_add_rule(create_empty_rule(), window, cx);
            }))
    }

    /// `<TemplateCard>`.
    fn render_template_card(
        &self,
        template: &Template,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let build = template.build;
        let name = template.name;
        div()
            .id(SharedString::from(format!("template-{}", template.id)))
            .flex()
            .flex_row()
            .items_start()
            .flex_1()
            .min_w_0()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(theme.settings_border())
            .bg(theme.settings_card_bg())
            .cursor_pointer()
            .hover(|style| style.bg(theme.settings_fill()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .bg(theme.settings_fill())
                    .child(
                        svg()
                            .path(template.icon)
                            .size(px(18.))
                            .text_color(theme.settings_muted()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(tr(name)),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(11.))
                            .line_height(px(15.))
                            .text_color(theme.settings_muted())
                            .child(tr(template.description)),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                // `addFromTemplate` also toasts `Added "{name}"`; no toast
                // layer here, the expanded editor is the feedback.
                tracing::info!(template = name, "added an automation template");
                this.automation_add_rule(build(), window, cx);
            }))
            .into_any_element()
    }

    /// `<RuleCard>`.
    fn render_rule_card(&self, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let Some(rule) = self.pages.automations.rules.get(index) else {
            return div().into_any_element();
        };
        let expanded = self.pages.expanded_rule.as_deref() == Some(rule.id.as_str());
        let enabled = rule.enabled;
        let has_custom_name = !rule.name.trim().is_empty();

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .p(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .bg(theme.settings_fill())
                    .when(!enabled, |this| this.opacity(0.6))
                    .child(
                        svg()
                            .path(trigger_icon(rule.trigger))
                            .size(px(18.))
                            .text_color(theme.settings_muted()),
                    ),
            )
            .child(
                div()
                    .id(("rule-expand-label", index))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .cursor_pointer()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if enabled {
                                theme.settings_text()
                            } else {
                                theme.settings_muted()
                            })
                            .child(rule_display_name(rule)),
                    )
                    .when(has_custom_name, |this| {
                        this.child(
                            div()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(theme.settings_muted())
                                .child(rule_summary(rule)),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.automation_toggle_expand(index, window, cx);
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.settings_muted())
                            .child(match (enabled, self.pages.automations_save_failed) {
                                (true, false) => t!("Enabled · Saved"),
                                (false, false) => t!("Disabled · Saved"),
                                (true, true) => t!("Enabled · Not saved"),
                                (false, true) => t!("Disabled · Not saved"),
                            }),
                    )
                    .child(
                        self.toggle(("rule-enabled", index), enabled, cx, move |this, cx| {
                            if let Some(rule) = this.pages.automations.rules.get_mut(index) {
                                rule.enabled = !rule.enabled;
                                this.automations_persist();
                                cx.notify();
                            }
                        }),
                    ),
            )
            .child(
                div()
                    .id(("rule-expand", index))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.settings_fill()))
                    .child(
                        svg()
                            .path(if expanded {
                                "icons/chevron-up.svg"
                            } else {
                                "icons/chevron-down.svg"
                            })
                            .size(px(16.))
                            .text_color(theme.settings_muted()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.automation_toggle_expand(index, window, cx);
                    })),
            );

        let mut card = self.card(false).flex().flex_col().child(header);
        if expanded {
            card = card.child(
                div()
                    .border_t_1()
                    .border_color(theme.settings_border())
                    .child(self.render_rule_editor(index, cx)),
            );
        }
        card.into_any_element()
    }

    fn automation_toggle_expand(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(rule) = self.pages.automations.rules.get(index) else {
            return;
        };
        let id = rule.id.clone();
        if self.pages.expanded_rule.as_deref() == Some(id.as_str()) {
            self.pages.expanded_rule = None;
        } else {
            self.pages.expanded_rule = Some(id);
        }
        self.rebuild_rule_editor(window, cx);
        cx.notify();
    }

    /// The editor field for one [`AutoField`], if the rebuilt editor has it.
    fn render_editor_input(&self, field: AutoField) -> gpui::AnyElement {
        match self.rule_editor_input(field) {
            Some(input) => {
                let id = SharedString::from(format!("auto-field-{field:?}"));
                self.pages_input(id, &input).into_any_element()
            }
            None => div().into_any_element(),
        }
    }

    /// `<Field>`: an 11px label over the control.
    fn editor_field(&self, label: &'static str, control: gpui::AnyElement) -> gpui::AnyElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(4.))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.settings_muted())
                    .child(label),
            )
            .child(control)
            .into_any_element()
    }

    /// `<GroupLabel>`.
    fn group_label(&self, text: &'static str) -> impl IntoElement {
        div()
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.theme.settings_muted())
            .child(text.to_uppercase())
    }

    /// `<RuleEditorBody>`.
    fn render_rule_editor(&self, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let Some(rule) = self.pages.automations.rules.get(index) else {
            return div().into_any_element();
        };
        let trigger = rule.trigger;
        let has_dangerous = rule
            .actions
            .iter()
            .any(|action| action_is_dangerous(action_type_of(action)));
        let compatibility_checked = self.pages.compatibility_checks.contains(&rule.id);

        let name_field = self.editor_field(t!("Name"), self.render_editor_input(AutoField::Name));

        let trigger_block = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(self.group_label(t!("When this happens")))
            .child(
                self.pages_select(
                    ("rule-trigger", index),
                    trigger_label(trigger),
                    MenuKind::AutomationTrigger(index),
                    cx,
                )
                .stretch_label(),
            )
            .when(trigger_is_removed(trigger), |this| {
                this.child(
                    div()
                        .px(px(4.))
                        .text_size(px(11.))
                        .text_color(Hsla::from(theme.amber_11))
                        .child(t!(
                            "Uploads were removed, so this rule never runs. Choose another trigger."
                        )),
                )
            });

        // -- Conditions -----------------------------------------------------
        let mut conditions_header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(self.group_label(t!("Only run if")));
        let mut header_right = div().flex().flex_row().items_center().gap(px(8.));
        if rule.conditions.len() > 1 {
            let mode_label = match rule.match_mode {
                MatchMode::All => t!("Match all"),
                MatchMode::Any => t!("Match any"),
            };
            header_right = header_right.child(self.pages_select(
                ("rule-match-mode", index),
                mode_label,
                MenuKind::AutomationMatchMode(index),
                cx,
            ));
        }
        header_right = header_right.child(
            ui::Button::settings(
                &theme,
                ("rule-add-condition", index),
                ui::ButtonVariant::Gray,
                ui::ButtonSize::Xs,
            )
            .label(t!("Add condition"))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.automation_mutate(index, true, window, cx, |rule| {
                    // The first condition type that applies to the trigger,
                    // falling back to `captureTargetIs`.
                    let kind = ALL_CONDITION_TYPES
                        .iter()
                        .copied()
                        .find(|kind| condition_applies_to_trigger(*kind, rule.trigger))
                        .unwrap_or(ConditionType::CaptureTargetIs);
                    rule.conditions.push(default_condition(kind));
                });
            })),
        );
        conditions_header = conditions_header.child(header_right);

        let conditions_body: gpui::AnyElement = if rule.conditions.is_empty() {
            div()
                .text_size(px(12.))
                .text_color(theme.settings_muted())
                .child(t!(
                    "Runs for every {trigger}.",
                    trigger = trigger_phrase(trigger).to_lowercase()
                ))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(
                    rule.conditions
                        .iter()
                        .enumerate()
                        .map(|(ci, condition)| self.render_condition_row(index, ci, condition, cx)),
                )
                .into_any_element()
        };

        // -- Actions ----------------------------------------------------------
        let actions_header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(self.group_label(t!("Then do this")))
            .child(
                ui::Button::settings(
                    &theme,
                    ("rule-add-action", index),
                    ui::ButtonVariant::Gray,
                    ui::ButtonSize::Xs,
                )
                .label(t!("Add action"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.automation_mutate(index, true, window, cx, |rule| {
                        // `copyToClipboard` when it applies, else the first
                        // applicable type, else `notify`.
                        let kind =
                            if action_applies_to_trigger(ActionType::CopyToClipboard, rule.trigger)
                            {
                                ActionType::CopyToClipboard
                            } else {
                                ALL_ACTION_TYPES
                                    .iter()
                                    .copied()
                                    .find(|kind| action_applies_to_trigger(*kind, rule.trigger))
                                    .unwrap_or(ActionType::Notify)
                            };
                        rule.actions.push(default_action(kind));
                    });
                })),
            );

        let action_count = rule.actions.len();
        let actions_body =
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(rule.actions.iter().enumerate().map(|(ai, action)| {
                    self.render_action_row(index, ai, action, trigger, action_count, cx)
                }));

        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pt(px(16.))
            .mt(px(8.))
            .border_t_1()
            .border_color(theme.settings_border())
            .child(
                ui::Button::settings(
                    &theme,
                    ("rule-test", index),
                    ui::ButtonVariant::Gray,
                    ui::ButtonSize::Xs,
                )
                .label(t!("Check compatibility"))
                .on_click(cx.listener(move |this, _, _window, cx| this.automation_test(index, cx))),
            )
            .child(
                div()
                    .id(("rule-delete", index))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .h(px(24.))
                    .px(px(8.))
                    .rounded(px(8.))
                    .text_size(px(12.))
                    .text_color(theme.settings_muted())
                    .cursor_pointer()
                    .hover(|style| {
                        style
                            .text_color(Hsla::from(rgb(0xef4444)))
                            .bg(Theme::with_alpha(rgb(0xef4444), 0.1))
                    })
                    .child(
                        svg()
                            .path("icons/trash.svg")
                            .size(px(14.))
                            .text_color(theme.settings_muted()),
                    )
                    .child(t!("Delete"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.automation_remove_rule(index, window, cx);
                    })),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(16.))
            .child(name_field)
            .child(trigger_block)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(conditions_header)
                    .child(conditions_body),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(actions_header)
                    .child(actions_body),
            )
            .when(has_dangerous, |this| {
                this.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(Hsla::from(theme.amber_11))
                        .child(
                            t!("In the FrameEver CLI, this automation runs commands with your \
                             permissions. Only use values you trust."),
                        ),
                )
            })
            .when(compatibility_checked, |this| {
                this.child(
                    div()
                        .id(("rule-compatibility-result", index))
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(Hsla::from(theme.amber_11))
                        .child(
                            t!("Compatibility checked: this automation doesn't run in FrameEver yet. \
                             Saved rules remain available to the FrameEver CLI. No actions were run."),
                        ),
                )
            })
            .child(footer)
            .into_any_element()
    }

    /// `<ConditionRow>`.
    fn render_condition_row(
        &self,
        rule: usize,
        index: usize,
        condition: &Condition,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let kind = condition_type_of(condition);
        let trigger = self
            .rule_at(rule)
            .map(|rule| rule.trigger)
            .unwrap_or(Trigger::ScreenshotTaken);
        let applies = condition_applies_to_trigger(kind, trigger);

        let value: gpui::AnyElement = match condition {
            Condition::CaptureTargetIs { target } => {
                let label = CAPTURE_TARGETS
                    .iter()
                    .find(|(candidate, _)| candidate == target)
                    .map(|(_, label)| tr(label))
                    .unwrap_or(t!("Window"));
                self.pages_select(
                    SharedString::from(format!("condition-target-{rule}-{index}")),
                    label,
                    MenuKind::AutomationConditionTarget(rule, index),
                    cx,
                )
                .stretch_label()
                .into_any_element()
            }
            Condition::RecordingModeIs { mode } => {
                let label = RECORDING_MODES
                    .iter()
                    .find(|(candidate, _)| candidate == mode)
                    .map(|(_, label)| tr(label))
                    .unwrap_or(t!("Studio"));
                self.pages_select(
                    SharedString::from(format!("condition-mode-{rule}-{index}")),
                    label,
                    MenuKind::AutomationConditionMode(rule, index),
                    cx,
                )
                .stretch_label()
                .into_any_element()
            }
            Condition::DurationAtLeast { .. } | Condition::DurationAtMost { .. } => self
                .render_editor_input(AutoField::ConditionSecs(index))
                .into_any_element(),
            Condition::WindowTitleContains { .. } => self
                .render_editor_input(AutoField::ConditionPattern(index))
                .into_any_element(),
            Condition::OrganizationIs { .. } => div().into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(8.))
                    .p(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(theme.settings_border())
                    .bg(theme.settings_card_bg())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_1()
                            .min_w_0()
                            .gap(px(8.))
                            .child(
                                div().flex_1().min_w_0().child(
                                    self.pages_select(
                                        SharedString::from(format!(
                                            "condition-type-{rule}-{index}"
                                        )),
                                        condition_label(kind),
                                        MenuKind::AutomationConditionType(rule, index),
                                        cx,
                                    )
                                    .stretch_label(),
                                ),
                            )
                            .child(div().flex_1().min_w_0().child(value)),
                    )
                    .child(self.render_row_icon_button(
                        SharedString::from(format!("condition-remove-{rule}-{index}")),
                        "icons/x.svg",
                        false,
                        cx,
                        move |this, window, cx| {
                            this.automation_mutate(rule, true, window, cx, move |rule| {
                                if index < rule.conditions.len() {
                                    rule.conditions.remove(index);
                                }
                            });
                        },
                    )),
            )
            .when(!applies, |this| {
                this.child(
                    div()
                        .px(px(4.))
                        .text_size(px(11.))
                        .text_color(Hsla::from(theme.amber_11))
                        .child(if condition_is_removed(kind) {
                            t!("Organizations were removed, so this condition never matches. Remove it.")
                        } else {
                            t!("This condition never matches for the selected trigger.")
                        }),
                )
            })
            .into_any_element()
    }

    /// `<RowButton>`: the 28px icon buttons on condition/action rows.
    fn render_row_icon_button(
        &self,
        id: SharedString,
        icon: &'static str,
        disabled: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .flex_shrink_0()
            .rounded(px(8.))
            .when(disabled, |this| this.opacity(0.4))
            .child(
                svg()
                    .path(icon)
                    .size(px(16.))
                    .text_color(theme.settings_muted()),
            )
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(theme.settings_fill()))
                    .on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)))
            })
            .into_any_element()
    }

    /// `<ActionRow>`.
    fn render_action_row(
        &self,
        rule: usize,
        index: usize,
        action: &Action,
        trigger: Trigger,
        action_count: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let kind = action_type_of(action);
        let applies = action_applies_to_trigger(kind, trigger);

        let header = div().flex().flex_row().items_center().gap(px(8.)).child(
            div().flex_1().min_w_0().child(
                self.pages_select(
                    SharedString::from(format!("action-type-{rule}-{index}")),
                    action_label(kind),
                    MenuKind::AutomationActionType(rule, index),
                    cx,
                )
                .stretch_label(),
            ),
        );
        let header = header
            .child(self.render_row_icon_button(
                SharedString::from(format!("action-up-{rule}-{index}")),
                "icons/chevron-up.svg",
                index == 0,
                cx,
                move |this, window, cx| {
                    this.automation_mutate(rule, true, window, cx, move |rule| {
                        if index > 0 && index < rule.actions.len() {
                            rule.actions.swap(index, index - 1);
                        }
                    });
                },
            ))
            .child(self.render_row_icon_button(
                SharedString::from(format!("action-down-{rule}-{index}")),
                "icons/chevron-down.svg",
                index + 1 == action_count,
                cx,
                move |this, window, cx| {
                    this.automation_mutate(rule, true, window, cx, move |rule| {
                        if index + 1 < rule.actions.len() {
                            rule.actions.swap(index, index + 1);
                        }
                    });
                },
            ))
            .child(self.render_row_icon_button(
                SharedString::from(format!("action-remove-{rule}-{index}")),
                "icons/x.svg",
                false,
                cx,
                move |this, window, cx| {
                    this.automation_mutate(rule, true, window, cx, move |rule| {
                        if index < rule.actions.len() {
                            rule.actions.remove(index);
                        }
                    });
                },
            ));

        let params = self.render_action_params(rule, index, action, cx);

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(theme.settings_border())
            .bg(theme.settings_card_bg())
            .child(header)
            .children(params)
            .map(|this| {
                let notice = if action_is_removed(kind) {
                    Some(t!(
                        "Uploads and webhooks were removed, so this action is skipped. Remove it or choose another action."
                    ))
                } else if !applies {
                    Some(t!("This action has no effect for the selected trigger."))
                } else {
                    None
                };
                this.children(notice.map(|notice| {
                    div()
                        .text_size(px(11.))
                        .text_color(Hsla::from(theme.amber_11))
                        .child(notice)
                }))
            })
            .into_any_element()
    }

    /// `<ActionParams>`: the per-type body of an action row.
    fn render_action_params(
        &self,
        rule: usize,
        index: usize,
        action: &Action,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let theme = self.theme;
        match action {
            Action::CopyToClipboard { source } => {
                let label = CLIPBOARD_SOURCES
                    .iter()
                    .find(|(candidate, _)| candidate == source)
                    .map(|(_, label)| tr(label))
                    .unwrap_or(t!("Original capture"));
                Some(
                    self.editor_field(
                        t!("Source"),
                        self.pages_select(
                            SharedString::from(format!("action-source-{rule}-{index}")),
                            label,
                            MenuKind::AutomationClipboardSource(rule, index),
                            cx,
                        )
                        .stretch_label()
                        .into_any_element(),
                    ),
                )
            }
            Action::SaveToLocation { .. } => {
                Some(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .child(
                            self.editor_field(
                                t!("Folder"),
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(8.))
                                    .child(div().flex_1().min_w_0().child(
                                        self.render_editor_input(AutoField::ActionDir(index)),
                                    ))
                                    .child(self.button(
                                        SharedString::from(format!("action-browse-{rule}-{index}")),
                                        (ui::ButtonVariant::Gray, None),
                                        t!("Browse"),
                                        false,
                                        cx,
                                        move |this, window, cx| {
                                            this.automation_pick_dir(
                                                rule,
                                                AutoField::ActionDir(index),
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .into_any_element(),
                            ),
                        )
                        .child(self.editor_field(
                            t!("Filename template (optional)"),
                            self.render_editor_input(AutoField::ActionFilename(index)),
                        ))
                        .into_any_element(),
                )
            }
            Action::Export { profile, .. } => {
                Some(self.render_export_params(rule, index, profile, cx))
            }
            Action::RunCommand { use_shell, .. } => {
                let use_shell = *use_shell;
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.))
                                .child(self.editor_field(
                                    t!("Program"),
                                    self.render_editor_input(AutoField::ActionProgram(index)),
                                ))
                                .child(self.editor_field(
                                    t!("Arguments (space-separated)"),
                                    self.render_editor_input(AutoField::ActionArgs(index)),
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .child(self.toggle(
                                    SharedString::from(format!("action-shell-{rule}-{index}")),
                                    use_shell,
                                    cx,
                                    move |this, cx| {
                                        if let Some(Action::RunCommand { use_shell, .. }) = this
                                            .pages
                                            .automations
                                            .rules
                                            .get_mut(rule)
                                            .and_then(|rule| rule.actions.get_mut(index))
                                        {
                                            *use_shell = !*use_shell;
                                            this.automations_persist();
                                            cx.notify();
                                        }
                                    },
                                ))
                                .child(div().text_size(px(13.)).child(t!("Run through shell"))),
                        )
                        .into_any_element(),
                )
            }
            Action::Notify { .. } => Some(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(self.editor_field(
                        t!("Title"),
                        self.render_editor_input(AutoField::ActionNotifyTitle(index)),
                    ))
                    .child(self.editor_field(
                        t!("Body"),
                        self.render_editor_input(AutoField::ActionNotifyBody(index)),
                    ))
                    .into_any_element(),
            ),
            Action::ApplyPreset { name } => {
                let names = store::preset_names();
                if names.is_empty() {
                    return Some(
                        div()
                            .px(px(2.))
                            .py(px(6.))
                            .text_size(px(11.))
                            .text_color(theme.settings_muted())
                            .child(t!("No presets yet — create one in the editor first."))
                            .into_any_element(),
                    );
                }
                let label = if name.is_empty() {
                    SharedString::from(names[0].clone())
                } else {
                    SharedString::from(name.clone())
                };
                Some(
                    self.editor_field(
                        t!("Preset"),
                        self.pages_select(
                            SharedString::from(format!("action-preset-{rule}-{index}")),
                            label,
                            MenuKind::AutomationPreset(rule, index),
                            cx,
                        )
                        .stretch_label()
                        .into_any_element(),
                    ),
                )
            }
            _ => None,
        }
    }

    /// `<ExportParams>`.
    fn render_export_params(
        &self,
        rule: usize,
        index: usize,
        profile: &ExportProfile,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let format_label = EXPORT_FORMATS
            .iter()
            .find(|(candidate, _)| *candidate == profile.format)
            .map(|(_, label)| *label)
            .unwrap_or("MP4");
        let resolution_label = RESOLUTION_PRESETS
            .iter()
            .find(|(value, ..)| *value == resolution_value(profile))
            .map(|(_, label, ..)| *label)
            .unwrap_or("1080p");
        let compression_label = COMPRESSIONS
            .iter()
            .find(|(candidate, _)| {
                *candidate
                    == profile
                        .compression
                        .unwrap_or(AutomationExportCompression::Web)
            })
            .map(|(_, label)| tr(label))
            .unwrap_or(t!("Web"));
        let is_mp4 = profile.format == ExportFormat::Mp4;

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        self.editor_field(
                            t!("Format"),
                            self.pages_select(
                                SharedString::from(format!("export-format-{rule}-{index}")),
                                format_label,
                                MenuKind::AutomationExportFormat(rule, index),
                                cx,
                            )
                            .stretch_label()
                            .into_any_element(),
                        ),
                    )
                    .child(
                        self.editor_field(
                            t!("Resolution"),
                            self.pages_select(
                                SharedString::from(format!("export-resolution-{rule}-{index}")),
                                resolution_label,
                                MenuKind::AutomationExportResolution(rule, index),
                                cx,
                            )
                            .stretch_label()
                            .into_any_element(),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        self.editor_field(
                            t!("Frame rate"),
                            self.pages_select(
                                SharedString::from(format!("export-fps-{rule}-{index}")),
                                format!("{} FPS", profile.fps),
                                MenuKind::AutomationExportFps(rule, index),
                                cx,
                            )
                            .stretch_label()
                            .into_any_element(),
                        ),
                    )
                    .when(is_mp4, |this| {
                        this.child(
                            self.editor_field(
                                t!("Compression"),
                                self.pages_select(
                                    SharedString::from(format!(
                                        "export-compression-{rule}-{index}"
                                    )),
                                    compression_label,
                                    MenuKind::AutomationExportCompression(rule, index),
                                    cx,
                                )
                                .stretch_label()
                                .into_any_element(),
                            ),
                        )
                    }),
            )
            .child(
                self.editor_field(
                    t!("Destination folder (optional, blank = project folder)"),
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(self.render_editor_input(AutoField::ActionExportDir(index))),
                        )
                        .child(self.button(
                            SharedString::from(format!("export-browse-{rule}-{index}")),
                            (ui::ButtonVariant::Gray, None),
                            t!("Browse"),
                            false,
                            cx,
                            move |this, window, cx| {
                                this.automation_pick_dir(
                                    rule,
                                    AutoField::ActionExportDir(index),
                                    window,
                                    cx,
                                );
                            },
                        ))
                        .into_any_element(),
                ),
            )
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// CLI install (crates/cli-install/src/lib.rs, ported)
// ---------------------------------------------------------------------------

/// A direct port of `crates/cli-install` -- this standalone workspace cannot
/// depend on the crate, and the whole point is that `status`/`install`/
/// `uninstall` behave byte-for-byte like the Tauri commands
/// (`src-tauri/src/cli.rs`) so the two apps manage the same shim.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn hotkey(code: &str, meta: bool, ctrl: bool, alt: bool, shift: bool) -> Hotkey {
        Hotkey {
            code: code.into(),
            meta,
            ctrl,
            alt,
            shift,
        }
    }

    #[test]
    fn shortcuts_need_a_typing_safe_modifier() {
        assert!(shortcut_needs_modifier(&hotkey(
            "KeyA", false, false, false, false
        )));
        assert!(shortcut_needs_modifier(&hotkey(
            "Digit4", false, false, false, true
        )));
        assert!(!shortcut_needs_modifier(&hotkey(
            "Digit4", true, false, false, true
        )));
        assert!(!shortcut_needs_modifier(&hotkey(
            "KeyS", false, true, false, false
        )));
        assert!(!shortcut_needs_modifier(&hotkey(
            "KeyS", false, false, true, false
        )));
        assert!(!shortcut_needs_modifier(&hotkey(
            "F5", false, false, false, false
        )));
        assert!(!shortcut_needs_modifier(&hotkey(
            "F12", false, false, false, true
        )));
        assert!(shortcut_needs_modifier(&hotkey(
            "Space", false, false, false, false
        )));
    }

    #[test]
    fn every_hotkey_action_appears_once_with_a_label_and_effect() {
        let shown: Vec<&str> = SCREENSHOT_SHORTCUTS
            .iter()
            .chain(RECORDING_SHORTCUTS.iter())
            .chain(WHILE_RECORDING_SHORTCUTS.iter())
            .copied()
            .collect();
        assert_eq!(shown.len(), 13);
        let settings = store::GeneralSettings::default();
        for action in &shown {
            assert_eq!(shown.iter().filter(|other| *other == action).count(), 1);
            assert_ne!(shortcut_label(action), "Shortcut", "{action}");
            assert!(
                !shortcut_effect(action, &settings, crate::main_window::Mode::Studio).is_empty(),
                "{action}"
            );
        }
    }

    /// The reserved two lines only hold if no outcome-dependent description
    /// wraps to a third at the window's minimum width (about 55 characters
    /// per line there).
    #[test]
    fn outcome_descriptions_fit_the_reserved_two_lines() {
        let mut settings = store::GeneralSettings::default();
        let mut longest = 0;
        for screenshot in [
            store::PostScreenshotBehaviour::OpenEditor,
            store::PostScreenshotBehaviour::Copy,
            store::PostScreenshotBehaviour::CopyAndOpenEditor,
            store::PostScreenshotBehaviour::Save,
        ] {
            settings.post_screenshot_behaviour = screenshot;
            for open_links_disabled in [false, true] {
                settings.disable_auto_open_links = open_links_disabled;
                for studio in [
                    store::PostStudioBehaviour::OpenEditor,
                    store::PostStudioBehaviour::ShowOverlay,
                    store::PostStudioBehaviour::DoNothing,
                ] {
                    settings.post_studio_recording_behaviour = studio;
                    for action in SCREENSHOT_SHORTCUTS
                        .iter()
                        .chain(RECORDING_SHORTCUTS.iter())
                        .filter(|action| description_follows_outcome(action))
                    {
                        let text = capitalized_sentence(&shortcut_effect(
                            action,
                            &settings,
                            crate::main_window::Mode::Studio,
                        ));
                        longest = longest.max(text.chars().count());
                    }
                }
            }
        }
        assert!(
            longest <= 104,
            "longest outcome description is {longest} characters"
        );
    }

    #[test]
    fn every_after_capture_value_has_exactly_one_card() {
        use store::SettingsEnum as _;
        for value in store::PostScreenshotBehaviour::ALL {
            assert_eq!(
                SCREENSHOT_OUTCOMES
                    .iter()
                    .filter(|(v, _)| v == value)
                    .count(),
                1
            );
        }
        for value in store::PostStudioBehaviour::ALL {
            assert_eq!(
                STUDIO_OUTCOMES.iter().filter(|(v, _)| v == value).count(),
                1
            );
        }
        for disabled in [false, true] {
            assert_eq!(
                INSTANT_OUTCOMES
                    .iter()
                    .filter(|(v, _)| *v == disabled)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn descriptions_follow_the_after_capture_settings() {
        let mut settings = store::GeneralSettings::default();
        let studio = crate::main_window::Mode::Studio;
        assert_eq!(
            shortcut_effect("screenshotArea", &settings, studio),
            "select an area and open it in the editor"
        );
        settings.post_screenshot_behaviour = store::PostScreenshotBehaviour::Copy;
        assert_eq!(
            shortcut_effect("screenshotArea", &settings, studio),
            "select an area and copy it to your clipboard"
        );
        assert!(
            shortcut_effect("startStudioRecording", &settings, studio)
                .ends_with("open the editor when you stop")
        );
        settings.post_studio_recording_behaviour = store::PostStudioBehaviour::ShowOverlay;
        assert!(!shortcut_effect("startStudioRecording", &settings, studio).contains("editor"));
        settings.disable_auto_open_links = true;
        assert!(
            shortcut_effect("startInstantRecording", &settings, studio)
                .ends_with("copy its share link when you stop")
        );
        assert_eq!(
            shortcut_effect(
                "openRecordingPickerWindow",
                &settings,
                crate::main_window::Mode::Instant
            ),
            "choose a window, then record it in Instant mode"
        );
        assert_eq!(
            capitalized_sentence("stop the current recording"),
            "Stop the current recording."
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn combos_read_naturally_inside_a_sentence() {
        assert_eq!(
            hotkey_inline_text(&hotkey("Digit4", true, false, false, true)),
            "⌘⇧4"
        );
        assert_eq!(
            hotkey_inline_text(&hotkey("KeyR", false, true, true, false)),
            "⌃⌥R"
        );
        assert_eq!(
            hotkey_inline_text(&hotkey("Slash", true, false, false, false)),
            "⌘/"
        );
    }

    /// gpui keystroke names against the `KeyboardEvent.code` strings hotkeys
    /// written by the Tauri app carry.
    #[test]
    fn keystrokes_map_to_w3c_codes() {
        assert_eq!(hotkey_code_for_key("a").as_deref(), Some("KeyA"));
        assert_eq!(hotkey_code_for_key("Z").as_deref(), Some("KeyZ"));
        assert_eq!(hotkey_code_for_key("4").as_deref(), Some("Digit4"));
        assert_eq!(hotkey_code_for_key("f5").as_deref(), Some("F5"));
        assert_eq!(hotkey_code_for_key("f12").as_deref(), Some("F12"));
        assert_eq!(hotkey_code_for_key("space").as_deref(), Some("Space"));
        assert_eq!(hotkey_code_for_key("enter").as_deref(), Some("Enter"));
        assert_eq!(hotkey_code_for_key(",").as_deref(), Some("Comma"));
        assert_eq!(hotkey_code_for_key("up").as_deref(), Some("ArrowUp"));
        // Unbindable keys are consumed but never stored.
        assert_eq!(hotkey_code_for_key("fn"), None);
        assert_eq!(hotkey_code_for_key("escape"), None);
    }

    /// Every code the capture can produce parses as a `global_hotkey` `Code`.
    /// This is the shared-store guarantee: the Tauri `HotkeysStore` fails to
    /// deserialize wholesale on one unknown code, losing every binding, so a
    /// code this page writes must always be one that side can read back.
    #[test]
    fn every_producible_code_parses_for_the_tauri_app() {
        let mut keys: Vec<String> = Vec::new();
        for c in 'a'..='z' {
            keys.push(c.to_string());
        }
        for c in '0'..='9' {
            keys.push(c.to_string());
        }
        for c in [',', '.', '/', ';', '\'', '[', ']', '\\', '-', '=', '`'] {
            keys.push(c.to_string());
        }
        // gpui function keys stop at f19 on Apple keyboards; f20 is the
        // largest either side can see.
        for n in 1..=20 {
            keys.push(format!("f{n}"));
        }
        for named in [
            "space",
            "enter",
            "tab",
            "backspace",
            "delete",
            "up",
            "down",
            "left",
            "right",
            "home",
            "end",
            "pageup",
            "pagedown",
        ] {
            keys.push(named.to_string());
        }

        for key in keys {
            let Some(code) = hotkey_code_for_key(&key) else {
                panic!("{key} should map to a code");
            };
            assert!(
                global_hotkey::hotkey::Code::from_str(&code).is_ok(),
                "{key} produced {code}, which global_hotkey cannot parse"
            );
        }
    }

    #[test]
    fn hotkey_chips_render_in_macos_modifier_order() {
        let hotkey = Hotkey {
            code: "KeyS".into(),
            meta: true,
            ctrl: false,
            alt: true,
            shift: true,
        };
        assert_eq!(
            hotkey_display_keys(&hotkey),
            ["meta", "alt", "shift", "KeyS"]
        );
    }

    #[test]
    fn only_debug_cargo_builds_can_switch_back_to_the_dev_harness() {
        let cargo_build = std::path::Path::new(
            "/Users/x/FrameEver/apps/desktop-gpui/target/debug/frameever-gpui",
        );
        let installed_app =
            std::path::Path::new("/Applications/FrameEver.app/Contents/MacOS/FrameEver");

        assert!(dev_supervisor_available_for(Some(cargo_build), true));
        assert!(!dev_supervisor_available_for(Some(cargo_build), false));
        assert!(!dev_supervisor_available_for(Some(installed_app), true));
        assert!(!dev_supervisor_available_for(Some(installed_app), false));
        assert!(!dev_supervisor_available_for(None, true));
    }

    /// The takeover's whole timeline, read off its one clock.
    #[test]
    fn the_takeover_counts_down_while_the_lines_fade() {
        // The number is up from the first frame and never reaches zero.
        assert_eq!(takeover_frame(0.).2, SWITCH_COUNTDOWN_FROM);
        assert_eq!(takeover_frame(1000.).2, SWITCH_COUNTDOWN_FROM - 1);
        assert_eq!(takeover_frame(SWITCH_TAKEOVER_MS as f32 - 1.).2, 1);
        assert_eq!(takeover_frame(SWITCH_TAKEOVER_MS as f32).2, 1);

        // First line, faded in over its lead, and out again as its slot ends.
        let (index, alpha, _) = takeover_frame(0.);
        assert_eq!(index, 0);
        assert_eq!(alpha, 0.);
        assert_eq!(takeover_frame(SWITCH_FADE_MS).1, 1.);
        assert_eq!(takeover_frame(SWITCH_SENTENCE_MS as f32 - 1.).1.round(), 0.);

        // The last line holds rather than blanking while the number finishes.
        let (index, alpha, _) = takeover_frame(SWITCH_TAKEOVER_MS as f32 - 1.);
        assert_eq!(index, SWITCH_SENTENCES.len() - 1);
        assert_eq!(alpha, 1.);
    }

    /// `conditionAppliesToTrigger` / `actionAppliesToTrigger` against the
    /// tables in `utils/automations.ts`.
    #[test]
    fn applicability_matches_the_web_tables() {
        assert!(condition_applies_to_trigger(
            ConditionType::CaptureTargetIs,
            Trigger::ScreenshotTaken
        ));
        assert!(!condition_applies_to_trigger(
            ConditionType::CaptureTargetIs,
            Trigger::StudioRecordingFinished
        ));
        assert!(condition_applies_to_trigger(
            ConditionType::DurationAtLeast,
            Trigger::StudioRecordingFinished
        ));
        assert!(!condition_applies_to_trigger(
            ConditionType::DurationAtLeast,
            Trigger::InstantRecordingFinished
        ));
        // `organizationIs` maps to `null` and never applies.
        for trigger in ALL_TRIGGERS {
            assert!(!condition_applies_to_trigger(
                ConditionType::OrganizationIs,
                trigger
            ));
        }

        assert!(action_applies_to_trigger(
            ActionType::CopyToClipboard,
            Trigger::ScreenshotTaken
        ));
        assert!(!action_applies_to_trigger(
            ActionType::CopyToClipboard,
            Trigger::StudioRecordingFinished
        ));
        assert!(action_applies_to_trigger(
            ActionType::SkipEditor,
            Trigger::StudioRecordingFinished
        ));
        assert!(!action_applies_to_trigger(
            ActionType::SkipEditor,
            Trigger::UploadCompleted
        ));
        // notify/runCommand/webhook always apply.
        assert!(action_applies_to_trigger(
            ActionType::Notify,
            Trigger::RecordingStarted
        ));
    }

    #[test]
    fn pickers_never_offer_removed_cloud_items() {
        assert!(!ALL_TRIGGERS.into_iter().any(trigger_is_removed));
        assert!(!ALL_CONDITION_TYPES.into_iter().any(condition_is_removed));
        assert!(!ALL_ACTION_TYPES.into_iter().any(action_is_removed));
        assert!(trigger_is_removed(Trigger::UploadCompleted));
        assert!(condition_is_removed(ConditionType::OrganizationIs));
        assert!(action_is_removed(ActionType::Upload));
        assert!(action_is_removed(ActionType::Webhook));
    }

    /// `ruleSummary` / `autoRuleName` / `ruleDisplayName`.
    #[test]
    fn rule_names_match_the_web_strings() {
        let mut rule = create_empty_rule();
        assert_eq!(rule_summary(&rule), "Screenshot taken → Copy to clipboard");
        assert_eq!(auto_rule_name(&rule), "Screenshot → Clipboard");
        assert_eq!(rule_display_name(&rule), "Screenshot → Clipboard");

        rule.name = "  My rule  ".to_string();
        assert_eq!(rule_display_name(&rule), "My rule");

        rule.actions.clear();
        assert_eq!(rule_summary(&rule), "Screenshot taken → no actions yet");
        assert_eq!(auto_rule_name(&rule), "Screenshot automation");
    }

    #[test]
    fn export_defaults_match_the_web_profile() {
        let Action::Export {
            profile,
            destination,
        } = default_action(ActionType::Export)
        else {
            panic!("not an export action");
        };
        assert_eq!(profile.format, ExportFormat::Mp4);
        assert_eq!(profile.fps, 30);
        assert_eq!(profile.resolution_base.x, 1920);
        assert_eq!(profile.compression, Some(AutomationExportCompression::Web));
        assert_eq!(destination, ExportDestination::ProjectFolder);
        assert_eq!(resolution_value(&profile), "1080p");
    }

    #[test]
    fn secs_render_without_a_trailing_zero() {
        assert_eq!(format_secs(5.), "5");
        assert_eq!(format_secs(300.), "300");
        assert_eq!(format_secs(2.5), "2.5");
    }

    #[test]
    fn the_bundled_changelog_splits_into_versions() {
        let entries = parse_changelog(
            "# Changelog

intro

---

## [2.2.0] — 2026-06-01

### Fixed
- one

---

## [2.1.0]
- two
",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, "2.2.0");
        assert_eq!(entries[0].date, "2026-06-01");
        assert!(entries[0].content.contains("- one"));
        assert!(!entries[0].content.contains("---"));
        assert_eq!(entries[1].date, "");
        assert!(!parse_changelog(CHANGELOG).is_empty());
    }

    #[test]
    fn markdown_reduces_to_clean_paragraphs() {
        let paragraphs = markdown_paragraphs(
            "### Added
- **Bold** `code` [link](https://x)
![img](a.png)
",
        );
        assert_eq!(paragraphs.len(), 2);
        assert!(paragraphs[0].heading);
        assert_eq!(paragraphs[0].text, "Added");
        assert_eq!(paragraphs[1].text, "- Bold code link");
    }

    #[test]
    fn the_templates_build_local_rules() {
        assert_eq!(TEMPLATES.len(), 5);
        let ids: Vec<&str> = TEMPLATES.iter().map(|template| template.id).collect();
        assert_eq!(
            ids,
            [
                "copy-screenshot",
                "ocr-screenshot",
                "save-screenshot",
                "reveal-screenshot",
                "export-studio"
            ]
        );
        // Every template rule gets a fresh id and starts enabled.
        let a = (TEMPLATES[0].build)();
        let b = (TEMPLATES[0].build)();
        assert_ne!(a.id, b.id);
        assert!(a.enabled && a.conditions.is_empty());
    }
}
