//! App-prefs хоста (`kamin:prefs:get`) — читаем в фоне, отдаём событием.
//!
//! Отдельный запрос, а не часть `status`: раньше префы добирались лениво при
//! первом открытии Customize, и этого хватало, пока их читала только сама
//! панель. `skipDeleteConfirm` спрашивают на ПЕРВОМ удалении сессии — до
//! Customize пользователь может не дойти ни разу за запуск, и настройка,
//! включённая вчера, сегодня читалась бы как выключенная.

use smol::channel::Sender;

use crate::host::events::CzEvent;
use crate::host_link::{self, ShellEvent};

/// Прочитать app-prefs хоста и разослать `PrefsLoaded`.
pub fn request_app_prefs(tx: Sender<ShellEvent>) {
    std::thread::spawn(move || {
        let Some(client) = host_link::client() else {
            return;
        };
        let Ok(v) = client.request("kamin:prefs:get", vec![]) else {
            return;
        };
        // Дефолты — те же, что в `services/app-prefs.ts`: ответ старого хоста
        // (без нового ключа) не должен молча включать пропуск подтверждения.
        let flag = |key: &str, default: bool| {
            v.get(key)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(default)
        };
        let _ = tx.try_send(ShellEvent::Cz(CzEvent::PrefsLoaded(
            flag("backgroundToasts", true),
            flag("useConptyDll", false),
            flag("skipDeleteConfirm", false),
        )));
    });
}
