//! Форма курсора над веб-вью.
//!
//! В offscreen-режиме курсор окном управляем МЫ: CEF лишь сообщает желаемую
//! форму (`on_cursor_change`). Храним её по вью, а обёртка элемента вешает
//! соответствующий стиль gpui — над ссылкой рука, над текстом каретка.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static CURSORS: LazyLock<Mutex<HashMap<String, gpui::CursorStyle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Запомнить форму курсора вью. `true` — форма сменилась (нужен кадр).
pub(crate) fn set(id: &str, style: gpui::CursorStyle) -> bool {
    let Ok(mut map) = CURSORS.lock() else {
        return false;
    };
    if map.get(id) == Some(&style) {
        return false;
    }
    map.insert(id.to_string(), style);
    true
}

/// Текущая форма курсора вью.
pub(crate) fn style(id: &str) -> gpui::CursorStyle {
    CURSORS
        .lock()
        .ok()
        .and_then(|m| m.get(id).copied())
        .unwrap_or(gpui::CursorStyle::Arrow)
}

/// Забыть курсор закрытого вью.
pub(crate) fn forget_view(id: &str) {
    if let Ok(mut map) = CURSORS.lock() {
        map.remove(id);
    }
}

/// Тип курсора CEF → стиль gpui. Неизвестные формы — обычная стрелка.
pub(crate) fn from_cef(t: cef::sys::cef_cursor_type_t) -> gpui::CursorStyle {
    use cef::sys::cef_cursor_type_t as C;
    use gpui::CursorStyle as S;
    match t {
        C::CT_HAND => S::PointingHand,
        C::CT_IBEAM | C::CT_VERTICALTEXT => S::IBeam,
        C::CT_CROSS => S::Crosshair,
        C::CT_EASTRESIZE | C::CT_WESTRESIZE | C::CT_EASTWESTRESIZE | C::CT_COLUMNRESIZE => {
            S::ResizeLeftRight
        }
        C::CT_NORTHRESIZE | C::CT_SOUTHRESIZE | C::CT_NORTHSOUTHRESIZE | C::CT_ROWRESIZE => {
            S::ResizeUpDown
        }
        C::CT_NORTHEASTRESIZE | C::CT_SOUTHWESTRESIZE => S::ResizeUpRightDownLeft,
        C::CT_NORTHWESTRESIZE | C::CT_SOUTHEASTRESIZE => S::ResizeUpLeftDownRight,
        C::CT_GRAB => S::OpenHand,
        C::CT_GRABBING => S::ClosedHand,
        C::CT_NODROP | C::CT_NOTALLOWED => S::OperationNotAllowed,
        C::CT_COPY | C::CT_ALIAS => S::DragCopy,
        C::CT_CONTEXTMENU => S::ContextualMenu,
        _ => S::Arrow,
    }
}
