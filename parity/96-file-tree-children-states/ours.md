# 96 file-tree-children-states — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:67-68` (`DIR_RENDER_CAP`), `:161-170` (усечение), `:344-372` («Show N more…»), `:619-627` (корневой «Loading…»); `crates/shell/src/root.rs:1143` (`ShowMoreDir` → `show_all`), `:711-713` (loading-set)

## Структура (gpui-дерево кратко)
```
rows(dir): дети рекурсивно плоским списком (без обёртки .children)
├── [entries.len() > 200 && !show_all] → рендер первых 200
└── capped → div #"{panel_key}:more:{dir}" .flex .items_center
      .pl((depth+1)*12+8) .py(2) .rounded(RADIUS_XS) .text_size(FS_XS)
      .text_color(text_muted) .hover(bg text_primary 6%)
      on_mouse_down(Left) → ShowMoreDir(dir)  → "Show {N} more…"

Корень (file_tree): root_expanded && cache пуст →
    div .pl(20) .py(2) .text_color(text_muted) "Loading…"
```
Для раскрытой ПОДдиректории без листинга детей нет вовсе — индикатор только spinner-глиф в chevron строки (tree.loading).

## Метрики (из кода, точные)
- Кап `DIR_RENDER_CAP` = **200**; клик по «Show more» ставит dir в `show_all` → показываются ВСЕ.
- showMore: `pl (depth+1)*12+8`, `py 2`, radius 4, `FS_XS` 11, `text_muted` #838aa0, hover `text_primary` a=0.06.
- Loading (корень): `pl 20`, `py 2`, `text_muted`, шрифт наследуемый FS_SM.

## Отличия от original.md той же папки
1. **Кап 200 без шага**: оригинал TREE_CHILD_CAP=100 + шаг 200 («догрузка» порциями); у нас 200 и клик раскрывает всё сразу.
2. **Лейбл**: «Show {N} more…» без «({rest} hidden)» и **без иконки codicon-ellipsis**.
3. **«(empty)» не рендерится** — пустая раскрытая папка выглядит как закрытая (ничего под ней).
4. **«Loading…» только на корневом уровне** (и с pl 20, не indentPx(1)=20 — тут совпало); в поддиректориях текстового Loading нет, только chevron-spinner.
5. hover showMore: `text_primary 6%` фон без смены цвета текста; оригинал — `bg-surface 55%` + `color: text-primary`.
6. padding showMore `py 2` vs `3px 0`; у нас есть radius/фон-хайлайт, у оригинала кнопка без скругления фона (background: none, hover-фон без radius-указания — фактически тот же класс, расхождение минимально).

## Дополнение атрибутов (цикл 10)

- гэпы: у строк «Loading…» и «(empty)» flex-gap нет — это одиночные текстовые блоки с `pl = depth*12 + 8` и `py 2` (`file_list.rs:168-176,181-189`), как `.loading`/`.emptyChild { padding: 2px 0 }` (`FileTreeView.module.css:155-160`); у строки «Show N more» gap 6 между глифом и текстом (`file_list.rs:408`) = `.showMore { gap: 6px }` (`:164-168`).
- шрифты: «Loading…»/«(empty)»/«Show N more» — кегль FS_XS 11 (font-size 11), text-muted (`ui/file_list.rs`, `fn rows`); корневой ветки «Loading…» больше нет — она была недостижима и удалена (ревью ц.12)
