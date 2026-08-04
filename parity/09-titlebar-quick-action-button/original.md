# 09 titlebar-quick-action-button — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.tsx:54-66
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.module.css:9-33

## JSX-структура (кратко, вложенность)
```
<button type=button class={btn [active]} data-tooltip={title} aria-label={title}
        aria-pressed={active ?? false}>
  {children}   // PanelIcon slot="left" ЛИБО <i class="fas fa-gear">
</button>
```

## Метрики (ИЗ CSS)
.btn:
- размеры: width: var(--layout-icon-button-round); height: var(--layout-icon-button-round)
- отступы: не заданы
- скругления: border-radius: var(--radius-sm)
- шрифт: не задан; `.btn :global(.codicon) { font-size: 14px !important; }`
- цвета: color: var(--text-secondary); background: transparent
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:inline-flex; align-items:center; justify-content:center

## Состояния
.active:
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)
(при hover активной кнопки: `.btn:hover` специфичнее (0,2,0 против 0,1,0 у `.active`) → фон на hover = var(--bg-surface))
