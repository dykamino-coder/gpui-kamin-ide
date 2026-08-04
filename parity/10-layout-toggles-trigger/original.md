# 10 layout-toggles-trigger — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.tsx:165-180
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.module.css:1-33

## JSX-структура (кратко, вложенность)
```
<div class=anchor>                         // relative-обёртка для outside-click
  <button type=button class=trigger aria-haspopup=menu aria-expanded={open}
          aria-label="Layout panels" data-tooltip="Layout panels">
    <i class="fas fa-table-columns" aria-hidden />
  </button>
  {open && портал-меню в <body>}           // элемент 11
</div>
```

## Метрики (ИЗ CSS)
.anchor:
- position: relative; -webkit-app-region: no-drag

.trigger:
- размеры: width: 26px; height: 26px
- отступы: padding: 0
- скругления: border-radius: var(--radius-md)
- шрифт: `.trigger > i { font-size: 13px; line-height: 1; }`
- цвета: color: var(--text-secondary); background: transparent; border: none
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:grid; place-items:center; cursor:pointer

## Состояния
`.trigger[aria-expanded="true"]` (popover открыт):
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)
