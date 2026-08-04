# 64 file-panel-split-handle — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 133-142), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.splitHandle [role="separator"] [aria-orientation="horizontal"]
  [aria-label="Resize bottom pane"] [data-tooltip="Drag to resize"]
  onMouseDown={onSplitDown}
└─ span.splitGrip [aria-hidden="true"]
```
Рендерится только при `filePanelBottomVisible`. Drag: cursor "row-resize"; `next = max(BOTTOM_PANE_MIN_HEIGHT_PX=100, startHeight - deltaY)` → filePanelBottomHeight (пиксели, не ratio) + `layoutActiveEditorNow()`.

## Метрики (ИЗ CSS, точные значения)
### .splitHandle
- flex-shrink: 0; height: 10px
- cursor: row-resize
- position: relative; display: flex; align-items: center; justify-content: center
- background: transparent (гейт-фон просвечивает)
- padding-right НЕТ (в отличие от RightPanel.splitHandle — тут нет activity bar сбоку)

### .splitGrip
- display: block; width: 32px; height: 3px
- background: var(--bg-overlay)
- border-radius: var(--radius-xs)
- opacity: 0.7
- transition: opacity 0.15s, background 0.15s

## Состояния (классы-варианты с метриками)
- `.splitHandle:hover .splitGrip`: opacity 1; background var(--accent-primary)
- active-класса нет (drag через JS)
