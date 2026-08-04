//! Оверлеи главного окна: слои (`layers/*`), hit-регионы (`region`) и
//! win32-хелперы титлбара (`subclass`).
//!
//! Раньше здесь жило ВТОРОЕ прозрачное окно поверх главного — оверлеи иначе
//! накрывались child-окнами WebView2. С CEF страницы рисуются в кадр, и стек
//! перенесён в слой главного окна (`state/overlay_stack.rs`, Ф6 плана 101).

pub(crate) mod diag_states;
pub(crate) mod layers;
mod region;
mod subclass;
mod tool_menu;
mod tool_submenu;

pub use region::{
    dropdown_shadow, hit_area, hit_rects_snapshot, input_area, region_area, tooltip_region,
};
pub use subclass::{start_native_window_drag, toggle_main_maximize};

use std::sync::atomic::{AtomicIsize, Ordering};

/// HWND главного окна. Ставится в main после открытия; читают подложка web,
/// тосты и win32-хелперы титлбара.
pub(super) static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

/// Опубликовать HWND главного окна.
pub fn set_main_hwnd(hwnd: isize) {
    MAIN_HWND.store(hwnd, Ordering::Relaxed);
}

/// HWND главного окна (0 — ещё не найден).
pub fn main_hwnd_isize() -> isize {
    use windows::Win32::Foundation::HWND;
    let cached = MAIN_HWND.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let found = crate::probe::shot::find_window(false)
        .map(|h: HWND| h.0 as isize)
        .unwrap_or(0);
    if found != 0 {
        MAIN_HWND.store(found, Ordering::Relaxed);
    }
    found
}
