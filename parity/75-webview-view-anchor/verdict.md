# 75 — якорь contributed-вью: вердикт цикла 14

## Что было расхождением

`.frame { border-radius: var(--radius-lg) }` = 16. Правка цикла 13 поставила
16 только в wry-ветке, а боевой путь — visual hosting: `webview_body_dyn`
уходит в `visual_wv_body` раньше, клип зоны шёл с `radius = 0.0`, а вырез
фона под вебвью скруглялся `RADIUS_MD` = 12. Видимый радиус — 12/0, не 16.

## Правка

- `root.rs` — в `sync_zone_view` передаётся `m::RADIUS_LG * sf` вместо `0.0`
  (клип DComp-визуала в физических px, отсюда множитель scale factor).
- `ui/glint.rs` — вырез фона `const R: f32 = m::RADIUS_LG` вместо `RADIUS_MD`.

## Проверка

Пересборка + перезапуск, скрин главного окна: углы тел вебвью-панелей
(«Claude Bridge», «Bridge Console») скруглены заподлицо с рамкой карточки,
подложка в углах не просвечивает.

Статус: **MATCH**.

## Цикл 16: DIVERGES

Закрыто: радиус рамки вью 16 на боевом пути; регрессия по браузеру снята (см. 67).
Осталось: `.frameFlush` для contributed-страницы Customize (у нас инсеты 8/8/8/16 вместо edge-to-edge).

## Цикл 19: DIVERGES

Радиус рамки вью 16 закрыт.
Осталось: `.frameFlush` для contributed-страницы Customize (у нас инсеты вместо edge-to-edge).

## Цикл 23: DIVERGES

Осталось `.frameFlush`: в composition-режиме тело contributed-страницы Customize идёт с инсетами 8/8, у оригинала — edge-to-edge без радиуса.

## Цикл 23: MATCH

Закрыто в этом цикле: тело contributed-страницы Customize идёт `.bodyFlush` — edge-to-edge, без инсетов 8/8 и без радиуса; вебвью красит себя сам (`CustomizePanel.module.css:36-42`).
