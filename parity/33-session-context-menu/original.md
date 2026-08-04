# 33 session-context-menu — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionContextMenu.tsx` (41-66, 88-92), `SessionContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
// Одно на приложение, монтируется в App.tsx; driven by signal sessionMenu.
<div .menu role="menu" ref
     style={left/top = клик, клампится к viewport с margin 8px (MENU_MARGIN_PX); visibility:hidden до измерения}>
  <button .item role="menuitem"><i .codicon.codicon-edit/> Rename</button>
  {s.open && <button .item><i .codicon.codicon-sparkle/> Auto-rename from chat</button>}   ← exec "claude-bridge.regenerateTitle"
  <button .item><i .codicon.codicon-{pinned-dirty|pin}/> {Unpin from top bar | Pin to top bar}</button>
  {s.open && <button .item><i .codicon.codicon-circle-slash/> Deactivate (free memory)</button>}
  <div .swatches>…</div>                                    ← элемент 34
  <div .divider/>
  <button .item.danger role="menuitem"><i .codicon.codicon-trash/> Delete</button>
</div>
```
Закрытие: mousedown вне (capture) / Escape.

## Метрики (ИЗ CSS, точные значения)
- `.menu`:
  - `position: fixed; z-index: var(--z-titlebar-popover, 10001)` (должно перекрывать титлбар — `--z-dropdown` оставлял меню за таб-стрипом)
  - `min-width: 200px; padding: var(--space-1)`
  - `border-radius: var(--radius-md)`
  - `background: var(--bg-surface); border: 1px solid var(--divider-soft)`
  - `box-shadow: var(--shadow-dropdown, 0 6px 24px rgb(0 0 0 / 30%))`
- `.item`:
  - `display: flex; align-items: center; gap: 8px; width: 100%`
  - `padding: 6px 8px`
  - `border: none; border-radius: var(--radius-sm); background: transparent`
  - `color: var(--text-secondary); font: inherit; font-size: var(--fs-sm); text-align: left`
  - `cursor: pointer`
- `.item .codicon` (`:global`): `font-size: 14px`
- `.divider`: `height: 1px; margin: var(--space-1) 4px; background: var(--divider-soft)`

## Состояния (классы-варианты с метриками)
- `.item:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent); color: var(--text-primary)`
- `.danger`: `color: var(--accent-red)`
- `.danger:hover`: `background: color-mix(in srgb, var(--accent-red) 16%, transparent); color: var(--accent-red)`
- Пункты «Auto-rename from chat» и «Deactivate» — только при `session.open`.
- Pin-иконка: `codicon-pinned-dirty` при pinned, иначе `codicon-pin`.
- До измерения (`pos == null`): `visibility: hidden`.
