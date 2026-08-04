# 126 workspace-symbols — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/WorkspaceSymbols.tsx` (79-111); CSS — переиспользование `overlays/QuickOpen.module.css` (метрики идентичны №124)

## JSX-структура (кратко, вложенность)
```
div.backdrop role=presentation onMouseDown → close
└─ div.box role=presentation stopPropagation
   ├─ input.input [ref фокус] placeholder="Go to symbol in workspace…"
   └─ ul.list role=listbox aria-label="Workspace symbols"
      ├─ (нет результатов && query) li.empty "No symbols"
      └─ li.item [.itemActive] role=option aria-selected × N
         ├─ span.codicon.codicon-{SYMBOL_ICON[kind] ?? symbol-misc}
         ├─ span.itemName {name}
         └─ span.itemPath "{containerName · }{basename(uri)}"
```
- Открытие: Ctrl/Cmd+T (без Shift); Esc закрывает. Debounce `WS_DEBOUNCE_MS = 120` мс; минимум 1 символ.
- SymbolKind→codicon карта: 4 class, 5 method, 6 property, 7 field, 8 constructor, 9 enum, 10 interface, 11 function, 12 variable, 13 constant, 22 struct, 1/2 namespace, 23 event; fallback `symbol-misc`.
- Enter/клик → `openFileAt(uri, range)` (открыть + reveal диапазона).

## Метрики (ИЗ CSS, точные значения)
Полностью из QuickOpen.module.css:
- `.backdrop`: fixed inset 0; z-index var(--z-overlay); flex center/flex-start; padding-top: 12vh; background: rgba(0,0,0,0.35); backdrop-filter: blur(2px)
- `.box`: width min(640px, calc(100vw - 32px)); background var(--bg-mantle); border 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent); border-radius var(--radius-md); box-shadow var(--shadow-dropdown); overflow hidden
- `.input`: padding 12px 14px; background transparent; border-bottom 1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent); font-size var(--fs-md)
- `.list`: padding var(--space-1) 0; max-height min(50vh, 480px); overflow-y auto
- `.item`: flex baseline; gap var(--space-2); padding 6px 14px; cursor pointer
- `.itemName`: font-size var(--fs-sm); color var(--text-primary); font-weight 500
- `.itemPath`: flex 1; font-size var(--fs-xs); color var(--text-muted); ellipsis; text-align right
- `.empty`: padding 12px 14px; color var(--text-muted); font-size var(--fs-sm); text-align center
- Codicon-иконка символа: без собственного класса, размер по умолчанию codicon.

## Состояния (классы-варианты с метриками)
- `.itemActive`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- Light-тема: `.itemActive` background var(--accent-primary), текст/путь → var(--accent-action-fg) (см. №124).
