# 39 activity-tile — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:82-99`, `ActivityBar.module.css` (`.list`, `.btn`, `.btnActive`, `.btnImage`, `.tileDragging`)

## JSX-структура (кратко, вложенность)
```
<li key={id} data-tile="1" class={dragging ? "tileDragging" : undefined}>
  <button type="button"
          class="btn [btnActive]"
          aria-pressed={isActive}
          aria-label={item.label}
          data-tooltip={item.label}
          onPointerDown={beginActivityDrag(e, slot, id)}   // pointer-drag; pointerup = клик-активация
          onKeyDown={Enter|Space → activateActivity}
          onContextMenu={openActivityContextMenu(slot, id, clientX, clientY)}>
    <ToolIcon icon={item.icon} imageClassName="btnImage"/>   // элемент 51
  </button>
</li>
```
- HTML5 drag не используется (Tauri `dragDropEnabled` его глушит) — pointer-based drag.
- `isActive = !customizeOwnsBar && id === state.active`.

## Метрики (ИЗ CSS, точные значения)
`.btn` (общий селектор `.btn, .picker`):
- `width: 32px; height: 32px`
- `display: grid; place-items: center`
- `background: transparent; border: none`
- `border-radius: var(--radius-sm)`
- `color: var(--text-muted)`
- `font: inherit; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`

Иконка внутри:
- `.btn :global(.codicon)` — `font-size: 18px; line-height: 1`
- `.btn img`, `.btnImage` — `width: 18px; height: 18px; object-fit: contain` (VSIX SVG/PNG; asset как есть, без filter-перекраски)

Контейнер `.list`: `gap: 2px` между плитками (см. элемент 38).

## Состояния (классы-варианты с метриками)
- `.btn:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.btnActive`, `.btnActive:hover`: `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` — иконка остаётся PRIMARY (не акцентная), без кольца/ring.
- `.tileDragging > .btn`: `opacity: 0.3` (тайл-«призрак» на исходной позиции во время drag).
- focus: отдельных стилей в модуле нет.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.btn` НЕТ (`activity-bar/ActivityBar.module.css:53-66`) — бокс ровно 32×32 (`:55-56`), глиф центрируется `display: grid; place-items: center` (`:57-58`); внешние отступы задаёт список: `.list { margin: 0; padding: 0; gap: 2px }` (`ActivityBar.module.css:40-45`), а вертикальный воздух колонки — `.bar { padding: var(--space-3) 0 }` = 12px сверху/снизу (`:10`) и `.bar { gap: var(--space-2) }` = 8 между группами (`:9`)
