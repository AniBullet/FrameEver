use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::Win32::Foundation::{GlobalFree, HGLOBAL, HWND};
use windows::Win32::System::Com::{
    DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{CF_HDROP, DROPEFFECT_COPY};
use windows::Win32::UI::Shell::{DROPFILES, SHCreateDataObject, SHDoDragDrop};

const DROPFILES_SIZE: usize = std::mem::size_of::<DROPFILES>();

fn drop_list_header() -> DROPFILES {
    DROPFILES {
        pFiles: DROPFILES_SIZE as u32,
        fWide: true.into(),
        ..Default::default()
    }
}

fn drop_list_names(paths: &[PathBuf]) -> Vec<u16> {
    paths
        .iter()
        .flat_map(|path| path.as_os_str().encode_wide().chain([0]))
        .chain([0])
        .collect()
}

unsafe fn global_drop_list(paths: &[PathBuf]) -> windows::core::Result<HGLOBAL> {
    let names = drop_list_names(paths);
    let names_size = std::mem::size_of_val(names.as_slice());
    unsafe {
        let global = GlobalAlloc(GMEM_MOVEABLE, DROPFILES_SIZE + names_size)?;
        let target = GlobalLock(global).cast::<u8>();
        if target.is_null() {
            let error = windows::core::Error::from_thread();
            GlobalFree(Some(global)).ok();
            return Err(error);
        }
        target
            .cast::<DROPFILES>()
            .write_unaligned(drop_list_header());
        std::ptr::copy_nonoverlapping(
            names.as_ptr().cast::<u8>(),
            target.add(DROPFILES_SIZE),
            names_size,
        );
        GlobalUnlock(global).ok();
        Ok(global)
    }
}

unsafe fn files_data_object(paths: &[PathBuf]) -> windows::core::Result<IDataObject> {
    unsafe {
        let data: IDataObject = SHCreateDataObject(None, None, None)?;
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let global = global_drop_list(paths)?;
        let mut medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            ..Default::default()
        };
        medium.u.hGlobal = global;
        if let Err(error) = data.SetData(&format, &medium, true) {
            GlobalFree(Some(global)).ok();
            return Err(error);
        }
        Ok(data)
    }
}

pub fn drag_files_out(
    source: Option<super::NativeWindow>,
    paths: &[PathBuf],
) -> Result<(), String> {
    let paths = paths
        .iter()
        .map(|path| super::super::clipboard_file_path(path))
        .collect::<Result<Vec<_>, String>>()?;
    let hwnd = source.map_or(HWND::default(), |source| HWND(source.hwnd()));
    unsafe {
        let data = files_data_object(&paths).map_err(|error| error.to_string())?;
        SHDoDragDrop(Some(hwnd), &data, None, DROPEFFECT_COPY)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drop_list_holds_every_path_in_wide_characters_and_ends_with_two_nulls() {
        let header = drop_list_header();
        assert_eq!({ header.pFiles }, 20);
        assert!({ header.fWide }.as_bool());
        assert!(!{ header.fNC }.as_bool());
        let names = drop_list_names(&[PathBuf::from(r"C:\a.mp4"), PathBuf::from(r"D:\帧.png")]);
        let expected: Vec<u16> = "C:\\a.mp4\0D:\\帧.png\0\0".encode_utf16().collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn the_drop_list_memory_starts_with_the_header_and_then_the_names() {
        let paths = [PathBuf::from(r"C:\a.mp4")];
        unsafe {
            let global = global_drop_list(&paths).unwrap();
            let bytes = GlobalLock(global).cast::<u8>();
            let header = bytes.cast::<DROPFILES>().read_unaligned();
            let names =
                std::slice::from_raw_parts(bytes.add(DROPFILES_SIZE).cast::<u16>(), 10).to_vec();
            GlobalUnlock(global).ok();
            GlobalFree(Some(global)).ok();
            assert_eq!({ header.pFiles }, 20);
            assert_eq!(names, "C:\\a.mp4\0\0".encode_utf16().collect::<Vec<u16>>());
        }
    }
}
