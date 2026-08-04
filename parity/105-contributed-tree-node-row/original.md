# 105 contributed-tree-node-row — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:144-178` (TreeNode 81-186), CSS из `file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.node
├── button.row.{rowDir|rowFile}[.rowSelected] (ref=rowRef)
│   (style: paddingLeft = depth*12 + 8 px; aria-expanded только если expandable;
│    data-tooltip = node.tooltip ?? node.label; draggable = dndEnabled;
│    onDragStart/onDragOver/onDrop → hostRpc.trees.handleDrag/handleDrop;
│    onClick: toggle expand + reportExpansion, выставить selection + reportSelection, выполнить node.command)
│   ├── expandable ? i.codicon.codicon-chevron-{down|right}.chevron : span.chevronSpacer
│   ├── {node.checkboxState !== undefined} → span.treeCheckbox (элемент 106)
│   ├── <NodeIcon node expanded /> (элемент 107)
│   ├── span.label {node.label}
│   └── {node.description} → span (инлайн: opacity 0.55; marginLeft "6px"; fontSize "0.85em")
└── {expandable && expanded} → div.children → <TreeLevel parent={node.handle} depth={depth+1} />
```
- expandable = collapsibleState !== 0 (NONE); стартовое expanded = сигнал ?? (collapsibleState === 2 EXPANDED).
- rowDir/rowFile по expandable (не по типу файла).
- reveal-action: scrollIntoView({block:"nearest"}) + focus + expand, затем consume.

## Метрики (ИЗ CSS, точные значения)
Общие с file-tree строками (FileTreeView.module.css):
- `.node`, `.children`: display: contents
- `.row`: display flex; align-items center; gap 6px; width 100%; height 22px; padding-right 8px; box-sizing border-box; background transparent; border 1px solid transparent; border-radius var(--radius-xs); color var(--text-secondary); text-align left; cursor pointer; white-space nowrap; overflow hidden; font inherit; font-size var(--fs-sm)
- padding-left инлайн: depth*12 + 8 px
- `.chevron`/`.chevronSpacer`: flex-shrink 0; font-size 13px; width 16px; text-align center; color var(--text-muted)
- `.label`: flex 1; overflow hidden; text-overflow ellipsis
- description (инлайн): opacity 0.55; margin-left 6px; font-size 0.85em

## Состояния (классы-варианты с метриками)
- `.row:hover`: background color-mix(in srgb, var(--bg-surface) 55%, transparent); color var(--text-primary)
- `.rowSelected`(+ :hover): background linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent)); border-color color-mix(in srgb, var(--accent-primary) 45%, transparent); color var(--text-primary); `.rowSelected .chevron`: color inherit
- `.rowDir`/`.rowFile` — маркеры без CSS-правил
- draggable только при зарегистрированном TreeDragAndDropController (treeDnd[viewId]).
