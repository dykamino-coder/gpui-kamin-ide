//! Фокус клавиатуры между веб-вью и окном приложения.
//!
//! Offscreen-браузер не видит, активно ли окно: фокус он теряет, только когда
//! ему сказали `set_focus(false)`. Без этого каретка в поле ввода мигала и
//! при свёрнутом или неактивном окне, и каждое мигание — два кадра страницы в
//! секунду, каждый из которых перерисовывал окно целиком (INC-2026-0064).
//! Chromium в своём окне делает так же: окно ушло из фокуса — виджет страницы
//! теряет фокус и каретка останавливается, вернулось — фокус возвращается тому
//! же вью.

use std::sync::Mutex;

/// Вью, которому последним отдали фокус клавиатуры.
static FOCUSED: Mutex<Option<String>> = Mutex::new(None);

/// Отдать фокус вью, забрав его у прежнего: без явного `set_focus(false)`
/// каретка в прежнем вью продолжала мигать и ловить ввод.
pub(crate) fn focus_view(id: &str) {
    let prev = FOCUSED
        .lock()
        .map(|mut f| f.replace(id.to_string()))
        .unwrap_or(None);
    if let Some(prev) = prev
        && prev != id
    {
        super::input::set_focus(&prev, false);
    }
    super::input::set_focus(id, true);
}

/// Клик мимо всех вью: забрать фокус у последнего (клик в наш интерфейс).
pub(crate) fn blur_all() {
    let prev = FOCUSED.lock().map(|mut f| f.take()).unwrap_or(None);
    if let Some(prev) = prev {
        super::input::set_focus(&prev, false);
    }
}

/// Клик мимо всех веб-вью: забрать у страницы фокус клавиатуры, иначе каретка
/// в ней продолжает мигать и ловить ввод, когда пользователь ушёл в наш UI.
pub(crate) fn blur_if_outside(x: f32, y: f32) {
    if !super::element::hit_any_view(x, y) {
        blur_all();
    }
}

/// Окно стало активным или перестало им быть. Владелец фокуса не меняется:
/// вернувшись в окно, пользователь продолжает в том же поле.
pub(crate) fn window_active(active: bool) {
    let owner = FOCUSED.lock().ok().and_then(|f| f.clone());
    if let Some(id) = owner {
        super::input::set_focus(&id, active);
    }
}
