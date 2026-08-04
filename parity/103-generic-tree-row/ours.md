# 103 generic-tree-row — наша реализация

**НЕ РЕАЛИЗОВАНО** (вместе с контейнером, см. 102-generic-tree/ours.md).

Строки generic-дерева (TreeRow: indent 14px/уровень, chevron 14px grid-center глиф 10px со скрываемым `.chevronHidden`, codicon-иконки dir=`accent_yellow`/file=`text_muted`, правый `.meta` моноширинный FS_XS, padding 4px/SPACE_2, selected — тот же accent-градиент 26→14% + бордер 45%) в gpui-порте отсутствуют.

Ближайший родственник — строки `ui/file_list.rs` (элементы 94/95), но у них другой indent (12px+8), img-иконки вместо codicon, нет meta-слота и chevronHidden.

## Отличия от original.md той же папки
Полное отсутствие компонента.

## Дополнение атрибутов (цикл 10)

- гэпы: компонента нет; ближайший аналог — строки файлового дерева `file_list.rs:211` с `gap 6` (у generic-строки оригинала `gap: var(--space-2)` 8, `Tree.module.css:10`), в contributed-дереве тоже 6 (`contributed_tree.rs:301`).
- цвета: компонента нет; в аналоге (`file_list.rs:200-243`) hover-фон bg_surface α .55 = #3d3f51 dark / #e6e1d4 light (`palette.rs:57,95`) + текст text_primary #cfd4e2 / #322e28, выделение — градиент 90° accent_primary α .26 → α .14 (#89b4fa / #da8343, `palette.rs:83,121`) с бордером accent α .45; базовый цвет строки у нас text_secondary #adb3c7 / #524c43 (`file_list.rs:221`) против `--text-primary` у generic-строки оригинала (`Tree.module.css:18`) — расхождение при портировании учесть.
- скругления: компонента нет; в аналоге radius-xs 4 (`file_list.rs:220`, `metrics/lib.rs:36`) — совпадает с `.row { border-radius: var(--radius-xs) }` оригинала (`Tree.module.css:17`).
- ховер: компонента нет; в аналоге hover = фон bg_surface α .55 + text_primary, и только у НЕвыделенной строки (`file_list.rs:225-227`), что соответствует паре `.row:hover` / `.row.selected:hover` оригинала (`Tree.module.css:26-37`); transition `background var(--transition-fast) 150ms` (`:23`) в gpui не воспроизводится.
