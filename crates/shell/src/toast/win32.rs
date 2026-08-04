//! Win32-часть тостов: рабочая область, перемещение окна, курсор, фокус.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::toast::fallback_area;
use gpui::App;

/// Рабочая область монитора ПОД КУРСОРОМ (`work_area_logical`): тосты
/// обходят таскбар и садятся на активный экран пользователя.
#[cfg(windows)]
pub(crate) fn work_area(cx: &App) -> (f32, f32, f32, f32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    // Win32 отдаёт ФИЗИЧЕСКИЕ px, а окна gpui позиционируются логическими
    let scale = match crate::probe::shot::find_window(false) {
        Some(hwnd) => {
            use windows::Win32::UI::HiDpi::GetDpiForWindow;
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
        }
        None => return fallback_area(cx),
    };
    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return fallback_area(cx);
        }
        let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or(40),
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return fallback_area(cx);
        }
        let r = info.rcWork;
        (
            r.left as f32 / scale,
            r.top as f32 / scale,
            (r.right - r.left) as f32 / scale,
            (r.bottom - r.top) as f32 / scale,
        )
    }
}
#[cfg(not(windows))]
pub(crate) fn work_area(cx: &App) -> (f32, f32, f32, f32) {
    fallback_area(cx)
}
/// Подвинуть окно тоста: API перемещения у gpui нет (только `resize`),
/// поэтому дергаем `SetWindowPos` по сырому HWND окна.
#[cfg(windows)]
pub(crate) fn move_window(window: &gpui::Window, x: f32, y: f32) {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos,
    };

    // `Window::window_handle` — инherent-метод gpui (AnyWindowHandle),
    // сырой HWND даёт только трейт
    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(isize::from(h.hwnd) as *mut std::ffi::c_void);
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            (x * scale) as i32,
            (y * scale) as i32,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}
#[cfg(not(windows))]
pub(crate) fn move_window(_window: &gpui::Window, _x: f32, _y: f32) {}
/// Курсор внутри окна тоста? `on_hover` gpui снимает ховер только по
/// движению ВНУТРИ окна: когда указатель уходит на чужое окно, событие
/// не приходит вовсе и карточка навсегда остаётся «под мышью» — таймер
/// не возобновляется (поймано замером полосы). Поэтому опрашиваем сами.
#[cfg(windows)]
pub fn cursor_over(hwnd_raw: isize) -> bool {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};
    let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return false;
        }
        let mut r = RECT::default();
        if GetWindowRect(hwnd, &mut r).is_err() {
            return false;
        }
        pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom
    }
}
#[cfg(not(windows))]
pub fn cursor_over(_hwnd_raw: isize) -> bool {
    false
}
/// Сырой HWND окна (для опроса курсора).
#[cfg(windows)]
pub fn raw_hwnd(window: &gpui::Window) -> isize {
    use raw_window_handle::RawWindowHandle;
    match raw_window_handle::HasWindowHandle::window_handle(window) {
        Ok(h) => match h.as_raw() {
            RawWindowHandle::Win32(w) => isize::from(w.hwnd),
            _ => 0,
        },
        Err(_) => 0,
    }
}
#[cfg(not(windows))]
pub fn raw_hwnd(_window: &gpui::Window) -> isize {
    0
}
/// Клик по карточке отдаёт фокус главному окну (`focus_main`).
#[cfg(windows)]
pub fn focus_main() {
    use windows::Win32::UI::WindowsAndMessaging::{SW_RESTORE, SetForegroundWindow, ShowWindow};
    if let Some(hwnd) = crate::probe::shot::find_window(false) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}
#[cfg(not(windows))]
pub fn focus_main() {}
