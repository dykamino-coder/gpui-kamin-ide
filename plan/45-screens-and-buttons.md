# 45 — Каждый экран, панель и кнопка (100%) + как повторить

Справочник для имплементатора: что видит пользователь, что делает каждый контрол, как воспроизвести в GPUI. Хост-UI (нативная оболочка) — здесь; чат-вебвью Бриджа — §Ч (заполняется отчётом chat-buttons + 80-bridge-ux).

Формат: **Контрол → поведение → как повторить**.

## ЭКРАН 0. Boot / пустые состояния
- **Splash (нет)**: окно скрыто до первого пейнта, показывается через 2 rAF (revealWindow). Повтор: GPUI окно visible=false → show после первого draw.
- **WelcomePlaceholder** (нет открытых сессий, центр): лого Kaminoid + «KaminIDE» + версия + слоган + кнопки:
  - «New session in folder…» → нативный folder-picker → sessions.newSessionInFolder(path). Повтор: GPUI file-dialog → hostRpc.sessions.newSessionInFolder.
  - «Empty session» → sessions.newNoFolderSession. Повтор: прямой RPC.
  - Фича-чипы (Claude chat+tools / files+editor / terminal) — информативные, не кликабельны.

## ЭКРАН 1. Титлбар (42px) — см. 10-shell-window §Титлбар (полный порядок/иконки/действия/тултипы уже там)
Ключевые кнопки и повтор:
- Лого — статичен.
- Sidebar-toggle (PanelIcon slot=left) → setSidebarVisible(!v). Повтор: GPUI кнопка → тоггл layout-сигнала.
- Customize-шестерёнка (fa-gear, только при скрытом сайдбаре) → openCustomize("settings")/leaveCustomize.
- SessionTabs (слот): чипы открытых сессий, drag-reorder (pointer), «+» пикер (folder/no-folder). Повтор: GPUI горизонтальный список draggable-чипов + popover-меню.
- Команд-поиск «Type a command…» → commands.execute("workbench.action.showCommands") (открывает CommandPalette). Повтор: кнопка → открыть палитру.
- LayoutToggles (fa-table-columns) → дропдаун: 6 чекбоксов регионов (child disabled без родителя) + пресеты (save/apply/export/import/overwrite/star-default/delete). Повтор: popover с чекбоксами → тогглы layout-сигналов; пресеты → layout-presets стор + нативный save-диалог для export.
- ThemeQuickToggle (иконка по теме) → Appearance-поповер: колонки Dark/Light/Icons (встроенные+contributed) + System. Повтор: popover; клик темы → setTheme/setContributedTheme; клик icon-темы → setIconTheme.
- DevTools (fa-bug) → девтулзы активного вебвью / SystemLogPanel (см. 98 Q2).
- Minimize/Maximize/Close → нативные оконные методы.

## ЭКРАН 2. Activity Bar (48px, вертикальная, для sidebar/rightTop/rightBottom)
- Тайл активности: ЛКМ → активировать слот на этот id (panelStates[slot].active). ПКМ → ActivityContextMenu (Hide/unpin + «Move to ▸» другие слоты). Pointer-drag → перенос между зонами (ActivityDragGhost за курсором). Повтор: GPUI вертикальный список ToolIcon-кнопок; drag через gpui drag-drop; контекст-меню popover.
- Фикс-тайл Customize-шестерёнка (вверху sidebar-бара) → openCustomize.
- «…» пикер (в конце) → ActivityPicker popover: pin/unpin активности в pinned[]. Повтор: popover со списком доступных активностей + чекбоксы.
- Встроенные активности (id→тело): projects→SessionsMode, tree→FileTreeView, terminal→TerminalView, extensions→ExtensionsPanel, problems→ProblemsPanel; contributed→ContributedContainerBody.
- BottomTabBar (горизонтальный вариант для main/mainBottom/centralBottom): те же данные как подписанные табы + «Open Tool ▾».

## ЭКРАН 3. Sidebar
### 3a. SessionsMode (projects)
- «No folder session» / «New session» строки → newNoFolderSession / newSession. Повтор: строки-кнопки → RPC.
- Заголовок PROJECTS.
- ProjectGroup (папка): клик по заголовку → раскрыть/свернуть (иконка + счётчик). Hover-справа поповер: «New session here» → newSession(projectId); «Delete project» → confirm → removeProject. «N inactive sessions» строка → раскрыть список неактивных.
- SessionItem (строка сессии): клик → activateSession (ре-рутит дерево). Статус-точка (bridgeStatus/working=pulse). Отн. время. Pin-кнопка → toggleSessionPinned. Hover-справа поповер: rename/disconnect(deactivate)/delete. Dbl-click/F2 → инлайн-переименование (renameSession, ставит nameSetByUser). ПКМ → SessionContextMenu: Rename / Auto-rename from chat (только live → regenerateTitle) / Pin-Unpin / Deactivate / свотчи цвета+сброс (setSessionColor) / Delete (confirm→removeSession). Повтор: GPUI список строк; popover-меню; inline-edit по dbl-click; свотчи = ряд цветных кнопок.
### 3b. CustomizeMode
- Нав-список: Settings/Design/Extensions/Logs/System + contributed customize-контейнеры (TOC-деревья). Клик → activeCustomizePanel = id. Повтор: список-кнопки → сигнал активной подпанели.

## ЭКРАН 4. Main (центр)
- Нет сессий → WelcomePlaceholder. Customize → CustomizePanel. Иначе BottomTabBar(main) + тело активности.
- **CustomizePanel** подэкраны:
  - **SettingsPanel**: тоггл «background notifications» → appPrefs.set(backgroundToasts). Тоггл «ConPTY DLL» → appPrefs.set(useConptyDll). LegacyBridgeCard (если Electron Bridge найден): «Remove old Bridge» → uninstall_electron_bridge (реимпорт сессий сначала). Повтор: GPUI switch-контролы → hostRpc.prefs.set; карта видна по detect_electron_bridge.
  - **DesignPanel**: справочник дизайн-системы (Colors/Typography/Spacing/Radius/Shadows/Components — живые сэмплы). Повтор: статический экран из theme-токенов (полезен как визуальный тест паритета темы).
  - **SystemLogPanel**: фильтр по уровню + поиск + Clear (clearSystemLog). Строки лога новые сверху. Повтор: GPUI виртуализированный список + фильтры.
  - **LogsPanel** (VS Code Output): выбор канала (список) + буфер; Search/Clear/Copy тулбар; sticky-bottom автоскролл. Повтор: dropdown каналов + текст-вью + тулбар.
  - **ExtensionsPanel**: группы Installed/Built-in. Per-строка: Enable/Disable → extensions.setEnabled; Uninstall (sideloaded) → confirm → extensions.uninstall. «Install from .vsix» → dialog.openVsix → extensions.installVsix. Повтор: GPUI список + кнопки → RPC.

## ЭКРАН 5. File Panel (Files/Web)
- FilePanelModeTabs «Files | Web» → filePanelMode (персист). Повтор: сегмент-контрол.
- **Files** (FileViewer):
  - FileViewerTabs (pill-строка): клик таба → выбрать файл; middle-click / × → закрыть; drag → reorder; ПКМ → меню (Close/Close Others/Close to Right/Close All + файловые операции); pin-иконка → закрепить; overflow ▾ при нехватке места. dirty ● индикатор. Повтор: GPUI таб-строка draggable + popover-меню.
  - Тело: редактор (gpui-component) | WebviewPanelView (webview://id) | Empty («Pick a file… or Ctrl+P»).
- **Web** (BrowserPane): нав-бар — Back/Forward/Reload (browser.back/forward/reload) + адрес-инпут (Enter → browser.navigate, нормализация URL/DuckDuckGo). Тело = нативный child-webview. Повтор: GPUI нав-бар + отдельный wry-вебвью браузера, bounds по плейсхолдеру.

## ЭКРАН 6. Редактор (gpui-component)
Контролов-кнопок нет (клавиатурный); поведение — 40-components §MonacoEditor + 98 Q1. Ctrl+S save, reveal/selection синк, decorations. Гуттер: клик по номеру строки (selection), маркеры диагностик (клик → ?). Контекст-меню редактора (ПКМ): стандартные + Go to Definition/Peek/Rename/Format (через LSP-провайдеры). Повтор: gpui-editor + контекст-меню, команды → hostRpc.lang.*.

## ЭКРАН 7. File Tree
- FileTreeHeader: имя папки; Indicator «Indexing…»; Locate-selected (reveal каскад + smooth scroll + flash); Collapse-all↔Expand-all (treeFoldVersion); Refresh (полный remount). Повтор: тулбар-кнопки → сигналы/RPC.
- Строка-папка: клик → раскрыть/свернуть (tree-expansion, персист); chevron|loading-spin; ctrl/shift+click → мультиселект (не раскрытие). Строка-файл: клик → openFile; dbl-click → pin. ПКМ (строка) → FileContextMenu (New File/Folder, Rename→PromptModal, Delete→confirm→fs.trash, Reveal in OS, Open in Terminal, Copy Path, Open In ▸ каскад, contributed explorer/context). ПКМ (пусто) → root-меню. Drag → нативный OS-drag (beginNativeDrag); drop файла → open/move/copy. RowBadge (декорации). Повтор: GPUI дерево (gpui-component tree) + контекст-меню + drag-drop + иконки (25 иконко-план).

## ЭКРАН 8. Terminal
- TerminalToolbar: таб сессии (codicon-terminal + label + ×→dispose); скролл-шевроны при overflow; «+» дропдаун шелл-профилей (клик → создать PTY на профиле; star → setDefaultShell). Повтор: GPUI таб-строка + popover профилей → hostRpc.pty.create/shells.list.
- Тело: нативный терминал-элемент (буфер живёт при неактивности). Ctrl+C = copy-selection|SIGINT; paste; resize→pty.resize.

## ЭКРАН 9. Right Panel / Main Bottom Panel
- Две карты (RightPanel), каждая со своим ActivityBar (top aligned-top, bottom aligned-bottom). MainBottomPanel = drawer под main. Контролы = как Activity Bar (§2). Ресайзы — 30-layout.

## ЭКРАН 10. Status Bar (24px) — см. 40-components §status-bar (полный состав)
- Лево: ● N active (тултип) / ⚠ N failed / ⊘ N off / N cmds + contributed-left (клик→commands.execute).
- Право: contributed-right + UTF-8 + LF/CRLF (при активном редакторе) + VersionUpdateItem (idle «KaminIDE {v}»→checkForUpdate; available «⬇ Update X.Y.Z»→installUpdate; downloading→progress %/MB). Повтор: GPUI footer, сегменты-кнопки; $(icon)-парсинг codicon.

## ЭКРАН 11. Оверлеи/модалки — см. 40-components §overlays
CommandPalette (Ctrl+Shift+P), QuickOpen (Ctrl+P), FindInFiles (Ctrl+Shift+F), WorkspaceSymbols (Ctrl+T), ConfirmModal, PromptModal, QuickPickModal, Tooltip, Toasts, контекст-меню (файл/активити/сессия). Каждый: клавиатура + бэкдроп-дисмисс; повтор — GPUI overlay-слой поверх, clampToViewport, порталы не нужны (GPUI рисует поверх нативно).

## §Ч. Чат-вебвью Бриджа (label → поведение → bridge.*)
**Повтор в GPUI:** вебвью НЕ переписывается — работает как есть в wry. «Повтор» = (1) вебвью-хостинг (70), (2) editor-context цепочка для attach-file (kamin:editor:active/selections → редактор GPUI обязан слать). Модель монтирования в KaminIDE: только 3 root'а — ChatRoot / ToolsRoot / CustomizeRoot. Sidebar/Titlebar вебвью-бандла в KaminIDE НЕ монтируются (хост рисует их нативно) — перечислены как inactive в §Ч.8.

### Ч.1 chat-header (ChatHeaderStrip — живая полоса над чатом)
- ConnectionStatusBadge — точка 12px (green connected/yellow connecting/grey disconnected/red error); без клика; тултип = статус/ошибка/обратный отсчёт реконнекта (тик 1s)
- CwdDisplay — статичный cwd (тултип = полный путь)
- FolderCrumb — чип папки; клик → bridge.openFolder(cwd); краснеет+warning когда bridge.pathExists=false; встраивает OpenVscodeButton
- OpenExplorerButton (folder-open) → bridge.openFolder(cwd)
- OpenVscodeButton (лого VSCode) → bridge.openInVscode(cwd)
- DiagnosticButton (стетоскоп) → собирает диагностику (session-store+drop-log+render-pipeline) → bridge.saveSessionDiag(name,json); check-иконка 1.5s
- MemoryInfoButton (circle-info) → поповер снапшота JS-heap (performance.memory) + retained entries/tabs; ресэмпл на открытие; без bridge
- DownloadJsonlButton (download) → bridge.downloadJsonl(tabId); фазы idle→busy(живой % из onJsonlDownloadProgress, disabled)→done(check 4s)→failed(red); гард двойного клика; держит crash-watchdog busy
- RegenerateTitleButton (magic-wand) → bridge.regenerateTitle(tabId) (снимает name-sticky, инжектит /rename); только connected
- ReconnectButton (rotate-right) → bridge.reconnectTab(tabId) (WS reopen → server --resume); всегда
- DisconnectButton (link-slash) → bridge.disconnectTab(tabId); только connected
- SessionStats — бар давления контекста + %/токены/стоимость; при ≥70% (или ≥500k warn / ≥90% danger) показывает Compact → bridge.sendInput(tabId,"/compact <handoff>\r")
- PlanProgress — N/M бейдж из activePlan; без клика; one-shot success-тост при all-done; скрыт при replay
- HeaderViewToggles — list-иконка → toggle jsonlVisible (+персист); terminal-иконка → toggle viewerVisible (+персист); встраивает PanelTripletToggle; скрыт в customize
- PanelTripletToggle — 3 сегмента: center=filePanelVisible+bridge.setLayout; bottom=filePanelBottomVisible+setLayout (disabled когда файл-колонка скрыта); right=rightPanelVisible+персист. aria-pressed отражает state
- (легаси, ChatHeader-вариант: ToggleServerPathButton → serverPathVisible; SessionIds статичны; ToggleViewerButton — дубль HeaderViewToggles)

### Ч.2 input-bar
- AttachButton (скрепка) → bridge.openFileDialog() (фолбэк openImageDialog) → в pendingAttachments (картинки = data-URI превью)
- VoiceButton (мик) → useVoiceInput toggle; idle(mic)→countdown(цифра)→recording(stop)→processing(spinner, disabled); скрыт без Whisper/mic-bridge
- SendButton — 3 режима: send(↑ "Send (Enter)") / queue(layer-group, пурпур count badge) / stop(красный "Stop interrupt"). sendMode = (busy && !hasText)?stop:send; stop→send.stop(tabId), иначе build+send.sendToTerminal (гейт not-ready/loading)
- ScrollDownPill — плавающая «Scroll down»; видна при скролле вверх; клик → __scrollChatToBottom + активный терминал вниз
- ActiveFileStatusBar — полоса активного файла/selection хост-редактора; акцент при auto-attach; содержит AttachActiveFileSwitch
- AttachActiveFileSwitch (iOS-toggle «attach file») → setAttachActiveFile(!on) (персист); disabled без файла; при on каждый send авто-аттачит активный файл+строки
- AttachActiveFileButton — альт-вариант того же (file-lines, aria-pressed)
- AttachmentRemoveButton (×) → убрать вложение
- PromptTextarea/SlashAutocomplete — слэш-автокомплит (Tab/Enter=применить), ↑/↓=история, Enter(no shift)=send

### Ч.3 dropdown (общий Dropdown: один открыт, закрытие outside/Esc/iframe-blur; триггер = иконка+имя+chevron)
- ModelDropdown (right) — опции Opus 4.8·1M / Opus 4.8 / Sonnet 5 / Haiku 4.5 / Fable 5 → currentModel + bridge.changeModel(tabId, cliName); эффективная = tab.model||DEFAULT
- EffortDropdown (left) — Low/Medium/High/Extra high/Max → bridge.changeEffort(tabId,value)
- PermissionsDropdown (left) — Default/Accept edits/Plan mode/Auto/Don't ask/Bypass → bridge.changePermissionMode(tabId,value); текущее инференсом по JSONL назад

### Ч.4 widgets (WidgetsPanel)
- QueueWidget — «Message Queue · N pending»; Send now (только при working) → bridge.interruptSession(tabId) (ТОЛЬКО interrupt, не re-send); × на строке → убрать из localQueue; самоскрытие при пустой
- AskUserWidget — таб-кнопки вопросов (check при ответе); AskUserOption радио-карты → updateAnswer; AskUserTextInput свободный; PlanToggle «▶ Show plan»; plan-approval: Approve→respondAskUser(id,'Approved') / Reject→'Rejected'; вопросы: Next→ (multi) / Submit(disabled пока не всё)→respondAskUser(id,answer) / Skip→'[Skipped by user]'
- PermissionWidget — Allow→respondPermission(id,'allow_once') / Always→'allow_session' / Deny→'deny'
- ElicitationWidget (CLI/ExitPlanMode) — schema-поля; Submit/Approve→respondElicitation(id,'accept',content) / Deny→'deny' / Dismiss→'dismiss'
- McpElicitationWidget — Submit→respondMcpElicitation(id,{action:'accept',content}) / Cancel→{action:'cancel'}
- WidgetSchemaField — text input / enum select / boolean Yes-No

### Ч.5 agent-tiles
- SubagentButtonsRow — чип на бегущего субагента (точка+имя); клик → fullscreenAgentId=name; скрыт без бегущих
- SubagentFullscreen — оверлей чата агента (JsonlEntry); Back → fullscreenAgentId=null; авто-очистка на смене таба
- AgentsToolPanel — PanelTabs Active/Completed; дерево TeamGroup (badge disbanded / running/total) → AgentRow (terminated→«kicked»); клик строки → AgentReader (inline chat + Back). **Действий disband/kick в вебвью НЕТ** — статус display-only (завершение — CLI-side)

### Ч.6 jsonl-viewer
- ThinkingBlock «▶ Thinking» → toggle; CollapsibleBlock «▶/▼» → toggle (усечение 1000 симв)
- ToolUseGroup/ToolGroupEntry (ToolGroupSummary «N tool calls · chips ›», role=button, Enter/Space) → раскрыть отдельные JsonlToolUse
- MessageActions (hover): Copy → electronBridge.writeClipboard (check 1.2s); Save as file… → electronBridge.saveTextAs (фолбэк blob)
- WriteToolRender/EditToolRender — «Show full/Collapse» для >40 строк
- ConversationSegmentTabs — pill сегментов (Current/Original/дата); клик → openDisplaySegment (загрузка compact-сегмента, м.б. archived) + scroll; overflow ▾ = меню всех бесед по дате; скрыт при ≤1 сегменте
- ToolCounterToast — не интерактивный «N pending · M done» (pointer-events:none)

### Ч.7 terminal (TerminalPanel)
- Кнопок нет; только TerminalResizeHandle (drag левой кромки → resize + персист terminal-panel-width, кап 60% родителя); в Console — fill, без ручки/хедера

### Ч.8 sidebar/titlebar вебвью-бандла — НЕ монтируются в KaminIDE (легаси App.tsx/AppLayout.tsx)
Существуют в бандле, но inactive: SidebarTopActions (New/Customize/Stats), sessions/* (NewSession/FolderAdd/FolderHeader/SessionItem+Close/Pin/Rename/StatusDot/SavedSessions*/SessionAgentTree/ProjectsHeader/SidebarTree), titlebar/* (TabChip+TabContextMenu, NewSessionModal, ThemeQuickToggle, TitlebarQuickActions, TitlebarButton). ActivityIndicator + ToolCounterToast — переиспользуются ChatRoot; остальное дремлет. (В KaminIDE их роль выполняет нативный хост — §Экраны 1-3.)

### Ч.9 Tools views (ToolsRoot, секция из view-id URL)
- Console — TerminalPanel fill, без контролов
- Plan (PlanList) — PanelTabs Active/Completed (счётчики) → фильтр по статусу; строки не интерактивны (тултип=описание); empty-state
- Todos (TodoList) — N/M + «COMPLETE» бейдж; строки-снапшот, без табов
- Agents — §Ч.5 (Active/Completed, дерево, click-to-read, Back; disband/kick нет)
- (in-chat RightPanel bottom: CardTabBar Plan/Todos; FilesList CardTabBar Files/Write/Read — Files=ProjectTree, Write/Read клик → openFileInTab + авто-открытие файл-колонки; RightPanel drag width+split)

### Ч.10 Customize pages (CustomizeRoot, секция на view)
- **Settings** (SettingsPanel): Server URL + Token инпуты (debounce auto-save, setConfigAndHandleTokenChange); ConnectButton: Test Connect (/tokens/resolve) | Create Token (модалка → POST /tokens) + New Default Session (bridge.createTab); WhisperSettings; Base Prompt textarea + Save (bridge.setBasePrompt); Session Logs → ExportTranscriptsButton; Live Streaming Switch'и (enabled/thinking/side-quests) → optimistic PUT /streaming-settings с rollback+тост. (ThemeToggle есть, но тема ушла в нативные настройки Kamin)
- **Skills** (3-колонки): поиск; строки-select; New skill → AddSkillForm (Create=bridge.createSkill/Cancel); SkillActions: Open in editor (openSkill) / Show in Explorer (showSkillInFolder) / Delete (deleteSkill)
- **Agents** (3-колонки, read-only): поиск; detail: Open in editor / Show in Explorer; без create/delete
- **Connectors/MCP** (3-колонки): поиск; Add connector → AddServerForm (PresetButtonRow чипы, ServerTypeSelect, Stdio/Http/OAuth группы); ConnectorActions: enable/disable toggle (toggleMcpServer) / Test (testMcpServer) / Edit / Connect OAuth/Re-auth (oauthConnectMcpServer) / Remove (removeMcpServer)
- **Hooks**: CardTabBar Active/Library/Log; Active=ActiveHooks (3-col, HookEditor, HookTestRunner, PluginHookApprovalModal); Library=шаблоны; Log=журнал
- **Plugins**: PluginsTabs Active/By Anthropic/Personal + поиск; MarketplaceRow: select/Refresh/Add GitHub-URL (MarketplaceCloneForm)/Add local (installLocalPlugin); PluginCardActions: Cache-Update (syncPluginCache/refreshPluginSource) / Configure (PluginOptionsModal) / Open cache / Open source; SetTokenModal для приватных
- **Monitors**: чипы-статус (select) + лог-пейн; без start/stop (lifecycle по табам); onMonitorStarted/Stopped/Status/Output
- **Sync** (SyncedDataPanel): Force sync now (forceSyncAndLoad) / Reload (load); SyncTree строки → openFile (SyncFileViewer)
- **Logs**: чипы All/Errors/Warnings/Info (счётчики) + поиск + Copy (copyWithToast) + Clear (bridge.clearLogs); строки со стеком разворачиваются
- **Stats**: CardTabBar Overview/Models; диапазон All/30d/7d → refetch /stats/overview; Overview=тайлы+heatmap-tooltip; Models=stacked-bar tooltip+legend; read-only

**Сводка bridge.*-вызовов чата (обязаны работать через вебвью-хостинг):** openFolder, openInVscode, pathExists, saveSessionDiag, downloadJsonl+onJsonlDownloadProgress, regenerateTitle, reconnectTab, disconnectTab, sendInput, interruptSession, changeModel, changeEffort, changePermissionMode, respondAskUser, respondPermission, respondElicitation, respondMcpElicitation, openFileDialog/openImageDialog, setLayout, createTab, setBasePrompt, createSkill/openSkill/showSkillInFolder/deleteSkill, toggle/test/oauth/removeMcpServer, syncPluginCache/refreshPluginSource/installLocalPlugin, clearLogs, + REST к серверу (/tokens, /streaming-settings, /stats).

## Как пользоваться этим документом при имплементации
Каждый экран = отдельная задача GPUI-модуля; «как повторить» = прямая инструкция. Кнопка считается закрытой, когда её поведение в GPUI байт-в-байт совпадает с kamin-ide (визуальный + функциональный тест).
