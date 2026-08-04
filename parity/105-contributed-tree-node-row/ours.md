# 105 contributed-tree-node-row — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`level`), root.rs (`ShellEvent::TreeClick`)

## Структура/содержание
```
div .row  (id "tv:<view>:<handle>")
├─ expandable ? chevron-down|right в боксе 16 : спейсер 16
├─ (checkboxState) .treeCheckbox (элемент 106)
├─ node_icon (элемент 107)
├─ label .flex_1 .overflow_hidden .text_ellipsis .whitespace_nowrap
└─ (description) ml 6, opacity .55, fs 0.85em
```
Клик — `TreeClick{expandable, expanded, command}`: тоггл раскрытия + `kamin:tree:reportExpansion`, выделение + `kamin:tree:reportSelection`, затем `node.command` через `kamin:commands:execute` (и на листьях, и на родителях). Tooltip = `node.tooltip ?? node.label`.

## Метрики (из кода, точные)
- `.row`: flex, items-center, gap 6, w-full, `paddingLeft = depth*12 + 8`, pr SPACE_2 8, h 22, border 1px transparent (резерв), radius RADIUS_XS 4, fs FS_SM 12, цвет text-secondary.
- hover невыделенной: bg-surface 55% + text-primary.
- `.rowSelected`: линейный градиент 90° accent-primary 26% → 14%, рамка accent-primary 45%, текст text-primary; chevron наследует цвет.
- `.chevron`/спейсер: бокс 16, глиф 13, text-muted.

## Отличия от original.md той же папки
1. DnD не портирован (нет `draggable`, handleDrag/handleDrop).
2. Reveal-действие (`scrollIntoView` + focus + expand по `kamin:tree:reveal`) не портировано.
3. `.rowDir`/`.rowFile` — маркеры без правил, в порте не нужны.

## Дополнение атрибутов (цикл 10)

- цвета: базовый текст text_secondary #adb3c7 dark / #524c43 light (`contributed_tree.rs:312`, `palette.rs:64,102`); hover — фон bg_surface α .55 #3d3f51 / #e6e1d4 + текст text_primary #cfd4e2 / #322e28 (`contributed_tree.rs:291-296,314-316`); выделение — линейный градиент 90° accent_primary α .26 → α .14 (#89b4fa / #da8343) с бордером accent α .45 и текстом text_primary (`contributed_tree.rs:320-334`); chevron text_muted #838aa0 / #6e685d, у выделенной строки цвет наследуется (`contributed_tree.rs:365-367`); бордер по умолчанию прозрачный — резерв под выделение (`:308-309`). 1:1 с `.row`/`.row:hover`/`.rowSelected` (`FileTreeView.module.css:62-94`).
- шрифты: строка fs-sm 12 (`contributed_tree.rs:311`, `metrics/lib.rs:43`); chevron codicon 13 (`contributed_tree.rs:361-363`); иконка-глиф codicon кеглем fs-sm 12 в боксе 16×16 (`contributed_tree.rs:173,200-209`); description — 0.85em от fs-sm = 10.2px при opacity .55 (`contributed_tree.rs:398`, инлайн-стиль оригинала `TreeViewBody.tsx`); галка чекбокса codicon fs-xs 11 (`contributed_tree.rs:249`). Собственного font-weight у строки нет.
