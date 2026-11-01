pub struct FineTimer;

#[cfg(windows)]
impl FineTimer {
    pub fn new() -> Self {
        unsafe {
            windows_sys::Win32::Media::timeBeginPeriod(1);
        }
        Self
    }
}

#[cfg(windows)]
impl Drop for FineTimer {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Media::timeEndPeriod(1);
        }
    }
}

#[cfg(not(windows))]
impl FineTimer {
    pub fn new() -> Self {
        Self
    }
}
