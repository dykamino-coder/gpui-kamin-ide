//! LSP поверх code_editor (Zed-паттерн gpui-component: InputState.lsp.*_provider)
//! через exthost-каналы хоста — ТЕ ЖЕ провайдеры расширений, что у Monaco
//! оригинала: kamin:lang:hover / kamin:lang:definition. Ключ документа =
//! абсолютный fsPath (DocState.uri хоста). Синк: kamin:doc:open при открытии
//! таба; before-request re-open, если буфер менялся (полный текст —
//! инкременты не нужны при наших размерах); kamin:doc:close при закрытии.

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::{Result, anyhow};
use gpui::{App, Task, Window};
use gpui_component::input::{Rope, RopeExt};
use serde_json::{Value, json};

/// Провайдер hover/definition для ОДНОГО документа (свой на таб).
pub struct HostLsp {
    /// Абсолютный fsPath — ключ документа в mirror хоста.
    pub path: String,
    language_id: String,
    version: Cell<i64>,
    last_hash: Cell<u64>,
}

fn hash_str(text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

impl HostLsp {
    pub fn new(path: &str, language_id: &str) -> Self {
        Self {
            path: path.to_string(),
            language_id: language_id.to_string(),
            version: Cell::new(1),
            last_hash: Cell::new(0),
        }
    }

    /// Открыть документ в mirror хоста (звать при создании таба).
    pub fn open(&self, text: &str) {
        self.last_hash.set(hash_str(text));
        let payload = json!({
            "uri": &self.path,
            "languageId": &self.language_id,
            "version": self.version.get(),
            "content": text,
        });
        std::thread::spawn(move || {
            if let Some(client) = crate::host_link::client() {
                let _ = client.request("kamin:doc:open", vec![payload]);
            }
        });
    }

    /// Закрыть документ (звать при закрытии таба).
    pub fn close(path: String) {
        std::thread::spawn(move || {
            if let Some(client) = crate::host_link::client() {
                let _ = client.request("kamin:doc:close", vec![json!(path)]);
            }
        });
    }

    /// Полный re-open, если буфер менялся с последнего синка (child
    /// syncDocOpen перезаписывает состояние — инкременты не нужны).
    fn sync_payload(&self, text: &Rope) -> Option<Value> {
        let full = text.to_string();
        let h = hash_str(&full);
        if h == self.last_hash.get() {
            return None;
        }
        self.last_hash.set(h);
        self.version.set(self.version.get() + 1);
        Some(json!({
            "uri": &self.path,
            "languageId": &self.language_id,
            "version": self.version.get(),
            "content": full,
        }))
    }
}

fn dto_range(v: &Value) -> Option<lsp_types::Range> {
    Some(lsp_types::Range {
        start: lsp_types::Position {
            line: v.get("startLine")?.as_u64()? as u32,
            character: v.get("startChar")?.as_u64()? as u32,
        },
        end: lsp_types::Position {
            line: v.get("endLine")?.as_u64()? as u32,
            character: v.get("endChar")?.as_u64()? as u32,
        },
    })
}

impl gpui_component::input::HoverProvider for HostLsp {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Option<lsp_types::Hover>>> {
        let pos = text.offset_to_position(offset);
        let uri = self.path.clone();
        let sync = self.sync_payload(text);
        cx.background_executor().spawn(async move {
            let Some(client) = crate::host_link::client() else {
                return Ok(None);
            };
            if let Some(doc) = sync {
                let _ = client.request("kamin:doc:open", vec![doc]);
            }
            let v = client
                .request(
                    "kamin:lang:hover",
                    vec![json!(uri), json!(pos.line), json!(pos.character)],
                )
                .map_err(|e| anyhow!(e))?;
            // HoverDto[]: contents (markdown-строки) склеиваем; range первого
            let Some(arr) = v.as_array().filter(|a| !a.is_empty()) else {
                return Ok(None);
            };
            let mut parts: Vec<String> = Vec::new();
            let mut range = None;
            for dto in arr {
                if range.is_none() {
                    range = dto.get("range").and_then(dto_range);
                }
                if let Some(cs) = dto.get("contents").and_then(Value::as_array) {
                    parts.extend(cs.iter().filter_map(|c| c.as_str().map(str::to_string)));
                }
            }
            if parts.is_empty() {
                return Ok(None);
            }
            Ok(Some(lsp_types::Hover {
                contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                    kind: lsp_types::MarkupKind::Markdown,
                    value: parts.join("\n\n"),
                }),
                range,
            }))
        })
    }
}

impl gpui_component::input::DefinitionProvider for HostLsp {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<lsp_types::LocationLink>>> {
        let pos = text.offset_to_position(offset);
        let uri = self.path.clone();
        let sync = self.sync_payload(text);
        cx.background_executor().spawn(async move {
            let Some(client) = crate::host_link::client() else {
                return Ok(Vec::new());
            };
            if let Some(doc) = sync {
                let _ = client.request("kamin:doc:open", vec![doc]);
            }
            let v = client
                .request(
                    "kamin:lang:definition",
                    vec![json!(uri), json!(pos.line), json!(pos.character)],
                )
                .map_err(|e| anyhow!(e))?;
            let mut out = Vec::new();
            if let Some(arr) = v.as_array() {
                for dto in arr {
                    // LocationDto: uri = fsPath ИЛИ file://-URL — оба в Uri
                    let Some(u) = dto.get("uri").and_then(Value::as_str) else {
                        continue;
                    };
                    let uri_str = if u.starts_with("file:") {
                        u.to_string()
                    } else {
                        // fsPath → file-URI (пробелы кодируем; остального в
                        // наших путях достаточно)
                        format!("file:///{}", u.replace('\\', "/").replace(' ', "%20"))
                    };
                    let Ok(target_uri) = uri_str.parse::<lsp_types::Uri>() else {
                        continue;
                    };
                    let Some(range) = dto.get("range").and_then(dto_range) else {
                        continue;
                    };
                    let sel = dto
                        .get("targetSelectionRange")
                        .and_then(dto_range)
                        .unwrap_or(range);
                    out.push(lsp_types::LocationLink {
                        origin_selection_range: None,
                        target_uri,
                        target_range: range,
                        target_selection_range: sel,
                    });
                }
            }
            Ok(out)
        })
    }
}
