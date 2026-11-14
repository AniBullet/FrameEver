use super::*;
use crate::screen_ink::{InkOverlay, ScreenInk};

pub(crate) fn ink_displays() -> Vec<scap_targets::DisplayId> {
    if cfg!(test) {
        return Vec::new();
    }
    scap_targets::Display::list()
        .iter()
        .map(scap_targets::Display::id)
        .collect()
}

pub(crate) fn open_ink_overlays(
    ink: Entity<ScreenInk>,
    displays: &[scap_targets::DisplayId],
    cx: &mut App,
) -> Vec<WindowHandle<InkOverlay>> {
    if cfg!(test) {
        return Vec::new();
    }
    let mut opened = Vec::new();
    for (index, id) in displays.iter().enumerate() {
        let Some(display) = scap_targets::Display::from_id(id) else {
            continue;
        };
        let Some(bounds) = capture_display_bounds(&display) else {
            tracing::warn!(index, "display has no bounds for drawing on screen");
            continue;
        };
        let ink = ink.clone();
        let handle = match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: gpui_display_id(Some(&display), cx),
                titlebar: None,
                kind: WindowKind::PopUp,
                focus: false,
                show: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: gpui::WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| InkOverlay::new(index, ink, window, cx)),
        ) {
            Ok(handle) => handle,
            Err(error) => {
                tracing::error!(%error, index, "drawing overlay could not open");
                continue;
            }
        };
        let native = handle
            .update(cx, |_, window, _| platform::native_window(window))
            .ok()
            .flatten();
        if let Some(native) = native {
            platform::place_overlay_panel(
                &native,
                f64::from(f32::from(bounds.origin.x)),
                f64::from(f32::from(bounds.origin.y)),
                f64::from(f32::from(bounds.size.width)),
                f64::from(f32::from(bounds.size.height)),
                platform::target_overlay_level(),
            );
        }
        opened.push(handle);
    }
    opened
}

pub(crate) fn set_ink_overlays_interactive(
    windows: &[WindowHandle<InkOverlay>],
    interactive: bool,
    cx: &mut App,
) {
    if windows.is_empty() {
        return;
    }
    let pointer_display = scap_targets::Display::get_containing_cursor()
        .and_then(|display| {
            scap_targets::Display::list()
                .into_iter()
                .position(|candidate| candidate.id() == display.id())
        })
        .unwrap_or(0);
    let mut keyboard = None;
    for (index, handle) in windows.iter().enumerate() {
        let native = handle
            .update(cx, |_, window, _| platform::native_window(window))
            .ok()
            .flatten();
        if let Some(native) = native
            && let Err(error) = platform::set_ink_overlay_interactive(&native, interactive)
        {
            tracing::error!(%error, interactive, "drawing overlay input could not be changed");
        }
        if interactive && (index == pointer_display || windows.len() == 1) {
            handle
                .update(cx, |overlay, window, cx| overlay.take_keyboard(window, cx))
                .ok();
            keyboard = Some(*handle);
        }
    }
    if !interactive {
        return;
    }
    if let Some(handle) = keyboard {
        cx.defer(move |cx| {
            handle
                .update(cx, |_, window, _| window.activate_window())
                .ok();
        });
    }
    if let Some(controls) = cx
        .try_global::<AppWindows>()
        .and_then(|windows| windows.controls)
    {
        controls
            .update(cx, |controls, window, _| {
                if !controls.tray_hidden() {
                    platform::show_window_without_focus(window);
                }
            })
            .ok();
    }
}
