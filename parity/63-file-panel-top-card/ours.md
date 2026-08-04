# 63 file-panel-top-card — наша реализация
Файлы: crates/shell/src/root.rs:4496-4549 (top_card + обёртка), 4118-4314 (web-ветка), 4315-4487 (редактор), 4488-4495 (плейсхолдер); crates/shell/src/ui/glint.rs:122-233

## Структура (gpui-дерево кратко)
```
div h=relative(1 − bottom_ratio) .min_h(100)
└─ gap_wrap_v_top (px 4, pt 4, pb 0)
   └─ glint_surface_wv_holed(top_card)
      top_card: div .flex_col .size_full .min_h(0)
        .on_drop(ExternalPaths → OpenFile) .on_drop(DraggedFile → OpenFile)
      ├─ modeHeader: div .flex .justify_end .items_center .flex_shrink_0
      │    .pt(6) .px(8) → file_panel_mode_tabs (элемент 66)
      └─ top_content:
         web-режим  → browser_pane / visual_frame (элемент 67)
         есть табы  → редактор: полоса editor_tabs_bar + Save-кнопка + рамка
                      editor_bg radius 12 (breadcrumb h24 + Input + minimap + sticky)
         иначе      → panel_placeholder("File",
                      "Click a file in any panel, or drag-and-drop one from outside", SlotIcon::Center)
```

## Метрики (из кода, точные)
- Карточка: glint radius 16 / inner 15, заливка bg_mantle (#262533 / #fbf7f4)
- modeHeader: pt 6, px 8 (SPACE_2), pb 0, justify-end — точно `padding: 6px 8px 0`
- Редакторная рамка: mx 4, mt 4, mb 4, rounded 12 (RADIUS_MD), bg editor_bg (#1d1c25 / #fcfaf6); breadcrumb h 24, px 12, fs 11, JetBrains Mono, text_muted
- Save-кнопка (dirty): px 12, py 3, rounded 8, bg accent_action, fs 11 semibold, text accent_action_fg, hover opacity 0.9
- Плейсхолдер: label «File» fs 16 semibold + hint fs 12 (текст совпадает с оригиналом)

## Отличия от original.md той же папки
1. Drop-target ЕСТЬ (внешние файлы из Explorer + drag из дерева → открыть в редакторе) — оригинал явно «без drops». Расширение поведения, не потеря.
2. Вместо `<FileViewer />` (Monaco + свои табы) — собственный стек: editor_tabs_bar + gpui-component Input(code_editor) + breadcrumb + sticky-scroll + minimap; Save-кнопка в полосе табов (в оригинале грязность — dirty-точка на табе, сохранение Ctrl+S).
3. `.topCard { flex: 1 }` → у нас верх задан долей relative(1−bottom_ratio) (инверсия схемы высот, см. 61/64).
4. aria-label «File card» — нет DOM.
5. Сама карточка и modeHeader — 1:1 (glint 16, header 6/8/0 justify-end).
