pub fn enabled() -> bool {
    crate::store::GeneralSettings::load().enable_notifications
}

#[cfg(windows)]
pub fn show(title: &str, body: &str) -> Result<(), String> {
    use tauri_winrt_notification::Toast;

    let app_id = if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID
    } else {
        crate::brand::APP_ID
    };
    Toast::new(app_id)
        .title(title)
        .text1(body)
        .show()
        .map_err(|error| format!("Failed to send notification: {error}"))
}

#[cfg(not(windows))]
pub fn show(_title: &str, _body: &str) -> Result<(), String> {
    Err("Notifications are available on Windows only".to_string())
}
