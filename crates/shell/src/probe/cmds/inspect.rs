//! Осмотр: дерево регионов, метрики, кадры, дети окна, состояния.
//!
//! Ветки probe-обработчика вынесены как есть (`plan/100-refactor-250.md`).

use crate::probe::registry;

use serde_json::{Value, json};

/// `None` — команда не из этой группы.
pub(crate) fn handle_inspect(cmd: &str, req: &Value) -> Option<Value> {
    if !matches!(
        cmd,
        "children"
            | "emit"
            | "focus"
            | "hits"
            | "metric"
            | "overlay"
            | "ping"
            | "resize"
            | "rpc"
            | "screen"
            | "screenshot"
            | "shape"
            | "tools"
            | "tree"
            | "treesel"
    ) {
        return None;
    }
    // Тела веток перенесены дословно: ранние `return json!(…)`
    // внутри них должны выходить из ВЕТКИ, а не из этой функции.
    Some((|| -> Value {
        match cmd {
            "ping" => json!({
                "ok": true,
                "app": "kaminide-gpui",
                "version": env!("CARGO_PKG_VERSION"),
            }),
            "tree" => json!({"ok": true, "regions": registry::snapshot()}),
            "metric" => {
                let Some(id) = req.get("id").and_then(Value::as_str) else {
                    return json!({"ok": false, "err": "missing id"});
                };
                match registry::metric(id) {
                    Some(bounds) => json!({"ok": true, "id": id, "bounds": bounds}),
                    None => json!({"ok": false, "err": format!("unknown region: {id}")}),
                }
            }
            // {"cmd":"screen","x","y","w","h"} — зона РЕАЛЬНОГО экрана (BitBlt)
            "screen" => {
                #[cfg(windows)]
                {
                    let g = |k: &str| req.get(k).and_then(Value::as_i64).unwrap_or(0) as i32;
                    let path = std::env::temp_dir().join("kaminide-screen.png");
                    match crate::probe::shot::capture_screen_zone(
                        &path,
                        g("x"),
                        g("y"),
                        g("w"),
                        g("h"),
                    ) {
                        Ok((w, h)) => {
                            json!({"ok": true, "path": path.display().to_string(), "w": w, "h": h})
                        }
                        Err(e) => json!({"ok": false, "err": e}),
                    }
                }
                #[cfg(not(windows))]
                json!({"ok": false, "err": "windows only"})
            }
            "screenshot" => {
                #[cfg(windows)]
                {
                    let path = req
                        .get("path")
                        .and_then(Value::as_str)
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| std::env::temp_dir().join("kaminide-gpui-shot.png"));
                    let overlay = req.get("target").and_then(Value::as_str) == Some("overlay");
                    match crate::probe::shot::capture_to_png_ex(&path, overlay) {
                        Ok((w, h)) => {
                            json!({"ok": true, "path": path.display().to_string(), "w": w, "h": h})
                        }
                        Err(e) => json!({"ok": false, "err": e}),
                    }
                }
                #[cfg(not(windows))]
                {
                    json!({"ok": false, "err": "windows only"})
                }
            }
            // Диагностика wv2: child-окна main (класс/rect/visible)
            "children" => {
                #[cfg(windows)]
                {
                    json!({"ok": true, "children": crate::probe::shot::list_children()})
                }
                #[cfg(not(windows))]
                {
                    json!({"ok": false, "err": "windows only"})
                }
            }
            // Дев-мост к WS-методам хоста: {"cmd":"rpc","method":"kamin:...","params":[...]}
            "rpc" => {
                let Some(method) = req.get("method").and_then(Value::as_str) else {
                    return json!({"ok": false, "err": "missing method"});
                };
                let params = req
                    .get("params")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                match crate::host_link::client() {
                    None => json!({"ok": false, "err": "ws not connected"}),
                    Some(client) => match client.request(method, params) {
                        Ok(value) => json!({"ok": true, "value": value}),
                        Err(e) => json!({"ok": false, "err": e}),
                    },
                }
            }
            // Dev-инъекция оверлей-событий (клик-инъекции ещё нет): открыть
            // контекст-меню / модалку из живого состояния, чтобы снять скриншот.
            // {"cmd":"overlay"} — активные оверлей-стейты последнего кадра
            "overlay" => json!({"ok": true, "states": crate::probe::registry::overlay_states()}),
            // {"cmd":"flushLayout"} — донести отложенный layout-патч на диск.
            // Стенды ОБЯЗАНЫ звать перед рестартом процесса: kill между
            // дебаунсами терял хвост изменений юзера («лейаут не тот»).
            "flushLayout" => {
                crate::layout_store::flush_now();
                json!({"ok": true})
            }
            // {"cmd":"emit","kind":"sessionMenu"|"confirm"|"prompt"|"close", ...}
            // {"cmd":"tools"} — contributed-тулы реестра и их вью
            // (сверка модели «контейнер → N вью»)
            "tools" => json!({"ok": true, "tools": crate::activity::dyn_tools_list()
                    .into_iter()
                    .map(|t| json!({
                        "id": t.id,
                        "label": t.label,
                        "views": t.views.iter()
                            .map(|v| json!({"id": v.id, "name": v.name, "webview": v.webview}))
                            .collect::<Vec<_>>(),
                    }))
                    .collect::<Vec<_>>()}),
            // {"cmd":"focus"} — состояние `:focus-visible`
            "focus" => {
                let (visible, id, ids) = crate::ui::focus_ring::debug_state();
                json!({"ok": true, "keyboard": visible, "focused": id, "stops": ids})
            }
            // {"cmd":"treesel"} — выделение файлового дерева и якорь Shift
            "treesel" => {
                let (sel, anchor) = crate::probe::registry::tree_selection();
                json!({"ok": true, "selected": sel, "anchor": anchor})
            }
            // {"cmd":"shape","text":"…","size":11,"weight":600,"spacing":0.66}
            // → кладёт запрос; следующий вызов без "text" вернёт ширину.
            // Меряет ТЕМ ЖЕ шейпером, что и рендер (ц.32)
            "shape" => {
                if let Some(text) = req.get("text").and_then(Value::as_str) {
                    crate::probe::registry::request_shape(crate::probe::registry::ShapeReq {
                        text: text.to_string(),
                        size: req.get("size").and_then(Value::as_f64).unwrap_or(11.0) as f32,
                        weight: req.get("weight").and_then(Value::as_f64).unwrap_or(400.0) as f32,
                        spacing: req.get("spacing").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                        mono: req.get("mono").and_then(Value::as_bool).unwrap_or(false),
                    });
                    json!({"ok": true, "queued": true})
                } else {
                    match crate::probe::registry::shape_result() {
                        Some((text, width)) => json!({"ok": true, "text": text, "width": width}),
                        None => json!({"ok": false, "err": "no result yet"}),
                    }
                }
            }
            // {"cmd":"resize","w":1200,"h":800} — размер ГЛАВНОГО окна в
            // логических px: без него адаптер вьюпорта нечем проверить
            "resize" => {
                #[cfg(windows)]
                {
                    let g = |k: &str| req.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    match crate::probe::input::resize_main(g("w"), g("h")) {
                        Ok(()) => json!({"ok": true}),
                        Err(e) => json!({"ok": false, "err": e}),
                    }
                }
                #[cfg(not(windows))]
                {
                    json!({"ok": false, "err": "windows only"})
                }
            }
            // {"cmd":"hits"} — hit-регионы overlay (физ. px клиента)
            "hits" => json!({"ok": true, "rects": crate::overlay::hit_rects_snapshot()}),
            "emit" => crate::probe::host::emit(req),
            _ => unreachable!("список команд выше"),
        }
    })())
}
