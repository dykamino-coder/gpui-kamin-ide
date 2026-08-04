# 62 file-panel-width-handle — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 101-112), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.resizeHandle (+ .resizeHandleActive при drag)  [data-tooltip="Drag to resize"]
  [role="separator"] [aria-orientation="vertical"] [aria-label="Resize file panel"]
  onMouseDown={onWidthDown}
└─ span.resizeHandleBar [aria-hidden="true"]
```
Drag: cursor "col-resize"; `desired = max(FILE_PANEL_MIN_WIDTH_PX=100, startWidth - deltaX)`, затем `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX=100)`; на каждое изменение — синхронный `layoutActiveEditorNow()` (убивает мерцание minimap Monaco). Не рендерится при fill.

## Метрики (ИЗ CSS, точные значения)
### .resizeHandle
- position: absolute; top: 0; left: calc(-1 * var(--space-2)) — целиком в ЛЕВОМ зазоре (между main и file)
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

- отступы: padding/margin нет; геометрия — инсет `position: absolute; top: 0; left: calc(-1 * var(--space-2))` = −8px, `width: var(--space-2)` 8px, `height: 100%` (`FilePanel.module.css:17-29`) — хит-зона целиком в левом 8px-зазоре, ровно как у правой панели (`RightPanel.module.css:31-43`).
