//! Типы contributed-дерева: узел, метаданные, состояние вью, флаги.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use serde_json::Value;

// TreeItemCollapsibleState / TreeItemCheckboxState
pub const NONE: i64 = 0;
pub const EXPANDED: i64 = 2;
pub const CHECKED: i64 = 1;
pub const UNCHECKED: i64 = 0;
/// Узел от провайдера (`TreeNodeDto`).
#[derive(Clone, Debug)]
pub struct TreeNodeDto {
    pub handle: String,
    pub label: String,
    pub description: Option<String>,
    pub tooltip: Option<String>,
    pub codicon: Option<String>,
    pub resource_uri: Option<String>,
    pub collapsible: i64,
    pub checkbox: Option<i64>,
    pub checkbox_tooltip: Option<String>,
    pub command: Option<(String, Vec<Value>)>,
}
/// Мета вью от `createTreeView` (title/description/badge/message).
#[derive(Clone, Default, Debug)]
pub struct TreeMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub message: Option<String>,
    pub badge: Option<(String, Option<String>)>,
}
/// Состояние одного tree-вью в RootView.
#[derive(Default)]
pub struct TreeViewState {
    pub meta: TreeMeta,
    /// Уровни: ключ — handle родителя, "" — корень. None-значение = грузится.
    pub levels: std::collections::HashMap<String, Option<Vec<TreeNodeDto>>>,
    pub expanded: std::collections::HashMap<String, bool>,
    pub selected: Option<String>,
    /// Вью зарегистрировала `TreeDragAndDropController` (`kamin:tree:dnd`).
    pub dnd: bool,
    /// Корень уже запрашивали. Отдельный флаг, а не «есть ли состояние»:
    /// состояние создаёт и широковещательная мета (`kamin:tree:meta`), и
    /// тогда первый показ вью пропускал запрос детей и висел на «Loading…».
    pub root_requested: bool,
    /// Программный скролл тела (reveal → scroll-to-row).
    pub scroll: gpui::ScrollHandle,
    /// Хэндл-измеритель строк: `overflow_y_scrollbar_with` кладёт наш список
    /// ОДНИМ ребёнком скролл-области, поэтому bounds строк снимаем отдельным
    /// track_scroll на самом списке (сам он не скроллится).
    pub rows: gpui::ScrollHandle,
    /// Одноразовый `treeReveal`: handle узла, к которому надо проскроллить.
    /// Снимается рендером, чтобы перемонтирование не скроллило повторно.
    pub reveal: std::cell::RefCell<Option<String>>,
}
