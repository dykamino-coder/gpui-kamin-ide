//! Команды runtime-приёмки: чтение страниц вью, Bridge-команды чата,
//! панели и снимки областей.
//!
//! Сценарии инцидентов Bridge (переключение сессий, Back в читалке агента,
//! Compact, отправка промпта, закрытие вкладки) живут внутри CEF-страниц.
//! Клики по координатам в них ломаются от любой перестановки раскладки, а
//! клавиши в чужое окно слать нельзя. Поэтому стенд ходит теми же путями,
//! что кнопки: `window.bridgeCmd` чата (`webview/src/lib/command-api.ts`),
//! события `PinTool`/`UnpinTool`/`RevealView` раскладки и PrintWindow-кадр.
//! Всё это существует только в сборке с feature `probe` (loopback-порт
//! `KAMIN_PROBE_PORT`), как и остальные команды канала.

use std::time::Duration;

use crate::activity::PanelSlot;
use crate::host::events::{TermEvent, TreeEvent};
use crate::host_link::ShellEvent;
use crate::probe::wv_eval;
use serde_json::{Value, json};

const CHAT_VIEW: &str = "claudeBridgeChat";

/// `None` — команда не из этой группы.
pub(crate) fn handle_accept(cmd: &str, req: &Value) -> Option<Value> {
    Some(match cmd {
        // {"cmd":"wveval","id":"claudeBridgeAgentsView","expr":"document.title"}
        // {"cmd":"wveval","id":"…","body":"const n=…; return n;","timeoutMs":5000}
        "wveval" => {
            let id = req.get("id").and_then(Value::as_str).unwrap_or(CHAT_VIEW);
            let expr = req.get("expr").and_then(Value::as_str);
            let body = req.get("body").and_then(Value::as_str);
            eval(id, expr, body, timeout(req))
        }
        // {"cmd":"bridge","call":"sendAndWait","args":["/compact"]} —
        // `window.bridgeCmd.<call>(...args)` в чате; `help` перечисляет вызовы
        "bridge" => {
            let Some(call) = req.get("call").and_then(Value::as_str) else {
                return Some(json!({"ok": false, "err": "call required"}));
            };
            if !call.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Some(json!({"ok": false, "err": "bad call name"}));
            }
            let args = req.get("args").cloned().unwrap_or_else(|| json!([]));
            let body = format!(
                "if(!window.bridgeCmd)throw new Error('bridgeCmd not installed');\
return await window.bridgeCmd.{call}(...{args});"
            );
            eval(CHAT_VIEW, None, Some(&body), timeout(req))
        }
        // {"cmd":"webviews"} — поднятые CEF-вью (id)
        "webviews" => json!({"ok": true, "views": crate::web::all_view_ids()}),
        // {"cmd":"showView","id":"claudeBridgeAgents"} — как reveal расширения:
        // пин контейнера в его слот; {"slot":"rightBottom"} — в заданный слот
        "showView" => {
            let Some(id) = req.get("id").and_then(Value::as_str) else {
                return Some(json!({"ok": false, "err": "id required"}));
            };
            let ev = match slot(req) {
                Some(s) => ShellEvent::PinTool(s, id.to_string()),
                None => ShellEvent::Tree(TreeEvent::RevealView(id.to_string(), None)),
            };
            send(ev)
        }
        // {"cmd":"hideView","id":"claudeBridgeAgents","slot":"rightTop"}
        "hideView" => {
            let (Some(id), Some(s)) = (req.get("id").and_then(Value::as_str), slot(req)) else {
                return Some(json!({"ok": false, "err": "id and slot required"}));
            };
            send(ShellEvent::UnpinTool(s, id.to_string()))
        }
        // {"cmd":"shotRegion","id":"right-top","path":"C:/…/agents.png"}
        "shotRegion" => shot_region(req),
        _ => return None,
    })
}

fn timeout(req: &Value) -> Duration {
    let ms = req
        .get("timeoutMs")
        .and_then(Value::as_u64)
        .unwrap_or(10_000);
    Duration::from_millis(ms.clamp(100, 600_000))
}

fn slot(req: &Value) -> Option<PanelSlot> {
    let name = req.get("slot").and_then(Value::as_str)?;
    PanelSlot::ALL.into_iter().find(|s| s.as_str() == name)
}

fn send(ev: ShellEvent) -> Value {
    match crate::host_link::event_tx() {
        Some(tx) => match tx.try_send(ev) {
            Ok(()) => json!({"ok": true}),
            Err(e) => json!({"ok": false, "err": e.to_string()}),
        },
        None => json!({"ok": false, "err": "event channel not ready"}),
    }
}

/// Скрипт исполняется на UI-потоке (как `wvjs`), ответ ждём здесь.
fn eval(id: &str, expr: Option<&str>, body: Option<&str>, timeout: Duration) -> Value {
    if !crate::web::all_view_ids().iter().any(|v| v == id) {
        return json!({"ok": false, "view": id, "err": "view is not live"});
    }
    let nonce = wv_eval::reserve();
    let js = wv_eval::wrap(nonce, expr, body);
    let sent = send(ShellEvent::Term(TermEvent::WvJs(id.to_string(), js)));
    if sent["ok"] != true {
        let _ = wv_eval::wait(nonce, Duration::ZERO);
        return sent;
    }
    wv_eval::reply(id, wv_eval::wait(nonce, timeout), timeout)
}

fn shot_region(req: &Value) -> Value {
    #[cfg(windows)]
    {
        let Some(id) = req.get("id").and_then(Value::as_str) else {
            return json!({"ok": false, "err": "id required"});
        };
        let path = req
            .get("path")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("kaminide-region.png"));
        match crate::probe::shot_region::capture_region(id, &path) {
            Ok(rect) => json!({"ok": true, "path": path.display().to_string(), "rect": rect}),
            Err(e) => json!({"ok": false, "err": e}),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = req;
        json!({"ok": false, "err": "windows only"})
    }
}
