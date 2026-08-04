//! Ветка probe-эмита: панели, терминал, файловые операции и меню
//! (`plan/100-refactor-250.md`). Разбор аргументов — как в `probe_emit`.

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host::events::TermEvent;
use crate::host::events::TreeEvent;
use crate::host_link::ShellEvent;
use serde_json::Value;

pub(crate) fn emit_tools(kind: &str, req: &Value, sid: String, name: String) -> Option<ShellEvent> {
    Some(match kind {
        // {"cmd":"emit","kind":"pinTool","name":"mainBottom","id":"terminal"}
        "pinTool" => {
            use crate::activity::PanelSlot;
            let slot = PanelSlot::ALL
                .into_iter()
                .find(|s| s.as_str() == name)
                .unwrap_or(PanelSlot::MainBottom);
            ShellEvent::PinTool(slot, sid)
        }
        // {"cmd":"emit","kind":"termWrite","name":"dir\r"} — байты в PTY
        "termWrite" => ShellEvent::Term(TermEvent::TermInput(name)),
        // {"cmd":"emit","kind":"termScroll","line":10} — скролл вьюпорта
        "termScroll" => ShellEvent::Term(TermEvent::TermScroll(
            req.get("line").and_then(Value::as_i64).unwrap_or(5) as i32,
        )),
        "termMenu" => ShellEvent::Term(TermEvent::ToggleTermMenu),
        // {"cmd":"emit","kind":"quickPick","dir":true(=canPickMany)} — дев-пик
        "quickPick" => ShellEvent::QuickPickShow(
            0,
            serde_json::json!([
                {"label": "First option", "description": "desc one"},
                {"label": "Second option", "picked": true},
                {"label": "Group", "kind": -1},
                {"label": "Third option", "detail": "with detail"},
            ]),
            serde_json::json!({
                "title": "Probe Quick Pick",
                "placeHolder": "Filter options…",
                "canPickMany": req.get("dir").and_then(Value::as_bool).unwrap_or(false),
            }),
        ),
        // fs-операции для live-верификации undo-стека
        "fsCopy" => ShellEvent::Ed(EdEvent::FsCopy(vec![name])),
        "fsPaste" => ShellEvent::Ed(EdEvent::FsPaste(name)),
        "fsDelete" => ShellEvent::Ed(EdEvent::FsDelete(name)),
        "fsUndo" => ShellEvent::Ed(EdEvent::UndoFsOp),
        // {"cmd":"emit","kind":"showMore","name":"C:/dir"} — раскрыть кап
        "showMore" => ShellEvent::Tree(TreeEvent::ShowMoreDir(name)),
        // {"cmd":"emit","kind":"selectNode","name":"C:\path","ctrl":true} —
        // селект узла дерева (Ctrl синтетически не инъектируется)
        // {"cmd":"emit","kind":"newSessionMenu","x":..,"y":..}
        "newSessionMenu" => ShellEvent::ToggleNewSessionMenu(
            req.get("x").and_then(Value::as_f64).unwrap_or(900.0) as f32,
            req.get("y").and_then(Value::as_f64).unwrap_or(20.0) as f32,
        ),
        "selectNode" => ShellEvent::Tree(TreeEvent::SelectTreeNode(
            name,
            req.get("ctrl").and_then(Value::as_bool).unwrap_or(false),
            req.get("shift").and_then(Value::as_bool).unwrap_or(false),
        )),
        // {"cmd":"emit","kind":"pinTab","line":2} — pin/unpin файл-таба
        "pinTab" => ShellEvent::Ed(EdEvent::TogglePinEditorTab(
            req.get("line").and_then(Value::as_u64).unwrap_or(0) as usize,
        )),
        // {"cmd":"emit","kind":"tabMenu","line":0,"name":"C:/path","x":..,"y":..}
        "tabMenu" => ShellEvent::Ed(EdEvent::OpenEditorTabMenu(
            req.get("line").and_then(Value::as_u64).unwrap_or(0) as usize,
            name,
            req.get("x").and_then(Value::as_f64).unwrap_or(600.0) as f32,
            req.get("y").and_then(Value::as_f64).unwrap_or(120.0) as f32,
        )),
        // {"cmd":"emit","kind":"termNew","name":"cmd"} — новый шелл по профилю
        "termNew" => ShellEvent::Term(TermEvent::TermNew(name)),
        // {"cmd":"emit","kind":"layoutFlag","name":"main"} — тумблер панели
        "layoutFlag" => ShellEvent::ToggleLayoutFlag(match name.as_str() {
            "mainBottom" => "mainBottom",
            "file" => "file",
            "fileBottom" => "fileBottom",
            "right" => "right",
            "rightBottom" => "rightBottom",
            _ => "main",
        }),
        "layoutPopover" => ShellEvent::ToggleLayoutPopover,
        "appearance" => ShellEvent::ToggleAppearancePopover,
        // {"cmd":"emit","kind":"contribTheme","id":"<themeId>","name":"<путь
        // к json>","dir":true(=dark uiTheme)} — применить contributed-тему
        // (стенды матрицы тем: вернуть тему юзера после прогонов dark/light)
        "contribTheme" => ShellEvent::Cz(CzEvent::SetContributedTheme(
            sid,
            name,
            req.get("dir").and_then(Value::as_bool).unwrap_or(true),
        )),
        "customizePanel" => ShellEvent::Cz(CzEvent::SetCustomizePanel(
            crate::ui::customize::PANELS
                .iter()
                .find(|(id, _, _)| *id == name)
                .map(|(id, _, _)| *id)
                .unwrap_or("settings"),
        )),
        // {"cmd":"emit","kind":"toolPicker","name":"mainBottom","x":..,"y":..}
        "toolPicker" => {
            use crate::activity::PanelSlot;
            let slot = PanelSlot::ALL
                .into_iter()
                .find(|s| s.as_str() == name)
                .unwrap_or(PanelSlot::MainBottom);
            ShellEvent::OpenToolPicker(
                slot,
                req.get("x").and_then(Value::as_f64).unwrap_or(600.0) as f32,
                req.get("y").and_then(Value::as_f64).unwrap_or(700.0) as f32,
                true,
            )
        }
        _ => return None,
    })
}
