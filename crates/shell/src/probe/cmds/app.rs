//! Приложенческие команды probe: single-instance форвард из второго запуска
//! («Open with KaminIDE» по уже работающему приложению) — открыть папку,
//! поднять окно.

use serde_json::{Value, json};

/// `None` — команда не из этой группы.
pub(crate) fn handle_app(cmd: &str, req: &Value) -> Option<Value> {
    match cmd {
        "openFolder" => {
            let Some(path) = req.get("path").and_then(|v| v.as_str()) else {
                return Some(json!({"ok": false, "err": "path required"}));
            };
            let path = path.to_string();
            // Тот же путь, что «New session» в папке из UI: host создаёт/
            // активирует сессию, workspace:changed прилетит штатно.
            std::thread::spawn(move || {
                if let Some(c) = crate::host_link::client() {
                    let _ = c.request("kamin:sessions:newSessionInFolder", vec![json!(path)]);
                }
            });
            focus_main_window();
            Some(json!({"ok": true}))
        }
        "focusWindow" => {
            focus_main_window();
            Some(json!({"ok": true}))
        }
        // {"cmd":"hostReq","method":"kamin:diag:memory","params":[...]} —
        // прокси произвольного host-запроса через аутентифицированный WS
        // шелла (стендам недоступен wsToken хоста). Loopback-only dev-канал.
        "hostReq" => {
            let Some(method) = req.get("method").and_then(|v| v.as_str()) else {
                return Some(json!({"ok": false, "err": "method required"}));
            };
            let params = req
                .get("params")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            Some(match crate::host_link::client() {
                Some(c) => match c.request(method, params) {
                    Ok(v) => json!({"ok": true, "result": v}),
                    Err(e) => json!({"ok": false, "err": e}),
                },
                None => json!({"ok": false, "err": "host not connected"}),
            })
        }
        _ => None,
    }
}

/// Поднять и сфокусировать главное окно (второй запуск без папки = юзер
/// просто ткнул ярлык — покажем существующее окно).
#[cfg(windows)]
fn focus_main_window() {
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    if let Some(hwnd) = crate::probe::shot::find_window(false) {
        // Safety: hwnd только что найден enum'ом наших окон; вызовы —
        // стандартный restore+foreground, без разыменований.
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
fn focus_main_window() {}
