# 95 file-tree-file-row — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:228-253` (FileLeaf), `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`, `file-tree-helpers.tsx` (indentPx)

## JSX-структура (кратко, вложенность)
```
button.row.rowFile[.rowSelected]
  (style: paddingLeft = depth*12 + 8 px; data-tree-id={path}; draggable (всегда);
   onDragStart → beginNativeDrag; onClick → setActiveTreeNode + (Ctrl/Shift-select, иначе openFile);
   onContextMenu → openFileContextMenu; onKeyDown → onRowKey)
├── span.chevronSpacer (aria-hidden) — выравнивающий спейсер вместо chevron
├── <TreeIcon className={icon} name type="file" />
├── span.label (style.color = decorationColor(deco.color) при decoration; data-tooltip = deco.tooltip ?? path) {name}
└── <RowBadge deco />
```
Selected — только из explorer-selection (синхронизирована с активным файлом редактора).

## Метрики (ИЗ CSS, точные значения)
`.row` — как у 94:
- display: flex; align-items: center; gap: 6px; width: 100%; height: 22px
- padding-right: 8px; padding-left инлайн `depth*12 + 8`px; box-sizing: border-box
- background: transparent; border: 1px solid transparent; border-radius: var(--radius-xs)
- color: var(--text-secondary); text-align: left; cursor: pointer; white-space: nowrap; overflow: hidden
- font: inherit; font-size: var(--fs-sm)

`.chevronSpacer` (общее правило с `.chevron`):
- flex-shrink: 0; font-size: 13px; width: 16px; text-align: center; color: var(--text-muted)

`.icon`: flex-shrink: 0; width: 16px; height: 16px

`.label`: flex: 1; overflow: hidden; text-overflow: ellipsis

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--bg-surface) 55%, transparent); color: var(--text-primary)
- `.rowSelected`, `.rowSelected:hover`: background: linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent)); border-color: color-mix(in srgb, var(--accent-primary) 45%, transparent); color: var(--text-primary)
- `.rowFile` — класс-маркер без CSS-правил
- `.flash` (locate): animation: treeFlash 0.9s ease-out 1 (40% accent → transparent)
