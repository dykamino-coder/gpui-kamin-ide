# 84 extensions-panel — оригинал
Файлы: `src/renderer/components/extensions/ExtensionsPanel.tsx` (79-110 — `ExtensionsPanel`), `src/renderer/components/extensions/ExtensionsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ header.header
│  ├─ span — «Extensions»
│  └─ button.installBtn [data-tooltip="Install from a .vsix archive"] — codicon-cloud-download + «Install»
└─ div.list
   ├─ list.length===0 → p.empty — «No extensions installed.»
   ├─ sideloaded>0 → div.groupHeader «Installed — {N}» + Row×N
   └─ builtin>0   → div.groupHeader «Built-in — {N}» + Row×N   (Row — элемент 85)
```
Сортировка по displayName; иконки — кэш localStorage + host fetch.

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `height:100%; min-height:0`
- `.header`: flex, `align-items:center; justify-content:space-between`; `gap:var(--space-2)`; padding `var(--space-1) var(--space-2) var(--space-1) var(--space-3)`; font-size `var(--fs-xs)`; `text-transform:uppercase`; `letter-spacing:0.04em`; color `var(--text-muted)`; `flex-shrink:0`
- `.installBtn`: `inline-flex; align-items:center; gap:4px`; padding `3px 8px`; font-size `var(--fs-xs)`; `text-transform:none; letter-spacing:0`; `border-radius:var(--radius-sm)`; border `1px solid color-mix(in srgb, var(--accent-primary) 40%, transparent)`; background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--text-primary)`
  - hover: background `color-mix(in srgb, var(--accent-primary) 26%, transparent)`
  - `.installBtn .codicon`: `font-size:12px`
- `.list`: `flex:1; min-height:0; overflow:auto`; padding `0 var(--space-2) var(--space-2)`
- `.empty`: `padding:var(--space-3)`; color `var(--text-muted)`; font-size `var(--fs-sm)`
- `.groupHeader`: padding `var(--space-2) var(--space-2) 4px`; font-size `var(--fs-xs)`; `font-weight:600`; `text-transform:uppercase`; `letter-spacing:0.04em`; color `var(--text-muted)`
- transition — нет (кроме элементов строки, см. 85)

## Состояния (классы-варианты с метриками)
- empty (0 расширений), группы условные (sideloaded/builtin)
- hover `.installBtn` — усиление акцентной заливки 14%→26%
