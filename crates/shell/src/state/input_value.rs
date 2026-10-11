//! Программная запись значения инпута с событием `Change`.
//!
//! gpui-component 0.7.1: `set_value` больше не шлёт `InputEvent::Change`, а
//! подписчики (dirty таба, фильтр палитр, адресная строка) ждут его, как в
//! 0.5.1. Здесь — одна точка, которая шлёт событие сама.

use gpui::{Context, Entity, Window};
use gpui_component::input::{EditorState, InputEvent, InputState};

/// `set_value` + `InputEvent::Change` для однострочного инпута.
pub(crate) fn set_value_emit<V>(
    input: &Entity<InputState>,
    text: String,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    input.update(cx, |st, cx| {
        st.set_value(text, window, cx);
        cx.emit(InputEvent::Change);
    });
}

/// То же для код-редактора (`EditorState`).
pub(crate) fn set_editor_value_emit<V>(
    input: &Entity<EditorState>,
    text: String,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    input.update(cx, |st, cx| {
        st.set_value(text, window, cx);
        cx.emit(InputEvent::Change);
    });
}
