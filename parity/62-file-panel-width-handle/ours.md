# 62 file-panel-width-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:22-86 (v_handle); crates/shell/src/root.rs:4715-4733 (main_file_handle), 2952-2958 (DragKind::MainFile), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
v_handle("main-file-handle") — сиблинг между main_wrap и file_wrap в body
= div .relative .w(0) .h_full .flex_shrink_0
  └─ div .absolute .left(-4) .w(8) .h_full .cursor_col_resize
       .tooltip("Drag to resize") .on_mouse_down .on_hover
     └─ when(show): v_bar 3px (fade 30% / solid 40% / fade 30%), tint(accent_primary, 0.25)
```
show = hovered_handle == id || dragging(MainFile).

## Метрики (из кода, точные)
- Hit: 8px (SPACE_2), центр на стыке (−4..+4)
- Полоса hover/drag: 3px × 100%, tint-primary-strong (accent_primary α 0.25: dark #89b4fa, light #da8343), вертикальное растворение концов 0–30 / 70–100%
- idle: пусто (нет элемента)
- Drag (MainFile): вправо → main шире; `nf = (init − d).max(PANEL_MIN_SIZE=100)` → file_panel_width_ratio = ratio_from_width(nf, viewport_w); персист на mouse-up
- Центр защищён flex: main_wrap min_w 100 flex_1

## Отличия от original.md той же папки
1. Позиция: оригинал absolute `left: -8px` целиком в зазоре; у нас −4..+4 симметрично стыку.
2. `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX=100)` не воспроизведён формулой: у нас только `max(100)`, а невозможность задавить центр обеспечивает flex-раскладка (main_wrap min_w 100). Поведение на границе близко, но file может «упереться» иначе при узком окне.
3. Результат хранится ratio (не px) — см. 61.
4. idle-полоса с opacity 0 + transition 0.15s → у нас мгновенное появление, полосы в idle нет.
5. `layoutActiveEditorNow()` (анти-мерцание Monaco minimap) не нужен — редактор наш, релэйаут в кадре.
6. role/aria «Resize file panel» — нет DOM.

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding нет — `v_handle("main-file-handle")` (`root.rs:5600-5618`) это `w(0)` сиблинг с хит-зоной `absolute; left −4; w SPACE_2 8; h 100%` (`splitter.rs:63-79`); против инсета `left: −8px` у оригинала — наша зона центрирована на стыке.
