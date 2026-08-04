# 84 — verdict (review cycle 1)
VERDICT: MATCH
Хедер/installBtn/list/empty/groupHeader/сортировка — 1:1 (extensions_panel.rs).
Прим.: ls .04em нет в gpui; реальные data-URL иконки — фаза расширений.

## Цикл 5: MATCH

Extensions-панель 1:1: хедер 4/8/4/12 fs-xs uppercase muted, installBtn 3/8 gap4 border accent 40% bg 14%/hover 26% codicon 12, `.list` 0/8/8 + скролл, `.empty` p12 fs-sm, groupHeader 8/8/4 «TITLE — N», сортировка по displayName.

## Цикл 6: MATCH

Extensions-панель 1:1.

## Цикл 13 (добивка): DIVERGES

Закрыто: сортировка регистронезависимая, как `localeCompare` — байтовый `cmp`
ставил «Zebra» перед «apple».

Осталось: наше состояние загрузки со статус-текстом (в оригинале его нет).

## Цикл 16: MATCH

Панель Extensions: `.header` 4/8/4/12, `.installBtn` accent 14 %/бордер 40 % + кодикон 12, `.groupHeader` fs-xs/600, `.empty` p12.

## Цикл 19: MATCH

Панель Extensions: `.header` 4/8/4/12, `.installBtn` accent 14 % + кодикон 12, `.groupHeader`, `.disabled` .55 — подтверждено кадром.
