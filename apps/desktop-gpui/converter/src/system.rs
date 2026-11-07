use std::path::{Component, Path, PathBuf};

use frameever_converter::find_installs_on_this_machine;
use frameever_utils::brand::APP_NAME;

use crate::wizard::System;

pub fn this_machine() -> System {
    System {
        find_installs: Box::new(find_installs_on_this_machine),
        running: Box::new(running_processes),
        volume: Box::new(volume),
        free_space: Box::new(free_space),
        installed_app: Box::new(installed_frameever),
    }
}

fn volume(path: &Path) -> Option<String> {
    match path.components().next()? {
        Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().to_uppercase()),
        _ => None,
    }
}

fn installed_from_display_icon(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(value.trim().trim_matches('"'));
    path.is_file().then_some(path)
}

#[cfg(windows)]
fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn running_processes() -> Vec<String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
    };
    let mut names = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return names;
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let end = entry
                .szExeFile
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(entry.szExeFile.len());
            names.push(String::from_utf16_lossy(&entry.szExeFile[..end]));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    names
}

#[cfg(windows)]
fn free_space(path: &Path) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let path = wide(path.as_os_str());
    let mut available = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(available)
}

#[cfg(windows)]
fn installed_frameever() -> Option<PathBuf> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
        },
    };
    let key = wide(std::ffi::OsStr::new(&format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{APP_NAME}"
    )));
    let value = wide(std::ffi::OsStr::new("DisplayIcon"));
    let read = |root: HKEY| -> Option<String> {
        let mut buffer = vec![0u16; 1024];
        let mut bytes = (buffer.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let end = buffer
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..end]))
    };
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .filter_map(read)
        .find_map(|icon| installed_from_display_icon(&icon))
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(|dir| {
                    PathBuf::from(dir)
                        .join(APP_NAME)
                        .join(format!("{APP_NAME}.exe"))
                })
                .filter(|path| path.is_file())
        })
}

#[cfg(windows)]
pub fn reduce_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut animates = 1;
    let read = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut animates).cast(), 0)
    };
    read != 0 && animates == 0
}

#[cfg(not(windows))]
pub fn reduce_motion() -> bool {
    false
}

#[cfg(not(windows))]
fn running_processes() -> Vec<String> {
    Vec::new()
}

#[cfg(not(windows))]
fn free_space(_: &Path) -> Option<u64> {
    None
}

#[cfg(not(windows))]
fn installed_frameever() -> Option<PathBuf> {
    None
}
