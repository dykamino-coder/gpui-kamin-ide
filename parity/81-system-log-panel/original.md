# 81 system-log-panel — оригинал
Файлы: `src/renderer/components/main/SystemLogPanel.tsx` (27-72), `src/renderer/components/main/SystemLogPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.layout
├─ div.toolbar
│  ├─ input[type=search].search [placeholder="Filter logs…"]
│  ├─ div.levels [role=group, aria-label="Filter by level"]
│  │  └─ 4 × button.levelBtn(.levelActive) [aria-pressed] — all / error / warning / info
│  └─ button.clear [data-tooltip="Clear logs"] — codicon-clear-all
└─ visible.length===0 →
   div.empty — i.fas.fa-inbox + span («No system logs yet — …» | «No logs match the filter.»)
   иначе:
   ul.list
   └─ li.row(.error|.warning|.info)
      ├─ i.codicon.codicon-{error|warning|info}.icon
      ├─ span.source — e.source
      ├─ span.message — e.message
      └─ span.time [data-tooltip=absoluteTime] — relativeTime
```
Порядок: newest-first (reverse).

## Метрики (ИЗ CSS, точные значения)
- `.layout`: flex column; `height:100%; min-height:0`
- `.toolbar`: flex, `align-items:center`; `gap:var(--space-2)`; padding `0 0 var(--space-2)`; `flex-shrink:0`
- `.search`: `flex:1; min-width:0`; `height:28px`; padding `0 10px`; background `var(--bg-base)`; border `1px solid var(--divider-soft)`; `border-radius:var(--radius-sm)`; color `var(--text-primary)`; `font:inherit`; font-size `var(--fs-sm)`; `outline:none`
  - focus: `border-color:var(--accent-primary)`
- `.levels`: flex; `gap:2px`
- `.levelBtn`: padding `4px 10px`; background transparent; border `1px solid transparent`; `border-radius:var(--radius-sm)`; color `var(--text-muted)`; `font:inherit`; font-size `var(--fs-xs)`; `text-transform:capitalize`
  - hover: color `var(--text-primary)`; background `color-mix(in srgb, var(--text-primary) 8%, transparent)`
  - `.levelActive`: color `var(--text-primary)`; background `color-mix(in srgb, var(--accent-primary) 22%, transparent)`
- `.clear`: `display:grid; place-items:center`; 28×28px; `flex-shrink:0`; background transparent; border none; `border-radius:var(--radius-sm)`; color `var(--text-muted)`
  - hover: color `var(--text-primary)`; background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.list`: `flex:1; min-height:0; overflow-y:auto`; `margin:0; padding:0; list-style:none`; font-family `var(--font-mono, ui-monospace, monospace)`; font-size `var(--fs-xs)`
- `.row`: `display:grid; grid-template-columns:16px max-content 1fr max-content`; `align-items:baseline`; `gap:var(--space-2)`; padding `3px var(--space-2)`; `border-bottom:1px solid color-mix(in srgb, var(--divider-soft) 50%, transparent)`
  - hover: background `color-mix(in srgb, var(--text-primary) 5%, transparent)`
- `.icon`: `align-self:center`; `font-size:13px`
  - `.error .icon`: color `var(--accent-red)`; `.warning .icon`: `var(--accent-yellow, #d8a657)`; `.info .icon`: `var(--accent-blue)`
- `.source`: color `var(--text-muted)`; `white-space:nowrap`
- `.message`: color `var(--text-primary)`; `white-space:pre-wrap; word-break:break-word; overflow-wrap:anywhere`
  - `.error .message`: color `var(--accent-red)`
- `.time`: color `var(--text-muted)`; `white-space:nowrap`; font-size `var(--fs-xs)`
- `.empty`: `flex:1`; flex column, центрирование; `gap:var(--space-2)`; color `var(--text-muted)`; `text-align:center`; `padding:var(--space-4)`
  - `.empty > i`: `font-size:24px; opacity:0.5`
- transition — нет

## Состояния (классы-варианты с метриками)
- `.levelActive` — активный сегмент фильтра (акцент 22%)
- `.row.error|.warning|.info` — цвет иконки; error красит и message в `--accent-red`
- empty-state: разные тексты для «пусто вообще» и «ничего не подошло под фильтр»
