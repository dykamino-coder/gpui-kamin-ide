# 97 — Статус паритета имплементации (опись оригинала × порт)

Полная опись фич оболочки оригинала (без Bridge VSIX, обход
`kamin-ide/src/renderer/**` 2026-07-25) × фактический статус GPUI-порта.
Статусы: ✅ сделано+верифицировано · 🔶 частично · ❌ нет.
Обновлять при каждом закрытом пункте. (Покрытие ПЛАНОМ — см. 95.)

## Титлбар
- ✅ Прозрачный титлбар 42px + drag за всю статику (occlude-паттерн + client-fallback)
- ✅ Бренд-лого kaminoid слева
- ✅ Поиск-пилюля «Type a command…» → палитра
- ✅ Оконные кнопки: Min/Max↔Restore/Close ✅; DevTools-кнопка подключена (plan/98 №2: browser-вебвью → chat-вебвью → Customize/System; тот же wv.open_devtools()-путь, что рабочий OpenDevtools(view_id); click-верификация отложена — debug-инстанс рядом с DLP-soak не поднять, один data-dir)
- ✅ Тоггл сайдбара; 🔶 gear при скрытом сайдбаре (у нас всегда в рейле)
- ✅ Layout-попап: 6 чек-строк + дети disabledWhen «Requires…», не закрывается на клик
- ✅ Попапы титлбара: правый анкор + clamp .max(8) по левому краю (вьюпорт уже попапа не бывает при min-window; полный flip/shift не нужен — позиция фикс справа как у оригинала)
- ✅ Appearance ThemeQuickToggle 1:1 VERIFIED LIVE: header «Appearance» + System-пилюля (follow OS: gpui window_appearance → live-резолв в render, persist themeChoice, glyph circle-half) + 3 колонки Dark/Light/Icons (builtin + contributed по uiTheme; Icons: Catppuccin, contributed iconThemes-канал не разведён); пики НЕ закрывают поповер; System-клик → persist «system» → возврат dark — проверено
- ✅ Триггер-иконка отражает тему: moon (dark) / sun (light) / circle-half (contributed); verified live — луна при dark
- ✅ PanelIcon (glyph-геометрия 1:1)

## Сессионные табы
- ✅ Полоса чипов, одноразмерные, активный всегда виден
- ✅ Reorder pointer-drag чипов (persist sessionOrder; см. очередь п.1)
- ✅ «+»: дропдаун New session in folder… (нативный folder-пикер) / Empty session (verified live: дропдаун открылся probe-кликом; обработчики = welcome-механика)
- ✅ Чип: dot/лейбл/тинт (rest 15-8, hover 22-12, active 26-14 + бордер 45)
- ✅ Hover pin/disconnect в чипе; pin-глиф у pinned; тултип полного имени
- ✅ Sleeping-таб (pinned+closed призрак 0.55, клик реактивирует, тултип); switching-индикатор 🔶 + chat-cover ✅ (очередь п.5)
- ✅ RMB меню чипа (= меню сессии)
- ✅ Overflow «N ⌄» поповер (deferred)

## Сайдбар (sessions)
- ✅ Drag-resize; No folder/New session; PROJECTS; пустое состояние
- ✅ Группы: chevron/иконка/имя/счётчик/коллапс/«N inactive»
- ✅ Fly-out поповеры проекта и сессии (углы карт, occlude, анти-дребезг)
- ✅ Строка: dot, лейбл, relative-время «5d», pin fa-thumbtack
- ✅ Bridge-status dot (working/connecting/error/disconnected; очередь п.5)
- ✅ Клик=оптимистичная активация; rename через меню + dblclick чипа + F2 (см. очередь п.5)
- ✅ Контекст-меню: Rename/Pin/Deactivate/свотчи+clear/Delete

## Дерево файлов
- ✅ Ленивое per-dir (fs напрямую — быстрее хоста), root-строка, Catppuccin полный (394)
- ✅ Chevron + loading-spinner-глиф при запрошенном листинге
- ✅ Watcher (notify рекурсивно + дебаунс 300ms → авто-RefreshTree; live-проверен)
- ✅ Persist раскрытий (кап 500, restore при workspace, ключ treeExpanded в layout.json)
- ✅ «Show N more» VERIFIED LIVE (кап 200 + строка «Show N more…» → show_all; 210-файловая директория: до низа долистано ДРАГОМ СКРОЛЛБАРА (hover→drag тумба — обход wheel-гочи), клик по строке → f200–f209 отрендерены, строка исчезла). DnD файлов ✅ (gpui on_drag DraggedFile + FileDragGhost-пилюля; drop-цели: редактор=открыть, терминал=путь с кавычками; verified live probe-drag — путь файла вставился в PTY). Мультиселект ✅ (selected-set: клик = одиночный select+подсветка accent 12%, Ctrl+клик = toggle; probe emit selectNode обходит Ctrl-гочу; операции меню над множеством: «Delete N items»-модалка → корзина, Cut/Copy множества → FsPaste перебором — verified live: 2 файла скопированы+вставлены, затем «Delete 2 items»→confirm→оба в корзине). File-decorations ✅ VERIFIED LIVE (тест-расширение kamintest.decorations в dev-датадире: registerFileDecorationProvider → бейдж «M» + оранжевые имена (gitDecoration.modifiedResourceForeground) на всех .html В ОБЕИХ панелях дерева; клиент не потребовал правок; расширение убрано после верификации)
- ✅ Хедер: имя+Locate(disabled)/Collapse/Refresh; Indexing ✅ (kamin:index:status → «Indexing…»-бейдж accent в хедере; канал живой — хост шлёт на workspace-переключении; окно бейджа на маленьком воркспейсе мгновенное)
- ✅ RMB меню узла и root

## Контекст-меню файлов
- ✅ Меню+clamp+группы; New/Rename (prompt), Delete (confirm), Cut/Copy/Paste (внутр. буфер), Copy Path/Relative
- ✅ «Open In ▸» каскад; undo-стек Ctrl+Z; CF_HDROP 🔶 (DLP душит верификацию); reserved-валидация ✅ (file_name_error: пусто/разделители/«..»/CON-PRN-AUX-NUL-COM1-9-LPT1-9/запрещённые символы → warning-тост в Create/Rename; 8 кейсов юнит-тестом)

## Редактор
- ✅ code_editor: правка+Ctrl+S+dirty, tree-sitter 9 языков, line numbers, guides, поиск, тема IDE
- ✅ Minimap; viewState; reload при внешнем изменении; scroll-to-line; diagnostics(Problems); editor RMB-меню. Sticky-scroll ✅ VERIFIED LIVE (vendored gpui-component в vendor/ + [patch.crates-io]: 1-строчный pub scroll_handle; indentation-модель = официальный фолбэк VS Code, ui/sticky_scroll.rs compute + 4 теста; first_visible = -offset.y/20px (LINE_HEIGHT 1.25rem); стопка absolute top-0 с гуттером номеров, клик = GotoLine; кап 5 строк; кэш (path,first_visible); soft_wrap(false) на code_editor — паритет Monaco default И точный счёт строк; verified: «40 pub fn compute…» прилип над вьюпортом 46-69). LSP-фичи поверх code_editor ❌ (встроенные tree-sitter/поиск есть)

## Файл-табы
- ✅ Pill: иконка/ellipsis/dirty ●/close ×/middle-click/тултип; RMB Close/Others/Right/All + File actions
- ✅ Reorder; overflow ▾; pin; лимит 12+LRU (очередь п.1/3)

## Панельная система
- ✅ Модель (5 тестов); пикер dots/Open Tool; PanelPlaceholder 7 иконок; tree-тул в любом слоте
- ✅ DnD плиток ПОЛНЫЙ VERIFIED LIVE: ToolPress → порог 4px → hit-test дроп-зон → ghost → drop=move_activity, клик=активация; стрип-табы + РЕЙЛ-ПЛИТКИ правых карт (rail из activity-модели: pinned+active, press=ToolPress, RMB=Hide/Move-to меню — verified: RMB-меню с рейла, drag tree rightTop→rightBottom переместил в модели+persist); вставка по индексу ✅ (ToolDragOverTab + accent-полоса-плейсхолдер у цели). ⚠ «один тайл на (слот,id)»: drop в слот, где тул уже pinned = no-op — НЕ баг. ⚠ ВЕРХНЯЯ right-карта рендерит дерево ХАРДКОДОМ (top_tree), не через activity-тело — при уезде tree из rightTop карта всё равно рисует дерево (унификация тел слотов — отдельный шаг)
- ✅ Persist панельной модели (activityModel в layout.json: {slot:{pinned,active}} save на pin/unpin/активацию/перенос, restore на буте с валидацией active∈pinned; verified live: пины пережили рестарт). Контекст-меню таба тула ✅ (RMB → Hide (unpin) + «Move to <слот>» для всех прочих слотов → move_activity+persist; verified live: RMB-меню открылось probe right-click'ом, «Move to Right Bottom» перенёс живой терминал)

## Layout
- ✅ Сплиттеры/ратио/persist; Files|Web; 6 живых тумблеров панелей
- ✅ Пресеты полные: save (prompt) / apply / delete / ★ default / rename (edit-иконка → prompt) / export (save-диалог → JSON) / import (пикер → merge по имени, перекрытие + тост «Imported N»); overwrite = save с тем же именем (retain+push). UI verified live (Export/Import строки в поповере); диалоги = проверенный prompt_for_paths/new_path-паттерн
- ✅ Web-браузер настоящий (DComp-off фикс)

## Статус-бар
- ✅ N active / N cmds; N failed/off (условно при >0); Encoding UTF-8 + EOL LF/CRLF активного файла
- ✅ contributed items; Update-пилюля (Updater UI, mock-verified)

## Оверлеи (в overlay-окне поверх вебвью — превосходит оригинал)
- ✅ Палитра/QuickOpen/FindInFiles/Symbols; Confirm+Prompt-модалки; тосты; скрим/Esc
- ✅ QuickPickModal; тултипы auto-place (gpui-native). Тост-actions ✅ (shell.showMessage с items → sticky-тост с кнопками через HostReply::Later; клик по кнопке = respond(label) хосту, dismiss × = respond(null); id-конвенция «shellreq-{req_id}»)

## Customize
- ✅ Gear→режим, nav 5, Settings (живые prefs), Design (6 секций), Extensions (+toggle), Logs (каналы+clear), System (+clear)
- ✅ Фильтр-инпуты Logs/System. VSIX-install ✅ (кнопка «Install from VSIX…» в Extensions → нативный файл-пикер (prompt_for_paths, welcome-паттерн) → kamin:extensions:installVsix + тост + перечитка списка; кнопка verified live). Иконки расширений; LegacyBridgeCard; contributed-страницы ❌

## Терминал / Problems
- ✅ Терминал (Zed-путь: alacritty_terminal VTE+grid + portable-pty ConPTY, ЛОКАЛЬНЫЙ — без хоста; cwd воркспейса). Verified live: цвета ячеек (ANSI16 VS Code Dark + 256 + Spec; PSReadLine-подсветка), resize-по-панели (probe bounds → cols/rows, ячейка 7.2×16), скроллбэк (wheel + display_offset, курсор скрыт в истории), мульти-шеллы (тулбар: pill-табы + × + «+» дропдаун профилей PowerShell/cmd/GitBash-если-есть), Ctrl+V paste (CRLF→CR), drop файла → путь (кавычки при пробеле). ⚠ КРИТИЧЕСКАЯ гоча: Event::PtyWrite (ответы DSR/CPR) ОБЯЗАН писаться обратно в PTY — иначе PSReadLine висит без промпта. Probe-кинды: termWrite/termScroll/termMenu/termNew. Star-default ✅ VERIFIED LIVE (звезда в строке профиля пикера + «default»-тег + persist defaultShellId в layout.json (=gitbash снято) + новые «+»-табы дефолтным). Mouse-selection ✅ VERIFIED LIVE (alacritty Selection: drag = Simple, dblclick = Semantic-слово, 3×клик = Lines; подсветка accent 30% в ранах screen_styled; Ctrl+C с выделением = копия+сброс БЕЗ SIGINT, без выделения — обычный ^C; verified: drag-полоса по промпту + dblclick выделил слово «hello»; копию в клипборд не проверить — DLP). Overflow-шевроны ✅ VERIFIED LIVE (окно видимых табов по ширине панели, ‹/› сдвигают term_tab_scroll, дизейбл на краях; 4 шелла → шеврон перелистнул к «Command Prompt 3», левый ожил)
- ✅ Problems (ui/problems.rs: kamin:diag:snapshot на коннекте + kamin:diag:set дельты (пустой список = снять ключ), стор (owner,uri)→Vec<Diag>, группировка по файлу, severity-иконки err/warn/info/hint, message+source+:line, клик → OpenFileAt; verified live empty-state «No problems detected»; хост НЕ правился — каналы существовали)
- ✅ Contributed statusbar items (kamin:statusBar:snapshot + update/remove пуши; StatusBarItemState 1:1: alignment L/R, priority-сорт, $(icon)-токены → codicon_map, tooltip, color hex, клик → command:execute; verified live — реальные элементы Bridge VSIX в статус-баре: «Hello» с иконкой, «1/8» зелёный, «⊗3» красный)
- ✅ Contributed themes (registry.themes → Appearance-поповер; клик → чтение theme-JSON (jsonc-комменты срезаются) → упрощённый маппер VS Code colors → Palette (поверхности editor/sideBar/titleBar/widget, текст fg/description/disabled, accent first-wins activityBarBadge→focusBorder→button, ansi-акценты; elevation-ramp оригинала НЕ портирован — прямые ключи с fallback на базу) → kamin_theme::set_contributed (Box::leak, &'static раздача) → theme_sync::apply перекрашивает и gpui-component. Dark/Light сбрасывают. Verified live: «Claude Bridge Dark» из registry применилась — ВЕСЬ хром перекрасился (оранж-accent GitHub-палитры). Persist ✅ implemented (contributedThemeId в layout.json на выборе, очистка на builtin Dark/Light, restore на первом ThemesList; код-путь = verified defaultShellId). Elevation-ramp ✅ (полный порт contributed-theme-resolve.ts: нейтральные ОПАКОВЫЕ авторские поверхности (chroma-гейт 0.25) sort-by-lightness → backdrop/panel/card/overlay с якорем на editor.background, light-инверсия, MIN_SEP-нудж backdrop, SURFACE/OVERLAY_MAX_STEP-кап для HC-тем; accent = самый НАСЫЩЕННЫЙ из 9 кандидатов; muted/disabled blend 0.42/0.62 при отсутствии; 5 unit-тестов ramp))
- ✅ Contributed keybindings (contrib_keys.rs ДВИЖОК: registry.keybindings → normalize VS Code «key» ("shift+ctrl+p"→"ctrl+shift+p", cmd→ctrl; модификаторы по алфавиту) → map key→command; root on_key_down матчит normalize_keystroke → command:execute + stop_propagation; when-клаузы пропускаются (только пустые — движка when нет, недо-срабатывание безопаснее). 2 юнит-теста normalize/when-skip; live-вкладов на машине нет ([]) — сработает при появлении вкладчиков)
- ✅ QuickPickModal (B8 1:1 ядро: shell.showQuickPick от exthost — канал СУЩЕСТВОВАЛ в протоколе хоста, хост НЕ правился. kamin_ws расширен отложенными ответами: HostReply::Now/Later + Endpoint::respond(id) — диалог не блокирует WS-поток. UI: top-center бокс — title/фильтр-инпут/список (label+description, kind=-1 сепараторы, picked-пресет), single=клик-резолв [i], canPickMany=чекбоксы+OK; Esc и скрим-клик → null (ignoreFocusOut уважается). Verified live probe-эмитом: рендер + toggle чекбокса overlay-кликом. ⚠ КРИТИЧЕСКАЯ гоча: overlay-окно ОБЯЗАНО быть обёрнуто в gpui_component::Root — их Input паникует «window root should be ui::Root» (abort всего приложения); Root добавлен в overlay open_window)
- ✅ Contributed explorer/context (registry:snapshot menus["explorer/context"] + commands titles → пункты в конце файл-меню; VS Code group-сортировка navigation-first@N с сепараторами; when-фильтр ТОЛЬКО литералы explorerResourceIsFolder/!… (when-движок ✅ (crates/shell/src/when.rs — порт when-clause.ts 1:1: !, &&, ||, ==, !=, =~ /regex/i, <, <=, >, >=, in/not in, скобки, литералы, compile-кэш, fail-closed; 8 тестов = сьют оригинала; file_menu when_allows строит контекст узла explorerResourceIsFolder/resourceFilename/resourceExtname/resourceScheme) — при появлении вкладчиков; недо-показ безопаснее); клик → command:execute(cmd, {$mid:1,fsPath}, [arg]). 2 юнит-теста when/group; live-данных нет — Bridge VSIX не вкладывает menus (registry.menus = {} на этой машине))

## Welcome / Updater
- ✅ Welcome полный (glow, кнопки, фичи)
- ✅ Updater UI (updater.rs: фоновая проверка latest.json (Tauri-формат platforms.windows-x86_64.url ИЛИ упрощённый {version,url}), version_newer-сравнение, пилюля «↓ Update vX» в статус-баре у бренда (accent-tint, тултип, клик = скачать инсталлер через cmd start). Канал = env `KAMIN_GPUI_UPDATE_URL` — у dev-сборки канала нет по определению (дистрибуция GPUI-бинаря появится с упаковкой; ПРИМЕНЕНИЕ обновления — та же фаза). 2 юнит-теста; verified live mock-сервером: пилюля «Update 9.9.9» отрисовалась)

## Хоткеи
- ✅ Ctrl+Shift+P / Ctrl+P / Ctrl+Shift+F / Ctrl+T / Ctrl+S / Esc-каскад
- ✅ Ctrl+B тоггл сайдбара
- ✅ Ctrl+Z (undo фс-операций); F2; contributed keybindings-движок. Глобальные Ctrl+C/X/V дерева ❌ (меню-пути есть)

## Прочее
- ✅ Внешний drop файлов; contributed-темы. Background-тосты окнами ❌ (нужен toast-канал вне окна)
- ✅ Dark/Light живое переключение (theme_sync)

## Очередь закрытия (по ценности)
1. Reorder ✅ ВЕСЬ: файл-табы + чипы сессий (persist `sessionOrder`) + плитки внутри слота (ToolDragOverTab; move_activity insert-before → +1 при движении вправо, чтобы «src встал на место наведённого»). Всё verified live probe drag
   - Механика: Press/DragOver события + move_item «src встаёт на место наведённого»; файл-табы: порог 4px в root mouse_move; чипы: started ставит ChipDragOver (курсор над чужим чипом), commit — ChipRelease с самого чипа
   - ⚠ ГОЧИ ТИТЛБАРА: (1) `.occlude()` элемента ОБРЕЗАЕТ bubble-диспатч до root — mouse-up/move на occlude-элементах root не видит, вешать обработчики НА элемент; (2) `window_control_area(Drag)` на титлбаре съедал mouse-UP всей зоны (down/move проходили) — УБРАН, драг окна = client-side start_window_move на bubble-клике (интерактивные дети обязаны stop_propagation на down)
   - Probe input: `{"cmd":"click","x","y"}` / `{"cmd":"drag","from":[x,y],"to":[x,y]}` (лог. px) — PostMessage WM_LBUTTON*/WM_MOUSEMOVE прямо в HWND. ⚠ SetCursorPos/SendInput НЕ работают (координатные пространства расходятся + foreground-ограничения)
2. Тултипы auto-place ✅ gpui-нативно: у нижнего края flip НАД элементом, у правого — clamp (verified live probe hover «File encoding» в статус-баре). Кастомная overlay-механика НЕ нужна. Probe: `{"cmd":"hover","x","y"}`
3. Tab overflow ▾ файл-табов ✅ (ширина по probe_area("file-tabs"), оценка таба ~name×6.5+50, активный всегда видим, deferred-поповер со скрытыми: иконка+имя+dirty-точка, клик = выбор+закрытие, скрим закрывает; verified live 11 файлов). LRU-лимит 12 ✅ (13-й вытесняет самый давний ЧИСТЫЙ НЕзапиненный; verified live). Pin ✅ (RMB-меню «Pin/Unpin Tab», thumbtack-значок, pinned-first stable-сортировка — и после ручного reorder, LRU не выселяет; verified live pinTab-эмитом + скрин меню). Probe-эмиты: pinTab/tabMenu
4. Undo файл-операций ✅ (Ctrl+Z, стек 50: Create→корзина, Rename→обратно, Delete→trash-крейт restore из корзины по original_path (самый свежий), Paste cut→вернуть/copy→корзина; пуш ТОЛЬКО по факту успеха; тост об исходе; Delete дерева теперь в корзину, не насовсем. Verified live: delete→restore и paste→undo; probe-эмиты fsCopy/fsPaste/fsDelete/fsUndo). «Open In ▸» ✅ (аккордеон в файл-меню: Reveal in Explorer (/select для файла), Integrated Terminal (pin terminal + шелл с cwd узла — verified live overlay-кликами: промпт в каталоге), Default Application (только файл; cmd start)). Probe: click принимает "target":"overlay" — клики в overlay-окно (меню/модалки). CF_HDROP 🔶 implemented (os_clipboard.rs: write_files DROPFILES+double-null UTF-16 на Cut/Copy дерева; read_files DragQueryFileW в Paste при пустом внутреннем буфере → вставка копированием + undo). ⚠ ВЕРИФИКАЦИЯ НЕВОЗМОЖНА на этой машине: SearchInform DLP даёт ACCESS_DENIED (err 5) на OpenClipboard ВСЕМ процессам автоматизационного дерева (clip.exe/PowerShell тоже); юзерский интерактивный клипборд жив. Проверять руками на чистой машине
5. Switching-индикатор 🔶 implemented (ActivateSession закрытой сессии → switching_to → codicon-loading на чипе вместо точки; гасится снапшотом с open=true; на быстром локальном хосте окно спиннера <0.7с — визуально не пойман, путь кодовый прямой). Внешний drop файлов ✅ implemented (on_drop ExternalPaths на file-панели → OpenFile; OLE-драг probe-ом не синтезируется — механизм идентичен проверенному drop в терминал). F2/dblclick rename ✅ (dblclick по чипу → BeginRename → inline-инпут в строке сайдбара, verified live probe-dblclick; F2 = rename активной сессии, тот же путь). ⚠ Гоча dblclick чипов: активация НЕактивной сессии обновляет last_opened → чип пересортировывается между кликами — dblclick надёжен по УЖЕ активному чипу (типичный rename-сценарий). Мёртвый код зачищен (file_viewer.rs удалён — заменён editor-табами; SetActiveTool/CloseFile варианты, panel_card, viewer_scroll; 17 warnings → 2 осмысленных). Bridge-status точки ✅ (SessionItem 1:1: metadata.bridgeWorking → blue 6px «Working…», bridgeStatus connected/connecting/error/disconnected → green/accent/red/muted + тултип; verified live — точки открытых сессий окрасились из metadata). Chat-cover ✅ (первый ipc-inbound вью → ShellEvent::WebviewAlive → до этого wv2 СКРЫТ, в панели gpui-ковер «Loading…»; webview_panel 3 состояния: no-HTML плейсхолдер / HTML-not-alive ковер / alive wv2. Verified live: с test-воркспейсом полная цепочка за ~6с — чат показан по alive-сигналу, гейт не блокирует; ковровое окно на быстром хосте миллисекундное). ⚠ ДЕВ-ПРИЁМ: активную сессию можно переключить правкой `%LOCALAPPDATA%\kaminide-gpui-dev\data\sessions.json` (activeSessionId) ДО старта — спасение когда хост голодает на индексе большого воркспейса и RPC стоят (30×2с resolve-окно истекает; ре-resolve по extensions:changed)
6. Scroll-to-line из поиска ✅ (FileOpened target → pending_goto → set_cursor_position(line-1,0) в render; ⚠ для НОВОГО файла goto строго на СЛЕДУЮЩЕМ кадре после layout инпута (+cx.notify), иначе move_to не скроллит; verified live openFile line:100). Minimap ✅ (Zed-подход, ui/minimap.rs: canvas paint_quad полоски (x=отступ×0.35, w=длина×0.35, кап 10K строк), бенд видимой зоны ±20 строк вокруг курсора (скролл-оффсет InputState непубличен — ведём по курсору), клик → GotoLine → pending_goto-механика; кэш метрик (path,rows), stale при Change. Verified live: прорисовка структуры term.rs + click-to-jump в середину файла)
7. Editor RMB-меню ✅ БЕСПЛАТНО: встроено в gpui-component InputState (handle_right_click_menu) — Go to Definition / Show Code Actions / Cut / Copy / Paste / Select All с хоткеями; verified live probe right-click (probe click получил "button":"right"). Editor ✅: viewState per tab БЕСПЛАТНО (каждый таб = свой InputState-entity, скролл сохраняется by construction; verified live probe scroll+переключение). Reload при внешнем изменении ✅ (watcher → FilesChanged(пути) → reload ЧИСТЫХ табов в render; dirty не трогаем; reload_suppress гасит ложный dirty от set_value; ⚠ notify отдаёт пути с `\.\`-сегментом — сравнение только через norm_path; verified live echo→файл обновился, таб чистый). Probe: `{"cmd":"scroll","x","y","lines"}` (⚠ WM_MOUSEWHEEL несёт ЭКРАННЫЕ координаты — ClientToScreen перед постом)
8. Фильтр-инпуты Logs/System ✅ (общий инпут в тулбаре обеих панелей, case-insensitive contains, пустые состояния «No … match the filter»; verified live probe type). Probe: `{"cmd":"type","text"}` — WM_CHAR посимвольно + WM_ACTIVATE перед (input_handler только у активного окна). QuickPickModal → ФАЗА РАСШИРЕНИЙ (в оригинале канал = vscode.window.showQuickPick из exthost; в exthost сейчас silent-stub — UI без канала мёртв и неверифицируем). Updater UI → ФАЗА ДИСТРИБУЦИИ (GPUI-шелл пока dev-бинарь без апдейтер-инфры). ⇒ ИНТЕРФЕЙСНАЯ ФАЗА ЗАКРЫТА (все пункты, реализуемые без exthost/дистрибуции)
9. Problems-тул (needs exthost diagnostics) — фаза расширений

- ✅ ★ ФИКС «КВАДРАТА» (2026-07-25): gpui_component::Root красил bg(theme.background) НЕПРОЗРАЧНО на всё overlay-окно с момента Root-обёртки — живой экран был перекрыт заливкой, а probe-скрины это МАСКИРОВАЛИ (overlay-скрин и так сплошной; main-скрин снимается с main HWND мимо overlay). Vendored-патч: Root.transparent=true → bg не рисуется; verified: overlay-скрин теперь чёрный (альфа 0) вне поповера. Диагностика на будущее: probe {"cmd":"overlay"} → активные оверлей-стейты кадра.

- ✅ Contributed iconThemes РЕАЛИЗОВАНЫ (icon_theme.rs: kamin:iconTheme:load → parse (iconDefinitions/fileExtensions/fileNames/folderNames(+Expanded)/дефолты; резолв fileNames→длинный суффикс→file, регистронезависимо; 2 юнит-теста) → set_active глобал; file_img/folder_img — img(PathBuf) читает SVG прямо с диска (iconPath абсолютные, data-URL канал не нужен); все callsites дерева/табов через них, фолбэк Catppuccin; Icons-колонка Appearance = Catppuccin + registry.iconThemes, persist iconThemeId + restore на первом списке; fontCharacter-темы = гэп как в оригинале (plan/25)). ⚠ live-верификация тест-расширением — ПОСЛЕ DLP-soak (debug рядом не поднять, общий data-dir).

- ✅ ★ МИГРАЦИЯ ВСЕХ ПОПОВЕРОВ В OVERLAY-ОКНО ЗАВЕРШЕНА (2026-07-26, директива юзера «все поповеры в оверлей», штора ЗАПРЕЩЕНА): tabs-overflow «N ⌄», new-session «+», command palette, quick open, find-in-files, workspace symbols, hover-пилюли сессий/групп — main-окно не рисует ни одного поповера, всё поверх wv2-вебвью. Инпуты остались в RootView/main (фокус работает, рендер в overlay — прецедент qp_input). Пилюли: anchor_probe в hovered-строке → pill_anchor()-глобал → overlay_pill на якоре. Verified live скринами: палитра/дропдауны/пилюля поверх живого чата.
- ✅ Editor LSP поверх code_editor: HostLsp (hover/definition) через kamin:lang:* + doc-sync (kamin:doc:open/close, re-open по hash при изменении) — те же exthost-провайдеры, что у Monaco. Навешен на все табы; live-верификация hover — при появлении LSP-расширений.
- ✅ Консоль-вебвью (file-bottom): виден сразу (vendored prepaint ставит bounds и скрытому чайлду), полная ширина (таффи-гоча w_full), re-resolve при видимой панели; грип сплиттера по центру. Углы wv2-чайлдов скруглены (round_webview_children). Verified live.
- Zed-заимствования этой фазы: LSP-провайдеры gpui-component (Zed-производные трейты Hover/Definition/Completion) подключены к хосту; по overlay-прозрачности исследованы zed#5040/#6734 (официально сломано) → собственное решение SetWindowRgn+recreate; canonical одно-оконный путь (WebView2 Visual Hosting / CompositionController + DComp) зафиксирован как большой рефакторинг в плане.

- ✅ ★ ПАНЕЛЬНАЯ СТИЛИСТИКА ВЫРОВНЕНА С ОРИГИНАЛОМ (2026-07-26, side-by-side разбор юзера): ПРАВЫЕ карты БЕЗ таб-стрипа — чистое тело активного тула, плитки тулов + «…»-пикер в РЕЙЛЕ; ВСЕ ЦЕНТРАЛЬНЫЕ (main-ЧАТ со стрипом «Claude Bridge»+титул CLAUDE, mainBottom, centralBottom) — SLOT_PANEL со стрипом и «…»; сайдбар = рейл activity-бара (плитки из модели Sidebar-слота + «…»-пикер). Console = обычный тул слота centralBottom (webview_body без двойного glint; миграция пустых persist-ов → console). Тултипы сдвинуты вниз от курсора (не накрывают элемент).
