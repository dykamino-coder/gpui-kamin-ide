# 57 right-panel-width-handle — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 113-124), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.resizeHandle (+ .resizeHandleActive при drag)  [data-tooltip="Drag to resize"]
  [role="separator"] [aria-orientation="vertical"] [aria-label="Resize right panel"]
  onMouseDown={onWidthDown}
└─ span.resizeHandleBar [aria-hidden="true"]
```
Drag: cursor "col-resize"; drag влево растит правую панель. Если File-панель видима — торг между Right и File (кламп RIGHT_PANEL_MIN_WIDTH_PX=100 / FILE_PANEL_MIN_WIDTH_PX=100, вызывает `layoutActiveEditorNow()`); иначе рост против центра через `clampGrowth(..., MAIN_MIN_WIDTH_PX=100)`. Не рендерится при fill.

## Метрики (ИЗ CSS, точные значения)
### .resizeHandle
- position: absolute; top: 0; left: calc(-1 * var(--space-2)) — целиком в ЛЕВОМ зазоре
- width: var(--space-2); height: 100%
- cursor: col-resize
- z-index: var(--z-resize-handle)
- user-select: none; display: flex; align-items: stretch; justify-content: center

### .resizeHandleBar
- display: block; width: 2px; height: 100%
- opacity: 0
- background: `linear-gradient(to bottom, transparent 0%, var(--bg-overlay) 30%, var(--bg-overlay) 70%, transparent 100%)`
- transition: opacity 0.15s, background 0.15s, width 0.15s
- pointer-events: none

## Состояния (классы-варианты с метриками)
- `.resizeHandle:hover .resizeHandleBar` и `.resizeHandleActive .resizeHandleBar` (drag):
  - opacity: 1; width: 3px
  - background: `linear-gradient(to bottom, transparent 0%, var(--tint-primary-strong) 30%, var(--tint-primary-strong) 70%, transparent 100%)`

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin нет; вся геометрия — инсет `position: absolute; top: 0; left: calc(-1 * var(--space-2))` = −8px при `width: var(--space-2)` 8px, `height: 100%` (`RightPanel.module.css:31-43`), т.е. хит-зона целиком лежит в 8px-зазоре слева от колонки.
