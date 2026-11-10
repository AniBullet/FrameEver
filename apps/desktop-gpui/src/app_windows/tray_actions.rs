use super::*;

pub(crate) fn camera_capture_ready(
    mode: RecordingMode,
    cx: &mut App,
) -> Result<Option<flume::Receiver<()>>, String> {
    if mode != RecordingMode::Instant {
        return Ok(None);
    }
    let Some(handle) = cx
        .try_global::<AppWindows>()
        .and_then(|windows| windows.camera)
    else {
        return Ok(None);
    };
    handle
        .update(cx, |_, window, _| {
            if !platform::window_is_visible(window) {
                return None;
            }
            let (sender, receiver) = flume::bounded(1);
            window.refresh();
            window.on_next_frame(move |window, _| {
                window.on_next_frame(move |_, _| {
                    if let Err(error) = sender.send(()) {
                        tracing::debug!(%error, "camera capture preparation was cancelled");
                    }
                });
            });
            Some(receiver)
        })
        .map_err(|error| {
            t!(
                "Couldn't prepare camera controls for capture: {error}",
                error = error
            )
        })
}

pub(crate) fn tray_open_library_item(path: PathBuf, cx: &mut App) {
    let main = cx.global::<AppWindows>().main;
    if let Err(error) = main.update(cx, |view, window, cx| {
        view.select_library_project(path, window, cx);
    }) {
        tracing::error!("could not select the recent project in the library: {error}");
    }
    show_library(cx);
    cx.activate(true);
}

pub(crate) fn tray_last_area_available(cx: &App) -> bool {
    let mode = cx
        .global::<AppWindows>()
        .main
        .read(cx)
        .map_or(Mode::Studio, MainWindow::mode);
    crate::target_overlay::last_locked_target(mode).is_some()
}

pub(crate) fn tray_capture_last_area(screenshot: bool, cx: &mut App) {
    if RecordingSession::global(cx).read(cx).phase != Phase::Idle {
        return;
    }
    let main = cx.global::<AppWindows>().main;
    let mode = if screenshot {
        Mode::Screenshot
    } else {
        main.read(cx).map_or(Mode::Studio, MainWindow::mode)
    };
    let Some(target) = crate::target_overlay::last_locked_target(mode) else {
        tracing::warn!("the locked recording area is no longer available");
        return;
    };
    if screenshot {
        crate::screenshot::take_screenshot(target, cx);
    } else {
        crate::hotkeys::start_now(main, mode, target, cx);
    }
}

pub(crate) fn tray_controls_hidden(cx: &App) -> bool {
    cx.global::<AppWindows>()
        .controls
        .and_then(|handle| handle.read(cx).ok())
        .is_some_and(ControlsWindow::tray_hidden)
}

pub(crate) fn tray_toggle_controls(cx: &mut App) {
    let hidden = !tray_controls_hidden(cx);
    if let Some(handle) = cx.global::<AppWindows>().controls
        && let Err(error) =
            handle.update(cx, |view, window, _| view.set_tray_hidden(hidden, window))
    {
        tracing::error!("could not change recording controls visibility: {error}");
    }
}

pub(crate) fn tray_restart_recording(cx: &mut App) {
    if let Some(handle) = cx.global::<AppWindows>().controls
        && let Err(error) = handle.update(cx, |view, window, cx| view.restart_from_tray(window, cx))
    {
        tracing::error!("could not open the restart confirmation: {error}");
    }
}

pub(crate) fn tray_export_status(cx: &App) -> Option<(String, u8)> {
    let windows = cx.global::<AppWindows>();
    for (_, handle) in &windows.editors {
        let Ok(editor) = handle.read(cx) else {
            continue;
        };
        let Some(export) = editor
            .export
            .as_ref()
            .filter(|export| export.phase.is_busy())
        else {
            continue;
        };
        let name = editor.name_input.read(cx).text().to_string();
        let progress = (u64::from(export.rendered) * 100 / u64::from(export.total_frames.max(1)))
            .min(100) as u8;
        return Some((name, progress));
    }
    windows
        .screenshot_editors
        .iter()
        .find_map(|(path, handle)| {
            handle
                .read(cx)
                .ok()
                .filter(|view| view.export_in_flight())
                .map(|_| {
                    (
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        0,
                    )
                })
        })
}

pub(crate) fn tray_show_export(cx: &mut App) {
    let editor = cx
        .global::<AppWindows>()
        .editors
        .iter()
        .find_map(|(_, handle)| {
            handle
                .read(cx)
                .ok()?
                .export
                .as_ref()
                .filter(|export| export.phase.is_busy())
                .map(|_| *handle)
        });
    if let Some(handle) = editor {
        if let Err(error) = handle.update(cx, |_, window, _| window.activate_window()) {
            tracing::error!("could not show the active export: {error}");
        }
        cx.activate(true);
        return;
    }
    let screenshot = cx
        .global::<AppWindows>()
        .screenshot_editors
        .iter()
        .find_map(|(_, handle)| {
            handle
                .read(cx)
                .ok()
                .filter(|view| view.export_in_flight())
                .map(|_| *handle)
        });
    if let Some(handle) = screenshot {
        if let Err(error) = handle.update(cx, |_, window, _| window.activate_window()) {
            tracing::error!("could not show the active screenshot export: {error}");
        }
        cx.activate(true);
    }
}

pub(crate) fn focus_camera_preview(cx: &mut App) {
    open_camera_window(cx);
    if let Some(handle) = cx.global::<AppWindows>().camera {
        if let Err(error) = handle.update(cx, |view, window, cx| {
            view.focus_controls(window, cx);
            window.activate_window();
        }) {
            tracing::error!("could not focus the camera preview: {error}");
        }
        cx.activate(true);
    }
}
