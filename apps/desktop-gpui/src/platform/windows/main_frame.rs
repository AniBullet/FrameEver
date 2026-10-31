use std::ffi::c_void;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_CLOAK, DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    MONITORINFO, MonitorFromPoint, MonitorFromWindow,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetWindowLongPtrW, GetWindowRect, HTBORDER, HTBOTTOM, HTBOTTOMLEFT,
    HTBOTTOMRIGHT, HTCAPTION, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HWND_NOTOPMOST,
    HWND_TOPMOST, IsIconic, IsZoomed, MINMAXINFO, SW_HIDE, SW_MAXIMIZE, SW_RESTORE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    WM_GETMINMAXINFO, WM_NCDESTROY, WM_NCHITTEST, WS_MAXIMIZEBOX,
};

use super::NativeWindow;
use crate::main_window::shell::{COMPACT_HEIGHT, COMPACT_WIDTH, Frame, Placement, Screen};

const SUBCLASS_ID: usize = 0x4645_4d46;
const STANDARD_DPI: f64 = 96.;
const FIXED_SIZE: usize = 1;
const LIMIT_BITS: usize = 16;
const EMPTY_RECT: RECT = RECT {
    left: 0,
    top: 0,
    right: 0,
    bottom: 0,
};

fn pack_limits(fixed: bool, min_width: f64, min_height: f64) -> usize {
    usize::from(fixed) | (min_width as usize) << 1 | (min_height as usize) << (LIMIT_BITS + 1)
}

fn unpack_limits(limits: usize) -> (f64, f64) {
    let mask = (1 << LIMIT_BITS) - 1;
    (
        ((limits >> 1) & mask) as f64,
        ((limits >> (LIMIT_BITS + 1)) & mask) as f64,
    )
}

fn without_resize_edges(hit: LRESULT) -> LRESULT {
    match hit as u32 {
        HTTOP | HTTOPLEFT | HTTOPRIGHT => HTCAPTION as LRESULT,
        HTLEFT | HTRIGHT | HTBOTTOM | HTBOTTOMLEFT | HTBOTTOMRIGHT => HTBORDER as LRESULT,
        _ => hit,
    }
}

pub fn install(native: &NativeWindow) {
    let hwnd = native.hwnd();
    let limits = pack_limits(true, COMPACT_WIDTH, COMPACT_HEIGHT);
    unsafe {
        if SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, limits) == 0 {
            tracing::warn!("could not install the main window frame limits");
        }
    }
    set_maximize_box(hwnd, false);
}

fn set_maximize_box(hwnd: HWND, enabled: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let style = if enabled {
            style | WS_MAXIMIZEBOX as isize
        } else {
            style & !(WS_MAXIMIZEBOX as isize)
        };
        SetWindowLongPtrW(hwnd, GWL_STYLE, style);
    }
}

unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    limits: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        unsafe { RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID) };
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    match message {
        WM_NCHITTEST if limits & FIXED_SIZE != 0 => without_resize_edges(result),
        WM_GETMINMAXINFO => {
            unsafe { apply_size_limits(hwnd, lparam, limits) };
            result
        }
        _ => result,
    }
}

unsafe fn apply_size_limits(hwnd: HWND, lparam: LPARAM, limits: usize) {
    if lparam == 0 || unsafe { IsIconic(hwnd) != 0 || IsZoomed(hwnd) != 0 } {
        return;
    }
    let Some(insets) = frame_insets(hwnd) else {
        return;
    };
    let scale = unsafe { GetDpiForWindow(hwnd) } as f64 / STANDARD_DPI;
    let (min_width, min_height) = unpack_limits(limits);
    let width = (min_width * scale).round() as i32 + insets.left + insets.right;
    let height = (min_height * scale).round() as i32 + insets.top + insets.bottom;
    let info = unsafe { &mut *(lparam as *mut MINMAXINFO) };
    info.ptMinTrackSize = POINT {
        x: width,
        y: height,
    };
    if limits & FIXED_SIZE != 0 {
        info.ptMaxTrackSize = info.ptMinTrackSize;
    }
}

fn client_rect_on_screen(hwnd: HWND) -> Option<RECT> {
    let mut client = EMPTY_RECT;
    let mut origin = POINT { x: 0, y: 0 };
    unsafe {
        if GetClientRect(hwnd, &mut client) == 0 || ClientToScreen(hwnd, &mut origin) == 0 {
            return None;
        }
    }
    Some(RECT {
        left: origin.x,
        top: origin.y,
        right: origin.x + client.right,
        bottom: origin.y + client.bottom,
    })
}

fn frame_insets(hwnd: HWND) -> Option<RECT> {
    let client = client_rect_on_screen(hwnd)?;
    let mut outer = EMPTY_RECT;
    if unsafe { GetWindowRect(hwnd, &mut outer) } == 0 {
        return None;
    }
    Some(RECT {
        left: client.left - outer.left,
        top: client.top - outer.top,
        right: outer.right - client.right,
        bottom: outer.bottom - client.bottom,
    })
}

fn to_frame(rect: RECT) -> Frame {
    Frame {
        x: rect.left as f64,
        y: rect.top as f64,
        width: (rect.right - rect.left) as f64,
        height: (rect.bottom - rect.top) as f64,
    }
}

fn work_area(monitor: HMONITOR) -> Option<Frame> {
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    (unsafe { GetMonitorInfoW(monitor, &mut info) } != 0).then(|| to_frame(info.rcWork))
}

pub fn screen(native: &NativeWindow) -> Option<Screen> {
    let hwnd = native.hwnd();
    if unsafe { IsIconic(hwnd) } != 0 {
        return None;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    Some(Screen {
        frame: to_frame(client_rect_on_screen(hwnd)?),
        work_area: work_area(monitor)?,
        scale: unsafe { GetDpiForWindow(hwnd) } as f64 / STANDARD_DPI,
        maximized: unsafe { IsZoomed(hwnd) } != 0,
    })
}

pub fn restore_minimized(native: &NativeWindow) -> bool {
    let minimized = unsafe { IsIconic(native.hwnd()) } != 0;
    if minimized {
        unsafe { ShowWindow(native.hwnd(), SW_RESTORE) };
    }
    minimized
}

pub fn launch_screen(position: Option<(f64, f64)>) -> Option<Screen> {
    let (x, y) = position.unwrap_or_default();
    let point = POINT {
        x: x as i32,
        y: y as i32,
    };
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTOPRIMARY) };
    let area = work_area(monitor)?;
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    let scale = if dpi_x == 0 {
        1.
    } else {
        dpi_x as f64 / STANDARD_DPI
    };
    Some(Screen {
        frame: area,
        work_area: area,
        scale,
        maximized: false,
    })
}

fn set_dwm_flag(hwnd: HWND, attribute: i32, value: bool) {
    let value = i32::from(value);
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute as u32,
            std::ptr::addr_of!(value).cast::<c_void>(),
            std::mem::size_of_val(&value) as u32,
        );
    }
}

fn set_client_frame(hwnd: HWND, frame: Frame, on_top: bool) {
    let Some(insets) = frame_insets(hwnd) else {
        return;
    };
    let insert_after = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe {
        SetWindowPos(
            hwnd,
            insert_after,
            frame.x.round() as i32 - insets.left,
            frame.y.round() as i32 - insets.top,
            frame.width.round() as i32 + insets.left + insets.right,
            frame.height.round() as i32 + insets.top + insets.bottom,
            SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

pub fn place(native: &NativeWindow, placement: Placement, hide: bool) {
    let hwnd = native.hwnd();
    let was_zoomed = unsafe { IsZoomed(hwnd) } != 0;
    let cloak = hide && was_zoomed;
    set_dwm_flag(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, true);
    if cloak {
        set_dwm_flag(hwnd, DWMWA_CLOAK, true);
    }
    if hide {
        unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
    let limits = pack_limits(
        !placement.resizable,
        placement.min_width,
        placement.min_height,
    );
    unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, limits) };
    set_maximize_box(hwnd, placement.resizable);
    if was_zoomed {
        unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    set_client_frame(hwnd, placement.frame, placement.on_top);
    if unsafe { GetDpiForWindow(hwnd) } != dpi {
        set_client_frame(hwnd, placement.frame, placement.on_top);
    }
    if placement.maximized {
        unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
    }
    if cloak {
        unsafe { ShowWindow(hwnd, SW_HIDE) };
        set_dwm_flag(hwnd, DWMWA_CLOAK, false);
    }
    set_dwm_flag(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_survive_the_subclass_data_round_trip() {
        let packed = pack_limits(true, 330., 386.);
        assert_eq!(packed & FIXED_SIZE, FIXED_SIZE);
        assert_eq!(unpack_limits(packed), (330., 386.));
        let packed = pack_limits(false, 900., 600.);
        assert_eq!(packed & FIXED_SIZE, 0);
        assert_eq!(unpack_limits(packed), (900., 600.));
    }

    #[test]
    fn a_fixed_window_never_offers_a_resize_edge() {
        for edge in [HTLEFT, HTRIGHT, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT] {
            assert_eq!(without_resize_edges(edge as LRESULT), HTBORDER as LRESULT);
        }
        for edge in [HTTOP, HTTOPLEFT, HTTOPRIGHT] {
            assert_eq!(without_resize_edges(edge as LRESULT), HTCAPTION as LRESULT);
        }
        assert_eq!(without_resize_edges(1), 1);
    }
}
