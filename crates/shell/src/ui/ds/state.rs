//! Состояние семплов дизайн-системы и его действия.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

/// Уникальный суффикс id тоста семпла (в оригинале — `Date.now()`).
pub(crate) fn next_id() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
/// Состояние интерактивных семплов Design-панели (живёт в RootView).
#[derive(Clone)]
pub struct DesignState {
    /// `DropdownRow`: открыт ли список тем.
    pub dropdown_open: bool,
    /// `DropdownRow`: выбранный пункт (id), начальный — "dark".
    pub picked: String,
    /// `CheckboxDropdownRow`: A/B/C, начальные true/false/true.
    pub checks: [bool; 3],
    /// `TreeRow`: раскрытые узлы семпла (`useState`, начальные src + src/host).
    pub tree_expanded: std::collections::HashSet<String>,
    /// `TreeRow`: выбранный узел семпла.
    pub tree_selected: String,
    /// `TabStripRow`: активный таб семпла (`useState("terminal")`).
    pub strip_tab: String,
    /// `IconColumnRow`: активная плитка семпла (`useState("projects")`).
    pub column_tile: String,
}
/// Что кликнули в семплах (маршрутизируется в RootView).
#[derive(Clone)]
pub enum DesignAction {
    ToggleDropdown,
    Pick(String),
    ToggleCheck(usize),
    /// Клик по строке семпл-дерева: id + это папка (тогда ещё и раскрытие).
    TreeClick(String, bool),
    /// Клик по табу горизонтального стрипа-семпла.
    PickStripTab(String),
    /// Клик по плитке колонки-семпла.
    PickColumnTile(String),
}
