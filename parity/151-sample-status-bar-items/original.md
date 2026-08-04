# 151 sample-status-bar-items — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:180-195, components/status-bar/StatusBar.module.css

## Содержание/структура
`StatusItemRow()` в Block «Status-bar items»: 4 кнопки на реальных классах StatusBar.module.css:
1. `.item .ok`: `codicon-circle-filled` + «3 active»
2. `.item .warn`: `codicon-warning` + «2 failed»
3. `.item`: «UTF-8» (нейтральный)
4. `.item .brand`: «KaminIDE 0.0.1»

## Метрики
- `.item`: flex; align-items center; gap 4px; padding `0 var(--space-2)` (0 8px); color `--text-muted`; border-radius `--radius-xs` (4px); font-size `--fs-xs` (11px)
- `.item .codicon`: font-size 12px !important

Контекст живого StatusBar (не в превью): `.statusBar` height `var(--layout-status-bar-height)`; background transparent; flex, align-items stretch; font-size 11px; color `--text-muted`; padding 0 8px; gap 4px. `.left`/`.right` gap 2px; `.right` margin-left auto.

## Состояния/варианты
- hover (`.item:hover`): background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`; color `--text-primary`
- `.ok`: color `--accent-green` (#a6e3a1)
- `.warn`: color `--accent-yellow` (#f9e2af)
- `.brand`: color `--accent-primary` (#89b4fa); font-weight 500
- в живом StatusBar также: `.clickable` cursor pointer; `.item:disabled` cursor default, hover нейтрализован; `.update` (accent-пилюля 22% tint, weight 600, hover 34%); `.downloading` + `.progressFill` (fill `color-mix(accent-primary 32%)`, transition width 120ms linear) + `.progressLabel` — в превью не показаны
