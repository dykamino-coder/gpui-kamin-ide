# 134 design-shadow-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:117-134, design-sections.module.css:134-153, theme/dark-theme.css:104-113 (variables.css:74-82 — зеркальный фоллбек)

## Содержание/структура
`ShadowTokens()` — `.shadowGrid` из 9 `.shadowItem`:
`.shadowBox` (прямоугольник со `style="boxShadow: var(--shadow-*)"`) + `<code class=tokenName>--shadow-*</code>`.
Порядок из SHADOW_TOKENS: shadow-mini, shadow-card, shadow-bar, shadow-tab, shadow-dropdown, shadow-card-popup, shadow-toast, shadow-lg, shadow-modal.

## Метрики
CSS:
- `.shadowGrid`: grid `repeat(auto-fill, minmax(140px, 1fr))`, gap 16px
- `.shadowItem`: flex column, align-items center, gap 8px
- `.shadowBox`: 100×64px; background `--bg-primary` (#313240 dark); border-radius `--radius-sm` (8px)

Полная таблица токенов (дефолтная тёмная тема):

| Токен | Значение |
|---|---|
| --shadow-mini | 0 2px 8px rgba(0, 0, 0, 0.3) |
| --shadow-card | 0 0 6px rgba(0, 0, 0, 0.2) |
| --shadow-bar | 0 -4px 12px rgba(0, 0, 0, 0.4) |
| --shadow-tab | 0 6px 18px rgba(0, 0, 0, 0.45) |
| --shadow-dropdown | 0 4px 16px rgba(0, 0, 0, 0.5) |
| --shadow-card-popup | 0 8px 24px rgba(0, 0, 0, 0.5) |
| --shadow-toast | 0 10px 40px rgba(0, 0, 0, 0.4) |
| --shadow-lg | 0 8px 16px rgba(0, 0, 0, 0.3) |
| --shadow-modal | 0 8px 32px rgba(0, 0, 0, 0.5) |

## Состояния/варианты
Статичная витрина, интерактива нет. 9 тонов elevation от mini до modal.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — `.shadowGrid`, `.shadowItem` и `.shadowBox` не задают padding/margin (design-sections.module.css:135-153); расстояния только gap 16 (`--space-4`) в гриде и gap 8 (`--space-2`) в ячейке
