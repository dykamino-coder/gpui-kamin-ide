# 49 bottom-tab — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/BottomTabBar.tsx:48-64`, `BottomTabBar.module.css` (`.tab`, `.tabActive`, `.tabDragging`, `.tabImage`, `.tabLabel`)

## JSX-структура (кратко, вложенность)
```
<button type="button" data-tab="1"
        class="tab [tabActive] [tabDragging]"
        aria-pressed={isActive}
        aria-label={item.label}
        data-tooltip={item.label}
        onPointerDown={beginActivityDrag(e, slot, id)}
        onKeyDown={Enter|Space → activateActivity}
        onContextMenu={openActivityContextMenu(slot, id, x, y)}>
  <ToolIcon icon={item.icon} size={TAB_ICON_SIZE_PX} imageClassName="tabImage"/>  // TAB_ICON_SIZE_PX = 13
  <span class="tabLabel">{item.label}</span>
</button>
```

## Метрики (ИЗ CSS, точные значения)
`.tab`:
- `display: inline-flex; align-items: center; gap: 6px`
- `padding: 4px 10px; height: 24px`
- `background: transparent; border: none`
- `border-radius: var(--radius-sm)`
- `color: var(--text-secondary)`
- шрифт: `font-size: 11px; font-weight: 500; letter-spacing: 0.02em` (family/line-height не заданы — наследуются)
- `white-space: nowrap; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`

Иконка:
- `.tab :global(.codicon)` — `font-size: 13px; line-height: 1`
- `.tabImage` — `width: 13px; height: 13px; object-fit: contain`
- SVG-ветка ToolIcon получает `size=13` пропом (TAB_ICON_SIZE_PX, экспортируется для Design-panel sample)

`.tabLabel`: `overflow: hidden; text-overflow: ellipsis; min-width: 0`

## Состояния (классы-варианты с метриками)
- `.tab:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.tabActive`, `.tabActive:hover`: `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` — без кольца.
- `.tabDragging`: `opacity: 0.3`
- focus: отдельных стилей нет.
