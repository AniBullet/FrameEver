use std::io;

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            GetWindowDisplayAffinity, MA_NOACTIVATE, SetWindowDisplayAffinity,
            WDA_EXCLUDEFROMCAPTURE, WM_MOUSEACTIVATE, WM_NCDESTROY,
        },
    },
};

const SUBCLASS_ID: usize = 0x4645_4344;

pub(super) fn prepare(hwnd: HWND, interactive: bool) -> io::Result<()> {
    if let Some(reason) = super::capture_exclusion::streamed_display_reason() {
        return Err(io::Error::other(format!(
            "capture exclusion unavailable: {reason}"
        )));
    }
    if unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut affinity = 0;
    if unsafe { GetWindowDisplayAffinity(hwnd, &mut affinity) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if affinity != WDA_EXCLUDEFROMCAPTURE {
        return Err(io::Error::other(format!(
            "capture exclusion was not confirmed: affinity={affinity}"
        )));
    }
    if !interactive && unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, 0) } == 0 {
        return Err(io::Error::other(
            "could not keep countdown panel non-activating",
        ));
    }
    Ok(())
}

unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    if message == WM_NCDESTROY {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID);
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
