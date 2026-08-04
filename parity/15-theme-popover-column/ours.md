# 15 theme-popover-column — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:515-530 (closure `column`),
532-588 (наполнение Dark/Light/Icons)

## Структура (gpui-дерево кратко)
```
div (flex col, min_w 140)
 ├ colTitle (uppercase, fs XS, muted)
 └ rows: item… (элемент 16)
```
Dark = «Kamin Dark» + contributed c dark_ui; Light = «Kamin Light» +
contributed light; Icons = «Catppuccin» (built-in) + contributed icon-темы.

## Метрики (из кода, точные)
- column: min_w px(140.0), flex col; gap между строками — НЕ задан
- colTitle: px SPACE_2 (8), py SPACE_1 (4), fs m::FS_XS (11),
  color p.text_muted, .to_uppercase()

## Отличия от original.md той же папки
1. colList max-height: 320px + overflow-y:auto — НЕ РЕАЛИЗОВАНО (длинный
   список тем растянет поповер).
2. gap 1px между item'ами (.colList) — нет.
3. letter-spacing 0.04em у colTitle — нет.
4. Ширина: min_w 140 по контенту вместо грид-ячейки minmax(140px, 1fr) —
   колонки разной ширины (см. 14, п.3).
5. role=listbox / aria — не применимо.

## Дополнение атрибутов (цикл 10)

- цвета: у самой колонки фона нет — прозрачная поверх bg_surface #3d3f51 поповера (`crates/shell/src/ui/layout_popover.rs:720`); заголовок колонки text_muted #838aa0 (`layout_popover.rs:564`); строки внутри text_primary #cfd4e2 (`layout_popover.rs:500`), picked-фон accent_primary #89b4fa при альфе 0.16 (`layout_popover.rs:524`), hover bg text_primary@0.10 (`layout_popover.rs:490`)
- шрифты: заголовок колонки FS_XS = 11 + `to_uppercase()` (`layout_popover.rs:563,565`); строки FS_SM = 12 (`layout_popover.rs:499`); font-weight в колонке не задан (SEMIBOLD только у заголовка «Appearance» самого поповера, `layout_popover.rs:661`)
