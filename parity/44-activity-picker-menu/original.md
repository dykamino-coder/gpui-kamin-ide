# 44 activity-picker-menu — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityPicker.tsx:140-174`, `ActivityBar.module.css` (`.menu`, `.menuPortal`, `.menuLabel`, `.menuItem`, `.menuItemImage`, `.menuLabelText`)

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<ul ref={menuRef} class="menu menuPortal" role="listbox"
    style="left:{pos.left}px; top:{pos.top}px; visibility: visible|hidden">
  <li class="menuLabel">Tools</li>
  {activityRegistry.map(it =>
    <li key={it.id}>
      <button type="button" class="menuItem"
              onClick={isPinned ? unpinFromPanel : pinToPanel; close}>
        <ToolIcon icon={it.icon} imageClassName="menuItemImage"/>
        <span class="menuLabelText">{it.label}</span>
        {isPinned && <i class="codicon codicon-check" aria-hidden="true"/>}
      </button>
    </li>)}
</ul>
```
- Позиционирование: измерение trigger rect + menu rect → `clampToViewport({side: popDirection==="up" ? "top" : "bottom", offset: 6})` — flip+shift, чтобы не вылезло за окно. Стартует `visibility:hidden`, показывается после первого замера (useLayoutEffect). Re-measure на window resize и scroll (capture).
- Закрытие: outside mousedown (capture, с проверкой menuRef — портал не потомок anchor) + Escape.

## Метрики (ИЗ CSS, точные значения)
`.menu`:
- `min-width: 220px`
- `background: var(--bg-surface)`
- `border: 1px solid var(--divider-soft)`
- `border-radius: var(--radius-md)`
- `box-shadow: var(--shadow-dropdown)`
- `list-style: none; margin: 0; padding: var(--space-1)`
- `z-index: var(--z-dropdown)`
- `display: flex; flex-direction: column; gap: 1px`

`.menuPortal`:
- `position: fixed`
- `max-height: calc(100vh - 16px); max-width: calc(100vw - 16px)`
- `overflow-y: auto`

`.menuLabel` (заголовок «Tools»):
- `padding: var(--space-1) var(--space-3)`
- `font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em`
- `color: var(--text-muted)`

`.menuItem`:
- `display: flex; align-items: center; gap: var(--space-2); width: 100%`
- `padding: var(--space-2) var(--space-3)`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-primary); font: inherit; font-size: var(--fs-sm)`
- `text-align: left; cursor: pointer`

`.menuItemImage` (img-ветка ToolIcon): `width: 18px; height: 18px; object-fit: contain`
`.menuLabelText`: `flex: 1`

## Состояния (классы-варианты с метриками)
- `.menuItem:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent)`
- Запиненный пункт: галка `codicon-check` в конце строки (спец-стилей нет, наследует цвет пункта).
- transition/анимаций появления нет.
