//! Реестр расширений и вебвью: HTML, доставка сообщений.
//!
//! Ветки перенесены дословно из `connect_ws` (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use crate::host::events::TermEvent;
use serde_json::Value;
use smol::channel::Sender;

use crate::host::connect::{KNOWN_WEBVIEWS, dynamic_webviews, webview_known};
use crate::host::status::request_status;
use crate::host_link::{ShellEvent, client, resolved_views, t0};

/// Имя параметра `on_event_tx` сохранено из замыкания: тела веток
/// перенесены дословно. `true` — канал обработан здесь.
pub(crate) fn handle(on_event_tx: &Sender<ShellEvent>, channel: &str, payload: &Value) -> bool {
    match channel {
        "kamin:registry:update" | "kamin:extensions:changed" => {
            // Хост присылает САМ СПИСОК вместе с событием — берём его
            // напрямую, без ответного RPC (он шёл 12-13 с, и «4 active»
            // появлялось через 35 с при активации меньше секунды).
            if let Some(arr) = payload.get("list").and_then(Value::as_array) {
                let items: Vec<crate::ui::extensions_panel::ExtDesc> = arr
                    .iter()
                    .filter_map(crate::ui::extensions_panel::ExtDesc::from_value)
                    .collect();
                let counts = crate::ui::status_bar::StatusCounts {
                    ext_active: items.iter().filter(|e| e.active).count(),
                    ext_failed: items.iter().filter(|e| e.activation_error).count(),
                    ext_disabled: items.iter().filter(|e| !e.enabled).count(),
                    ..Default::default()
                };
                let _ = on_event_tx.try_send(ShellEvent::Cz(CzEvent::ExtensionsLoaded(items)));
                let _ = on_event_tx.try_send(ShellEvent::Cz(CzEvent::StatusCounts(counts)));
            }
            request_status(on_event_tx.clone());
            std::thread::spawn(|| {
                if let Some(client) = client() {
                    // Встроенные + contributed: страницы Customize тоже
                    // теряли resolve, если активация обогнала первый.
                    // ТОЛЬКО ещё не резолвнутые: registry:update прилетает
                    // регулярно (MCP-статусы и т.п.), и безусловный resolve
                    // гонял 1.6-2МБ HTML КАЖДОГО вью по кругу — хост
                    // захлёбывался, а шелл перегружал iframe'ы бесконечно
                    // («Restoring your session…» навсегда).
                    let dynamic: Vec<String> =
                        dynamic_webviews().lock().unwrap().iter().cloned().collect();
                    let ids = KNOWN_WEBVIEWS
                        .iter()
                        .map(|s| (*s).to_string())
                        .chain(dynamic)
                        .filter(|id| !resolved_views().lock().unwrap().contains(id));
                    for id in ids {
                        let _ = client
                            .request("kamin:webviewView:resolve", vec![serde_json::json!(id)]);
                    }
                }
            });
        }
        // HTML любого известного вью → хосту на /__webview + навигация
        "kamin:webviewView:html" => {
            eprintln!(
                "[t+{:.1}s] webview html arrived",
                t0().elapsed().as_secs_f32()
            );
            let id = payload
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if webview_known(id) {
                resolved_views().lock().unwrap().insert(id.to_string());
                // В сторе — shim + html БЕЗ тема-блока: тему вставляет
                // обработчик схемы при КАЖДОЙ отдаче (`web/scheme.rs`), так
                // перезагрузка вью всегда получает актуальную палитру, а стор
                // не зависит от темы (дедуп/rev не дёргаются её сменой).
                let html = format!(
                    "{}{}",
                    crate::ui::chat_webview::vscode_api_shim(id),
                    payload
                        .get("html")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                );
                let _ = on_event_tx.try_send(ShellEvent::Term(TermEvent::WebviewHtml(
                    id.to_string(),
                    html,
                )));
            }
        }
        // Сообщения расширения к вебвью: батч {ids,msg} → доставка per-id.
        // Доставляем ПРЯМО ОТСЮДА (WS-поток) в CEF, минуя main loop: раньше
        // каждый пост шёл событием через UI-очередь, и лавина pty-output от
        // прогретого пула сессий (тысячи событий) задерживала tab:switched на
        // ДЕСЯТКИ СЕКУНД — «переключение не работает». RootView для доставки
        // не нужен (web::deliver потокобезопасен: outbox + post_task CEF).
        "kamin:webview:post" => {
            use std::collections::HashMap as Hm;
            let mut per_view: Hm<String, Vec<Value>> = Hm::new();
            if let Some(batch) = payload.get("batch").and_then(Value::as_array) {
                for group in batch {
                    let Some(msg) = group.get("msg") else {
                        continue;
                    };
                    if let Some(ids) = group.get("ids").and_then(Value::as_array) {
                        for id in ids.iter().filter_map(Value::as_str) {
                            if webview_known(id) {
                                per_view.entry(id.into()).or_default().push(msg.clone());
                            } else {
                                // Диагностика молчаливых потерь: id вне реестра =
                                // вью не получит НИЧЕГО (класс «панель агентов
                                // пустая»). Один warn на id за жизнь процесса.
                                warn_unknown_view_once(id);
                            }
                        }
                    }
                }
            }
            // Каждое вью — включая contributed Customize-страницы — живёт под
            // СВОИМ id (czShared-контейнер упразднён: без имени раздела в
            // pathname бандл не мог выбрать секцию).
            for (id, msgs) in per_view {
                if let Ok(json) = serde_json::to_string(&msgs) {
                    if json.contains("tab:switched") {
                        eprintln!(
                            "[wv:deliver] {id} tab:switched t+{:.2}s",
                            t0().elapsed().as_secs_f32()
                        );
                    }
                    if crate::web::enabled() {
                        crate::web::deliver(&id, json);
                    }
                }
            }
        }
        // Тост от хоста (showInformationMessage/…)
        _ => return false,
    }
    true
}

/// Один warn на неизвестный вебвью-id за жизнь процесса: сообщения таким вью
/// молча выбрасываются (`webview_known`), и класс багов «панель пустая»
/// иначе не оставляет никакого следа в логе.
fn warn_unknown_view_once(id: &str) {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if seen.lock().unwrap().insert(id.to_string()) {
        eprintln!("[wv:drop] сообщения вью {id} отброшены: id вне реестра webview_known");
    }
}
