# 102 generic-tree — оригинал
Файлы: `kamin-ide/src/renderer/components/tree/Tree.tsx:38-53`, `kamin-ide/src/renderer/components/tree/Tree.module.css`

## JSX-структура (кратко, вложенность)
```
ul.tree [role=tree]
└── nodes.map → <TreeRow key={n.id} node depth={0} expanded selectedId onToggle onSelect />
    (TreeRow — элемент 103; открытые dir рендерят ul.subtree [role=group] с детьми depth+1)
```
Полностью контролируемое: caller владеет `expanded: ReadonlySet<string>` и `selectedId: string | null`; клик по любому узлу → `onSelect(node)`, по dir дополнительно `onToggle(id)`. TreeNode: {id, label, type: "dir"|"file", meta?, icon?, children?}.

## Метрики (ИЗ CSS, точные значения)
`.tree`, `.subtree` (общее правило):
- list-style: none; margin: 0; padding: 0
- Отступ вложенности НЕ через ul — через paddingLeft строки (depth * 14px, инлайн; см. 103).

## Состояния (классы-варианты с метриками)
- У контейнера вариантов нет; всё состояние на строках (103).

## Дополнение атрибутов (цикл 10)

- цвета: N/A: цвета — `.tree`/`.subtree` содержат только `list-style: none; margin: 0; padding: 0` (`Tree.module.css:1-5`); весь цвет принадлежит строке (элемент 103): `.row { color: var(--text-primary) }` #cfd4e2 dark / #322e28 light (`:18`; `dark-theme.css:34`, `light-theme.css:44`).
