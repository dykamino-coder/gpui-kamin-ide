//! CF_HDROP: файлы в OS-клипборде (обмен с Проводником).
//! Copy/Cut дерева дублирует путь сюда; Paste при пустом внутреннем
//! буфере читает отсюда (скопированное в Проводнике вставляется в дерево).

#![cfg(windows)]

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

/// DROPFILES-заголовок (shellapi.h): offset до списка путей + fWide.
#[repr(C)]
struct DropFiles {
    p_files: u32,
    pt: [i32; 2],
    f_nc: i32,
    f_wide: i32,
}

/// Пути из клипборда (пусто — нет CF_HDROP).
pub fn read_files() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        if OpenClipboard(None::<HWND>).is_err() {
            return out;
        }
        if let Ok(handle) = GetClipboardData(CF_HDROP.0 as u32) {
            let hdrop = HDROP(handle.0);
            let count = DragQueryFileW(hdrop, u32::MAX, None);
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None);
                let mut buf = vec![0u16; len as usize + 1];
                let got = DragQueryFileW(hdrop, i, Some(&mut buf));
                if got > 0 {
                    out.push(String::from_utf16_lossy(&buf[..got as usize]));
                }
            }
        }
        let _ = CloseClipboard();
    }
    out
}

/// Положить пути как CF_HDROP (double-null-terminated UTF-16 список).
pub fn write_files(paths: &[String]) -> Result<(), String> {
    let mut wide: Vec<u16> = Vec::new();
    for p in paths {
        wide.extend(p.replace('/', "\\").encode_utf16());
        wide.push(0);
    }
    wide.push(0);
    let header = std::mem::size_of::<DropFiles>();
    let total = header + wide.len() * 2;
    unsafe {
        let hmem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, total).map_err(|e| e.to_string())?;
        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            return Err("GlobalLock failed".into());
        }
        let df = ptr as *mut DropFiles;
        (*df).p_files = header as u32;
        (*df).pt = [0, 0];
        (*df).f_nc = 0;
        (*df).f_wide = 1;
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr.add(header), wide.len() * 2);
        let _ = GlobalUnlock(hmem);
        OpenClipboard(None::<HWND>).map_err(|e| e.to_string())?;
        let _ = EmptyClipboard();
        let res = SetClipboardData(CF_HDROP.0 as u32, Some(HANDLE(hmem.0)));
        let _ = CloseClipboard();
        res.map(|_| ()).map_err(|e| e.to_string())
    }
}
