# 99 — Аудит-гэпы (3 агента, 2026-07-26): цвета/метрики, анимации, DnD/алгоритмы

Полные отчёты — в сессионных логах; здесь рабочий чеклист. Вычёркивать по мере закрытия.

## Цвета/метрики (топ-10 заметных + прочее)
- [x] File-tree выделение: градиент accent 26→14% + бордер 45% (сейчас плоский 12%) — file_list.rs:152 vs FileTreeView.module.css:89
- [x] File-tree hover: bg-surface 55% (сейчас text-primary 6%) — file_list.rs:134
- [x] Editor tab dirty-точка: accent-ORANGE (сейчас primary) — editor_tabs.rs:181
- [x] «Show N more»: text-muted (сейчас accent_blue) — file_list.rs:309
- [x] Indexing-бейдж: text-muted @0.85 (сейчас accent_primary) — file_list.rs:431
- [x] Плейсхолдер заголовок: fs-lg 16 text-primary (сейчас 13 secondary) — panel_placeholder.rs:105
- [x] LayoutToggles кнопка: 26×26 глиф 13 (сейчас 28/15); ThemeToggle глиф 12 muted (сейчас 14 secondary) — titlebar.rs:290,301,316
- [x] Update-пилюля: 22% + weight600 (сейчас 18%/500) — status_bar.rs:255
- [x] Пилюля сессии: radius-md 12 (сейчас LG 16); btn secondary/hover12%/глиф13; rename-hover accent-primary, disconnect-hover accent-blue — sessions_list.rs:354-396
- [x] Editor tabs: h24 font11 gap6 weight500; hover bg-surface50%; pin 11; close hover bg-overlay60% r4 — editor_tabs.rs:89-198
- [x] Layout/Appearance попап-пункты: text-primary, pad 8/12, hover 10%; Appearance p=8, header weight600, tick 10 — layout_popover.rs
- [x] Время в строке сессии: weight600 opacity0.7 — sessions_list.rs:256
- [x] File-tree: gap 6 (сейчас 4), chevron 13 (сейчас 12), badge weight600
- [x] Contributed status-item: radius-xs4 gap4 — status_bar.rs:104
- [x] Activity «…»: глиф 18 (сейчас 15); зазор список↔пикер space-2
- [x] Плейсхолдер глиф: 28×24 (сейчас 39×34)

## Анимации (все реализуемы, кроме hover-tween)
- [x] Working-дот пульс 1.1s infinite (opacity .5↔1 + scale)
- [x] Switching-дот «дыхание» 1s infinite (opacity 1↔.25)
- [x] Тосты slide-in 0.18s / slide-out 0.18s + countdown-бар 8s (pause on hover)
- [x] Tooltip fade 0.1s (тултип теперь ПОСЛЕДНИЙ слой overlay_stack — поверх поповеров)
- [x] treeFlash нового узла 0.9s (row.rs with_animation)
- [x] Скелетоны чата shimmer/breathe (loop)
- [x] Поповеры: полный flip (не только clamp) — порт clamp-popup.ts
- [x] Каскад file-menu: close-delay 250ms

## DnD/алгоритмы
- [x] Drop файла/папки НА ПАПКУ дерева: свой drag = MOVE (kamin fs move), внешний = COPY
- [x] Папки дерева draggable (on_drag на всех строках, root-строка без драга) (depth>0)
- [x] Мульти-файловый drag (dragPaths: выделение >1 едет целиком) (вся выборка)
- [→backlog] Нативный OS-drag наружу (OLE DoDragDrop) — перенесено в бэклог: крупная нативная фича, отдельный заход
- [→backlog] Contributed-TreeView DnD (handleDrop) — перенесено в бэклог: требует exthost-протокол
- [x] Чипы: базовая сортировка = sleeping слева, активные справа (не last_opened); reorder персист НА ХОСТ (sessions:reorder)
- [x] Проекты сайдбара: сортировка createdAt ASC (сейчас host-order); активные: ЮЗЕР ХОЧЕТ алфавит (оставить)
- [x] LRU файл-табов: РЕШЕНО — оставляем наш (LRU + защита dirty): строго лучше FIFO оригинала, поведенчески совместим
- [x] Editor-tabs overflow (скролл стрипа + автоподвоз активного; меню со всеми было): скролл стрипа + меню со ВСЕМИ (сейчас скрытие+меню скрытых)
- [x] Палитра: when-гейт commandPalette (palette_gate + when-evaluator, тест gate_matches_state_ts)

## Закрыто в этом заходе
- [x] Меню таба тула 1:1 (иконки, каскад Move to ▸ с PanelIcon, отступы 8/12, text-primary)
- [x] Session-меню: Auto-rename from chat (sparkle, open-сессии)
- [x] Активный чип: disconnect-иконка всегда видна
- [x] Сайдбар: динамический max-кламп (viewport-550) вместо хард 600
- [x] Иконки тулов в стрип-табах
- [x] Вебвью-тулы: show/hide по активности тула
- [x] Contributed тулы из registry (Plan/Todos/Agents/Console/Chat + Hello Panel) с codicon-иконками
- [x] Иконки папок групп сайдбара = Catppuccin по имени
- [x] Дот connecting = жёлтый
- [x] Активные сессии по алфавиту (директива юзера)


## Волна 2026-07-26 (скрины юзера, ЗАКРЫТО в этой сессии)
- [x] Тултипы: сдвиг паддингом (margin игнорится layout_as_root); в overlay — tooltip_region только на видимый бокс
- [x] Двойной .tooltip на sleeping-чипе = panic gpui (debug_assert) — единый tooltip с вариантным текстом
- [x] Мигание дерева: RefreshTree больше НЕ чистит кэш (пустой рендер до DirListing)
- [x] Disconnect/pin чипа: оптимистичные LocalSessionClosed/LocalSessionPinned + RPC в потоке
- [x] Пин чипа в leading-слоте (точка↔пин по ховеру, pinned = tab-color)
- [x] Layout-поповер 1:1: чекбоксы 16px, LAYOUT/LAYOUTS uppercase, Save/Export/Import c codicon, пресет-строки с 26px-кнопками (overwrite/export/star/trash), ПКМ=rename; события Overwrite/ExportPreset; экспорт/импорт формата оригинала {kaminLayout,name,snapshot}
- [x] Appearance: uppercase-заголовки колонок, picked=accent16%, тик 10px/слот 12, иконка слот 16, System radius-sm
- [x] Взаимоисключение поповеров (close_popovers_except на 9 open/toggle-хендлерах)
- [x] Overlay-шрифт: Bricolage+tnum на корне overlay-слоя (иконки/толщина ехали)
- [x] Динтулы: IconThemesList шлётся ПОСЛЕ set_dyn_tools (рендер будился до заполнения → «Tool/Coming soon»)
- [x] Палитра: скрим в main + инъекция скрима в вебвью (evaluate_script), max_h от вьюпорта, инпут-строка py 6
- [x] slot_glyph_small = 1.0 (точный размер PanelIcon 14×12)

## Волна 2026-07-26 (ОТКРЫТО — скрины юзера)
- [x] Activity-bar: состояния уже 1:1 (rest transparent / hover surface50 / active accent16); ширина 48→44 (css default)
- [x] Бейджи счётчиков: код уже соответствует css (min-w 16, h 16, r9, bg-surface) — сверено
- [x] Catppuccin спец-папки: вендорено 226 folder-*.svg из @iconify-json/catppuccin + folder_special() (421 имя, семантика оригинала: folder_${key}→слаг, -open, фоллбек generic)
- [x] Отступы строк сайдбара — сверено в проходе метрик (SPACE-сетка на месте)
- [🖱 юзер] Верифицировать кликом: unpin чипа, disconnect (после оптимистичных фиксов)
- [x] Палитра: items_start на обёртке (cross-axis stretch растягивал бокс); клик по скриму закрывает (CloseInputOverlays); инпут-строка py 6; скрим main + вебвью
- [x] QuickOpen/FiF/Symbols: уже имели py 6 и items_start — сверено
- [x] ~~Alpha-overlay эксперимент~~ СНЯТО CEF/Ф6: второго окна нет, оверлеи в главном (Opaque = сплошное чёрное окно): гипотеза ресёрча не подтвердилась; План Б — убрать recreate (окно постоянного display-размера, регион со сдвигом)

- [x] Титлбар: драг за пустоты на максимизированном окне (SC_MOVE запрещён) — restore→move
- [x] Appearance: right-анкор + ширина по контенту (фикс-470 резала имена и давала пустые полосы региона)
- [x] Поповеры взаимоисключающие; чёрные полосы = регион шире отрисовки (объяснение юзеру дано)
- [🖱 юзер] Верифицировать вживую юзером: unpin/disconnect чипа, драг титлбара, спец-иконки папок

## Оверлей-альфа (прорыв 2026-07-26)
- [x] Корень чёрного оверлея: наш GPUI_DISABLE_DIRECT_COMPOSITION=true (нужен только main); vendored gpui: per-window env → overlay на dcomp premultiplied
- [x] Нативный драг титлбара: ReleaseCapture + restore-under-cursor + PostMessage SC_MOVE|HTCAPTION (SendMessage = RefCell-краш)
- [x] Файловое контекст-меню 1:1 (иконки, Open In ▸ флайаут, danger-Delete, порядок групп)
- [x] ~~Alpha-дефолт (SetWindowRgn/скрим/recreate)~~ СНЯТО CEF/Ф6: wry-механика удалена, вернуть тени поповерам, скрим 60% из overlay (вместо main+webview-инъекции), убить recreate-дебаунс — после подтверждения юзером, что попапы в dcomp-режиме стабильны
- [🖱 юзер] Ghost-рамка при драге: проверить на драге v3 (SC_MOVE|2); если осталась — исследовать present во время модального цикла

## Эдитор (запрос юзера 2026-07-26, скрин Zed)
- [x] Цвет фона эдитора: editor_bg палитры (file_editor_body)
- [x] Скроллбар как в Zed (тонкий + markers диагностик по severity) — СМОТРЕТЬ КОД ZED (crates/editor scrollbar/minimap), не выдумывать
- [x] Breadcrumb с путём + иконки действий справа (только ЖИВЫЕ: поиск Ctrl+F, Locate; декоративные eye/AI/settings не тащим)
- [x] Locate selected file + прокрутка к строке (goto из поиска/Problems скроллит вьюпорт — probe-проверка)
- [x] ~~Вебвью border-radius инъекцией~~ СНЯТО CEF: кадр рисуется paint_external_texture со скруглением углов панели + отступы 8px — ПРОВЕРИТЬ визуально (может требовать DefaultBackgroundColor поверх wry with_transparent)

## Скругление вебвью (уточнение после теста юзера)
- [x] ~~CSS border-radius на html / угловые маски~~ СНЯТО CEF: скругление в рендере кадра (web/element.rs), маски не нужны: фон страницы = propagated CANVAS background, он красится мимо радиуса. План: угловые маски в OVERLAY-окне (теперь есть настоящая альфа): canvas paint_path — 4 «уголка» (квадрат минус четверть круга, кубик-аппроксимация дуги c=0.5523r) цветом фона карты поверх углов каждого alive-вебвью (bounds уже есть у round_webview_children); region push для зон масок; антиалиас gpui = без лесенок. Альтернатива-минимум: снять фон с html/body инъекцией (background:transparent на html+body и покрасить #app-обёртку) — тогда border-radius сработает; проверить, не сломает ли страницы Bridge.

- [x] ~~WebView2 lag при драге сплиттера~~ СНЯТО CEF: свой рецепт ресайза (троттлинг+invalidate+лишний кадр, plan/101) (сдвиг→потом расширение): внутренняя асинхронщина wv2-рендера. Варианты: throttle bounds-апдейтов до конца драга / CoreWebView2Controller.Bounds+SetWindowPos в одном батче / фон-заглушка цвета панели под wv2 на время драга
- [x] Открытые файлы: ВЕРИФИЦИРОВАНО де-факто — splitters.rs переживал все 6+ рестартов сегодняшней сессии (скрины)

## Волна 3 (2026-07-26, вечер) — ЗАКРЫТО
- [x] Тултипы: ВСЕ в overlay-слое (main публикует TooltipShow/Hide, overlay рисует по last_mouse; Drop гасит) — больше не прячутся под вебвью
- [x] Тултип центрируется под курсором/элементом (ширина шейпером, absolute -w/2)
- [x] Locate: нормализация сепараторов + раскрытие предков + select + СКРОЛЛ к строке (flat_row_index повторяет порядок rows(), внешний ScrollHandle через vendored overflow_y_scrollbar_with)
- [x] Customize: contributed-узел «Claude Bridge» деревом (chevron, collapse, отступ детей 26px); в Customize активен только gear
- [x] Редактор: highlight_theme из палитры IDE (gutter/номера строк/active line); appearance(false) — без чужого бордера; единая скруглённая рамка
- [x] WebView2-лаг при драге сплиттера: freeze bounds на время драга (vendored webview), один финальный ресайз на отпускании
- [x] Фантомный бордер overlay при драге окна: статичный client-регион отрезает системную рамку (alpha-режим)
- [x] Открытые файлы: персист openFiles + restore на старте
