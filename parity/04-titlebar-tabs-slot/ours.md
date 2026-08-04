# 04 titlebar-tabs-slot — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:245-255 (контейнер),
crates/shell/src/ui/session_tabs.rs:392-399 (row внутри)

## Структура (gpui-дерево кратко)
```
div (flex, items-center, min_w 0, flex_shrink, overflow_hidden, h_full)
 └ session_tabs row (flex, items-center, min_w 0, overflow_hidden,
                     pl 48px, pr SPACE_3)
```
После слота отдельно идут «+» (28×28) и div.flex_1 (пустота-drag).

## Метрики (из кода, точные)
- контейнер: min_w 0, flex_shrink (НЕ flex:1), overflow_hidden, h_full
- row: pl px(48.0), pr m::SPACE_3 (12)
- скругления/шрифт/цвета: не заданы на слоте (несут чипы)

## Отличия от original.md той же папки
1. flex:1 отсутствует — слот сжимается по контенту (чипы фикс-180px),
   остаток ширины забирает соседний div.flex_1. В оригинале слот тянется.
2. padding: оригинал `0 var(--space-3)` (12/12); у нас pl=48 (сознательно —
   «воздух после quick-actions», юзер просил трижды), pr=12. Слева +36px
   к оригиналу.
3. aria-label="Open sessions" — не применимо.
4. -webkit-app-region: no-drag — заменяет occlude()/stop_propagation на чипах.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — сам слот не интерактивен (`crates/shell/src/ui/titlebar.rs:284-305`); ховер только у «+» внутри слота: bg = микс accent_primary 36% + bg_surface 64% (непрозрачный), fg accent_primary #89b4fa (`titlebar.rs:326-338`)
