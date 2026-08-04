# 138 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока Dropdown нет вовсе: в design_panel.rs 10 блоков, среди них его нет (component-samples.tsx:99-143 + .dropdownMenu/.dropdownItem*).

## Цикл 7: DIVERGES

Блок реализован (`design_samples.rs::sample_dropdown`), метрики меню/label/item/hint
сверены и совпали. Ревью нашло три дефекта — все исправлены: глифы пунктов и галка 16
(в `.dropdownItem` кегль кодикона не переопределён → база `.codicon{16px}` из
skeleton.css, не 13), лишний пустой flex-ребёнок в триггере убран (`ds_btn` не
добавляет пустую подпись), ховер работает и на выбранном пункте
(`.dropdownItem:hover` 0,2,0 бьёт `.dropdownItemPicked` 0,1,0).

Осталось: `letter-spacing .04em` (нет в gpui); нет пары кадров — вердикт по коду.

## Цикл 15: DIVERGES

Закрыто: светлая ветка `[data-theme=light] .dropdownItemPicked` (заливка accent, текст/глиф/hint в accent-action-fg, вес 600).

## Цикл 18: DIVERGES

Закрыто: пункт растягивается на ширину меню (дали явные 220 — `w_full` внутри absolute taffy не резолвил, заливка была короче на 58 px), hint выбранного пункта в светлой теме красится в `accent-action-fg`.
Ждёт пиксельного подтверждения.

## Цикл 23: DIVERGES

По коду закрыто (меню 220, hint выбранного в светлой теме), кадром не подтверждено: `ours.png` — общий скрин Design-панели в другой позиции скролла, блока Dropdown в нём нет.

## Цикл 26: DIVERGES

Закрыто: в СВЕТЛОЙ теме выбранный пункт под курсором сохраняет accent-заливку —
`[data-theme=light] .dropdownItemPicked` объявлен позже `.dropdownItem:hover`
при равной специфичности; раньше подпись выцветала на сером фоне.

Осталось: `min-width: 220px` заменён жёсткой шириной, меню не растянется под
длинный пункт; `transition` и `letter-spacing` — упоры движка; пункты не
таб-стопы, у оригинала это `<button>`

## Цикл 33: DIVERGES

Триггер дропдауна — тот же `ds_btn`, значит кольцо `:focus-visible` он получил
вместе со всеми кнопками дизайн-страницы (18 таб-стопов `ds-`).

Осталось: ПУНКТЫ меню таб-стопами не стали (у оригинала это `<button>`);
`min-width: 220px` заменён жёсткой шириной; `transition` — упор движка

## Цикл 35: DIVERGES

Пункты меню стали таб-стопами: у оригинала это `<button>`, значит
`button:focus-visible` (`theme/global.css:38-43`). Проверено `probe focus`
при открытом дропдауне (новый эмит `dsDropdown`): стопы `dark`, `light`,
`system`.

Жёсткая ширина `w(220)` заменена на `min_w(220)` — у оригинала
`min-width: 220px`, и меню обязано расти под длинный пункт.

Осталось: `transition` — упор движка
