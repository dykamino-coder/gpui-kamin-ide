//! Действия над сессиями: активация, создание, данные контекст-меню.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host_link::{self};
use kamin_metrics as m;
use kamin_model::Session;
use serde_json::json;

/// Активация сессии (общая для сайдбара и табов титлбара).
pub fn activate_session(id: String) {
    std::thread::spawn(move || {
        if let Some(client) = host_link::client()
            && let Err(e) = client.request("kamin:sessions:setActive", vec![json!(id)])
        {
            eprintln!("setActive failed: {e}");
        }
    });
}
pub(crate) fn new_no_folder_session() {
    std::thread::spawn(|| {
        if let Some(client) = host_link::client() {
            let _ = client.request("kamin:sessions:newNoFolderSession", vec![]);
        }
    });
}
/// SessionMenuData из сессии (для контекст-меню; используют сайдбар И табы).
pub fn menu_data(s: &Session) -> crate::ui::context_menu::SessionMenuData {
    crate::ui::context_menu::SessionMenuData {
        id: s.id.clone(),
        name: s.name.clone(),
        open: s.open,
        pinned: s.pinned,
        color: s.color.clone(),
    }
}
/// Строка сессии — .row 24px, padding 0 8 0 16, dot 4px, active-градиент.
/// `rename` = Some(инпут), если строка в inline-переименовании.
/// Сколько ширины сайдбара НЕ достаётся метке строки сессии.
///
/// Раньше стояло подогнанное по одному кадру число 102.4 — оно разъезжалось
/// при другой ширине сайдбара и при закреплённой сессии (у той видна кнопка
/// пина). Теперь слагаемые взяты из CSS оригинала
/// (`SessionItem.module.css`) поимённо: `.row { padding: 0 8 0 16 }` и рамка
/// 1px с каждой стороны, `.dot` 4 + `gap` 8, `.time` (ширина строки вроде
/// «46d» на fs-xs 11 semibold) + свой `gap` 8, `.pin` 20 + `gap` 8 у
/// закреплённых.
pub(crate) fn label_deduction(s: &Session) -> f32 {
    const ROW_INSETS: f32 = 16.0 + 8.0 + 2.0;
    // Инсеты КОНТЕЙНЕРОВ вокруг строки (root SPACE_1×2 + list SPACE_1 слева
    // + SPACE_3 справа): sidebar_w приходит сырой шириной сайдбара, а строка
    // живёт внутри этих отступов — без вычета имя лезло на колонку времени.
    const LIST_INSETS: f32 = 8.0 + m::SPACE_1 + m::SPACE_3;
    const DOT: f32 = 4.0 + m::SPACE_2;
    const PIN: f32 = 20.0 + m::SPACE_2;
    let time = crate::ui::text_fit::approx_width(
        &crate::ui::time_fmt::relative_ms(s.last_opened),
        m::FS_XS,
    ) + m::SPACE_2;
    LIST_INSETS + ROW_INSETS + DOT + time + if s.pinned { PIN } else { 0.0 }
}
