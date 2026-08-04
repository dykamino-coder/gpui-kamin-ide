# 94 file-tree-folder-row — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:171-195` (FolderNode), `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`, `file-tree-helpers.tsx` (indentPx)

## JSX-структура (кратко, вложенность)
```
div.node
└── button.row.rowDir[.rowSelected][.dropTarget]
    (style: paddingLeft = indentPx(depth) = depth*12 + 8 px;
     aria-expanded; data-tree-id={path}; draggable={depth > 0};
     onDragStart → beginNativeDrag; onClick → setActiveTreeNode + (Ctrl/Shift-select через applyClickSelection, иначе toggle expand);
     onContextMenu → openFileContextMenu; onKeyDown → onRowKey (Delete/F2/Ctrl+X/C/V))
    ├── i.codicon.chevron — loading ? "codicon-loading codicon-modifier-spin" : expanded ? "codicon-chevron-down" : "codicon-chevron-right" (aria-hidden)
    ├── <TreeIcon className={icon} name type="dir" expanded isRoot={depth===0} />
    ├── span.label (style.color = decorationColor(deco.color) при decoration; data-tooltip = deco.tooltip ?? path) {name}
    └── <RowBadge deco /> (элемент 97)
```

## Метрики (ИЗ CSS, точные значения)
`.node`: display: contents (не создаёт бокс).

`.row`:
- display: flex; align-items: center; gap: 6px
- width: 100%; height: 22px; padding-right: 8px; box-sizing: border-box
- padding-left — инлайн: `depth*12 + 8`px (INDENT_PX=12, BASE_INDENT_PX=8, file-tree-helpers.tsx:14-17)
- background: transparent
- border: 1px solid transparent (зарезервирован, чтобы accent-бордер selected не сдвигал layout)
- border-radius: var(--radius-xs)
- color: var(--text-secondary)
- text-align: left; cursor: pointer; white-space: nowrap; overflow: hidden
- font: inherit; font-size: var(--fs-sm)

`.chevron`:
- flex-shrink: 0; font-size: 13px; width: 16px; text-align: center; color: var(--text-muted)

`.icon` (бокс TreeIcon):
- flex-shrink: 0; width: 16px; height: 16px

`.label`:
- flex: 1; overflow: hidden; text-overflow: ellipsis

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--bg-surface) 55%, transparent); color: var(--text-primary)
- `.rowSelected`, `.rowSelected:hover`:
  - background: linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent))
  - border-color: color-mix(in srgb, var(--accent-primary) 45%, transparent)
  - color: var(--text-primary)
- `.rowSelected .chevron`: color: inherit
- `.dropTarget`, `.dropTarget:hover` (drag файла/папки над строкой папки):
  - background: color-mix(in srgb, var(--accent-primary) 22%, transparent)
  - outline: 1px solid var(--accent-primary); outline-offset: -1px
- Спиннер загрузки: chevron заменяется на `codicon-loading codicon-modifier-spin` (метрики те же, что `.chevron`)
- `.rowDir` — селектора в CSS-модуле нет (класс-маркер без правил)
- `.flash` (при locate): animation: treeFlash 0.9s ease-out 1; @keyframes treeFlash: 0% background color-mix(in srgb, var(--accent-primary) 40%, transparent) → 100% transparent
