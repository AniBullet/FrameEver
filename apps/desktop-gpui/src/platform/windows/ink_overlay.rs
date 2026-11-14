use std::io;

use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TRANSPARENT,
    },
};

pub(super) fn extended_style(current: u32, interactive: bool) -> u32 {
    if interactive {
        current & !(WS_EX_TRANSPARENT | WS_EX_NOACTIVATE)
    } else {
        current | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_NOACTIVATE
    }
}

pub(super) fn set_interactive(hwnd: HWND, interactive: bool) -> io::Result<()> {
    let current = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    let wanted = extended_style(current, interactive);
    if wanted != current {
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted as isize);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
    let applied = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    if applied & WS_EX_TRANSPARENT == wanted & WS_EX_TRANSPARENT {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "ink overlay input style was not applied: {applied:#x}"
        )))
    }
}
