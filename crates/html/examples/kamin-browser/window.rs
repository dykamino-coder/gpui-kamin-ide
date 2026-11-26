//! Platform window controls without changing the embedded engine or GPUI.

use gpui::Window;

pub fn maximize_or_restore(window: &Window) {
    if window.is_fullscreen() {
        window.toggle_fullscreen();
    } else if window.is_maximized() {
        restore(window);
    } else {
        window.zoom_window();
    }
}

#[cfg(windows)]
fn restore(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{SW_RESTORE, ShowWindowAsync},
    };

    if let Ok(handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(handle) = handle.as_raw()
    {
        // GPUI's Windows zoom operation only maximizes; restoring uses the same owner HWND.
        unsafe {
            let _ = ShowWindowAsync(HWND(handle.hwnd.get() as *mut _), SW_RESTORE);
        }
    }
}

#[cfg(not(windows))]
fn restore(window: &Window) {
    window.zoom_window();
}
