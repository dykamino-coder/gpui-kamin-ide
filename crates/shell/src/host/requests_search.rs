//! Запросы поиска и чтения: find-in-files, символы воркспейса, файл, команды.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::events::EdEvent;
pub use crate::host::events::ShellEvent;
use crate::host_link::client;
use serde_json::Value;
use smol::channel::Sender;

/// Фоновый текстовый поиск (kamin:index:findInFiles) → FindInFilesResults.
pub fn request_find_in_files(tx: Sender<ShellEvent>, query: String) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let Ok(v) = client.request("kamin:index:findInFiles", vec![serde_json::json!(query)])
        else {
            let _ = tx.try_send(ShellEvent::FindInFilesResults(Vec::new()));
            return;
        };
        let hits = v
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|h| {
                        Some(crate::ui::find_in_files::TextHit {
                            rel: h.get("rel")?.as_str()?.to_string(),
                            abs: h.get("abs")?.as_str()?.to_string(),
                            line: h.get("line").and_then(Value::as_u64).unwrap_or(0) as u32,
                            match_start: h.get("matchStart").and_then(Value::as_u64).unwrap_or(0)
                                as usize,
                            match_end: h.get("matchEnd").and_then(Value::as_u64).unwrap_or(0)
                                as usize,
                            snippet: h
                                .get("snippet")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let _ = tx.try_send(ShellEvent::FindInFilesResults(hits));
    });
}

/// Фоновый поиск символов (kamin:lang:workspaceSymbol) → WorkspaceSymbolsResults.
pub fn request_workspace_symbols(tx: Sender<ShellEvent>, query: String) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let Ok(v) = client.request("kamin:lang:workspaceSymbol", vec![serde_json::json!(query)])
        else {
            let _ = tx.try_send(ShellEvent::WorkspaceSymbolsResults(Vec::new()));
            return;
        };
        let hits = v
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|h| {
                        Some(crate::ui::workspace_symbols::SymbolHit {
                            name: h.get("name")?.as_str()?.to_string(),
                            kind: h.get("kind").and_then(Value::as_u64).unwrap_or(0) as u32,
                            container: h
                                .get("containerName")
                                .and_then(Value::as_str)
                                .map(String::from),
                            uri: h.get("uri")?.as_str()?.to_string(),
                            // reveal-диапазон: открытие прыгает к символу
                            line: h
                                .get("range")
                                .and_then(|r| r.get("startLine"))
                                .and_then(Value::as_u64)
                                .map(|l| l as u32 + 1),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let _ = tx.try_send(ShellEvent::WorkspaceSymbolsResults(hits));
    });
}

/// Фоновое чтение файла → ShellEvent::Ed(EdEvent::FileOpened). Крупные файлы усечём.
pub fn request_read_file(tx: Sender<ShellEvent>, path: String, target: Option<u32>) {
    std::thread::spawn(move || {
        // Напрямую с диска (как listDir): мгновенно, не зависит от готовности
        // WS/хоста и не голодает под индексером.
        match std::fs::read_to_string(&path) {
            Ok(mut text) => {
                // Защита от гигантских файлов
                const MAX: usize = 2_000_000;
                if text.len() > MAX {
                    let mut cut = MAX;
                    while cut < text.len() && !text.is_char_boundary(cut) {
                        cut += 1;
                    }
                    text.truncate(cut);
                    text.push_str("\n\n… (truncated)");
                }
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::FileOpened(path, text, target)));
            }
            Err(e) => {
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::FileOpenFailed(path, e.to_string())));
            }
        }
    });
}

/// Фоновый запрос реестра команд → ShellEvent::Commands.
pub fn request_commands(tx: Sender<ShellEvent>) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let Ok(snap) = client.request("kamin:registry:snapshot", vec![]) else {
            return;
        };
        let cmds = snap
            .get("commands")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        Some(crate::ui::command_palette::CommandItem {
                            id: c.get("id")?.as_str()?.to_string(),
                            title: c.get("title")?.as_str()?.to_string(),
                            category: c.get("category").and_then(Value::as_str).map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        // `contributes.menus.commandPalette` + ключи контекста реестра:
        // без них палитра показывает команды, которые оригинал прячет
        let gate: Vec<(String, String)> = snap
            .get("menus")
            .and_then(|m| m.get("commandPalette"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        Some((
                            m.get("command")?.as_str()?.to_string(),
                            m.get("when")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let ctx: Vec<(String, Value)> = snap
            .get("contextKeys")
            .and_then(Value::as_object)
            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let _ = tx.try_send(ShellEvent::Commands(cmds));
        let _ = tx.try_send(ShellEvent::PaletteGate(gate, ctx));
    });
}
