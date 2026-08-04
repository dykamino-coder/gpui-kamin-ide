# 59 right-panel-split-handle — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 155-164), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.splitHandle [role="separator"] [aria-orientation="horizontal"]
  [aria-label="Resize right-panel split"] [data-tooltip="Drag to resize"]
  onMouseDown={onSplitDown}
└─ span.splitGrip [aria-hidden="true"]
```
Рендерится только при bottomShown. Drag: cursor "row-resize"; delta по Y / высоту колонки прибавляется к rightPanelSplit, кламп [RIGHT_PANEL_SPLIT_LOWER=0.15, RIGHT_PANEL_SPLIT_UPPER=0.85] (config/constants.ts:89-90).

## Метрики (ИЗ CSS, точные значения)
### .splitHandle
- flex-shrink: 0; height: 10px
- cursor: row-resize
- position: relative; display: flex; align-items: center; justify-content: center
- background: transparent (гейт-фон просвечивает между карточками)
- padding-right: var(--layout-activity-bar-width, 48px) — грип центрируется по карточке, не по колонке (activity bar справа не рассекается)

### .splitGrip
- display: block; width: 32px; height: 3px
- background: var(--bg-overlay)
- border-radius: var(--radius-xs)
- opacity: 0.7
- transition: opacity 0.15s, background 0.15s

## Состояния (классы-варианты с метриками)
- `.splitHandle:hover .splitGrip`: opacity 1; background var(--accent-primary)
- active-класса нет (drag через JS)
