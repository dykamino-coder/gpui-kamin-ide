# 28 session-item-row — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (88-119), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
<div .row [.active][.tinted][.inactive] ref={rowRef}
     style="--tab-color: resolveSessionColor(session.color ?? var(--accent-primary))"
     role="button" tabIndex={0}
     onClick={activateSession} onDblClick={beginRename}
     onContextMenu={openSessionMenu(x,y)} onKeyDown={F2→beginRename}
     onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
  <span .dot data-bridge={status} data-tooltip={statusTip}/>   ← элемент 29
  <span .label>{session.name}</span>
  <span .time data-tooltip={absoluteTime}>{relativeTime(session.lastOpened)}</span>
  <button .action.pin[.pinned]>…</button>                      ← элемент 30
  {showActions && createPortal(<div .actionsPop …/>)}          ← элемент 32
</div>
```
`tinted = !!session.color`; `.inactive` при `!session.open`. В режиме rename рендерится вариант `.row.editing` (элемент 31).

## Метрики (ИЗ CSS, точные значения)
- `.row`:
  - `--tab-color: var(--accent-primary)` (дефолт; переопределяется inline)
  - `display: flex; align-items: center; gap: var(--space-2); width: 100%`
  - `height: 24px; box-sizing: border-box`
  - `padding: 0 8px 0 16px` (right 8 / left 16)
  - `border: 1px solid transparent; border-radius: var(--radius-xs)`
  - `color: var(--text-secondary); font-size: var(--fs-sm); text-align: left`
  - `cursor: pointer; white-space: nowrap; overflow: hidden`
- `.label`: `flex: 1; overflow: hidden; text-overflow: ellipsis`
- `.time`:
  - `flex-shrink: 0; margin-left: auto`
  - `font-size: var(--fs-xs); font-weight: 600; color: var(--text-muted); opacity: 0.7; white-space: nowrap`
- transition на самой строке нет.

## Состояния (классы-варианты с метриками)
- `.row:hover` (не-selected): `background: color-mix(in srgb, var(--bg-surface) 55%, transparent); color: var(--text-primary)`
- `.tinted` (цветная, не активная): `background: linear-gradient(90deg, color-mix(in srgb, var(--tab-color) 24%, transparent), color-mix(in srgb, var(--tab-color) 13%, transparent))`
- `.tinted:hover`: то же с 30% / 17%
- `.active, .active:hover`:
  - `background: linear-gradient(90deg, color-mix(in srgb, var(--tab-color) 26%, transparent), color-mix(in srgb, var(--tab-color) 14%, transparent))`
  - `border-color: color-mix(in srgb, var(--tab-color) 45%, transparent)`
  - `color: var(--text-primary)`
- Light theme (`[data-theme="light"]`):
  - `.tinted`: 26% / 16%; `.tinted:hover`: 34% / 22%
  - `.active`: gradient 42% / 26%, `border-color` 60%
- `.inactive`: `opacity: 0.6`; `.inactive:hover`: `opacity: 1`; light theme: `opacity: 0.8`
- `.row:hover .action`: `display: inline-flex; opacity: 0.7` (кнопки-экшены появляются на hover)
