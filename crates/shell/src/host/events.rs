//! Типы, которыми хост говорит с UI: событие `ShellEvent` и его спутники
//! (отмена файловой операции, контейнеры и страницы Customize).
//!
//! Это ЧИСТЫЕ ДАННЫЕ: ни запросов, ни сокета — только описание того, что
//! может случиться. Вынесено из `host_link.rs` без изменений.

pub use crate::host::events_cz::CzEvent;
pub use crate::host::events_editor::EdEvent;
pub use crate::host::events_term::TermEvent;
pub use crate::host::events_tree::TreeEvent;
use kamin_sidecar::HostEndpoint;
use serde_json::Value;

/// Запись undo-стека файл-операций (Ctrl+Z; инверсия — в root::undo_fs).
#[derive(Clone)]
pub enum FsUndo {
    /// Создан файл/папка → undo = в корзину.
    Create(String),
    /// Переименован from→to → undo = обратно.
    Rename { from: String, to: String },
    /// Удалён (в корзину) → undo = restore из корзины.
    Delete(String),
    /// Вставка: dst создан; cut_src=Some(исходник) для cut → undo = вернуть.
    Paste {
        dst: String,
        cut_src: Option<String>,
    },
}

/// Contributed-контейнер `location = "customize"` и его страницы —
/// то, что CustomizeMode рисует как раскрываемый узел с детьми.
#[derive(Clone)]
pub struct CzContainer {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub views: Vec<CzPage>,
}

/// Одна contributed-страница Customize (`contributes.views[<container>]`).
#[derive(Clone)]
pub struct CzPage {
    pub id: String,
    pub name: String,
    pub icon: String,
}

/// События к UI (перекачиваются в RootView foreground-циклом).
#[derive(Clone)]
pub enum ShellEvent {
    HostReady(HostEndpoint),
    /// Контекст-меню веб-страницы: показать (Some) или закрыть (None).
    WebMenu(Option<crate::ui::web_menu::WebMenuState>),
    /// Клик по пункту меню страницы: (вью, команда CEF).
    WebMenuCmd(String, i32),
    WsConnected,
    WsDisconnected,
    /// Любое host-событие: (channel, payload) — маршрутизация в сторы на UI-стороне
    /// (поля читаются со следующей фазой сторов).
    #[allow(dead_code)]
    HostEvent(String, Value),
    /// Ответ kamin:sessions:list / kamin:sessions:changed.
    Sessions(Value),
    /// Ответ kamin:workspace:get / событие workspace:changed: {path|null}.
    Workspace(Option<String>),
    /// События домена `events_tree` — вложенный enum, чтобы корневой
    /// список оставался читаемым.
    Tree(TreeEvent),
    /// Клик по тайлу activity-бара (id активности).
    ActivityClicked(&'static str),
    /// Тоггл видимости сайдбара (кнопка титлбара).
    ToggleSidebar,
    /// Оптимистичное локальное закрытие сессии (disconnect-клик): чип/строка
    /// гаснут мгновенно, снапшот хоста подтвердит.
    LocalSessionClosed(String),
    /// Клик по main-скриму: закрыть палитру/QuickOpen/FiF/symbols.
    CloseInputOverlays,
    /// События домена `events_editor` — вложенный enum, чтобы корневой
    /// список оставался читаемым.
    Ed(EdEvent),
    /// Тултип main-окна → рисуется в overlay-слое (поверх вебвью).
    /// (text, x, y) — позиция мыши В МОМЕНТ показа (window.mouse_position
    /// рендера тултипа; last_mouse запаздывал при SendInput-телепорте).
    TooltipShow(String, f32, f32),
    /// События домена `events_cz` — вложенный enum, чтобы корневой
    /// список оставался читаемым.
    Cz(CzEvent),
    TooltipHide,
    /// Оптимистичный локальный pin/unpin (чип титлбара).
    LocalSessionPinned(String, bool),
    /// Перезаписать сохранённый пресет ТЕКУЩИМ layout-ом.
    OverwriteLayoutPreset(String),
    /// Экспорт ОДНОГО пресета в JSON-файл (формат оригинала).
    ExportPreset(String),
    /// Кнопка «N ⌄» переполнения табов.
    ToggleTabsOverflow(f32, f32),
    /// События домена `events_term` — вложенный enum, чтобы корневой
    /// список оставался читаемым.
    Term(TermEvent),
    /// DevTools-кнопка титлбара: девтулзы активного вебвью, иначе System-лог.
    TitlebarDevtools,
    /// Открыть DevTools вебвью (диагностика).
    OpenDevtools(String),
    /// Подтверждение prompt-модалки со значением инпута.
    ConfirmModalInput(String),
    /// Панельная система: pin/unpin/активация тула в слоте.
    PinTool(crate::activity::PanelSlot, String),
    UnpinTool(crate::activity::PanelSlot, String),
    /// Открыть пикер тулзов слота в точке (x, y); up — раскрытие вверх.
    OpenToolPicker(crate::activity::PanelSlot, f32, f32, bool),
    CloseToolPicker,
    /// Титлбар: поповер «Layout panels» / «Appearance».
    ToggleLayoutPopover,
    ToggleAppearancePopover,
    /// Тумблер видимости панели: "main"|"mainBottom"|"file"|"fileBottom"|
    /// "right"|"rightBottom".
    ToggleLayoutFlag(&'static str),
    /// Выбор темы: "dark"|"light".
    SetThemeChoice(&'static str),
    /// Нажатие на плитку тула (начало возможного dnd): (слот, id, x, y).
    ToolPress(crate::activity::PanelSlot, String, f32, f32),
    /// Зажатая ЛКМ над табом тула (слот, индекс) — цель вставки reorder.
    ToolDragOverTab(crate::activity::PanelSlot, usize),
    /// Layout-пресеты: сохранить текущий (имя из prompt-модалки идёт через
    /// ConfirmModalInput+ModalAction), применить/удалить/сделать дефолтным.
    ApplyLayoutPreset(String),
    DeleteLayoutPreset(String),
    SetDefaultLayoutPreset(String),
    OpenSaveLayoutPrompt,
    /// Пресеты: prompt переименования / экспорт в файл / импорт из файла.
    OpenRenamePresetPrompt(String),
    ExportPresets,
    ImportPresets,
    /// То же для чипов сессий (id вместо индекса)
    ChipPress(String, f32, f32),
    ChipDragOver(String),
    /// mouse-up на чипе: .occlude() чипа обрезает bubble до root —
    /// коммит drag/click шлём событием с самого чипа
    ChipRelease,
    /// showQuickPick от exthost: (req_id, items, options) — модалка выбора.
    QuickPickShow(u64, Value, Value),
    /// Выбор сделан: (req_id, indices|None=cancel) → respond хосту.
    QuickPickResolve(u64, Option<Vec<usize>>),
    /// Мульти-пик: тоггл чекбокса.
    QuickPickToggle(usize),
    /// RMB по табу тула: (слот, id, x, y) → меню Hide / Move to.
    OpenToolTabMenu(crate::activity::PanelSlot, String, f32, f32),
    CloseToolTabMenu,
    /// Каскад «Move to ▸» (true=открыт).
    ToolMenuSub(bool),
    /// Move to: (src, id, dst).
    MoveToolTo(
        crate::activity::PanelSlot,
        String,
        crate::activity::PanelSlot,
    ),
    /// «+» титлбара: дропдаун New session in folder… / Empty session.
    ToggleNewSessionMenu(f32, f32),
    NewSessionInFolderPrompt,
    NewEmptySession,
    /// Коллапс/раскрытие группы проекта (projectId).
    ToggleProjectCollapse(String),
    /// Показ «N inactive sessions» группы (projectId).
    ToggleInactive(String),
    /// Активировать сессию (оптимистично: подсветка сразу, RPC в фоне).
    ActivateSession(String),
    /// Пульс перерисовки, пока живёт шторка переключения чата: её грейс,
    /// фейд и форс-таймаут считаются В РЕНДЕРЕ, а точные заказы кадров без
    /// событий не рисуют — без пульса шторка зависала навсегда.
    CoverTick,
    /// Чат отчитался `chat:bound` — реально показывает свой активный таб.
    /// Быстрый локальный сигнал снятия шторки (см. chat_webview::handle_inbound).
    ChatBound,
    /// Открыть контекст-меню сессии в точке (x, y).
    OpenSessionMenu(crate::ui::context_menu::SessionMenuData, f32, f32),
    /// Закрыть контекст-меню сессии.
    CloseSessionMenu,
    /// Начать inline-переименование сессии (id).
    BeginRename(String),
    /// Подтвердить inline-переименование (читает inline-инпут).
    CommitRename,
    /// Отменить inline-переименование.
    CancelRename,
    /// Открыть модалку (confirm/prompt).
    OpenModal(crate::ui::modal::Modal),
    /// Подтверждение активной модалки (выполнить действие).
    ConfirmModal,
    /// Закрыть модалку без действия.
    CloseModal,
    /// Тоггл палитры команд (Ctrl+Shift+P / клик по пилюле).
    TogglePalette,
    /// Закрыть палитру команд.
    ClosePalette,
    /// Список команд реестра (ответ на registry:snapshot).
    Commands(Vec<crate::ui::command_palette::CommandItem>),
    /// `contributes.menus.commandPalette` (команда, when) + ключи контекста
    /// реестра — гейт видимости палитры (`state.ts:68-76`).
    PaletteGate(Vec<(String, String)>, Vec<(String, serde_json::Value)>),
    /// Выполнить команду по id (из палитры).
    RunCommand(String),
    Toast(crate::ui::toasts::Toast),
    /// Убрать тост по id (auto/manual) — фаза 1: closing + slide-out.
    DismissToast(String),
    /// Фаза 2 Dismiss: удалить карту после slide-out.
    ToastGone(String),
    /// ↑/↓ в инпут-оверлее (probe-верификация: SendInput до gpui не доходит).
    OverlayMove(i32),
    /// Переход фокуса Tab (true) / Shift+Tab (false) — дев-мост probe.
    FocusStep(bool),
    /// Закрыть просмотрщик файла.
    /// Тоггл Quick Open (Ctrl+P).
    /// Наведение на строку оверлея переносит активный индекс
    /// (`onMouseEnter` оригинала): «qo» | «fif» | «ws».
    OverlayRowHover(&'static str, usize),
    ToggleQuickOpen,
    /// Закрыть Quick Open.
    CloseQuickOpen,
    /// Результаты нечёткого поиска файла.
    QuickOpenResults(Vec<crate::ui::quick_open::FileHit>),
    /// Тоггл Find in Files (Ctrl+Shift+F).
    ToggleFindInFiles,
    /// Закрыть Find in Files.
    CloseFindInFiles,
    /// Результаты текстового поиска.
    FindInFilesResults(Vec<crate::ui::find_in_files::TextHit>),
    /// Тоггл Go to Symbol in Workspace (Ctrl+T).
    ToggleWorkspaceSymbols,
    /// Закрыть Go to Symbol.
    CloseWorkspaceSymbols,
    /// Результаты поиска символов.
    WorkspaceSymbolsResults(Vec<crate::ui::workspace_symbols::SymbolHit>),
    /// Записать текст в инпут ОТКРЫТОГО оверлея (`fif`/`qo`/`ws`/`palette`/`qp`).
    /// Нужно парити-гейту: probe физически не доставляет клавиши в gpui
    /// (WM_CHAR окном не разбирается), а без запроса списки пусты и строки
    /// нечем сверять. Идёт через `InputState::set_value` → `InputEvent::Change`
    /// → тот же дебаунс и тот же запрос к хосту, что и при ручном вводе.
    SetOverlayQuery(&'static str, String),
    /// Показать ВНЕШНИЙ тост — отдельное окно поверх всех
    /// (`externalToast.show` оригинала), не строка внутреннего стека.
    ExternalToast(crate::ui::toasts::Toast),
    /// Наведение на строку/группу сессий (id) → показать hover-поповер; None =
    /// увести (закрытие с задержкой).
    HoverPill(Option<String>),
}
