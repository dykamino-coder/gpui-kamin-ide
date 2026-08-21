//! Старт связки: sidecar, WS-подключение, ре-резолв вебвью.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
pub use crate::host::events::ShellEvent;
use crate::host::paths::{data_dirs, dev_repo};
use crate::host::status::request_status;
use crate::host_link::{EVENT_TX, client, client_slot, endpoint_slot, resolved_views, t0};
use kamin_sidecar::{HostConfig, HostEndpoint, HostMode, HostState};
use kamin_ws::WsClient;
use serde_json::Value;
use smol::channel::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Старт всей связки. Дёргается один раз из main.
pub fn start(tx: Sender<ShellEvent>) {
    let _ = EVENT_TX.set(tx.clone());
    let (data_dir, cache_dir) = data_dirs();
    let _ = std::fs::create_dir_all(&data_dir);
    let _ = std::fs::create_dir_all(&cache_dir);

    let state = HostState::default();
    let state_for_reconnect = state.clone();
    let tx_endpoint = tx.clone();

    // Packaged-раскладка: каталог `runtime/` рядом с exe (node + kamin-host.mjs
    // + builtin-extensions из payload-скрипта kamin-ide) включает Runtime-режим;
    // без него — dev-репо как раньше. Единственная развилка dev/prod.
    let runtime_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("runtime")))
        .filter(|d| d.join("kamin-host.mjs").is_file());
    let mode = match runtime_dir {
        Some(runtime_dir) => HostMode::Runtime { runtime_dir },
        None => HostMode::DevRepo {
            repo_root: dev_repo(),
        },
    };
    kamin_sidecar::start(
        HostConfig {
            mode,
            app_version: env!("CARGO_PKG_VERSION"),
            data_dir,
            cache_dir,
            // «Open with KaminIDE» на холодном старте: host сам создаст/
            // активирует сессию в папке (host-main.ts openFolderPath).
            open_folder: crate::win_integration::stored_launch_folder(),
        },
        state,
        Arc::new(move |endpoint| {
            let _ = tx_endpoint.try_send(ShellEvent::HostReady(endpoint.clone()));
            connect_ws(endpoint, tx_endpoint.clone(), state_for_reconnect.clone());
        }),
    );
}

/// Contributed-вью, которые шелл монтирует как вебвью-оверлеи (id из
/// registry). Держим синхронно с картами в root.rs.
pub const KNOWN_WEBVIEWS: &[&str] = &[
    "claudeBridgeChat",
    "claudeBridgePlanView",
    "claudeBridgeConsoleView",
];

/// Динамические вебвью-id (contributed Customize-страницы из registry).
pub(crate) fn dynamic_webviews() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static S: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

pub fn register_dynamic_webview(id: &str) {
    dynamic_webviews().lock().unwrap().insert(id.to_string());
}

/// Известен ли вебвью-id (builtin ИЛИ contributed).
pub fn webview_known(id: &str) -> bool {
    KNOWN_WEBVIEWS.contains(&id) || dynamic_webviews().lock().unwrap().contains(id)
}

/// Ленивый resolve contributed-вью (по клику страницы Customize).
pub fn resolve_webview(id: String) {
    std::thread::spawn(move || {
        if let Some(client) = client() {
            let _ = client.request("kamin:webviewView:resolve", vec![serde_json::json!(id)]);
        }
    });
}

/// (Пере)подключение WS-клиента к endpoint. При разрыве — retry через
/// RETRY_DELAY_MS к последнему известному endpoint (host мог перезапуститься —
/// свежий endpoint придёт через on_endpoint сайдкара и победит).
fn connect_ws(endpoint: HostEndpoint, tx: Sender<ShellEvent>, state: HostState) {
    std::thread::spawn(move || {
        let on_event_tx = tx.clone();
        let on_event: kamin_ws::EventListener = Arc::new(move |channel: &str, payload: &Value| {
            crate::host::ws_events::dispatch(&on_event_tx, channel, payload);
        });
        // host→клиент запросы (shell.*): showMessage → тост (кнопки-actions —
        // с интерактивными тостами); остальные диалоги — следующая фаза.
        let req_tx = tx.clone();
        let on_host_request: kamin_ws::HostRequestHandler = Arc::new(
            move |req_id: u64, method: &str, params: &[Value]| match method {
                // QuickPick: (items, options) — ответ после выбора юзера
                // (respond(req_id) из root по QuickPickResolve)
                "shell.showQuickPick" => {
                    let items = params.first().cloned().unwrap_or(Value::Array(vec![]));
                    let options = params.get(1).cloned().unwrap_or(Value::Null);
                    let _ = req_tx.try_send(ShellEvent::QuickPickShow(req_id, items, options));
                    kamin_ws::HostReply::Later
                }
                "shell.showMessage" => {
                    // (severity, message, items?) → тост + system-лог.
                    // С items — sticky-тост с кнопками, ответ хосту отложен
                    // (клик по кнопке = выбранный item; dismiss = null).
                    let severity = params.first().and_then(Value::as_str).unwrap_or("info");
                    let message = params
                        .get(1)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let items: Vec<String> = params
                        .get(2)
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default();
                    let level: &'static str = match severity {
                        "error" => "error",
                        "warning" => "warning",
                        _ => "info",
                    };
                    let _ = req_tx.try_send(ShellEvent::Cz(CzEvent::SystemLog(
                        level,
                        "host".into(),
                        message.clone(),
                    )));
                    let has_items = !items.is_empty();
                    let toast = crate::ui::toasts::Toast {
                        id: if has_items {
                            format!("shellreq-{req_id}")
                        } else {
                            format!("shellmsg-{severity}-{message:.32}")
                        },
                        severity: severity.to_string(),
                        title: None,
                        message,
                        actions: items,
                        sticky: has_items, // ждём выбора — не авто-скрывать
                    };
                    let _ = req_tx.try_send(ShellEvent::Toast(toast));
                    if has_items {
                        kamin_ws::HostReply::Later
                    } else {
                        kamin_ws::HostReply::Now(Ok(Value::Null))
                    }
                }
                // Скрепка чата и загрузки страниц: нативные диалоги.
                // Диалог блокирует — уводим в поток, ответ отдаём позже.
                "shell.showOpenDialog" => {
                    let options = params.first().cloned().unwrap_or(Value::Null);
                    std::thread::spawn(move || {
                        let reply = crate::host::dialogs::show_open_dialog(&options);
                        if let Some(c) = crate::host_link::client() {
                            c.respond(req_id, Ok(reply));
                        }
                    });
                    kamin_ws::HostReply::Later
                }
                "shell.showSaveDialog" => {
                    let options = params.first().cloned().unwrap_or(Value::Null);
                    std::thread::spawn(move || {
                        let reply = crate::host::dialogs::show_save_dialog(&options);
                        if let Some(c) = crate::host_link::client() {
                            c.respond(req_id, Ok(reply));
                        }
                    });
                    kamin_ws::HostReply::Later
                }
                // env.clipboard.readText: DOM-пути у exthost нет, читаем мы.
                // arboard не требует фокуса — как в kamin-ide.
                "shell.readClipboard" => {
                    let text = arboard::Clipboard::new()
                        .and_then(|mut c| c.get_text())
                        .unwrap_or_default();
                    kamin_ws::HostReply::Now(Ok(Value::String(text)))
                }
                // env.openExternal: ссылка/путь — системному обработчику.
                "shell.openExternal" => {
                    let target = params
                        .first()
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let ok = !target.is_empty() && open::that_detached(&target).is_ok();
                    kamin_ws::HostReply::Now(Ok(Value::Bool(ok)))
                }
                other => kamin_ws::HostReply::Now(Err(format!("not implemented: {other}"))),
            },
        );

        let disc_tx = tx.clone();
        let disc_state = state.clone();
        let on_disconnect = Arc::new(move || {
            let _ = disc_tx.try_send(ShellEvent::WsDisconnected);
            *client_slot().lock().unwrap() = None;
            std::thread::sleep(Duration::from_millis(kamin_ws::RETRY_DELAY_MS));
            if let Some(next) = disc_state.snapshot() {
                connect_ws(next, disc_tx.clone(), disc_state.clone());
            }
        });

        match WsClient::connect(
            endpoint.port,
            &endpoint.token,
            on_event,
            on_host_request,
            on_disconnect,
        ) {
            Ok(client) => {
                let client = Arc::new(client);
                *client_slot().lock().unwrap() = Some(client.clone());
                *endpoint_slot().lock().unwrap() = Some(endpoint.clone());
                let _ = tx.try_send(ShellEvent::WsConnected);
                // Гидрация: сессии + воркспейс (re-seed на реконнект — тот же путь)
                if let Ok(snap) = client.request("kamin:sessions:list", vec![]) {
                    let _ = tx.try_send(ShellEvent::Sessions(snap));
                }
                if let Ok(ws_folder) = client.request("kamin:workspace:get", vec![]) {
                    // ответ: {path|null} ИЛИ строка — берём обе формы
                    let path = ws_folder
                        .get("path")
                        .and_then(Value::as_str)
                        .or_else(|| ws_folder.as_str())
                        .map(String::from);
                    let _ = tx.try_send(ShellEvent::Workspace(path));
                }
                // Contributed-вью: резолвить с РЕТРАЕМ — Bridge-VSIX может
                // зарегистрировать провайдер позже коннекта (resolveView до
                // регистрации молча no-op, повторов host не делает).
                eprintln!("[t+{:.1}s] ws connected", t0().elapsed().as_secs_f32());
                {
                    let client = client.clone();
                    std::thread::spawn(move || {
                        for _ in 0..30 {
                            let pending: Vec<&str> = KNOWN_WEBVIEWS
                                .iter()
                                .copied()
                                .filter(|id| !resolved_views().lock().unwrap().contains(*id))
                                .collect();
                            if pending.is_empty() {
                                break;
                            }
                            for id in pending {
                                let _ = client.request(
                                    "kamin:webviewView:resolve",
                                    vec![serde_json::json!(id)],
                                );
                            }
                            std::thread::sleep(Duration::from_secs(2));
                        }
                    });
                }
                request_status(tx.clone());
            }
            Err(e) => {
                eprintln!("ws connect failed: {e}");
                std::thread::sleep(Duration::from_millis(kamin_ws::RETRY_DELAY_MS));
                if let Some(next) = state.snapshot() {
                    connect_ws(next, tx, state);
                }
            }
        }
    });
}
