use std::sync::{
    Mutex, OnceLock, PoisonError,
    atomic::{AtomicBool, Ordering},
};

use tracing_appender::non_blocking::WorkerGuard;

use crate::i18n::t;

static MAIN_THREAD: OnceLock<std::thread::ThreadId> = OnceLock::new();
static FINISHED: AtomicBool = AtomicBool::new(false);
static FAILURE_SHOWN: AtomicBool = AtomicBool::new(false);
static LOG_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

pub fn begin(log_guard: Option<WorkerGuard>) {
    *LOG_GUARD.lock().unwrap_or_else(PoisonError::into_inner) = log_guard;
    let _ = MAIN_THREAD.set(std::thread::current().id());
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        pid = std::process::id(),
        executable = %std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        "FrameEver starting"
    );
}

pub fn flush_log() {
    drop(
        LOG_GUARD
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take(),
    );
}

pub fn exit(code: i32) -> ! {
    flush_log();
    std::process::exit(code);
}

pub fn finished() {
    FINISHED.store(true, Ordering::Release);
}

fn on_main_thread() -> bool {
    MAIN_THREAD
        .get()
        .is_some_and(|main| *main == std::thread::current().id())
}

pub fn report_panic(message: &str, location: &str) {
    if FINISHED.load(Ordering::Acquire) || !on_main_thread() {
        return;
    }
    show_failure(&format!("{message} ({location})"));
}

pub fn fail(error: impl std::fmt::Display) -> ! {
    let error = error.to_string();
    tracing::error!(%error, "FrameEver could not start");
    crate::crash_capture::record_startup_failure(&error);
    show_failure(&error);
    exit(1);
}

fn show_failure(error: &str) {
    if FAILURE_SHOWN.swap(true, Ordering::AcqRel) {
        return;
    }
    let folder = crate::diagnostics::logs_dir();
    show_error(
        t!("Couldn't start FrameEver"),
        &failure_message(error, &folder.display()),
    );
}

fn failure_message(error: &str, folder: &dyn std::fmt::Display) -> String {
    format!(
        "{error}\n\n{}",
        t!("Details were saved in {folder}.", folder = folder)
    )
}

pub fn confirm_quit_unresponsive() -> bool {
    prepare_dialog();
    crate::platform::confirm_dialog(
        t!("Quit the FrameEver that isn't responding?"),
        t!(
            "FrameEver is already running but isn't responding. Quitting it closes that copy, and anything it hasn't saved is lost."
        ),
        t!("Quit and reopen"),
        t!("Cancel"),
        true,
    )
}

#[cfg(target_os = "windows")]
fn show_error(title: &str, message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MessageBoxW,
    };

    let wide = |text: &str| {
        text.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let (title, message) = (wide(title), wide(message));
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_error(title: &str, message: &str) {
    if !on_main_thread() {
        return;
    }
    prepare_dialog();
    crate::platform::alert_dialog(title, message);
}

#[cfg(target_os = "macos")]
fn prepare_dialog() {
    if on_main_thread() {
        crate::platform::activate_app();
    }
}

#[cfg(not(target_os = "macos"))]
fn prepare_dialog() {}

#[cfg(test)]
mod tests {
    #[test]
    fn a_failure_names_the_reason_and_where_the_details_are() {
        assert_eq!(
            super::failure_message("The main window could not be opened", &"C:\\logs"),
            "The main window could not be opened\n\nDetails were saved in C:\\logs."
        );
    }
}
