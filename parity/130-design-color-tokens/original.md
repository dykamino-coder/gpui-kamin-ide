# 130 design-color-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:13-42, design-sections.module.css:1-50, theme/variables.css, theme/dark-theme.css

## Содержание/структура
`ColorTokens()` — `.colorGroups` (flex column, gap `--space-4` 16px) из 4 групп `.colorGroup` (flex column, gap `--space-2` 8px):
- `<h3 class=groupLabel>` — заголовок группы (Surface / Text / Accent / Semantic)
- `.swatches` — grid `repeat(auto-fill, minmax(180px, 1fr))`, gap 8px
- каждый `.swatch`: `.swatchChip` (div с `style="background: var(--<token>)"`) + `<code class=swatchName>--<token></code>`

Группы и токены (порядок из COLOR_GROUPS):
- Surface: bg-primary, bg-base, bg-mantle, bg-sidebar, bg-surface, bg-overlay
- Text: text-primary, text-subtext, text-secondary, text-muted, text-disabled
- Accent: accent-blue, accent-sapphire, accent-teal, accent-green, accent-yellow, accent-orange, accent-red, accent-maroon, accent-pink, accent-purple, accent-rosewater
- Semantic: accent-primary, accent-action, accent-action-hover, accent-action-fg

Никакой JS-резолюции значений — браузер резолвит `var(--token)` при пейнте (dark/light переключается само).

## Метрики
CSS:
- `.groupLabel`: margin 0; font-size `--fs-xs` (11px); text-transform uppercase; letter-spacing 0.06em; color `--text-muted`
- `.swatches`: grid, `grid-template-columns: repeat(auto-fill, minmax(180px, 1fr))`, gap 8px
- `.swatch`: flex, align-items center, gap 8px, padding 8px, background `color-mix(in srgb, var(--bg-surface) 30%, transparent)`, border-radius `--radius-xs` (4px)
- `.swatchChip`: 28×28px, border-radius 4px, border `1px solid color-mix(in srgb, var(--text-primary) 12%, transparent)`, flex-shrink 0
- `.swatchName`: font-family `--font-mono`, font-size 11px, color `--text-secondary`, word-break break-all

Полная таблица токен → значение (дефолтная тёмная тема, dark-theme.css / :root в variables.css):

| Токен | Значение (dark) |
|---|---|
| --bg-primary | #313240 |
| --bg-base | #313240 (alias --bg-primary) |
| --bg-mantle | #262533 |
| --bg-sidebar | #1d1d28 |
| --bg-surface | #3d3f51 |
| --bg-overlay | #515567 |
| --text-primary | #cfd4e2 |
| --text-subtext | #afb6ca |
| --text-secondary | #adb3c7 |
| --text-muted | #838aa0 |
| --text-disabled | #60667b |
| --accent-blue | #89b4fa |
| --accent-sapphire | #74c7ec |
| --accent-teal | #94e2d5 |
| --accent-green | #a6e3a1 |
| --accent-yellow | #f9e2af |
| --accent-orange | #fab387 |
| --accent-red | #f38ba8 |
| --accent-maroon | #eba0ac |
| --accent-pink | #f5c2e7 |
| --accent-purple | #cba6f7 |
| --accent-rosewater | #f5e0dc |
| --accent-primary | var(--accent-blue) → #89b4fa |
| --accent-action | var(--accent-blue) → #89b4fa |
| --accent-action-hover | var(--accent-sapphire) → #74c7ec |
| --accent-action-fg | var(--bg-primary) → #313240 |

Сопутствующие цветовые токены темы, не показанные в свотчах (dark-theme.css / variables.css :root, для полноты палитры):

| Токен | Значение (dark) |
|---|---|
| --glint-border | linear-gradient(135deg, rgba(255,255,255,0.18) 0%, var(--bg-mantle) 22%, var(--bg-mantle) 78%, rgba(255,255,255,0.18) 100%) — в :root-фоллбеке mid-стопы var(--bg-base) |
| --editor-bg | #1d1c25 |
| --editor-fg | #dcdce4 |
| --editor-cursor | #a0a0d0 |
| --overlay-modal | rgba(0, 0, 0, 0.5) |
| --overlay-soft | rgba(0, 0, 0, 0.35) |
| --overlay-deep | rgba(0, 0, 0, 0.6) |
| --bg-surface-hover | #3b3b52 |
| --bg-overlay-hover | #3e3e56 |
| --bg-tint-red | #2e1e22 |
| --bg-tint-red-soft | #45283b |
| --bg-tint-green | #1e2e1e |
| --bg-tint-green-soft | #1e2e1e |
| --bg-tint-orange | #2e1e1e |
| --bg-tint-blue | #1a1a27 |
| --accent-blue-soft | #b4d0fb |
| --accent-blue-soft-2 | #b4befe |
| --accent-blue-soft-3 | #c0d3ff |
| --accent-purple-soft | #b48bef |
| --accent-green-soft | #94d899 |
| --accent-red-dark | #e06c8a |
| --accent-red-dark-2 | #e06c88 |
| --accent-red-dark-3 | #e87c99 |
| --accent-orange-dark | #f9b36d |
| --accent-yellow-dark | #8a7a2e |
| --text-muted-2 | #7f849c |
| --text-muted-light | #acb2d2 |
| --divider-soft | color-mix(in srgb, var(--text-primary) 6%, transparent) |

Семейство semantic-primary алиасов (variables.css): --accent-primary-soft → --accent-blue-soft; --accent-primary-soft-2 → --accent-blue-soft-2; --accent-primary-soft-3 → --accent-blue-soft-3; --bg-tint-primary → --bg-tint-blue; --tint-primary-* → --tint-blue-*.

Tint-токены (color-mix, variables.css:103-151): --tint-red-soft 10%, --tint-red-soft-2 8%, --tint-red-medium 18%, --tint-red-border 30%, --tint-red-border-strong 40% (от accent-red); --tint-blue-soft 6%, --tint-blue-medium 12%, --tint-blue-strong 25%, --tint-blue-border 25%, --tint-blue-border-strong 50% (от accent-blue); --tint-yellow-soft 8%/-medium 12%/-strong 18%/-border 30% (accent-yellow); --tint-green-soft 8%/-medium 14%/-strong 18%/-border 40% (accent-green); --tint-purple-soft 8%/-medium 12%/-border 25% (accent-purple); --tint-orange-soft 14% (accent-orange); --tint-muted-soft 8%/-medium 18% (text-muted); --tint-overlay-scrim 70%/-heavy 92% (bg-sidebar); --tint-surface-soft 40%/-medium 55% (bg-surface); --tint-overlay-medium 50%/-strong 80% (bg-overlay).

## Состояния/варианты
Статичная витрина, интерактива нет. Значения свотчей меняются вместе с темой (`[data-theme="dark"]` / `[data-theme="light"]` на `<html>`; `:root`-фоллбек в variables.css зеркалит dark для первого пейнта).
