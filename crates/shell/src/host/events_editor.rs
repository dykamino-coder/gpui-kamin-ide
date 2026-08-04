//! Вариантты `ShellEvent` домена `events_editor`
//! (`plan/100-refactor-250.md`).

use super::events::*;

#[derive(Clone)]
pub enum EdEvent {
    /// Каскад «Open In ▸» файлового меню (true = открыт).
    FileMenuOpenIn(bool),
    /// Locate: раскрыть предков выбранного файла в дереве и выделить его.
    LocateSelectedFile,
    /// RMB по узлу дерева: (path, is_dir, x, y).
    OpenFileMenu(String, bool, f32, f32),
    CloseFileMenu,
    /// Файловый буфер: пометить cut/copy; вставить в каталог.
    FsCut(Vec<String>),
    FsCopy(Vec<String>),
    FsPaste(String),
    /// Drop НА ПАПКУ дерева (plan/99 п.34): свой драг = MOVE (copy=false),
    /// внешний из ОС = COPY (copy=true).
    FsDropMove {
        dest: String,
        paths: Vec<String>,
        copy: bool,
    },
    /// Клипборд: абсолютный путь / путь относительно воркспейса.
    CopyToClipboard(String),
    CopyRelativePath(String),
    /// Файл-табы редактора: закрыть/выбрать/закрыть-другие/направо/все.
    CloseEditorTab(usize),
    SelectEditorTab(usize),
    CloseOtherEditorTabs(usize),
    CloseEditorTabsRight(usize),
    CloseAllEditorTabs,
    /// RMB по файл-табу: (index, path, x, y).
    OpenEditorTabMenu(usize, String, f32, f32),
    CloseEditorTabMenu,
    /// Нажатие на файл-таб (кандидат drag-reorder; клик = select на mouse-up)
    TabPress(usize, f32, f32),
    /// Курсор над табом i при зажатой ЛКМ (цель вставки reorder)
    TabDragOver(usize),
    /// «N ▾» скрытых файл-табов.
    ToggleFileTabsOverflow,
    /// Успешная файл-операция → запись в undo-стек (пуш только по факту).
    PushFsUndo(FsUndo),
    /// Ctrl+Z — откат последней файл-операции.
    UndoFsOp,
    /// Удаление в корзину (подтверждённое модалкой или probe-эмитом).
    FsDelete(String),
    /// Pin/Unpin файл-таба (pinned всегда слева, LRU не выселяет).
    TogglePinEditorTab(usize),
    /// Изменившиеся на диске пути (watcher) — reload чистых открытых табов.
    FilesChanged(Vec<String>),
    /// «Open In ▸» в файл-меню: развернуть/свернуть подпункты.
    /// Открыть терминал-тул с cwd = каталог узла дерева.
    OpenInTerminal(String),
    /// Переход к строке открытого файла (клик по minimap), 1-based.
    GotoLine(String, u32),
    /// Удалить НАВСЕГДА (зарезервированное имя — в корзину не уходит).
    FsDeletePermanent(String),
    /// Переключить режим файловой панели (files|web), persist.
    SetFileMode(&'static str),
    /// Открыть файл (клик в дереве) → запрос readText.
    OpenFile(String),
    /// Открыть файл на строке (1-based) — из поиска.
    OpenFileAt(String, u32),
    /// Содержимое файла прочитано: (path, text, целевая строка 1-based).
    FileOpened(String, String, Option<u32>),
    /// Чтение файла не удалось — вкладка рисует карточку `.error`
    /// вместо редактора (`MonacoEditor.tsx:345`).
    FileOpenFailed(String, String),
    /// Открыть панель поиска активного таба редактора (Ctrl+F). Нужно
    /// парити-гейту: клавиши probe в gpui не доставляет, а панель рисует
    /// вендорный `Input`, и снять её кадром иначе нечем.
    EditorFind,
}
