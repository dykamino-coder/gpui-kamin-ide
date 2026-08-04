# 136 sample-list-item — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:69-82, design-sections.module.css:252-308

## Содержание/структура
`ActiveItemRow()` в Block «List item — active selection (sidebar pattern)»: `<ul class=itemList>` из 4 `<li><button class=listItem>`:
1. codicon-folder + «Sessions»
2. codicon-settings-gear + «Settings (active)» — `.listItem .listItemActive`
3. codicon-extensions + «Extensions»
4. codicon-debug-disconnect + «Disabled» — `disabled`

Зеркалит паттерн строки sidebar/customize: иконка + label, hover тонируется, active = tinted (dark) / filled-accent (light).

## Метрики
- `.itemList`: list-style none; margin/padding 0; flex column, gap 2px; width 100%; max-width 280px
- `.listItem`: flex, align-items center, gap 8px; width 100%; padding `8px 12px`; border none; border-radius 8px; background transparent; color `--text-secondary`; font inherit, size `--fs-md` (13px); text-align left; cursor pointer; transition `background 150ms ease`
- `.listItem .codicon` (`:global`): font-size 14px

## Состояния/варианты
- hover (`.listItem:hover:not([disabled])`): background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `--text-primary`
- active (`.listItemActive`): background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `--accent-primary`
- active hover (`.listItemActive:hover`): background `color-mix(... accent-primary 22% ...)`; color `--accent-primary` (без этого generic-hover перебил бы active)
- light theme (`[data-theme="light"] .listItemActive`): background `--accent-primary`; color `--accent-action-fg`; font-weight 600; codicon тоже `--accent-action-fg`; hover → background `--accent-action-hover`
- disabled (`.listItem[disabled]`): opacity 0.45; cursor not-allowed
