# 106 contributed-tree-checkbox — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`checkbox`), root.rs (`ShellEvent::TreeCheckbox`)

## Структура/содержание
Рендерится, когда у узла задан `checkboxState`. Клик — `cx.stop_propagation()` + `kamin:tree:reportCheckbox` с ИНВЕРТИРОВАННЫМ состоянием; провайдер обновляет модель, `onDidChangeTreeData` возвращает перевёрнутый узел.

## Метрики (из кода, точные)
14×14, margin-right 4, flex-shrink 0, центровка, radius 3, рамка 1px currentColor (белый 35%), cursor pointer; при CHECKED — codicon-check размером FS_XS 11. `checkboxTooltip` вешается тултипом, если задан.

## Отличия от original.md той же папки
Клавиатурного тоггла (Space/Enter, tabIndex=0) нет — в порте нет фокус-навигации по строкам дерева.

## Дополнение атрибутов (цикл 10)

- цвета: заливки нет; цвет рамки передаётся параметром `border` = цвет строки (эквивалент currentColor): text_secondary #adb3c7 dark / #524c43 light у обычной строки и text_primary #cfd4e2 / #322e28 у выделенной (`contributed_tree.rs:372-379`, `palette.rs:62,64,100,102`); галка тоже наследует цвет строки (`contributed_tree.rs:249`).
- шрифты: галка — codicon кеглем fs-xs 11 (`contributed_tree.rs:249`, `metrics/lib.rs:42`) = `.treeCheckbox { font-size: 11px }` (`FileTreeView.module.css:117`); собственного семейства/веса у бокса нет.
- ховер: N/A: ховер — своего ховера у чекбокса нет; цвет рамки берётся ОДИН раз по признаку выделения строки и на ховер строки НЕ реагирует (в оригинале currentColor меняется по `.row:hover`) — расхождение, а не отсутствие семейства
