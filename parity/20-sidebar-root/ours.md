# 20 sidebar-root — наша реализация
Файлы: `crates\shell\src\root.rs:5245-5296` (обвязка), `crates\shell\src\root.rs:2561-2587` (gap_wrap), `crates\shell\src\root.rs:2925,2944-2951` (drag-кламп), `crates\metrics\src\lib.rs:59-61`

## Структура (gpui-дерево кратко)
```
body (flex row, pl BODY_GUTTER_X=4)
└─ .when(sidebar_visible)
   └─ div .relative .w(sidebar_w) .flex_shrink_0 .h_full
      ├─ probe_area("sidebar")            ← CDP-замена (замер rect)
      └─ gap_wrap (px 4, pt 4, pb 4)      ← эмуляция .body{gap:8; padding:0 4}
         └─ customize_open ? customize_nav(...) : sessions_sidebar(...)
   └─ v_handle("sidebar-handle", ...)     ← элемент 21
```
`sidebar_w = layout.sidebar_width_px.round()` (дефолт `SIDEBAR_DEFAULT = 270`). Фон прозрачный — радиальный градиент фона просвечивает (как оригинал).

## Метрики (из кода, точные)
- Ширина: `layout.sidebar_width_px` px, персист в layout_store; drag-кламп `PANEL_MIN_SIZE = 100 .. viewport_w − 550`
- `flex_shrink_0` (не ужимается)
- Обёртка gap_wrap: `px(4)` + `pt(4)` + `pb(4)`, `min_w/min_h 0`, `overflow_hidden`
- Собственных bg/border/radius нет

## Отличия от original.md той же папки
1. **min-width: у нас кламп 100 (`PANEL_MIN_SIZE`), в оригинале `SIDEBAR_MIN_WIDTH_PX` (и `PRIMARY_SIDEBAR_MIN_WIDTH = 200` в metrics)** — сайдбар можно ужать до 100px.
2. `flex_shrink_0` vs оригинальный `flex-shrink: 1` (у оригинала сайдбар ужимается до inline min-width, у нас — жёсткая ширина).
3. Оригинал: customize-режим ПИНИТ сайдбар видимым (`return null` только если `!sidebarVisible && mode !== "customize"`). У нас `.when(self.sidebar_visible, …)` — при скрытом сайдбаре customize-навигация не рендерится вовсе.
4. Drop-target активностей (`data-activity-drop` over/blocked, onDragOver/Drop) НЕ РЕАЛИЗОВАН.
5. Нет aria (`aria-label="Primary side bar"`) — в gpui нет аналога.
6. gap_wrap даёт свои 4px паддинга внутри колонки (оригинал — нулевой паддинг, зазор из `.body { gap }`); визуально эквивалентно, но паддинг «внутри» ширины сайдбара, т.е. полезная ширина контента на 8px меньше при той же sidebar_width.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — у корня сайдбара ховер-стиля нет (`crates/shell/src/root.rs:6236-6244`: только relative/w/flex_shrink_0/h_full/probe_area). Ховер-реакция есть у смежной ручки: `v_handle` показывает полосу 3px цвета accent_primary #89b4fa при альфе 0.25 (`crates/shell/src/ui/splitter.rs:62,86`), а видимость гонит state `hovered_handle`, не CSS-ховер (`root.rs:6286,6292-6299`)
