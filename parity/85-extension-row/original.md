# 85 extension-row — оригинал
Файлы: `src/renderer/components/extensions/ExtensionsPanel.tsx` (56-77 — `Row`), `src/renderer/components/extensions/ExtensionsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.row(.disabled при !e.enabled)
├─ icon ? img.icon (data-URL) : i.codicon.codicon-extensions.iconFallback
├─ div.meta
│  ├─ span.name [data-tooltip=e.id] — displayName
│  └─ span.sub — «{version} · {active|idle|disabled|activation error}»
└─ div.rowActions
   ├─ button.toggle — «Disable» | «Enable»
   └─ !builtin → button.uninstall [data-tooltip="Uninstall", aria-label] — codicon-trash
```

## Метрики (ИЗ CSS, точные значения)
- `.row`: flex, `align-items:center`; `gap:var(--space-2)`; `padding:var(--space-2)`; `border-radius:var(--radius-sm)`
  - hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- `.disabled`: `opacity:0.55`
- `.icon`: 26×26px; `flex-shrink:0`; `border-radius:var(--radius-xs)`; `object-fit:contain`
- `.iconFallback`: 26×26px; `flex-shrink:0`; `display:grid; place-items:center`; `font-size:16px`; color `var(--text-muted)`
- `.meta`: `flex:1; min-width:0`; flex column
- `.name`: font-size `var(--fs-sm)`; color `var(--text-primary)`; `overflow:hidden; text-overflow:ellipsis; white-space:nowrap`
- `.sub`: font-size `var(--fs-xs)`; color `var(--text-muted)`
- `.rowActions`: flex, `align-items:center`; `gap:4px`; `flex-shrink:0`
- `.toggle`: `flex-shrink:0`; padding `2px 10px`; font-size `var(--fs-xs)`; `border-radius:var(--radius-sm)`; border `1px solid color-mix(in srgb, var(--text-muted) 30%, transparent)`; background `var(--bg-surface)`; color `var(--text-primary)`
  - hover: background `var(--bg-overlay)`
- `.uninstall`: `display:grid; place-items:center`; 24×22px; border none; `border-radius:var(--radius-sm)`; background transparent; color `var(--text-muted)`
  - hover: background `color-mix(in srgb, var(--accent-red) 16%, transparent)`; color `var(--accent-red)`
- transition — нет

## Состояния (классы-варианты с метриками)
- `.disabled` (opacity 0.55) — выключенное расширение
- Статус-текст в `.sub`: disabled / activation error / active / idle
- uninstall-кнопка только у sideloaded (`!builtin`)
- hover-эффекты: строка (surface 60%), toggle (bg-overlay), uninstall (red 16% + red-текст)
