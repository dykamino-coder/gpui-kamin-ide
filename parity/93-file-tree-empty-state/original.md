# 93 file-tree-empty-state — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:40-53`, `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root (+ className)
├── <FileTreeHeader />
└── div.empty
    ├── i.codicon.codicon-folder.emptyIcon (aria-hidden)
    ├── p.emptyHint "No active session with a folder."
    └── p.emptyHint "Pick a session in Projects, or start one with a folder."
```
Рендерится, когда `workspaceFolder.value` = null.

## Метрики (ИЗ CSS, точные значения)
`.empty`:
- flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center
- gap: var(--space-2); padding: var(--space-5)
- text-align: center; color: var(--text-muted)

`.emptyIcon`:
- font-size: 32px; color: var(--text-disabled)

`.emptyHint`:
- margin: 0; font-size: var(--fs-sm)

В CSS есть также `.openBtn` (кнопка «Open Folder»: margin-top var(--space-2); padding 6px 14px; background var(--accent-primary); color var(--accent-action-fg); border 1px solid var(--accent-primary); border-radius var(--radius-sm); font-size var(--fs-sm); font-weight 600; transition background var(--transition-fast); hover: background/border-color var(--accent-action-hover)) — в текущем JSX empty-состояния кнопка НЕ рендерится (класс не используется в .tsx).

## Состояния (классы-варианты с метриками)
- Нет hover/active-вариантов; статичный блок.
