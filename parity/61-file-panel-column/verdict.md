# 61 — verdict (review cycle 1)
VERDICT: DIVERGES
Ширина ratio×viewport vs px-персист; flex_shrink_0 vs shrink1+min-w100; нет fill.

## Цикл 5: DIVERGES

Ширина хранится как `filePanelWidthRatio` от вьюпорта, у оригинала `filePanelWidth` в px; `flex_shrink_0` вместо shrink 1 + min-w 100; fill-режима нет.

## Цикл 6: DIVERGES

Ширина файловой панели всё ещё ratio от вьюпорта вместо px; `flex_shrink_0`; fill нет.

## Цикл 11: DIVERGES

Претензия «ratio вместо px» СНЯТА: оригинал сам хранит ratio
(`signals/layout-ratios.ts:21,28-31`), `crates/metrics/src/layout_math.rs` — построчный
порт.

Осталось: колонка `flex_shrink_0` без `min_w(100)` против `.filePanel { flex-shrink: 1 }`
+ `minWidth: 100`; fill-режим не портирован.

## Цикл 16: DIVERGES

Ширина файловой колонки у нас — доля вьюпорта, у оригинала ПИКСЕЛИ (`FilePanel.tsx:91-98`): при ресайзе окна колонка масштабируется, оригинал держит px.

## Цикл 19: MATCH

Файловая колонка: живая ширина в px при драге, ratio только на персист, пересчёт при смене вьюпорта — совпало с `layout-ratios.ts`.
