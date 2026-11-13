//! Sidecar, WS и ре-резолв вебвью (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
pub use crate::host::events::ShellEvent;
use crate::host::paths::{data_dirs, dev_repo};
use crate::host::{rebind, status::request_status};
use crate::host_link::{EVENT_TX, client, client_slot, t0, view_resolution};
use kamin_sidecar::{HostConfig, HostEndpoint, HostMode, HostState};
use kamin_ws::WsClient;
use serde_json::Value;
use smol::channel::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Старт всей связки. Дёргается один раз из main.
pub fn start(tx: Sender<ShellEvent>) {
    let _ = EVENT_TX.set(tx.clone());
    #[cfg(debug_assertions)]
    if crate::native_acceptance::config::Config::from_env().no_host {
        return;
    }
    let (data_dir, cache_dir) = data_dirs();
    let _ = std::fs::create_dir_all(&data_dir);
    let _ = std::fs::create_dir_all(&cache_dir);

    let state = HostState::default();
    let state_for_reconnect = state.clone();
    let tx_endpoint = tx;

    // runtime/kamin-host.mjs рядом с exe выбирает packaged-режим, иначе dev-репо.
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

/// Вебвью-id синхронны с картами root.rs.
pub const KNOWN_WEBVIEWS: &[&str] = &[
    "claudeBridgeChat",
    "claudeBridgePlanView",
    "claudeBridgeConsoleView",
];

/// Contributed Customize webview IDs.
pub(crate) fn dynamic_webviews() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static S: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

pub fn register_dynamic_webview(id: &str) {
    dynamic_webviews().lock().unwrap().insert(id.to_string());
}

pub fn webview_known(id: &str) -> bool {
    KNOWN_WEBVIEWS.contains(&id) || dynamic_webviews().lock().unwrap().contains(id)
}

/// Ленивый resolve contributed-вью (по клику страницы Customize).
pub fn resolve_webview(id: String) {
    #[cfg(debug_assertions)]
    if id == crate::native_acceptance::VIEW {
        return;
    }
    std::thread::spawn(move || {
        if let Some(client) = client() {
            request_view(&client, &id);
        }
    });
}

pub(crate) fn request_view(client: &WsClient, id: &str) {
    crate::host::view_resolution::request_once(view_resolution(), id, || {
        client
            .request("kamin:webviewView:resolve", vec![serde_json::json!(id)])
            .map(|_| ())
    });
}

/// Retry через RETRY_DELAY_MS только к тому же хосту; новый ведёт on_endpoint.
pub(crate) fn connect_ws(endpoint: HostEndpoint, tx: Sender<ShellEvent>, state: HostState) {
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
                    let level: &'static str = match severity {
                        "error" => "error",
                        "warning" => "warning",
                        _ => "info",
                    };
                    let _ = req_tx.try_send(ShellEvent::Cz(CzEvent::SystemLog(
                        level,
                        "host".into(),
                        message,
                    )));
                    let (toast, has_items) = crate::host::show_message::toast_for(req_id, params);
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

        let (disc_tx, disc_state, own) = (tx.clone(), state.clone(), endpoint.clone());
        let on_disconnect = Arc::new(move || {
            let _ = disc_tx.try_send(ShellEvent::WsDisconnected);
            *client_slot().lock().unwrap() = None;
            std::thread::sleep(Duration::from_millis(kamin_ws::RETRY_DELAY_MS));
            if let Some(next) = rebind::retry_target(disc_state.snapshot(), &own) {
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
                rebind::adopt_endpoint(&endpoint, &tx);
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
                // зарегистрировать провайдер позже коннекта. Ошибка запроса
                // до регистрации оставляет вью доступным для повтора.
                eprintln!("[t+{:.1}s] ws connected", t0().elapsed().as_secs_f32());
                {
                    std::thread::spawn(move || {
                        for _ in 0..30 {
                            for id in KNOWN_WEBVIEWS {
                                request_view(&client, id);
                            }
                            if KNOWN_WEBVIEWS
                                .iter()
                                .all(|id| view_resolution().lock().unwrap().is_requested(id))
                            {
                                break;
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
                if let Some(next) = rebind::retry_target(state.snapshot(), &endpoint) {
                    connect_ws(next, tx, state);
                }
            }
        }
    });
}
