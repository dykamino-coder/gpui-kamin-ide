//! Таб редактора: файл, его буфер и признаки состояния.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use gpui::Entity;
use gpui_component::input::InputState;

/// Таб редактора: файл + его code_editor-буфер.
pub struct EditorTab {
    pub path: String,
    pub input: Entity<InputState>,
    pub dirty: bool,
    /// EOL файла на момент открытия ("LF"|"CRLF") — статус-бар.
    pub eol: &'static str,
    /// Последняя активность таба — LRU-вытеснение при лимите 12.
    pub last_used: std::time::Instant,
    /// Pinned: всегда слева, LRU не выселяет.
    pub pinned: bool,
    pub(crate) _sub: gpui::Subscription,
}
