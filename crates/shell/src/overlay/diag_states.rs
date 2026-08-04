//! Публикация активных оверлей-состояний для probe-команды `overlay`.
//!
//! Вынесено из `mod.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::overlay::region::apply_window_region;
use crate::root::RootView;

/// Собирает список активных оверлеев кадра и отдаёт его в probe-реестр.
/// Пустой список = ни одного `hit_area` в кадре → paint-хук не вызовется,
/// поэтому окно режется в ноль прямо здесь.
pub(crate) fn publish_states(r: &RootView, window: &gpui::Window) {
    let _ = window;
    let mut st: Vec<&'static str> = Vec::new();
    if !r.toasts.is_empty() {
        st.push("toasts");
    }
    if r.session_menu.is_some() {
        st.push("session_menu");
    }
    if r.file_menu.is_some() {
        st.push("file_menu");
    }
    if r.ed.editor_tab_menu.is_some() {
        st.push("editor_tab_menu");
    }
    if r.layout_popover {
        st.push("layout_popover");
    }
    if r.appearance_popover {
        st.push("appearance_popover");
    }
    if r.tool_tab_menu.is_some() {
        st.push("tool_tab_menu");
    }
    if r.quick_pick.is_some() {
        st.push("quick_pick");
    }
    if r.tool_picker.is_some() {
        st.push("tool_picker");
    }
    if r.modal.is_some() {
        st.push("modal");
    }
    if r.tabs_overflow_open.is_some() {
        st.push("tabs_overflow");
    }
    if r.new_session_menu.is_some() {
        st.push("new_session_menu");
    }
    if r.palette_open {
        st.push("palette");
    }
    if r.sov.quickopen_open {
        st.push("quickopen");
    }
    if r.sov.fif_open {
        st.push("fif");
    }
    if r.sov.ws_open {
        st.push("ws");
    }
    if r.hover_pill.is_some() {
        st.push("hover_pill");
    }
    let empty = st.is_empty();
    crate::probe::registry::set_overlay_states(st);
    if empty {
        // Ни одного hit_area в кадре → paint-хук не вызовется —
        // резать окно в ноль прямо здесь
        #[cfg(windows)]
        apply_window_region(window.scale_factor());
    }
}
