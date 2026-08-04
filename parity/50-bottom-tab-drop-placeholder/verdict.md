
## Цикл 3: MATCH

Active-таб accent 16% + text-primary.

## Цикл 4: DIVERGES

Drop-плейсхолдер таба (36×24 dashed) заменён на `border_l_2` у целевого таба; вставки в конец нет. Волна 8.

## Цикл 8: DIVERGES

Drop-стаб таба 36×24 dashed не реализован (у нас `border_l_2` на целевом табе), вставки в конец нет.

## Цикл 9: DIVERGES

36x24 dashed не реализован; вместо него border_l_2 на целевом табе (slot_panel.rs:116); вставки в конец нет.

## Цикл 10: DIVERGES

Реализован: `slot_panel::drop_placeholder` — 36×24, radius-sm 8, рамка 1px dashed
accent-primary 70%, фон accent-primary 14%; вставляется по `overIndex` внутрь `.tabs`
и в конец при `overIndex == pinned.len()`. Прежняя индикация (`border-left` на
соседнем табе) убрана.

Осталось: кадр состояния drag.

## Цикл 17: MATCH

Плейсхолдер таба 36×24, пунктир accent 70 % + заливка 14 %, r-sm; вставка по позиции.

## Цикл 21: MATCH

Плейсхолдер таба: 36 × 23.6, заливка accent 14 %, пунктир accent 70 % — точное совпадение.
