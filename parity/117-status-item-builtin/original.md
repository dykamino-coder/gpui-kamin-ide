# 117 status-item-builtin — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (147-158), `StatusBar.module.css` (23-49)

## JSX-структура (кратко, вложенность)
```
button.item [.ok|.warn|.brand] type=button tabIndex=-1 [data-tooltip={title} aria-label={title}]
├─ (icon) span.codicon.codicon-{icon} [aria-hidden]
└─ span {label}
```
- Чисто информационный: нет onClick; `tabIndex=-1` держит вне tab-order, но hover-тултип работает (в отличие от `disabled`).
- Варианты: "N active" (ok), "N failed" (warn), "N off", "N cmds".

## Метрики (ИЗ CSS, точные значения)
`.item`:
- display: flex; align-items: center; gap: 4px
- padding: 0 var(--space-2)
- color: var(--text-muted)
- border-radius: var(--radius-xs)
- font-size: var(--fs-xs)

`.item .codicon`: font-size: 12px !important

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.ok`: color: var(--accent-green)
- `.warn`: color: var(--accent-yellow)
- `.brand`: color: var(--accent-primary); font-weight: 500
- transition отсутствует.
