//! Замена процесса kamin-host: новый хост — новый extension host и новые вью.
//!
//! Сайдкар поднимает НОВЫЙ kamin-host (свежие порт и токен), когда прежний
//! умер. `kamin:exthost:respawned` при этом не приходит: родитель шлёт его
//! только про перезапуск своего ребёнка, а у нового родителя ребёнок первый.
//! Отметки resolve прежнего хоста навсегда гасили запросы к новому, его
//! расширение не привязывало вью чата, и всё, что чат слал, терялось:
//! invoke висели, события не шли, сообщения оставались `Queued`
//! (INC-2026-0115).
//!
//! Цепочка повторов WS-клиента ведёт только СВОЙ хост: к новому подключает
//! `on_endpoint` сайдкара. Иначе оба пути подключались к новому хосту, и
//! каждое событие доходило до вью дважды.

use kamin_sidecar::HostEndpoint;
use serde_json::Value;
use smol::channel::Sender;

use crate::host_link::{ShellEvent, endpoint_slot, view_resolution};

/// Запомнить хост успешного подключения; другой хост — отвязать вью от прежнего.
pub(crate) fn adopt_endpoint(endpoint: &HostEndpoint, tx: &Sender<ShellEvent>) {
    let previous = endpoint_slot().lock().unwrap().replace(endpoint.clone());
    if previous.is_some_and(|p| p != *endpoint) {
        host_replaced(tx);
    }
}

fn host_replaced(tx: &Sender<ShellEvent>) {
    view_resolution().lock().unwrap().reset();
    // Живые страницы держат состояние расширения умершего хоста: id вкладок,
    // ожидающие ответа invoke, номера переключений. Новый хост восстанавливает
    // вкладки заново, как на холодном старте, поэтому и страница обязана
    // начать с нуля, даже если HTML тот же.
    crate::ui::chat_webview::reload_on_next_html();
    // У нового ребёнка пустые зеркала документов — тот же посев, что после
    // перезапуска ребёнка (BR-04, `state/events/connection.rs`).
    let _ = tx.try_send(ShellEvent::HostEvent(
        "kamin:exthost:respawned".into(),
        Value::Null,
    ));
}

/// Куда повторять подключение: только к тому же хосту, что и раньше.
pub(crate) fn retry_target(
    snapshot: Option<HostEndpoint>,
    own: &HostEndpoint,
) -> Option<HostEndpoint> {
    snapshot.filter(|next| next == own)
}

#[cfg(test)]
#[path = "rebind_tests.rs"]
mod tests;
