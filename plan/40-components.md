# 40 — Покомпонентный инвентарь (renderer → GPUI)

Источник: `src/renderer/components/**` (полный список), `App.tsx`, `main.tsx`. Каждый пункт обязан получить GPUI-эквивалент. gpui-component покрывает базу (кнопки, дропдауны, деревья, табы, модалки, resizable) — ниже отмечено, что кастом.

## Загрузка (main.tsx / App.tsx)
- Boot: шрифты/иконки/темы → error-трапы → бридж → мгновенная contributed-тема из кэша → external-drop → initIpc → render → revealWindow (окно скрыто до пейнта)
- App: AppLayout(sidebar, main) + всегда смонтированные оверлеи: ConfirmModal, PromptModal, QuickPickModal, Tooltip, ActivityContextMenu, ActivityDragGhost, FileContextMenu, SessionContextMenu, QuickOpen, FindInFiles, WorkspaceSymbols, PersistentWebviewLayer

## titlebar/ — см. 10-shell-window (Titlebar, TitlebarQuickActions, LayoutToggles, LayoutPresetsSection, ThemeQuickToggle, TitlebarButton, PanelIcon — рукописный SVG 14×12 «рамка с подсвеченным слотом», 9 вариантов слотов)

## session-tiles/
- SessionTabs: чипы открытых сессий в титлбаре; pointer-drag reorder; «+» пикер (folder/no-folder); пустая зона — drag окна
- SessionTab: чип (иконка-статус, имя, цвет сессии)

## activity-bar/ — см. 30-layout (ActivityBar, BottomTabBar, ActivityPicker, ActivityContextMenu, ActivityDragGhost)

## activity-bodies/
- ActivityBody: резолвер id→тело (встроенные/contributed/placeholder)
- ContributedContainerBody (+ContributedViewBody): вебвью-вью резолвится лениво через host с retry/backoff → скелетон → Retry; чат без сессии → WelcomePlaceholder
- TreeViewBody: contributed TreeDataProvider; ленивые дети (trees.getChildren); стили строк как у файлового дерева
- PersistentWebviewLayer: КРИТИЧНО — каждый contributed-вебвью смонтирован ОДИН раз на жизнь приложения, спозиционирован поверх плейсхолдера [data-webview-anchor]; per-frame rAF-синк ректа при драгах/resize + burst-синк на смену сессии/layout; чат получает switch-cover шиммер. В GPUI: wry-оверлеи позиционируются по ректам плейсхолдеров тем же принципом

## sidebar/
- Sidebar: aside с drag-ручкой; режимы sessions (ActivityBody активного id) / customize (CustomizeMode); null когда скрыт
- SessionsMode: экшн-строки (No folder session / New session) + заголовок PROJECTS + список ProjectGroup
- ProjectGroup: раскрываемая группа-папка (иконка + счётчик) — активные сессии + сворачиваемые «N inactive sessions»; hover-поповер справа (New session here / Delete project)
- SessionItem: строка сессии — статус-точка (bridgeStatus/working = pulse-анимация), имя, отн. время, pin; hover-поповер справа (rename/disconnect/delete); dbl-click/F2 инлайн-переименование; правый клик → SessionContextMenu
- SessionContextMenu: Rename, Auto-rename from chat (только live), Pin/Unpin, Deactivate, свотчи цвета + сброс, Delete
- CustomizeMode: нав-список: Settings/Design/Extensions/Logs/System + contributed customize-контейнеры как TOC-деревья

## main/
- MainContent: центр — Customize→CustomizePanel; нет сессий→WelcomePlaceholder; иначе BottomTabBar(main) + ActivityBody | PanelPlaceholder
- WelcomePlaceholder: лого Kaminoid + версия + слоган + кнопки New-session-in-folder / Empty-session + фича-чипы
- CustomizePanel: активная подпанель (settings/extensions/logs/system/design | contributed) + заголовок/подзаголовок
- SettingsPanel: тогглы background notifications, ConPTY DLL + LegacyBridgeCard
- DesignPanel: справочник дизайн-системы (Colors/Typography/Spacing/Radius/Shadows/Components — живые сэмплы); данные: component-samples.tsx, component-samples-extra.tsx, design-sections.tsx
- SystemLogPanel: фильтруемый лог host/ext/renderer (уровень+поиск+clear, новые сверху)
- LogsPanel: VS Code Output — список каналов + буфер, поиск/clear/copy, sticky-bottom автоскролл

## settings/ — SettingsPanel, LegacyBridgeCard (карта одноразовой очистки Electron Bridge, видна только при его наличии)

## file-panel/
- FilePanel: aside; верхняя карта (Files/Web) + опц. нижняя (BottomTabBar+activity); width-drag + bottom-split drag; fill-режим
- FilePanelModeTabs: сегмент «Files | Web» (codicon files/globe), персист
- BrowserPane: Web-режим — DOM нав-бар (back/forward/reload + адрес) над НАТИВНЫМ child-webview; синк bounds к плейсхолдеру; скрывается когда DOM-поповер перекрывает или в Files-режиме

## file-viewer/ (редакторная зона)
- FileViewer: FileViewerTabs + тело: редактор | WebviewPanelView (webview://id) | Empty; drop-зона внешних файлов; LRU/pinned кольцо открытых файлов (лимит 12)
- FileViewerTabs: pill-строка, role=tablist; pinned-first; drag-reorder сドроп-индикатором (порог 4px); middle-click close; dirty ●; pin-иконка; правый клик = файловое меню + Close/Others/To Right/All; overflow ▾ меню (ResizeObserver)
- WebviewTabIcon: иконка = иконка расширения-владельца (кэш data URL) | codicon-browser
- WebviewPanelView → 70-webviews
- MonacoEditor → **заменяется gpui-component editor**; требования к паритету:
  - кэш модель/вьюстейт на путь (курсор/undo переживают переключение), выселение при закрытии
  - зеркало документов в host (kamin:doc:*), EOL в статус-бар, Ctrl+S save
  - reveal/selection-синк (kamin:editor:*), decorations/insertSnippet/applyEdits (host-request'ы)
  - TextMate/tree-sitter подсветка + contributed-темы; все kamin:lang:* фичи (21 вид) через WS
  - Известно из памяти: peek/goto — в контекст-меню; фича-гэп gpui-editor vs Monaco зафиксировать на имплементации (minimap, multi-cursor, column-select, find-widget, folding, inlay hints, semantic tokens rendering)

## file-tree/
- FileTreeView: ленивое дерево (fs:listDir), автораскрытие корня, кап детей 100 (+200 «show more»), dir-cache сид, chokidar-refresh (fsRevision), reveal/locate каскад (smooth scroll + flash), ctrl/shift мультиселект, нативный drag-out, empty-state
- FileTreeHeader: имя папки + Indexing… + Locate / Collapse↔Expand-all / Refresh
- FileContextMenu: портал: extra(tab) + встроенные операции (вкл. Open In ▸ каскад) + contributed explorer/context, группы с сепараторами, viewport-clamp, отложенное закрытие сабменю
- TreeIcon + file-icons + vendor-таблицы: Catppuccin синхронно → async-апгрейд на contributed icon-тему
- Строки: Folder (chevron|loading-spin → TreeIcon → label с decoration-цветом+tooltip → RowBadge), File (spacer → TreeIcon → label → RowBadge); indentPx(depth); клавиатура onRowKey — хелперы в file-tree-helpers.tsx (RowBadge, indentPx, onRowKey, useFileDecoration, visibleOrder)

## terminal/
- TerminalView: пер-слотовые списки сессий; авто-открытие первой; empty-state; NO split — только табы
- TerminalToolbar: таб-строка (codicon-terminal + label + ×), скролл-шевроны при overflow (80% page), «+» дропдаун шелл-профилей со star-default
- TerminalSession → **заменяется нативным терминал-элементом**: один на PTY; буфер живёт при неактивности; тема из editor-bg/fg/cursor + term-палитра; fontSize 13, scrollback 5000, cursorBlink; Ctrl+C = copy-selection если есть выделение, иначе SIGINT; native paste; resize→pty.resize; data-pty-id для file-drop paste путей

## right-panel/ — RightPanel → 30-layout (две карты + свои ActivityBar, зеркальный align)

## main-bottom-panel/ — MainBottomPanel → 30-layout

## problems/
- ProblemsPanel: группировка по файлам (сворачиваемая), фильтры error/warning с каунтами, кап 100 файлов/200 строк
- ProblemRow: глиф северити + сообщение + source(code) + [Ln,Col]; клик → openFileAt

## extensions/
- ExtensionsPanel: группы Installed (sideloaded) / Built-in; иконка+версия+статус; Enable/Disable, Uninstall, Install-from-.vsix; кэш иконок

## command-palette/
- CommandPalette: Ctrl+Shift+P; скрим + диалог, поиск, список (кап 50), категория+id, Enter = первый, футер-счётчик

## overlays/
- ConfirmModal (sanitized HTML, danger-вариант, Esc/бэкдроп, возврат фокуса) — бэкенд showInformationMessage({modal:true})
- PromptModal (валидация) — showInputBox
- QuickPickModal — фильтр, single/canPickMany (чекбоксы+OK), сепараторы, matchOnDescription/Detail, codicon-текст, ignoreFocusOut
- QuickOpen (Ctrl+P): fuzzy по index.findFile, стрелки, бэкдроп
- FindInFiles (Ctrl+Shift+F): substring по индексу, line+snippet, подсветка
- WorkspaceSymbols (Ctrl+T): SymbolKind→codicon
- Toasts: стек снизу-справа (info/success/warning/error, action-кнопки резолвят промис, dismiss; авто 4s)
- Tooltip: один document-level по data-tooltip; two-pass measure + clampToViewport; принимает тултипы из вебвью (координаты frame→host)
- Все поповеры: clampToViewport (flip+shift); порталы; ОБЯЗАНЫ рисоваться НАД вебвью-оверлеями (в GPUI: при показе меню/диалога поверх wry — скрыть/зашторить вебвью, как BrowserPane already делает)

## panel-placeholder/
- PanelPlaceholder (PanelIcon + label + hint + «Open Tool ▾»), ActivityPlaceholder, ChatSwitchSkeleton (чат-шиммер при переключении), WebviewLoadingSkeleton + WebviewLoadError (Retry)

## status-bar/ — StatusBar: см. точный состав в отчёте (лево: ● N active / ⚠ N failed / ⊘ N off / N cmds + contributed-left по priority desc; право: contributed-right по priority asc + UTF-8 + LF/CRLF (только при активном редакторе) + VersionUpdateItem: idle «KaminIDE {v}» → check / доступен «⬇ Update X.Y.Z» → install / загрузка progress-pill %/MB). $(icon)-парсинг codicon в тексте, опц. цвет, клик → commands.execute

## tool-icon/ — ToolIcon (image URL → Phosphor SVG path → codicon fallback), tool-icon-paths.ts (портировать таблицу как есть)

## tree/ — Tree.tsx: генерик-дерево (переиспользуемый примитив)

## Чеклист паритета (компоненты)
- [ ] Каждый файл выше имеет GPUI-эквивалент или явное решение «в другом доке» (10/30/70/80)
- [ ] Редактор: список паритет-фич против Monaco зафиксирован и закрыт
- [ ] Терминал: нативный элемент с точным поведением TerminalSession
- [ ] Все оверлеи над wry-вебвью (правило штор)
- [ ] Скелетоны/шиммеры/empty-состояния — все
