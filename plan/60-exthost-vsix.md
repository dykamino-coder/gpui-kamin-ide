# 60 — Экстеншн-хост и VSIX (НЕ переписывается; хостится GPUI-шеллом)

Источники: `src/kamin-host/**`, `src/exthost/**`. Весь Node-код переносится без изменений; ниже — что шелл обязан обеспечить и где границы.

## Процессы/транспорт
```
GPUI-шелл ──stdio(NDJSON)──► kamin-host PARENT ──fork IPC(advanced)──► exthost CHILD
    ▲                                │ (в проде: kaminhost.exe = переименованный node.exe)
    └── Renderer(GPUI) ──WS /rpc?token── ┘
```
- Роль-диспетчер kamin-host.ts; авто-транспорт (KAMIN_HOST_TRANSPORT=stdio для Rust-шелла)
- PARENT: PTY, fs, индекс, watcher, workspace, sessions, темы; argv --builtin-dir/--data-dir/--cache-dir/--open-folder; env KAMIN_CACHE_DIR
- forkExtHost: serialization:"advanced" ОБЯЗАТЕЛЬНА (Uint8Array через workspace.fs); респаун-бэкофф [200,1000,5000,15000], лимит 6; killProcessTree жнёт форкнутые LSP (taskkill /T); whenReady-гейт (30s); на респаун broadcast kamin:exthost:respawned → шелл обязан РЕ-СИДИТЬ всё состояние
- CHILD: только exthost (изоляция крашей нативных аддонов); config/storage/documents/editors — in-process
- Каналы parent↔child: CHILD_INVOKE (мультиплекс), MIRROR_WORKSPACE/SESSIONS/SESSIONS_ACTIVE/WATCH; child→parent HOST_SEED/BROADCAST/REQUEST_RENDERER/FS/LIST_FILES/SESSION/ENV/CHILD_READY
- ws-server: случайный loopback-порт + 24-байт токен; request() → самому свежему клиенту; методы = services + exthostMethods + heartbeat

## vscode.* поверхность (реализована, target 1.95.0) — шелл её потребляет, не реализует
- commands: registerCommand/executeCommand/getCommands (+registerTextEditorCommand стаб)
- window: showInformation/Warning/ErrorMessage (модальные → ConfirmModal), showQuickPick (canPickMany), showInputBox, showOpen/SaveDialog (→ нативные диалоги шелла), showWorkspaceFolderPick, showTextDocument, createOutputChannel, createStatusBarItem, createWebviewPanel, createInputBox/createQuickPick, createTreeView/registerTreeDataProvider, registerWebviewViewProvider, registerWebviewPanelSerializer, registerFileDecorationProvider, createTextEditorDecorationType, setStatusBarMessage, withProgress, activeTextEditor/visibleTextEditors + события. Стабы: createTerminal, customEditor, terminalLink, uriHandler, tabGroups, notebook
- workspace: workspaceFolders/name/rootPath, textDocuments, getConfiguration/onDidChangeConfiguration (default←global←workspace), fs (полный, через HostFs), findFiles (глоб по индексу), createFileSystemWatcher, applyEdit (fileOps + пер-uri правки редактора), asRelativePath, openTextDocument, onDidChangeWorkspaceFolders, isTrusted:true
- languages: ВСЕ 21 register*Provider (полный список в 50-state-ipc §2) + match (настоящий scoreSelector — критично для vscode-languageclient didOpen), getLanguages, диагностики (реальные). Стабы: CallHierarchy, TypeHierarchy, LinkedEditingRange, Drop/PasteEdit, InlineCompletion, RangeSemanticTokens, OnType/RangeFormatting и пр.
- env: appName "KaminIDE", clipboard (→ шелл), openExternal (→ шелл), uriScheme kamin-ide
- extensions: all/getExtension + фейк-активный vscode.typescript-language-features (для Volar)
- Стаб-неймспейсы: debug, tasks, authentication, l10n (реальная загрузка бандлов), notebooks, tests, scm, comments, chat, lm
- Contribution points (парсятся из package.json, применяются на prepare, снимаются на unload): menus, keybindings (-command unbind), submenus, iconThemes, languages, grammars, viewsContainers (activitybar/panel/auxiliarybar/customize + workbench.view.extension.<id>), views (webview-типа + <viewId>.focus), themes (4 вида uiTheme), commands titles, configuration defaults

## LSP
- LSP-серверы форкают САМИ расширения (vscode-languageclient, child_process) внутри CHILD; шелл не участвует
- Поток к редактору: GPUI editor → kamin:lang:* → child → LanguageFeatures.provide* (все зарегистрированные провайдеры, matchesSelector) → DTO назад. Semantic tokens ремапятся к стандартной легенде
- vscode.execute*Provider-команды зарегистрированы (MCP LSP-тулзы Бриджа ходят в живые провайдеры)
- Диагностики: kamin:diag:set дельты + diag:snapshot на reconnect

## PTY/терминал
- node-pty в PARENT (без отдельного форка); env через applyEnvCollections; win32 useConptyDll app-pref (bundled ConPTY default / system для AppLocker)
- dispose: graceful \x03+exit → kill-tree через 250ms
- shells: discoverShells async-кэш; порядок обнаружения/выдачи powershell → pwsh → cmd → git-bash → WSL-дистры (shells.ts:78-102; без ре-сорта; дефолт выбирает рендерер из kamin.terminal.defaultShell)

## Builtin/VSIX
- Прод шипит ТОЛЬКО claude-bridge (NOT_SHIPPED: hello-world, welcome, icon-theme-fixture)
- Двухфазная загрузка: prepareAll (статика, мгновенно) → фоновый activateStartup; onStartupFinished — ОТДЕЛЬНО ПОСЛЕ устаканивания старта; onCommand через резолвер; onLanguage по открытию документа; 10s таймаут активации; deps force-activate
- ВНИМАНИЕ (из Servo-исследования): activateStartup ждёт listFiles — гигантский workspace (домашняя папка) вешает активацию → Бридж не активируется. При порте проверить/забороть
- VSIX install: нативный пикер → kamin:extensions:installVsix → tar.exe (bsdtar) extract → userExtDir → live install. Uninstall/enable/disable персист. Иконки ≤512KiB data-URL
- require('vscode') → per-ext api; require('kaminide') → {version:"2.0.0", sessions: SessionsApi} (all/projects/active/get/onDidChange*/create/setActive/update/delete)

## Sessions-сервис
- PARENT-owned JsonStore sessions.json (НЕ JSONL; транскрипты JSONL — внутри Бридж-VSIX)
- Один-раз импорт легаси Electron Bridge конфига (+reimport идемпотентно)
- Инвариант active⟹open; активная сессия рулит корнем дерева через applyWorkspaceFolder

## Что шелл ОБЯЗАН предоставить (renderer-coupling из инвентаря)
1. Спаун+супервизия kamin-host (stdio NDJSON), парсинг kamin-host:ready → {wsPort,wsToken} → отдать рендер-слою; Job Object (KILL_ON_JOB_CLOSE), CREATE_NO_WINDOW, рестарт-лимит 3; в проде node.exe→kaminhost.exe переименование (анти taskkill /IM node.exe)
2. Нативные open/save диалоги, openExternal, clipboard — ответы на HOST_REQUEST_RENDERER/shell.*
3. DPAPI secrets (secret.encrypt/decrypt хэндлеры)
4. Вебвью-хостинг (70-webviews): контент-протокол, acquireVsCodeApi, postMessage-релей, тултипы, вотчдог
5. Редактор, говорящий kamin:doc:* / kamin:lang:* / kamin:diag:* / kamin:editor:* (авторитетный интерфейс: src/exthost/exthost-contract.ts)
6. WS-клиент — протокол шелл-агностичен, менять нечего

## Чеклист паритета (exthost)
- [ ] Спаун/супервизия хоста + endpoint-подхват + Job Object + rename-трюк
- [ ] Ре-сид ВСЕГО на exthost:respawned
- [ ] Все shell.*-хэндлеры + DPAPI
- [ ] Редакторный контракт (doc/lang/diag/editor) полностью
- [ ] VSIX install/uninstall/enable/disable UI-флоу
- [ ] Активация: проверить startup-гейт listFiles на больших папках
