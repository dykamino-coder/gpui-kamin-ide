# KaminIDE — исчерпывающий инвентарь UI-элементов (159 host + webview-перечень)

Источник host-renderer: `%PROJECTS%\kamin-ide\src\renderer\components\` (все подпапки).
Формат строки: `NN. slug — файл.tsx:строки JSX + module.css — описание — ключевые css-классы`

Статусы: [ ] не начат · [O] original.md/png собраны · [U] ours.md/png собраны · [V] вердикт чист (100%)

## 1. Titlebar (19)

1. `titlebar-root` — `titlebar/Titlebar.tsx:34-86` + `Titlebar.module.css` — `<header>` высотой 42px, прозрачный (радиальный градиент `appWrapper` просвечивает), drag-region везде кроме no-drag контролов — `.titlebar`
2. `titlebar-left-cluster` — `titlebar/Titlebar.tsx:35-40` + `Titlebar.module.css` — левый кластер, ширина пиннится к `sidebarWidth` (или `auto` при скрытом сайдбаре) — `.leftCluster`
3. `titlebar-brand-logo` — `titlebar/Titlebar.tsx:36-38` + `Titlebar.module.css` — бренд-иконка `kaminoid.svg`, `draggable=false`, `aria-hidden` — `.brand`, `.brandLogo`
4. `titlebar-tabs-slot` — `titlebar/Titlebar.tsx:41` + `Titlebar.module.css` — flex:1 слот под стрип сессионных табов — `.tabsSlot`
5. `titlebar-command-search-button` — `titlebar/Titlebar.tsx:43-51` + `Titlebar.module.css` — фейк-строка поиска «Type a command…» с codicon-search, дёргает `workbench.action.showCommands` — `.searchButton`, `.searchHint`
6. `titlebar-window-controls-cluster` — `titlebar/Titlebar.tsx:54-85` + `Titlebar.module.css` — DevTools / minimize / maximize-restore / close — `.controls`
7. `titlebar-button` — `titlebar/TitlebarButton.tsx:30-35` + `TitlebarButton.module.css` — универсальная кнопка титлбара, iconSet `fas`|`codicon`, варианты default/devtools/close — `.btn`, `.close`, `.devtools`, `.devtoolsLabel`
8. `titlebar-quick-actions-row` — `titlebar/TitlebarQuickActions.tsx:27-51` + `TitlebarQuickActions.module.css` — 2-кнопочный кластер: тумблер сайдбара + шестерёнка Customize (только при скрытом сайдбаре), с разделителем — `.row`, `.divider`
9. `titlebar-quick-action-button` — `titlebar/TitlebarQuickActions.tsx:54-66` + `TitlebarQuickActions.module.css` — внутренняя `ActionBtn` с `aria-pressed`/active — `.btn`, `.active`
10. `layout-toggles-trigger` — `titlebar/LayoutToggles.tsx:165-180` + `LayoutToggles.module.css` — квадратный чип `fa-table-columns`, открывает popover раскладки — `.anchor`, `.trigger`
11. `layout-toggles-menu` — `titlebar/LayoutToggles.tsx:117-163` + `LayoutToggles.module.css` — портал-меню в `<body>` с 6 `menuitemcheckbox` (Left / Left Bottom / File / Center Bottom / Right / Right Bottom), не закрывается по клику, clampToViewport — `.menu`, `.menuLabel`, `.menuItem`, `.check`, `.checkOn`, `.itemIcon`, `.itemLabel`, `.itemHint`, `.divider`
12. `layout-presets-section` — `titlebar/LayoutPresetsSection.tsx:98-167` + `LayoutToggles.module.css` — секция «Layouts» внутри того же popover: Save/Export/Import + строки пресетов с 4 икон-кнопками (overwrite, export, star-default, trash) — `.menuLabel`, `.menuItem`, `.presetEmpty`, `.presetRow`, `.presetApply`, `.presetIconBtn`
13. `theme-quick-toggle-trigger` — `titlebar/ThemeQuickToggle.tsx:51-66` + `ThemeQuickToggle.module.css` — кнопка внешнего вида, иконка `fa-moon`/`fa-sun`/`fa-circle-half-stroke` по активной теме — `.root`, `.trigger`
14. `theme-popover` — `titlebar/ThemeQuickToggle.tsx:82-118` + `ThemeQuickToggle.module.css` — `role=dialog` «Appearance»: хедер + тумблер System + 3 колонки — `.menu`, `.header`, `.title`, `.sysToggle`, `.sysOn`, `.columns`
15. `theme-popover-column` — `titlebar/ThemeQuickToggle.tsx:121-128` + `ThemeQuickToggle.module.css` — колонка Dark / Light / Icons (`role=listbox`) — `.column`, `.colTitle`, `.colList`
16. `theme-popover-item` — `titlebar/ThemeQuickToggle.tsx:130-152` + `ThemeQuickToggle.module.css` — `role=option` со всегда отрисованной галкой (visibility-toggle, чтобы ширина не прыгала) — `.item`, `.picked`, `.itemIcon`, `.itemName`, `.itemTick`
17. `panel-icon-svg` — `titlebar/PanelIcon.tsx:46-90` (без css-модуля, inline SVG 14×12) — общая иконка-рамка с подсвеченным слотом; 9 вариантов: `left`, `right`, `right-top`, `right-bottom`, `center`, `center-bottom`, `bottom`, `main-bottom`, `main` — currentColor, `opacity 0.85`
18. `session-tabs-strip` — `session-tiles/SessionTabs.tsx:98-138` + `SessionTabs.module.css` — стрип открытых сессий в `tabsSlot`: pointer-drag reorder, drop-бар, «+» с пикером (folder/no-folder), drag-region спейсер — `.strip`, `.dropBar`, `.newTab`, `.spacer`, `.picker`, `.pickerItem`
19. `session-tab-chip` — `session-tiles/SessionTab.tsx:26-65` + `SessionTab.module.css` — чип сессии: dot→pin на hover, label, disconnect (×), тонировка `--tab-color`, состояния sleeping/switching/pinned/dragging — `.tab`, `.active`, `.tinted`, `.sleeping`, `.switching`, `.pinnedTab`, `.dndDragging`, `.leading`, `.dot`, `.pin`, `.pinned`, `.label`, `.close`

## 2. Sidebar / Sessions (18)

20. `sidebar-root` — `sidebar/Sidebar.tsx:52-63` + `Sidebar.module.css` — `<aside>` переменной ширины, режимы `sessions`/`customize`, drop-target активностей — `.sidebar`, `[data-activity-slot=sidebar]`, `[data-activity-drop]`
21. `sidebar-resize-handle` — `sidebar/Sidebar.tsx:64-73` + `Sidebar.module.css` — вертикальный сплиттер на правом крае, `role=separator`, `clampGrowth` против min-width центра — `.resizeHandle`, `.resizeHandleActive`, `.resizeHandleBar`
22. `sidebar-body-resolver` — `sidebar/Sidebar.tsx:81-85` — выбор тела по активной активности; при отсутствии — `ActivityPlaceholder("No tool selected")`
23. `sessions-mode-root` — `sidebar/SessionsMode.tsx:12-27` + `SessionsMode.module.css` — активность «Projects»: 2 экшн-кнопки (No folder session / New session), лендмарк-хедер `PROJECTS`, скроллящийся список, empty-текст — `.root`, `.actions`, `.action`, `.header`, `.list`, `.empty`
24. `project-group-header` — `sidebar/ProjectGroup.tsx:44-52` + `ProjectGroup.module.css` — сворачиваемый заголовок проекта: chevron + `TreeIcon(dir)` + имя + счётчик — `.group`, `.header`, `.headerMain`, `.chevron`, `.icon`, `.name`, `.count`
25. `project-actions-popover` — `sidebar/ProjectGroup.tsx:53-84` + `ProjectGroup.module.css` — портал-тулбар вправо по hover: «New session here» + «Delete project» — `.actionsPop`, `.popAction`, `.add`, `.delete`
26. `project-sessions-list` — `sidebar/ProjectGroup.tsx:85-103` + `ProjectGroup.module.css` — список активных сессий + empty — `.sessions`, `.empty`
27. `project-inactive-toggle` — `sidebar/ProjectGroup.tsx:91-98` + `ProjectGroup.module.css` — раскрывашка «N inactive session(s)» — `.inactiveToggle`, `.inactiveOpen`
28. `session-item-row` — `sidebar/SessionItem.tsx:88-119` + `SessionItem.module.css` — строка сессии: dot, label, relative-time, pin; click активирует, dblclick/F2 — inline rename, right-click — меню; тонировка `--tab-color` — `.row`, `.active`, `.tinted`, `.inactive`, `.label`, `.time`
29. `session-status-dot` — `sidebar/SessionItem.tsx:102-107` + `SessionItem.module.css` — цветной/пульсирующий индикатор из `session.metadata.bridgeStatus`/`bridgeWorking` — `.dot` + `[data-bridge=connected|connecting|error|disconnected|working]`
30. `session-pin-button` — `sidebar/SessionItem.tsx:111-119` + `SessionItem.module.css` — всегда-на-месте кнопка pin/unpin (`fa-thumbtack`) — `.action`, `.pin`, `.pinned`
31. `session-rename-input` — `sidebar/SessionItem.tsx:73-86` + `SessionItem.module.css` — inline-инпут переименования (Enter commit / Esc cancel / blur commit) — `.editing`, `.input`
32. `session-actions-popover` — `sidebar/SessionItem.tsx:120-162` + `SessionItem.module.css` — портал-тулбар вправо по hover: rename / disconnect / delete, hover-bridge через `::before` — `.actionsPop`, `.popAction`, `.rename`, `.disconnect`, `.delete`
33. `session-context-menu` — `sidebar/SessionContextMenu.tsx:41-66,88-92` + `SessionContextMenu.module.css` — глобальное контекст-меню сессии (одно на приложение, монтируется в `App.tsx`): Rename, Auto-rename from chat, Pin/Unpin, Deactivate, Delete — `.menu`, `.item`, `.danger`, `.divider`
34. `session-color-swatches` — `sidebar/SessionContextMenu.tsx:67-87` + `SessionContextMenu.module.css` — ряд цветных свотчей `SESSION_COLORS` + кнопка сброса цвета — `.swatches`, `.swatch`, `.swatchActive`, `.swatchClear`
35. `customize-mode-nav` — `sidebar/CustomizeMode.tsx:84-96` + `CustomizeMode.module.css` — сайдбар в режиме Customize: лендмарк `CUSTOMIZE` + список страниц (Settings/Design/Extensions/Logs/System) — `.root`, `.header`, `.title`, `.list`
36. `customize-nav-item` — `sidebar/CustomizeMode.tsx:25-37` + `CustomizeMode.module.css` — плоская строка навигации (иконка codicon или `<img>` + подпись), вариант `child` для вложенных — `.item`, `.child`, `.active`
37. `customize-contributed-tree` — `sidebar/CustomizeMode.tsx:42-72` + `CustomizeMode.module.css` — раскрывающийся узел контрибьютнутого `customize`-контейнера расширения + его view-страницы — `.item`, `.active`, `.chevron`, `.chevronOpen`

## 3. ActivityBar (14)

38. `activity-bar-nav` — `activity-bar/ActivityBar.tsx:117-128` + `ActivityBar.module.css` — вертикальная колонка иконок-активностей, один инстанс на панель, `align top|bottom` — `.bar`, `.barReverse`, `[data-activity-strip]`, `[data-activity-orientation=vertical]`
39. `activity-tile` — `activity-bar/ActivityBar.tsx:82-99` + `ActivityBar.module.css` — квадратная плитка-тумблер: pointer-drag между зонами, right-click → контекст-меню — `.list`, `.btn`, `.btnActive`, `.btnImage`, `.tileDragging`
40. `activity-customize-tile` — `activity-bar/ActivityBar.tsx:131-148` + `ActivityBar.module.css` — фиксированная системная плитка «Customize» (только в сайдбаре, не перетаскивается) — `.btn`, `.btnActive`
41. `activity-drop-placeholder` — `activity-bar/ActivityBar.tsx:150-152` + `ActivityBar.module.css` — вставочный плейсхолдер между плитками при drag — `.dropPlaceholder`
42. `activity-picker-dots-trigger` — `activity-bar/ActivityPicker.tsx:126-138` + `ActivityBar.module.css` — «…» (`codicon-more`) в конце стрипа — `.pickerAnchor`, `.pickerAnchorInline`, `.picker`
43. `activity-picker-open-tool-pill` — `activity-bar/ActivityPicker.tsx:115-125` + `panel-placeholder/PanelPlaceholder.module.css` — широкая пилюля «Open Tool ▾» для пустых панелей — `.trigger`
44. `activity-picker-menu` — `activity-bar/ActivityPicker.tsx:140-174` + `ActivityBar.module.css` — портал-listbox «Tools»: pin/unpin из глобального реестра, галка у запиненных, flip+shift позиционирование — `.menu`, `.menuPortal`, `.menuLabel`, `.menuItem`, `.menuItemImage`, `.menuLabelText`
45. `activity-context-menu` — `activity-bar/ActivityContextMenu.tsx:132-169` + `ActivityContextMenu.module.css` — портал у курсора: «Hide» + «Move to ▸» — `.menu`, `.item`, `.itemLabel`, `.itemMoveTo`, `.chevron`
46. `activity-context-submenu` — `activity-bar/ActivityContextMenu.tsx:171-204` + `ActivityContextMenu.module.css` — сабменю целевых слотов (Sidebar/Left/Left Bottom/Center Bottom/Right/Right Bottom) с `PanelIcon` — `.submenu`, `.subItem`, `.subItemIcon`, `.subItemLabel`
47. `activity-drag-ghost` — `activity-bar/ActivityDragGhost.tsx:13-21` + `ActivityDragGhost.module.css` — плавающая иконка-«призрак» под курсором при pointer-драге плитки (монтируется в `App.tsx`) — `.ghost`
48. `bottom-tab-bar-strip` — `activity-bar/BottomTabBar.tsx:70-84` + `BottomTabBar.module.css` — горизонтальный стрип табов для `main`/`mainBottom`/`centralBottom`, скрытый скроллбар — `.strip`, `.tabs`, `.pickerSlot`, `[data-activity-orientation=horizontal]`
49. `bottom-tab` — `activity-bar/BottomTabBar.tsx:48-64` + `BottomTabBar.module.css` — pill-таб «иконка + подпись», drag-start + контекст-меню, `TAB_ICON_SIZE_PX = 13` — `.tab`, `.tabActive`, `.tabDragging`, `.tabImage`, `.tabLabel`
50. `bottom-tab-drop-placeholder` — `activity-bar/BottomTabBar.tsx:87-89` + `BottomTabBar.module.css` — вставочная метка при drag по стрипу — `.dropPlaceholder`
51. `tool-icon` — `tool-icon/ToolIcon.tsx:26-48` (+ `tool-icon-paths.ts`, без css-модуля) — единый рендер иконки активности: URL→`<img>`, встроенный токен→vendored Phosphor SVG (`currentColor`), иначе codicon-шрифт; `size=18` по умолчанию

## 4. Panels / Slots (40)

52. `app-shell` — `layout/AppLayout.tsx:55-79` + `AppLayout.module.css` — 3-рядный каркас titlebar/body/statusbar; брендовый радиально-градиентный фон, flex-row body с `gap:--space-2` и симметричным гуттером, `mainColumn` — `.appWrapper`, `.body`, `.bodyNoSidebar`, `.mainColumn`
53. `main-content` — `main/MainContent.tsx:35-58` + `MainContent.module.css` — центральная колонка («Left»): BottomTabBar + тело активности, либо `CustomizePanel`, либо `WelcomePlaceholder`; высота в % от `mainSplit` — `.main`, `[data-activity-slot=main]`
54. `main-bottom-panel` — `main-bottom-panel/MainBottomPanel.tsx:57-86` + `MainBottomPanel.module.css` — нижний ящик центральной колонки («Left Bottom»), доля высоты `1 - mainSplit`, glint-border карточка — `.panel`, `.card`
55. `main-bottom-resize-handle` — `main-bottom-panel/MainBottomPanel.tsx:64-73` + `MainBottomPanel.module.css` — горизонтальный сплиттер по верхней кромке ящика (10px) — `.resizeHandle`, `.resizeHandleBar`
56. `right-panel-column` — `right-panel/RightPanel.tsx:102-110` + `RightPanel.module.css` — правая колонка (2 карточки + свои ActivityBar'ы), width или `flex:1 1 0` в режиме fill — `.column`
57. `right-panel-width-handle` — `right-panel/RightPanel.tsx:113-124` + `RightPanel.module.css` — вертикальный сплиттер, торгует шириной с File-панелью или центром — `.resizeHandle`, `.resizeHandleActive`, `.resizeHandleBar`
58. `right-panel-top-card` — `right-panel/RightPanel.tsx:133-151` + `RightPanel.module.css` — верхняя карточка `rightTop` + вертикальный ActivityBar справа, drop-target — `.cardWithBar`, `.card`, `[data-activity-slot=rightTop]`
59. `right-panel-split-handle` — `right-panel/RightPanel.tsx:155-164` + `RightPanel.module.css` — прозрачный разделитель между карточками с «грипом» — `.splitHandle`, `.splitGrip`
60. `right-panel-bottom-card` — `right-panel/RightPanel.tsx:166-184` + `RightPanel.module.css` — нижняя карточка `rightBottom` + зеркальный ActivityBar (`align=bottom`) — `.cardWithBar`, `.card`, `[data-activity-slot=rightBottom]`
61. `file-panel-column` — `file-panel/FilePanel.tsx:91-98` + `FilePanel.module.css` — файловая колонка, вертикально разбита на 2 карточки — `.filePanel`
62. `file-panel-width-handle` — `file-panel/FilePanel.tsx:101-112` + `FilePanel.module.css` — вертикальный сплиттер, синхронный relayout Monaco (`layoutActiveEditorNow`) — `.resizeHandle`, `.resizeHandleActive`, `.resizeHandleBar`
63. `file-panel-top-card` — `file-panel/FilePanel.tsx:114-129` + `FilePanel.module.css` — верхняя карточка: mode-хедер + BrowserPane / FileViewer / PanelPlaceholder — `.card`, `.topCard`, `.modeHeader`
64. `file-panel-split-handle` — `file-panel/FilePanel.tsx:133-142` + `FilePanel.module.css` — горизонтальный сплиттер между карточками — `.splitHandle`, `.splitGrip`
65. `file-panel-bottom-card` — `file-panel/FilePanel.tsx:143-155` + `FilePanel.module.css` — нижняя карточка `centralBottom` с `BottomTabBar` + телом активности, фикс-высота — `.card`, `.bottomCardWithTabs`
66. `file-panel-mode-tabs` — `file-panel/FilePanelModeTabs.tsx:10-29` + `FilePanelModeTabs.module.css` — сегментированный переключатель Files | Web (склеены в центре, скругления по краям) — `.switcher`, `.tab`, `.left`, `.right`, `.active`
67. `browser-pane` — `file-panel/BrowserPane.tsx:77-104` + `BrowserPane.module.css` — Web-режим: DOM-навбар (back/forward/reload + адресная строка-форма) над viewport-плейсхолдером, к которому позиционируется нативный child-webview; скрывается при перекрытии поповерами — `.pane`, `.navbar`, `.navBtn`, `.addrForm`, `.addr`, `.viewport`, `[data-browser-viewport]`
68. `panel-placeholder` — `panel-placeholder/PanelPlaceholder.tsx:31-42` + `PanelPlaceholder.module.css` — пустое состояние панели без выбранной активности: `PanelIcon`-глиф + подпись + подсказка + пилюля «Open Tool» — `.placeholder`, `.glyph`, `.label`, `.hint`, `.trigger`
69. `activity-placeholder` — `panel-placeholder/ActivityPlaceholder.tsx:21-27` + `ActivityPlaceholder.module.css` — пустое тело УЖЕ выбранной активности (без пикера), глиф 36px — `.placeholder`, `.glyph`, `.label`, `.hint`
70. `webview-loading-skeleton` — `panel-placeholder/WebviewLoadingSkeleton.tsx:38-60` + `WebviewLoadingSkeleton.module.css` — шиммер-скелет (тулбар-пилюля + поиск + 6 строк) на время resolve вебвью, с «Waiting for the extension host… Ns · attempt N» после 3с — `.wrap`, `.bar`, `.sk`, `.pill`, `.search`, `.rows`, `.row`, `.icon`, `.lines`, `.line`, `.lineDim`, `.waitNote`, `.srOnly`
71. `webview-load-error` — `panel-placeholder/WebviewLoadingSkeleton.tsx:65-75` + `WebviewLoadingSkeleton.module.css` — терминальное состояние «This panel didn't load» + кнопка Retry — `.errWrap`, `.errIcon`, `.errTitle`, `.errHint`, `.retry`
72. `chat-switch-skeleton` — `panel-placeholder/ChatSwitchSkeleton.tsx:10-21` + `ChatSwitchSkeleton.module.css` — брендовая «шторка» над чат-iframe при переключении сессии: логотип с дышащим свечением, подпись, indeterminate-полоса — `.wrap`, `.brand`, `.glow`, `.logo`, `.caption`, `.bar`, `.barFill`
73. `contributed-container-body` — `activity-bodies/ContributedContainerBody.tsx:33-37` + `ContributedContainerBody.module.css` — тело контрибьютнутого view-контейнера расширения (список его views) — `.root`
74. `contributed-view-section` — `activity-bodies/ContributedContainerBody.tsx:62-77` + `ContributedContainerBody.module.css` — секция одного view: хедер (title/description/badge из `createTreeView`) + тело — `.view`, `.title`, `.viewDescription`, `.viewBadge`
75. `webview-view-anchor` — `activity-bodies/ContributedContainerBody.tsx:136-142` + `ContributedContainerBody.module.css` — «якорь» (rect + border-radius), над которым PersistentWebviewLayer позиционирует живой iframe; внутри — скелет/ретрай — `.frame`, `.frameFlush`, `[data-webview-anchor]`
76. `persistent-webview-layer` — `activity-bodies/PersistentWebviewLayer.tsx:49-53, 198-217` (inline styles, без css-модуля) — `position:fixed` слой z-index 5: iframe'ы вебвью-views живут всё время работы приложения и синхронизируются по rect якоря (rAF-loop при драге сплиттера/ресайзе, burst на смену сессии); внутри — шторка `ChatSwitchSkeleton`
77. `welcome-placeholder` — `main/WelcomePlaceholder.tsx:11-37` + `WelcomePlaceholder.module.css` — брендовый экран без открытых сессий: логотип, «KaminIDE», версия, тэглайн, 2 CTA-кнопки, 3 фичи-чипа — `.welcome`, `.logoWrap`, `.logo`, `.title`, `.version`, `.tagline`, `.actions`, `.primary`, `.secondary`, `.features`, `.feature`
78. `customize-content-panel` — `main/CustomizePanel.tsx:31-48, 81-88` + `CustomizePanel.module.css` — контент-область Customize: хедер (title+subtitle) + тело выбранной страницы (Settings/Design/Extensions/Logs/System/контрибьютнутая) + `ComingSoon` — `.panel`, `.header`, `.title`, `.subtitle`, `.body`, `.bodyFlush`, `.placeholder`
79. `design-panel-shell` — `main/DesignPanel.tsx:19-40, 43-55` + `DesignPanel.module.css` — контейнер дизайн-системы: 6 секций с заголовком/сабтайтлом/телом — `.root`, `.section`, `.sectionHeader`, `.sectionTitle`, `.sectionSubtitle`, `.sectionBody`
80. `logs-panel` — `main/LogsPanel.tsx:73-137` + `LogsPanel.module.css` — VS Code «Output»: слева навигация каналов, справа тулбар (search-инпут, copy, clear-all) + `<pre>` буфер с auto-scroll; empty-state — `.layout`, `.list`, `.item`, `.active`, `.itemName`, `.itemExt`, `.right`, `.toolbar`, `.search`, `.toolBtn`, `.body`, `.empty`
81. `system-log-panel` — `main/SystemLogPanel.tsx:27-72` + `SystemLogPanel.module.css` — системный лог (newest-first): search + сегментированный фильтр уровней (all/error/warning/info) + clear; строки с иконкой уровня, source, message, relative-time — `.layout`, `.toolbar`, `.search`, `.levels`, `.levelBtn`, `.levelActive`, `.clear`, `.empty`, `.list`, `.row`, `.icon`, `.source`, `.message`, `.time`, `.error`, `.warning`, `.info`
82. `settings-panel` — `settings/SettingsPanel.tsx:28-74` + `SettingsPanel.module.css` — страница Settings: 2 секции (Notifications, Terminal) с чекбокс-строками и описаниями — `.root`, `.section`, `.sectionTitle`, `.row`, `.rowText`, `.rowDesc`
83. `legacy-bridge-card` — `settings/LegacyBridgeCard.tsx:82-101` + `LegacyBridgeCard.module.css` — одноразовая карточка «Legacy Electron Bridge detected» с иконкой, описанием найденного и кнопкой удаления — `.card`, `.icon`, `.body`, `.title`, `.desc`, `.remove`
84. `extensions-panel` — `extensions/ExtensionsPanel.tsx:85-108` + `ExtensionsPanel.module.css` — список расширений с хедером + кнопкой Install (.vsix), группы «Installed — N» / «Built-in — N», empty — `.root`, `.header`, `.installBtn`, `.list`, `.empty`, `.groupHeader`
85. `extension-row` — `extensions/ExtensionsPanel.tsx:56-77` + `ExtensionsPanel.module.css` — строка расширения: иконка (data-URL или codicon-fallback), имя+версия/статус, Enable/Disable, trash-uninstall — `.row`, `.disabled`, `.icon`, `.iconFallback`, `.meta`, `.name`, `.sub`, `.rowActions`, `.toggle`, `.uninstall`
86. `problems-panel` — `problems/ProblemsPanel.tsx:44-102` + `ProblemsPanel.module.css` — Problems: хедер со счётчиками-фильтрами (errors/warnings), группы по файлу (chevron + TreeIcon + имя + dir + count), «Show N more files» — `.root`, `.header`, `.counts`, `.countBtn`, `.countActive`, `.errIcon`, `.warnIcon`, `.list`, `.empty`, `.group`, `.fileRow`, `.chevron`, `.fileIcon`, `.fileName`, `.fileDir`, `.fileCount`, `.showMore`
87. `problem-row` — `problems/ProblemRow.tsx:29-40` + `ProblemsPanel.module.css` — строка диагностики: глиф уровня, message, origin `source(code)`, `[Ln x, Col y]`; клик — reveal в редакторе — `.row`, `.sevIcon`, `.sevError`, `.sevWarning`, `.sevInfo`, `.sevHint`, `.message`, `.origin`, `.location`
88. `terminal-view` — `terminal/TerminalView.tsx:54-77` + `TerminalView.module.css` — тело активности Terminal (per-slot состояние): тулбар + стек сессий + empty-state — `.root`, `.body`, `.empty`
89. `terminal-toolbar` — `terminal/TerminalToolbar.tsx:151-216` + `TerminalToolbar.module.css` — хедер терминала: overflow-шевроны прокрутки, стрип pill-табов с close, «+» New terminal — `.bar`, `.scrollBtn`, `.tabs`, `.tab`, `.tabActive`, `.tabLabel`, `.close`, `.anchor`, `.addBtn`
90. `terminal-shell-menu` — `terminal/TerminalToolbar.tsx:112-149` + `TerminalToolbar.module.css` — портал-меню оболочек: иконка+label, тег «default», star-кнопка выбора дефолта, empty-строка — `.menu`, `.menuEmpty`, `.menuRow`, `.menuItem`, `.itemIcon`, `.itemLabel`, `.defaultTag`, `.starBtn`, `.starOn`
91. `terminal-session-host` — `terminal/TerminalSession.tsx:142-149` + `TerminalView.module.css` — хост одного xterm-инстанса на PTY, `display:none` при неактивности (буфер/скролл выживают), тема из `--editor-bg/-fg/-cursor` — `.session`, `[data-pty-id]`

## 5. FileTree (16)

92. `file-tree-root` — `file-tree/FileTreeView.tsx:55-74` + `FileTreeView.module.css` — корень дерева: хедер + скроллящееся тело, right-click по пустой области = меню корня — `.root`, `.body`, `[data-file-tree]`
93. `file-tree-empty-state` — `file-tree/FileTreeView.tsx:40-53` + `FileTreeView.module.css` — «No active session with a folder» с folder-глифом и двумя подсказками — `.empty`, `.emptyIcon`, `.emptyHint`
94. `file-tree-folder-row` — `file-tree/FileTreeView.tsx:171-195` + `FileTreeView.module.css` — строка папки: chevron (или spinner `codicon-loading codicon-modifier-spin`), `TreeIcon`, label с decoration-цветом, badge; draggable при depth>0, Ctrl/Shift-select — `.node`, `.row`, `.rowDir`, `.rowSelected`, `.dropTarget`, `.chevron`, `.icon`, `.label`, `[data-tree-id]`
95. `file-tree-file-row` — `file-tree/FileTreeView.tsx:228-253` + `FileTreeView.module.css` — строка файла: chevron-спейсер, `TreeIcon(file)`, label, badge; клик открывает файл — `.row`, `.rowFile`, `.rowSelected`, `.chevronSpacer`, `.icon`, `.label`
96. `file-tree-children-states` — `file-tree/FileTreeView.tsx:196-222` + `FileTreeView.module.css` — контейнер детей + «Loading…», «(empty)», «Show N more (M hidden)» (кап 100, шаг 200) — `.children`, `.loading`, `.emptyChild`, `.showMore`
97. `file-tree-row-badge` — `file-tree/file-tree-helpers.tsx:62-65` + `FileTreeView.module.css` — бейдж FileDecoration (git-статус и пр.), цвет из ThemeColor — `.badge`
98. `file-tree-header-toolbar` — `file-tree/FileTreeHeader.tsx:26-77` + `FileTreeHeader.module.css` (+ `FileTreeView.module.css` `.flash`) — тулбар дерева: имя папки, индикатор «Indexing…» со спиннером, 3 икон-кнопки (locate selected / collapse-all↔expand-all / refresh); locate скроллит+«флешит» строку — `.header`, `.title`, `.indexing`, `.actions`, `.btn`, `.flash`
99. `tree-icon-img` — `file-tree/TreeIcon.tsx:39-40` + `TreeIcon.module.css` (+ `file-icons.ts`, `vendor/fileIcons.ts`, `vendor/folderIcons.ts`) — иконка строки: синхронно Catppuccin, затем апгрейд до иконки активной contributed icon-theme — `.img`
100. `file-context-menu` — `file-tree/FileContextMenu.tsx:133-146` + `FileContextMenu.module.css` — портал-меню файловых операций у курсора, группы с сепараторами, встроенные + контрибьютнутые `explorer/context` — `.menu`, `.item`, `.danger`, `.hasSub`, `.itemIcon`, `.label`, `.chevron`, `.separator`
101. `file-context-submenu` — `file-tree/FileContextMenu.tsx:147-157` + `FileContextMenu.module.css` — каскад «Open In ▸» с grace-задержкой закрытия 250мс — `.menu`, `.item`, `.itemIcon`, `.label`
102. `generic-tree` — `tree/Tree.tsx:38-53` + `Tree.module.css` — переиспользуемое рекурсивное дерево (`role=tree`), полностью контролируемое (expanded set + selectedId) — `.tree`, `.subtree`
103. `generic-tree-row` — `tree/Tree.tsx:63-106` + `Tree.module.css` — строка: chevron (скрываемый), codicon папки/файла, label, правый meta; indent 14px/уровень — `.row`, `.selected`, `.chevron`, `.chevronHidden`, `.iconDir`, `.iconFile`, `.label`, `.meta`
104. `contributed-tree-view-body` — `activity-bodies/TreeViewBody.tsx:42-49` + `file-tree/FileTreeView.module.css` (переиспользование) — тело контрибьютнутого `TreeDataProvider`: опциональный message-баннер + ленивые уровни — `.root`, `.body`, `.loading`, `.emptyChild`
105. `contributed-tree-node-row` — `activity-bodies/TreeViewBody.tsx:144-178` + `file-tree/FileTreeView.module.css` — строка провайдерного узла: chevron/spacer, чекбокс, иконка, label, description; drag&drop если провайдер зарегистрировал контроллер — `.node`, `.row`, `.rowDir`, `.rowFile`, `.rowSelected`, `.chevron`, `.chevronSpacer`, `.label`
106. `contributed-tree-checkbox` — `activity-bodies/TreeViewBody.tsx:162-174` + `file-tree/FileTreeView.module.css` — `role=checkbox` (`TreeItemCheckboxState`), клавиатурный toggle — `.treeCheckbox`
107. `contributed-tree-node-icon` — `activity-bodies/TreeViewBody.tsx:189-197` + `file-tree/FileTreeView.module.css` — ThemeIcon→codicon / resourceUri→`TreeIcon` / generic-глиф — `.icon`

## 6. Editor (8)

108. `file-viewer-wrapper` — `file-viewer/FileViewer.tsx:62-78` + `FileViewer.module.css` — обвязка редактора: таб-стрип + тело (Monaco / webview-панель), слой удерживаемых `retainContextWhenHidden` панелей, drop-zone для внешних файлов — `.viewer`, `.body`, `.bodyFlush`, `.retainLayer`, `[data-drop-zone=editor]`
109. `file-viewer-empty` — `file-viewer/FileViewer.tsx:81-88` + `FileViewer.module.css` — пустое состояние с file-глифом и подсказкой про `Ctrl+P` (`<kbd>`) — `.empty`
110. `file-viewer-tabs-strip` — `file-viewer/FileViewerTabs.tsx:155-165, 196-199` + `FileViewerTabs.module.css` — стрип открытых файлов (`role=tablist`), pointer-reorder, dashed-индикатор вставки, скрытый скроллбар — `.bar`, `.strip`, `.dropIndicator`
111. `file-viewer-tab` — `file-viewer/FileViewerTabs.tsx:166-195` + `FileViewerTabs.module.css` — pill-таб: pin-иконка, `TabIcon` (файловая или иконка расширения-владельца вебвью), label, dirty-точка, кнопка close; middle-click закрывает, right-click — меню с Close/Close Others/Close to the Right/Close All — `.tab`, `.tabActive`, `.tabDragging`, `.pinIcon`, `.tabIcon`, `.label`, `.dirty`, `.close`
112. `file-viewer-tabs-overflow` — `file-viewer/FileViewerTabs.tsx:200-232` + `FileViewerTabs.module.css` — «▾» при переполнении + меню всех открытых файлов — `.overflow`, `.overflowBtn`, `.overflowMenu`, `.overflowItem`, `.overflowItemActive`, `.overflowLabel`
113. `monaco-editor-host` — `file-viewer/MonacoEditor.tsx:345-348` + `MonacoEditor.module.css` (+ `monaco-loader.ts` и др.) — контейнер Monaco: minimap, sticky-scroll, скроллбары 8px, `fixedOverflowWidgets`, `--font-mono`/13px; error-состояние «Failed to open» — `.host`, `.error`, `:global(.monaco-editor .scrollbar .slider)`
114. `webview-panel-view` — `file-viewer/WebviewPanelView.tsx:369-387` + `WebviewPanelView.module.css` — sandbox-iframe расширения (`kaminwebview://`) + fade-cover с спиннером до `__kaminReady`, watchdog/crash-reload, ретрай-карточка — `.container`, `.frame`, `.loader`, `.loaderHidden`, `.spinner`
115. `webview-tab-icon` — `file-viewer/WebviewTabIcon.tsx:29-38` (без css-модуля) — иконка таба вебвью-панели: иконка владеющего расширения (`hostRpc.extensions.icon`), fallback `codicon-browser`

## 7. StatusBar (5)

116. `status-bar-root` — `status-bar/StatusBar.tsx:29-48` + `StatusBar.module.css` — `<footer>` с левой и правой группами, contributed-элементы отсортированы по priority — `.statusBar`, `.left`, `.right`
117. `status-item-builtin` — `status-bar/StatusBar.tsx:147-158` + `StatusBar.module.css` — встроенный информационный item (N active / N failed / N off / N cmds), `tabIndex=-1`, тон ok/warn/brand — `.item`, `.ok`, `.warn`, `.brand`
118. `status-item-contributed` — `status-bar/StatusBar.tsx:69-83` + `StatusBar.module.css` — item расширения с парсингом `$(icon)`, цветом и командой по клику — `.item`, `.clickable`
119. `status-editor-encoding-eol` — `status-bar/StatusBar.tsx:55-64` + `StatusBar.module.css` — «UTF-8» + LF/CRLF, только при активном текстовом редакторе — `.item`
120. `status-version-update` — `status-bar/StatusBar.tsx:90-145` + `StatusBar.module.css` — трёхсостоянный бренд-item: «KaminIDE x.y.z» (check for updates) → «⬇ Update x.y.z» → прогресс-бар «Updating N%» (`role=progressbar`) — `.item`, `.clickable`, `.brand`, `.update`, `.downloading`, `.progressFill`, `.progressLabel`

## 8. Overlays (9)

121. `confirm-modal` — `overlays/ConfirmModal.tsx:73-98` + `ConfirmModal.module.css` — замена `window.confirm`/`alert`, sanitized HTML-body, Esc/backdrop = cancel, автофокус Confirm, восстановление фокуса; danger-вариант — `.overlay`, `.dialog`, `.title`, `.body`, `.actions`, `.cancelBtn`, `.confirmBtn`, `.danger`
122. `prompt-modal` — `overlays/PromptModal.tsx:71-102` + `PromptModal.module.css` — замена `window.prompt`: инпут с placeholder/defaultValue, live-валидация с inline-ошибкой и блокировкой OK — `.overlay`, `.dialog`, `.title`, `.input`, `.invalid`, `.error`, `.actions`, `.cancelBtn`, `.confirmBtn`
123. `quick-pick-modal` — `overlays/QuickPickModal.tsx:65-123` + `QuickPickModal.module.css` — `showQuickPick`: фильтр-инпут, prompt, listbox с separator-строками, `canPickMany` с чекбоксами и «OK (N)»; `$(icon)`-рендер — `.overlay`, `.panel`, `.title`, `.input`, `.prompt`, `.list`, `.empty`, `.separator`, `.item`, `.check`, `.label`, `.description`, `.detail`, `.actions`, `.cancelBtn`, `.okBtn`
124. `quick-open` — `overlays/QuickOpen.tsx:84-125` + `QuickOpen.module.css` — Ctrl+P: backdrop + бокс, инпут, listbox результатов (имя + путь), стрелки/Enter, debounce 80мс — `.backdrop`, `.box`, `.input`, `.list`, `.empty`, `.item`, `.itemActive`, `.itemName`, `.itemPath`
125. `find-in-files` — `overlays/FindInFiles.tsx:89-137` + `FindInFiles.module.css` — Ctrl+Shift+F: инпут, статус-строка, строки с `rel:line` и сниппетом с `<mark>` — `.backdrop`, `.box`, `.input`, `.status`, `.list`, `.item`, `.itemActive`, `.itemHeader`, `.itemRel`, `.itemLine`, `.itemSnippet`, `.match`
126. `workspace-symbols` — `overlays/WorkspaceSymbols.tsx:79-111` + `QuickOpen.module.css` (переиспользование) — Ctrl+T «Go to Symbol in Workspace»: codicon по `SymbolKind`, имя, containerName + файл — `.backdrop`, `.box`, `.input`, `.list`, `.empty`, `.item`, `.itemActive`, `.itemName`, `.itemPath`
127. `command-palette` — `command-palette/CommandPalette.tsx:26-90` + `CommandPalette.module.css` — Ctrl+Shift+P: скрим-кнопка + панель, инпут-строка с codicon-search и `<kbd>Esc</kbd>`, список команд (category + title + id), футер «N commands · Enter to run» — `.scrim`, `.palette`, `.inputRow`, `.input`, `.kbd`, `.list`, `.empty`, `.row`, `.title`, `.category`, `.id`, `.footer`
128. `toasts-stack` — `overlays/Toasts.tsx:24-62` + `Toasts.module.css` — стек in-app тостов (bottom-right), translucent blurred surface, иконка severity, title/message, кнопки-действия (резолвят промис), dismiss, leaving-анимация — `.stack`, `.toast`, `.info`, `.success`, `.warning`, `.error`, `.leaving`, `.icon`, `.content`, `.title`, `.message`, `.actions`, `.actionBtn`, `.dismiss`
129. `tooltip` — `overlays/Tooltip.tsx:123-138` + `Tooltip.module.css` — единственный document-level тултип по атрибуту `data-tooltip`; двухпроходное измерение + `clampToViewport`; принимает и тултипы из вебвью (через `webviewTooltip`) — `.tooltip`, `[data-tooltip]`, `[data-tooltip-popup]`

## 9. Misc (30)

130. `design-color-tokens` — `main/design-sections.tsx:24-42` + `design-sections.module.css` — 4 группы свотчей (Surface/Text/Accent/Semantic) — `.colorGroups`, `.colorGroup`, `.groupLabel`, `.swatches`, `.swatch`, `.swatchChip`, `.swatchName`
131. `design-typography-tokens` — `main/design-sections.tsx:52-78` + `design-sections.module.css` — `--font-sans`/`--font-mono` образцы + шкала `fs-xs…fs-xl` — `.typoStack`, `.typoSample`, `.typoScale`, `.typoRow`, `.tokenName`, `.tokenValue`
132. `design-spacing-tokens` — `main/design-sections.tsx:80-94` + `design-sections.module.css` — `space-1…space-7` (4…28px) с полосками-мерками — `.spaceStack`, `.spaceRow`, `.spaceBar`, `.tokenName`, `.tokenValue`
133. `design-radius-tokens` — `main/design-sections.tsx:103-115` + `design-sections.module.css` — `radius-xs/sm/md/lg` (4/8/12/16px) — `.radiusGrid`, `.radiusItem`, `.radiusBox`
134. `design-shadow-tokens` — `main/design-sections.tsx:123-134` + `design-sections.module.css` — 9 тонов elevation (`shadow-mini`…`shadow-modal`) — `.shadowGrid`, `.shadowItem`, `.shadowBox`
135. `sample-buttons` — `main/component-samples.tsx:58-67` + `design-sections.module.css` — Primary/Secondary/Danger/Ghost — `.btnPrimary`, `.btnSecondary`, `.btnDanger`, `.btnGhost`
136. `sample-list-item` — `main/component-samples.tsx:69-82` + `design-sections.module.css` — паттерн строки сайдбара (обычная/active/disabled) — `.itemList`, `.listItem`, `.listItemActive`
137. `sample-input` — `main/component-samples.tsx:84-97` + `design-sections.module.css` — базовый текстовый инпут — `.input`
138. `sample-dropdown` — `main/component-samples.tsx:99-143` + `design-sections.module.css` — дропдаун-меню: group-label, item (иконка+label+hint+галка) — `.dropdownAnchor`, `.dropdownTrigger`, `.dropdownMenu`, `.dropdownGroupLabel`, `.dropdownItem`, `.dropdownItemPicked`, `.dropdownItemHint`
139. `sample-tree` — `main/component-samples.tsx:145-167` + `design-sections.module.css` — живой `Tree` в рамке — `.treeFrame`
140. `sample-chips-kbd-code-badge` — `main/component-samples.tsx:169-180` + `design-sections.module.css` — чипы (active/muted/danger), `<kbd>`, инлайн-код, бейдж — `.chip`, `.chipMuted`, `.chipDanger`, `.kbd`, `.codeInline`, `.badge`
141. `sample-toast-triggers` — `main/component-samples.tsx:182-192` + `design-sections.module.css` — 5 кнопок пуша in-app тостов — `.btnSecondary`
142. `sample-modal-triggers` — `main/component-samples.tsx:194-202` + `design-sections.module.css` — Confirm / Confirm danger / Prompt — `.btnSecondary`, `.btnDanger`
143. `sample-external-toast-triggers` — `main/component-samples.tsx:204-237` + `design-sections.module.css` — 4 кнопки внешних (out-of-app) тостов — `.btnSecondary`
144. `sample-tooltip` — `main/component-samples.tsx:239-245` + `design-sections.module.css` — демонстрация `data-tooltip` — `.btnGhost`
145. `sample-block-wrapper` — `main/component-samples.tsx:247-255` + `design-sections.module.css` — обёртка одного блока-примера — `.compStack`, `.compRow`, `.compLabel`, `.compHint`, `.compInline`
146. `sample-horizontal-tab-strip` — `main/component-samples-extra.tsx:44-71` + `activity-bar/BottomTabBar.module.css` — превью рецепта BottomTabBar/FileViewerTabs — `.strip`, `.tabs`, `.tab`, `.tabActive`, `.tabLabel`
147. `sample-vertical-icon-column` — `main/component-samples-extra.tsx:73-107` + `activity-bar/ActivityBar.module.css` — превью рецепта ActivityBar — `.bar`, `.list`, `.btn`, `.btnActive`, `.pickerAnchor`, `.picker`
148. `sample-checkbox-dropdown` — `main/component-samples-extra.tsx:109-141` + `titlebar/LayoutToggles.module.css` — превью рецепта LayoutToggles — `.menu`, `.menuLabel`, `.menuItem`, `.check`, `.checkOn`, `.itemLabel`
149. `sample-context-menu` — `main/component-samples-extra.tsx:143-159` + `activity-bar/ActivityContextMenu.module.css` — статичное превью Hide / Move to ▸ — `.menu`, `.item`, `.itemLabel`, `.itemMoveTo`, `.chevron`
150. `sample-section-header` — `main/component-samples-extra.tsx:161-178` (inline-стили) — лендмарк-заголовок сайдбара (uppercase, muted, letter-spacing 0.08em, `ss01`)
151. `sample-status-bar-items` — `main/component-samples-extra.tsx:180-195` + `status-bar/StatusBar.module.css` — превью 4 состояний status-item — `.item`, `.ok`, `.warn`, `.brand`
152. `sample-panel-icon-family` — `main/component-samples-extra.tsx:197-211` + `design-sections.module.css` — все 8 вариантов `PanelIcon` с подписями — `.codeInline`
153. `sample-placeholders` — `main/component-samples-extra.tsx:213-221` (inline-стили) + `ActivityPlaceholder.module.css` — превью `ActivityPlaceholder` в карточке
154. `global-scrollbar` — `theme/global.css:25-29` (+ `theme/skeleton.css:20-23`) — сквозной стиль скроллбара: 8×8px, прозрачный трек, скруглённый thumb `--bg-overlay`, hover `--text-disabled` — `::-webkit-scrollbar*`
155. `glint-surface-card-ring` — `theme/global.css:91-100` (+ токен `--glint-border`) — общий рецепт «карточки»: padding-box fill + border-box градиентная рамка; используется MainContent / FilePanel / RightPanel / MainBottomPanel — `.glint-surface`
156. `focus-visible-ring` — `theme/global.css:39-52` — единый focus-ring для интерактивных элементов — `[role='button']:focus-visible`, `:focus-visible`
157. `activity-drop-highlight` — `theme/global.css:53-70` — подсветка принимающей карточки при drag активности (over / blocked) — `[data-activity-drop="over"]`, `[data-activity-drop="blocked"]`
158. `dragging-body-classes` — `theme/global.css:72-86` — глобальные состояния драга: iframe'ы теряют pointer-events, hover-эффекты гасятся, курсор `grabbing` — `body.kamin-dragging`, `body.kamin-tool-dragging`
159. `legacy-app-shell-css` — `App.module.css:1-22` — **мёртвый файл**: классы `.app`, `.workbench`, `.center` нигде не импортируются; в порт gpui не нужен

## Счётчики (host renderer — портируется в gpui)

| Зона | Элементов |
|---|---|
| Titlebar | 19 |
| Sidebar / Sessions | 18 |
| ActivityBar | 14 |
| Panels / Slots | 40 |
| FileTree | 16 |
| Editor | 8 |
| StatusBar | 5 |
| Overlays | 9 |
| Misc | 30 |
| **ВСЕГО** | **159** |

## webview (не портируется, рендерится вебвью — без метрик)

titlebar/, chat-header/, input-bar/ (композер), dropdown/, jsonl-viewer/
(транскрипт + рендереры тулов), widgets/, right-panel/ (PlanList/TodoList/
FilesList/ProjectTree), sidebar/ (sessions+customize+tree), customize/
(connectors/skills/plugins/hooks/agents/settings/logs/monitors/stats/sync),
agent-tiles/, file-panel/, terminal/, layout/, overlays/, ui/-примитивы,
icons/BridgeIcon. Полный перечень — в выводе инвентарь-агента
(tasks/a2a745b4cbeaf9b56.output).
