# 95 — Мастер-чеклист 100% паритета

Правило: пункт закрыт, когда (а) покрыт планом, (б) при имплементации — работает идентично kamin-ide 0.2.87. Сейчас трекается покрытие ПЛАНОМ. Детальные чеклисты — в конце каждого дока; здесь свод.

## Свод по областям
| Область | Док | Покрытие планом |
|---|---|---|
| Архитектура/процессы | 00 | ✅ |
| Окно/титлбар/шелл-команды/тосты/диагностика | 10 | ✅ |
| Тема/палитры/шрифты/иконки/анимации | 20 | ✅ |
| 100% трансляция VS Code-тем + иконко-тем + product icons | 25 | ✅ |
| Layout/сплиттеры/активити-слоты/персист | 30 | ✅ |
| Все компоненты renderer | 40 | ✅ |
| Каждый экран/панель/кнопка + как повторить | 45 | ✅ |
| State/RPC/события/window.kamin/персист | 50 | ✅ |
| Exthost/vscode.*/LSP/PTY/VSIX | 60 | ✅ |
| 100% VS Code API (гэп-инвентарь + 6 волн дорожной карты) | 65 | ✅ |
| Встроенные команды workbench/editor/vscode (реализованные + гэпы + keybindings) | 66 | ✅ |
| Вебвью (протокол/шим/оверлеи/шторы/вотчдог/браузер) | 70 | ✅ |
| Bridge UX (вклады/чат/мост/чек-скрипт) | 80 | ✅ |
| Сборка/апдейтер/дистрибуция | 90 | ✅ |
| Решения по всем открытым вопросам | 98 | ✅ (все 5 закрыты) |

## Открытые вопросы — ВСЕ ЗАКРЫТЫ (решения в plan/98-decisions.md)
1. ✅ Editor: gpui-component как база (есть search/lsp/popovers/tree-sitter); гэп-лист и порядок закрытия зафиксированы; фолбэк = форк редактора, НЕ Monaco.
2. ✅ DevTools-кнопка: девтулзы активного wry-вебвью, иначе SystemLogPanel.
3. ✅ Апдейтер: tauri-updater-совместимый протокол клиентом; сервер и подпись не меняются.
4. ✅ Контент-протокол вебвью: loopback-HTTP kamin-host (/__webview + /__resource), перенос из webview.rs в Node; setHtml взять из cef-migration 642da7d.
5. ✅ listFiles-гейт активации: bound (глубина/таймаут ~2s) + onStartupFinished не ждёт полного listFiles; чинится в kamin-host, полезно и текущему Tauri.

## Лог сверок (план ↔ код)
- 2026-07-24 v1: инвентаризация 5 областей (5 параллельных разведчиков, ~800K токенов анализа кода), документы 00–90 написаны.
- 2026-07-24 сверка №1 (2 независимых верификатора против кода kamin-ide 0.2.87):
  - UI-верификатор: 28/28 Rust-команд покрыты, порядок титлбара/статус-бар/все константы/hex-цвета/z-шкала/градиенты — точны. 3 находки (ratio-клампы File-панели; файлы-хелперы DesignPanel/file-tree; атрибуция оконных команд) — ИСПРАВЛЕНЫ в 30/40/10.
  - Платформа-верификатор: все RPC-методы 1:1, все host-события, все 60 signals-файлов, view-ids Бриджа, session-поля, бэкофф, апдейтер-эндпоинт — точны. 7 находок (search_in_files = первичный путь Find-in-Files; heap_flush; toast_get/action; тема claude-bridge-dark; diag-слой в 00; rightPanelSplit НЕ персистится фактически; Bridge-конфиг вложен под config) — ИСПРАВЛЕНЫ в 50/80/00.
  - Итог: все найденные расхождения устранены; обе половины «everything else checked out». Покрытие планом = 100% по результатам независимой сверки.
- 2026-07-24 закрытие блокеров старта (по запросу): все 5 открытых вопросов решены (plan/98); написан plan/45 — КАЖДЫЙ экран/панель/кнопка хост-UI + чат-вебвью Бриджа (по отчёту разведчика: header/composer/dropdowns/widgets/agents/jsonl/tools/10 customize-страниц) с label→поведение→bridge.*-вызов→как повторить. Блокеров имплементации нет.
- 2026-07-24 расширение плана (по запросу): +25 (100% трансляция VS Code-тем/иконок — вкл. fontCharacter-гэп, полный словарь workbench-ключей, tokenColors/semantic, product icons) и +65 (100% VS Code API: полный d.ts-инвентарь — реально функционально ~48%, стабы ~40%, отсутствует ~12%; 29 классов и 12 енумов MISSING; 6 волн закрытия: терминал → tasks/inline-completions → debug/DAP → scm/auth/uri → notebooks/tests/custom-editors → chat/lm/mcp; замер через generate-locked-spec + corpus:compat). Примечание инвентаря: research/vscode-extension-api/15-locked-spec.md в репо ОТСУТСТВУЕТ (есть только генератор scripts/generate-locked-spec.ts) — память reference_vscode_research_folder устарела.

- 2026-07-24 сверка №2 (сплошная, 2 верификатора против всего плана 16 доков + код):
  - UI/визуал/метрики: числа точны (30+ значений, ноль ошибок), но plan/23 покрывал 34/60 CSS-модулей → **ДОСНЯТЫ все 60** (батч 2: terminal/problems/main-подпанели/skeletons/extensions/file-viewer/browser/settings/tree/context-menu). Orphan-дубль 45-screens-buttons.md УДАЛЁН. Skeleton-blur/TreeIcon-filter/спиннер добавлены в plan/24.
  - Платформа/API/команды: **0 пропущенных** (все RPC/события/host-request/Rust-команды/60 signals/built-in id/claude-bridge вклады на месте). 3 неточности исправлены: shell-порядок (powershell→pwsh→cmd→git-bash), lang 21→22 методов, /__webview роут — НЕ на main (на cef-migration, забрать) → plan/00,50,60,98. 2 косметики (EVT_READY payload +extensions, updater-плейсхолдер {{current_version}}) → plan/00,90.
  - Итог: все находки устранены; независимая сплошная сверка подтверждает — главный план и побочные ПОЛНЫ (nothing overlooked), покрытие 100%.

## Процедура следующей сверки
1. Разведчик на область читает док + соответствующий код, репортит: (а) что в коде есть, а в плане нет; (б) что в плане неточно.
2. Гэпы вносятся в док, лог пополняется.
3. Повторять до пустого репорта по каждой области.


## Имплементационный свод (2026-07-25, из 97-parity-status)
Детальный статус по каждому пункту — plan/97-parity-status.md (главный трекер). Компакт:

**✅ Реализовано + verified live:** титлбар (лого/табы/поповеры/搜索-пилюля/layout-тумблеры/appearance/new-session), сайдбар сессий 1:1 (группы/пины/цвета/rename/hover-пилюли/контекст-меню/bridge-точки), activity-бар + панельная система (5 слотов, pinned/active-модель, persist, tool-picker, DnD плиток ПОЛНЫЙ: стрип-табы+рейлы, вставка по индексу, RMB Hide/Move-to), дерево файлов (Catppuccin, ленивые листинги, Show-N-more, мультиселект+операции, DnD файлов, undo/корзина, file-decorations c бейджами, контекст-меню builtin+contributed с полным when-движком, inline-rename, CF_HDROP-код), редактор (code_editor 9 языков, мульти-табы LRU-12+pin+overflow, dirty/Save, RMB-меню, viewState, external reload, scroll-to-line, minimap, sticky-scroll (vendored-патч), soft_wrap off), терминал (ANSI-цвета, скроллбэк, мульти-шеллы, star-default, overflow-шевроны, mouse-selection+Ctrl+C-copy, drop путей, Ctrl+V), поиск (Ctrl+P/Ctrl+Shift+F/Ctrl+T/Ctrl+Shift+P), оверлеи (модалки/prompt/QuickPick/тосты+actions/меню/тултипы; единое overlay-окно), статус-бар (счётчики+contributed+update-пилюля+EOL), Customize (Settings/Design/Extensions/Logs/System + 10 contributed-страниц Bridge через czShared), темы (Dark/Light + contributed c полным elevation-ramp + persist), keybindings contributed, вебвью-мост (чат/plan/console: shim+theme+ipc+deliver+alive-ковер), Problems/diags, welcome, layout (сплиттеры/persist/пресеты save-apply-delete-default-rename-export-import), updater-клиент, probe-инфраструктура (клики/драги/type/скриншоты/эмиты).

**❌ Внешне-средовое (код готов, среда не позволяет):** CF_HDROP-чтение и клипборд-копия (DLP душит клипборд автоматизационного дерева — руками/чистая машина), DLP-сутки release-бинаря (task #45 — рабочая машина).

**Гэп-технологии:** LSP-фичи поверх встроенного tree-sitter code_editor (форк-канал vendor/gpui-component открыт).
