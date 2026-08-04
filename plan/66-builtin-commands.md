# 66 — Встроенные команды (workbench.*/editor.*/vscode.*) — реализация 100%

Отдельно от plan/65 (там API executeCommand): здесь КОНКРЕТНЫЕ built-in command-id, которые зовут расширения/клавиши. KaminIDE реализовал лишь малую часть — остальное при вызове КИДАЕТ «command not found» (Бридж показывает «Extension crashed»). GPUI-приложение обязано перенести реализованные + закрыть гэпы.

## Архитектура (переносится)
- ЕДИНЫЙ registry (exthost/registry.ts): и contributed, и built-in команды в одной Map; built-in = зарегистрированы хостом на бут с source:"host". Отдельной «builtinCommands»-таблицы НЕТ.
- Каждый built-in хендлер = тонкий шим: broadcast `kamin:*` события в рендерер (ipc.ts превращает в UI-состояние) либо вызов host-сервиса.
- executeCommand(id): lookup → miss → missingCommandResolver (ленивая onCommand:<id> активация) → retry → miss → throw.
- Добавить built-in = один registry.registerCommand в exthost/index.ts (+ обработчик `kamin:*` в рендерере если правит UI). В GPUI: тот же реестр + нативные обработчики событий.

## (A) Что KaminIDE РЕАЛИЗУЕТ (перенести 1:1) — полный список
| Command ID | Что делает |
|---|---|
| workbench.action.showCommands | открыть палитру (kamin:command-palette:open) |
| workbench.action.toggleAuxiliaryBar | kamin:layout:toggle auxiliaryBar |
| workbench.action.togglePanel | kamin:layout:toggle panel |
| workbench.action.toggleSidebarVisibility | kamin:layout:toggle primarySideBar |
| workbench.action.reloadWindow | kamin:window:reload → перезагрузка |
| workbench.action.restartExtensionHost | exit(0) → супервизор респаунит |
| revealFileInOS | kamin:fs:reveal → показать в ОС |
| setContext | registry.setContext(key,value) для when-условий |
| testing.clearTestResults | no-op (эскейп-хетч для fire-and-forget) |
| workbench.view.extension.<containerId> | kamin:view:reveal {container} — ДИНАМИЧЕСКИ на каждый contributed activity-bar контейнер |
| <viewId>.focus | kamin:view:reveal {container,view} — ДИНАМИЧЕСКИ на каждый contributed view |
| vscode.executeHoverProvider | → languageFeatures.provideHover |
| vscode.executeDefinitionProvider | → provideDefinition |
| vscode.executeReferenceProvider | → provideReferences |
| vscode.executeDocumentSymbolProvider | → provideDocumentSymbols |

Плюс НЕ команды, а хардкод-хорды рендерера (global-input.ts) — перенести как нативные шорткаты:
- Ctrl+Shift+P → палитра (напрямую, не через showCommands); Ctrl+B → сайдбар; Ctrl+Z/C/X/V → undo/clipboard файловых операций когда дерево активно; Esc → закрыть палитру. (⚠ два пути к одному состоянию: клавиша прямо + команда через реестр — оба обязаны давать одинаковый результат.)

## (B) Гэпы: частые built-in, которые расширения зовут, но их НЕТ (дорожная карта)
Приоритет по real-world импакту. Многие имеют ГОТОВЫЙ UI в рендерере (QuickOpen/FindInFiles/WorkspaceSymbols) — нужно лишь привязать command-id → дешёвые победы.

### Волна A — файл/редактор (высший импакт)
- vscode.open (Uri в редактор) — критично, ломается «открой файл/ссылку»
- vscode.openWith (custom editor), vscode.diff (сравнение) — GitLens/Git, превью
- workbench.action.files.save / saveAll / newUntitledFile — «save before run» флоу формоттеров/тест-раннеров
- revealInExplorer (в наше дерево — отлично от revealFileInOS), copyFilePath / copyRelativeFilePath
- editor.action.formatDocument / formatSelection — формат по команде (Prettier/ESLint) — высокий импакт
- editor.action.rename, editor.action.quickFix / codeAction, editor.action.marker.next/.prev
- editor.action.goToDeclaration / revealDefinition / goToImplementation / goToTypeDefinition / goToReferences (UI-варианты; execute*Provider уже есть)
- editor.action.commentLine, toggleWordWrap, editor.action.insertSnippet (как command-id)

### Волна B — навигация/quick-open (UI уже есть — привязать)
- workbench.action.quickOpen → QuickOpen.tsx (готов)
- workbench.action.findInFiles → FindInFiles.tsx (готов)
- workbench.action.showAllSymbols → WorkspaceSymbols.tsx (готов)
- workbench.action.gotoLine / gotoSymbol
- list.focusDown/Up/select/expand/collapse — навигация дерева/списков с клавиатуры (нет вообще)

### Волна C — вью/лейаут/вкладки
- workbench.action.closeActiveEditor / closeAllEditors / nextEditor / previousEditor
- workbench.view.explorer / scm / search / debug / extensions (фокус встроенных вью; сейчас только contributed)
- workbench.action.toggleZenMode / toggleFullScreen / toggleActivityBarVisibility / toggleStatusbarVisibility
- workbench.action.openSettings / openSettingsJson / openGlobalKeybindings / openGlobal|WorkspaceSettings — очень частое «настрой в settings»

### Волна D — терминал (UI есть, команд нет)
- workbench.action.terminal.new / sendSequence / clear / kill / focus

### Волна E — LSP-команды (дополнить семейство; сейчас 4 из ~11)
- vscode.executeCodeActionProvider, executeFormatDocumentProvider, executeCompletionItemProvider, executeDocumentHighlights, executeWorkspaceSymbolProvider, executeSignatureHelpProvider, executeTypeDefinition/Implementation/DeclarationProvider

### Волна F — debug-команды
- workbench.action.debug.start/.stop/.stepOver/… (связано с волной debug в plan/65)

## (C) Резолв палитры и клавиш (переносится)
- Палитра: filteredCommands = ВЕСЬ снапшот реестра (нет отдельного built-in списка); фильтр isInternalCommand (_-префикс) + when-гейтинг contributes.menus.commandPalette; матч по title/id/category; execute → hostRpc.commands.execute → registry.executeCommand.
- Клавиши: сначала хардкод-хорды (Ctrl+Shift+P/B/Z/C/X/V/Esc — прямо на состояние), затем contributed: eventToChord (по физическому code, layout-независимо) → resolveFromIndex (последний when-удовлетворённый выигрывает) → execute. Индекс строится из contributed keybindings (дефолтной таблицы клавиш НЕТ). Two-stroke (ctrl+k ctrl+s) НЕ поддержан.
- Прочие точки execute: status-bar items, tree-view node commands, редакторные code-action/language-feature, титлбар-палитра, session-меню.

⚠ Пробел для 100%: у KaminIDE НЕТ дефолтной таблицы клавиш VS Code (F12=goToDefinition, Shift+F12=references, F2=rename, Ctrl+/=comment, Ctrl+P=quickOpen, Ctrl+Shift+F=findInFiles, Ctrl+`=terminal и т.д.). Расширения на них не полагаются (у них свои contributes.keybindings), но ПОЛЬЗОВАТЕЛЬ ждёт их. Для 100% паритета: посеять default-keybindings (карта VS Code) → те же command-id. Внести в волны A-D параллельно командам.

## Чеклист паритета (built-in команды)
- [ ] Перенести все реализованные (A) в GPUI-реестр + нативные обработчики событий (layout/reveal/palette/reload/setContext/view-reveal/4 LSP-execute)
- [ ] Хардкод-хорды рендерера как нативные шорткаты (Ctrl+Shift+P/B/Z/C/X/V/Esc)
- [ ] Волна A: файл/редактор команды (vscode.open/diff/save/format/rename/codeAction/goTo*)
- [ ] Волна B: навигация — привязать готовые QuickOpen/FindInFiles/WorkspaceSymbols + list.*
- [ ] Волна C: вкладки/вью-фокус/лейаут-тогглы/settings
- [ ] Волна D: терминал-команды
- [ ] Волна E: дополнить execute*Provider (все ~11)
- [ ] Волна F: debug-команды (с plan/65 debug-волной)
- [ ] Default-keybindings таблица VS Code → command-id
- [ ] Каждая команда: реестр + палитра-видимость (when) + клавиша, оба пути к одному состоянию
