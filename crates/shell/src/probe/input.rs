//! probe `click`/`drag` — синтетический ввод мыши БЕЗ курсора/foreground:
//! WM_MOUSEMOVE/WM_LBUTTONDOWN/WM_LBUTTONUP через PostMessage прямо в main
//! HWND. Координаты приходят в ЛОГИЧЕСКИХ px client-области (та же система,
//! что probe_registry bounds) → ×scale (GetDpiForWindow/96) в lparam.
//! Разблокирует live-верификацию pointer-механик (reorder, dnd, сплиттеры).

#![cfg(windows)]

pub use crate::probe::keys::{press_key_to, type_text_to};

use windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    PostMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
};

/// MK_LBUTTON для wparam зажатой ЛКМ.
const MK_LBUTTON: usize = 0x0001;

fn lparam(x: i32, y: i32) -> LPARAM {
    LPARAM(((y as isize) << 16) | (x as isize & 0xffff))
}

fn post(msg: u32, wparam: usize, x: i32, y: i32) -> Result<(), String> {
    post_to(false, msg, wparam, x, y)
}

fn post_to(overlay: bool, msg: u32, wparam: usize, x: i32, y: i32) -> Result<(), String> {
    // Ф6: отдельного overlay-окна больше нет — «overlay»-цель шлём в главное
    // окно (оверлеи живут его слоем). Параметр оставлен ради старых стендов.
    let _ = overlay;
    let hwnd = crate::probe::shot::find_window(false).ok_or("window not found")?;
    unsafe { PostMessageW(Some(hwnd), msg, WPARAM(wparam), lparam(x, y)) }
        .map_err(|e| e.to_string())
}

/// ЛКМ-клик в OVERLAY-окне (меню/поповеры/модалки живут там).
/// Координаты — те же логические px (overlay накрывает main 1:1).
pub fn click_overlay(x: f32, y: f32) -> Result<(), String> {
    let (px, py) = scaled(x, y)?;
    post_to(true, WM_MOUSEMOVE, 0, px, py)?;
    sleep_ms(30);
    post_to(true, WM_LBUTTONDOWN, MK_LBUTTON, px, py)?;
    sleep_ms(30);
    post_to(true, WM_LBUTTONUP, 0, px, py)?;
    Ok(())
}

/// Логические px → клиентские физические px (lparam-система WndProc).
fn scaled(x: f32, y: f32) -> Result<(i32, i32), String> {
    let hwnd = crate::probe::shot::find_window(false).ok_or("main window not found")?;
    let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0;
    Ok(((x * scale).round() as i32, (y * scale).round() as i32))
}

pub(crate) fn sleep_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

/// Колесо в точке (лог. px): lines>0 = вверх. ⚠ WM_MOUSEWHEEL несёт
/// ЭКРАННЫЕ координаты (gpui делает ScreenToClient) — не client, как кнопки.
pub fn scroll(x: f32, y: f32, lines: i32) -> Result<(), String> {
    let hwnd = crate::probe::shot::find_window(false).ok_or("main window not found")?;
    let (px, py) = scaled(x, y)?;
    let mut pt = POINT { x: px, y: py };
    unsafe {
        let _ = ClientToScreen(hwnd, &mut pt);
    }
    let delta = (lines * 120) as i16; // WHEEL_DELTA
    let wparam = ((delta as u16 as usize) << 16) & 0xffff0000;
    unsafe {
        PostMessageW(
            Some(hwnd),
            WM_MOUSEWHEEL,
            WPARAM(wparam),
            lparam(pt.x, pt.y),
        )
    }
    .map_err(|e| e.to_string())
}

/// Изменить размер ГЛАВНОГО окна (логические px). Нужен парити-гейту:
/// адаптер вьюпорта (масштабирование колонок, пол размеров, дебаунс) иначе
/// не проверяется вовсе.
pub fn resize_main(w: f32, h: f32) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos, ShowWindow,
    };
    let hwnd = crate::probe::shot::find_window(false).ok_or("main window not found")?;
    let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0;
    let scale = if scale <= 0.0 { 1.0 } else { scale };
    unsafe {
        // Развёрнутое окно SetWindowPos игнорирует
        let _ = ShowWindow(hwnd, SW_RESTORE);
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            (w * scale) as i32,
            (h * scale) as i32,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .map_err(|e| e.to_string())
    }
}

/// Ховер (только WM_MOUSEMOVE) — проверка тултипов/hover-состояний.
pub fn hover(x: f32, y: f32) -> Result<(), String> {
    let (px, py) = scaled(x, y)?;
    post(WM_MOUSEMOVE, 0, px, py)
}

/// ЛКМ-клик в точке (лог. px client-области).
pub fn click(x: f32, y: f32) -> Result<(), String> {
    let (px, py) = scaled(x, y)?;
    post(WM_MOUSEMOVE, 0, px, py)?;
    sleep_ms(30);
    post(WM_LBUTTONDOWN, MK_LBUTTON, px, py)?;
    sleep_ms(30);
    post(WM_LBUTTONUP, 0, px, py)?;
    Ok(())
}

/// ПКМ-клик (контекст-меню редактора и т.п.).
pub fn right_click(x: f32, y: f32) -> Result<(), String> {
    const WM_RBUTTONDOWN: u32 = 0x0204;
    const WM_RBUTTONUP: u32 = 0x0205;
    const MK_RBUTTON: usize = 0x0002;
    let (px, py) = scaled(x, y)?;
    post(WM_MOUSEMOVE, 0, px, py)?;
    sleep_ms(30);
    post(WM_RBUTTONDOWN, MK_RBUTTON, px, py)?;
    sleep_ms(30);
    post(WM_RBUTTONUP, 0, px, py)?;
    Ok(())
}

/// Драг в ЗАДАННОЕ окно. Без этого «нажал внутри оверлея → отпустил снаружи»
/// было непроверяемо: `drag` всегда слал в главное окно, и тест мерил не то
/// (ревью ц.28).
/// Тот же жест, но БЕЗ отпускания кнопки: нужен, чтобы снять кадр В ПРОЦЕССЕ
/// перетаскивания (артефакты ресайза видны только пока ручка зажата).
pub fn drag_hold(overlay: bool, x1: f32, y1: f32, x2: f32, y2: f32) -> Result<(), String> {
    let (px1, py1) = scaled(x1, y1)?;
    let (px2, py2) = scaled(x2, y2)?;
    post_to(overlay, WM_MOUSEMOVE, 0, px1, py1)?;
    sleep_ms(30);
    post_to(overlay, WM_LBUTTONDOWN, MK_LBUTTON, px1, py1)?;
    sleep_ms(40);
    const STEPS: i32 = 12;
    for i in 1..=STEPS {
        let t = i as f32 / STEPS as f32;
        let (mx, my) = (
            px1 + ((px2 - px1) as f32 * t) as i32,
            py1 + ((py2 - py1) as f32 * t) as i32,
        );
        post_to(overlay, WM_MOUSEMOVE, MK_LBUTTON, mx, my)?;
        sleep_ms(15);
    }
    Ok(())
}

/// Отпустить кнопку там, где сейчас курсор (пара к [`drag_hold`]).
pub fn drag_release(overlay: bool, x: f32, y: f32) -> Result<(), String> {
    let (px, py) = scaled(x, y)?;
    post_to(overlay, WM_LBUTTONUP, 0, px, py)?;
    Ok(())
}

pub fn drag_to(overlay: bool, x1: f32, y1: f32, x2: f32, y2: f32) -> Result<(), String> {
    let (px1, py1) = scaled(x1, y1)?;
    let (px2, py2) = scaled(x2, y2)?;
    post_to(overlay, WM_MOUSEMOVE, 0, px1, py1)?;
    sleep_ms(30);
    post_to(overlay, WM_LBUTTONDOWN, MK_LBUTTON, px1, py1)?;
    sleep_ms(40);
    const STEPS: i32 = 12;
    for i in 1..=STEPS {
        let t = i as f32 / STEPS as f32;
        let (mx, my) = (
            px1 + ((px2 - px1) as f32 * t) as i32,
            py1 + ((py2 - py1) as f32 * t) as i32,
        );
        post_to(overlay, WM_MOUSEMOVE, MK_LBUTTON, mx, my)?;
        sleep_ms(15);
    }
    sleep_ms(40);
    post_to(overlay, WM_LBUTTONUP, 0, px2, py2)?;
    Ok(())
}
