//! Сабклассы WndProc: per-pixel hit-test overlay и зеркалирование состояния main.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use super::MAIN_HWND;
use std::sync::atomic::{AtomicIsize, Ordering};

/// HWND overlay-окна. Ставится overlay-окном при первом (видимом) рендере.
pub(super) static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);
/// Запомнить HWND главного окна (main.rs).
/// Нативный драг окна за титлбар: gpui start_window_move на Windows —
/// НЕ реализован (только wayland/x11). ReleaseCapture + WM_NCLBUTTONDOWN
/// HTCAPTION = системный caption-drag (restore-under-cursor, Snap — всё
/// как у нативного заголовка).
#[cfg(windows)]
pub fn start_native_window_drag() {
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetWindowRect, IsZoomed, PostMessageW, SW_RESTORE, SWP_NOSIZE, SWP_NOZORDER,
        SetWindowPos, ShowWindow, WM_SYSCOMMAND,
    };
    let h = MAIN_HWND.load(Ordering::Relaxed);
    if h == 0 {
        return;
    }
    let hwnd = HWND(h as *mut _);
    unsafe {
        let _ = ReleaseCapture();
        // Максимизированное: restore + окно ПОД КУРСОР (нативное поведение
        // caption-drag: restore-under-cursor), затем drag-move.
        if IsZoomed(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let mut r = RECT::default();
            let _ = GetWindowRect(hwnd, &mut r);
            let w = r.right - r.left;
            let _ = SetWindowPos(
                hwnd,
                None,
                pt.x - w / 3,
                pt.y - 16,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER,
            );
        }
        // SC_MOVE|HTCAPTION (0xF012) ЧЕРЕЗ PostMessage: SYSCOMMAND уходит в
        // DefWindowProc мимо NC-обработки gpui; Post (не Send!) — иначе
        // модальный цикл стартует внутри mouse-хендлера gpui → реентрантный
        // заём App («RefCell already borrowed», краш).
        let _ = PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(0xF012), LPARAM(0));
    }
}

#[cfg(not(windows))]
pub fn start_native_window_drag() {}

/// Развернуть/восстановить ГЛАВНОЕ окно. `Window::zoom_window()` у gpui на
/// Windows ВСЕГДА шлёт `SW_MAXIMIZE` (`window.rs: fn zoom`), восстановления
/// в нём нет вовсе — кнопка «restore» после максимайза не работала (баг
/// найден юзером). Нативный путь окна (`HTMAXBUTTON`) делает именно это.
#[cfg(windows)]
pub fn toggle_main_maximize() {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        IsZoomed, PostMessageW, SC_MAXIMIZE, SC_RESTORE, WM_SYSCOMMAND,
    };
    let h = MAIN_HWND.load(Ordering::Relaxed);
    if h == 0 {
        return;
    }
    let hwnd = HWND(h as *mut _);
    unsafe {
        // НЕ ShowWindow: кнопка зовёт нас ИЗНУТРИ обработчика gpui, и
        // синхронный ShowWindow доставлял WM_SIZE реентрантно — коллбэки
        // размера молча пропускались (RefCell окна занят), вьюпорт оставался
        // старым: окно на весь экран, контент в углу прежнего размера до
        // следующего события (снаружи, из probe-потока, работало — потому и
        // «у тебя ок, у меня нет»). PostMessage выполняет разворот на чистом
        // стеке своей очереди.
        let cmd = if IsZoomed(hwnd).as_bool() {
            SC_RESTORE
        } else {
            SC_MAXIMIZE
        };
        let _ = PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(cmd as usize), LPARAM(0));
    }
}

#[cfg(not(windows))]
pub fn toggle_main_maximize() {}
