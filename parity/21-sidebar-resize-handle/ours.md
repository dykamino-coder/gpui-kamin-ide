# 21 sidebar-resize-handle — наша реализация
Файлы: `crates\shell\src\ui\splitter.rs:22-86` (v_bar/v_handle), `crates\shell\src\root.rs:5278-5295` (монтаж), `crates\shell\src\root.rs:2916-2951,2997-3008,3376-3383` (drag/persist/handle_show)

## Структура (gpui-дерево кратко)
```
v_handle("sidebar-handle"):
div .relative .w(0) .h_full .flex_shrink_0        ← нулевая ширина в потоке
└─ div#sidebar-handle .absolute .left(-4) .top_0
     .w(SPACE_2=8) .h_full .items_center .justify_center
     .cursor_col_resize .tooltip("Drag to resize")
     .on_mouse_down(begin_drag DragKind::Sidebar) .on_hover(hovered_handle)
   └─ .when(show) v_bar(tint(accent_primary,0.25), 3.0)
        ← 3 сегмента: fade-in 30% / solid 40% / fade-out 30% (linear_gradient 180°)
```
`show = hover ручки ИЛИ активный drag` (state-driven через `RootView.hovered_handle`, occlude не используется — mouse-up должен пузыриться до корня).

## Метрики (из кода, точные)
- Hit-зона: 8px (`SPACE_2`), absolute `left: -4px` — сидит в межколоночном зазоре
- Грип: ширина 3px, высота 100%, цвет `tint(p.accent_primary, 0.25)` (dark: #89b4fa @ 25% ≈ tint-primary-strong)
- Градиент растворения: transparent→color на 0–30%, solid 30–70%, color→transparent 70–100%
- Idle: пусто (ничего не рисуется)
- Drag-кламп: `100 .. viewport_w − 550`; персист `sidebarWidthPx` одним патчем на mouse-up

## Отличия от original.md той же папки
1. Idle-полоса 2px (`opacity: 0`, bg-overlay градиент) не рендерится вовсе — визуально идентично (у оригинала она невидима), но transition width 2→3px и opacity 0.15s отсутствуют (в gpui нет transition; появление грипа мгновенное).
2. Кламп роста: у нас хардкод `viewport − 550`; оригинал — `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX)` от фактической min-width центральной колонки.
3. Минимум: 100 (`PANEL_MIN_SIZE`) vs `SIDEBAR_MIN_WIDTH_PX` оригинала.
4. Нет `role="separator"` / `aria-orientation` / `aria-label` (нет аналога в gpui).
5. Позиция hit-зоны: оригинал `right: calc(-1*8px)` от сайдбара (целиком в gap справа); у нас нулевой элемент ПОСЛЕ сайдбара с `left:-4` — центр совпадает, но зона наполовину накрывает край сайдбара, а не gap целиком.
6. `z-index` не задаётся (порядок отрисовки по дереву).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — грип 4px без содержимого и паддингов (`crates/shell/src/ui/splitter.rs`, `fn v_handle`); зазор вокруг даёт `.body gap`
