# 25 project-actions-popover — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (53-84), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<div .actionsPop role="toolbar" aria-label="Project actions" tabIndex={-1}
     style={left/top из clampToViewport(anchor=header, side:"right", offset:4); visibility:hidden до измерения}
     onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
  <button .popAction.add aria-label="New session in this project" data-tooltip="New session here">
    <i .codicon.codicon-add/>
  <button .popAction.delete aria-label="Delete project" data-tooltip="Delete project + its sessions">
    <i .codicon.codicon-trash/>
```
Появление: hover по header группы; позиционируется СПРАВА от header, `POPOVER_OFFSET_PX = 4`. Закрытие: mouseleave, если relatedTarget не header/попап (без таймера — мост через `::before`).

## Метрики (ИЗ CSS, точные значения)
- `.actionsPop`:
  - `position: fixed; z-index: var(--z-dropdown, 1000)`
  - `display: flex; align-items: center; gap: 2px; padding: 3px`
  - `background: var(--bg-surface)`
  - `border: 1px solid var(--divider-soft); border-radius: var(--radius-md)`
  - `box-shadow: var(--shadow-md, 0 4px 16px rgb(0 0 0 / 35%))`
- `.actionsPop::before` (невидимый hover-мост через gap):
  - `content: ""; position: absolute; top: 0; bottom: 0; left: -10px; width: 10px`
- `.popAction`:
  - `display: inline-flex; align-items: center; justify-content: center`
  - `width: 24px; height: 24px; flex-shrink: 0; padding: 0`
  - `background: transparent; border: none; border-radius: var(--radius-xs)`
  - `cursor: pointer; color: var(--text-secondary)`
  - `transition: background var(--transition-fast), color var(--transition-fast)`
- `.popAction .codicon` (`:global`): `font-size: 14px`

## Состояния (классы-варианты с метриками)
- `.popAction:hover`: `background: color-mix(in srgb, var(--text-primary) 12%, transparent); color: var(--text-primary)`
- `.add:hover`: `color: var(--accent-primary)`
- `.delete:hover`: `background: color-mix(in srgb, var(--accent-red) 15%, transparent); color: var(--accent-red)`
- До измерения (`pos == null`): `visibility: hidden`
