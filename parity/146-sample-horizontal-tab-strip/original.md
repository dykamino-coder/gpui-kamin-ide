# 146 sample-horizontal-tab-strip — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:44-71, components/activity-bar/BottomTabBar.module.css, BottomTabBar.tsx:24 (TAB_ICON_SIZE_PX = 13)

## Содержание/структура
`TabsRow()` в Block «Horizontal tab strip», hint: «BottomTabBar / FileViewerTabs recipe — pill tabs, accent-tinted active state.»
Превью использует реальные классы BottomTabBar.module.css:
- `.strip` (inline style: width 100%, maxWidth 360) → `.tabs` → 3 кнопки `.tab` (+ `.tabActive` у активной): Terminal (icon terminal), Problems (warning), Output (output)
- каждая кнопка: `<ToolIcon size={13}>` + `<span class=tabLabel>` , `aria-pressed`, клик = setActive
- начальный active = "terminal"

## Метрики
- `.strip`: flex; align-items center; gap `--space-1` (4px); flex-shrink 0; padding `4px var(--space-2)` = 4px 8px; border-radius `--radius-sm` (8px)
- `.tabs`: flex; align-items center; gap 4px; flex 1; min-width 0; overflow-x auto; scrollbar-width none
- `.tab`: inline-flex; align-items center; gap 6px; padding 4px 10px; height 24px; background transparent; border none; border-radius 8px; color `--text-secondary`; font-size 11px; font-weight 500; letter-spacing 0.02em; white-space nowrap; cursor pointer; transition background+color 150ms ease
- `.tab .codicon`: font-size 13px; line-height 1; `.tabImage` (VSIX SVG/PNG): 13×13px, object-fit contain
- `.tabLabel`: overflow hidden; text-overflow ellipsis; min-width 0
- Иконка ToolIcon: TAB_ICON_SIZE_PX = 13

Остальные классы модуля (в превью не используются): `.tabDragging` opacity 0.3; `.dropPlaceholder` 36×24px, dashed `color-mix(accent-primary 70%)`, bg `color-mix(accent-primary 14%)`, radius 8px; `.pickerSlot` flex-shrink 0, margin-left auto.

## Состояния/варианты
- hover (`.tab:hover`): background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `--text-primary`
- active (`.tabActive`, и `.tabActive:hover`): background `color-mix(in srgb, var(--accent-primary) 16%, transparent)`; color `--text-primary` (без ring)
