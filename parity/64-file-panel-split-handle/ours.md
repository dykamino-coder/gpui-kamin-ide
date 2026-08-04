# 64 file-panel-split-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:88-136 (h_handle); crates/shell/src/root.rs:4551-4569 (вызов), 2976-2982 (DragKind::FileBottom), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
h_handle("file-bottom-handle", pr=0)
= div .flex_shrink_0 .h(8) .min_w(0) .pr(0)
    .flex .items_center .justify_center .cursor_row_resize
    .tooltip("Drag to resize")
  └─ div .relative (probe_area + грип 32×3 rounded 4)
```
Сиблинг между верхней картой и слотом centralBottom в file_column.

## Метрики (из кода, точные)
- Hit: высота 8px; pr 0 (rail сбоку нет — грип по центру колонки, как в оригинале)
- Грип: 32×3, radius 4; idle bg_overlay opacity 0.7; hover/drag accent_primary opacity 1
- Drag: `ratio = init − dy/body_h` (вниз → низ меньше), кламп [BOTTOM_RATIO_MIN 0.1, BOTTOM_RATIO_MAX 0.8]; персист `filePanelBottomHeightRatio` на mouse-up

## Отличия от original.md той же папки
1. Высота hit-зоны 8px против 10px.
2. Модель ресайза: оригинал двигает ПИКСЕЛЬНУЮ высоту низа (`max(100, startHeight − deltaY)`), низ фиксирован в px; у нас ratio колонки с клампом [0.1, 0.8] — при resize окна низ масштабируется, у оригинала нет; жёсткого min 100px у низа нет (0.1 доли может быть <100px на низких окнах).
3. Нет transition 0.15s.
4. Рендерится всегда (filePanelBottomVisible-гейта нет).
5. `layoutActiveEditorNow()` не нужен.
6. role/aria «Resize bottom pane» — нет DOM; tooltip совпадает.

## Дополнение атрибутов (цикл 10)

- цвета: грип idle — bg_overlay #515567 dark / #d6d0c0 light при opacity 0.7 (`splitter.rs:113-114`, `palette.rs:58,96`); hover/drag — accent_primary #89b4fa / #da8343 при opacity 1 (`splitter.rs:106`, `palette.rs:83,121`); сама хит-зона (h 10px) без фона — просвечивает подложка bg_sidebar #1d1d28 / #f4f1ea (`palette.rs:56,94`). Совпадает с `.splitGrip` / `.splitHandle:hover .splitGrip` (`FilePanel.module.css:108-121`).
