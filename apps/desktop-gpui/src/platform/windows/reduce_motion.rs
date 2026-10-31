use std::io;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW, WM_NCDESTROY,
    WM_SETTINGCHANGE,
};

const SUBCLASS_ID: usize = 0x4652_4D4F;

type OnChange = Box<dyn Fn()>;

pub fn follow(cx: &mut gpui::App) {
    cx.set_reduce_motion(system_reduce_motion());

    let (changed, changes) = flume::unbounded::<()>();
    let watching = listener_window().and_then(|hwnd| {
        watch(
            hwnd,
            Box::new(move || {
                changed.send(()).ok();
            }),
        )
    });
    if let Err(error) = watching {
        tracing::warn!(%error, "reduced motion changes will need a restart");
        return;
    }
    cx.spawn(async move |cx| {
        while changes.recv_async().await.is_ok() {
            cx.update(|cx| cx.set_reduce_motion(system_reduce_motion()));
        }
    })
    .detach();
}

fn system_reduce_motion() -> bool {
    let mut animates = 1;
    let read = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut animates).cast(), 0)
    };
    let reduce = reduces_motion((read != 0).then_some(animates != 0));
    tracing::debug!(reduce, "system reduce motion");
    reduce
}

fn reduces_motion(client_area_animation: Option<bool>) -> bool {
    client_area_animation == Some(false)
}

fn listener_window() -> io::Result<HWND> {
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(hwnd)
}

fn watch(hwnd: HWND, on_change: OnChange) -> io::Result<()> {
    let callback = Box::into_raw(Box::new(on_change));
    if unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, callback as usize) } == 0 {
        drop(unsafe { Box::from_raw(callback) });
        return Err(io::Error::other("could not watch for settings changes"));
    }
    Ok(())
}

unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    let callback = reference_data as *mut OnChange;
    if message == WM_NCDESTROY {
        unsafe { RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID) };
        drop(unsafe { Box::from_raw(callback) });
    } else if message == WM_SETTINGCHANGE {
        unsafe { (*callback)() };
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyWindow, GA_PARENT, GetAncestor, GetDesktopWindow, IsWindowVisible, SendMessageW,
        WM_SETTINGCHANGE, WM_TIMECHANGE,
    };

    use super::*;

    #[test]
    fn motion_is_reduced_only_when_windows_says_animations_are_off() {
        assert!(reduces_motion(Some(false)));
        assert!(!reduces_motion(Some(true)));
        assert!(!reduces_motion(None));
    }

    #[test]
    fn the_listener_gets_broadcasts_and_stays_out_of_sight() {
        let hwnd: HWND = listener_window().unwrap();
        assert_eq!(
            unsafe { GetAncestor(hwnd, GA_PARENT) },
            unsafe { GetDesktopWindow() },
            "a message-only window never receives WM_SETTINGCHANGE"
        );
        assert_eq!(unsafe { IsWindowVisible(hwnd) }, 0);
        assert_ne!(unsafe { DestroyWindow(hwnd) }, 0);
    }

    #[test]
    fn a_settings_change_reports_and_other_messages_do_not() {
        let hwnd = listener_window().unwrap();
        let changes = Rc::new(Cell::new(0));
        let counted = changes.clone();
        watch(hwnd, Box::new(move || counted.set(counted.get() + 1))).unwrap();

        unsafe { SendMessageW(hwnd, WM_TIMECHANGE, 0, 0) };
        assert_eq!(changes.get(), 0);

        unsafe { SendMessageW(hwnd, WM_SETTINGCHANGE, 0, 0) };
        assert_eq!(changes.get(), 1);

        assert_ne!(unsafe { DestroyWindow(hwnd) }, 0);
    }
}
