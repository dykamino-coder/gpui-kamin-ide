# 107 contributed-tree-node-icon — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`node_icon`)

## Структура/содержание
Три взаимоисключающие ветки в порядке оригинала:
1. `node.codicon` (ThemeIcon) → `codicon_by_name(...)`, бокс 16;
2. `node.resourceUri` → basename → `icon_theme::file_img` (collapsibleState == NONE) либо `folder_img(name, expanded)`, 16×16;
3. иначе — codicon-circle-outline (лист) / codicon-folder (узел), 16.

## Метрики (из кода, точные)
Бокс 16×16, flex-shrink 0; цвет codicon-веток наследуется от строки (text-secondary, у выделенной — text-primary).

## Отличия от original.md той же папки
Light-фильтр `saturate(3.2) brightness(0.7)` для `<img>`-иконок (TreeIcon.module.css) не применяется — общий пробел порта (см. элемент 99).

## Дополнение атрибутов (цикл 10)

- отступы: у бокса иконки padding/margin нет — фикс 16×16 (`contributed_tree.rs:200-209`), img-вариант тоже 16×16 (`:184-188`); отступ уровня строки `pl = depth*12 + 8` (`:303`, `indent()` `:150-152`), правый край `pr SPACE_2 8` (`:304`); чекбокс перед иконкой добавляет `mr 4` (`:227`) — как `.treeCheckbox { margin-right: 4px }`.
- гэпы: собственного gap у иконки нет (`icon_box` — flex-center без gap, `contributed_tree.rs:200-209`); расстояние до лейбла даёт строка `gap 6` (`contributed_tree.rs:301`) = `.row { gap: 6px }`.
- цвета: codicon-глиф своего цвета не имеет — наследует цвет строки: text_secondary #adb3c7 dark / #524c43 light, на hover/выделении text_primary #cfd4e2 / #322e28 (`contributed_tree.rs:167-196` + строка `:312,315,334`); при `resourceUri` рисуется img-иконка Catppuccin/contributed-темы со СВОИМИ цветами внутри SVG (`contributed_tree.rs:177-189`, `icon_theme.rs:119-138`); светлотемный фильтр оригинала `saturate(3.2) brightness(0.7)` (`TreeIcon.module.css:6`) НЕ портирован — grep по `crates/shell/src` не даёт ни `saturate`, ни `brightness`.
- ховер: N/A: ховер — у иконки собственных hover-правил нет (`contributed_tree.rs:167-210`), как и у `.icon` оригинала; меняется только унаследованный цвет при ховере строки (105).
