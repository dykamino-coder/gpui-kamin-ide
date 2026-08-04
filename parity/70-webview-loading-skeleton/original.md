# 70 webview-loading-skeleton — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.tsx (строки 38-60), kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.module.css

## JSX-структура (кратко, вложенность)
```
div.wrap [role="status"] [aria-label="Loading panel…"]
├─ div.bar [aria-hidden]                (тулбар-скелет)
│  ├─ span.sk.pill
│  └─ span.sk.search
├─ div.rows [aria-hidden]               (6 строк, SKELETON_ROWS = 6)
│  └─ ×6 div.row
│     ├─ span.sk.icon
│     └─ div.lines
│        ├─ span.sk.line
│        └─ span.sk.lineDim
├─ div.waitNote  (только seconds >= 3, EXPLAIN_AFTER_S)
│  текст: `Waiting for the extension host to open this panel · {N}s` + ` · attempt {N}` при attempts > 1
└─ span.srOnly "Loading…"
```
Секундомер: setInterval 1000ms.

## Метрики (ИЗ CSS, точные значения)
### .wrap
- position: absolute; inset: 0; overflow: hidden
- display: flex; flex-direction: column; gap: 14px
- padding: 16px 18px
- background: `var(--bg-surface, var(--editor-bg, #22222e))`

### .bar
- display: flex; align-items: center; gap: 10px; flex-shrink: 0

### .rows
- display: flex; flex-direction: column; gap: 14px; min-height: 0

### .row
- display: flex; align-items: center; gap: 12px

### .lines
- display: flex; flex-direction: column; gap: 7px; flex: 1; min-width: 0

### .sk (шиммер-примитив)
- position: relative; overflow: hidden; border-radius: 6px
- background: `color-mix(in srgb, var(--text-primary, #cdd6f4) 8%, transparent)`
- `::after`: inset 0; transform translateX(-100%); background `linear-gradient(90deg, transparent, color-mix(in srgb, var(--text-primary, #cdd6f4) 9%, transparent), transparent)`
- animation: `kaminSkShimmer 1.25s ease-in-out infinite`; keyframes: `100% { transform: translateX(100%); }`

### Размеры скелет-блоков
- .pill: 84×22px, border-radius 8px
- .search: flex 1, height 22px, border-radius 8px
- .icon: 30×30px, border-radius 8px, flex-shrink 0
- .line: height 11px, width var(--sk-row)
- .lineDim: height 9px, opacity 0.6, width calc(var(--sk-row) * 0.62)
- Ширины строк (nth-child 6n+1..6n+6): 90% / 70% / 80% / 60% / 75% / 50%

### .waitNote
- margin-top: var(--space-3, 12px); text-align: center
- font-size: 11px; color: var(--text-disabled)
- font-variant-numeric: tabular-nums

### .srOnly
- position absolute; width/height 1px; overflow hidden; clip rect(0 0 0 0); white-space nowrap

## Состояния (классы-варианты с метриками)
- seconds < 3: без waitNote; >= 3: waitNote появляется
- attempts > 1: добавляется ` · attempt N`
- анимация: только shimmer (1.25s ease-in-out infinite); hover/focus нет
