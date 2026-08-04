# 123 — verdict (review cycle 1)
VERDICT: DIVERGES
НЕТ скрима overlay-modal + backdrop-отмены; нет shadow-modal; title без border-b;
input без bg-base/border/r-sm (+лишний search-глиф); нет .prompt/.empty/.detail/
Cancel; OK без (N); item center/py4/fs-sm/secondary/hover-tint vs baseline/8/fs-md/
primary/accent18%; unchecked-чек muted vs accent; separator без border/uppercase;
фильтр только label.

## Цикл 2: DIVERGES
Нет скрима/shadow-modal/prompt/empty/detail/Cancel/OK(N); item-рецепт; separator; фильтр.

## Цикл 5: DIVERGES

QuickPick — худший в зоне: не было скрима вовсе (**добавлен волной 8**), нет shadow-modal, нет max-h панели (только у списка) вместо 60vh, title без border-bottom, инпут «палитрой» вместо обрамлённого поля bg-base с focus-accent, нет `.prompt`/`.empty`/`.detail`/Cancel, «OK» без «(N)», строка py4/fs12/secondary/hover 8% вместо py8/fs13/primary/accent 18%, невыбранный чекбокс muted вместо accent, separator без border-top и uppercase, фильтр только по label, нет `on_key_down` (Esc держится только на main).

## Цикл 6: DIVERGES

Скрим добавлен ✓. **Волна 10**: `max_h 60vh` перенесён на ПАНЕЛЬ + `overflow_hidden` + `shadow::modal()`. Осталось: обрамлённый инпут bg-base, рецепт строк (py8/fs13/primary/accent 18%), `.prompt`/`.empty`/`.detail`/Cancel, «OK (N)», border-top у separator, фильтр по description, Escape.

## Цикл 7: DIVERGES

ЛОЖНАЯ в обратную сторону: скрима НЕТ вовсе, клик мимо не закрывает. Осталось: инпут без рамки; строки py4 против 8/12 fs-md accent 18%; нет prompt/empty/detail/Cancel; OK без счётчика; separator без border-top; фильтр только по label.


## Цикл 12: DIVERGES

Закрыто почти всё, что было открыто: полноэкранный скрим `--overlay-modal`
(тёмная — чёрный .5, светлая — ink-tinted .28) с закрытием по клику мимо, если
расширение не запросило `ignoreFocusOut`; инпут в собственной рамке (margin 8/12/0,
padding 8/12, bg-base, рамка bg-surface 70%, radius-sm, fs-md); строки 8/12, fs-md,
baseline, ховер accent 18%; `.detail` — моно fs-xs справа с эллипсисом (парсился и
не рисовался вовсе); `.description` fs-sm; separator uppercase с border-top
(первый — без линии); `.prompt` над списком; `.empty` курсивом; кнопка Cancel и
счётчик `OK (N)`; у title и блока кнопок — разделительные линии; фильтр ищет по
label + description + detail.

Осталось: анимации `qpFade` 0.12s нет (в gpui нет CSS-анимаций); проверка кадром
светлой темы.

## Цикл 13: DIVERGES

Закрыто: скрим общий и тем-зависимый (свой `overlay_modal` удалён).

Осталось: «No matching items» и условие пустого состояния по РЕЗУЛЬТАТУ
фильтра; цвет чекбокса accent-primary всегда; кнопка OK — accent-primary без
SEMIBOLD и с accent-action-hover; гейты `matchOnDescription/Detail`;
`renderCodiconText`; Enter в multi-режиме.

## Цикл 13 (добивка): DIVERGES

Закрыто: чекбокс всегда accent-primary (невыбранный был text-muted) и не
садится на базовую линию; пустое состояние считается по РЕЗУЛЬТАТУ фильтра и
пишет «No matching items»; кнопка OK — `accent-primary` с ховером
`accent-action-hover` и без SEMIBOLD.

Осталось: гейты `matchOnDescription`/`matchOnDetail`; `renderCodiconText`;
Enter в multi-режиме; свой `max_h` списка поверх высоты панели.

## Цикл 14: DIVERGES

Закрыто: кегль чекбокса 13 → **16** (каскад); убран лишний глиф лупы в
инпуте (у оригинала поле голое); текст пустого состояния «No matching items»;
счётчик «OK (N)» печатается всегда, включая «OK (0)».

Осталось: цвет невыбранного чекбокса (у оригинала accent-primary в обоих
состояниях), `align-self: center` у галки, кнопка OK на `accent-primary` с
ховером `accent-action-hover` и без SEMIBOLD, гейты
`matchOnDescription/Detail`, `renderCodiconText`.

## Цикл 14 (добивка): DIVERGES

Закрыто по CSS оригинала: чекбокс красится `accent-primary` в ОБОИХ
состояниях и центрируется по вертикали (`align-self: center` — у `Div` в gpui
такого метода нет, поэтому центрируем боксом); кнопка OK — фон
`--accent-primary`, ховер `--accent-action-hover`, СВОЕГО начертания у неё
нет (стояли accent-action, SEMIBOLD и opacity .9).

Проверено на живом окне: панель, поле фильтра без лишнего глифа, строки,
сепаратор и `detail` — на местах.

Осталось: гейты `matchOnDescription`/`matchOnDetail` и `alwaysShow`;
`renderCodiconText` для `$(icon)`; Enter в multi-режиме резолвит первый
элемент вместо всех отмеченных.

## Цикл 15: DIVERGES

Закрыто: глиф `circle-large-outline` (ebb5).
Осталось: высота ряда инпута, фокус-бордер, `.okBtn` без рамки, пустое состояние при нулевом результате фильтра, `matchOnDescription/Detail`, `alwaysShow`, `renderCodiconText`.

## Цикл 17: DIVERGES

Закрыто: гейты `matchOnDescription`/`matchOnDetail`, `alwaysShow`, пустое состояние по результату фильтра, высота панели `min(100vh − 84, 60vh)`.
Осталось: зазор чекбокс↔лейбл в строке с `detail`, `.separator:first-child` по отфильтрованному списку, высота ряда инпута.

## Цикл 20: DIVERGES

Закрыто: `.list { flex: 1 }` — ряд Cancel/OK прижат к низу панели (под ним зияло ~290 px); кегль ряда ввода 13 и его бокс.
Осталось: `gap` между чекбоксом и лейблом в строке с `detail` (его съедает `ml_auto`).

## Цикл 23: DIVERGES

★ Вердикт ц.20 снят: список QuickPick завёрнут в `div().p(4)` БЕЗ `flex_1`, из-за чего `flex_1` самого списка мёртв — под рядом Cancel/OK 294 px пустого mantle. Плюс у инпута пика нет accent-рамки в фокусе, и ряд инпута 42.4 лог. против ≈34 по CSS.

## Цикл 23: DIVERGES

Закрыто в этом цикле: список тянется (обёртка получила `flex_1` + `min_h 0`) — пустого mantle под рядом кнопок больше нет; у инпута появилась accent-рамка. Осталось: ряд инпута 42.4 лог. против ≈34 по CSS — `Input` тянет свой бокс ~26.

## Цикл 26: DIVERGES

Закрыто: Enter больше не резолвит `[0]` — multi отдаёт отмеченные, single
первый ОТФИЛЬТРОВАННЫЙ не-сепаратор (`QuickPickModal.tsx:53,83-87`); фильтр
стал одним источником правды для списка и Enter; `.separator:first-child`
считается по отфильтрованным, а не по исходному массиву; пустое состояние
считает `filtered.length`, где сепараторы участвуют; убран чужой `max-height`
у списка, оставлявший ~60 px пустоты; `.okBtn` получил
`border: 1px solid transparent`, как у Cancel.

Осталось: `renderCodiconText` для label, description и detail — `$(icon)`
сейчас печатается текстом; высота ряда ввода ~42 против ~33; рамка `.input`
вне фокуса должна быть `bg-surface 70 %`, у нас всегда accent;
`letter-spacing: 0.04em` у сепаратора — упор движка

## Цикл 27: DIVERGES

Закрыто: рамка `.input` вне фокуса — `bg-surface 70 %`, accent только при
каретке (CSS:42,48); до правки поле всегда выглядело сфокусированным, даже
когда фокус ушёл на строку списка.

Осталось: `renderCodiconText` для label, description и detail — `$(icon)`
печатается текстом; высота ряда ввода ~42 против ~33;
`letter-spacing: 0.04em` у сепаратора — упор движка

## Цикл 35: DIVERGES

`renderCodiconText` портирован (`ui/codicon_text.rs`): строка разбирается на
куски `$(icon)` / текст ровно как регуляркой оригинала
(`utils/codicon-text.tsx:6-12`), имя резолвится по нашей карте кодиконов,
неизвестное печатается как есть, чтобы текст не терялся. Подключено к
`label`, `description` и `detail` пика — раньше `$(check)` печаталось
буквально. Разбор покрыт юнит-тестом (`splits_like_the_original_regex`:
простая строка, одиночная иконка, текст вокруг, две подряд, незакрытая
скобка).

Трекинг сепаратора `letter-spacing: 0.04em` тоже проставлен (в прошлом цикле
он числился упором движка — упор снят вендорным патчем плана 99).

Осталось: высота ряда ввода ~42 против ~33 — тот же вендорный `Input`, что и
в досье 31
