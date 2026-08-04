# 86 problems-panel — оригинал
Файлы: `src/renderer/components/problems/ProblemsPanel.tsx` (44-102), `src/renderer/components/problems/ProblemsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ header.header
│  ├─ span — «Problems»
│  └─ span.counts
│     ├─ button.countBtn(.countActive) [data-tooltip="Filter errors", disabled при 0]
│     │  └─ i.codicon.codicon-error(.errIcon при >0) + {counts.errors}
│     └─ button.countBtn(.countActive) [data-tooltip="Filter warnings", disabled при 0]
│        └─ i.codicon.codicon-warning(.warnIcon при >0) + {counts.warnings}
└─ div.list
   ├─ 0 файлов → p.empty — «No problems have been detected in the workspace.»
   ├─ файлы (cap 100, step 200) → div.group
   │  ├─ button.fileRow (toggle collapse)
   │  │  ├─ i.codicon.codicon-chevron-{right|down}.chevron
   │  │  ├─ <TreeIcon.fileIcon name type=file>
   │  │  ├─ span.fileName — basename
   │  │  ├─ span.fileDir [data-tooltip=uri] — dirname
   │  │  └─ span.fileCount — diagnostics.length
   │  ├─ !collapsed → ProblemRow×N (cap 200/файл; элемент 87)
   │  └─ >200 → div.fileDir style={padding:"2px 0 2px 28px"} — «… N more problems in this file»
   └─ hiddenFiles>0 → button.showMore — codicon-ellipsis + «Show N more files (M hidden)»
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `height:100%; min-height:0`
- `.header`: flex, `align-items:center; justify-content:space-between`; padding `8px 8px 8px 12px`; font-size `var(--fs-xs)`; `font-weight:500`; `text-transform:uppercase`; `letter-spacing:0.08em`; `font-feature-settings:"ss01"`; color `var(--text-muted)`; `flex-shrink:0` (совпадает с FileTreeHeader)
- `.counts`: `inline-flex; gap:4px`; `text-transform:none; letter-spacing:0`
- `.countBtn`: `inline-flex; align-items:center; gap:3px`; padding `1px 6px`; border `1px solid transparent`; `border-radius:9px`; background transparent; color `var(--text-muted)`; `font:inherit`; font-size `var(--fs-xs)`
  - hover (не disabled): background `color-mix(in srgb, var(--bg-surface) 70%, transparent)`
  - disabled: `cursor:default; opacity:0.8`
  - `.countActive`: background `color-mix(in srgb, var(--accent-primary) 18%, transparent)`; border-color `color-mix(in srgb, var(--accent-primary) 40%, transparent)`; color `var(--text-primary)`
  - `.countBtn .codicon`: `font-size:12px`
- `.errIcon`: color `var(--accent-red)`; `.warnIcon`: color `var(--accent-yellow)` (окрашены только при count>0)
- `.list`: `flex:1; min-height:0; overflow:auto`; padding `0 0 var(--space-2)`; font-size `var(--fs-sm)`
- `.empty`: `height:100%`; flex column, центрирование; `text-align:center`; `padding:var(--space-5)`; `margin:0`; color `var(--text-muted)`; font-size `var(--fs-sm)`
- `.group`: flex column
- `.fileRow`: flex, `align-items:center`; `gap:6px`; `width:100%; height:24px`; padding `0 var(--space-2)`; background transparent; border none; color `var(--text-secondary)`; `text-align:left`; `white-space:nowrap; overflow:hidden`; `font:inherit`; font-size `var(--fs-sm)`
  - hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- `.chevron`: `flex-shrink:0`; `font-size:13px`; `width:16px`; `text-align:center`; color `var(--text-muted)`
- `.fileIcon`: `flex-shrink:0`; 16×16px
- `.fileName`: color `var(--text-primary)`; `flex-shrink:0`
- `.fileDir`: `flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- `.fileCount`: `flex-shrink:0`; `min-width:16px; height:16px`; padding `0 5px`; `inline-flex`, центр; `border-radius:9px`; background `var(--bg-surface)`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- `.showMore`: flex, `align-items:center; gap:6px`; `width:100%`; border none; background none; `font:inherit`; font-size `var(--fs-xs)`; color `var(--text-muted)`; padding `6px 10px`; `text-align:left`
  - hover: color `var(--text-primary)`; background `color-mix(in srgb, var(--bg-surface) 55%, transparent)`
- transition — нет

## Состояния (классы-варианты с метриками)
- `.countActive` — активный severity-фильтр (accent 18% + бордер 40%)
- `.countBtn:disabled` — при нулевом счётчике (opacity 0.8)
- collapse per file (chevron right/down), caps: 100 файлов (+200 по showMore), 200 строк/файл
