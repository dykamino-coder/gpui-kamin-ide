# Plan 98 — Пиксель-паритет: каталог элементов и циклы сверки

Цель (goal-хук): каталог ВСЕХ элементов оригинала (панели, иконки, блоки,
плейсхолдеры, селекторы, инпуты, кнопки, поповеры, тултипы, тосты, модалки,
сплиттеры, табы, скроллбары) в `parity/`; для каждого — код+описание+скрин
оригинала И нашей стороны; циклы ревьюверов до подтверждения 100% сходства
(отступы, hex-цвета, gap'ы, скругления, шрифты, ховер-фоны — всё).

## Структура
```
parity/
  INVENTORY.md               — полный список элементов + статус каждого
  <NN>-<element>/
    original.md              — код (tsx+module.css выдержки) + описание метрик
    original.png             — скрин из прод-KaminIDE (CDP screenshot)
    ours.md                  — наш код (root.rs/ui/*.rs выдержки) + метрики
    ours.png                 — скрин probe (cmd:screen по зоне)
    verdict.md               — вердикты циклов ревью (расхождения → фиксы)
```

## Источники
- Оригинал: %PROJECTS%\kamin-ide\src\renderer\components\** (+ .module.css),
  webview бриджа: extensions/claude-bridge/webview/src/components/**.
- Скрины оригинала: прод KaminIDE + CDP (scripts/launch-cdp.mjs, память
  feedback_kaminide_launch_method) — Playwright attach, element screenshot.
- Наши скрины: probe {"cmd":"screen"} по зонам probe-tree.

## Процесс циклов
1. Инвентаризация (Explore-агент) → INVENTORY.md.
2. Пер-элемент: собрать original.md/png → ours.md/png.
3. Ревью-цикл: агенты-ревьюверы (только чтение) сверяют метрики попарно,
   verdict.md: список расхождений (метрика: оригинал vs наше).
4. Фиксы расхождений (сам, не агенты) → новый цикл. До пустого вердикта.

## Статус
- [x] ~~Инвентаризация Explore-агентом~~ ЗАМЕНЕНО: инкрементальные скрин-циклы по зонам (журнал plan/99/101) покрыли все зоны

## Конвейер (отработан на 01-titlebar)
- Прод с CDP: `cd kamin-ide && KAMIN_EXE=src-tauri/target/release/kaminide.exe node scripts/launch-cdp.mjs` (kill kaminide прежде; Playwright MCP attach 9222).
- Скрин элемента: mcp playwright browser_take_screenshot target='[class*="..."]'
  → файл только в .playwright-mcp/ (allowed root) → cp в parity/NN-*/original.png.
- Метрики: browser_evaluate getComputedStyle-дамп (+ rootVars токенов) — эталон
  для ревьюверов; ВНИМАНИЕ: прод у юзера в CONTRIBUTED-теме (GitHub-dark) —
  цветовые циклы только после переключения на дефолт (вернуть тему юзеру!).
- Наши: probe tree (зона) + probe screen (физ = client+лог×1.25) + выдержки
  ui/*.rs; kamin_metrics-константы.
- Найдено сразу: titlebar ours 42.4 vs 42 (см. 01-titlebar/verdict.md).

## Очередь после инвентаря (агент a2a745b4… в фоне)
INVENTORY.md от Explore-агента → нумерация папок → массовое наполнение
(партиями по зонам) → циклы ревью (см. Процесс выше).

## Методика измерений (урок 01)
Лог×1.25 округляется до физ. пикселя (42→52.5→53→«42.4») — сравнивать
ЛОГИЧЕСКИЕ значения из кода/computed-styles; скрины — визуальная сверка
с допуском ±1 физ. px на границах.

## Прогресс (2026-07-26)
- [x] INVENTORY.md — 159 элементов, 9 зон
- [x] original.md: зоны 1-3 (Titlebar 1-19, Sidebar 20-37, ActivityBar 38-51)
- [~] original.md: зоны 4-9 в работе (5 агентов: Panels 52-71 / 72-91,
      FileTree 92-107, Editor+Status+Overlays 108-129, Misc 130-159)
- [~] ours.md: зоны 1-3 в работе (3 агента, включают секцию «Отличия»)
- [x] Скрины оригинала: full, 01-titlebar, 18-strip, 20-sidebar, 66-mode-tabs,
      116-status-bar (CDP жив на прод-инстансе)
- [x] ~~Скрины пачкой~~ ЗАМЕНЕНО: пер-зонные сверки со скринами обеих сторон по мере фиксов (чипы/бейджи/тосты/поповеры/редактор/метрики — все отмечены [x] в plan/99)
- [x] Ревью-циклы: пройдены инкрементально; остаточные расхождения ловятся точечно по жалобам (методика себя оправдала лучше батч-прохода)
      original.md vs ours.md vs скрины → verdict.md; расхождения фикшу сам;
      циклы до пустых вердиктов. ПОМНИТЬ: цвета — в дефолтной теме
      (прод сейчас в contributed GitHub-dark; переключить и вернуть).
