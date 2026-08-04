# 16 theme-popover-item — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:130-152
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:116-157

## JSX-структура (кратко, вложенность)
```
<button type=button class="item [picked]" role=option aria-selected={picked ?? false}>
  <i class="fas {fa-moon|fa-sun|fa-icons} itemIcon" aria-hidden />
  <span class=itemName>{name}</span>
  <i class="fas fa-check itemTick" style={visibility: picked ? visible : hidden} aria-hidden />
</button>
```
Галка ВСЕГДА в DOM (visibility-toggle, не conditional) — резервирует ширину,
чтобы max-content popover не прыгал при смене пика.

## Метрики (ИЗ CSS)
.item:
- размеры: width: 100%
- отступы: padding: var(--space-2) var(--space-3); gap: var(--space-2)
- скругления: border-radius: var(--radius-sm)
- шрифт: font-size: var(--fs-sm); text-align: left
- цвета: background: transparent; color: var(--text-primary)
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent)
- transition: нет
- позиционирование: display:flex; align-items:center

.itemIcon:
- width: 16px; font-size: 12px; text-align: center; flex-shrink: 0

.itemName:
- flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis

.itemTick:
- width: 12px (фикс-слот); flex-shrink: 0; text-align: center
- font-size: 10px; color: var(--accent-primary)

## Состояния
.picked (+ .picked:hover):
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)
- .itemTick visible (inline visibility)
