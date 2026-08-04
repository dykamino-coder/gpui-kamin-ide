# 148 sample-checkbox-dropdown — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:109-141, components/titlebar/LayoutToggles.module.css

## Содержание/структура
`CheckboxDropdownRow()` в Block «Checkbox dropdown», hint: «LayoutToggles recipe — clicks toggle items WITHOUT closing the menu (only outside-click / Esc dismiss).»
Превью — статично встроенное меню (`style="position: static; boxShadow: none"`):
- `<ul class=menu role=menu>`:
  - `<li class=menuLabel>Sample</li>`
  - 3 `<li><button role=menuitemcheckbox aria-checked class=menuItem>`:
    - `<span class="check [checkOn]" aria-hidden>` — внутри `codicon-check` только когда включено
    - `<span class=itemLabel>Option A/B/C</span>`
- state: A=true, B=false, C=true; клик тогглит только свой пункт

## Метрики
- `.menu`: position fixed (в превью переопределено на static); z-index `--z-dropdown` (100); min-width 220px; background `--bg-surface` (#3d3f51); border `1px solid var(--divider-soft)`; border-radius `--radius-md` (12px); box-shadow `--shadow-dropdown`; list-style none; margin 0; padding `--space-1` (4px); flex column; gap 1px; max-height `calc(100vh - 16px)`; overflow-y auto
- `.menuLabel`: padding 4px 12px; font-size 11px; uppercase; letter-spacing 0.04em; color `--text-muted`
- `.menuItem`: flex; align-items center; gap 8px; width 100%; padding 8px 12px; background transparent; border none; border-radius 8px; color `--text-primary`; font inherit, 12px; text-align left; cursor pointer
- `.check`: inline-flex; центрирование; 16×16px; border-radius 3px; border `1px solid var(--bg-overlay)`; flex-shrink 0; `.check .codicon` 12px, line-height 1
- `.itemLabel`: flex 1

Сопутствующие классы модуля (не в превью): `.anchor` relative + `-webkit-app-region: no-drag`; `.trigger` 26×26px, grid, radius 12px, color `--text-secondary`, `> i` 13px, hover bg `--bg-surface` + `--text-primary`, `[aria-expanded="true"]` bg `color-mix(accent-primary 16%)`; `.itemIcon` color `--text-muted`; `.itemHint` 11px `--text-disabled`; `.divider` 1px, margin 4px 8px, bg `--divider-soft`; `.presetEmpty`; `.presetRow` flex gap 1px; `.presetApply` (flex 1, padding 8px 12px, hover `color-mix(text-primary 10%)`, label ellipsis); `.presetIconBtn` 26×26, hover `color-mix(text-primary 10%)` + `--text-primary`, `[aria-pressed="true"]` color `--accent-primary`, `> i` 13px.

## Состояния/варианты
- checked (`.checkOn`): background `--accent-primary`; border-color `--accent-primary`; color `--accent-action-fg` (галка)
- unchecked: пустой квадрат с рамкой `--bg-overlay`
- `.menuItem:hover:not([disabled])`: background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.menuItem[disabled]`: cursor not-allowed; color `--text-muted`; `.itemIcon` opacity 0.4
- Ключевое поведение: клик по пункту НЕ закрывает меню; закрытие — outside-click / Esc
