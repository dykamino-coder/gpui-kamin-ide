# 90 terminal-shell-menu — оригинал
Файлы: `src/renderer/components/terminal/TerminalToolbar.tsx` (112-149 — portal-меню; позиционирование 99-110), `src/renderer/components/terminal/TerminalToolbar.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
ul.menu [role=menu] style={left,top из clampToViewport(side:"bottom", offset:6px); visibility}
├─ shells.length===0 → li.menuEmpty — «No shells discovered»
└─ shells.map → li.menuRow
   ├─ button.menuItem [role=menuitem] (click → close + onOpen(id))
   │  ├─ i.codicon.codicon-{s.icon ?? "terminal"}.itemIcon
   │  ├─ span.itemLabel — s.label
   │  └─ isDefault → span.defaultTag — «default»
   └─ button.starBtn(.starOn) [aria-pressed, data-tooltip="Default shell"|"Set as default"]
      └─ i.codicon.codicon-star-{full|empty}
```
Закрытие: клик вне (mousedown capture) или Escape; POPUP_OFFSET_PX=6.

## Метрики (ИЗ CSS, точные значения)
- `.menu`: `position:fixed`; `z-index:var(--z-dropdown)`; `min-width:200px`; background `var(--bg-surface)`; border `1px solid var(--divider-soft)`; `border-radius:var(--radius-md)`; `box-shadow:var(--shadow-dropdown)`; `list-style:none; margin:0`; `padding:var(--space-1)`; flex column; `gap:1px`; `max-height:calc(100vh - 16px)`; `overflow-y:auto`
- `.menuEmpty`: padding `var(--space-2) var(--space-3)`; font-size `var(--fs-sm)`; color `var(--text-muted)`
- `.menuRow`: flex, `align-items:center`; `gap:2px`; `.menuRow .menuItem { flex:1 }`
- `.menuItem`: flex, `align-items:center`; `gap:var(--space-2)`; `width:100%`; padding `var(--space-2) var(--space-3)`; background transparent; border none; `border-radius:var(--radius-sm)`; color `var(--text-primary)`; `font:inherit`; font-size `var(--fs-sm)`; `text-align:left`
  - hover: background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.itemIcon`: `width:16px; text-align:center`; color `var(--text-muted)`
- `.itemLabel`: `flex:1; white-space:nowrap`
- `.defaultTag`: font-size `var(--fs-xs)`; color `var(--text-muted)`; `text-transform:uppercase`; `letter-spacing:0.04em`
- `.starBtn`: 24×24px; `inline-flex`, центр; `flex-shrink:0`; background transparent; border none; `border-radius:var(--radius-sm)`; color `var(--text-muted)`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover: background `color-mix(in srgb, var(--text-primary) 10%, transparent)`; color `var(--text-primary)`
  - codicon: `font-size:12px`
- `.starOn`, `.starOn:hover`: color `var(--accent-primary)`

## Состояния (классы-варианты с метриками)
- `.starOn` — выбранный дефолтный шелл (акцентная звезда star-full; у остальных star-empty)
- `.menuEmpty` — 0 обнаруженных шеллов
- `visibility:hidden` до вычисления позиции (двухпроходное измерение)
