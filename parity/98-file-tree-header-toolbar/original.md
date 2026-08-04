# 98 file-tree-header-toolbar — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeHeader.tsx:26-77`, `kamin-ide/src/renderer/components/file-tree/FileTreeHeader.module.css`, `FileTreeView.module.css` (`.flash` — flashRow)

## JSX-структура (кратко, вложенность)
```
header.header
├── span.title (data-tooltip = root) {folderName ?? "PROJECT"}   // последняя часть пути
├── {indexing.value} → span.indexing (data-tooltip="Building the search index (Ctrl+P)…")
│   ├── i.codicon.codicon-loading.codicon-modifier-spin (aria-hidden)
│   └── "Indexing…"
└── div.actions
    ├── button.btn [aria-label/data-tooltip "Locate selected file"; disabled = !root || !selectedFile]
    │   └── i.codicon.codicon-target
    ├── button.btn [collapsed ? "Expand all folders" : "Collapse all folders"; disabled = !root; onClick toggleCollapseAll]
    │   └── i.codicon.{codicon-expand-all | codicon-collapse-all}
    └── button.btn [aria-label/data-tooltip "Refresh"; disabled = !root; onClick: workspaceFolder → null → queueMicrotask восстановить (полный ремаунт)]
        └── i.codicon.codicon-refresh
```
Locate: revealTarget.value = path → каскадное раскрытие предков; поллинг `[data-tree-id]` каждые 50мс до 60 попыток; найдя — scrollIntoView({block:"center", behavior:"smooth"}) + класс `.flash` на 900мс (SCROLL_FLASH_MS).

## Метрики (ИЗ CSS, точные значения)
`.header`:
- display: flex; align-items: center; gap: var(--space-1)
- padding: 8px 8px 8px 12px; flex-shrink: 0

`.title`:
- flex: 1; font-size: var(--fs-xs); font-weight: 500; letter-spacing: 0.08em
- color: var(--text-muted); font-feature-settings: "ss01"
- overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-transform: uppercase

`.indexing`:
- display: inline-flex; align-items: center; gap: 4px; flex-shrink: 0
- font-size: var(--fs-xs); color: var(--text-muted); opacity: 0.85
- `.indexing :global(.codicon)`: font-size: 12px

`.actions`: display: inline-flex; align-items: center; gap: 2px

`.btn`:
- width: 22px; height: 22px; display: grid; place-items: center
- background: transparent; border: none; color: var(--text-muted)
- border-radius: var(--radius-xs); cursor: pointer
- transition: background var(--transition-fast), color var(--transition-fast)
- `.btn :global(.codicon)`: font-size: 14px

`.flash` (в FileTreeView.module.css): animation: treeFlash 0.9s ease-out 1; keyframes: 0% background color-mix(in srgb, var(--accent-primary) 40%, transparent) → 100% transparent

## Состояния (классы-варианты с метриками)
- `.btn:hover:not([disabled])`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.btn[disabled]`: opacity: 0.4; cursor: not-allowed
- Collapse/Expand — одна кнопка, иконка и подписи переключаются по `treeAllCollapsed`.
