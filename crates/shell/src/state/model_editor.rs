//! Поля `RootView`, вынесенные группой `ed`
//! (`plan/100-refactor-250.md`).

use crate::state::drag::TabDrag;
use crate::state::editor_tab::EditorTab;
use gpui::Entity;
use gpui_component::input::InputState;

#[derive(Default)]
pub struct EditorState {
    /// RMB-меню файл-таба редактора.
    pub editor_tab_menu: Option<crate::ui::editor_tabs::EditorTabMenu>,
    /// Sticky-scroll кэш: (path, first_visible, липкие (idx, текст)).
    pub sticky_cache: (String, usize, Vec<(usize, String)>),
    /// Drag-reorder файл-таба: (src, start, started, over-таб).
    pub tab_drag: Option<TabDrag>,
    /// Поповер «N ▾» скрытых файл-табов.
    pub file_tabs_overflow_open: bool,
    /// Пути на reload в render (внешние изменения из watcher) +
    /// подавление dirty от программного set_value.
    pub pending_reload: Vec<String>,
    pub reload_suppress: std::collections::HashSet<String>,
    /// Переход к строке после открытия файла (scroll-to-line из поиска).
    pub pending_goto: Option<(String, u32)>,
    pub minimap_stale: bool,
    /// Зеркальный редактор минимапы (Zed: отдельный `minimap_editor`).
    pub minimap_input: Option<Entity<InputState>>,
    /// Путь → текст ошибки чтения: вкладка рисует карточку `.error`.
    pub editor_errors: std::collections::HashMap<String, String>,
    /// Открытый файл-просмотрщик (path + строки + целевая строка).
    /// Scroll-хэндл viewer (для scroll_to_item на строку из поиска).
    /// РЕДАКТОР центра: мульти-табы (code_editor per файл) + активный.
    pub editor_tabs: Vec<EditorTab>,
    pub editor_active: usize,
    /// Сессия, чьи файл-табы сейчас загружены (табы привязаны к сессии,
    /// как веб-страницы в Web-режиме); None = ещё не привязаны.
    pub(crate) editor_tabs_session: Option<String>,
    /// Отложенные наборы табов других сессий: id → (tabs, active).
    pub(crate) editor_tabs_stash: std::collections::HashMap<String, (Vec<EditorTab>, usize)>,
    /// Файл, ждущий создания редактора (нужен window): (path, text).
    pub pending_editor: Option<(String, String)>,
    /// Скролл стрипа файл-табов (plan/99 п.42: стрип прокручивается, а не
    /// только режется overflow'ом).
    pub tabs_scroll: gpui::ScrollHandle,
    /// true → на ближайшем рендере стрип довозит АКТИВНЫЙ таб в видимую
    /// зону (ставится при смене активного таба, снимается рендером).
    pub tabs_reveal_active: bool,
}
