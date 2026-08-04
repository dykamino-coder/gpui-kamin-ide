# 45 activity-context-menu — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityContextMenu.tsx:132-169`, `ActivityContextMenu.module.css` (`.menu`, `.item`, `.itemLabel`, `.itemMoveTo`, `.chevron`)

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<div ref={rootRef} class="menu" role="menu"
     style="left:{pos.left}px; top:{pos.top}px; visibility: visible|hidden">
  <button type="button" role="menuitem" class="item"
          onMouseEnter={closeSubmenu}
          onClick={unpinFromPanel; close}>
    <i class="codicon codicon-eye-closed"/>
    <span class="itemLabel">Hide</span>
  </button>
  <button type="button" role="menuitem" class="item itemMoveTo"
          aria-haspopup="menu" aria-expanded={submenuOpen}
          onMouseEnter={openSubmenu} onClick={toggleSubmenu}>
    <i class="codicon codicon-arrow-right"/>
    <span class="itemLabel">Move to</span>
    <i class="codicon codicon-chevron-right chevron"/>
  </button>
  {submenuOpen && createPortal(<ul class="submenu">…</ul>, body)}   // элемент 46
</div>
```
- Открывается у курсора (anchor = точка x/y нулевого размера), `clampToViewport(side:"bottom", offset: MENU_OFFSET_PX = 0)`; `visibility:hidden` до первого замера.
- Закрытие: outside mousedown (capture), Escape, любой scroll (capture), window blur.

## Метрики (ИЗ CSS, точные значения)
`.menu` (общий селектор `.menu, .submenu`):
- `position: fixed; z-index: var(--z-dropdown)`
- `min-width: 180px`
- `background: var(--bg-surface)`
- `border: 1px solid var(--divider-soft)`
- `border-radius: var(--radius-md)`
- `box-shadow: var(--shadow-dropdown)`
- `list-style: none; margin: 0; padding: var(--space-1)`
- `display: flex; flex-direction: column; gap: 1px`
- `max-height: calc(100vh - 16px); max-width: calc(100vw - 16px); overflow-y: auto`

`.item`:
- `display: flex; align-items: center; gap: var(--space-2); width: 100%`
- `padding: var(--space-2) var(--space-3)`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-primary); font: inherit; font-size: var(--fs-sm)`
- `text-align: left; cursor: pointer`

`.itemLabel`: `flex: 1`
`.chevron`: `font-size: 12px; color: var(--text-muted)`

## Состояния (классы-варианты с метриками)
- `.item:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.itemMoveTo[aria-expanded="true"]` (сабменю открыто): `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` — «хлебная крошка» пока открыто сабменю.
- transition/анимаций нет.
