# 14 theme-popover — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:82-118
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:32-89

## JSX-структура (кратко, вложенность)
```
<div class=menu role=dialog aria-label="Appearance">   // НЕ портал: absolute внутри .root
  <div class=header>
    <span class=title>Appearance</span>
    <button class="sysToggle [sysOn]" aria-pressed data-tooltip="Follow the OS light/dark setting">
      <i class="fas fa-circle-half-stroke" /><span>System</span>
    </button>
  </div>
  <div class=columns>
    <Column title="Dark">…</Column>       // элементы 15/16
    <Column title="Light">…</Column>
    <Column title="Icons">…</Column>
  </div>
</div>
```
Пики НЕ закрывают popover; закрытие — outside-click / Esc / window blur.

## Метрики (ИЗ CSS)
.menu:
- размеры: width: max-content
- отступы: padding: var(--space-2); margin: 0; gap: var(--space-2) (flex-column)
- скругления: border-radius: var(--radius-md)
- цвета: background: var(--bg-surface); border: 1px solid var(--divider-soft)
- тень: box-shadow: var(--shadow-dropdown)
- позиционирование: position: absolute; top: calc(100% + 4px); right: 0; z-index: var(--z-overlay); display:flex; flex-direction:column

.header:
- display:flex; align-items:center; justify-content:space-between; gap: var(--space-3); padding: 0 var(--space-1)

.title:
- font-size: var(--fs-sm); font-weight: 600; color: var(--text-primary)

.sysToggle:
- padding: var(--space-1) var(--space-2); gap: var(--space-2)
- background: transparent; color: var(--text-muted); border-radius: var(--radius-sm)
- font-size: var(--fs-xs); white-space: nowrap; display:inline-flex; align-items:center
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent); color: var(--text-primary)

.columns:
- display: grid; grid-template-columns: repeat(3, minmax(140px, 1fr)); gap: var(--space-2)

## Состояния
.sysOn (+ .sysOn:hover) — System активен:
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)
