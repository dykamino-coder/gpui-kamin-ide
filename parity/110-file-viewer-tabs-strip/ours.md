# 110 file-viewer-tabs-strip — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\editor_tabs.rs:37-313 (editor_tabs_bar), crates\shell\src\root.rs:4330-4355 (вызов; ширина из probe-реестра «file-tabs», −16)

## Структура (gpui-дерево кратко)
```
div.bar: flex.items_center.gap(4).flex_shrink_0.px(8).pt(4).overflow_hidden
├─ tab × visible (№111)
└─ (hidden непусты) кнопка «N ▾» + deferred-меню (№112)
```
Раскладка: оценка ширины таба `tab_width_est = chars×6.5 + 50` (+4 gap); не влезшие в `available_w − 40` уходят в hidden; активный всегда видим (подмена последнего видимого). Drag-reorder: mouse-down → `TabPress(i,x,y)` (порог 4px разруливает root), зажатая ЛКМ над табом → `TabDragOver(i)`; цель вставки = `border_l_2 accent_primary` на самом табе.

## Метрики (из кода, точные)
- Полоса: gap 4 (SPACE_1 — совпадает), px 8 (SPACE_2), pt 4 (SPACE_1), pb НЕТ, overflow_hidden
- Индикатор вставки: левый бордер 2px p.accent_primary #89b4fa на целевом табе
- Порог драга 4px (root.rs, TabDrag.started)

## Отличия от original.md той же папки
1. Индикатор вставки — `border_l_2` на табе (сдвигает контент на 2px) вместо absolute-полосы 2×(h−10) rounded-1, позиционируемой в px.
2. Padding: у оригинала 4px сверху И снизу (`padding: 4px var(--space-2)`), у нас только pt 4.
3. Overflow-детект: оценка ширины по числу символов (6.5px/симв) vs реальный `scrollWidth > clientWidth + 1` + ResizeObserver — при пропорциональном шрифте оценка неточна.
4. Активный таб принудительно остаётся видимым подменой последнего (в оригинале стрип скроллится, scrollIntoView).
5. `.tabDragging` (opacity 0.3 у перетаскиваемого) не реализован — визуально драг показывает только индикатор цели.
6. Нет role=tablist / aria-label / tabIndex.
7. При 0 табов полоса не рендерится (ветка редактора не активна) — поведение совпадает.

## Дополнение атрибутов (цикл 10)

- скругления: таб border-radius 8 (RADIUS_SM) (editor_tabs.rs:102); close-кнопка border-radius 4 (RADIUS_XS) (editor_tabs.rs:202); dirty-точка rounded_full 6×6 (editor_tabs.rs:186); сама полоса `.bar` без скругления
- ховер: неактивный таб — bg p.bg_surface #3d3f51 α .5 + text p.text_primary #cfd4e2 (editor_tabs.rs:90,171); close-крестик: opacity 0 → 0.7 по group_hover таба (editor_tabs.rs:222), у активного 0.7 постоянно (editor_tabs.rs:220), собственный hover крестика — bg p.bg_overlay #515567 α .6 + opacity 1.0 (editor_tabs.rs:203-205); у активного таба hover нет (bg accent_primary α .16 фиксирован, editor_tabs.rs:167-169)
