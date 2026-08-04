# 43 activity-picker-open-tool-pill — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityPicker.tsx:115-125`, `kamin-ide/src/renderer/components/panel-placeholder/PanelPlaceholder.module.css` (`.trigger`)

## JSX-структура (кратко, вложенность)
```
<div class="pickerAnchorInline" ref={anchorRef}>       // ActivityBar.module.css
  <button type="button"
          class="trigger"                              // PanelPlaceholder.module.css
          aria-haspopup="listbox"
          aria-expanded={open}
          onClick={stopPropagation; toggle open}>
    <span>Open Tool</span>
    <i class="fas fa-chevron-down" aria-hidden="true"/>   // FontAwesome
  </button>
  {menu}   // тот же портал-listbox (элемент 44)
</div>
```
- variant="openTool"; используется в PanelPlaceholder (пустая панель). Обёртка inline (`pickerAnchorInline`), чтобы родитель управлял вертикальным размещением.

## Метрики (ИЗ CSS, точные значения)
`.trigger`:
- `display: inline-flex; align-items: center; gap: var(--space-2)`
- `padding: var(--space-1) var(--space-3)` (вертикаль var(--space-1), горизонталь var(--space-3))
- `background: color-mix(in srgb, var(--accent-primary) 16%, transparent)`
- `color: var(--text-primary)` (текст PRIMARY, не accent — accent-on-transparent читался блёкло)
- `border: none; border-radius: var(--radius-sm)`
- `font-size: var(--fs-sm)`
- `margin-top: var(--space-1)`
- `transition: background var(--transition-fast)`
- `.trigger > i { font-size: 10px }` (шеврон)

## Состояния (классы-варианты с метриками)
- `.trigger:hover`: `background: color-mix(in srgb, var(--accent-primary) 26%, transparent)`
- Открытое меню: только `aria-expanded="true"`, визуального класса нет.
