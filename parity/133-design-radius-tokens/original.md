# 133 design-radius-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:96-115, design-sections.module.css:113-132, theme/variables.css:162-170

## Содержание/структура
`RadiusTokens()` — `.radiusGrid` из 4 `.radiusItem`:
`.radiusBox` (квадрат со `style="borderRadius: var(--radius-*)"`) + `<code class=tokenName>--radius-*</code>` + `<span class=tokenValue>Npx</span>`.

## Метрики
CSS:
- `.radiusGrid`: grid `repeat(auto-fill, minmax(120px, 1fr))`, gap 12px
- `.radiusItem`: flex column, align-items center, gap 4px
- `.radiusBox`: 80×80px; background `--bg-surface` (#3d3f51 dark); border `1px solid color-mix(in srgb, var(--accent-primary) 50%, transparent)`; border-radius = токен

Полная таблица токенов (variables.css):

| Токен | Значение | Назначение (комментарий в css) |
|---|---|---|
| --radius-xs | 4px | chips, badges, inline code, micro buttons |
| --radius-sm | 8px | cards inside cards: code blocks, tables, plugin grid items |
| --radius-md | 12px | level-1 cards: chat bubbles, button groups, capsule buttons |
| --radius-lg | 16px | level-0 panels: mainPanel, terminal panel, asst-merge container |
| --radius-xl | 16px | alias --radius-lg (legacy callers), в витрине не показан |

RADIUS_TOKENS в tsx: radius-xs 4px, radius-sm 8px, radius-md 12px, radius-lg 16px. Правило шкалы (комментарий variables.css): outer = inner + padding — концентрическая 4-ступенчатая шкала с якорем 16px.

## Состояния/варианты
Статичная витрина, интерактива нет.
