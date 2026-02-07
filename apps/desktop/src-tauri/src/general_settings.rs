use crate::window_exclusion::WindowExclusion;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::path::PathBuf;
use std::str::FromStr;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::StoreExt;
use tracing::{error, instrument};
use uuid::Uuid;

#[derive(Default, Serialize, Deserialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum PostStudioRecordingBehaviour {
    #[default]
    OpenEditor,
    ShowOverlay,
}

#[derive(Default, Serialize, Deserialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum MainWindowRecordingStartBehaviour {
    #[default]
    Close,
    Minimise,
}

#[derive(Default, Serialize, Deserialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum PostDeletionBehaviour {
    #[default]
    DoNothing,
    ReopenRecordingWindow,
}

#[derive(Default, Serialize, Deserialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum EditorPreviewQuality {
    Quarter,
    #[default]
    Half,
    Full,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum RecordingQuality {
    High,
    #[default]
    Standard,
    Low,
}

impl RecordingQuality {
    /// 返回该质量级别对应的比特率倍数（相对于标准质量）
    pub fn bitrate_multiplier(&self) -> f32 {
        match self {
            Self::High => 1.5,     // 高质量：1.5倍比特率
            Self::Standard => 1.0, // 标准质量：基准比特率
            Self::Low => 0.6,      // 低质量：0.6倍比特率
        }
    }

    /// 返回该质量级别对应的 bits per pixel 值（用于编码器）
    pub fn bits_per_pixel(&self) -> f32 {
        match self {
            Self::High => 0.25,     // 高质量
            Self::Standard => 0.15, // 标准质量（类似 Social）
            Self::Low => 0.08,      // 低质量（类似 Web）
        }
    }
}

impl MainWindowRecordingStartBehaviour {
    pub fn perform(&self, window: &tauri::WebviewWindow) -> tauri::Result<()> {
        match self {
            Self::Close => window.hide(),
            Self::Minimise => window.minimize(),
        }
    }
}

const DEFAULT_EXCLUDED_WINDOW_TITLES: &[&str] = &[
    "Cap",
    "Cap Settings",
    "Cap Recording Controls",
    "Cap Camera",
];

pub fn default_excluded_windows() -> Vec<WindowExclusion> {
    DEFAULT_EXCLUDED_WINDOW_TITLES
        .iter()
        .map(|title| WindowExclusion {
            bundle_identifier: None,
            owner_name: None,
            window_title: Some((*title).to_string()),
        })
        .collect()
}

// When adding fields here, #[serde(default)] defines the value to use for existing configurations,
// and `Default::default` defines the value to use for new configurations.
// Things that affect the user experience should only be enabled by default for new configurations.
#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettingsStore {
    #[serde(default = "uuid::Uuid::new_v4")]
    pub instance_id: Uuid,
    #[serde(default)]
    pub hide_dock_icon: bool,
    #[serde(default)]
    pub auto_create_shareable_link: bool,
    #[serde(default = "default_true")]
    pub enable_notifications: bool,
    #[serde(default)]
    pub disable_auto_open_links: bool,
    #[serde(default = "default_true")]
    pub has_completed_startup: bool,
    #[serde(default)]
    pub theme: AppTheme,
    #[serde(default)]
    pub commercial_license: Option<CommercialLicense>,
    #[serde(default)]
    pub last_version: Option<String>,
    #[serde(default)]
    pub window_transparency: bool,
    #[serde(default)]
    pub post_studio_recording_behaviour: PostStudioRecordingBehaviour,
    #[serde(default)]
    pub main_window_recording_start_behaviour: MainWindowRecordingStartBehaviour,
    #[serde(default = "default_true", rename = "custom_cursor_capture2")]
    pub custom_cursor_capture: bool,
    #[serde(default = "default_server_url")]
    pub server_url: String,
    #[serde(default)]
    pub recording_countdown: Option<u32>,
    #[serde(
        default = "default_enable_native_camera_preview",
        skip_serializing_if = "no"
    )]
    pub enable_native_camera_preview: bool,
    #[serde(default)]
    pub auto_zoom_on_clicks: bool,
    #[serde(default)]
    pub post_deletion_behaviour: PostDeletionBehaviour,
    #[serde(default = "default_excluded_windows")]
    pub excluded_windows: Vec<WindowExclusion>,
    #[serde(default = "default_instant_mode_max_resolution")]
    pub instant_mode_max_resolution: u32,
    #[serde(default)]
    pub default_project_name_template: Option<String>,
    #[serde(default = "default_true")]
    pub crash_recovery_recording: bool,
    #[serde(default = "default_max_fps")]
    pub max_fps: u32,
    #[serde(default)]
    pub editor_preview_quality: EditorPreviewQuality,
    #[serde(default)]
    pub main_window_position: Option<WindowPosition>,
    #[serde(default)]
    pub camera_window_position: Option<WindowPosition>,
    #[serde(default)]
    pub recordings_save_path: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub recording_quality: RecordingQuality,
}

fn default_enable_native_camera_preview() -> bool {
    cfg!(all(debug_assertions, target_os = "macos"))
}

fn no(_: &bool) -> bool {
    false
}

fn default_true() -> bool {
    true
}

fn default_instant_mode_max_resolution() -> u32 {
    1920
}

fn default_max_fps() -> u32 {
    60
}

fn default_server_url() -> String {
    std::option_env!("VITE_SERVER_URL")
        .unwrap_or("https://cap.so")
        .to_string()
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommercialLicense {
    license_key: String,
    expiry_date: Option<f64>,
    refresh: f64,
    activated_on: f64,
}

impl Default for GeneralSettingsStore {
    fn default() -> Self {
        Self {
            instance_id: uuid::Uuid::new_v4(),
            hide_dock_icon: false,
            auto_create_shareable_link: false,
            enable_notifications: true,
            disable_auto_open_links: false,
            has_completed_startup: false,
            theme: AppTheme::System,
            commercial_license: None,
            last_version: None,
            window_transparency: false,
            post_studio_recording_behaviour: PostStudioRecordingBehaviour::OpenEditor,
            main_window_recording_start_behaviour: MainWindowRecordingStartBehaviour::Close,
            custom_cursor_capture: true,
            recordings_save_path: None,
            language: None,
            server_url: default_server_url(),
            recording_countdown: Some(3),
            enable_native_camera_preview: default_enable_native_camera_preview(),
            auto_zoom_on_clicks: false,
            post_deletion_behaviour: PostDeletionBehaviour::DoNothing,
            excluded_windows: default_excluded_windows(),
            instant_mode_max_resolution: 1920,
            default_project_name_template: None,
            crash_recovery_recording: true,
            max_fps: 60,
            editor_preview_quality: EditorPreviewQuality::Half,
            main_window_position: None,
            camera_window_position: None,
            recording_quality: RecordingQuality::Standard,
        }
    }
}

#[derive(Default, Debug, Copy, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl GeneralSettingsStore {
    pub fn get(app: &AppHandle<Wry>) -> Result<Option<Self>, String> {
        match app.store("store").map(|s| s.get("general_settings")) {
            Ok(Some(store)) => {
                // Handle potential deserialization errors gracefully
                match serde_json::from_value(store) {
                    Ok(settings) => Ok(Some(settings)),
                    Err(e) => Err(format!("Failed to deserialize general settings store: {e}")),
                }
            }
            _ => Ok(None),
        }
    }

    // i don't trust anyone to not overwrite the whole store lols
    pub fn update(app: &AppHandle, update: impl FnOnce(&mut Self)) -> Result<(), String> {
        let Ok(store) = app.store("store") else {
            return Err("Store not found".to_string());
        };

        let mut settings = Self::get(app)?.unwrap_or_default();
        update(&mut settings);
        store.set("general_settings", json!(settings));
        store.save().map_err(|e| e.to_string())
    }

    fn save(&self, app: &AppHandle) -> Result<(), String> {
        let Ok(store) = app.store("store") else {
            return Err("Store not found".to_string());
        };

        store.set("general_settings", json!(self));
        store.save().map_err(|e| e.to_string())
    }

    fn default_recordings_base_path(app: &AppHandle) -> Option<PathBuf> {
        if let Ok(video_dir) = app.path().video_dir() {
            let base = video_dir.join("Cap Recordings");
            if std::fs::create_dir_all(&base).is_ok() {
                return Some(base);
            }
        }
        if cfg!(target_os = "windows") {
            std::env::var("USERPROFILE")
                .ok()
                .map(PathBuf::from)
                .and_then(|p| {
                    let videos_path = p.join("Videos").join("Cap Recordings");
                    if std::fs::create_dir_all(&videos_path).is_ok() {
                        Some(videos_path)
                    } else {
                        None
                    }
                })
        } else {
            app.path().home_dir().ok().and_then(|p| {
                let videos_path = p.join("Videos").join("Cap Recordings");
                if std::fs::create_dir_all(&videos_path).is_ok() {
                    Some(videos_path)
                } else {
                    None
                }
            })
        }
    }

    pub fn recordings_base_path(app: &AppHandle) -> PathBuf {
        let custom_path = Self::get(app)
            .ok()
            .flatten()
            .and_then(|s| s.recordings_save_path)
            .and_then(|p| PathBuf::from_str(&p).ok());

        if let Some(path) = custom_path {
            let recordings_dir = path.join("recordings");
            if std::fs::create_dir_all(&recordings_dir).is_ok() {
                return path;
            }
        }

        if let Some(base_path) = Self::default_recordings_base_path(app) {
            let recordings_dir = base_path.join("recordings");
            if std::fs::create_dir_all(&recordings_dir).is_ok() {
                return base_path;
            }
        }

        let fallback_base = app.path().app_data_dir().unwrap_or_default();
        let fallback_path = fallback_base.join("recordings");
        std::fs::create_dir_all(&fallback_path).unwrap_or_default();
        fallback_base
    }

    pub fn recordings_path(app: &AppHandle) -> PathBuf {
        Self::recordings_base_path(app).join("recordings")
    }

    pub fn exports_video_path(app: &AppHandle) -> PathBuf {
        let custom_path = Self::get(app)
            .ok()
            .flatten()
            .and_then(|s| s.recordings_save_path)
            .and_then(|p| PathBuf::from_str(&p).ok());

        if let Some(path) = custom_path {
            let exports_dir = path.join("exports").join("video");
            if std::fs::create_dir_all(&exports_dir).is_ok() {
                return exports_dir;
            }
        }

        if let Some(base_path) = Self::default_recordings_base_path(app) {
            let exports_dir = base_path.join("exports").join("video");
            if std::fs::create_dir_all(&exports_dir).is_ok() {
                return exports_dir;
            }
        }

        let fallback_path = app
            .path()
            .app_data_dir()
            .unwrap_or_default()
            .join("exports")
            .join("video");
        std::fs::create_dir_all(&fallback_path).unwrap_or_default();
        fallback_path
    }

    pub fn exports_screenshot_path(app: &AppHandle) -> PathBuf {
        let custom_path = Self::get(app)
            .ok()
            .flatten()
            .and_then(|s| s.recordings_save_path)
            .and_then(|p| PathBuf::from_str(&p).ok());

        if let Some(path) = custom_path {
            let exports_dir = path.join("exports").join("screenshot");
            if std::fs::create_dir_all(&exports_dir).is_ok() {
                return exports_dir;
            }
        }

        if let Some(base_path) = Self::default_recordings_base_path(app) {
            let exports_dir = base_path.join("exports").join("screenshot");
            if std::fs::create_dir_all(&exports_dir).is_ok() {
                return exports_dir;
            }
        }

        let fallback_path = app
            .path()
            .app_data_dir()
            .unwrap_or_default()
            .join("exports")
            .join("screenshot");
        std::fs::create_dir_all(&fallback_path).unwrap_or_default();
        fallback_path
    }
}

pub fn init(app: &AppHandle) {
    println!("Initializing GeneralSettingsStore");

    let store = match GeneralSettingsStore::get(app) {
        Ok(Some(store)) => store,
        Ok(None) => GeneralSettingsStore::default(),
        Err(e) => {
            error!("Failed to deserialize general settings store: {}", e);
            GeneralSettingsStore::default()
        }
    };

    if let Err(e) = store.save(app) {
        error!("Failed to save general settings: {}", e);
    }

    println!("GeneralSettingsState managed");
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(app))]
pub fn get_default_recordings_path(app: AppHandle) -> String {
    GeneralSettingsStore::recordings_base_path(&app)
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
#[specta::specta]
#[instrument]
pub fn get_default_excluded_windows() -> Vec<WindowExclusion> {
    default_excluded_windows()
}
