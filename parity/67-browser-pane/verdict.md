# 67 — verdict (review cycle 1)
VERDICT: DIVERGES
Все фиксы подтверждены (навбар 4/6, addr h26 bg-base focus-accent, hover
surface-hover, инсет 6). Остаток: codicon navBtn 14 vs 16; viewport без r-md (невидимо).

## Цикл 5: MATCH

Браузер-пейн: навбар `py4 px6 gap4`, navBtn 26 + hover bg-surface-hover, адресная строка h26 px10 bg-base + focus accent, вьюпорт инсет 6. Мелочь: у div-вьюпорта нет `radius-md` (скругление даёт dcomp-клип).

## Цикл 6: MATCH

Браузер-пейн 1:1 (радиус вьюпорта даёт dcomp-клип).

## Цикл 16: DIVERGES

Закрыто: `.viewport` браузера снова 12 — радиус масок теперь хранится ПО ЗОНЕ (`set_zone_mask_radius`), общая константа 16 была регрессией ц.15.
Осталось: `onFocus → select()` в адресной строке.

## Цикл 19: DIVERGES

Закрыто: `.viewport` браузера снова 12 (радиус хранится по зоне).
Осталось: `onFocus → select()` в адресной строке.

## Цикл 23: DIVERGES

Радиус 12 подтверждён. Осталось: `onFocus → select()` адресной строки; в wry-ветке у `.viewport` нет скругления вовсе.

## Цикл 24: MATCH

`onFocus → select()` порт: `browser_select_all_on_focus` шлёт `SelectAll` при
получении фокуса адресной строкой и сбрасывает флаг при потере. Проверено
живьём: клик по строке — весь URL подсвечен, курсор в конце.

Претензия ц.23 «в wry-ветке у `.viewport` нет скругления вовсе» не
подтверждается: `set_zone_mask_radius("browser", RADIUS_MD)` живой, углы
поверхности WebView2 срезаны — на кадре скругление режет и страницу, и её
горизонтальный скроллбар (нижний левый угол).
