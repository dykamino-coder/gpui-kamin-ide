# 140 sample-chips-kbd-code-badge — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:169-180, design-sections.module.css:385-434

## Содержание/структура
`ChipsRow()` в Block «Chips · Kbd · Code · Badge»:
- `<span class=chip>active</span>` (зелёный)
- `<span class="chip chipMuted">idle</span>`
- `<span class="chip chipDanger">error</span>`
- `<kbd class=kbd>Ctrl+Shift+P</kbd>`
- `<code class=codeInline>npm run check</code>`
- `<span class=badge>3</span>`

## Метрики
`.chip` (база, зелёный):
- inline-flex; align-items center; gap 4px; padding `1px var(--space-2)` = 1px 8px
- border-radius `--radius-xs` (4px); font-size `--fs-xs` (11px)
- background `color-mix(in srgb, var(--accent-green) 14%, transparent)`; color `--accent-green`; border `1px solid color-mix(... accent-green 30% ...)`

`.chipMuted`: background `color-mix(text-muted 12%)`; color `--text-muted`; border-color `color-mix(text-muted 25%)`.
`.chipDanger`: background `color-mix(accent-red 14%)`; color `--accent-red`; border-color `color-mix(accent-red 30%)`.

`.kbd`: font `--font-mono` 11px; color `--text-secondary`; background `color-mix(in srgb, var(--bg-overlay) 50%, transparent)`; padding 2px 6px; border-radius 4px; border `1px solid color-mix(in srgb, var(--bg-surface) 70%, transparent)`.

`.codeInline`: font `--font-mono` 11px; color `--accent-primary`; background `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; padding 1px 6px; border-radius 4px.

`.badge`: inline-grid; place-items center; min-width 18px; height 18px; padding 0 6px; border-radius 9px; font-size 11px; font-weight 600; background `--accent-red` (#f38ba8); color `--bg-primary` (#313240).

## Состояния/варианты
Статичные; hover-состояний нет. Три варианта chip: default (green/active), muted (idle), danger (red/error).
