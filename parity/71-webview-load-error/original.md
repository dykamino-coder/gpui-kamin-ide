# 71 webview-load-error — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.tsx (строки 65-75), kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.module.css

## JSX-структура (кратко, вложенность)
```
div.errWrap [role="alert"]
├─ i.fas.fa-triangle-exclamation.errIcon [aria-hidden]
├─ div.errTitle  "This panel didn't load"
├─ div.errHint   "The extension host may still be starting up."
└─ button.retry  onClick={onRetry}
   ├─ i.fas.fa-rotate [aria-hidden]
   └─ " Retry"
```
Терминальное состояние после исчерпания retry-бюджета resolve.

## Метрики (ИЗ CSS, точные значения)
### .errWrap
- position: absolute; inset: 0
- display: flex; flex-direction: column; align-items: center; justify-content: center
- gap: 8px; padding: 24px; text-align: center
- background: `var(--bg-surface, var(--editor-bg, #22222e))`

### .errIcon
- font-size: 22px
- color: `var(--accent-yellow, #f9e2af)`; opacity: 0.85
- margin-bottom: 4px

### .errTitle
- font-size: var(--fs-md, 13px); font-weight: 600
- color: `var(--text-primary, #cdd6f4)`

### .errHint
- font-size: var(--fs-sm, 12px); color: `var(--text-muted, #9399b2)`
- max-width: 280px; line-height: 1.4

### .retry
- display: inline-flex; align-items: center; gap: 6px
- padding: 6px 16px
- border-radius: var(--radius-sm, 8px)
- border: `1px solid var(--divider-soft, color-mix(in srgb, var(--text-primary, #cdd6f4) 14%, transparent))`
- background: `color-mix(in srgb, var(--text-primary, #cdd6f4) 6%, transparent)`
- color: `var(--text-primary, #cdd6f4)`
- font-size: var(--fs-sm, 12px); cursor: pointer
- transition: background 0.15s ease

## Состояния (классы-варианты с метриками)
- `.retry:hover`: background `color-mix(in srgb, var(--text-primary, #cdd6f4) 12%, transparent)`
- других вариантов нет
