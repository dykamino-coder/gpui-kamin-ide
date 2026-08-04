
## Цикл 3: MATCH

Порядок рейла bottom-карты {picker, list} с justify-end — совпал.

## Цикл 4: DIVERGES

Drag-ghost — текстовая пилюля вместо квадрата 28×28 с иконкой (accent 22% на bg-surface, border accent 50%, shadow 0 4 14 .35, opacity .92, центр на курсоре). Волна 8.

## Цикл 8: DIVERGES

Drag-ghost: нужен квадрат 28×28 r-sm (accent 22% на bg-surface, бордер accent 50%, глиф accent-primary, shadow 0 4 14 /35%, opacity .92, центр НА курсоре) вместо текстовой пилюли со смещением.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Ghost переписан по `ActivityDragGhost.module.css`: 28×28, radius-sm, фон = НЕПРОЗРАЧНЫЙ микс accent 22% + bg-surface, рамка accent 50%, глиф тула 18px цветом accent-primary, shadow 0 4 14 /35%, opacity .92, левый/верхний край = курсор − 14 (эквивалент `translate(-50%,-50%)`). Была текстовая пилюля с лейблом и смещением (+10,+8), причём её фон менялся от наличия цели — выдумка, в оригинале один класс без вариантов. `root.rs:5907-5969`; глиф вынесен в `activity_bar::tool_glyph()` (= `ToolIcon.tsx`, дефолт 18).

## Цикл 9: DIVERGES

Все СЕМЬ утверждений волны 16 подтверждены по коду (root.rs:6018-6069): 28x28, radius-sm, непрозрачный микс accent 22%+bg-surface, рамка accent 50%, shadow 0 4 14 /35%, opacity .92, центр на курсоре. ОСТАЛОСЬ: кегль глифа 18 безусловно (у оригинала codicon-тулы 16 по каскаду); lookup builtin-only -> contributed даёт circle-large, которого нет в codicon_glyph; ghost в ГЛАВНОМ окне, не в overlay, вебвью на время драга не гасятся.

## Цикл 13: DIVERGES

Закрыто: неизвестные имена иконок больше не падают в codicon-file — резолв идёт через
общую карту `codicon_map`, поэтому `circle-large` и прочие известные имена рисуются
правильно (своя 4-строчная таблица её игнорировала).

Осталось: кегль глифа ghost 18 против 16 по каскаду оригинала; ghost рисуется в
главном окне, а не в overlay.

## Цикл 13 (добивка): DIVERGES

Закрыто: иконка ghost'а берётся из ОБЩЕГО реестра — у contributed-тула была
заглушка `circle-large`; codicon-ветка уменьшена до 16 (`.ghost` своего
правила для `.codicon` не задаёт).

Осталось: ghost рисуется в main-окне, а не в overlay.

## Цикл 17: MATCH

Ghost 28×28, центр строго на курсоре, рамка accent 50 %, непрозрачный микс accent 22 % + bg-surface, тень 0 4 14 /35 %, opacity .92.

## Цикл 21: MATCH

Ghost 28×28, центр строго на курсоре, заливка = непрозрачный микс accent 22 % + bg-surface при opacity .92, рамка accent 50 %.
