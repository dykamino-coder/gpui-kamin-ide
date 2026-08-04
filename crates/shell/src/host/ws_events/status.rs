//! Сессии, статус-бар, декорации файлов, индекс и диагностика.
//!
//! Ветки перенесены дословно из `connect_ws` (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use crate::host::events::TreeEvent;
use serde_json::Value;
use smol::channel::Sender;

use crate::host_link::ShellEvent;

/// Имя параметра `on_event_tx` сохранено из замыкания: тела веток
/// перенесены дословно. `true` — канал обработан здесь.
pub(crate) fn handle(on_event_tx: &Sender<ShellEvent>, channel: &str, payload: &Value) -> bool {
    match channel {
        "kamin:sessions:changed" => {
            let _ = on_event_tx.try_send(ShellEvent::Sessions(payload.clone()));
        }
        // Contributed statusbar: пуш-апдейты от exthost
        "kamin:statusBar:update" => {
            let _ = on_event_tx.try_send(ShellEvent::Cz(CzEvent::StatusBarUpsert(payload.clone())));
        }
        "kamin:statusBar:remove" => {
            if let Some(id) = payload.get("id").and_then(Value::as_str) {
                let _ =
                    on_event_tx.try_send(ShellEvent::Cz(CzEvent::StatusBarRemove(id.to_string())));
            }
        }
        // Problems: дельта диагностик одного (owner, uri)
        "kamin:fileDecoration:changed" => {
            // uris: null|[] = refresh all; список = точечная инвалидация
            let uris = payload.get("uris").and_then(|u| u.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            });
            let uris = match uris {
                Some(v) if !v.is_empty() => Some(v),
                _ => None,
            };
            let _ = on_event_tx.try_send(ShellEvent::Tree(TreeEvent::DecoInvalidate(uris)));
        }
        "kamin:index:status" => {
            let ind = payload
                .get("indexing")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let _ = on_event_tx.try_send(ShellEvent::Cz(CzEvent::IndexStatus(ind)));
        }
        "kamin:diag:set" => {
            let _ = on_event_tx.try_send(ShellEvent::Cz(CzEvent::DiagSet(payload.clone())));
        }
        // Provider дёрнул onDidChangeTreeData → перечитать уровни
        // `<viewId>.focus` / `workbench.view.extension.<id>` хоста
        // (`exthost/view-reveal.ts`) — «раскрыть вью»: у customize-
        // контейнера это открытие Customize на нужном разделе, у
        // остальных — пин+активация слота (`ipc.ts:290-317`).
        // Без обработчика клик по MCP-счётчикам статус-бара молчал
        _ => return false,
    }
    true
}
