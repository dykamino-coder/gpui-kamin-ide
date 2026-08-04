# 53 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: glint r16, height mainSplit, welcome-ветка. Расхождения: нет
data-activity-drop (over accent10%+dashed / blocked red12%+inset); BottomTabBar
скрыт при пустом pinned (оригинал всегда); лишний min_h(100); нет flex-shrink 0.

## Цикл 5: DIVERGES

Нет drop-состояний `[data-activity-drop=over|blocked]` (`global.css:53-67`: accent 10% + 1px dashed accent 60% offset −2 / red 12% + inset 2px red 60%). Стрип скрыт при пустом `pinned` — оригинал всегда рисует `.strip` с пикером. Лишняя пилюля «Open Tool ▾»: `MainContent.tsx:55` даёт placeholder БЕЗ `activitySlot`, у нас `slot_panel` всегда передаёт кнопку.

## Цикл 6: DIVERGES

Лишняя пилюля у центральной карты убрана ✓. Осталось: drop-состояния `over`/`blocked`, стрип скрыт при пустом `pinned` (оригинал рисует его всегда).

## Цикл 13: DIVERGES

Закрыто: дроп-подсветка карты приведена к оригиналу — ФОН на всю карту
(`background-color` инсета не имеет), обводка отдельным слоем с
`outline-offset: -2px`, радиус 16 под glint-карту. Раньше был вдвинут и фон,
и у карты оставалась неподсвеченная рамка.

Осталось: `.main { flex-shrink: 0 }` — наша секция сжимаема.

## Цикл 14: DIVERGES

Закрыто по ревью ц.14: заливка дроп-подсветки на glint-картах УБРАНА —
в оригинале её не видно: `.glint-surface { background: … }` идёт в том же
`global.css` ПОЗЖЕ правил `[data-activity-drop]` при равной специфичности и
сбрасывает `background-color` шорткатом. Остаётся только пунктирная обводка;
на баре и рейлах (обычный `<nav>`, не glint) заливка сохранена.

Второе: слой обводки стал `deferred` — CSS-outline красится ПОСЛЕ потомков, а
у нас кольцо было первым ребёнком карты и уходило под тело.

Осталось: `.main { flex-shrink: 0 }` — наша верхняя секция сжимаема.

## Цикл 16: MATCH

Main-карта: `h(relative(main_split))`, glint r16; подавление заливки drop-подсветки верно — `.glint-surface` идёт позже `[data-activity-drop]`.

## Цикл 19: MATCH

Main-карта: доля `main_split`, `flex_shrink_0`, glint r16.
