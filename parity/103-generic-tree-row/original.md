# 103 generic-tree-row — оригинал
Файлы: `kamin-ide/src/renderer/components/tree/Tree.tsx:63-106` (TreeRow), `kamin-ide/src/renderer/components/tree/Tree.module.css`

## JSX-структура (кратко, вложенность)
```
li [role=treeitem, aria-expanded (только dir), aria-selected]
├── button.row[.selected] (style: paddingLeft = depth*14 px; onClick: dir → onToggle(id), всегда onSelect(node))
│   ├── span.chevron[.chevronHidden если нет детей] (aria-hidden)
│   │   └── i.codicon.codicon-chevron-{down|right}
│   ├── i.codicon.codicon-{node.icon ?? (dir ? "folder" : "file")}.{iconDir|iconFile} (aria-hidden)
│   ├── span.label {node.label}
│   └── {node.meta} → span.meta
└── {isOpen && children} → ul.subtree [role=group] → рекурсивные TreeRow (depth+1)
```
INDENT_PX = 14 (Tree.tsx:56).

## Метрики (ИЗ CSS, точные значения)
`.row`:
- display: inline-flex; align-items: center; gap: var(--space-2); width: 100%
- padding: 4px var(--space-2) (+ инлайн paddingLeft = depth*14 px); box-sizing: border-box
- background: transparent; border: 1px solid transparent (резерв под selected-бордер); border-radius: var(--radius-xs)
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer
- transition: background var(--transition-fast)

`.chevron`:
- width: 14px; display: grid; place-items: center; font-size: 10px; color: var(--text-muted); flex-shrink: 0

`.iconDir`: color: var(--accent-yellow); flex-shrink: 0; font-size: var(--fs-sm)
`.iconFile`: color: var(--text-muted); flex-shrink: 0; font-size: var(--fs-sm)

`.label`: flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap

`.meta`: font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-muted); flex-shrink: 0

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--bg-surface) 55%, transparent)
- `.row.selected`, `.row.selected:hover`:
  - background: linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent))
  - border-color: color-mix(in srgb, var(--accent-primary) 45%, transparent)
  - color: var(--text-primary)
- `.chevronHidden`: visibility: hidden (лист/пустой dir — место сохраняется)
- chevron: codicon-chevron-down (открыт) ↔ codicon-chevron-right (закрыт)
