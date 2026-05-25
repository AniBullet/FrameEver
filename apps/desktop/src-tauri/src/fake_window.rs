use scap_targets::{Display, DisplayId, bounds::LogicalBounds};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tauri::{AppHandle, Manager, WebviewWindow};
use tokio::{sync::RwLock, time::sleep};
use tracing::instrument;

const RECORDING_CONTROLS_LABEL: &str = "in-progress-recording";
const RECORDING_CONTROLS_WIDTH: f64 = 320.0;
const RECORDING_CONTROLS_HEIGHT: f64 = 150.0;
const RECORDING_CONTROLS_OFFSET_Y: f64 = 120.0;

pub struct FakeWindowBounds(pub Arc<RwLock<HashMap<String, HashMap<String, LogicalBounds>>>>);

pub struct RecordingAreaState(pub Arc<RwLock<Option<RecordingAreaInfo>>>);

pub struct RecordingAreaInfo {
    pub display_id: DisplayId,
    pub bounds: LogicalBounds,
}

pub async fn set_recording_area(app: &AppHandle, display_id: DisplayId, bounds: LogicalBounds) {
    let state = app.state::<RecordingAreaState>();
    *state.0.write().await = Some(RecordingAreaInfo { display_id, bounds });
}

pub async fn clear_recording_area(app: &AppHandle) {
    let state = app.state::<RecordingAreaState>();
    *state.0.write().await = None;
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(state))]
pub async fn set_fake_window_bounds(
    window: tauri::Window,
    name: String,
    bounds: LogicalBounds,
    state: tauri::State<'_, FakeWindowBounds>,
) -> Result<(), String> {
    let mut state = state.0.write().await;
    let map = state.entry(window.label().to_string()).or_default();

    map.insert(name, bounds);

    Ok(())
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(state, window))]
pub async fn remove_fake_window(
    window: tauri::Window,
    name: String,
    state: tauri::State<'_, FakeWindowBounds>,
) -> Result<(), String> {
    let mut state = state.0.write().await;
    let Some(map) = state.get_mut(window.label()) else {
        return Ok(());
    };

    map.remove(&name);

    if map.is_empty() {
        state.remove(window.label());
    }

    Ok(())
}

fn get_display_id_for_cursor() -> Option<DisplayId> {
    Display::get_containing_cursor().map(|d| d.id())
}

fn get_display_by_id(id: &DisplayId) -> Option<Display> {
    Display::list().into_iter().find(|d| &d.id() == id)
}

fn calculate_bottom_center_position(display: &Display) -> Option<(f64, f64)> {
    let bounds = display.raw_handle().logical_bounds()?;
    let x = bounds.position().x();
    let y = bounds.position().y();
    let width = bounds.size().width();
    let height = bounds.size().height();

    let pos_x = x + (width - RECORDING_CONTROLS_WIDTH) / 2.0;
    let pos_y = y + height - RECORDING_CONTROLS_HEIGHT - RECORDING_CONTROLS_OFFSET_Y;
    Some((pos_x, pos_y))
}

pub fn spawn_fake_window_listener(app: AppHandle, window: WebviewWindow) {
    window.set_ignore_cursor_events(true).ok();

    let is_recording_controls = window.label() == RECORDING_CONTROLS_LABEL;

    tokio::spawn(async move {
        let state = app.state::<FakeWindowBounds>();
        let area_state = app.state::<RecordingAreaState>();
        let mut current_display_id: Option<DisplayId> = get_display_id_for_cursor();

        loop {
            sleep(Duration::from_millis(1000 / 20)).await;

            if is_recording_controls && let Some(cursor_display_id) = get_display_id_for_cursor() {
                let area_info = area_state.0.read().await;
                match area_info.as_ref() {
                    Some(info) => {
                        if current_display_id.as_ref() != Some(&info.display_id)
                            && let Some(display) = get_display_by_id(&info.display_id)
                            && let Some(display_bounds) = display.raw_handle().logical_bounds()
                        {
                            let area_x = display_bounds.position().x() + info.bounds.position().x();
                            let area_y = display_bounds.position().y() + info.bounds.position().y();
                            let area_w = info.bounds.size().width();
                            let area_h = info.bounds.size().height();

                            let margin = 16.0;
                            let center_x = area_x + area_w / 2.0;
                            let below_y = area_y + area_h + margin;
                            let disp_bottom =
                                display_bounds.position().y() + display_bounds.size().height();

                            let final_y = if below_y + RECORDING_CONTROLS_HEIGHT <= disp_bottom {
                                below_y
                            } else {
                                (area_y - RECORDING_CONTROLS_HEIGHT - margin)
                                    .max(display_bounds.position().y())
                            };

                            let final_x = (center_x - RECORDING_CONTROLS_WIDTH / 2.0)
                                .max(display_bounds.position().x() + margin)
                                .min(
                                    display_bounds.position().x() + display_bounds.size().width()
                                        - RECORDING_CONTROLS_WIDTH
                                        - margin,
                                );

                            let _ =
                                window.set_position(tauri::LogicalPosition::new(final_x, final_y));
                            current_display_id = Some(info.display_id.clone());
                        }
                    }
                    None => {
                        let display_changed =
                            current_display_id.as_ref() != Some(&cursor_display_id);

                        if display_changed
                            && let Some(display) = get_display_by_id(&cursor_display_id)
                            && let Some((pos_x, pos_y)) = calculate_bottom_center_position(&display)
                        {
                            let _ = window.set_position(tauri::LogicalPosition::new(pos_x, pos_y));
                            current_display_id = Some(cursor_display_id);
                        }
                    }
                }
            }

            let map = state.0.read().await;

            let Some(windows) = map.get(window.label()) else {
                window.set_ignore_cursor_events(true).ok();
                continue;
            };

            let (Ok(window_position), Ok(mouse_position), Ok(scale_factor)) = (
                window.outer_position(),
                window.cursor_position(),
                window.scale_factor(),
            ) else {
                let _ = window.set_ignore_cursor_events(true);
                continue;
            };

            let mut ignore = true;

            for bounds in windows.values() {
                let x_min = (window_position.x as f64) + bounds.position().x() * scale_factor;
                let x_max = (window_position.x as f64)
                    + (bounds.position().x() + bounds.size().width()) * scale_factor;
                let y_min = (window_position.y as f64) + bounds.position().y() * scale_factor;
                let y_max = (window_position.y as f64)
                    + (bounds.position().y() + bounds.size().height()) * scale_factor;

                if mouse_position.x >= x_min
                    && mouse_position.x <= x_max
                    && mouse_position.y >= y_min
                    && mouse_position.y <= y_max
                {
                    ignore = false;
                    break;
                }
            }

            window.set_ignore_cursor_events(ignore).ok();

            let focused = window.is_focused().unwrap_or(false);
            if !ignore {
                if !focused {
                    window.set_focus().ok();
                }
            } else if focused {
                window.set_ignore_cursor_events(ignore).ok();
            }
        }
    });
}

pub fn init(app: &AppHandle) {
    app.manage(FakeWindowBounds(Default::default()));
    app.manage(RecordingAreaState(Default::default()));
}
