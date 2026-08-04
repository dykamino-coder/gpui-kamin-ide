# 145 sample-block-wrapper — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:40-56,247-255 (дубликат Block в component-samples-extra.tsx:223-231), design-sections.module.css:155-193

## Содержание/структура
- `ComponentSamples()` — корневая обёртка `.compStack`, перечисляет все sample-блоки (ButtonsRow → … → ExtraSamples)
- `Block({ label, hint?, children })` — обёртка одного примера:
  - `.compRow` (контейнер)
  - `<h3 class=compLabel>{label}</h3>`
  - опц. `<p class=compHint>{hint}</p>`
  - `.compInline` — строка с самими образцами

Block определён дважды (идентично) — в component-samples.tsx и component-samples-extra.tsx.

## Метрики
- `.compStack`: flex column; gap `--space-4` (16px)
- `.compRow`: flex column; gap `--space-2` (8px)
- `.compLabel`: margin 0; font-size `--fs-xs` (11px); text-transform uppercase; letter-spacing 0.06em; color `--text-muted`
- `.compHint`: margin `0 0 var(--space-1)` (0 0 4px); font-size 11px; color `--text-muted`; line-height `--lh-snug` (1.3)
- `.compHint code`: font `--font-mono`; 11px; color `--text-secondary`
- `.compInline`: flex; flex-wrap wrap; gap 8px

## Состояния/варианты
hint опционален. Интерактива нет.
