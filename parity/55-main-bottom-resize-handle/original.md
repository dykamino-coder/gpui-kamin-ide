# 55 main-bottom-resize-handle — оригинал
Файлы: kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.tsx (строки 64-73), kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.resizeHandle [role="separator"] [aria-orientation="horizontal"]
  [aria-label="Resize Left Bottom"] [data-tooltip="Drag to resize"]
  onMouseDown={onResizeDown}
└─ span.resizeHandleBar [aria-hidden="true"]
```
Drag: cursor "row-resize" через useDragHandler; delta по Y / высоту колонки прибавляется к mainSplit, кламп [0.2, 0.85]. Если `!mainVisible` — drag не начинается.

## Метрики (ИЗ CSS, точные значения)
### .resizeHandle
- flex-shrink: 0; height: 10px; width: 100%
- cursor: row-resize
- display: flex; align-items: center; justify-content: center
- position: relative; user-select: none
- background: transparent; border: none; padding: 0; color: inherit; font: inherit
- `:focus { outline: none; }`

### .resizeHandleBar (грип)
- display: block; width: 32px; height: 3px
- background: var(--bg-overlay)
- border-radius: var(--radius-xs)
- opacity: 0.7
- transition: opacity 0.15s, background 0.15s
- pointer-events: none

## Состояния (классы-варианты с метриками)
- `.resizeHandle:hover .resizeHandleBar`: opacity 1; background var(--accent-primary)
- focus: outline: none
- active-класса нет (drag через JS)
