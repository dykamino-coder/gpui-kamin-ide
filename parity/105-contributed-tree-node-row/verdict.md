

## Цикл 4: DIVERGES

Строка узла contributed-дерева (label/description/tooltip) не реализована.

## Цикл 8: DIVERGES

Строка узла contributed-дерева не реализована.

## Цикл 10: DIVERGES

Не реализовано (см. 104).

## Цикл 7: DIVERGES

CSS-паритет строки полный: gap 6, h 22, pr 8, отступ `depth*12+8`, border 1px
transparent, r-xs 4, fs-sm, text-secondary; hover bg-surface 55% только у невыделенной;
selected — градиент 90° accent 26→14% + рамка 45%; chevron бокс 16 / глиф 13 / muted и
наследование цвета у выделенной; порядок детей; description ml 6 / op .55 / .85em;
tooltip `tooltip ?? label`.

Исправлено по ревью: команда узла уходила несуществующим методом
`kamin:commands:execute` — теперь `kamin:command:execute` (клик по узлу с командой
работал вхолостую); строка получила `overflow: hidden` + `white-space: nowrap`.

Осталось: DnD и reveal не портированы.

## Цикл 13 (ревью зоны): DIVERGES

Закрыто: шеврон 13 → **16**. Каскад кодиконов: класс, задающий `font-size` НА ТОМ ЖЕ элементе, что и `.codicon`, имеет специфичность (0,1,0) и проигрывает вендорному `.codicon[class*=codicon-]` (0,2,0) — значит в оригинале глиф 16.

Осталось: reveal-действие (`scrollIntoView` + focus + expand); DnD
(`TreeDragAndDropController`).

## Цикл 15: DIVERGES

Осталось: DnD-контроллер дерева, `treeReveal` (scrollIntoView + авто-раскрытие), «…» при усечении.

## Цикл 18: DIVERGES

MATCH по всей геометрии строки, description и тултипу.
Осталось: `treeReveal` (scrollIntoView + focus + авто-раскрытие), DnD, «…» при усечении.

## Цикл 21: DIVERGES

Закрыто: ховер строки красит лейбл.
Осталось: `treeReveal`, DnD, «…».

## Цикл 22 (правка): DIVERGES

Закрыто: «…» при усечении. Многоточие в порте дописывает `text_fit`, движок его не рисует; добавлен `fit_approx` (оценка по числу символов, ±1 символ) и бюджеты по ширине контейнера из probe прошлого кадра. Проверено кадром: «Create new docx d…».

## Цикл 22 (правка 2): MATCH

Закрыт `treeReveal`: `kamin:tree:reveal` раскрывает цепочку `expandPath`
(с дозагрузкой уровней и повторами, пока они едут), выделяет узел, при
`expand` раскрывает его сам и скроллит `block:"nearest"`. Скролл считаем по
bounds строки: `overflow_y_scrollbar_with` кладёт весь список ОДНИМ ребёнком
скролл-области, поэтому `scroll_to_item` там бесполезен — на самом списке
висит отдельный измеряющий `track_scroll`.

`focus` оригинала (фокус кнопки строки) не портируется: у плоских div-ов gpui
фокуса нет; это часть отдельной задачи `:focus-visible` (156).

Проверено живьём командой фикстуры `hello-world.revealLeaf` из свёрнутого
состояния: `Veggies` раскрылся, `Veggies · two` выделен и доскроллен целиком.

Заодно строка переведена с `mouse-down` на `on_click` — перетаскивание больше
не раскрывает и не выделяет узел (в браузере после драга `click` не
приходит; gpui гасит pending-click при активном драге, `div.rs:1569-1576`).

## Цикл 23: MATCH

`treeReveal` (expandPath + select + expand) и DnD (`handleDrag`/`handleDrop` + подсветка цели) подтверждены по коду и живьём; `action.focus` не воспроизводится — у div-ов gpui фокуса нет.
