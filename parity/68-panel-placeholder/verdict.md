# 68 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: плейсхолдер-рецепт, габарит 28×24, слоты ×2, label/hint/trigger.
Расхождения: рамка глифа на весь бокс 28×24 border1 vs rect-инсет (24×20, stroke
2.4); hint lh 1.4 + max_w 360 — ИСПРАВЛЕНО после ревью (1.3, без max-w);
пилюля не передаётся правым картам.

## Цикл 5: MATCH

PanelIcon сверен поштучно (1.5/4.5/8/4.75/6/7/3.5/9), глиф 28×24, label 16/600, hint 12 lh1.3, пилюля px12 py4 mt4 r8 bg 16%/hover 26% + иконка 10. Отклонение: `bar()` клампит палочки во внутренность рамки → они на 2px короче SVG с каждого конца.

## Цикл 6: MATCH

PanelIcon 1:1; кламп палочек — отмеченное отклонение.

## Цикл 13: DIVERGES

Закрыто: у контейнера появился `text-align: center` — многострочный label
прижимался влево.

Осталось: штрих рамки 2px против `stroke-width: 1.2` (в gpui нет SVG-штриха).

## Цикл 16: MATCH

PanelPlaceholder: глиф 28×24 (scale 2.0), label fs-lg/600, hint fs-sm lh 1.3, пилюля accent 16 %→26 %, `fa-chevron` 10.

## Цикл 19: MATCH

PanelPlaceholder: глиф 28×24, label fs-lg/600, hint lh 1.3, пилюля accent 16 %→26 %.
