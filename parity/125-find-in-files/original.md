# 125 find-in-files — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/FindInFiles.tsx` (89-137), `FindInFiles.module.css`

## JSX-структура (кратко, вложенность)
```
div.backdrop role=presentation onMouseDown → close
└─ div.box role=presentation stopPropagation
   ├─ input.input [ref фокус] placeholder="Search in files…"
   ├─ div.status  "Searching…" | "Type at least 2 chars" | "{N} hits"
   └─ ul.list role=listbox aria-label="Find in Files results"
      └─ li.item [.itemActive] role=option aria-selected × N
         ├─ div.itemHeader
         │  ├─ span.itemRel {rel}
         │  └─ span.itemLine ":{line}"
         └─ div.itemSnippet
            ├─ span {до матча}
            ├─ mark.match {матч}
            └─ span {после}
```
- Открытие: Ctrl/Cmd+Shift+F (document capture); Esc закрывает. Debounce `FIF_DEBOUNCE_MS = 220` мс; минимум 2 символа; backend ≤ 200 хитов.

## Метрики (ИЗ CSS, точные значения)
`.backdrop`:
- position: fixed; inset: 0; z-index: var(--z-overlay)
- flex; justify-content: center; align-items: flex-start; padding-top: 10vh
- background: rgba(0, 0, 0, 0.35); backdrop-filter: blur(2px)

`.box`:
- width: min(720px, calc(100vw - 32px)); max-height: 76vh
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)
- overflow: hidden; flex column

`.input`: width: 100%; padding: 12px 14px; background: transparent; color: var(--text-primary); border: none; border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent); font-size: var(--fs-md); outline: none

`.status`: padding: 6px 14px; font-size: var(--fs-xs); color: var(--text-muted)

`.list`: list-style: none; margin: 0; padding: 0 0 var(--space-2); overflow-y: auto

`.item`: padding: 6px 14px; cursor: pointer; flex column; gap: 2px; border-radius: var(--radius-xs)

`.itemHeader`: display: flex; align-items: baseline; gap: 4px; font-size: var(--fs-xs); color: var(--text-muted)
`.itemRel`: overflow hidden + ellipsis + nowrap
`.itemLine`: font-variant-numeric: tabular-nums
`.itemSnippet`: font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-secondary); nowrap + hidden + ellipsis

`.match`:
- background: color-mix(in srgb, var(--accent-orange) 35%, transparent)
- color: var(--text-primary); border-radius: 2px

## Состояния (классы-варианты с метриками)
- `.itemActive`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- active управляется mouseenter/стрелками; :hover-класса нет; transition отсутствует.
