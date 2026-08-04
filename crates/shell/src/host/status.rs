//! Фоновый сбор счётчиков статус-бара.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::connect::register_dynamic_webview;
use crate::host::customize_manifests::customize_pages_from_manifests;
use crate::host::customize_snapshot::customize_pages_from;
use crate::host::events::CzEvent;
pub use crate::host::events::ShellEvent;
use crate::host_link::client;
use serde_json::Value;
use smol::channel::Sender;

/// Фоновый сбор счётчиков статус-бара (extensions:list + registry:snapshot).
pub fn request_status(tx: Sender<ShellEvent>) {
    // Навигация Customize — СРАЗУ из манифестов на диске, не дожидаясь ни
    // снапшота реестра, ни поднятия MCP: это статические данные расширения.
    // Ответ хоста ниже её просто уточнит (динамические/sideloaded вью).
    let manifest_pages = customize_pages_from_manifests();
    if !manifest_pages.is_empty() {
        let _ = tx.try_send(ShellEvent::Cz(CzEvent::CustomizePages(manifest_pages)));
    }
    std::thread::spawn(move || {
        // Небольшая фора интерфейсу: сначала уходят запросы, которые видит
        // пользователь (resolve страниц), потом счётчики. Раньше здесь стояло
        // 4000 мс — они прямо прибавлялись к «4 active через 35 с», когда
        // подъём хоста ускорился с 12 с до 325 мс.
        std::thread::sleep(std::time::Duration::from_millis(300));
        let Some(client) = client() else { return };
        let mut counts = crate::ui::status_bar::StatusCounts::default();
        if let Ok(exts) = client.request("kamin:extensions:list", vec![])
            && let Some(arr) = exts.as_array()
        {
            counts.ext_active = arr
                .iter()
                .filter(|e| e.get("active").and_then(Value::as_bool).unwrap_or(false))
                .count();
            counts.ext_failed = arr
                .iter()
                .filter(|e| e.get("activationError").is_some_and(|v| !v.is_null()))
                .count();
            counts.ext_disabled = arr
                .iter()
                .filter(|e| !e.get("enabled").and_then(Value::as_bool).unwrap_or(true))
                .count();
        }
        if let Ok(snap) = client.request("kamin:registry:snapshot", vec![]) {
            // Contributed Customize-страницы уже отданы ИЗ МАНИФЕСТОВ до этого
            // потока (см. начало `request_status`). Снапшот их лишь уточняет —
            // и БЕЗ цикла перезапросов: `kamin:registry:snapshot` на холодном
            // старте не отвечает вовсе, а сорок висящих запросов занимали
            // очередь хоста, из-за чего resolve страницы ждал 10.8 с (замер).
            {
                let pages = customize_pages_from(&snap);
                if !pages.is_empty() {
                    let _ = tx.try_send(ShellEvent::Cz(CzEvent::CustomizePages(pages)));
                }
            }
            counts.cmd_count = snap
                .get("commands")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            // Contributed explorer/context: (command, label, group, when)
            let titles: std::collections::HashMap<String, String> = snap
                .get("commands")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| {
                            Some((
                                c.get("id").and_then(Value::as_str)?.to_string(),
                                c.get("title").and_then(Value::as_str)?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let items: Vec<crate::ui::file_menu::ContribMenuItem> = snap
                .get("menus")
                .and_then(|m| m.get("explorer/context"))
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            let command = m.get("command").and_then(Value::as_str)?.to_string();
                            Some(crate::ui::file_menu::ContribMenuItem {
                                label: titles
                                    .get(&command)
                                    .cloned()
                                    .unwrap_or_else(|| command.clone()),
                                command,
                                group: m
                                    .get("group")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                                when: m
                                    .get("when")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::ExplorerMenuItems(items)));
            let themes: Vec<(String, String, String, bool)> = snap
                .get("themes")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| {
                            Some((
                                t.get("id").and_then(Value::as_str)?.to_string(),
                                t.get("label").and_then(Value::as_str)?.to_string(),
                                t.get("path").and_then(Value::as_str)?.to_string(),
                                t.get("uiTheme").and_then(Value::as_str) != Some("vs"),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::ThemesList(themes)));
            let icon_themes: Vec<(String, String, String)> = snap
                .get("iconThemes")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| {
                            Some((
                                t.get("id").and_then(Value::as_str)?.to_string(),
                                t.get("label").and_then(Value::as_str)?.to_string(),
                                t.get("path").and_then(Value::as_str)?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            // Contributed тулы: view-контейнеры activitybar/auxiliarybar +
            // первое вью контейнера (тело-вебвью)
            let dyn_tools: Vec<crate::activity::DynTool> = snap
                .get("viewContainers")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| {
                            let loc = c.get("location").and_then(Value::as_str)?;
                            if loc != "activitybar" && loc != "auxiliarybar" {
                                return None;
                            }
                            let cid = c.get("id").and_then(Value::as_str)?;
                            // ВСЕ вью контейнера, в порядке манифеста:
                            // тело панели рисует их стопкой
                            let views: Vec<crate::activity::DynView> = snap
                                .get("views")
                                .and_then(Value::as_array)
                                .map(|vs| {
                                    vs.iter()
                                        .filter(|v| {
                                            v.get("containerId").and_then(Value::as_str)
                                                == Some(cid)
                                        })
                                        .filter_map(|v| {
                                            Some(crate::activity::DynView {
                                                id: v
                                                    .get("id")
                                                    .and_then(Value::as_str)?
                                                    .to_string(),
                                                name: v
                                                    .get("name")
                                                    .and_then(Value::as_str)
                                                    .unwrap_or("")
                                                    .to_string(),
                                                webview: v.get("type").and_then(Value::as_str)
                                                    == Some("webview"),
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            Some(crate::activity::DynTool {
                                id: cid.to_string(),
                                label: c
                                    .get("title")
                                    .and_then(Value::as_str)
                                    .unwrap_or(cid)
                                    .to_string(),
                                icon: c
                                    .get("icon")
                                    .and_then(Value::as_str)
                                    .unwrap_or("extensions")
                                    .to_string(),
                                location: loc.to_string(),
                                views,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            for v in dyn_tools.iter().flat_map(|t| &t.views) {
                // tree-вью не резолвятся как вебвью — иначе панель ждёт html,
                // которого провайдер никогда не пришлёт
                if v.webview {
                    register_dynamic_webview(&v.id);
                    // ДОРЕЗОЛВ сразу: snapshot приходит ПОЗЖЕ registry:update,
                    // чей resolve-цикл этих id ещё не знал. Без этого contributed
                    // aux-панель (Agents/Todos/Tools) оставалась без attach у
                    // расширения — html жил из дискового кэша, а сообщения ей
                    // никогда не шли («панель агентов пустая», прод + стенд).
                    // resolve_webview сам по потоку; повторный resolve уже
                    // резолвнутого вью гасится на стороне resolve-цикла хоста
                    // дедупом html (rev не меняется — iframe не перегружается).
                    if !crate::host_link::resolved_views()
                        .lock()
                        .unwrap()
                        .contains(&v.id)
                    {
                        crate::host::connect::resolve_webview(v.id.clone());
                    }
                }
            }
            crate::activity::set_dyn_tools(dyn_tools);
            // Нотификация ПОСЛЕ set_dyn_tools: этот event будит рендер,
            // который уже видит заполненный динреестр тулов
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::IconThemesList(icon_themes)));

            let keys: Vec<(String, String, String)> = snap
                .get("keybindings")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|k| {
                            Some((
                                k.get("key").and_then(Value::as_str)?.to_string(),
                                k.get("command").and_then(Value::as_str)?.to_string(),
                                k.get("when")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::KeybindingsList(keys)));
        }
        let _ = tx.try_send(ShellEvent::Cz(CzEvent::StatusCounts(counts)));
        // Contributed statusbar items + Problems-снапшот (диаг-дельты
        // приходят потом через kamin:diag:set)
        if let Ok(items) = client.request("kamin:statusBar:snapshot", vec![]) {
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::StatusBarSnapshot(items)));
        }
        if let Ok(diags) = client.request("kamin:diag:snapshot", vec![]) {
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::DiagSnapshot(diags)));
        }
    });
}
