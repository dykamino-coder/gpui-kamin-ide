//! Страницы Customize прямо из манифестов builtin-расширений (без RPC).
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::connect::register_dynamic_webview;
pub use crate::host::events::{CzContainer, CzPage};
use crate::host::paths::dev_repo;
use serde_json::Value;

/// Страницы Customize ПРЯМО ИЗ МАНИФЕСТОВ builtin-расширений, без RPC к хосту.
///
/// Снапшот реестра идёт через exthost-ребёнка и на холодном старте отвечает
/// десятками секунд (замер: `kamin:extensions:list` упирался в 60-секундный
/// таймаут клиента, пока поднимались MCP-серверы). Ждать его, чтобы показать
/// НАВИГАЦИЮ, незачем: `contributes.viewsContainers.customize` и
/// `contributes.views[<container>]` — статические данные на диске.
pub fn customize_pages_from_manifests() -> Vec<CzContainer> {
    let dir = dev_repo().join("builtin-extensions");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<CzContainer> = Vec::new();
    for ext in rd.filter_map(std::result::Result::ok) {
        let Ok(raw) = std::fs::read_to_string(ext.path().join("package.json")) else {
            continue;
        };
        let Ok(pkg) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let contributes = pkg.get("contributes");
        // Контейнеры, объявленные в location = customize
        let cz: Vec<&Value> = contributes
            .and_then(|c| c.get("viewsContainers"))
            .and_then(|v| v.get("customize"))
            .and_then(Value::as_array)
            .map(|arr| arr.iter().collect())
            .unwrap_or_default();
        let Some(views) = contributes.and_then(|c| c.get("views")) else {
            continue;
        };
        for c in cz {
            let Some(cid) = c.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(arr) = views.get(cid).and_then(Value::as_array) else {
                continue;
            };
            let pages: Vec<CzPage> = arr
                .iter()
                .filter_map(|v| {
                    let id = v.get("id").and_then(Value::as_str)?;
                    register_dynamic_webview(id);
                    Some(CzPage {
                        id: id.to_string(),
                        name: v
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or(id)
                            .to_string(),
                        icon: v
                            .get("icon")
                            .and_then(Value::as_str)
                            .unwrap_or("circle-small")
                            .to_string(),
                    })
                })
                .collect();
            out.push(CzContainer {
                id: cid.to_string(),
                title: c
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(cid)
                    .to_string(),
                icon: c
                    .get("icon")
                    .and_then(Value::as_str)
                    .unwrap_or("circle-small")
                    .to_string(),
                views: pages,
            });
        }
    }
    out
}
