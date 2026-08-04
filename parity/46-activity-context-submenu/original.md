# 46 activity-context-submenu — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityContextMenu.tsx:171-204`, `ActivityContextMenu.module.css` (`.submenu`, `.subItem`, `.subItemIcon`, `.subItemLabel`)

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<ul ref={submenuRef} class="submenu" role="menu"
    style="left:{subPos.left}px; top:{subPos.top}px; visibility: visible|hidden">
  {targets.map(e =>            // SLOT_ENTRIES минус текущий slot
    <li key={e.slot}>
      <button type="button" role="menuitem" class="subItem"
              onClick={moveActivity(state.slot, id, e.slot, MAX_SAFE_INTEGER); close}>
        <span class="subItemIcon"><PanelIcon slot={e.icon}/></span>
        <span class="subItemLabel">{e.label}</span>
      </button>
    </li>)}
</ul>
```
- SLOT_ENTRIES (порядок и подписи): sidebar→"Sidebar" (icon left), main→"Left" (main), mainBottom→"Left Bottom" (main-bottom), centralBottom→"Center Bottom" (center-bottom), rightTop→"Right" (right-top), rightBottom→"Right Bottom" (right-bottom). `centralTop` исключён намеренно.
- Позиционирование: anchor = rect строки `.itemMoveTo`, `clampToViewport(side:"right", offset: 4)`.
- Move = append в конец целевой панели (тот же путь, что DnD-drop на пустой бар).

## Метрики (ИЗ CSS, точные значения)
`.submenu` — идентично `.menu` элемента 45 (общий селектор):
- `position: fixed; z-index: var(--z-dropdown); min-width: 180px`
- `background: var(--bg-surface); border: 1px solid var(--divider-soft)`
- `border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)`
- `list-style: none; margin: 0; padding: var(--space-1)`
- `display: flex; flex-direction: column; gap: 1px`
- `max-height: calc(100vh - 16px); max-width: calc(100vw - 16px); overflow-y: auto`

`.subItem` — идентично `.item`:
- `display: flex; align-items: center; gap: var(--space-2); width: 100%`
- `padding: var(--space-2) var(--space-3)`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer`

`.subItemIcon`: `display: inline-flex; align-items: center; justify-content: center; color: var(--text-muted)` (внутри — `PanelIcon`, титлбарный размер 14×12)
`.subItemLabel`: `flex: 1`

## Состояния (классы-варианты с метриками)
- `.subItem:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent)`
- transition/анимаций нет.
