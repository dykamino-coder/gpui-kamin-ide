//! Глобальные действия шелла + их клавиши (keymap в main.rs). Роутятся через
//! фокус-цепочку корня (RootView.focus_handle + key_context "Root").

use gpui::actions;

actions!(
    kaminide,
    [
        /// Ctrl+Shift+P — тоггл палитры команд.
        TogglePalette,
        /// Ctrl+P — тоггл Quick Open (поиск файла).
        ToggleQuickOpen,
        /// Ctrl+Shift+F — тоггл Find in Files (текстовый поиск).
        ToggleFindInFiles,
        /// Ctrl+T — тоггл Go to Symbol in Workspace.
        ToggleWorkspaceSymbols,
        /// Escape — закрыть верхний оверлей (палитра/модалка/меню).
        CloseOverlay,
        /// Enter в инпут-оверлеях: открыть активный результат / выполнить команду.
        PressEnter,
        /// Ctrl+S — сохранить открытый в редакторе файл.
        SaveFile,
        /// Ctrl+B — тоггл сайдбара.
        ToggleSidebarAction,
        /// Ctrl+Z — откат последней файл-операции дерева (не редактора:
        /// сфокусированный InputState перехватывает свой undo раньше).
        UndoFileOp,
        /// F2 — переименовать активную сессию.
        RenameActiveSession,
    ]
);
