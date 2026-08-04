# 09 — verdict (review cycle 1)
VERDICT: DIVERGES
Базовый цвет text-muted, должен text-secondary; svg тумблера жёстко text-secondary
(hover не поднимает до primary); fa-gear 13 vs 12.

## Цикл 2: MATCH

## Цикл 8: DIVERGES

`.active { color: text-primary }` съедается жёстким `text_secondary` на самом `svg()` (`titlebar.rs:234`): у оригинала иконка panel-left (208,212,225), у нас (173,179,199). Единственное расхождение ЦВЕТА в зоне.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Цвет svg тумблера теперь следует active-состоянию, как `currentColor` в `TitlebarQuickActions.module.css` (`.btn` secondary → `.active` primary): `titlebar.rs:236-244`. Замер ink после правки (207,212,226) против оригинала (208,212,225) — разница в пределах сглаживания. Подъём цвета по ХОВЕРУ остаётся недостижим (svg в gpui не реагирует).

## Цикл 9: MATCH

Закрыто. `titlebar.rs:246-250`: цвет svg следует active. Самый яркий ink тумблера (207,212,226) = `--text-primary #cfd4e2` (`dark-theme.css:34`) точно в токен; было (173,179,199) = secondary. Подъём цвета по ХОВЕРУ недостижим — ограничение gpui.

## Цикл 11: MATCH

svg активного тумблера (207,212,226) = --text-primary.

## Цикл 15: MATCH

Кнопка 28×28 r8, active accent 16 %, fa 12, замер 28.0. Ховер не поднимает цвет SVG — упор в gpui.

## Цикл 19: DIVERGES

Закрыто: ховер поднимает и цвет SVG (`group_hover` от группы кнопки) — `.btn:hover { color: text-primary }`.
Ждёт пиксельного подтверждения.

## Цикл 23: MATCH

Кнопка quick-action подтверждена пикселями: без ховера ink `#adb3c7` = text-secondary, под ховером фон `#3d3f51` = bg-surface и ink `#cfd4e2` = text-primary; на активной ховер перебивает `.active`, как `.btn:hover` (0,2,0) > `.active` (0,1,0).
