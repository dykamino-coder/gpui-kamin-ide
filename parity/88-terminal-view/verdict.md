# 88 — verdict (review cycle 1)
VERDICT: DIVERGES
Ядро подтверждено (mantle-карта mx6 mb6, body editor-bg, инсеты 8/22/10/14,
mono13/lh17, cell 7.8×17, cursor editor-cursor).
Расхождения: ранний return «Starting shell…» БЕЗ карты и тулбара (оригинал:
.root+toolbar всегда, .empty absolute); нет .empty (terminal 28 op.6 + «No
terminal yet — pick a shell from the “+” menu.»); scrollback 10000 vs 5000.

## Цикл 5: DIVERGES

Терминал: `.root` (mx6/mb6, bg-mantle, r12) и `.body` (editor-bg, r12, инсеты 8/22/10/14), mono 13, ячейка 7.8×17, курсор `editor_cursor` — подтверждены. Расхождение: при нуле сессий ветка выходит РАНО → голое «Starting shell…» без карты и тулбара; оригинал всегда рисует `.root` + тулбар, а `.empty` (absolute inset 0, gap 8, codicon-terminal 28 op .6 + «No terminal yet — pick a shell from the “+” menu.») лежит ВНУТРИ `.body`. Сессии не per-slot.

## Цикл 6: DIVERGES

При нуле сессий ранний выход без карты и тулбара.

## Цикл 11: DIVERGES

Закрыто: при нуле сессий панель больше не схлопывается в голый текст — рисуются карта
(`mx 6 / mb 6`, bg-mantle, radius-md), тулбар и `.empty` ВНУТРИ тела: codicon-terminal 28
при opacity .6 + «No terminal yet — pick a shell from the “+” menu.» на поверхности
editor-bg.

Осталось: кадр пустого состояния.

## Цикл 16: MATCH

Терминальная карта: margin 0/6/6, bg-mantle, r 12, тело editor-bg; замер тулбара h 30.4 при x = карта + 6 + 25.

## Цикл 16: DIVERGES (обновление)

Закрыто: карта больше не выезжает за правый край панели — отступы
`margin: 0 6 6` перенесены на внешнюю обёртку как padding (замер до правки:
тулбар выступал на 31.2 px).

## Цикл 19: MATCH

Терминальная карта: `margin: 0 6 6` вынесен во внешний padding, инсеты тела 8/22/10/14.
