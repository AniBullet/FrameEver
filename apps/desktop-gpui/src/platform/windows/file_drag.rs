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

fn drop_list(paths: &[PathBuf]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(DROPFILES_SIZE);
    bytes.extend_from_slice(&(DROPFILES_SIZE as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    let units = paths
        .iter()
        .flat_map(|path| path.as_os_str().encode_wide().chain([0]))
        .chain([0]);
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

unsafe fn global_bytes(bytes: &[u8]) -> windows::core::Result<HGLOBAL> {
    unsafe {
        let global = GlobalAlloc(GMEM_MOVEABLE, bytes.len())?;
        let target = GlobalLock(global).cast::<u8>();
        if target.is_null() {
            GlobalFree(Some(global))?;
            return Err(windows::core::Error::from_thread());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
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
        let mut medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            ..Default::default()
        };
        medium.u.hGlobal = global_bytes(&drop_list(paths))?;
        data.SetData(&format, &medium, true)?;
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
        let bytes = drop_list(&[PathBuf::from(r"C:\a.mp4"), PathBuf::from(r"D:\帧.png")]);
        assert_eq!(&bytes[..4], &20_u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1_i32.to_le_bytes());
        let units: Vec<u16> = bytes[20..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        let expected: Vec<u16> = "C:\\a.mp4\0D:\\帧.png\0\0".encode_utf16().collect();
        assert_eq!(units, expected);
    }
}
