# 104 contributed-tree-view-body — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:42-49` (TreeViewBody 27-50, TreeLevel 54-79), CSS переиспользуется из `file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root (FileTreeView.module.css)
├── {message} → div (инлайн-стиль: padding "4px 8px"; fontSize var(--fs-sm); opacity 0.75) {message}   // TreeView.message
└── div.body
    └── <TreeLevel viewId parent=undefined depth=0 version />
        ├── nodes === null → div.loading (paddingLeft = indentPx(depth)) "Loading…"
        ├── nodes.length === 0 && depth === 0 → div.emptyChild "(empty)"  (глубже — <></>)
        ├── nodes.slice(0, 100).map → <TreeNode key={n.handle}> (элемент 105)
        └── nodes.length > 100 → div.emptyChild "… {N-100} more"
```
- Дети лениво с хоста: hostRpc.trees.getChildren(viewId, parent); рефреш по treeChangeVersion[viewId].
- Кап TREE_CHILD_CAP = 100 (без кнопки догрузки — только счётчик остатка).
- indentPx(d) = d*12 + 8 px (INDENT_PX=12, BASE_INDENT_PX=8 — локальные копии в TreeViewBody.tsx:14-15).
- meta/DnD подтягиваются на mount (getMeta, hasDnd).

## Метрики (ИЗ CSS, точные значения)
Из FileTreeView.module.css:
- `.root`: flex: 1; display: flex; flex-direction: column; min-height: 0
- `.body`: flex: 1; overflow: auto; padding: 4px 6px 8px; font-size: var(--fs-sm)
- `.loading`, `.emptyChild`: font-size: var(--fs-xs); color: var(--text-muted); padding: 2px 0 (+ инлайн paddingLeft)
- message-баннер (инлайн): padding: 4px 8px; font-size: var(--fs-sm); opacity: 0.75

## Состояния (классы-варианты с метриками)
- Loading / empty / overflow («… N more») — см. структуру; вариантных классов у контейнера нет.
