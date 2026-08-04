# 89 terminal-toolbar — оригинал
Файлы: `src/renderer/components/terminal/TerminalToolbar.tsx` (151-216 — хедер; логика overflow 40-80), `src/renderer/components/terminal/TerminalToolbar.module.css`

## JSX-структура (кратко, вложенность)
```
header.bar
├─ overflow.enabled → button.scrollBtn [aria-label="Scroll tabs left", disabled=!canLeft] — codicon-chevron-left
├─ div.tabs (ref, скрытый скроллбар)
│  └─ button.tab(.tabActive)
│     ├─ i.codicon.codicon-terminal
│     ├─ span.tabLabel — s.label
│     └─ span.close [role=button, tabIndex=0, data-tooltip="Close"] — codicon-close
├─ overflow.enabled → button.scrollBtn [aria-label="Scroll tabs right", disabled=!canRight] — codicon-chevron-right
└─ div.anchor
   ├─ button.addBtn [aria-haspopup=menu, aria-expanded, data-tooltip="New terminal"] — codicon-add
   └─ portal-меню (элемент 90)
```
Скролл: page-step `max(32px, floor(clientWidth*0.8))`, `scrollTo({behavior:"smooth"})`; чевроны появляются только при переполнении (ResizeObserver + scroll).

## Метрики (ИЗ CSS, точные значения)
- `.bar`: flex, `align-items:flex-end`; `gap:var(--space-1)`; padding `0 25px`; `flex-shrink:0`; `min-height:30px`
- `.tabs`: flex, `align-items:flex-end`; `gap:2px`; `flex:1; min-width:0`; `overflow-x:auto`; `scrollbar-width:none`; `::-webkit-scrollbar{display:none}`
- `.scrollBtn`: 22×30px; `display:grid; place-items:center`; background transparent; border none; `border-radius:var(--radius-xs)`; color `var(--text-secondary)`; `flex-shrink:0`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover (не disabled): background `var(--bg-surface)`; color `var(--text-primary)`
  - disabled: `opacity:0.35; cursor:default`
  - codicon: `font-size:12px`
- `.tab`: `inline-flex; align-items:center; gap:6px`; padding `0 10px`; `height:30px`; background transparent; border none; `border-radius:8px 8px 0 0`; color `var(--text-secondary)`; `font-size:11px; font-weight:500; letter-spacing:0.02em`; `white-space:nowrap`; `flex:0 1 auto`; `min-width:80px; max-width:220px`; `position:relative`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - `.tab .codicon`: `font-size:12px; line-height:1`
  - hover: background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `var(--text-primary)`
- `.tabActive`, `.tabActive:hover`: background `var(--editor-bg)`; color `var(--text-primary)` — таб сливается с поверхностью консоли
  - `.tabActive::before/::after`: вогнутые уголки 6×6px, `position:absolute; bottom:0`; before: `left:-6px; background:radial-gradient(circle at 0 0, transparent 6px, var(--editor-bg) 6.5px)`; after: `right:-6px; radial-gradient(circle at 100% 0, …)`; `pointer-events:none`
- `.tabLabel`: `overflow:hidden; text-overflow:ellipsis; max-width:160px`
- `.close`: 16×16px; `inline-flex`, центр; `border-radius:var(--radius-xs)`; `color:inherit`; `opacity:0`
  - codicon: `font-size:11px`
  - `.tab:hover .close`, `.tabActive .close`: `opacity:0.7`
  - `.close:hover`: `opacity:1`; background `color-mix(in srgb, var(--bg-overlay) 60%, transparent)`
- `.anchor`: `position:relative; flex-shrink:0`
- `.addBtn`: 28×28px; `align-self:center`; `padding:0; margin:0`; `inline-flex`, центр; `line-height:1`; background transparent; border none; `border-radius:50%`; color `var(--text-secondary)`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover: background `var(--bg-surface)`; color `var(--text-primary)`
  - `[aria-expanded="true"]`: background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--accent-primary)`
  - codicon: `font-size:15px; width:14px; height:14px; line-height:1.1; display:block`

## Состояния (классы-варианты с метриками)
- `.tabActive` — editor-bg заливка + вогнутые уголки (inverted-radius, как Chrome/JetBrains)
- `.close` — скрыт (opacity 0), виден при hover таба/на активном (0.7), hover самой кнопки (1 + заливка)
- `.scrollBtn` — только при переполнении; disabled по краям
- `.addBtn[aria-expanded=true]` — акцентная подсветка открытого меню
