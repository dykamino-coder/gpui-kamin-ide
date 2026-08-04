# 132 design-spacing-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:80-94, design-sections.module.css:93-111, theme/variables.css:153-160

## Содержание/структура
`SpacingTokens()` — `.spaceStack` (flex column, gap 8px), 7 строк `.spaceRow`:
`<code class=tokenName>--space-N</code>` + `<span class=tokenValue>NNpx</span>` + `<span class=spaceBar style="width: var(--space-N)">` (полоска-мерка шириной в значение токена).

## Метрики
CSS:
- `.spaceStack`: flex column, gap 8px
- `.spaceRow`: grid `90px 60px 1fr`; align-items center; gap 12px
- `.spaceBar`: height 16px; background `--accent-primary`; border-radius `--radius-xs` (4px); width = `var(--space-N)`
- `.tokenName`: mono 11px `--text-muted`; `.tokenValue`: mono 11px `--text-disabled`

Полная таблица токенов (variables.css):

| Токен | Значение |
|---|---|
| --space-1 | 4px |
| --space-2 | 8px |
| --space-3 | 12px |
| --space-4 | 16px |
| --space-5 | 20px |
| --space-6 | 24px |
| --space-7 | 28px |

Массив values в tsx: ["4px","8px","12px","16px","20px","24px","28px"].

## Состояния/варианты
Статичная витрина, интерактива нет. Цвет полоски `--accent-primary` = #89b4fa (dark).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — `.spaceStack`, `.spaceRow` и `.spaceBar` не задают ни padding, ни margin (design-sections.module.css:94-111); всё расстояние даётся gap 8 (`--space-2`) и gap 12 (`--space-3`), внешний padding 16 приходит от `.sectionBody` (элемент 79)
