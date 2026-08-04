# 11 layout-toggles-menu — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.tsx:117-163
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.module.css:38-133

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<ul class=menu role=menu style={left,top,visibility}>   // fixed, clampToViewport, offset 6px (POPUP_OFFSET_PX)
  <li class=menuLabel>Layout</li>
  ×6 <li><button role=menuitemcheckbox aria-checked aria-disabled disabled class=menuItem>
       <span class="check [checkOn]">{on && <i class="codicon codicon-check">}</span>
       <span class=itemIcon><PanelIcon slot=… /></span>
       <span class=itemLabel>Left|Left Bottom|File|Center Bottom|Right|Right Bottom</span>
       {disabled && hint && <span class=itemHint>Requires …</span>}
     </button></li>
  <li class=divider role=separator />
  <LayoutPresetsSection />                               // элемент 12
</ul>
```
Клик по item НЕ закрывает меню; закрытие — outside-click / Esc.

## Метрики (ИЗ CSS)
.menu:
- размеры: min-width: 220px; max-height: calc(100vh - 16px); overflow-y: auto
- отступы: padding: var(--space-1); margin: 0; gap: 1px (flex-column)
- скругления: border-radius: var(--radius-md)
- цвета: background: var(--bg-surface); border: 1px solid var(--divider-soft)
- тень: box-shadow: var(--shadow-dropdown)
- позиционирование: position: fixed; z-index: var(--z-dropdown); display:flex; flex-direction:column; list-style:none

.menuLabel:
- padding: var(--space-1) var(--space-3); font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted)

.menuItem:
- width: 100%; padding: var(--space-2) var(--space-3); gap: var(--space-2)
- border-radius: var(--radius-sm); background: transparent; border: none
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer
- display:flex; align-items:center
- hover (`:hover:not([disabled])`): background: color-mix(in srgb, var(--text-primary) 10%, transparent)

.check:
- width: 16px; height: 16px; border-radius: 3px; border: 1px solid var(--bg-overlay); flex-shrink: 0
- inline-flex центр; `.check .codicon { font-size: 12px; line-height: 1; }`

.checkOn:
- background: var(--accent-primary); border-color: var(--accent-primary); color: var(--accent-action-fg)

.itemIcon: inline-flex центр; color: var(--text-muted); flex-shrink: 0
.itemLabel: flex: 1
.itemHint: font-size: var(--fs-xs); color: var(--text-disabled)
.divider: height: 1px; margin: var(--space-1) var(--space-2); background: var(--divider-soft)

## Состояния
- `[disabled]` (child-строка при скрытом родителе): cursor: not-allowed; color: var(--text-muted); `.itemIcon { opacity: 0.4 }`; hover-фон не применяется; aria-checked=false принудительно (effectiveOn = isOn && !disabled)
- checked (`checkOn`): см. выше; рендерится codicon-check
- позиция: side "bottom" от анкора, offset 6px; visibility:hidden до первого замера
