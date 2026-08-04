# 124 quick-open — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/QuickOpen.tsx` (84-125), `QuickOpen.module.css`

## JSX-структура (кратко, вложенность)
```
div.backdrop role=presentation onMouseDown → close
└─ div.box role=presentation onMouseDown stopPropagation
   ├─ input.input [ref фокус] placeholder="Type a file name…"
   │    ArrowDown/ArrowUp двигают active, Enter коммитит
   └─ ul.list role=listbox aria-label="Quick Open results"
      ├─ (нет результатов && query) li.empty "No matches"
      └─ li.item [.itemActive] role=option aria-selected × N (mouseenter → active, click → открыть)
         ├─ span.itemName {basename(rel)}
         └─ span.itemPath {dir(rel)}
```
- Открытие: Ctrl/Cmd+P (без Shift), обработчик capture на document; Esc закрывает. Debounce `QO_DEBOUNCE_MS = 80` мс; backend ≤ 50 хитов.

## Метрики (ИЗ CSS, точные значения)
`.backdrop`:
- position: fixed; inset: 0; z-index: var(--z-overlay)
- display: flex; justify-content: center; align-items: flex-start; padding-top: 12vh
- background: rgba(0, 0, 0, 0.35); backdrop-filter: blur(2px)

`.box`:
- width: min(640px, calc(100vw - 32px))
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)
- overflow: hidden; flex column

`.input`:
- width: 100%; padding: 12px 14px
- background: transparent; color: var(--text-primary)
- border: none; border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent)
- font-size: var(--fs-md); outline: none

`.list`: list-style: none; margin: 0; padding: var(--space-1) 0; max-height: min(50vh, 480px); overflow-y: auto

`.item`: display: flex; align-items: baseline; gap: var(--space-2); padding: 6px 14px; cursor: pointer

`.itemName`: font-size: var(--fs-sm); color: var(--text-primary); font-weight: 500
`.itemPath`: flex: 1; font-size: var(--fs-xs); color: var(--text-muted); overflow hidden + ellipsis + nowrap; text-align: right
`.empty`: padding: 12px 14px; color: var(--text-muted); font-size: var(--fs-sm); text-align: center

## Состояния (классы-варианты с метриками)
- `.itemActive`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- Светлая тема `[data-theme="light"] .itemActive`: background: var(--accent-primary); color: var(--accent-action-fg); `.itemName` → var(--accent-action-fg); `.itemPath` → color-mix(in srgb, var(--accent-action-fg) 80%, transparent)
- :hover-класса нет — active управляется mouseenter из TSX.
