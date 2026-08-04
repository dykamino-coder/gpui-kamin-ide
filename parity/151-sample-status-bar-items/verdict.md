# 151 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

gap 4, px 8, r-xs, fs 11, глиф 12, ok=green, warn=yellow, brand=accent+500, глифы circle-filled/warning — 1:1. Нет `.item:hover{bg-surface 60%; text-primary}` и .id().

## Цикл 13: DIVERGES

Закрыто: ховер элемента (`bg-surface 60 %` + text-primary). В досье он был
записан как «N/A, семпл статичный» — на деле `.item:hover` в оригинале есть.

Осталось: элементы — `div`, а не `<button tabIndex=-1>`.

## Цикл 15: DIVERGES

Осталось: у оригинала `<button>` без `font: inherit` рисуется UA-шрифтом формы, у нас Bricolage. Рецепт пилюль 1:1.

## Цикл 18: DIVERGES

Осталось: у оригинала `<button>` без `font: inherit` рисуется UA-шрифтом формы, у нас Bricolage. Рецепт пилюли 1:1.

## Цикл 23: DIVERGES

Пилюли — `div`-ы на корневом Bricolage, у оригинала `<button>` с UA-шрифтом формы. Рецепт (gap 4 / px 8 / r-xs / fs 11 / глиф 12 / ok-warn-brand) 1:1.

## Цикл 26: DIVERGES

Закрыто: `cursor: pointer` — глобальное правило `button` из `skeleton.css`,
пилюли оригинала это `<button>`, у нас была стрелка. Ревью сняло ЛОЖНЫЙ пункт,
висевший три цикла: «UA-шрифт `<button>`» — тот же `skeleton.css` задаёт
`font: inherit`, то есть Bricolage, ровно как у нас.

Осталось: пилюли не таб-стопы, у оригинала `<button>`

## Цикл 33: MATCH

Закрыто последнее расхождение: семпл-пилюли стали таб-стопами с кольцом
`:focus-visible`. У оригинала это `<button>`, а `button:focus-visible`
(`theme/global.css:38-43`) даёт кольцо каждому. Проверено probe-командой
`focus`: в списке таб-стопов четыре пилюли семпла
(`smp-sbi:3 active`, `smp-sbi:2 failed`, …).

Ранее в этой же зоне закрыто: `cursor: pointer` из глобального `button`-правила
`skeleton.css` и снят ЛОЖНЫЙ пункт про «UA-шрифт `<button>`» — тот же
`skeleton.css` задаёт `font: inherit`, то есть Bricolage, как у нас.

Рецепт сверен поштучно ещё в ц.26: gap 4, px 8, radius-xs, fs 11, глиф 12,
тона ok/warn/brand, ховер `bg-surface` 60 % + text-primary с перекраской глифа.
