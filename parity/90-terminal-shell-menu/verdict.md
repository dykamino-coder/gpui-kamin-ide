# 90 — verdict (review cycle 1)
VERDICT: DIVERGES (мелочи)
Меню/menuItem/itemIcon/defaultTag/starBtn подтверждены 1:1.
Расхождения: shadow 0/6/24 vs --shadow-dropdown 0/4/16; нет .menuEmpty; absolute
вместо portal+clamp+max-h 100vh-16.

## Цикл 5: DIVERGES

Меню шеллов: min-w200, bg-surface, divider-soft, r12, dropdown-shadow, p4 gap1, пункт 8/12 r-sm fs-sm hover 10%, itemIcon w16 center muted codicon 12, defaultTag fs-xs uppercase, starBtn 24 hover 10%, `starOn` accent — 1:1. Остаток: `top(30)` без `POPUP_OFFSET_PX = 6` (меню вплотную к кнопке); нет `max-height: calc(100vh - 16px)` + скролла и клампа к вьюпорту; нет `.menuEmpty` «No shells discovered»; иконка пункта всегда terminal (оригинал `s.icon ?? "terminal"`); нет закрытия по Escape.

## Цикл 6: DIVERGES

Меню шеллов: `top(30)` без offset 6, нет max-height/скролла, `.menuEmpty`, `s.icon`, Escape.

## Цикл 11: DIVERGES

Закрыто: `POPUP_OFFSET_PX = 6` от нижней кромки кнопки; `max-height: calc(100vh − 16px)`
с прокруткой; `.menuEmpty` «No shells discovered» при нуле профилей.

Осталось: иконка пункта захардкожена `codicon-terminal` (у профилей порта своего поля
`icon` нет); `.menuRow { gap: 2px }` между пунктом и звездой — у нас общий gap 8.

## Цикл 13: DIVERGES

Закрыто: иконка пункта 12 → **16** (у `.itemIcon` кегля нет → база codicon).

Осталось: иконка пункта захардкожена `terminal` вместо `s.icon`; gap 2 между
пунктом и звездой; закрытие по Escape.

## Цикл 16: DIVERGES

Осталось: звезда должна быть СНАРУЖИ `.menuItem` (замер: правый инсет 17.6 вместо ~5, зазор «DEFAULT → звезда» 8 вместо 14); ховер красит всю строку вместо `.menuItem`; поповер анкорится `right: 0` вместо центровки по кнопке.

## Цикл 16: DIVERGES (обновление)

Закрыто по замечанию пользователя: меню перечисляет ВСЕ найденные шеллы
(Windows PowerShell, PowerShell 7, Command Prompt, Git Bash, WSL-дистрибутивы)
и рисует глиф ПО ПРОФИЛЮ (`terminal-powershell` / `-cmd` / `-bash` / `-linux`)
вместо общего `codicon-terminal`. Проверено кадром.

Осталось: звезда должна быть СНАРУЖИ `.menuItem` (правый инсет ~5 вместо
17.6), ховер красит только `.menuItem`, поповер центрируется по кнопке.

## Цикл 19: DIVERGES

Закрыто: звезда снаружи `.menuItem`, ховер только на строке-кнопке, глиф по профилю, список всех шеллов.
Осталось: центровка поповера по якорю вместо `right: 0`.

## Цикл 19 (доработка): DIVERGES

Закрыто: меню шеллов центрируется ПО ЯКОРЮ (`left = a.left + a.width/2 − p.width/2` с клампом гуттером 8, пересчитанным в координаты якоря), а не прижато `right: 0` — при min-w 200 и кнопке 28 это было ~86 px мимо.
Ждёт подтверждения замером.

## Цикл 23: MATCH

Меню шелла центрируется по якорю с клампом `[8, vw − 200 − 8]` и переводом в координаты якоря; `right: 0` остался фоллбэком до замера.
