# 65 — Покрытие 100% VS Code API

Инвентаризация против `@types/vscode` (~21K строк, target 1.95, stable-поверхность) vs реализация kamin-ide (`src/exthost/api/*`). Все 16 неймспейсов ПРИСУТСТВУЮТ как объекты — гэп в «настоящести», не в наличии.

## Текущее состояние (kamin-ide)

| Метрика | Оценка |
|---|---|
| Члены неймспейсов присутствуют (real+stub) | ~88% |
| РЕАЛЬНО функциональны | **~48%** |
| Стабы (форма верна, no-op) | ~40% |
| Отсутствуют | ~12% |
| Классы (из ~118 stable) | ~85 (72%) |
| Енумы (из ~66 stable) | ~54 (82%) |

Водораздел: **editor/document/language/config/fs/UI-примитивы — реальные** (мост в Monaco+host); **исполнительные поверхности — стабы** (debug, tasks, terminal, tests, notebooks, scm, custom editors, auth, chat/lm).

## По неймспейсам (сводка; ПОЛНЫЙ гэп-лист — источник для чеклистов имплементации)

- **commands** REAL (кроме registerTextEditorCommand — стаб)
- **env** REAL (клипборд/openExternal/статика); стабы asExternalUri (identity), telemetry, события; MISSING isAppPortable
- **window** — редакторы/документы/сообщения/инпуты/UI-примитивы REAL. Стабы: **createTerminal (без PTY!)**, registerCustomEditorProvider, terminalLink/ProfileProvider, **registerUriHandler**, withProgress (без прогресса), tabGroups, terminals, notebook-редакторы, WindowState/ColorTheme-события. MISSING: showNotebookDocument, notebook-события, withScmProgress
- **workspace** — почти всё REAL (fs, config, watcher, findFiles, applyEdit, doc-события). Стабы: registerTextDocumentContentProvider, registerFileSystemProvider, registerTaskProvider, notebooks, will/did-Files-события, updateWorkspaceFolders, saveAll (фейк), decode/encode. MISSING: save, saveAs
- **languages** — все члены есть; 21 register*Provider REAL (22 kamin:lang:* метода — colorProvider даёт 2 метода). Стабы-провайдеры (регистрируются, но НЕ опрашиваются — опасный класс гэпов): RangeSemanticTokens, Range/OnTypeFormatting, CallHierarchy, TypeHierarchy, LinkedEditingRange, DocumentDrop/PasteEdit, **InlineCompletion**, EvaluatableExpression, InlineValues, setLanguageConfiguration, createLanguageStatusItem
- **debug** — ВЕСЬ стаб. MISSING: activeStackItem, onDidChangeActiveStackItem, asDebugSourceUri
- **tasks** — стаб; executeTask ФЕЙКАЕТ выполнение
- **scm / comments / authentication / tests / notebooks / chat / lm** — стабы; MISSING: notebooks.createRendererMessaging, lm.invokeTool/registerMcpServerDefinitionProvider/registerLanguageModelChatProvider
- **l10n** REAL
- **Классы MISSING (~29)**: Chat* (8), LanguageModel* (7), Mcp{Http,Stdio}ServerDefinition, DebugStackFrame/Thread, DocumentDrop/PasteEdit(+Kind), InlineCompletionItem/List, TaskGroup, TelemetryTrustedValue, NotebookCellStatusBarItem, TestMessageStackFrame
- **Енумы MISSING (12)**: FilePermission, TerminalLocation, TerminalExitReason, TerminalShellExecutionCommandLineConfidence, NotebookEditorRevealType, NotebookCellStatusBarAlignment, SyntaxTokenType, DocumentPasteTriggerKind, InlineCompletionTriggerKind, ChatResultFeedbackKind, LanguageModelChatMessageRole, LanguageModelChatToolMode

## Дорожная карта до 100% (GPUI-приложение)

### Волна 0 — дешёвые победы (форма без поведения → номинальное покрытие ~100%)
12 енумов + ~29 классов (data-holders) + отсутствующие члены: workspace.save/saveAs, env.isAppPortable, window.showNotebookDocument, debug.asDebugSourceUri, notebooks.createRendererMessaging, lm.invokeTool. Ничего не активирует — но instanceof/типы перестают падать.

### Волна 1 — терминал (высший real-world импакт)
`window.createTerminal` РЕАЛЬНЫЙ: мост на существующий kamin:pty:* (PTY-сервис уже есть!) + интеграция в терминал-UI GPUI (сессия расширения = таб). sendText/show/hide/dispose, exitStatus, processId, onDidOpen/Close/ChangeActiveTerminal, terminals/activeTerminal, TerminalLocation/ExitReason, shellIntegration-события (минимум — стабы честные), TerminalLinkProvider/ProfileProvider. Также registerTextEditorCommand (простая обвязка activeTextEditor).

### Волна 2 — исполнение задач + inline completions
- **tasks**: реальный executeTask/fetchTasks поверх PTY (ShellExecution/ProcessExecution/CustomExecution), TaskGroup, события start/end/process*. UI: интеграция с терминал-табами.
- **InlineCompletion** (ghost text): провайдер → GPUI-редактор (столь же важен для Copilot-класса расширений).
- Дожать languages-стабы: Range/OnTypeFormatting, LinkedEditingRange, CallHierarchy/TypeHierarchy (у gpui-редактора будет UI пиков), DocumentDrop/PasteEdit, InlineValues, setLanguageConfiguration, LanguageStatusItem.
- tabGroups: реальная модель (у GPUI-шелла табы свои — маппинг прямой), TabInput* уже есть.

### Волна 3 — debug (DAP)
Полный Debug Adapter Protocol-клиент в exthost: DebugAdapterDescriptorFactory/Executable/Server/NamedPipe/Inline — запуск адаптеров; startDebugging реальный; брейкпоинты (UI в GPUI-редакторе: gutter, hover-переменные через EvaluatableExpression), activeDebugSession/Console, DebugStackFrame/Thread, tracker-фабрики. UI-минимум: панель стека/переменных/watch + debug-toolbar (это НОВЫЙ UI, в kamin-ide его нет — план UI-части в 40-components дополнить при старте волны).

### Волна 4 — SCM + auth + uri
- **scm**: SourceControl/ResourceGroup/ResourceState реальные + UI-панель (декорации дерева уже есть); quickDiff; inputBox.
- **authentication**: реальный реестр провайдеров + getSession/onDidChangeSessions + UI-подтверждения.
- **registerUriHandler** + OS-регистрация схемы `kamin-ide://` (deep links, OAuth-редиректы — связка с auth).
- comments: CommentController/Thread UI.

### Волна 5 — notebooks + tests + custom editors
- notebooks: сериализаторы, контроллеры с executeHandler, рендеринг ячеек (вебвью-рендереры через существующую вебвью-подсистему), createRendererMessaging, showNotebookDocument.
- tests: TestController/Item/Run + Test Explorer UI.
- custom editors: registerCustomEditorProvider через вебвью-подсистему (CustomTextEditor + CustomReadonlyEditor).

### Волна 6 — chat/lm/mcp (новая поверхность)
ChatParticipant + ответные части, lm.selectChatModels/invokeTool/registerTool, MCP server definitions. Естественная синергия с Бриджем (у нас уже есть MCP-инфраструктура в claude-bridge — можно бэкапить lm.* на неё).

### Постоянные правила
- Стаб допустим ТОЛЬКО честный: регистрация должна либо работать, либо кидать понятную ошибку в лог (не тихо глотать — «registered-but-dead» класс гэпов признан самым опасным).
- Каждая волна закрывается прогоном corpus:compat (top-10 VSIX корпус) + профильным расширением-репрезентантом (W1: Code Runner-класс; W3: js-debug; W4: GitLens/GitHub-auth; W5: Jupyter; W2: Copilot-класс ghost-text).
- generate-locked-spec.ts прогонять на каждой волне — актуализировать locked-спеку и мерить % покрытия автоматически.

## Чеклист паритета (API)
- [ ] Волна 0: енумы/классы/члены — номинальное 100%
- [ ] Волна 1: терминал реальный
- [ ] Волна 2: tasks + inline completions + languages-достройка + tabGroups
- [ ] Волна 3: debug/DAP + debug-UI
- [ ] Волна 4: scm + auth + uri + comments
- [ ] Волна 5: notebooks + tests + custom editors
- [ ] Волна 6: chat/lm/mcp
- [ ] Каждая волна: corpus:compat + расширение-репрезентант + locked-spec замер
