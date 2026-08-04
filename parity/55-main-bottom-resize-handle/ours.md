# 55 main-bottom-resize-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:88-136 (h_handle); crates/shell/src/root.rs:4059-4077 (вызов), 2983-2988 (DragKind::MainBottom), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
h_handle("main-bottom-handle", show, pr=0)
= div .flex_shrink_0 .h(SPACE_2=8) .min_w(0) .pr(0)
    .flex .items_center .justify_center .cursor_row_resize
    .tooltip("Drag to resize") .on_mouse_down(Left) .on_hover
  └─ div .relative (probe_area("main-bottom-handle") + грип)
     └─ div 32×3 .rounded(RADIUS_XS=4)
```
show = hovered_handle == id || dragging(MainBottom) — hover state-driven через RootView.hovered_handle (не CSS :hover).

## Метрики (из кода, точные)
- Hit-зона: высота 8px (SPACE_2), ширина — stretch колонки
- Грип: 32×3px, radius 4 (RADIUS_XS)
  - idle: bg_overlay (dark #515567 / light #d6d0c0), opacity 0.7
  - hover/drag: accent_primary (dark #89b4fa / light #da8343), opacity 1
- Drag: `main_split = init + dy/body_h`, кламп [MAIN_SPLIT_MIN 0.2, MAIN_SPLIT_MAX 0.85]; персист `mainSplit` одним патчем на mouse-up (end_drag)
- Курсор row-resize; tooltip «Drag to resize»

## Отличия от original.md той же папки
1. Высота hit-зоны 8px против 10px оригинала (`.resizeHandle { height: 10px }`).
2. Нет transition 0.15s (opacity/background) — переключение мгновенное.
3. Hover реализован состоянием (hovered_handle) — визуально то же, но подсветка не сработает во время чужого драга.
4. Гард оригинала «`!mainVisible` → drag не начинается» не нужен: при скрытом main ручка не рендерится вовсе.
5. role="separator"/aria — нет DOM; tooltip совпадает.

## Дополнение атрибутов (цикл 10)

- отступы: собственных паддингов нет — `h_handle(..., pr = 0.0)` в вызове для main-bottom (`root.rs:4906-4910`), т.е. `.pr(px(0))` (`splitter.rs:126`); грип 32×3 центрируется `justify_center` по всей ширине колонки. Для сравнения: у правой колонки та же функция вызывается с `pr = ACTIVITY_BAR_WIDTH 48` (`root.rs:5532`), у file-bottom тоже 0 (`root.rs:5420`).
