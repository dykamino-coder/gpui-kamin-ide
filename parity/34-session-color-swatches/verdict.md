
## Цикл 3: DIVERGES

Свотчи цвета: ховер-scale в gpui недоступен (нет transform) — отклонение; светлая палитра resolveSessionColor не подключена. Волна 7.

## Цикл 4: DIVERGES

У свотчей появился НЕсуществующий в оригинале `hover(opacity 0.85)` (в оригинале `transform: scale(1.15)`, которого в gpui нет) + не перенесён светлый `resolveSessionColor`. Волна 8.

## Цикл 8: DIVERGES

Выдуманный ховер свотчей **убран волной 15** (в оригинале `transform: scale(1.15)`, в gpui недоступен). Остаётся светлый `resolveSessionColor`.

## Цикл 9: DIVERGES

Выдуманный ховер убран ✓ (`context_menu.rs:133-135`). ОСТАЛОСЬ: светлые варианты цветов (`sessions.ts:21-37` `SESSION_COLORS[].light` + `resolveSessionColor`) — у нас только dark (`context_menu.rs:24-27`), `grep 1e66f5|40a02b|8839ef` по crates/ = 0; затрагивает `sessions_list.rs:100-104` и `session_tabs.rs:35-39`.

## Цикл 13: DIVERGES

Закрыто: `resolveSessionColor` (`sessions.ts:21-37`). В светлой теме каждый
цвет палитры подменяется насыщенным вариантом (blue #1e66f5, green #40a02b,
yellow #df8e1d, peach #fe640b, red #d20f39, mauve #8839ef, teal #179299,
pink #ea76cb); хранится всегда dark-значение. Подмена применена и к свотчам
меню, и к строке сессии, и к чипу титлбара — раньше её не было нигде.

Осталось: `transform: scale(1.15)` у свотча на ховере (в gpui трансформаций
нет).

## Цикл 16: MATCH

Свотчи 16×16 с 2 px бордером, активный — `text-primary`, `.swatchClear` 18×18 с кодиконом 13. `transform: scale(1.15)` на ховере — упор в gpui.

## Цикл 20: MATCH

Свотчи: 8 кругов 16×16, активный с рамкой 2 px `text-primary`, `.swatchClear` 18×18 с `circle-slash` 13; палитра и светлые варианты 1:1.
