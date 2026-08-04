//! Ввод: клик, драг, ховер, скролл, набор текста, клавиши.
//!
//! Ветки probe-обработчика вынесены как есть (`plan/100-refactor-250.md`).

use crate::host::events::TermEvent;
use serde_json::{Value, json};

/// `None` — команда не из этой группы.
pub(crate) fn handle_input(cmd: &str, req: &Value) -> Option<Value> {
    if !matches!(
        cmd,
        "click"
            | "drag"
            | "draghold"
            | "dragrelease"
            | "hover"
            | "key"
            | "scroll"
            | "type"
            | "wvjs"
            | "weburl"
            | "webkey"
            | "opendialog"
            | "webfocus"
            | "maximize"
    ) {
        return None;
    }
    // Тела веток перенесены дословно: ранние `return json!(…)`
    // внутри них должны выходить из ВЕТКИ, а не из этой функции.
    Some(match cmd {
        // {"cmd":"click","x":..,"y":..} — лог. px client-области;
        // "target":"overlay" — клик в overlay-окно (меню/модалки)
        "click" => {
            #[cfg(windows)]
            {
                let (x, y) = (
                    req.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    req.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                );
                let overlay = req.get("target").and_then(Value::as_str) == Some("overlay");
                let right = req.get("button").and_then(Value::as_str) == Some("right");
                let res = if overlay {
                    crate::probe::input::click_overlay(x, y)
                } else if right {
                    crate::probe::input::right_click(x, y)
                } else {
                    crate::probe::input::click(x, y)
                };
                match res {
                    Ok(()) => json!({"ok": true}),
                    Err(e) => json!({"ok": false, "err": e}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        // {"cmd":"drag","from":[x,y],"to":[x,y]}
        "drag" => {
            #[cfg(windows)]
            {
                let pt = |key: &str| -> Option<(f32, f32)> {
                    let a = req.get(key)?.as_array()?;
                    Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32))
                };
                match (pt("from"), pt("to")) {
                    (Some((x1, y1)), Some((x2, y2))) => {
                        // "target":"overlay" — драг в overlay-окно
                        let overlay = req.get("target").and_then(Value::as_str) == Some("overlay");
                        match crate::probe::input::drag_to(overlay, x1, y1, x2, y2) {
                            Ok(()) => json!({"ok": true}),
                            Err(e) => json!({"ok": false, "err": e}),
                        }
                    }
                    _ => json!({"ok": false, "err": "need from:[x,y] to:[x,y]"}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        // {"cmd":"hover","x":..,"y":..} — WM_MOUSEMOVE без клика
        "hover" => {
            #[cfg(windows)]
            {
                let (x, y) = (
                    req.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    req.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                );
                match crate::probe::input::hover(x, y) {
                    Ok(()) => json!({"ok": true}),
                    Err(e) => json!({"ok": false, "err": e}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        // {"cmd":"scroll","x":..,"y":..,"lines":-5} — колесо (минус = вниз)
        "scroll" => {
            #[cfg(windows)]
            {
                let (x, y) = (
                    req.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    req.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                );
                let lines = req.get("lines").and_then(Value::as_i64).unwrap_or(-3) as i32;
                match crate::probe::input::scroll(x, y, lines) {
                    Ok(()) => json!({"ok": true}),
                    Err(e) => json!({"ok": false, "err": e}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        // {"cmd":"type","text":"..."} — WM_CHAR в сфокусированный инпут
        "type" => {
            #[cfg(windows)]
            {
                let text = req.get("text").and_then(Value::as_str).unwrap_or("");
                // "target":"overlay" — ввод в инпут оверлея
                let overlay = req.get("target").and_then(Value::as_str) == Some("overlay");
                match crate::probe::input::type_text_to(overlay, text) {
                    Ok(()) => json!({"ok": true}),
                    Err(e) => json!({"ok": false, "err": e}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        // {"cmd":"maximize"} — развернуть/восстановить главное окно (стенд
        // подстройки контента под большой прыжок размера).
        "maximize" => {
            #[cfg(windows)]
            {
                crate::overlay::toggle_main_maximize();
                json!({"ok": true})
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        // {"cmd":"draghold","from":[x,y],"to":[x,y]} — вести и НЕ отпускать;
        // {"cmd":"dragrelease","at":[x,y]} — отпустить. Нужны стендам, чтобы
        // снимать кадр в процессе перетаскивания.
        "draghold" | "dragrelease" => {
            #[cfg(windows)]
            {
                let pair = |key: &str| -> (f32, f32) {
                    req.get(key)
                        .and_then(Value::as_array)
                        .map(|a| {
                            (
                                a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32,
                                a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32,
                            )
                        })
                        .unwrap_or((0.0, 0.0))
                };
                let overlay = req.get("target").and_then(Value::as_str) == Some("overlay");
                let res = if cmd == "draghold" {
                    let (x1, y1) = pair("from");
                    let (x2, y2) = pair("to");
                    crate::probe::input::drag_hold(overlay, x1, y1, x2, y2)
                } else {
                    let (x, y) = pair("at");
                    crate::probe::input::drag_release(overlay, x, y)
                };
                match res {
                    Ok(()) => json!({"ok": true}),
                    Err(e) => json!({"ok": false, "err": e}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        // {"cmd":"weburl","id":"browser","url":"..."} — открыть адрес в вью.
        // Нужна стендам: без неё страницу для проверки не подсунуть.
        "weburl" => {
            let id = req.get("id").and_then(Value::as_str).unwrap_or("browser");
            let url = req.get("url").and_then(Value::as_str).unwrap_or("");
            if url.is_empty() {
                json!({"ok": false, "err": "нет адреса"})
            } else {
                crate::web::navigate(id, url);
                json!({"ok": true})
            }
        }
        // {"cmd":"webkey","id":"browser","key":"a","char":"a","ctrl":false}
        // — клавиша прямо в CEF-вью (нажатие+отпускание). Нужна стендам:
        // probe не умеет доставлять клавиши через gpui (см. память).
        "webkey" => {
            let id = req.get("id").and_then(Value::as_str).unwrap_or("browser");
            let key = req.get("key").and_then(Value::as_str).unwrap_or("");
            let ch = req.get("char").and_then(Value::as_str);
            let flag = |name: &str| req.get(name).and_then(Value::as_bool).unwrap_or(false);
            if key.is_empty() {
                json!({"ok": false, "err": "нет клавиши"})
            } else {
                crate::web::probe_key(id, key, ch, flag("ctrl"), flag("shift"), flag("alt"));
                json!({"ok": true})
            }
        }
        // {"cmd":"opendialog"} — нативный open-диалог (тест rfd-пути
        // shell.showOpenDialog): результат в stdout, окно закрывает стенд.
        "opendialog" => {
            std::thread::spawn(|| {
                let opts = serde_json::json!({"title": "probe open dialog"});
                let reply = crate::host::dialogs::show_open_dialog(&opts);
                println!("[probe] opendialog ответ: {reply}");
            });
            json!({"ok": true})
        }
        // {"cmd":"webfocus","id":"browser"} — фокус клавиатуры в вью.
        "webfocus" => {
            let id = req.get("id").and_then(Value::as_str).unwrap_or("browser");
            crate::web::probe_focus(id);
            json!({"ok": true})
        }
        // {"cmd":"wvjs","id":"claudeBridgeChat","js":"..."} — выполнить
        // JS в composition-вью (диагностика страниц)
        "wvjs" => {
            #[cfg(windows)]
            {
                // COM STA: исполнять на UI-потоке (напрямую из TCP-потока
                // ExecuteScript молча теряется)
                let id = req.get("id").and_then(Value::as_str).unwrap_or("browser");
                let js = req.get("js").and_then(Value::as_str).unwrap_or("");
                match crate::host_link::event_tx() {
                    Some(tx) => {
                        let _ = tx.try_send(crate::host_link::ShellEvent::Term(TermEvent::WvJs(
                            id.to_string(),
                            js.to_string(),
                        )));
                        json!({"ok": true})
                    }
                    None => json!({"ok": false, "err": "no event channel"}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        "key" => {
            #[cfg(windows)]
            {
                let name = req.get("name").and_then(Value::as_str).unwrap_or("");
                let repeat = req.get("repeat").and_then(Value::as_u64).unwrap_or(1) as u32;
                match crate::probe::input::press_key_to(
                    req.get("target").and_then(Value::as_str) == Some("overlay"),
                    name,
                    repeat,
                ) {
                    Ok(()) => json!({"ok": true}),
                    Err(e) => json!({"ok": false, "err": e}),
                }
            }
            #[cfg(not(windows))]
            {
                json!({"ok": false, "err": "windows only"})
            }
        }
        _ => unreachable!("список команд выше"),
    })
}
