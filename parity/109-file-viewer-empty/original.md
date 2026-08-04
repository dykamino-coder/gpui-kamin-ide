# 109 file-viewer-empty — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewer.tsx` (81-88), `FileViewer.module.css` (44-70)

## JSX-структура (кратко, вложенность)
```
div.empty
├─ i.codicon.codicon-file [aria-hidden]
└─ p: "Pick a file from the tree, or press <kbd>Ctrl+P</kbd> to open one by name."
```

## Метрики (ИЗ CSS, точные значения)
`.empty`:
- flex: 1; display: flex; flex-direction: column
- align-items: center; justify-content: center
- gap: var(--space-2)
- padding: var(--space-5)
- color: var(--text-muted); text-align: center

`.empty .codicon` (глиф файла):
- font-size: 36px
- color: var(--text-disabled)

`.empty kbd`:
- display: inline-block
- padding: 2px 6px
- background: var(--bg-surface); color: var(--text-primary)
- border-radius: var(--radius-xs)
- font-family: var(--font-mono); font-size: var(--fs-xs)
- border: 1px solid color-mix(in srgb, var(--text-muted) 30%, transparent)

## Состояния (классы-варианты с метриками)
Одно статическое состояние; hover/active/transition отсутствуют.
