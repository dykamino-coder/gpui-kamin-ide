# 63 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: glint r16, modeHeader 6/8/0, web-ветка, плейсхолдер File без пилюли.
Расхождения: верх flex1+низ px (оригинал) vs ratio-механика; лишние on_drop (не видимо).

## Цикл 5: MATCH

Верхняя карта файловой панели: glint r16, modeHeader `pt6 px8 pb0 justify-end`, hint — 1:1.

## Цикл 6: MATCH

Верхняя карта 1:1.

## Цикл 13: DIVERGES

Закрыто: когда файл не выбран, карта показывает `PanelPlaceholder label="File"`
с хинтом «Click a file in any panel, or drag-and-drop one from outside»
(`FilePanel.tsx:120-126`). До этого рисовался `.empty` из `FileViewer`
(codicon-file 36 + «Ctrl+P») — состояние, до которого оригинал не доходит:
FilePanel коротит раньше. Наш порт `.empty` удалён как недостижимый.

Осталось: модель ресайза низа — ratio против px оригинала.

## Цикл 16: DIVERGES

Верхняя карта: у оригинала `flex: 1` (остаток после фикс-высоты ящика), у нас — доля `1 − bottom_ratio`.

## Цикл 19: DIVERGES

Инверсия ведущего размера: у оригинала `.topCard { flex: 1 }` (остаток), у нас доля `1 − bottom_ratio`, а низ — flex-остаток.

## Цикл 19 (доработка): DIVERGES

Закрыто: верх — `flex: 1` (остаток), как `.topCard`; ведущей стала нижняя карта.
Ждёт подтверждения замером после ресайза окна.

## Цикл 23: MATCH

Верхняя карта — `flex: 1` (остаток), ведущая нижняя. Живой замер: 468.6 + 10 + 355 = 833.6 = высота тела.
