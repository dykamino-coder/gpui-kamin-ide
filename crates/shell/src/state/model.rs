//! Модель корневого вида: поля RootView и константы метрик.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::state::drag::{ChipDrag, DragState, ToolDrag};
use crate::ui::file_list::TreeState;
use crate::ui::modal::Modal;
use crate::ui::radial_bg::RadialBg;
use gpui::{Entity, FocusHandle};
use gpui_component::input::InputState;
use kamin_model::{LayoutSnapshot, SessionsSnapshot};
use kamin_theme::ThemeKind;

pub struct RootView {
    pub theme: ThemeKind,
    /// Выбор темы: "dark" | "light" | "system" (follow OS). Persist themeChoice.
    pub theme_choice: &'static str,
    /// Contributed iconThemes из registry: (id, label, path).
    pub icon_themes: Vec<(String, String, String)>,
    /// Активная contributed icon-тема (None = Catppuccin). Persist iconThemeId.
    pub icon_theme_id: Option<String>,
    pub sessions: Option<SessionsSnapshot>,
    pub ws_connected: bool,
    pub host_port: Option<u16>,
    pub workspace: Option<String>,
    /// Стор дерева файлов: отдельная сущность, чтобы панель дерева
    /// перерисовывалась ТОЛЬКО на его `notify`, а не с каждым кадром окна
    /// (`plan/102-components.md`, шаг 1).
    pub tree_store: gpui::Entity<TreeState>,
    /// Компонент панели дерева на каждый слот, где стоит тул: у слотов свои
    /// probe-регионы и размеры, а сущность обязана быть стабильной — иначе
    /// gpui не сможет переиграть её прошлый кадр.
    pub file_tree_panels: std::collections::HashMap<
        &'static str,
        gpui::Entity<crate::ui::panels::file_tree_panel::FileTreePanel>,
    >,
    /// Активная активность слота sidebar (DEFAULT: "projects")
    pub sidebar_activity: &'static str,
    pub sidebar_visible: bool,
    /// Оверфлоу-дропдаун табов сессий: Some((x,y)) = открыт (рисуется в
    /// OVERLAY-окне — поверх вебвью; в main уходил под wv2-чайлд чата).
    pub tabs_overflow_open: Option<(f32, f32)>,
    /// Загруженный layout.json (persist — layout_store)
    pub layout: LayoutSnapshot,
    /// Сплит правой колонки (0.55 default; НЕ персистится — как в оригинале)
    pub right_split: f32,
    pub drag: Option<DragState>,
    pub hovered_handle: Option<&'static str>,
    /// Свёрнутые группы проектов (projectId).
    pub collapsed_projects: std::collections::HashSet<String>,
    /// Группы с раскрытым «N inactive» (projectId).
    pub inactive_open: std::collections::HashSet<String>,
    /// Contributed-вебвью по viewId (main/plan/console).
    /// Скрим-затемнение продублировано ВНУТРЬ вебвью (WebView2 поверх
    /// main-окна — нативный скрим их не накрывает): текущее состояние.
    pub webview_scrim: bool,
    /// Live-тултип main-окна (текст, позиция снапшотом) — рисует overlay.
    pub tooltip_live: Option<(gpui::SharedString, (f32, f32))>,
    pub cz: crate::state::model_customize::CustomizeState,
    /// Одноразовая авто-диагностика composition-чата отправлена.
    /// Bounds видимых вебвью (логические px main) — угловые маски в overlay.
    /// Последняя позиция мыши в main (для позиционирования тултипа).
    pub last_mouse: (f32, f32),
    /// Вьюпорт прошлого кадра: смена размера заказывает ЕЩЁ кадр, чтобы
    /// сошлись каскады по probe-замерам прошлого кадра (ширины колонок).
    /// Без этого после разворота окна интерфейс замирал полусобранным до
    /// первого события (точные заказы убрали «случайные» кадры).
    pub(crate) last_frame_viewport: (f32, f32),
    /// HTML, пришедший до создания вебвью (гонка): viewId → html.
    pub pending_html: std::collections::HashMap<String, String>,
    /// Активная модалка-оверлей (confirm/prompt), None = закрыта.
    pub modal: Option<Modal>,
    /// Инпут prompt-модалки. Раньше жил в сущности overlay-окна; с переносом
    /// оверлеев в главное окно (Ф6) живёт здесь.
    pub modal_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) modal_input_sub: Option<gpui::Subscription>,
    /// Момент открытия модалки — `fadeIn 0.12s ease-out`
    /// (`ConfirmModal.module.css:9,12-15`). Ревью ц.26 сняло отговорку «упор
    /// движка»: прозрачность мы анимируем в этом же коде.
    pub modal_at: Option<std::time::Instant>,
    /// Кому вернуть фокус после закрытия модалки (`ConfirmModal.tsx:46-56`
    /// запоминает `document.activeElement` и восстанавливает его на unmount).
    pub(crate) modal_focus_return: Option<String>,
    /// Автофокус кнопки Confirm ещё не отработал для ТЕКУЩЕЙ модалки.
    pub(crate) modal_autofocus_pending: bool,
    /// Открытое контекст-меню сессии (позиция + данные).
    pub session_menu: Option<crate::ui::context_menu::SessionMenu>,
    /// Каскад «Move to ▸» меню таба тула открыт.
    pub tool_menu_sub: bool,
    /// Открытое контекст-меню узла дерева.
    pub file_menu: Option<crate::ui::file_menu::FileMenu>,
    /// Файловый буфер Cut/Copy: (paths, is_cut) — мультиселект поддержан.
    pub fs_clipboard: Option<(Vec<String>, bool)>,
    /// Панельная модель: pinned/active per слот.
    pub activity: crate::activity::ActivityModel,
    /// Открытый пикер тулзов: (слот, x, y, вверх).
    pub tool_picker: Option<(crate::activity::PanelSlot, f32, f32, bool)>,
    pub ed: crate::state::model_editor::EditorState,
    /// Титлбар-поповеры: Layout panels / Appearance.
    pub layout_popover: bool,
    pub appearance_popover: bool,
    pub term: crate::state::model_term::TerminalState,
    /// Фокус ввода терминала (FocusHandle не имеет Default — живёт в корне).
    pub terminal_focus: FocusHandle,
    /// Активный drag плитки тула (activity-dnd 1:1: порог 4px, hit-test по
    /// probe-bounds слотов, отпускание = move_activity / клик = активация).
    pub tool_drag: Option<ToolDrag>,
    /// Drag-reorder чипа сессии + пользовательский порядок чипов
    /// (persist в layout.json ключ sessionOrder).
    pub chip_drag: Option<ChipDrag>,
    pub chip_order: Vec<String>,
    /// Дропдаун «+» титлбара (folder/no-folder).
    /// «+»-меню (folder/empty): Some((x,y)) = открыт (рисуется в overlay).
    pub new_session_menu: Option<(f32, f32)>,
    /// RMB-меню таба тула: (слот, id, x, y).
    pub tool_tab_menu: Option<(crate::activity::PanelSlot, String, f32, f32)>,
    /// Undo-стек файл-операций (Ctrl+Z), кап 50.
    pub fs_undo: Vec<crate::host_link::FsUndo>,
    /// Сессия в процессе открытия (спиннер на чипе до open в снапшоте).
    pub switching_to: Option<String>,
    /// Шторка переключения чата: (когда поднята, когда начала гаснуть).
    /// Ставится на ЛЮБОЙ переход session→session, снимается по первому
    /// сообщению чат-вебвью или по таймауту 2500 мс, уходит фейдом 140 мс.
    pub chat_cover: Option<(std::time::Instant, Option<std::time::Instant>)>,
    /// Вью, чей скрипт уже слал ipc (жив) — до этого wv2 скрыт (chat-cover:
    /// вместо белой вспышки виден gpui-плейсхолдер «Loading…»).
    pub webviews_alive: std::collections::HashSet<String>,
    /// Крышка загрузки вебвью: (когда появился html, когда вью «отрисовалось»).
    /// Второе — `__kaminReady` оригинала: пинг страницы либо фолбэк 1200 мс.
    pub webview_cover:
        std::collections::HashMap<String, (std::time::Instant, Option<std::time::Instant>)>,
    /// Contributed statusbar items от exthost (id → item), сорт при рендере.
    pub status_items: std::collections::HashMap<String, crate::ui::status_bar::ContribItem>,
    /// Problems: (owner, uri) → диагностики; пусто = ключ удалён.
    pub diags: std::collections::HashMap<(String, String), Vec<crate::ui::problems::Diag>>,
    /// Contributed explorer/context пункты меню дерева (registry:snapshot).
    pub explorer_menu: Vec<crate::ui::file_menu::ContribMenuItem>,
    /// Contributed темы (id, label, path, dark_ui) + активная.
    pub contrib_themes: Vec<(String, String, String, bool)>,
    pub contrib_theme_id: Option<String>,
    /// Contributed keybindings: нормализованный key → command.
    pub contrib_keys: std::collections::HashMap<String, String>,
    /// Доступное обновление (version, url) — пилюля в статус-баре.
    pub update_available: Option<(String, String)>,
    /// showQuickPick от exthost + фильтр-инпут.
    pub quick_pick: Option<crate::ui::quick_pick::QuickPickState>,
    /// Отложенный Tab-переход из probe (`apply` не видит `Window`).
    pub pending_focus_step: Option<bool>,
    /// Апдейт скачан и инсталлер ждёт: закрыть окно штатно на ближайшем кадре
    /// (см. CzEvent::QuitForUpdate — exit(0) ронял CEF-кэш).
    pub pending_quit: bool,
    /// Лейаут активной сессии уже применён на буте (один раз за запуск).
    pub layout_booted: bool,
    /// Идёт установка апдейта: (скачано, всего) — рисуется заливкой пилюли.
    pub update_progress: Option<(u64, Option<u64>)>,
    pub qp_input: Option<Entity<InputState>>,
    pub qp_sub: Option<gpui::Subscription>,
    /// Вьюпорт main-окна (лог. px) — система координат overlay-оверлеев
    /// (overlay-окно накрывает main 1:1, но свой viewport у gpui может
    /// отставать после win32-ресайза).
    pub main_viewport: (f32, f32),
    /// После активации сессии ждём снапшот хоста и один раз перепросим
    /// resolve вебвью (Bridge мог зарегистрировать провайдеры позже).
    pub pending_view_resolve: bool,
    /// Шторка ждёт СВЕЖИЙ bridgeShowing:true (протокол false→true):
    /// залежавшийся true с прошлого показа сессии снимать шторку не должен.
    pub cover_expect_fresh: bool,
    /// Output-каналы (VS Code Output) + system-лог.
    pub output: crate::output_log::OutputChannels,
    /// Id сессии в inline-переименовании (None = никто).
    pub renaming_session: Option<String>,
    /// Инпут inline-переименования (лениво в render).
    pub rename_input: Option<Entity<InputState>>,
    /// `contributes.menus.commandPalette`: (команда, when).
    pub(crate) palette_menu: Vec<(String, String)>,
    /// Ключи контекста реестра для `when`-клауз палитры.
    pub(crate) context_keys: crate::when::ContextValues,
    /// Палитра команд открыта.
    pub palette_open: bool,
    /// Инпут-состояние палитры (лениво в render) + подписка на Change.
    pub palette_input: Option<Entity<InputState>>,
    pub palette_sub: Option<gpui::Subscription>,
    /// Команды реестра (тянутся при открытии палитры).
    pub commands: Vec<crate::ui::command_palette::CommandItem>,
    /// Фокус корня — точка входа для глобальных action-клавиш.
    pub focus_handle: FocusHandle,
    /// Счётчики статус-бара (расширения/команды).
    pub status_counts: crate::ui::status_bar::StatusCounts,
    /// Стек тостов (снизу-справа).
    pub toasts: Vec<crate::ui::toasts::Toast>,
    /// Живые таймеры тостов (каунтдаун/ховер-пауза/closing) по id.
    pub toast_timers:
        std::collections::HashMap<String, std::sync::Arc<crate::ui::toasts::ToastTimer>>,
    /// Id строки/группы, над которой открыт hover-поповер действий (+gen для
    /// анти-дребезга закрытия).
    /// Открытое контекст-меню веб-страницы (в теме, слой оверлеев).
    pub web_menu: Option<crate::ui::web_menu::WebMenuState>,
    pub hover_pill: Option<String>,
    pub hover_pill_gen: u64,
    /// Запечённый glow-спрайт под лого Welcome.
    pub(crate) welcome_glow: std::sync::Arc<gpui::Image>,
    /// Веб-страница Web-режима пер-сессии: id → последний URL.
    pub(crate) browser_url_stash: std::collections::HashMap<String, String>,
    /// ЖИВАЯ ширина file-панели в px (оригинал: `filePanelWidth`-сигнал).
    /// ratio — только для персиста/масштабирования при ресайзе окна, поэтому
    /// кламп FILE_RATIO_MAX=0.6 НЕ душит драг (юзер: «не могу сильнее
    /// уменьшить чат»). None → derive из ratio (после ресайза окна).
    pub(crate) file_w_live: Option<f32>,
    /// Ширина вьюпорта прошлого кадра — смена = пересчёт live-ширин из ratio
    /// (viewport adapter оригинала).
    pub(crate) last_viewport_w: f32,
    /// Дебаунс адаптера (`RESIZE_SETTLE_MS`): свёртывание/восстановление
    /// сыплют очередью промежуточных размеров, реагировать надо на
    /// УСТОЯВШИЙСЯ — иначе накопительный фактор уезжает.
    pub(crate) viewport_settle: Option<(f32, std::time::Instant)>,
    /// Problems: severity-фильтр (0 err / 1 warn / None все).
    pub problems_filter: Option<u8>,
    /// Problems: свёрнутые файл-группы (uri).
    pub problems_collapsed: std::collections::HashSet<String>,
    /// Problems: кап показанных файлов (100 + шаги 200).
    pub problems_file_cap: usize,
    /// Браузер Web-режима: вебвью + адресный инпут (лениво в render).
    pub browser_input: Option<Entity<InputState>>,
    /// `onFocus → select()` адресной строки уже отработал для ТЕКУЩЕГО фокуса
    /// (`BrowserPane.tsx:96`); сбрасывается, когда фокус ушёл.
    pub browser_addr_selected: bool,
    /// Оверлеи поиска: Quick Open, Find in Files, Go to Symbol.
    pub sov: crate::state::model_search::SearchOverlays,
    /// Состояние интерактивных семплов Design-панели.
    pub(crate) design: crate::ui::design_samples::DesignState,
    /// `treeAllCollapsed` — режим кнопки Collapse/Expand в хедере дерева.
    pub(crate) tree_all_collapsed: bool,
    /// Contributed tree-вью (TreeDataProvider): состояние по viewId.
    pub(crate) trees: std::collections::HashMap<String, crate::ui::contributed_tree::TreeViewState>,
    /// Отложенный запрос парити-гейта в инпут оверлея (`SetOverlayQuery`):
    /// `set_value` требует `&mut Window`, а он есть только в `render`.
    pub(crate) probe_query: Option<(&'static str, String)>,
    /// Отложенный Ctrl+F в активный таб редактора (`EditorFind`).
    pub(crate) probe_editor_find: bool,
    /// Поколение грации каскада файлового меню (см. `SUB_CLOSE_DELAY_MS`).
    pub(crate) file_sub_gen: u32,
    /// Подписка на события инпута переименования (коммит по blur).
    pub(crate) rename_sub: Option<gpui::Subscription>,
    /// Путь, под который уже синхронизировано выделение дерева: синк должен
    /// срабатывать на СМЕНУ активного файла, а не каждый кадр.
    pub(crate) tree_synced_path: Option<String>,
    /// Ретрай resolve вью-вебвью contributed-тулов: провайдер регистрируется
    /// позже коннекта, host no-op без повтора — шлём при видимой панели не
    /// чаще раза в 5с на вью, пока html не пришёл.
    pub(crate) view_resolve_at: std::collections::HashMap<String, std::time::Instant>,
    /// Сколько раз уже перезапрашивали вью (для backoff, как в оригинале).
    pub(crate) view_resolve_tries: std::collections::HashMap<String, u32>,
    /// Когда началось ожидание вью — секундомер подписи скелета
    /// (`setInterval 1000` оригинала; у нас перерисовка идёт кадрами).
    pub(crate) view_resolve_start: std::collections::HashMap<String, std::time::Instant>,
    pub(crate) radial_bg: RadialBg,
    pub(crate) tx: smol::channel::Sender<ShellEvent>,
}
