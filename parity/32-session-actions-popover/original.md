# 32 session-actions-popover — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (120-162), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<div .actionsPop role="toolbar" aria-label="Session actions" tabIndex={-1}
     style={left/top из clampToViewport(anchor=row, side:"right", offset:4); visibility:hidden до измерения}
     onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
  <button .popAction.rename data-tooltip="Rename"><i .codicon.codicon-edit/></button>
  {session.open && <button .popAction.disconnect data-tooltip="Disconnect (free from memory)">
      <i .codicon.codicon-debug-disconnect/></button>}
  <button .popAction.delete data-tooltip="Delete session"><i .codicon.codicon-trash/></button>
```
Появление: hover строки; `POPOVER_OFFSET_PX = 4`, clampToViewport избегает нативного browser-вебвью. Закрытие немедленное (без таймера), если relatedTarget не row/попап — hover-мост `::before` делает handoff одним mouseleave.

## Метрики (ИЗ CSS, точные значения)
- `.actionsPop`:
  - `position: fixed; z-index: var(--z-dropdown, 1000)`
  - `display: flex; align-items: center; gap: 2px; padding: 3px`
  - `background: var(--bg-surface)`
  - `border: 1px solid var(--divider-soft); border-radius: var(--radius-md)`
  - `box-shadow: var(--shadow-md, 0 4px 16px rgb(0 0 0 / 35%))`
- `.actionsPop::before` (прозрачный hover-мост через gap слева):
  - `content: ""; position: absolute; top: 0; bottom: 0; left: -10px; width: 10px`
- `.popAction`:
  - `display: inline-flex; align-items: center; justify-content: center`
  - `width: 24px; height: 24px; flex-shrink: 0; padding: 0`
  - `background: transparent; border: none; border-radius: var(--radius-xs)`
  - `cursor: pointer; color: var(--text-secondary)`
- `.popAction > i`: `font-size: 13px`
- transition нет (в отличие от ProjectGroup-версии, где есть).

## Состояния (классы-варианты с метриками)
- `.popAction:hover`: `background: color-mix(in srgb, var(--text-primary) 12%, transparent); color: var(--text-primary)`
- `.popAction.rename:hover`: `color: var(--accent-primary)`
- `.popAction.disconnect:hover`: `color: var(--accent-blue)`
- `.popAction.delete:hover`: `color: var(--accent-red)`
- disconnect-кнопка рендерится только при `session.open`.
- До измерения (`pos == null`): `visibility: hidden`.
