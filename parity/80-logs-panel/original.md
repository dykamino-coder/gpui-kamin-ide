# 80 logs-panel — оригинал
Файлы: `src/renderer/components/main/LogsPanel.tsx` (73-137), `src/renderer/components/main/LogsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
channels.length===0 →
div.empty — i.fas.fa-inbox + span «No output channels yet. Extensions register them via <code>vscode.window.createOutputChannel(name)</code>.»

иначе:
div.layout
├─ nav.list [aria-label="Output channels"]
│  └─ button.item(.active) [data-tooltip="{extensionId} · {name}"]
│     ├─ span.itemName — c.name
│     └─ span.itemExt — c.extensionId
└─ div.right
   ├─ header.toolbar
   │  ├─ input[type=search].search [placeholder="Filter…"]
   │  ├─ button.toolBtn [data-tooltip="Copy entire buffer"] — codicon-copy (disabled при пустом буфере)
   │  └─ button.toolBtn [data-tooltip="Clear channel"] — codicon-clear-all (disabled при пустом буфере)
   └─ pre.body (ref, auto-scroll) — visibleBuffer
```
Поведение: фильтр debounce 150ms, сбрасывается при смене канала; stick-to-bottom с зазором 6px.

## Метрики (ИЗ CSS, точные значения)
- `.layout`: `display:grid; grid-template-columns:220px 1fr`; `gap:var(--space-3)`; `height:100%; min-height:0`
- `.list`: flex column; `gap:2px`; `overflow:auto`; `padding-right:var(--space-2)`
- `.item`: flex column, `align-items:flex-start`; `gap:2px`; padding `var(--space-2) var(--space-3)`; background transparent; border `1px solid transparent`; `border-radius:var(--radius-sm)`; color `var(--text-secondary)`; `font:inherit`; `text-align:left`; `width:100%`; `transition:background var(--transition-fast)`
  - hover: background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `var(--text-primary)`
  - `.item.active`: background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--accent-primary)`; border-color `color-mix(in srgb, var(--accent-primary) 35%, transparent)`
- `.itemName`: font-size `var(--fs-sm)`; `font-weight:500`
- `.itemExt`: font-size `var(--fs-xs)`; color `var(--text-muted)`; font-family `var(--font-mono)`
- `.right`: `display:grid; grid-template-rows:auto 1fr`; `gap:var(--space-2)`; `min-height:0`
- `.toolbar`: flex, `align-items:center`; `gap:var(--space-2)`
- `.search`: `flex:1`; padding `4px 8px`; background `var(--bg-base)`; color `var(--text-primary)`; border `1px solid var(--bg-surface)`; `border-radius:var(--radius-sm)`; font-size `var(--fs-sm)`; `outline:none`
  - focus: `border-color:var(--accent-primary)`
- `.toolBtn`: 26×26px; `display:grid; place-items:center`; background transparent; color `var(--text-secondary)`; border none; `border-radius:var(--radius-sm)`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover (не disabled): background `var(--bg-surface)`; color `var(--text-primary)`
  - `[disabled]`: `opacity:0.4; cursor:not-allowed`
  - `.toolBtn .codicon`: `font-size:14px`
- `.body`: background `var(--bg-base)`; border `1px solid var(--bg-surface)`; `border-radius:var(--radius-sm)`; `padding:var(--space-3)`; font-family `var(--font-mono)`; font-size `var(--fs-xs)`; color `var(--text-primary)`; `overflow:auto`; `white-space:pre-wrap; word-break:break-word`; line-height `var(--lh-snug)`; `margin:0`
- `.empty`: flex column, центрирование; `gap:var(--space-2)`; `height:100%`; color `var(--text-muted)`; `text-align:center`; `padding:var(--space-5)`
  - `.empty i`: `font-size:32px; opacity:0.6`; `.empty code`: `var(--font-mono)`, `var(--fs-xs)`

## Состояния (классы-варианты с метриками)
- `.item.active` — активный канал (акцентная заливка 14% + бордер 35%)
- `.toolBtn[disabled]` — при отсутствии канала/пустом буфере
- empty-state — при 0 каналов (вся панель заменяется `.empty`)
