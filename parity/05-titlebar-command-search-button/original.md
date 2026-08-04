# 05 titlebar-command-search-button — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:43-51
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:83-108

## JSX-структура (кратко, вложенность)
```
<button class=searchButton aria-label="Open command palette (Ctrl+Shift+P)"
        data-tooltip="Open command palette (Ctrl+Shift+P)"
        onClick=execute("workbench.action.showCommands")>
  <span class="codicon codicon-search" />
  <span class=searchHint>Type a command…</span>
</button>
```

## Метрики (ИЗ CSS)
.searchButton:
- размеры: height: 26px; width — авто по контенту
- отступы: padding: 0 var(--space-3); margin-right: var(--space-2); gap: var(--space-2)
- скругления: border-radius: var(--radius-sm)
- шрифт: font-size: var(--fs-xs)
- цвета: color: var(--text-muted);
  background: color-mix(in srgb, var(--bg-surface) 60%, transparent);
  border: 1px solid color-mix(in srgb, var(--bg-overlay) 30%, transparent)
- иконка: `.searchButton :global(.codicon) { font-size: 12px !important; }`
- hover: background: var(--bg-surface); color: var(--text-secondary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:flex; align-items:center; -webkit-app-region: no-drag

.searchHint:
- padding: 0 var(--space-2)

## Состояния
Только hover (см. выше). Вариантных классов нет.
