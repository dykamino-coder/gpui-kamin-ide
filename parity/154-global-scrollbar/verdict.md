# 154 global-scrollbar — verdict (review cycle 1)
VERDICT: MATCH
theme_sync.rs:418-421: track transparent, thumb bg-overlay #515567,
hover text-disabled #60667b, ScrollbarShow::Always; вендорная геометрия 8px/r4 = CSS.
Каверзы (не вердикт-брейкеры): 8px-thumb в 16px-лейне с инсетом 4 (не вплотную
к краю); webview-вариант skeleton.css не портирован (webview_theme.rs шлёт только
--vscode-scrollbarSlider-* переменные).

## Цикл 5: MATCH

Скроллбар: track прозрачный, thumb `bg_overlay` #515567, hover `text_disabled` #60667b, `ScrollbarShow::Always`; вендор при Always даёт width 8 / radius 4 = CSS. Каверза: thumb в 16px-лейне с инсетом 4 (не вплотную к краю); вариант из skeleton.css для вебвью не портирован.

## Цикл 13: MATCH

Ревью подтвердило: режим `Always`, трек прозрачный, ползунок bg-overlay
сплошной, ховер text-disabled, вендорная геометрия 8/4/4 = `global.css`.
Досье переписано: прежний текст утверждал режим Hover и alpha-цвета, а следом
строил на этом три ложных «отличия».

Осталось (записано как отличия, не как расхождения значений): скроллбар в gpui
принадлежит контейнеру, а не документу; вебвью-вариант не портирован.

## Цикл 15: MATCH

Скроллбар: трек прозрачный, thumb `bg-overlay`, hover `text-disabled`, всегда видим; вендорные 8 px/r4.

## Цикл 18: MATCH

Скроллбар: трек прозрачный, thumb `bg-overlay`, hover `text-disabled`, всегда видим, 8 px / r4.
