# 146 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Геометрия верна (strip gap 4 p 4/8; таб h24 px10 gap6 fs11/500, глиф 13, active accent 16%). Нет `.tab:hover{bg-surface 50%; text-primary}` — в sample_tab_strip ни одного .hover() и нет .id().

## Цикл 13: DIVERGES

Закрыто: у неактивного таба семпла появился ховер — `bg-surface 50 %` +
text-primary (`.tab:hover`); его не было вовсе, семпл был мёртвой картинкой.

Осталось: обёртка `.tabs` (flex 1 / min-w 0 / overflow-x); `letter-spacing
.02em`; иконки кодиконами вместо Phosphor-ассетов 13px.

## Цикл 15: DIVERGES

Закрыто: иконки берут токены тулов → Phosphor-ветка `ToolIcon` (terminal/warning), кегль 13.
Осталось: обёртка `.tabs` (flex 1, overflow-x auto), UA-шрифт `<button>`, `letter-spacing .02em`.

## Цикл 18: DIVERGES

Закрыто (подтверждено кадром): Terminal и Problems — Phosphor-SVG, Output — кодикон, все 13 px.
Осталось: слой `.tabs` (flex 1 / overflow-x auto), UA-шрифт `<button>`, `letter-spacing .02em`, ellipsis у лейбла.

## Цикл 23: DIVERGES

Нет слоя `.tabs` (flex 1 / min-width 0 / overflow-x auto) и `.tabLabel` с эллипсисом; `letter-spacing .02em` — упор движка; семпл неинтерактивен, у оригинала `useState` + `onClick`.

## Цикл 23: DIVERGES

Закрыто в этом цикле: таб-стрип-семпл интерактивен — активный таб живёт в `DesignState.strip_tab` (`useState("terminal")` оригинала) и переключается кликом. Осталось: нет слоя `.tabs` (flex 1 / min-width 0 / overflow-x auto) и `.tabLabel` с эллипсисом; `letter-spacing .02em` — упор движка.

## Цикл 23: DIVERGES

Закрыто в этом цикле: появился внутренний слой `.tabs` (flex 1 / min-width 0 / overflow-x auto) вместо прямых детей полосы, и `.tabLabel` с эллипсисом (`BottomTabBar.module.css:14-21,56-60`); ранее закрыта интерактивность. Осталось `letter-spacing .02em` — упор движка.

## Цикл 26: DIVERGES

Закрыто: `.tabLabel` с эллипсисом и `white-space: nowrap` — ц.23 объявила это
закрытым ложно, код с эллипсисом лежал в ДРУГОМ семпле; ховер таба теперь
перекрашивает и глиф через группу, раньше цвет прибивался аргументом на этапе
сборки.

Осталось: `gap` и `flex-shrink: 0` описаны на слое `.tabs`, а не на внешней
полосе; `transition` и `letter-spacing` — упоры движка; табы не таб-стопы

## Цикл 33: DIVERGES

Закрыто: табы семпла стали таб-стопами с кольцом `:focus-visible` — у
оригинала это `<button aria-pressed>`, а `button:focus-visible`
(`theme/global.css:38-43`) даёт кольцо каждому. Проверено `probe focus`:
три таб-стопа `smp-tab:Output`, `smp-tab:Problems`, `smp-tab:Terminal`.

Осталось: `gap` и `flex-shrink: 0` описаны на слое `.tabs`, а не на внешней
полосе; `transition` и `letter-spacing` — трекинг теперь есть (ц.32),
осталось проставить и сверить ширину

## Цикл 35: MATCH

`.strip` получил недостающие `gap: var(--space-1)` и `flex-shrink: 0`
(`BottomTabBar.module.css`) — раньше оба стояли только на внутреннем слое
`.tabs`, и внешняя полоса ими не управляла.

`letter-spacing: 0.02em` у `.tab` проставлен (упор движка снят вендорным
патчем плана 99). Ширина строки при этом трекинге сверена с оригиналом
напрямую в ц.34: «Terminal» 11px/500/0.22px — оригинал 46.50, наш шейпер
46.38.

Осталось только `transition` — общий упор движка по всем элементам.
