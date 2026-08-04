# 07 titlebar-button — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarButton.tsx:30-35
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarButton.module.css

## JSX-структура (кратко, вложенность)
```
<button type=button class={btn [close|devtools]} data-tooltip={label} aria-label={label}>
  <i class={"codicon codicon-<name>" | "fas fa-<name>"} aria-hidden />
  {variant==="devtools" && <span class=devtoolsLabel>DevTools</span>}
</button>
```

## Метрики (ИЗ CSS)
.btn (default):
- размеры: width: var(--layout-icon-button-titlebar); height: var(--layout-icon-button-titlebar)
- отступы: margin: 0 var(--space-1); padding не задан
- скругления: border-radius: 50%
- шрифт: не задан на кнопке
- иконка `.btn > i`: inline-flex центр; width 16px; height 16px; font-size 13px; line-height 1
- цвета: color: var(--text-muted); background не задан (прозрачный)
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:inline-flex; align-items:center; justify-content:center; -webkit-app-region: no-drag

## Состояния
.devtools (variant="devtools"):
- width: auto; padding: 0 var(--space-3); gap: var(--space-1); border-radius: var(--radius-md)
- hover: color: var(--accent-primary) (фон — от базового .btn:hover: var(--bg-surface))
- .devtoolsLabel: font-size: var(--fs-sm)

.close (variant="close"):
- hover: background: var(--accent-red); color: var(--bg-primary)
