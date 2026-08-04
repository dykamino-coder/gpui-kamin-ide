# 94 file-tree-folder-row — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:171-327` (`rows()`, ветка `is_dir`), `crates/shell/src/ui/icon.rs:45-55` (codicon), `crates/shell/src/icon_theme.rs:129-138` (folder_img)

## Структура (gpui-дерево кратко)
```
div #"{panel_key}:{path}" .flex .items_center .gap(6) .pl(depth*12+8) .pr(8) .py(2)
    .rounded(RADIUS_XS) .cursor_pointer .hover(bg_surface 55%)
    [selected → bg linear_gradient(90°, accent 26% → 14%) + border_1 accent 45%]
    on_mouse_down(Left): Ctrl → select-toggle, иначе select + toggle expand
    on_mouse_down(Right): stop_propagation + OpenFileMenu(path, true, x, y)
├── codicon(loading ? "\u{eb19}" : expanded ? CHEVRON_DOWN : CHEVRON_RIGHT, 12px)
│   .text_color(text_muted)
├── icon_theme::folder_img(name, expanded) 16×16 .flex_shrink_0
├── label: div .flex_1 .min_w(0) .overflow_hidden .text_ellipsis .whitespace_nowrap
│   [deco.color → text_color(deco_color(id))]
└── badge (элемент 97)
```
Дети рекурсивно append'ятся плоско в общий Vec (обёртки `.node`/`.children` нет — display:contents эквивалент).

## Метрики (из кода, точные)
- gap 6; `pl = depth*12 + 8`; `pr = SPACE_2` = 8; `py 2` (высота контентная); `rounded RADIUS_XS` = 4.
- hover: `bg_surface` #3d3f51 a=0.55.
- selected: градиент 90° `accent_primary` #89b4fa 26%→14% + `border_1` accent 45%; hover поверх selected тот же (hover-bg перекрывает? — hover задан всегда, при selected градиент в bg, hover заменит его на bg_surface 55% — см. отличия).
- chevron: глиф 12px в боксе 16×16, `text_muted` #838aa0; spinner-глиф `\u{eb19}` (codicon-loading).
- иконка 16×16; label — цвет deco или наследуемый.

## Отличия от original.md той же папки
1. **Высота строки**: `py(2)` (~20px контентно) вместо фиксированной `height: 22px`.
2. **Нет резервного `border: 1px solid transparent`** — бордер появляется только у selected → контент selected-строки сдвигается на 1px.
3. **Цвет текста не задан** (наследуется), оригинал: `--text-secondary` → hover `--text-primary`. Hover у нас цвет НЕ меняет.
4. **`.rowSelected:hover`**: у нас hover(bg_surface 55%) объявлен на всех строках — при наведении на selected градиент подменяется обычным hover-фоном (оригинал сохраняет градиент).
5. **chevron 12px** vs 13px оригинала; `.rowSelected .chevron: color inherit` не воспроизведён (остаётся muted).
6. **Спиннер не вращается** — статичный глиф codicon-loading без `codicon-modifier-spin` анимации.
7. **Папки не draggable** (draggable={depth>0} оригинала; у нас on_drag только у файлов) и **нет `.dropTarget`** (accent 22% + outline) — drop на папку не реализован.
8. **Нет Shift-select** (только Ctrl-toggle) и **нет клавиатуры** (Delete/F2/Ctrl+X/C/V, aria-expanded).
9. **Нет data-tooltip** (deco.tooltip ?? path) и нет `.flash`-анимации locate.
10. Клик срабатывает на mouse_down, не на click.

## Дополнение атрибутов (цикл 10)

- шрифты: строка своего кегля не задаёт — наследует fs-sm 12 от скролл-тела (`file_list.rs:663`, строка `:207-227`), ровно как `.row { font: inherit; font-size: var(--fs-sm) }` (`FileTreeView.module.css:80-81`); chevron codicon 13 в боксе 16 (`file_list.rs:307-316`) = `.chevron { font-size: 13px; width: 16px }` (`:120-127`); бейдж декорации fs-xs 11 + weight 600 SEMIBOLD (`file_list.rs:365-366`) = `.badge { var(--fs-xs); font-weight: 600 }` (`:147-153`); имя папки — тот же наследуемый кегль без weight.
