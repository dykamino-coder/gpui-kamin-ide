# 138 sample-dropdown — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:99-143, design-sections.module.css:310-374

## Содержание/структура
`DropdownRow()` в Block «Dropdown menu», форма зеркалит ThemeQuickToggle:
- `.dropdownAnchor` (relative inline-block) содержит trigger + меню
- trigger: `.btnSecondary .dropdownTrigger` — codicon-color-mode + «Theme» + codicon-chevron-down; клик тогглит open
- при open: `<ul class=dropdownMenu>`:
  - `<li class=dropdownGroupLabel>Built-in</li>`
  - 3 item'а (Dark/hint "default"/icon color-mode, Light/lightbulb, System/device-desktop): `<button class="dropdownItem [dropdownItemPicked]">` — codicon + `<span style=flex:1>label</span>` + опц. `.dropdownItemHint` + codicon-check у выбранного
- клик по item: setPicked + закрытие меню; начальный picked = "dark"

## Метрики
- `.dropdownAnchor`: position relative; display inline-block
- `.dropdownTrigger`: inline-flex; align-items center; gap 8px; codicon внутри font-size `--fs-md` (13px), line-height 1
- `.dropdownMenu`: position absolute; top `calc(100% + 4px)`; left 0; min-width 220px; background `--bg-mantle` (#262533); border-radius `--radius-md` (12px); box-shadow `--shadow-dropdown` (0 4px 16px rgba(0,0,0,0.5)); list-style none; margin 0; padding `--space-1` (4px); z-index `--z-dropdown` (100); flex column, gap 1px
- `.dropdownGroupLabel`: padding `4px 12px`; font-size 11px; uppercase; letter-spacing 0.04em; color `--text-muted`
- `.dropdownItem`: flex; align-items center; gap 8px; width 100%; padding `8px 12px`; background transparent; border none; color `--text-primary`; font inherit, size `--fs-sm` (12px); border-radius 8px; text-align left; cursor pointer
- `.dropdownItemHint`: font `--font-mono`, 11px, color `--text-muted`

## Состояния/варианты
- item hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- picked (`.dropdownItemPicked`): background `color-mix(in srgb, var(--accent-primary) 12%, transparent)`; color `--accent-primary`
- light theme picked: background `--accent-primary`; color `--accent-action-fg`; font-weight 600; codicon и hint тоже `--accent-action-fg`
- open/closed — по state; trigger визуально не меняется
