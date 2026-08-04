# 10 — Окно, титлбар, шелл-возможности

Источники: `src-tauri/` (lib.rs, tauri.conf.json, capabilities/), `components/titlebar/*`, `tauri-bridge.ts`.

## Окно
- 1400×900, min 800×600; **decorations:false** (frameless — титлбар рисуем сами; в GPUI нативно из коробки)
- backgroundColor #1d1d28; **создаётся hidden**, показывается после первого пейнта (revealWindow, 2 rAF) — никакого флеша при запуске
- Геометрия персистится: SIZE | POSITION | MAXIMIZED (VISIBLE намеренно исключён — не ломать hidden-until-painted)
- dragDropEnabled (нативный drag-in), drag-out файлов (аналог tauri-plugin-drag)
- **mimalloc как глобальный аллокатор** (уводит аллокации с дефолтной кучи процесса — иммунитет к DLP/AV heap-хукам). Release: lto=fat, opt-level=s, panic=abort, codegen-units=1, strip
- Single-instance: второй запуск форвардит argv, фокусит/разворачивает окно, эмитит open-folder (см. 90-packaging)

## Титлбар (42px, слева направо — точный порядок)
Фон = drag-region (пустые области таскают окно).
1. **Левый кластер** (ширина = ширине сайдбара, когда он виден):
   - Лого kaminoid.svg (не draggable)
   - Toggle сайдбара (PanelIcon slot="left"; tooltip Hide/Show sidebar)
   - Шестерёнка Customize (fas fa-gear) — ТОЛЬКО когда сайдбар скрыт; toggle openCustomize("settings")
2. **Слот табов** (flex 1): SessionTabs — чипы открытых сессий, drag-reorder, «+» пикер (folder / no-folder); пустая зона = drag-region
3. **Кнопка команд-поиска**: codicon-search + «Type a command…»; запускает workbench.action.showCommands; tooltip «Open command palette (Ctrl+Shift+P)»
4. **LayoutToggles**: fas fa-table-columns → дропдаун-портал: чекбоксы 6 регионов (child-строки disabled без родителя) + секция пресетов (save/apply/export/import/overwrite/star-default/delete; export через нативный save-диалог)
5. **ThemeQuickToggle**: иконка по активной теме (fa-sun/fa-moon/fa-circle-half-stroke) → Appearance-поповер: 3 колонки Dark/Light/Icons (встроенные + contributed темы/icon-темы) + System
6. **Контролы окна** (TitlebarButton, 36px):
   - DevTools: fas fa-bug + подпись «DevTools» (variant devtools). В GPUI: девтулзы главного окна нет (нет webview) — кнопка открывает девтулзы АКТИВНОГО wry-вебвью + панель системного лога; поведение уточнить при имплементации
   - Minimize: codicon chrome-minimize
   - Maximize/Restore: codicon chrome-maximize/chrome-restore (isWindowMaximized синхронизируется по событию resize окна)
   - Close: codicon chrome-close (variant close — красный hover)

## Нативные способности шелла (Rust-команды → GPUI-функции)
Оконные операции (minimize/toggle-maximize/close/is-maximized/is-minimized/start-dragging/theme/show) — сейчас ВСТРОЕННЫЙ window-плагин Tauri (вызовы из Titlebar.tsx), НЕ lib.rs; в GPUI — прямые методы окна. Кастомная поверхность lib.rs invoke_handler (28 команд):
- toggle_devtools
- get_host_endpoint (→ {port,token} от сайдкара)
- layout_get / layout_set (shallow-merge в <app_data>/layout.json, atomic temp+rename, write-lock на бут-бёрст)
- webview_set_html(id,html,roots) + async-протокол контента вебвью (см. 70-webviews)
- secret_encrypt / secret_decrypt (DPAPI, CRYPTPROTECT_UI_FORBIDDEN, hex)
- browser_set_bounds/hide/navigate/back/forward/reload + событие browser://navigated (embedded browser — wry child; hide = парковка в -32000; navigate нормализует URL/host/DuckDuckGo-поиск; default DuckDuckGo)
- search_in_files (ripgrep `ignore`-walker: gitignore-aware, skip binary/.git, case-insensitive substring, MAX_HITS 200, файлы ≤1MB)
- clipboard_write_text / clipboard_read_text (arboard — обход «document not focused»)
- updater_check / updater_install (см. 90-packaging)
- show_external_toast / toast_get / toast_action / set_toast_palette (см. ниже)
- detect_electron_bridge / uninstall_electron_bridge (legacy-очистка)
- diag_heartbeat / diag_pong / diag_visibility / heap_flush (freeze-диагностика, см. ниже)
- Нативные open/save диалоги

## Внешние тосты (out-of-app окна)
Frameless transparent always-on-top skip-taskbar окошки, стек снизу-справа активного монитора (work-area). Rust владеет очередью, overflow-бейджем, авто-дисмиссом (8s, пауза на hover), позиционированием. show_external_toast ждёт выбранное действие (oneshot). Палитра пушится из темы приложения (set_toast_palette). В GPUI — отдельные GPUI-окна (проще, чем webview-окна).

## Freeze-диагностика (сохранить всю обвязку)
- freeze_watchdog: Rust ПРОДИТ рендерер каждые 500ms → renderer отвечает diag_pong(seq) (unthrottleable) + diag_heartbeat (throttleable) + diag_visibility. Вердикт: pongs+beats встали = REAL FREEZE (порог 3000ms); только beats = BACKGROUNDED. Breadcrumbs + openMark. Win32: IsHungAppWindow/GetLastInputInfo/CPU%/working-set по дереву процессов
- native_stack: Suspend→GetThreadContext→Resume сэмплинг тредов (в GPUI — сэмплить wry-вебвью и свои треды)
- webview_watchdog: подписка WebView2 ProcessFailed → классификация KIND/REASON → kamin:webview:process-failed (in-place recovery iframe'а) — для wry-вебвью остаётся актуальным
- heap_sampler (opt-in KAMIN_HEAP_SAMPLER=1): CDP sampling heap profiler панельных вебвью
- diag_log: единый <app_data>/kamin-diag.log ([UTC][TYPE], append + head-truncate 131KB, std::fs)
- log_reset: вайп *.log при первом запуске новой версии (маркер .log-version), до открытия вотчдогов

## Чеклист паритета (шелл/окно)
- [ ] Frameless окно 1400×900/min 800×600, hidden-until-painted, bg #1d1d28
- [ ] Геометрия SIZE|POSITION|MAXIMIZED персист
- [ ] Титлбар: все 6 кластеров, точный порядок/иконки/тултипы, drag-region
- [ ] Все нативные команды из списка
- [ ] Внешние тосты + палитра
- [ ] Freeze-диагностика (prod/pong/verdict/лог/reset) в GPUI-варианте
- [ ] mimalloc + release-профиль
- [ ] drag-out файлов, нативные диалоги, DPAPI, arboard, ripgrep-поиск
