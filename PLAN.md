# KaminIDE → GPUI: план переписывания (100% паритет)

**Цель:** главное окно KaminIDE на GPUI (нативный Rust, стек Zed) — без WebView2 в главном окне (уходит корпоративный DLP-фриз: виснет именно главный WebView2, встроенные вебвью-панели не виснут). VSIX-вебвью (чат Бриджа и пр.) — через gpui-wry (WebView2-оверлей, не фризит). Экстеншн-хост (node, kamin-host) сохраняется без изменений.

**Источник истины:** `%PROJECTS%\kamin-ide` (0.2.87, ветка main). План сверяется с ним до 100% покрытия: интерфейс, панели, расположение кнопок, ресайзы, цвета, темы — всё.

## Технический фундамент (проверено 2026-07-24)

| Слой | Выбор | Статус |
|---|---|---|
| UI-движок | `gpui` (crates.io, стек Zed) | pre-1.0, Windows поддержан (Zed на Windows) |
| Компоненты | `gpui-component` (longbridge) | 60+ компонентов: dock/resizable панели, деревья, виртуализированные таблицы, code editor (200K строк, tree-sitter, LSP), markdown/HTML рендер, модалки, табы. Прод: Longbridge Pro |
| VSIX-вебвью | `gpui-wry` (feature-flag в gpui-component) | experimental; Windows+macOS; вебвью рисуется ОВЕРЛЕЕМ поверх GPUI-окна (контент под ним перекрыт) — тот же режим, что Tauri multiwebview у нас сейчас |
| Редактор | gpui-component code editor (Tree-sitter + Rope + LSP) | замена Monaco; LSP-провайдеры уже идут из kamin-host |
| Терминал | нативный терминал-элемент (референс: Zed terminal, alacritty_terminal crate) | замена xterm.js |
| Экстеншн-хост | существующий kamin-host (node.exe + kamin-host.mjs + node_modules + builtin-extensions) | БЕЗ ИЗМЕНЕНИЙ; связь — loopback WS `ws://127.0.0.1:<port>/rpc?token=` |
| Данные Бриджа | open-claude-bridge server (podman, :3456) | без изменений |

Ключевое архитектурное свойство: рендерер уже шелл-агностичен — data plane идёт через WS к kamin-host (проверено в Chrome/Servo миграции). GPUI-приложение = новый КЛИЕНТ того же WS-протокола + новый визуальный слой.

## Структура плана (документы)

- `plan/00-architecture.md` — процессная модель, WS-клиент, потоки данных
- `plan/10-shell-window.md` — окно, кастомный титлбар, кнопки, drag, ресайз, персист позиции
- `plan/20-theme-visual.md` — палитра токенов, градиенты, радиусы, шрифты, иконки, hover, тени
- `plan/22-pixel-fidelity.md` — правило №0: требование 1:1 (почему шрифт/размер/отступ критичны) + механизм гарантии
- `plan/23-exact-metrics.md` — ТОЧНЫЕ per-component значения из CSS-модулей (padding/gap/font/позиции — дословно)
- `plan/24-tricky-visuals.md` — хитрые визуалы (glint-бордер, радиальные фоны, тени, backdrop-blur) → механизм + рецепт переноса в GPUI
- `plan/25-vscode-theme-translation.md` — 100% трансляция VS Code цветовых тем (workbench-ключи, tokenColors, semantic) + файловых иконко-тем (fontCharacter!) + product icons
- `plan/30-layout-panels.md` — регионы, сплиттеры, min/max, collapse, layout.json персист
- `plan/40-components.md` — покомпонентный инвентарь renderer → GPUI-эквивалент
- `plan/45-screens-and-buttons.md` — КАЖДЫЙ экран/панель/кнопка: label→поведение→как повторить (100%)
- `plan/50-state-ipc.md` — полный контракт host RPC + события + window.kamin → Rust-трейты
- `plan/60-exthost-vsix.md` — хостинг kamin-host, vscode.* поверхность, что трогать нельзя
- `plan/65-vscode-api-100.md` — покрытие 100% VS Code API: полный гэп-лист (kamin-ide покрывал не всё — стабы/пропуски) + дорожная карта закрытия
- `plan/66-builtin-commands.md` — встроенные команды (workbench.*/editor.*/vscode.*): что KaminIDE реализовал + гэпы + default-keybindings
- `plan/70-webviews.md` — gpui-wry: чат/консоль/план, postMessage-релей, acquireVsCodeApi, тултипы поверх
- `plan/80-bridge-ux.md` — весь UX Бриджа (композер, селекторы, очередь, агенты, elicitation, стриминг)
- `plan/90-packaging.md` — сборка, single-exe, апдейтер, context menu, инсталлер
- `plan/95-parity-checklist.md` — 100%-чеклист сверки (галочка = пункт покрыт планом)
- `plan/96-testing.md` — план тестирования: kamin-probe (замена CDP для нативного окна), unit/контракт/E2E, метрик-асерты + пиксель-дифф против золотого 0.2.87, перф- и DLP-гейты, гейты по фазам
- `plan/98-decisions.md` — решения по всем открытым вопросам (закрыты до старта)

## Правила проекта (наследуются)

- **№0 — ПИКСЕЛЬ-ТОЧНОСТЬ 1:1** (plan/22): шрифт Bricolage, каждый font-size/отступ/радиус/позиция — РОВНО как в kamin-ide, не «≈». Точные per-component значения — plan/23. Визуальный дифф-тест обязателен.
- Perf-бюджеты жёсткие: <800ms cold, <90MB idle (нативный GPUI обязан быть лучше Tauri)
- Визуал = Bridge 1:1: градиент + rounded floating panels + Bricolage + FontAwesome
- ≤250 LOC на файл, атомарные компоненты
- Reuse: kamin-host, server, протоколы — копировать рабочее, не изобретать

## Статус

- [x] Фундамент проверен (gpui / gpui-component / gpui-wry)
- [x] Инвентаризация kamin-ide (5 областей, ~800K токенов анализа)
- [x] Документы plan/00–95 заполнены
- [x] Сверка план↔код: итерация 1 (2 независимых верификатора; 10 расхождений найдено и исправлено; остальное подтверждено точным) — покрытие 100%, лог в plan/95
- [x] Расширения: 100% VS Code API (plan/65), трансляция тем/иконок (plan/25)
- [x] Все открытые вопросы закрыты (plan/98); каждый экран/кнопка + как повторить (plan/45)
- [x] План тестирования (plan/96): техническое + визуальное, гейты по фазам
- [ ] Старт имплементации (блокеров нет; первые гейты — kamin-probe, шрифтовый гейт, DLP-сутки голого окна)
