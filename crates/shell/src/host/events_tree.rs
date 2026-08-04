//! Вариантты `ShellEvent` домена `events_tree`
//! (`plan/100-refactor-250.md`).

use serde_json::Value;

#[derive(Clone)]
pub enum TreeEvent {
    /// Листинг директории: (dirPath, DirEntryDto[]).
    DirListing(String, Value),
    /// Клик по папке в дереве: раскрыть/свернуть.
    ToggleDir(String),
    /// Тулбар дерева: перечитать листинги (root + раскрытые).
    RefreshTree,
    /// Тулбар дерева: свернуть все папки (корень остаётся).
    CollapseTree,
    /// «Show N more» дерева: раскрыть усечённый листинг директории.
    ShowMoreDir(String),
    /// Батч file-decorations: (path, dto|null) из kamin:fileDecoration:get.
    DecoSet(Vec<(String, Value)>),
    /// kamin:fileDecoration:changed: None = refresh all, Some(paths) = точечно.
    DecoInvalidate(Option<Vec<String>>),
    /// Селект узла дерева: (path, ctrl-toggle).
    SelectTreeNode(String, bool, bool),
    /// Показать тост (host `kamin:notification:show`).
    /// Клик по пилюле апдейта — качать и ставить внутри приложения.
    /// Кнопка Refresh в тулбаре дерева: ПОЛНЫЙ ремаунт (кэш и капы детей
    /// сбрасываются, папки снова показывают «Loading…»), как
    /// `FileTreeHeader.tsx:64-71`. Watcher шлёт обычный `RefreshTree` —
    /// тот кэш не чистит, иначе панель мигала бы на каждое событие ФС.
    RefreshTreeHard,
    /// Дети уровня contributed-дерева (view, handle родителя или "").
    TreeChildren {
        view: String,
        parent: String,
        nodes: Vec<crate::ui::contributed_tree::TreeNodeDto>,
    },
    /// Мета вью (createTreeView: title/description/badge/message).
    TreeMetaSet {
        view: String,
        meta: crate::ui::contributed_tree::TreeMeta,
    },
    /// Провайдер дёрнул onDidChangeTreeData → перечитать уровни вью.
    TreeChanged(String),
    /// Клик по строке дерева: тоггл + выделение + команда узла.
    TreeClick {
        view: String,
        handle: String,
        expandable: bool,
        expanded: bool,
        command: Option<(String, Vec<Value>)>,
    },
    /// `kamin:view:reveal` — (container, view?): раскрыть contributed-вью.
    RevealView(String, Option<String>),
    /// `kamin:tree:reveal` — раскрыть предков, выделить и проскроллить узел.
    /// `tries` — счётчик повторов, пока уровни ещё грузятся.
    TreeReveal {
        view: String,
        handle: String,
        expand_path: Vec<String>,
        select: bool,
        expand: bool,
        tries: u8,
    },
    /// `kamin:tree:dnd` — вью зарегистрировала DnD-контроллер.
    TreeDnd { view: String, enabled: bool },
    /// Начало перетаскивания узла (`handleDrag`).
    TreeDragStart { view: String, handle: String },
    /// Дроп на узел (`handleDrop`); `handle` — цель.
    TreeDrop { view: String, handle: String },
    /// Тоггл чекбокса узла (репорт провайдеру).
    TreeCheckbox {
        view: String,
        handle: String,
        state: i64,
    },
}
