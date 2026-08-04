# 123 quick-pick-modal — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/QuickPickModal.tsx` (65-123), `QuickPickModal.module.css`

## JSX-структура (кратко, вложенность)
```
div.overlay role=presentation onClick(backdrop; НЕ закрывает при ignoreFocusOut)
└─ div.panel role=dialog aria-modal=true aria-label={title ?? "Select"}
   ├─ (title) div.title
   ├─ input.input [ref фокус] placeholder={placeHolder ?? "Type to filter…"}
   │    Enter: multi → OK; single → первый selectable
   ├─ (prompt) div.prompt
   ├─ ul.list role=listbox aria-multiselectable={multi}
   │  ├─ (пусто) li.empty "No matching items"
   │  ├─ separator (kind=-1): li.separator role=separator {label}
   │  └─ li > button.item role=option aria-selected
   │     ├─ (multi) i.codicon.codicon-check|codicon-circle-large-outline .check
   │     ├─ span.label   (renderCodiconText: $(icon))
   │     ├─ (description) span.description
   │     └─ (detail) span.detail
   └─ (multi) div.actions
      ├─ button.cancelBtn "Cancel"
      └─ button.okBtn "OK ({checked.size})"
```
- Фильтр по label (+ description/detail при matchOnDescription/matchOnDetail); separators и alwaysShow обходят фильтр. Esc = resolve(null).

## Метрики (ИЗ CSS, точные значения)
`.overlay`:
- position: fixed; inset: 0; z-index: var(--z-modal)
- background: var(--overlay-modal)
- display: flex; justify-content: center; padding-top: var(--layout-palette-top-offset)
- animation: qpFade 0.12s ease-out

`.panel`:
- width: var(--layout-palette-width); max-width: calc(100vw - 32px); max-height: var(--layout-palette-max-height)
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 80%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-modal)
- flex column; overflow: hidden

`.title`: padding: var(--space-2) var(--space-4); font-size: var(--fs-sm); font-weight: 600; color: var(--text-primary); border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)

`.input`:
- margin: var(--space-2) var(--space-3) 0; padding: var(--space-2) var(--space-3)
- background: var(--bg-base); border: 1px solid color-mix(in srgb, var(--bg-surface) 70%, transparent)
- border-radius: var(--radius-sm); outline: none; color: var(--text-primary); font-size: var(--fs-md)

`.prompt`: padding: var(--space-1) var(--space-4) 0; font-size: var(--fs-sm); color: var(--text-secondary)

`.list`: list-style: none; margin: 0; padding: var(--space-1); overflow: auto; flex: 1; flex column; gap: 1px

`.item`:
- display: flex; align-items: baseline; gap: var(--space-2); width: 100%
- padding: var(--space-2) var(--space-3); border: none; background: transparent
- border-radius: var(--radius-sm); cursor: pointer; text-align: left
- font: inherit; font-size: var(--fs-md); color: var(--text-primary)

`.check`: align-self: center; font-size: 13px; color: var(--accent-primary); flex-shrink: 0
`.label`: flex-shrink: 0
`.description`: color: var(--text-muted); font-size: var(--fs-sm)
`.detail`: margin-left: auto; color: var(--text-muted); font-size: var(--fs-xs); font-family: var(--font-mono); nowrap + ellipsis
`.empty`: padding: var(--space-3) var(--space-4); color: var(--text-muted); font-style: italic

`.separator`:
- display: flex; align-items: center; gap: var(--space-2)
- padding: var(--space-1) var(--space-3); margin-top: var(--space-1)
- font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em
- color: var(--text-muted); border-top: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- `:first-child`: border-top: none; margin-top: 0

`.actions`: display: flex; justify-content: flex-end; gap: var(--space-2); padding: var(--space-2) var(--space-3); border-top: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)

`.cancelBtn`, `.okBtn`: padding: var(--space-1) var(--space-3); border-radius: var(--radius-sm); border: 1px solid transparent; font-size: var(--fs-sm); cursor: pointer
`.cancelBtn`: background: transparent; color: var(--text-secondary)
`.okBtn`: background: var(--accent-primary); color: var(--accent-action-fg, #fff)

## Состояния (классы-варианты с метриками)
- `.input:focus`: border-color: var(--accent-primary)
- `.item:hover`: background: color-mix(in srgb, var(--accent-primary) 18%, transparent)
- `.cancelBtn:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.okBtn:hover`: background: var(--accent-action-hover, var(--accent-primary))
- multi-чекбокс: codicon-check (выбран) / codicon-circle-large-outline (нет)
