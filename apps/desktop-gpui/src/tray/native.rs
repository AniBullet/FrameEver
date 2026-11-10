use std::{io, mem::size_of};

use windows_sys::Win32::{Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM}, Graphics::Gdi::{GetMonitorInfoW, MONITORINFO, MONITOR_DEFAULTTONEAREST, MonitorFromRect}, UI::{HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI}, Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass}, WindowsAndMessaging::{GetWindowDisplayAffinity, PostMessageW, WDA_EXCLUDEFROMCAPTURE, WM_CONTEXTMENU, WM_LBUTTONUP, WM_NCDESTROY, WM_RBUTTONDOWN, WM_RBUTTONUP}}};

const KEYBOARD_SUBCLASS: usize = 0x4645_5452;
const TRAY_CALLBACK: u32 = 6002;

pub(super) fn install_keyboard_bridge(hwnd: HWND) -> io::Result<()> {
    if unsafe { SetWindowSubclass(hwnd, Some(keyboard_bridge), KEYBOARD_SUBCLASS, 0) } == 0 {
        return Err(io::Error::other("could not install notification-area keyboard bridge"));
    }
    Ok(())
}

unsafe extern "system" fn keyboard_bridge(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _: usize, _: usize) -> LRESULT {
    let notification = lparam as u32 & 0xffff;
    if message == WM_CONTEXTMENU || message == TRAY_CALLBACK && notification == WM_CONTEXTMENU {
        unsafe { PostMessageW(hwnd, TRAY_CALLBACK, 0, WM_RBUTTONDOWN as LPARAM); PostMessageW(hwnd, TRAY_CALLBACK, 0, WM_RBUTTONUP as LPARAM); }
        return 0;
    }
    if message == TRAY_CALLBACK && notification == 0x401 {
        unsafe { PostMessageW(hwnd, TRAY_CALLBACK, 0, WM_LBUTTONUP as LPARAM); }
        return 0;
    }
    if message == WM_NCDESTROY { unsafe { RemoveWindowSubclass(hwnd, Some(keyboard_bridge), KEYBOARD_SUBCLASS); } }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn confirmed_exclusion(hwnd: HWND) -> io::Result<()> {
    let mut affinity = 0;
    if unsafe { GetWindowDisplayAffinity(hwnd, &mut affinity) } == 0 { return Err(io::Error::last_os_error()); }
    if affinity != WDA_EXCLUDEFROMCAPTURE { return Err(io::Error::other(format!("tray popup capture exclusion was not confirmed: affinity={affinity}"))); }
    Ok(())
}

pub(super) fn placement(icon: tray_icon::Rect, logical_width: f64, logical_height: f64) -> io::Result<([f64; 4], f64)> {
    let rect = RECT { left: icon.position.x.round() as i32, top: icon.position.y.round() as i32, right: (icon.position.x + icon.size.width as f64).round() as i32, bottom: (icon.position.y + icon.size.height as f64).round() as i32 };
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 { return Err(io::Error::last_os_error()); }
    let (mut dpi_x, mut dpi_y) = (0, 0);
    let result = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    if result < 0 || dpi_x == 0 { return Err(io::Error::other(format!("tray display DPI unavailable: HRESULT={result}"))); }
    let scale = f64::from(dpi_x) / 96.;
    let work = info.rcWork;
    let work = [work.left as f64, work.top as f64, work.right as f64, work.bottom as f64];
    let height = (logical_height * scale).min(work[3] - work[1] - 24. * scale).max(34. * scale);
    let width = (logical_width * scale).min(work[2] - work[0]);
    let origin = popup_origin([rect.left as f64, rect.top as f64, rect.right as f64, rect.bottom as f64], work, [width, height], 12. * scale);
    Ok(([origin[0], origin[1], width, height], scale))
}

pub(super) fn popup_origin(icon: [f64; 4], work: [f64; 4], size: [f64; 2], gap: f64) -> [f64; 2] {
    let distances = [icon[1] - work[1], work[2] - icon[2], work[3] - icon[3], icon[0] - work[0]];
    let edge = distances.iter().enumerate().min_by(|(_, a), (_, b)| a.total_cmp(b)).map(|(index, _)| index).unwrap_or(2);
    let (x, y) = match edge {
        0 => (icon[2] - size[0], work[1] + gap),
        1 => (work[2] - gap - size[0], icon[3] - size[1]),
        3 => (work[0] + gap, icon[3] - size[1]),
        _ => (icon[2] - size[0], work[3] - gap - size[1]),
    };
    [x.clamp(work[0], (work[2] - size[0]).max(work[0])), y.clamp(work[1], (work[3] - size[1]).max(work[1]))]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateIcon { Recording, Paused, Export(u8) }

pub(super) fn icon_pixels(state: StateIcon, size: u32) -> Vec<u8> {
    let mut pixels = vec![0; (size * size * 4) as usize];
    let scale = size as f32 / 48.;
    for y in 0..size {
        for x in 0..size {
            let (xx, yy) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            let corner_x = (xx - 24.).abs() - 13.2;
            let corner_y = (yy - 24.).abs() - 13.2;
            if corner_x.max(0.).hypot(corner_y.max(0.)) > 10.8 { continue }
            let mut color = [31, 63, 216, 255];
            let radius = (xx - 24.).hypot(yy - 24.);
            if (12.5..17.5).contains(&radius) {
                color = match state {
                    StateIcon::Export(step) => {
                        let angle = ((xx - 24.).atan2(24. - yy) + std::f32::consts::TAU) % std::f32::consts::TAU;
                        if angle <= (step.min(7) as f32 + 1.) / 8. * std::f32::consts::TAU { [255, 255, 255, 255] } else { [98, 120, 228, 255] }
                    }
                    _ => [255, 255, 255, 255],
                };
            }
            match state {
                StateIcon::Recording if (16.0..32.0).contains(&xx) && (16.0..32.0).contains(&yy) => color = [255, 59, 48, 255],
                StateIcon::Paused if ((17.0..23.0).contains(&xx) || (25.0..31.0).contains(&xx)) && (17.0..31.0).contains(&yy) => color = [245, 165, 36, 255],
                _ => {}
            }
            let offset = ((y * size + x) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_popup_stays_away_from_taskbars_on_every_edge() {
        assert_eq!(popup_origin([1800., 1040., 1820., 1080.], [0., 0., 1920., 1040.], [280., 500.], 12.), [1540., 528.]);
        assert_eq!(popup_origin([1800., 0., 1820., 40.], [0., 40., 1920., 1080.], [280., 500.], 12.), [1540., 52.]);
        assert_eq!(popup_origin([1880., 900., 1920., 920.], [0., 0., 1880., 1080.], [280., 500.], 12.), [1588., 420.]);
        assert_eq!(popup_origin([0., 900., 40., 920.], [40., 0., 1920., 1080.], [280., 500.], 12.), [52., 420.]);
    }

    #[test]
    fn tray_popup_clamps_on_negative_origin_and_small_displays() {
        assert_eq!(popup_origin([-120., 0., -100., 40.], [-1920., -1040., 0., 0.], [280., 500.], 12.), [-380., -512.]);
        assert_eq!(popup_origin([280., 240., 300., 280.], [0., 0., 300., 240.], [280., 200.], 12.), [20., 28.]);
    }

    #[test]
    fn tray_icons_have_exact_physical_pixels_and_distinct_states_at_all_scales() {
        for size in [16, 20, 24, 32] {
            let recording = icon_pixels(StateIcon::Recording, size);
            let paused = icon_pixels(StateIcon::Paused, size);
            assert_eq!(recording.len(), (size * size * 4) as usize);
            assert_eq!(&recording[((size / 2 * size + size / 2) * 4) as usize..((size / 2 * size + size / 2) * 4 + 4) as usize], &[255, 59, 48, 255]);
            assert_ne!(recording, paused);
            assert!(recording.chunks_exact(4).all(|pixel| pixel[3] == 0 || pixel[3] == 255));
            let frames: std::collections::HashSet<_> = (0..8).map(|frame| icon_pixels(StateIcon::Export(frame), size)).collect();
            assert_eq!(frames.len(), 8);
            assert_eq!(icon_pixels(StateIcon::Export(7), size), icon_pixels(StateIcon::Export(8), size));
        }
    }
}
