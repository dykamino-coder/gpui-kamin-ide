# 58 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: cardWithBar, glint r16, split%, label Right, rail-структура.
Расхождения: нет пилюли Open Tool (activitySlot=rightTop); зазор card/rail 4 vs 0;
нет drop-индикации; rail 44 vs 48, gap 2 vs 8, codicon 16/15 vs 18; лишний min_h 100.

## Цикл 5: DIVERGES

`RightPanel.tsx:148` передаёт `activitySlot="rightTop"` → у оригинала в пустом состоянии ЕСТЬ пилюля «Open Tool»; у нас placeholder без `extra`. Плюс нет drop-индикации. Рейл 48, гэпы 8+2, py 12, иконка 18, label «Right» — 1:1.

## Цикл 6: MATCH

Пилюля «Open Tool» добавлена (рецепт 1:1 с `.trigger`). Остаток зоны — общая drop-индикация (см. 53).

## Цикл 13: DIVERGES

Закрыто: пилюля «Open Tool ▾» правой ВЕРХНЕЙ карты открывает пикер ВВЕРХ —
`PanelPlaceholder` всегда `popDirection="up"`, а у нас стояло «вниз».

Осталось: см. общий пункт по подсветке дропа (закрыт в 53).

## Цикл 14: MATCH

Закрыто по ревью ц.14: заливка дроп-подсветки на glint-картах УБРАНА —
в оригинале её не видно: `.glint-surface { background: … }` идёт в том же
`global.css` ПОЗЖЕ правил `[data-activity-drop]` при равной специфичности и
сбрасывает `background-color` шорткатом. Остаётся только пунктирная обводка;
на баре и рейлах (обычный `<nav>`, не glint) заливка сохранена.

Второе: слой обводки стал `deferred` — CSS-outline красится ПОСЛЕ потомков, а
у нас кольцо было первым ребёнком карты и уходило под тело.

Подтверждено: пилюля «Open Tool ▾» открывает пикер вверх, label «Right».

## Цикл 16: MATCH

Верхняя карта правой колонки + рейл: `pl 4 / pr 0`.

## Цикл 19: MATCH

Верхняя карта правой колонки + рейл.
