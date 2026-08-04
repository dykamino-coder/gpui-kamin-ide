# 50 — State + IPC контракт (Rust-клиент)

Источники: `signals/*` (35+ файлов), `kamin-bridge.ts`, `tauri-bridge.ts`, `ws-server.ts`, `services/index.ts`, `exthost-bridge/parent-methods.ts`, `api/types.ts`.

Модель в GPUI: сторы (аналог signals) = gpui Entity/Model-состояния; host WS = Rust-клиент JSON-фреймов `req/res/evt` с id-корреляцией; host→renderer request'ы = зарегистрированные хэндлеры с ответом.

## 1. Сторы (перенос signals → Rust state)

### Ядро
- ipc (оркестрация: подписки на все события, все onHostRequest-хэндлеры, гидрация layout/sessions/workspace/registry/extensions/diagnostics)
- host-rpc (WS-транспорт, reconnect-listeners, request-handlers)
- state: registry (RegistrySnapshot), extensions (+LS-кэш), toasts, palette {open,query}, contextKeys, resolveToastAction (промисы кнопок тостов)

### Layout
- layout: sidebarMode ("sessions"|"customize"), sidebarVisible/Width, activeCustomizePanel, isWindowMaximized, filePanelVisible/Width/Mode ("files"|"web"), filePanelBottomVisible/Height, rightPanelVisible/Width/Split/BottomVisible, mainVisible/BottomVisible/Split
- layout-ratios: px↔ratio математика (file panel = ratio от вьюпорта; sidebar/right = абсолютные px)
- persistence: LayoutSnapshot ↔ состояние, DEFAULT_LAYOUT_SNAPSHOT на пустом сторе
- layout-autosave: дебаунс 250ms → layoutStore.set(patch); viewport-adapter на resize окна
- layout-presets: [LS] kamin.layoutPresets + kamin.layoutDefaultId; save/apply/overwrite/rename/delete/import/export JSON; default-on-boot
- activity: activityRegistry (встроенные + contributed), panelStates по 7 слотам (sidebar, rightTop, rightBottom, centralTop, centralBottom, main, mainBottom) — {pinned[], active}; dnd-ghost, context menu, pin/unpin/setActive/move

### Sessions
- sessions: projects/sessions/activeSessionId (зеркало host sessions.json), projectGroups (active/inactive), openSessions (табы титлбара: inactive слева/active справа, стабильный порядок), SESSION_COLORS
- sessions-cache [LS kamin.sessions.v1] — мгновенный пейнт до ответа хоста; пустой снапшот не кэшировать
- sessions-ui: контекст-меню сессии, renamingSessionId
- session-editor-sync: восстановление открытых файлов при переключении; автосейв editorState+layout+webUrl в host
- chat-switch-cover: оверлей-обложка при переключении чата (прячет флеш)

### Workspace/дерево
- workspace: workspaceFolder, indexing (debounce 250ms), fsRevision; fs-event bridge; dirCache (stale-while-revalidate)
- dir-cache [LS kamin.tree.dircache]: пер-дир кэш листингов (кап 200 дир / 600 записей / 1.5MB, debounce 400ms)
- tree-expansion [LS kamin.tree.expanded]
- file-selection (ctrl/shift мультиселект, sync с активным редактором), file-dnd (drop-target, нативный OS-drag), external-drop (drop → редактор/терминал-пути/копирование-перемещение в дерево), file-context-menu, file-decorations (versioned tick + bridge), search-files (findInFiles)

### Редактор/просмотр
- file-viewer: selectedFile, activeEditorEol (LF|CRLF), revealTarget/pendingReveal, openFiles (табы), treeAllCollapsed/foldVersion
- diagnostics: diagnosticsByFile, counts {errors,warnings,infos,hints}; applyDiagSet (дельты), hydrate на reconnect

### Терминал
- terminal-state: TerminalSessionDescriptor по слотам, terminalInstances (ptyId→handle), migrate/dispose
- terminal-actions: defaultShellId [LS kamin.terminal.defaultShell], создание PTY

### Тема/иконки — см. 20-theme-visual.md (theme, contributed-theme, icon-theme, external-toast-palette)

### Extensions/contributed UI
- extensions-cache [LS kamin.extensions.v1 + kamin.extensionIcons.v1]
- status-bar: statusBarItems + bridge (update/remove) + snapshot-pull
- tree-views: contributed TreeDataProvider (treeChangeVersion, meta, selection, expanded, dnd, reveal)
- webviews: webviewPanels + bridge (create/html/title/reveal/dispose); webview-views: html+roots по viewId; webview-tooltip (кросс-iframe тултип)
- output-channels: каналы + активный; append/replace/clear/dispose/show
- system-log (in-memory)

### Оверлеи/ввод/прочее
- overlays: confirmModal/promptModal/quickPickModal → Promise-результаты (бэкенд host-запросов shell.show*)
- global-input (глоб. клавиши, подавление контекст-меню), keybindings (chord-резолвер + индекс)
- background-toast (фокус-трекер окна; unfocused → нативный external toast), app-prefs (backgroundToasts, useConptyDll → host app-prefs.json)
- browser (embedded): browserUrl + browser://navigated
- updater: availableUpdate, appVersion, downloadProgress; события updater:progress / download-finished

## 2. WS-протокол хоста (полный контракт Rust-клиента)

`ws://127.0.0.1:<port>/rpc?token=<24-byte>`; JSON-фреймы `req/res/evt`. Host request() шлёт САМОМУ СВЕЖЕМУ клиенту (single-window инвариант); на disconnect — failAll.

### Методы сервисов (родитель)
workspace: get→{path|null}, set(path), close · bridge: serverUrl, reimportSessions · prefs: get/set (broadcast changed)
sessions: list, newSessionInFolder(folder), newSession(projectId?,name?), newNoFolderSession, rename, setColor, setPinned, delete, deleteProject, setActive(id|null), deactivate, reorder(id,beforeId|null), setState(id,{editorState,layout,webUrl}), update(id,{name?,metadata?})
fs: listDir→DirEntryDto[]{name,path,type,size,mtimeMs}, readText, writeText, mkdir, delete, trash, restoreTrash→bool, revealInOS, openExternal, openTerminal, move, copy, exists, clipboardWrite(paths,cut), clipboardRead→{paths,cut}
iconTheme: load(jsonPath)→IconThemeDoc, icon(abs)→svg
index: findFile(q)→{rel,abs,score}[], findInFiles(q)→{rel,abs,line,matchStart,matchEnd,snippet}[]
shells: list→{id,label,command,args,icon?}[] · pty: create({cwd?,shellId?,cols?,rows?})→ptyId, write, resize, dispose
doc: open/change/setLanguage/close/save (зеркало документов редактора) · editor: active/selections

### Методы exthost (через child)
registry:snapshot · command:execute(id,...args) · extensions: list/setEnabled/installVsix/uninstall/icon · theme:read(path)
lang:* (22 МЕТОДА; 21 register*Provider — colorProvider бэкает и documentColor, и colorPresentations): completion, hover, definition, formatting, references, documentHighlight, foldingRange, declaration, typeDefinition, implementation, signatureHelp, documentSymbol, documentLink, inlayHints, selectionRange, codeLens, documentColor, colorPresentations, rename, codeAction, semanticTokens, workspaceSymbol
webview: inbound, viewState, closed, restore, persistState · webviewView:resolve
tree: getChildren(viewId,handle?)→TreeNodeDto[], reportSelection/Expansion/Visibility/Checkbox, hasDnd, handleDrag/Drop, getMeta
fileDecoration:get(fsPath) · statusBar:snapshot · diag:snapshot

### События host→клиент (подписки)
- registry:update, extensions:changed, exthost:respawned (ре-сид всего!), exthost:restarting
- fs:event (батч {kind,path}: add/addDir/unlink/unlinkDir/change), workspace:changed, index:status {indexing}, fs:reveal
- pty:data {ptyId,data}, pty:exit {ptyId,code,signal?}
- sessions:changed (SessionsSnapshot)
- notification:show, output:event {channel,op,text?,extensionId}, prefs:changed, clipboard:write
- diag:set {owner,uri,diagnostics}
- statusBar:update / statusBar:remove
- tree:changed/meta/dnd/reveal
- fileDecoration:changed {uris|null}
- webview:create {id,viewType,title,ownerExtId?,localResourceRoots?,retainContextWhenHidden?,initialState?}, webview:html/title/reveal/dispose, webview:post {batch}
- webviewView:html {id,html,localResourceRoots?}, webviewView:reveal
- window:reload, layout:toggle (primarySideBar/panel/auxiliaryBar), command-palette:open, view:reveal {container,view?}

### Host→клиент request/response (обязан ответить)
- shell.showMessage(severity,msg,items)→label · showInputBox→string|undef · showQuickPick→indices|null · showOpenDialog→paths|null · showSaveDialog→path|null · openExternal→bool · readClipboard→string
- secret.encrypt/decrypt (DPAPI-релей — в GPUI родная реализация DPAPI)
- editor: applyEdits(uri,edits,eol?)→bool, revealRange, setDecorations(uri,key,options,items), disposeDecorationType, insertSnippet→bool, setSelections, show

## 3. window.kamin (реализуется GPUI-шеллом нативно)

РЕАЛЬНЫЕ в текущем Tauri (портировать): window.minimize/maximize/close/toggleDevTools/isMaximized/onMaximized · layoutStore.get/set (layout.json shallow-merge) · workspace.open (нативный пикер → workspace.set) · dialog.openVsix · webview.setHtml(id,html,roots) · secrets.encrypt/decrypt (DPAPI) · browser.setBounds/hide/navigate/back/forward/reload + onNavigated · externalToast.show→label / setPalette · nativeTheme (ОС) · hostEndpoint.get/onChanged (рестарт хоста!) · openFolder.onRequested («Open with KaminIDE») · hostLog.push
Плюс прямые команды: clipboard write/read, updater_check/install, diag_pong/visibility/heartbeat, heap_flush, toast_get/toast_action (поллинг/клики внешних тостов), **search_in_files** — ВАЖНО: нативный ripgrep-поиск это ПЕРВИЧНЫЙ путь Find-in-Files (search-files.ts вызывает Rust напрямую; host WS kamin:index:findInFiles — только фолбэк).
Остальное (registry/commands/fs/index/pty/…) — через host WS (в GPUI то же самое).

## 4. Персистентность

### layout.json (глобально; shallow-merge патчи, debounce 250ms)
LayoutSnapshot (ФАКТИЧЕСКИ персистятся capture/apply): sidebarVisible, sidebarWidthPx (px), filePanelVisible, filePanelMode, filePanelWidthRatio (ratio!), filePanelBottomVisible, filePanelBottomHeightRatio, rightPanelVisible, rightPanelWidthPx (px), rightPanelBottomVisible, mainVisible, mainBottomVisible, mainSplit, themeChoice (только глобально), activitySidebar/RightTop/RightBottom/CentralTop/CentralBottom/Main/MainBottom ({pinned[],active}).
⚠ rightPanelSplit объявлен в типе LayoutSnapshot и живёт как сигнал, но capture/applyLayoutSnapshot его НЕ пишут/читают — в layout.json его НЕТ (не включать в «схему 1:1»; решить при порте: воспроизвести как есть или починить персист).
Геометрия окна — отдельно (сейчас tauri-plugin-window-state; в GPUI свой window-state store).

### Пер-сессионный layout
Тот же снапшот минус themeChoice → KaminSession.layout (host sessions.json) через sessions:setState; захват при переключении + автосейв.

### localStorage-ключи → GPUI конфиг-стор (пейнт-акселераторы)
kamin.themeChoice, iconThemeId, contributedThemeId, contributedThemeData, sessions.v1, extensions.v1, extensionIcons.v1, tree.dircache, tree.expanded, terminal.defaultShell, layoutPresets, layoutDefaultId.

### Файлы хоста (dataDir — НЕ трогаем)
workspace.json, sessions.json, app-prefs.json, global-state.json, workspace-state.json, secrets.json (DPAPI), webview-panels.json; globalStorage/<extId>/, logs/, workspaceStorage/<hash>/. Индекс — в cache-dir.

## 5. Модель сессий/проектов

- Project {id, folderPath|null, createdAt}; имя = basename. null = «No folder».
- Session {id, name, projectId, color?, pinned?, open?, lastOpened, createdAt, editorState?{openFiles[{path,pinned}],activeFile}, layout?, webUrl?, metadata?}. open=активная (таб), !open=inactive (группа «N inactive»); pinned = «спящий» таб. metadata = сим Бриджа ({bridge:{conversationId}, nameSetByUser} — стикки-переименование).
- Активация сессии: workspace:changed БРОАДКАСТИТСЯ ПЕРВЫМ (дерево пере-рутится мгновенно, ленивые listDir), индекс перестраивается фоном. Отсутствующая папка → null (пустое дерево, не устаревшее).
- RegistrySnapshot: commands, views, viewContainers (activitybar/panel/auxiliarybar/customize), themes, iconThemes, languages, grammars, contextKeys, menus, keybindings, submenus. ExtensionDescriptor {id, displayName, version, active, builtin, enabled, activationError?, packageJSON, extensionPath}.

## Чеклист паритета (state/IPC)
- [ ] Rust WS-клиент: req/res/evt, id-корреляция, reconnect + re-seed (exthost:respawned!), heartbeat
- [ ] ВСЕ методы из §2 типизированы
- [ ] ВСЕ события из §2 подписаны и маршрутизированы в сторы
- [ ] ВСЕ host-request хэндлеры (§2) с нативными реализациями (диалоги, DPAPI, редактор)
- [ ] window.kamin-эквиваленты (§3) нативно
- [ ] layout.json схема 1:1 + debounce + viewport-adapter + пресеты
- [ ] LS-ключи → конфиг-стор с той же семантикой (кэши мгновенного пейнта)
- [ ] Модель сессий + порядок broadcast при активации
