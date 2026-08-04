# 92 file-tree-root — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:415-641` (`file_tree()`), `crates/shell/src/root.rs:3060-3104` (wiring в tool_body «tree»), `crates/shell/src/root.rs:132,364` (владелец `TreeState`)

## Структура (gpui-дерево кратко)
```
div .size_full .flex .flex_col .min_h(0)
├── header (элемент 98)
└── scroll_body: div #panel_key .relative .flex_1 .min_h(0) .flex .flex_col
      .text_size(FS_SM) .overflow_y_scrollbar_with(tree.scroll) .px(6) .pb(8)
    ├── probe_area(panel_key)
    ├── root_row (кастомная строка корня: chevron 12 + folder_img 16×16 + имя;
    │             pl 8, gap SPACE_1=4, py 2, radius XS, hover text_primary 6%;
    │             LMB toggle, RMB → меню корня)
    └── rows(root, depth=1) при root_expanded (элементы 94-96);
        если cache пуст → "Loading…" (pl 20, py 2, text_muted)
```
Смена workspace сбрасывает `TreeState` целиком (root.rs:638) — аналог ремаунта по `key={root}`.

## Метрики (из кода, точные)
- Тело: `px(6)`, `pb(8)`, **top-padding нет**; font `FS_SM` = 12px; фон прозрачный; скролл — `ScrollHandle` (программный, для Locate).
- root_row: `pl(8)`, `pr(SPACE_2=8)`, `py(2)`, `gap(SPACE_1=4)`, `rounded(RADIUS_XS=4)`, hover `text_primary` a=0.06 (#cfd4e2 @6%).
- Chevron корня: codicon 12px в боксе 16×16, цвет `text_muted` #838aa0.

## Отличия от original.md той же папки
1. **RMB по пустой области тела не открывает меню корня** — обработчик RMB только на строках; в оригинале `onContextMenu` на `.body` (e.target===currentTarget → меню корня). Меню корня доступно лишь через RMB по root_row.
2. **padding тела**: у нас `6px горизонталь + 8 низ`, верхних 4px нет (оригинал `padding: 4px 6px 8px`).
3. **Корень — кастомная строка**, не обычный FolderNode depth 0: gap 4 вместо 6, hover `text_primary 6%` вместо `bg-surface 55%`, нет TreeIcon-«isRoot», нет selected-состояния у корня.
4. `[data-file-tree]` нет — вместо него id `panel_key` + probe_area.

## Дополнение атрибутов (цикл 10)

- шрифты: скролл-тело задаёт базовый кегль fs-sm 12 (`file_list.rs:663`, `metrics/lib.rs:43`) = `.body { font-size: var(--fs-sm) }` (`FileTreeView.module.css:14`); заголовок хедера fs-xs 11 + weight 500 MEDIUM + ss01 (`file_list.rs:528-529`); бейдж «Indexing…» fs-xs 11 + глиф codicon 12 (`file_list.rs:541,544`); кнопки тулбара — глиф codicon 14 (`file_list.rs:453`).
