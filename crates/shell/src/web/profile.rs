//! Профиль, в котором СОЗДАН браузер вью: общие текстуры или software.
//!
//! `sw_mode()` — решение для БУДУЩИХ браузеров, а у живого профиль зашит при
//! создании (`shared_texture_enabled`). Без отдельной записи ватчдог не мог
//! отличить вью, оставшиеся на общих текстурах после перехода в software, и
//! процесс жил в смешанном режиме: чат и консоль рисовали чёрный accelerated-
//! кадр (INC-2026-0073).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// `true` — браузер вью создан с общими текстурами.
static ACCELERATED: LazyLock<Mutex<HashMap<String, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Запомнить профиль заказанного браузера и вернуть его (`true` — общие
/// текстуры). Решение берётся из `sw_mode()` в момент заказа.
pub(crate) fn record(id: &str) -> bool {
    let accelerated = !super::sw_mode();
    if let Ok(mut map) = ACCELERATED.lock() {
        map.insert(id.to_string(), accelerated);
    }
    accelerated
}

/// Браузер вью закрыт: профиля больше нет.
pub(crate) fn forget(id: &str) {
    if let Ok(mut map) = ACCELERATED.lock() {
        map.remove(id);
    }
}

/// Создан ли ТЕКУЩИЙ браузер вью с общими текстурами. Нет браузера — `false`:
/// accelerated-кадр закрытого браузера с тем же id не должен перебить кадры
/// нового software-браузера.
pub(crate) fn accelerated(id: &str) -> bool {
    ACCELERATED
        .lock()
        .is_ok_and(|map| map.get(id).copied().unwrap_or(false))
}
