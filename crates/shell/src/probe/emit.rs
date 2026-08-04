//! Инъекция UI-события probe: разбор kind → ShellEvent.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host::events::TermEvent;
use crate::host::events::TreeEvent;
use serde_json::{Value, json};

/// Инъекция оверлей-события в UI-канал (dev-верификация без клик-инъекции).
pub(crate) fn emit(req: &Value) -> Value {
    use crate::host_link::ShellEvent;

    let Some(tx) = crate::host_link::event_tx() else {
        return json!({"ok": false, "err": "event channel not ready"});
    };
    let kind = req.get("kind").and_then(Value::as_str).unwrap_or("");
    let sid = req
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("probe-session")
        .to_string();
    let name = req
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Probe session")
        .to_string();
    // Ветки сессий/оверлеев и файлов/вида живут по соседству; здесь —
    // остальное (`plan/100-refactor-250.md`).
    if let Some(ev) = crate::probe::emit_ui::emit_session(kind, req, sid.clone(), name.clone())
        .or_else(|| crate::probe::emit_view::emit_view(kind, req, sid.clone(), name.clone()))
        .or_else(|| crate::probe::emit_tools::emit_tools(kind, req, sid.clone(), name.clone()))
    {
        return match tx.try_send(ev) {
            Ok(()) => json!({"ok": true}),
            Err(e) => json!({"ok": false, "err": e.to_string()}),
        };
    }
    let ev = match kind {
        // {"cmd":"emit","kind":"theme","name":"light"|"dark"|"system"} —
        // переключение темы для сверки светлых веток (иконки, тени)
        "theme" => ShellEvent::SetThemeChoice(match name.as_str() {
            "light" => "light",
            "system" => "system",
            _ => "dark",
        }),
        // {"cmd":"emit","kind":"treeDnd","name":"<viewId>","dir":true} —
        // эмуляция `kamin:tree:dnd` (у вью появился DnD-контроллер)
        "treeDnd" => ShellEvent::Tree(TreeEvent::TreeDnd {
            view: name.clone(),
            enabled: req.get("dir").and_then(Value::as_bool).unwrap_or(true),
        }),
        // {"cmd":"emit","kind":"treeReveal","name":"<viewId>","id":"<handle>"}
        "treeReveal" => ShellEvent::Tree(TreeEvent::TreeReveal {
            view: name.clone(),
            handle: req
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            expand_path: Vec::new(),
            select: true,
            expand: req.get("dir").and_then(Value::as_bool).unwrap_or(false),
            tries: 0,
        }),
        // {"cmd":"emit","kind":"focusNext","dir":false} — Tab/Shift+Tab:
        // клавиши в gpui probe не доставляет, а `:focus-visible` живёт именно
        // на клавиатурной модальности
        "focusNext" => {
            ShellEvent::FocusStep(!req.get("dir").and_then(Value::as_bool).unwrap_or(false))
        }
        "palette" => ShellEvent::TogglePalette,
        "quickopen" => ShellEvent::ToggleQuickOpen,
        // {"cmd":"emit","kind":"overlayMove","delta":1} — ↑/↓ в оверлее
        "overlayMove" => {
            ShellEvent::OverlayMove(req.get("delta").and_then(Value::as_i64).unwrap_or(1) as i32)
        }
        // {"cmd":"emit","kind":"overlayQuery","target":"fif","name":"impl"} —
        // текст в инпут ОТКРЫТОГО оверлея. `type` бесполезен: WM_CHAR gpui не
        // разбирает, а без запроса списки пусты и сверять нечего
        "overlayQuery" => {
            let target: &'static str = match req.get("target").and_then(Value::as_str) {
                Some("fif") => "fif",
                Some("qo") => "qo",
                Some("ws") => "ws",
                Some("palette") => "palette",
                Some("qp") => "qp",
                _ => return json!({"ok": false, "err": "target: fif|qo|ws|palette|qp"}),
            };
            ShellEvent::SetOverlayQuery(target, name)
        }
        // {"cmd":"emit","kind":"extToast","name":"info|success|warning|error"}
        // — ВНЕШНИЙ тост отдельным окном (кнопки семпла шлют то же событие)
        // {"cmd":"emit","kind":"toast","name":"текст"} — ВНУТРЕННИЙ тост
        // (стек в окне), для стендов позиционирования.
        "toast" => ShellEvent::Toast(crate::ui::toasts::Toast {
            id: format!(
                "probe-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            ),
            severity: "info".into(),
            title: None,
            message: if name.is_empty() {
                "Probe internal toast".into()
            } else {
                name.to_string()
            },
            sticky: false,
            actions: Vec::new(),
        }),
        // {"cmd":"emit","kind":"applyPreset","name":"<имя пресета>"}
        "applyPreset" => ShellEvent::ApplyLayoutPreset(name.clone()),
        "extToast" => ShellEvent::ExternalToast(crate::ui::toasts::Toast {
            id: format!("probe-{name}"),
            severity: if name.is_empty() {
                "info".into()
            } else {
                name.clone()
            },
            title: Some("Probe toast".into()),
            message: "External toast from probe — hover pauses the timer bar.".into(),
            actions: if req.get("dir").and_then(Value::as_bool).unwrap_or(false) {
                vec!["Retry".into(), "Show log".into()]
            } else {
                Vec::new()
            },
            sticky: req.get("line").and_then(Value::as_i64).unwrap_or(0) == 1,
        }),
        // {"cmd":"emit","kind":"dsDropdown"} — тоггл дропдауна семпла
        "dsDropdown" => ShellEvent::Cz(CzEvent::DesignSample(
            crate::ui::design_samples::DesignAction::ToggleDropdown,
        )),
        // {"cmd":"emit","kind":"syslog","name":"…"} — строка в системный лог
        "syslog" => ShellEvent::Cz(CzEvent::PushSysLog(name)),
        // {"cmd":"emit","kind":"editorFind"} — Ctrl+F в активном табе редактора
        "editorFind" => ShellEvent::Ed(EdEvent::EditorFind),
        "findInFiles" => ShellEvent::ToggleFindInFiles,
        "workspaceSymbols" => ShellEvent::ToggleWorkspaceSymbols,
        "devtools" => ShellEvent::OpenDevtools(if name.is_empty() {
            "claudeBridgeChat".into()
        } else {
            name
        }),
        "customize" => ShellEvent::Cz(CzEvent::ToggleCustomize),
        // {"cmd":"emit","kind":"czPanel","name":"system"} — экран Customize
        // БЕЗ слепых кликов по наву (клик мимо переключал сессию)
        "czPanel" => {
            let panel: &'static str = match name.as_str() {
                "design" => "design",
                "extensions" => "extensions",
                "logs" => "logs",
                "system" => "system",
                _ => "settings",
            };
            ShellEvent::Cz(CzEvent::SetCustomizePanel(panel))
        }
        // {"cmd":"emit","kind":"czOpen"} — переключить режим Customize
        "czOpen" => ShellEvent::Cz(CzEvent::ToggleCustomize),
        // {"cmd":"emit","kind":"czContrib","name":"claudeBridgeCzSettings"} —
        // contributed-страница Customize (вебвью расширения) по её view-id
        "czContrib" => ShellEvent::Cz(CzEvent::SetCustomizeContribPage(name.clone())),
        // {"cmd":"emit","kind":"fileMenu","name":"C:\\path","dir":true,"x":..,"y":..}
        "fileMenu" => ShellEvent::Ed(EdEvent::OpenFileMenu(
            name,
            req.get("dir").and_then(Value::as_bool).unwrap_or(true),
            req.get("x").and_then(Value::as_f64).unwrap_or(600.0) as f32,
            req.get("y").and_then(Value::as_f64).unwrap_or(300.0) as f32,
        )),
        // {"cmd":"emit","kind":"fileMenuOpenIn","dir":true} — каскад «Open In ▸».
        // Открывается только по ховеру, а probe-ховер до gpui не доходит, так
        // что без этого ветка не проверяема
        "fileMenuOpenIn" => ShellEvent::Ed(EdEvent::FileMenuOpenIn(
            req.get("dir").and_then(Value::as_bool).unwrap_or(true),
        )),
        // {"cmd":"emit","kind":"viewLoadError","name":"<viewId>"} — карточка
        // терминальной ошибки загрузки вебвью-панели
        "viewLoadError" => ShellEvent::Term(TermEvent::ForceViewLoadError(name)),
        "close" => {
            let _ = tx.try_send(ShellEvent::CloseSessionMenu);
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
            let _ = tx.try_send(ShellEvent::CloseToolPicker);
            let _ = tx.try_send(ShellEvent::ClosePalette);
            ShellEvent::CloseModal
        }
        other => return json!({"ok": false, "err": format!("unknown emit kind: {other}")}),
    };
    match tx.try_send(ev) {
        Ok(()) => json!({"ok": true}),
        Err(e) => json!({"ok": false, "err": e.to_string()}),
    }
}
