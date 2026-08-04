
## Цикл 3: DIVERGES

Пульс working-точки: был sin 1s с opacity 1→0.4 без масштаба; оригинал 1.1s, opacity 0.5↔1, scale 1→1.5. Исправлено волной 6 (внутренний абсолютный кружок — transform в gpui нет).

## Цикл 4: MATCH

Точка статуса: 1.1 s, opacity 0.5↔1, «scale» 1→1.5 абсолютным внутренним кружком в боксе 6px; приоритет working > bridgeStatus > active-tab-color 1:1.

## Цикл 8: MATCH

Точка статуса 1:1 (1.1s, 0.5↔1, «scale» внутренним кружком, приоритет working > bridgeStatus > tab-color).

## Цикл 16: MATCH

Точка 4×4, `data-bridge` перебивает `.active`, working = 6 px accent-blue с пульсацией; тултипы статусов. `transform: scale` эмулирован анимацией внутреннего круга.

## Цикл 20: MATCH

Точка статуса: приоритеты bridgeWorking > bridgeStatus > active, working 6 px с пульсом 1.1 с, тултипы.
