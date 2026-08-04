
## Цикл 3: MATCH

Каркас sessions-mode совпал; flex_shrink на колонке и «customize держит сайдбар видимым» — на волне 7.

## Цикл 4: DIVERGES

`.sidebar` = `flex-shrink: 1`, у нас `flex_shrink_0`; и `Sidebar.tsx:52` рисует нав при `customize` даже со скрытым сайдбаром — у нас `when(sidebar_visible)`. Волна 8.

## Цикл 8: DIVERGES

`flex-shrink: 1` против нашего `flex_shrink_0`; нав Customize скрывается вместе с сайдбаром (оригинал держит его видимым). **Ширина исправлена волной 15**: `gap_wrap` съедал 8px тела — теперь `w(sidebar_w + 8)`.

## Цикл 9: DIVERGES

Ширина ЗАКРЫТА: probe `sidebar` w=222.4 = 215+8; активная строка 4.0-210.4 (W-8), у оригинала 4.0-216.0 (W-8 при 220); левый край контента лог. 60 у обеих. Нав Customize при скрытом сайдбаре ЗАКРЫТ: `root.rs:5727` `.when(sidebar_visible || customize_open)` = `Sidebar.tsx:24`. ОСТАЛОСЬ: `Sidebar.module.css:10` `flex-shrink:1` против нашего `.flex_shrink_0()` (`root.rs:5735`) — нужен `flex_shrink(1.)` + `min_w(PANEL_MIN_SIZE)`. Drop-таргет сайдбара (`Sidebar.tsx:58-61`, `data-activity-drop`) не реализован.

## Цикл 13: DIVERGES

Закрыто: карта сайдбара сжимается (`flex-shrink: 1` + `min-width: 100`), а не
выталкивает соседей — стоял `flex_shrink_0`.

Осталось: aria-разметки нет (ограничение порта).

## Цикл 16: MATCH

`min-width` 100 = `SIDEBAR_MIN_WIDTH_PX`, прозрачный фон, рендер-гейт `sidebar_visible || customize_open` — 1:1.

## Цикл 20: MATCH

Гейт рендера, `flex-shrink` + min-w 100, прозрачный фон; замер списка 223.2 = 231.2 − 4 − 4.
