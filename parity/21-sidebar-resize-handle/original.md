# 21 sidebar-resize-handle — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\Sidebar.tsx` (27-49, 64-73), `Sidebar.module.css`

## JSX-структура (кратко, вложенность)
```
<div .resizeHandle [+ .resizeHandleActive при drag]
     data-tooltip="Drag to resize"
     role="separator" aria-orientation="vertical" aria-label="Resize sidebar"
     onMouseDown={начало drag}>
  <span .resizeHandleBar aria-hidden="true"/>
</div>
```
Логика drag: `useDragHandler().begin({cursor:"col-resize"})`; `desired = max(SIDEBAR_MIN_WIDTH_PX, clientX - leftX)`; затем `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX)` — рост ограничен min-width центральной колонки. Ref ресинкается с сигналом перед drag (иначе jump).

## Метрики (ИЗ CSS, точные значения)
- `.resizeHandle`:
  - `position: absolute; top: 0; right: calc(-1 * var(--space-2))` — сидит ЦЕЛИКОМ в gap `--space-2` справа от сайдбара
  - `width: var(--space-2); height: 100%`
  - `cursor: col-resize`
  - `z-index: var(--z-resize-handle)`
  - `user-select: none; pointer-events: auto`
  - `display: flex; align-items: stretch; justify-content: center` (грип центрирован в gap)
- `.resizeHandleBar`:
  - `display: block; width: 2px; height: 100%`
  - `opacity: 0` (невидим в покое)
  - `background: linear-gradient(to bottom, transparent 0%, var(--bg-overlay) 30%, var(--bg-overlay) 70%, transparent 100%)`
  - `transition: opacity 0.15s, background 0.15s, width 0.15s`
  - `pointer-events: none`

## Состояния (классы-варианты с метриками)
- `.resizeHandle:hover .resizeHandleBar` и `.resizeHandleActive .resizeHandleBar` (во время drag):
  - `opacity: 1; width: 3px`
  - `background: linear-gradient(to bottom, transparent 0%, var(--tint-primary-strong) 30%, var(--tint-primary-strong) 70%, transparent 100%)`

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin НЕТ. Позиционирование — absolute: top 0, right calc(-1 * var(--space-2)) = −8px, width var(--space-2) = 8px, height 100% (`sidebar/Sidebar.module.css:19-25`) — хит целиком в 8-пиксельном зазоре `.body`, без захода на кромки соседей (комментарий `Sidebar.module.css:15-18`); грип центрируется `justify-content: center` (`:31`), ширина полосы 2px в покое → 3px по hover/active (`Sidebar.module.css:35,52`)
