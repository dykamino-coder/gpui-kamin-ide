

## Цикл 4: DIVERGES

Contributed TreeDataProvider не реализован: grep по `crates/shell/src` даёт
0 совпадений на `TreeDataProvider|kamin:trees|getChildren`.

## Цикл 8: DIVERGES

Contributed TreeDataProvider не реализован (grep по `TreeDataProvider|getChildren|kamin:trees|checkboxState` = 0).

## Цикл 10: DIVERGES

Contributed-дерево не реализовано: grep TreeDataProvider|treeGetChildren|checkboxState по crates/shell/src = 0; ветка tool_body рисует только вебвью.

## Цикл 7: DIVERGES

Тело портировано; метрики `.body` (4/6/8 + fs-sm), `.loading`/`.emptyChild`
(fs-xs, text-muted, py 2, отступ уровня), message-баннер (4/8, fs-sm, op .75), кап 100 +
«… N more», «(empty)» только на depth 0 — сверены и совпали.

Исправлено по ревью: узел, пришедший УЖЕ раскрытым (`collapsibleState == Expanded`),
теперь добирает свой уровень (раньше висело вечное «Loading…»); `kamin:tree:changed`
больше не затирает уровни в `None` — старое содержимое стоит до прихода нового.

Осталось: DnD не портирован; tree-страницы Customize по-прежнему только вебвью; живой
проверки нет — единственное tree-вью фикстур (`helloTree`) второе в контейнере, а
панель показывает первое (ограничение элемента 73).

## Цикл 13 (ревью зоны): MATCH

Ревью подтвердило посвойственно: тело pt 4 / px 6 / pb 8 + fs-sm + min-h 0,
баннер сообщения px 8 / py 4 / opacity .75, кап 100 + «… N more», «(empty)»
только на depth 0, «Loading…» с индентом уровня.

Осталось (ограничение движка): DnD-контроллер дерева.

## Цикл 14: MATCH

Подтверждено повторно: тело `pt 4 / px 6 / pb 8` + fs-sm + min-h 0, баннер
сообщения, кап 100 с «… N more», «(empty)» только на depth 0, «Loading…» с
индентом уровня.

## Цикл 15: MATCH

Тело contributed-дерева: баннер 4/8 fs-sm .75, `.body` 4/6/8, кап 100, `Loading…`/`(empty)`/«… N more».

## Цикл 18: DIVERGES

Закрыто: тело contributed-дерева получило видимый ползунок (`overflow_y_scrollbar`), как у файлового.
Осталось: DnD-контроллер `TreeDragAndDropController`.

## Цикл 21: DIVERGES

Осталось: DnD-контроллер `TreeDragAndDropController`. Тело, баннер, кап и ползунок — MATCH.

## Цикл 22: MATCH

Закрыт DnD-контроллер. `kamin:tree:dnd` (плюс пул `kamin:tree:hasDnd` при
первом показе — бродкаст уходит в момент регистрации, до нашей подписки)
включает перетаскивание строк; drag шлёт `handleDrag`, drop по строке —
`handleDrop`, подсветка цели `accent 22 % + accent border`, как у файлового
дерева.

Проверено живьём: перетаскивание в `helloTree` дошло до фикстуры — её
`handleDrop` отработал и выставил `message` «Moved 1 → Fruits».
