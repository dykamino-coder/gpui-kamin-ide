# 69 activity-placeholder — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/ActivityPlaceholder.tsx (строки 21-27), kamin-ide/src/renderer/components/panel-placeholder/ActivityPlaceholder.module.css

## JSX-структура (кратко, вложенность)
```
div.placeholder
├─ <ToolIcon icon={icon} size={36} className=glyph />   (GLYPH_SIZE_PX = 36)
├─ h2.label  {label}
└─ p.hint    "Nothing to show here yet."
```
Отличие от PanelPlaceholder: пустое тело УЖЕ выбранной активности — пикера «Open Tool» намеренно нет.

## Метрики (ИЗ CSS, точные значения)
### .placeholder
- flex: 1; display: flex; flex-direction: column
- align-items: center; justify-content: center; text-align: center
- gap: var(--space-2)
- padding: var(--space-5)
- color: var(--text-muted)

### .glyph
- font-size: 36px
- color: var(--text-disabled)
- margin-bottom: var(--space-1)

### .label
- margin: 0; font-size: var(--fs-md); font-weight: 600; color: var(--text-primary)
  (у PanelPlaceholder — fs-lg; здесь на ступень меньше)

### .hint
- margin: 0; font-size: var(--fs-xs); color: var(--text-muted)
- line-height: var(--lh-snug); max-width: 240px

## Состояния (классы-варианты с метриками)
- вариантов/hover/transition нет — статичный empty-state
