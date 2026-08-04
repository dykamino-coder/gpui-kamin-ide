# 102 generic-tree — наша реализация

Файлы: `crates/shell/src/ui/tree.rs` (`tree()`, `TreeNode`); единственный
потребитель — семпл Design-панели (`ui/design_panel.rs`, `sample_tree`).

> Цикл 13: РЕАЛИЗОВАНО. Раньше компонента не было вовсе, а семпл дерева был
> собран по рецепту file-tree — расходилось каждое свойство (ревью ц.13).

## Структура (gpui-дерево)
```
div .flex .flex_col                       // <ul role="tree">
  └ строка × узел (рекурсия по раскрытым):
      div#tree-<id> flex items_center gap SPACE_2, w_full,
                    py 4, pr SPACE_2, pl = depth × 14,
                    border 1px transparent, rounded RADIUS_XS,
                    FS_SM, text_primary, cursor_pointer
        ├ бокс шеврона 14 (глиф codicon 16, text_muted; лист — пустой бокс)
        ├ codicon 12 (папка — accent_yellow, файл — text_muted)
        ├ label .flex_1 ellipsis nowrap
        └ [meta] моно FS_XS text_muted, flex_shrink_0
```

## Метрики (из кода, точные)
- Индент **14** (`INDENT_PX`) — не «12 + 8» файлового дерева.
- Строка: `py` **4**, `pr` SPACE_2 **8**, `pl` = depth × 14 (инлайновый
  `paddingLeft` оригинала перебивает левую часть шортхенда — на глубине 0
  слева НОЛЬ); `gap` SPACE_2 **8**; rounded RADIUS_XS **4**;
  FS_SM **12**; цвет `text_primary` **#cfd4e2**.
- Резервная рамка 1px `transparent` (без сдвига раскладки при выделении).
- Ховер: `bg_surface` @ **0.55**. Выделенная: градиент 90°
  accent 26 % → 14 %, рамка accent 45 %, ховер её НЕ перебивает.
- Шеврон: бокс **14**, глиф **16** (модуль кегль задаёт РОДИТЕЛЮ, значит у
  глифа база `.codicon`), цвет `text_muted`; у листа бокс пустой (место
  сохраняется, как `visibility: hidden`).
- Иконка узла: **12** (`.iconDir/.iconFile` задают fs-sm НА ТОМ ЖЕ элементе),
  папка — `accent_yellow`, файл — `text_muted`; `icon` узла переопределяет.
- `meta`: моно, FS_XS **11**, `text_muted`, `flex-shrink: 0`.

## Отличия от original.md той же папки
1. Ролей `tree`/`treeitem`/`group`, `aria-expanded`/`aria-selected` нет.
2. `transition: background` — переходов в gpui нет.
3. Клик обрабатывается на `mouse_down` (в gpui нет отдельного `click`).

## Атрибуты
- отступы: строка py 4 / pr 8 / pl = depth × 14; рамка семпла p 8
- цвета: текст text-primary #cfd4e2, ховер bg-surface 55 %, выделение —
  градиент accent-primary 26 %→14 % и рамка 45 %, папка accent-yellow #f9e2af,
  файл и meta text-muted #838aa0
- шрифты: строка — кегль fs-sm 12; meta — моно fs-xs 11; шеврон 16; иконка 12
- скругления: строка radius-xs 4; рамка семпла radius-sm 8
- гэпы: внутри строки 8
- ховер: подложка bg-surface 55 %; у выделенной строки ховер ничего не меняет
