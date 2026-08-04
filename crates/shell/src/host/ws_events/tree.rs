//! Contributed-деревья: reveal, изменения, dnd, метаданные.
//!
//! Ветки перенесены дословно из `connect_ws` (`plan/100-refactor-250.md`).

use crate::host::events::TreeEvent;
use serde_json::Value;
use smol::channel::Sender;

use crate::host_link::ShellEvent;

/// Имя параметра `on_event_tx` сохранено из замыкания: тела веток
/// перенесены дословно. `true` — канал обработан здесь.
pub(crate) fn handle(on_event_tx: &Sender<ShellEvent>, channel: &str, payload: &Value) -> bool {
    match channel {
        "kamin:view:reveal" => {
            if let Some(container) = payload.get("container").and_then(Value::as_str) {
                let view = payload
                    .get("view")
                    .and_then(Value::as_str)
                    .map(String::from);
                let _ = on_event_tx.try_send(ShellEvent::Tree(TreeEvent::RevealView(
                    container.to_string(),
                    view,
                )));
            }
        }
        "kamin:tree:changed" => {
            if let Some(id) = payload.get("viewId").and_then(Value::as_str) {
                let _ =
                    on_event_tx.try_send(ShellEvent::Tree(TreeEvent::TreeChanged(id.to_string())));
            }
        }
        "kamin:tree:reveal" => {
            if let Some(id) = payload.get("viewId").and_then(Value::as_str) {
                let handle = payload
                    .get("handle")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let expand_path = payload
                    .get("expandPath")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                // Дефолты VS Code: select true, expand false
                let select = payload
                    .get("select")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                // `expand` бывает и числом (глубина) — любое ненулевое
                // раскрывает узел
                let expand = payload
                    .get("expand")
                    .is_some_and(|v| v.as_bool().unwrap_or(false) || v.as_i64().unwrap_or(0) > 0);
                let _ = on_event_tx.try_send(ShellEvent::Tree(TreeEvent::TreeReveal {
                    view: id.to_string(),
                    handle,
                    expand_path,
                    select,
                    expand,
                    tries: 0,
                }));
            }
        }
        "kamin:tree:dnd" => {
            if let Some(id) = payload.get("viewId").and_then(Value::as_str) {
                let _ = on_event_tx.try_send(ShellEvent::Tree(TreeEvent::TreeDnd {
                    view: id.to_string(),
                    enabled: payload
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                }));
            }
        }
        "kamin:tree:meta" => {
            if let Some(id) = payload.get("viewId").and_then(Value::as_str) {
                let meta = payload
                    .get("meta")
                    .map(crate::ui::contributed_tree::TreeMeta::from_value)
                    .unwrap_or_default();
                let _ = on_event_tx.try_send(ShellEvent::Tree(TreeEvent::TreeMetaSet {
                    view: id.to_string(),
                    meta,
                }));
            }
        }
        // Расширения (пере)активировались → перересолвить все вью
        // (первый resolve при коннекте гонится с активацией)
        _ => return false,
    }
    true
}
