# 130 design-color-tokens — вердикт ц.15

Расхождение: `.swatches` — `grid repeat(auto-fill, minmax(180px,1fr))`, у нас
flex-wrap с `min_w(180).flex_grow()`. Замер ревьювера: осиротевшие ряды
`--bg-overlay` и `--accent-rosewater` растягивались на 995 px вместо 192.6,
ряд Semantic шёл 240/240/251/240.

Правка: в vendored gpui добавлен `grid_cols_min(px)` →
`repeat(auto-fill, minmax(<min>, 1fr))`; свотчи переведены на грид, своя
ширина у ячейки снята. Проверено кадром Design-панели: пять равных дорожек,
неполные ряды держат ширину дорожки.

Статус: **MATCH** (остаётся `letter-spacing .06em` у groupLabel — упор в gpui).

## Цикл 15: MATCH

## Цикл 18: MATCH

Грид замерен живьём: 8 дорожек, шаг 206.40, карточка 198.40 — ровно `repeat(auto-fill, minmax(180px,1fr))`; неполные ряды держат дорожку. Правка ц.15 закрыта.
