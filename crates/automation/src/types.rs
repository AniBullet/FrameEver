use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

use frameever_project::XY;

#[derive(Serialize, Deserialize, Type, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomationsStore {
    #[serde(default)]
    pub version: u32,
    #[serde(default, deserialize_with = "deserialize_rules")]
    pub rules: Vec<AutomationRule>,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRule {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub trigger: Trigger,
    #[serde(default)]
    pub match_mode: MatchMode,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

fn deserialize_rules<'de, D>(deserializer: D) -> Result<Vec<AutomationRule>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let rules = Vec::<serde_json::Value>::deserialize(deserializer)?;
    let mut migrated = Vec::with_capacity(rules.len());
    for mut rule in rules {
        let id = rule
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        if rule.get("trigger").and_then(serde_json::Value::as_str) == Some("uploadCompleted") {
            tracing::warn!(rule_id = %id, "Removed retired upload-completed rule; local deletion refused");
            continue;
        }
        if let Some(actions) = rule
            .get_mut("actions")
            .and_then(serde_json::Value::as_array_mut)
        {
            let had_upload = actions.iter().any(|action| {
                action.get("type").and_then(serde_json::Value::as_str) == Some("upload")
            });
            let had_deletion = actions.iter().any(|action| {
                action.get("type").and_then(serde_json::Value::as_str) == Some("deleteLocalFiles")
            });
            actions.retain(|action| {
                let kind = action.get("type").and_then(serde_json::Value::as_str);
                !matches!(kind, Some("upload" | "webhook"))
                    && !(had_upload && kind == Some("deleteLocalFiles"))
            });
            if had_upload && had_deletion {
                tracing::warn!(rule_id = %id, "Removed local deletion from retired upload rule; local recording retained");
            }
        }
        migrated.push(serde_json::from_value(rule).map_err(serde::de::Error::custom)?);
    }
    Ok(migrated)
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum MatchMode {
    #[default]
    All,
    Any,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    ScreenshotTaken,
    StudioRecordingFinished,
    InstantRecordingFinished,
    RecordingStarted,
    VideoImported,
    RecordingDeleted,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Condition {
    CaptureTargetIs { target: CaptureTargetKind },
    RecordingModeIs { mode: AutomationRecordingMode },
    DurationAtLeast { secs: f64 },
    DurationAtMost { secs: f64 },
    WindowTitleContains { pattern: String },
    OrganizationIs { id: String },
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CaptureTargetKind {
    Display,
    Window,
    Area,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AutomationRecordingMode {
    Studio,
    Instant,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Action {
    CopyToClipboard {
        #[serde(default)]
        source: ClipboardSource,
    },
    #[serde(rename_all = "camelCase")]
    SaveToLocation {
        dir: String,
        #[serde(default)]
        filename_template: Option<String>,
    },
    Export {
        profile: ExportProfile,
        #[serde(default)]
        destination: ExportDestination,
    },
    RevealInFileManager,
    OpenFile,
    #[serde(rename_all = "camelCase")]
    RunCommand {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        use_shell: bool,
    },
    RecognizeTextToClipboard,
    #[serde(rename_all = "camelCase")]
    Notify {
        #[serde(default = "default_notify_title")]
        title_template: String,
        #[serde(default)]
        body_template: String,
    },
    OpenEditor,
    SkipEditor,
    ApplyPreset {
        name: String,
    },
    DeleteLocalFiles,
}

fn default_notify_title() -> String {
    "Cap Automation".to_string()
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardSource {
    #[default]
    Raw,
    Rendered,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfile {
    pub format: ExportFormat,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_resolution")]
    pub resolution_base: XY<u32>,
    #[serde(default)]
    pub compression: Option<AutomationExportCompression>,
    #[serde(default)]
    pub preset_name: Option<String>,
}

fn default_fps() -> u32 {
    30
}

fn default_resolution() -> XY<u32> {
    XY { x: 1920, y: 1080 }
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Mp4,
    Gif,
    Mov,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AutomationExportCompression {
    Maximum,
    Social,
    Web,
    Potato,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExportDestination {
    #[default]
    ProjectFolder,
    CustomPath {
        dir: String,
    },
}
