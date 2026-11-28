use std::{io, mem::size_of};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect},
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            CalculatePopupWindowPosition, GetCursorPos, GetWindowDisplayAffinity, PostMessageW,
            TPM_BOTTOMALIGN, TPM_HORIZONTAL, TPM_LEFTALIGN, TPM_RIGHTALIGN, TPM_TOPALIGN,
            TPM_VERTICAL, TPM_WORKAREA, WDA_EXCLUDEFROMCAPTURE, WM_CONTEXTMENU, WM_LBUTTONUP,
            WM_NCDESTROY, WM_RBUTTONDOWN, WM_RBUTTONUP,
        },
    },
};

const KEYBOARD_SUBCLASS: usize = 0x4645_5452;
const TRAY_CALLBACK: u32 = 6002;

pub(super) fn install_keyboard_bridge(hwnd: HWND) -> io::Result<()> {
    if unsafe { SetWindowSubclass(hwnd, Some(keyboard_bridge), KEYBOARD_SUBCLASS, 0) } == 0 {
        return Err(io::Error::other(
            "could not install notification-area keyboard bridge",
        ));
    }
    Ok(())
}

unsafe extern "system" fn keyboard_bridge(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    let notification = lparam as u32 & 0xffff;
    if message == WM_CONTEXTMENU || message == TRAY_CALLBACK && notification == WM_CONTEXTMENU {
        unsafe {
            PostMessageW(hwnd, TRAY_CALLBACK, 0, WM_RBUTTONDOWN as LPARAM);
            PostMessageW(hwnd, TRAY_CALLBACK, 0, WM_RBUTTONUP as LPARAM);
        }
        return 0;
    }
    if message == TRAY_CALLBACK && notification == 0x401 {
        unsafe {
            PostMessageW(hwnd, TRAY_CALLBACK, 0, WM_LBUTTONUP as LPARAM);
        }
        return 0;
    }
    if message == WM_NCDESTROY {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(keyboard_bridge), KEYBOARD_SUBCLASS);
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn confirmed_exclusion(hwnd: HWND) -> io::Result<()> {
    let mut affinity = 0;
    if unsafe { GetWindowDisplayAffinity(hwnd, &mut affinity) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if affinity != WDA_EXCLUDEFROMCAPTURE {
        return Err(io::Error::other(format!(
            "tray popup capture exclusion was not confirmed: affinity={affinity}"
        )));
    }
    Ok(())
}

pub(super) struct Placement {
    pub bounds: [f64; 4],
    pub scale: f64,
    pub work: [f64; 4],
}

pub(super) fn cursor_position() -> Option<[f64; 2]> {
    let mut point = POINT::default();
    (unsafe { GetCursorPos(&mut point) } != 0).then_some([point.x as f64, point.y as f64])
}

pub(super) fn placement(
    icon: tray_icon::Rect,
    cursor: Option<[f64; 2]>,
    logical_width: f64,
    logical_height: f64,
) -> io::Result<Placement> {
    let rect = RECT {
        left: icon.position.x.round() as i32,
        top: icon.position.y.round() as i32,
        right: (icon.position.x + icon.size.width as f64).round() as i32,
        bottom: (icon.position.y + icon.size.height as f64).round() as i32,
    };
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let (mut dpi_x, mut dpi_y) = (0, 0);
    let result = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    if result < 0 || dpi_x == 0 {
        return Err(io::Error::other(format!(
            "tray display DPI unavailable: HRESULT={result}"
        )));
    }
    let scale = f64::from(dpi_x) / 96.;
    let work = rect_array(info.rcWork);
    let height = (logical_height * scale)
        .min(work[3] - work[1] - 24. * scale)
        .max(34. * scale);
    let width = (logical_width * scale).min(work[2] - work[0]);
    let request = popup_request(
        rect_array(rect),
        cursor,
        rect_array(info.rcMonitor),
        work,
        12. * scale,
    );
    let bounds = calculate_popup(&request, [width, height])?;
    Ok(Placement {
        bounds,
        scale,
        work,
    })
}

fn rect_array(rect: RECT) -> [f64; 4] {
    [
        rect.left as f64,
        rect.top as f64,
        rect.right as f64,
        rect.bottom as f64,
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PopupRequest {
    pub anchor: [f64; 2],
    pub flags: u32,
    pub exclude: [f64; 4],
}

pub(super) fn popup_request(
    icon: [f64; 4],
    cursor: Option<[f64; 2]>,
    monitor: [f64; 4],
    work: [f64; 4],
    gap: f64,
) -> PopupRequest {
    let on_icon = |point: &[f64; 2]| {
        point[0] >= icon[0] - gap
            && point[0] <= icon[2] + gap
            && point[1] >= icon[1] - gap
            && point[1] <= icon[3] + gap
    };
    let anchor = cursor
        .filter(on_icon)
        .unwrap_or([(icon[0] + icon[2]) / 2., (icon[1] + icon[3]) / 2.]);
    let distances = [
        icon[1] - work[1],
        work[2] - icon[2],
        work[3] - icon[3],
        icon[0] - work[0],
    ];
    let edge = distances
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
        .unwrap_or(2);
    let (flags, exclude) = match edge {
        0 => (
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_VERTICAL,
            [
                monitor[0],
                monitor[1],
                monitor[2],
                icon[3].max(work[1]) + gap,
            ],
        ),
        1 => (
            TPM_RIGHTALIGN | TPM_TOPALIGN | TPM_HORIZONTAL,
            [
                icon[0].min(work[2]) - gap,
                monitor[1],
                monitor[2],
                monitor[3],
            ],
        ),
        3 => (
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_HORIZONTAL,
            [
                monitor[0],
                monitor[1],
                icon[2].max(work[0]) + gap,
                monitor[3],
            ],
        ),
        _ => (
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_VERTICAL,
            [
                monitor[0],
                icon[1].min(work[3]) - gap,
                monitor[2],
                monitor[3],
            ],
        ),
    };
    PopupRequest {
        anchor,
        flags: flags | TPM_WORKAREA,
        exclude,
    }
}

fn calculate_popup(request: &PopupRequest, size: [f64; 2]) -> io::Result<[f64; 4]> {
    let anchor = POINT {
        x: request.anchor[0].round() as i32,
        y: request.anchor[1].round() as i32,
    };
    let size = SIZE {
        cx: size[0].round() as i32,
        cy: size[1].round() as i32,
    };
    let exclude = RECT {
        left: request.exclude[0].round() as i32,
        top: request.exclude[1].round() as i32,
        right: request.exclude[2].round() as i32,
        bottom: request.exclude[3].round() as i32,
    };
    let mut result = RECT::default();
    if unsafe { CalculatePopupWindowPosition(&anchor, &size, request.flags, &exclude, &mut result) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok([
        result.left as f64,
        result.top as f64,
        (result.right - result.left) as f64,
        (result.bottom - result.top) as f64,
    ])
}

pub(super) fn submenu_frame(main: [f64; 4], total_width: f64, work: [f64; 4]) -> ([f64; 4], bool) {
    let grows_left = main[0] + total_width > work[2];
    let x = if grows_left {
        main[0] + main[2] - total_width
    } else {
        main[0]
    };
    (
        [
            x.clamp(work[0], (work[2] - total_width).max(work[0])),
            main[1],
            total_width,
            main[3],
        ],
        grows_left,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateIcon {
    Recording,
    Paused,
    Export(u8),
    Update,
}

const UPDATE_DOT_CENTER: (f32, f32) = (40., 8.);
const UPDATE_DOT_RADIUS: f32 = 6.;
const UPDATE_DOT_RIM: f32 = 9.;

pub(super) fn icon_pixels(state: StateIcon, size: u32) -> Vec<u8> {
    let mut pixels = vec![0; (size * size * 4) as usize];
    let scale = size as f32 / 48.;
    for y in 0..size {
        for x in 0..size {
            let (xx, yy) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            let offset = ((y * size + x) * 4) as usize;
            if state == StateIcon::Update {
                let dot = (xx - UPDATE_DOT_CENTER.0).hypot(yy - UPDATE_DOT_CENTER.1);
                if dot <= UPDATE_DOT_RADIUS {
                    pixels[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
                    continue;
                }
                if dot <= UPDATE_DOT_RIM {
                    continue;
                }
            }
            let corner_x = (xx - 24.).abs() - 13.2;
            let corner_y = (yy - 24.).abs() - 13.2;
            if corner_x.max(0.).hypot(corner_y.max(0.)) > 10.8 {
                continue;
            }
            let mut color = [31, 63, 216, 255];
            let radius = (xx - 24.).hypot(yy - 24.);
            if (12.5..17.5).contains(&radius) {
                color = match state {
                    StateIcon::Export(step) => {
                        let angle = ((xx - 24.).atan2(24. - yy) + std::f32::consts::TAU)
                            % std::f32::consts::TAU;
                        if angle <= (step.min(7) as f32 + 1.) / 8. * std::f32::consts::TAU {
                            [255, 255, 255, 255]
                        } else {
                            [98, 120, 228, 255]
                        }
                    }
                    _ => [255, 255, 255, 255],
                };
            }
            match state {
                StateIcon::Recording
                    if (16.0..32.0).contains(&xx) && (16.0..32.0).contains(&yy) =>
                {
                    color = [255, 59, 48, 255]
                }
                StateIcon::Paused
                    if ((17.0..23.0).contains(&xx) || (25.0..31.0).contains(&xx))
                        && (17.0..31.0).contains(&yy) =>
                {
                    color = [245, 165, 36, 255]
                }
                _ => {}
            }
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: [f64; 4] = [0., 0., 1920., 1080.];

    #[test]
    fn tray_popup_request_excludes_the_taskbar_on_every_edge() {
        let bottom = popup_request(
            [1800., 1040., 1820., 1080.],
            Some([1810., 1060.]),
            MONITOR,
            [0., 0., 1920., 1040.],
            12.,
        );
        assert_eq!(bottom.anchor, [1810., 1060.]);
        assert_eq!(bottom.exclude, [0., 1028., 1920., 1080.]);
        assert_eq!(bottom.flags, TPM_BOTTOMALIGN | TPM_VERTICAL | TPM_WORKAREA);

        let top = popup_request(
            [1800., 0., 1820., 40.],
            Some([1810., 20.]),
            MONITOR,
            [0., 40., 1920., 1080.],
            12.,
        );
        assert_eq!(top.exclude, [0., 0., 1920., 52.]);
        assert_eq!(top.flags, TPM_VERTICAL | TPM_WORKAREA);

        let right = popup_request(
            [1880., 900., 1920., 920.],
            Some([1900., 910.]),
            MONITOR,
            [0., 0., 1880., 1080.],
            12.,
        );
        assert_eq!(right.exclude, [1868., 0., 1920., 1080.]);
        assert_eq!(right.flags, TPM_RIGHTALIGN | TPM_WORKAREA);

        let left = popup_request(
            [0., 900., 40., 920.],
            Some([20., 910.]),
            MONITOR,
            [40., 0., 1920., 1080.],
            12.,
        );
        assert_eq!(left.exclude, [0., 0., 52., 1080.]);
        assert_eq!(left.flags, TPM_WORKAREA);
    }

    #[test]
    fn tray_popup_request_keeps_clear_of_an_auto_hidden_taskbar() {
        let request = popup_request(
            [1800., 1040., 1820., 1080.],
            Some([1810., 1060.]),
            MONITOR,
            MONITOR,
            12.,
        );
        assert_eq!(request.exclude[1], 1028.);
    }

    #[test]
    fn tray_popup_request_falls_back_to_the_icon_when_opened_by_keyboard() {
        let request = popup_request(
            [1800., 1040., 1820., 1080.],
            Some([400., 300.]),
            MONITOR,
            [0., 0., 1920., 1040.],
            12.,
        );
        assert_eq!(request.anchor, [1810., 1060.]);
    }

    fn primary_work_area() -> [f64; 4] {
        let monitor = unsafe {
            windows_sys::Win32::Graphics::Gdi::MonitorFromPoint(
                POINT { x: 0, y: 0 },
                windows_sys::Win32::Graphics::Gdi::MONITOR_DEFAULTTOPRIMARY,
            )
        };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        assert_ne!(unsafe { GetMonitorInfoW(monitor, &mut info) }, 0);
        rect_array(info.rcWork)
    }

    #[test]
    fn tray_popup_opens_from_the_cursor_above_a_bottom_taskbar_and_stays_on_screen() {
        let work = primary_work_area();
        let centre = ((work[0] + work[2]) / 2.).round();
        let icon = [centre - 10., work[3] - 40., centre + 10., work[3]];
        let request = popup_request(icon, Some([centre, work[3] - 20.]), work, work, 12.);
        let bounds = calculate_popup(&request, [280., 200.]).unwrap();
        assert_eq!(bounds, [centre, work[3] - 52. - 200., 280., 200.]);

        let edge = work[2] - 60.;
        let icon = [edge - 10., work[3] - 40., edge + 10., work[3]];
        let request = popup_request(icon, Some([edge, work[3] - 20.]), work, work, 12.);
        let bounds = calculate_popup(&request, [280., 200.]).unwrap();
        assert_eq!(bounds[0], work[2] - 280.);
        assert_eq!(bounds[1] + bounds[3], work[3] - 52.);
    }

    #[test]
    fn tray_submenu_grows_toward_the_side_with_room_without_moving_the_menu() {
        let work = [0., 0., 1920., 1040.];
        assert_eq!(
            submenu_frame([400., 500., 280., 300.], 564., work),
            ([400., 500., 564., 300.], false)
        );
        assert_eq!(
            submenu_frame([1600., 500., 280., 300.], 564., work),
            ([1316., 500., 564., 300.], true)
        );
    }

    #[test]
    fn the_update_icon_puts_a_white_dot_with_a_clear_rim_on_the_top_right() {
        for size in [16, 20, 24, 32] {
            let pixels = icon_pixels(StateIcon::Update, size);
            let at = |x: f32, y: f32| {
                let scale = size as f32 / 48.;
                let (px, py) = ((x * scale) as u32, (y * scale) as u32);
                let offset = ((py * size + px) * 4) as usize;
                pixels[offset..offset + 4].to_vec()
            };
            assert_eq!(at(40., 8.), vec![255, 255, 255, 255], "size {size}");
            assert_eq!(at(24., 24.)[3], 255, "size {size}");
            assert_ne!(pixels, icon_pixels(StateIcon::Recording, size));
        }
        let pixels = icon_pixels(StateIcon::Update, 48);
        let rim = ((14 * 48 + 34) * 4) as usize;
        assert_eq!(pixels[rim + 3], 0);
    }

    #[test]
    fn tray_icons_have_exact_physical_pixels_and_distinct_states_at_all_scales() {
        for size in [16, 20, 24, 32] {
            let recording = icon_pixels(StateIcon::Recording, size);
            let paused = icon_pixels(StateIcon::Paused, size);
            assert_eq!(recording.len(), (size * size * 4) as usize);
            assert_eq!(
                &recording[((size / 2 * size + size / 2) * 4) as usize
                    ..((size / 2 * size + size / 2) * 4 + 4) as usize],
                &[255, 59, 48, 255]
            );
            assert_ne!(recording, paused);
            assert!(
                recording
                    .chunks_exact(4)
                    .all(|pixel| pixel[3] == 0 || pixel[3] == 255)
            );
            let frames: std::collections::HashSet<_> = (0..8)
                .map(|frame| icon_pixels(StateIcon::Export(frame), size))
                .collect();
            assert_eq!(frames.len(), 8);
            assert_eq!(
                icon_pixels(StateIcon::Export(7), size),
                icon_pixels(StateIcon::Export(8), size)
            );
        }
    }
}
