# 59 right-panel-split-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:88-136 (h_handle); crates/shell/src/root.rs:4652-4670 (вызов, pr=ACTIVITY_BAR_WIDTH), 2989-2993 (DragKind::RightSplit)

## Структура (gpui-дерево кратко)
```
h_handle("right-split-handle", pr=44)
= div .flex_shrink_0 .h(8) .min_w(0) .pr(44)
    .flex .items_center .justify_center .cursor_row_resize
    .tooltip("Drag to resize")
  └─ div .relative (probe_area + грип 32×3 rounded 4)
```
Сиблинг между верхней и нижней card_with_rail в right_column_el.

## Метрики (из кода, точные)
- Hit: высота 8px (SPACE_2); pr 44 (ACTIVITY_BAR_WIDTH) — грип центрируется по карточке, не по колонке (rail справа не рассекается)
- Грип: 32×3, radius 4; idle bg_overlay (#515567/#d6d0c0) opacity 0.7; hover/drag accent_primary (#89b4fa/#da8343) opacity 1
- Drag: right_split = init + dy/body_h, кламп [RIGHT_SPLIT_MIN 0.15, RIGHT_SPLIT_MAX 0.85]

## Отличия от original.md той же папки
1. Высота hit-зоны 8px против 10px оригинала.
2. pr = 44 против `padding-right: var(--layout-activity-bar-width, 48px)` — при токене 44 совпадает, fallback 48 не воспроизводим.
3. Нет transition 0.15s.
4. Рендерится всегда (bottomShown-гейта нет — низ правой колонки не отключаем).
5. right_split после драга НЕ персистится (в end_drag патче отсутствует) — оригинал сохраняет rightPanelSplit.
6. role/aria — нет DOM.
