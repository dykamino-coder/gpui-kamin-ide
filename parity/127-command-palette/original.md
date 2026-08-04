# 127 command-palette — оригинал
Файлы: `kamin-ide/src/renderer/components/command-palette/CommandPalette.tsx` (26-90), `CommandPalette.module.css`

## JSX-структура (кратко, вложенность)
```
button.scrim type=button aria-label="Close command palette" onClick → close
└─ div.palette role=dialog aria-label="Command palette" onClick stopPropagation
   ├─ div.inputRow
   │  ├─ span.codicon.codicon-search
   │  ├─ input.input [ref фокус] placeholder="Type a command name…"
   │  │    Enter → выполнить list[0]
   │  └─ kbd.kbd "Esc"
   ├─ ul.list
   │  ├─ (пусто) li.empty  No commands match "{query}"
   │  └─ li > button.row × N (кап PALETTE_MAX_ROWS)
   │     ├─ span.title  [span.category "{category}: "] {title}
   │     └─ span.id {command id}
   └─ div.footer "{N} command(s) · Enter to run"
```
- Скрим — `<button>` (клавиатурно-достижимая цель закрытия).

## Метрики (ИЗ CSS, точные значения)
`.scrim`:
- position: fixed; inset: 0; z-index: var(--z-modal)
- background: var(--overlay-modal)
- display: flex; justify-content: center; padding: 0; padding-top: var(--layout-palette-top-offset)
- animation: fade 0.12s ease-out; border: none; cursor: default; font: inherit; color: inherit

`.palette`:
- width: var(--layout-palette-width); max-width: calc(100vw - 32px); max-height: var(--layout-palette-max-height)
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 80%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-modal)
- flex column; overflow: hidden

`.inputRow`:
- display: flex; align-items: center; gap: var(--space-2)
- padding: var(--space-3) var(--space-4)
- border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- `.codicon`: font-size: 16px !important; color: var(--text-muted)

`.input`: flex: 1; background: transparent; border: none; outline: none; color: var(--text-primary); font-size: var(--fs-md)

`.kbd`:
- font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-muted)
- background: color-mix(in srgb, var(--bg-overlay) 50%, transparent)
- padding: 2px 6px; border-radius: var(--radius-xs)

`.list`: list-style: none; margin: 0; padding: var(--space-1); overflow: auto; flex: 1; flex column; gap: 1px

`.row`:
- display: flex; align-items: baseline; justify-content: space-between; gap: var(--space-3)
- width: 100%; padding: var(--space-2) var(--space-3)
- border: none; background: transparent; border-radius: var(--radius-sm)
- cursor: pointer; text-align: left; font: inherit; font-size: var(--fs-md); color: inherit

`.title`: color: var(--text-primary); flex: 1
`.category`: color: var(--text-muted); font-weight: 500
`.id`: font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-muted)
`.empty`: padding: var(--space-3) var(--space-4); color: var(--text-muted); font-style: italic
`.footer`: padding: var(--space-2) var(--space-4); border-top: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent); font-size: var(--fs-xs); color: var(--text-muted)

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--accent-primary) 18%, transparent)
- `.list > li:first-child .row` (подсветка первой строки — цель Enter): background: color-mix(in srgb, var(--accent-primary) 12%, transparent)
