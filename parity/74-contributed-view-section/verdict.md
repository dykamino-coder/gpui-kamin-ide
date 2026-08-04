# 74 — verdict (review cycle 1)
VERDICT: DIVERGES
Хедер = только uppercase-имя. Нет .viewDescription (ml8, op .55) и .viewBadge
(min-w18 px5 r9 accent bg-base fs.75em); паддинг 12/4/2 vs 4/12; нет ls .04em;
титул не предпочитает meta.title.

## Цикл 5: DIVERGES

Хедер секции contributed-вью `px12 pt4 pb2`, у оригинала `padding: space-1 space-3` = 4/12 симметрично. Нет `.viewDescription` (ml 8, weight 400, opacity .55) и `.viewBadge` (min-w18, px5, r9, bg accent-primary, color bg-base, fs .75em, lh16, tooltip). Титул берётся из `name`, а не `meta.title`.

## Цикл 6: DIVERGES

Хедер вью: паддинги 4/12, `.viewDescription`, `.viewBadge`, титул из `meta.title`.

## Цикл 7: DIVERGES

Хедер переписан: padding 4/12, fs-xs, text-muted, flex-shrink 0, титул
`meta.title ?? name`, `.viewDescription` (ml 8, weight 400, op .55), `.viewBadge`
(ml auto, min-w 18, px 5, r 9, accent-primary, bg-base, .75em, lh 16, tooltip).
Исправлено по ревью: `text-transform: uppercase` наследуется — description и badge
тоже в верхнем регистре.

Осталось: стек нескольких `.view` в одном контейнере (панель показывает первое вью);
`letter-spacing .04em` (нет в gpui).

## Цикл 16: MATCH

Секция вью: `.title` 4/12 uppercase muted, `.viewDescription` ml 8 / .55, `.viewBadge` min-w 18 / r 9 / 0.75em / lh 16 — дословно.

## Цикл 19: MATCH

Секция вью: `.title` 4/12 uppercase muted, description ml 8 / .55, badge min-w 18 / r 9 / 0.75em / lh 16.
