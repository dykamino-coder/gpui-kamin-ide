//! Страницы Customize из снапшота реестра вью.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::connect::register_dynamic_webview;
pub use crate::host::events::{CzContainer, CzPage};
use serde_json::Value;

/// Contributed-страницы Customize из снапшота реестра: вью, чей контейнер
/// объявлен с `location = "customize"`. Каждый id сразу регистрируется как
/// известный вебвью — иначе `webview_known` отсеет и его HTML, и сообщения.
pub(crate) fn customize_pages_from(snap: &Value) -> Vec<CzContainer> {
    let empty: Vec<Value> = Vec::new();
    let views = snap
        .get("views")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut out: Vec<CzContainer> = Vec::new();
    let Some(containers) = snap.get("viewContainers").and_then(Value::as_array) else {
        return out;
    };
    // Оригинал (`CustomizeMode.tsx:82`) рисует УЗЕЛ НА КАЖДЫЙ customize-контейнер
    for c in containers
        .iter()
        .filter(|c| c.get("location").and_then(Value::as_str) == Some("customize"))
    {
        let Some(cid) = c.get("id").and_then(Value::as_str) else {
            continue;
        };
        let pages: Vec<CzPage> = views
            .iter()
            .filter(|v| v.get("containerId").and_then(Value::as_str) == Some(cid))
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
    out
}
