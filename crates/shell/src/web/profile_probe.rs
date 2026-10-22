//! Занят ли профиль CEF другим процессом — без вызовов CEF: после неудачного
//! `initialize` трогать CEF нельзя (`cef_app.h:68-76`), а до него — незачем.
//!
//! Признаки владельца из Chromium `process_singleton_win.cc`: файл `lockfile`
//! в корне профиля, открытый им без `FILE_SHARE_WRITE` (и удаляемый при
//! закрытии), и message-окно `Chrome_MessageWindow` с путём профиля в
//! заголовке. `CefShutdown` снимает оба — даже если процесс ещё жив.

use std::path::Path;

/// Профиль держит другой процесс. Ошибка проверки — «свободен»: решает CEF,
/// а лишнее ожидание при каждом старте хуже одного отказа.
#[cfg(windows)]
pub(crate) fn busy(root: &Path) -> bool {
    lock_held(&root.join("lockfile")) || message_window(root)
}

#[cfg(not(windows))]
pub(crate) fn busy(_root: &Path) -> bool {
    false
}

/// Подождать (≤ `PRE_INIT_BUDGET`), пока профиль не отпустит другой процесс;
/// вернуть потраченные миллисекунды. Шаги решает `profile_gate::pre_init_step`.
pub(crate) fn wait_until_free(root: &Path) -> u128 {
    use super::profile_gate::{Gate, pre_init_step};
    let start = std::time::Instant::now();
    while let Gate::Wait(step) = pre_init_step(busy(root), start.elapsed()) {
        std::thread::sleep(step);
    }
    start.elapsed().as_millis()
}

#[cfg(windows)]
fn lock_held(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt as _;
    const SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    // Без create/truncate: открываем только существующий файл и ничего в нём
    // не меняем; запись нужна, чтобы владелец с FILE_SHARE_READ нас не пустил.
    match std::fs::OpenOptions::new()
        .write(true)
        .share_mode(SHARE_ALL)
        .open(path)
    {
        Ok(_) => false,
        Err(e) => e.raw_os_error() == Some(ERROR_SHARING_VIOLATION),
    }
}

#[cfg(windows)]
fn message_window(root: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, HWND_MESSAGE};
    use windows::core::{PCWSTR, w};
    let title: Vec<u16> = root
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: обе строки нуль-терминированы и живут до конца вызова.
    unsafe {
        FindWindowExW(
            Some(HWND_MESSAGE),
            None,
            w!("Chrome_MessageWindow"),
            PCWSTR(title.as_ptr()),
        )
    }
    .is_ok_and(|hwnd| !hwnd.is_invalid())
}

/// Дождаться выхода процесса `pid`, не дольше `timeout`. Нет такого процесса
/// (уже вышел) — сразу.
#[cfg(windows)]
pub(crate) fn wait_for_exit(pid: u32, timeout: std::time::Duration) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    // SAFETY: хэндл закрывается здесь же; ожидание ограничено таймаутом.
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else {
            return;
        };
        let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        let _ = WaitForSingleObject(handle, ms);
        let _ = CloseHandle(handle);
    }
}

#[cfg(not(windows))]
pub(crate) fn wait_for_exit(_pid: u32, _timeout: std::time::Duration) {}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn lockfile_held_without_write_share_is_busy() {
        use std::os::windows::fs::OpenOptionsExt as _;
        let dir = std::env::temp_dir().join(format!("kamin-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!busy(&dir), "no lockfile");
        let lock = dir.join("lockfile");
        // Как владелец Chromium: GENERIC_WRITE, FILE_SHARE_READ.
        let held = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(0x1)
            .open(&lock)
            .unwrap();
        assert!(busy(&dir));
        drop(held);
        assert!(!busy(&dir), "released lockfile");
        std::fs::remove_file(&lock).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
