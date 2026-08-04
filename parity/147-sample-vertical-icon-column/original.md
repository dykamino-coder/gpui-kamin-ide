# 147 sample-vertical-icon-column — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:73-107, components/activity-bar/ActivityBar.module.css

## Содержание/структура
`VerticalIconColumnRow()` в Block «Vertical icon column», hint: «ActivityBar recipe — square icon tiles + picker dot at the end.»
Превью на реальных классах ActivityBar.module.css:
- `<nav class=bar aria-label="Sample activity bar">` → `<ul class=list>` из 3 `<li><button class="btn [btnActive]">` с `<ToolIcon>`: Projects (folders), Folder tree (tree-view), Search (search); aria-pressed, aria-label, data-tooltip
- ниже `.pickerAnchor` → `<button class=picker aria-label="More" data-tooltip="Add or remove items">` c `codicon-more`
- начальный active = "projects"

## Метрики
- `.bar`: flex column; align-items center; gap `--space-2` (8px); padding `var(--space-3) 0` (12px 0); width `var(--layout-activity-bar-width, 44px)`; flex-shrink 0; фон прозрачный (гейт-градиент app-фона просвечивает)
- `.list`: list-style none; margin/padding 0; flex column; gap 2px; width 100%; align-items center
- `.btn`, `.picker`: 32×32px; display grid; place-items center; background transparent; border none; border-radius `--radius-sm` (8px); color `--text-muted`; font inherit; cursor pointer; transition background+color 150ms ease
- `.btn .codicon`, `.picker .codicon`: font-size 18px; line-height 1; img-варианты (`.btnImage`, `.menuItemImage`, `.btn img`, `.picker img`): 18×18px, object-fit contain
- `.pickerAnchor`: position relative; flex; justify-content center; width 100%

Не используемые в превью классы модуля: `.tileDragging > .btn` opacity 0.3; `.dropPlaceholder` 32×32, dashed accent; `.barReverse` justify-content flex-end; `.pickerAnchorInline`; `.menu` (min-width 220px, bg `--bg-surface`, border `1px solid var(--divider-soft)`, radius 12px, shadow `--shadow-dropdown`, padding 4px, gap 1px, z `--z-dropdown`); `.menuPortal` (fixed, max-height calc(100vh - 16px), max-width calc(100vw - 16px), overflow-y auto); `.menuLabel`; `.menuItem` (+hover `color-mix(text-primary 10%)`); `.menuLabelText` flex 1.

## Состояния/варианты
- hover (`.btn:hover`, `.picker:hover`): background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `--text-primary`
- active (`.btnActive`, `.btnActive:hover`): background `color-mix(in srgb, var(--accent-primary) 16%, transparent)`; color `--text-primary` (иконка остаётся PRIMARY, не accent; без ring)
