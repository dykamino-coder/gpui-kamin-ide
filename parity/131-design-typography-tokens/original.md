# 131 design-typography-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:44-78, design-sections.module.css:52-91, theme/variables.css:172-204

## Содержание/структура
`TypographyTokens()` — `.typoStack` (flex column, gap `--space-3` 12px):
1. `.typoSample` (flex column, gap 2px): `<code class=tokenName>--font-sans</code>` + `<span style="font-family: var(--font-sans); font-size: var(--fs-lg)">Bricolage Grotesque — quick brown fox 0123456789</span>`
2. `.typoSample`: `<code class=tokenName>--font-mono</code>` + `<span style="font-family: var(--font-mono); font-size: var(--fs-md)">JetBrains Mono — quick brown fox 0123456789</span>`
3. `.typoScale` — 5 строк `.typoRow` по FS_SCALE: `<code class=tokenName>--fs-*</code>` + `<span class=tokenValue>NNpx</span>` + `<span style="font-size: var(--fs-*)">The five steps</span>`

## Метрики
CSS:
- `.typoStack`: flex column, gap 12px
- `.typoSample`: flex column, gap 2px
- `.typoScale`: flex column, gap 8px; margin-top 8px; padding-top 12px; border-top `1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent)`
- `.typoRow`: grid `90px 60px 1fr`; align-items baseline; gap 12px
- `.tokenName`: font `--font-mono`, 11px, color `--text-muted`
- `.tokenValue`: font `--font-mono`, 11px, color `--text-disabled`

Таблица токенов (variables.css):

| Токен | Значение |
|---|---|
| --font-sans | 'Bricolage Grotesque Variable', 'Bricolage Grotesque', -apple-system, sans-serif |
| --font-mono | 'JetBrains Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace |
| --fs-xs | 11px |
| --fs-sm | 12px |
| --fs-md | 13px |
| --fs-lg | 16px |
| --fs-xl | 22px |

Легаси-алиасы (в variables.css, в витрине не показаны): --fs-xxs → --fs-xs; --fs-10 → --fs-xs; --fs-base → --fs-sm; --fs-15 → --fs-md; --fs-18 → --fs-lg; --fs-2xl → --fs-lg.

Line-height токены (variables.css:199-204, в витрине не показаны): --lh-none 1; --lh-snug 1.3; --lh-normal 1.4; --lh-base 1.5; --lh-relaxed 1.6.

FS_SCALE в tsx дублирует значения строками: fs-xs 11px, fs-sm 12px, fs-md 13px, fs-lg 16px, fs-xl 22px.

Шрифт Bricolage Grotesque самохостится через `@import "@fontsource-variable/bricolage-grotesque"` (global.css:9); в списке два имени семейства — Variable (реальный бандл) и легаси 'Bricolage Grotesque' для копипасты из Bridge.

## Состояния/варианты
Статичная витрина. Демо-строки: sans-образец на `--fs-lg` (16px), mono-образец на `--fs-md` (13px), шкала — фраза «The five steps» в каждом из 5 размеров.
