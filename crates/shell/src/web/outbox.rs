//! Очередь сообщений «приложение → страница».
//!
//! Передавать пачку внутри текста скрипта нельзя: на многомегабайтной строке
//! Chromium умирает целиком (замерено — процесс падал через десяток секунд
//! после начала обмена). Поэтому странице уходит короткая команда, а тело она
//! забирает обычным запросом — тем же путём, которым ей отдаётся HTML.
//!
//! Очередь копится ПО ВЬЮ и отдаётся целиком: расширение шлёт сообщения
//! пачками и быстрее, чем страница успевает за ними прийти. Нумеровать пачки
//! и чистить старые было ошибкой — чат навсегда застревал на «восстанавливаю
//! сессию», потому что его ответы выбрасывались непрочитанными.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static PENDING: LazyLock<Mutex<HashMap<String, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Поставить пачку в очередь вью. `json` — массив сообщений.
pub(crate) fn push(view_id: &str, json: String) {
    if let Ok(mut map) = PENDING.lock() {
        map.entry(view_id.to_string()).or_default().push(json);
    }
}

/// Забрать всё накопленное для вью одним массивом. Порядок сохраняется.
pub(crate) fn take_all(view_id: &str) -> String {
    let batches = PENDING
        .lock()
        .ok()
        .and_then(|mut m| m.remove(view_id))
        .unwrap_or_default();
    // Каждая пачка — уже массив; склеиваем их в один, снимая внешние скобки.
    let inner: Vec<&str> = batches
        .iter()
        .map(|b| b.trim())
        .map(|b| b.strip_prefix('[').unwrap_or(b))
        .map(|b| b.strip_suffix(']').unwrap_or(b))
        .filter(|b| !b.is_empty())
        .collect();
    format!("[{}]", inner.join(","))
}
