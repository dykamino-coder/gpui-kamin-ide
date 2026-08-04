# 114 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет loader/spinner (22×22 border 2.5 accent-action, spin .7s) + fade 180ms; нет
WebviewLoadError+Retry; нет watchdogs 20s / 4s×8 / 180s / 1200ms (__kaminReady).

## Цикл 2: DIVERGES
loader/spinner/Retry/watchdogs отсутствуют.

## Цикл 5: DIVERGES

Вебвью-панель: нет loader-cover (spinner 22×22, border 2.5, top-color accent-action, 0.7s) и fade 180 мс; нет `WebviewLoadError` + Retry; нет вотчдогов 20s / 4s×8 / 180s / `__kaminReady` 1200 мс. Вместо всего — статичное «Loading…».

## Цикл 6: DIVERGES

Нет loader-cover, fade, Retry и вотчдогов — статичное «Loading…».

## Цикл 7: DIVERGES

Loader-cover со спиннером, fade 180ms, Retry и два вотчдога против статического глифа 22 + Loading.

## Цикл 15: DIVERGES

`WebviewPanelView` (контейнер, лоадер с фейдом, спиннер 22×22, ретрай-карточка, вотчдоги) не портирован.

## Цикл 20: DIVERGES

`WebviewPanelView` (контейнер, лоадер с фейдом 180 мс, спиннер 22×22, ретрай, вотчдоги) не портирован.

## Цикл 22: DIVERGES

Закрыто:
- **loader-cover** поверх вебвью: `absolute inset 0`, `bg-surface`, спиннер
  22×22 (кольцо `border 2.5` text-primary 16 %, верхняя четверть
  accent-action, вращение `.7s linear`), фейд 180 мс. Держится до пинга
  страницы (`__kaminReady` оригинала), но не дольше `READY_FALLBACK_MS` 1200 мс.
  Замер: бокс спиннера 28 device px при scale 1.25 = 22.4 лог., фон крышки
  `#3d3f51` = `--bg-surface`.
- **load-watchdog 20 с**: html есть, вью не ожил → карточка `WebviewLoadError`
  с Retry вместо крышки.
- Вращение в gpui: текст не поворачивается, поэтому кольцо статично, а дуга —
  SVG (`icons/spinner-arc.svg`) через `with_transformation(rotate)` +
  `with_animation`.

Осталось:
- crash-watchdog (пинг 4 с ×8 → reload, `BUSY_GRACE_MS` 180 с) и WebView2
  `ProcessFailed`;
- сам `createWebviewPanel` как ВКЛАДКА РЕДАКТОРА (у нас вебвью только
  панельные) — вместе с элементом 115.

## Цикл 23: DIVERGES

Крышка загрузки закрыта и подтверждена: `absolute inset 0`, bg-surface, кольцо 22 (border 2.5, accent-action, 700 мс), фейд 180 мс, `READY_FALLBACK_MS 1200`, при 20 с — карточка Retry. Осталось: это крышка contributed-ВЬЮ, а `WebviewPanelView` как ТАБ редактора в порте отсутствует.

## Цикл 26: DIVERGES

Осталось: сам элемент — вебвью как ВКЛАДКА редактора (`createWebviewPanel`,
схема `kaminwebview://`, sandbox, retain-слой) отсутствует; нет crash-watchdog
(`CRASH_PING_INTERVAL_MS 4000`, `CRASH_MISS_LIMIT 8`, `BUSY_GRACE_MS 180000`,
`ProcessFailed`); таймер `READY_FALLBACK_MS` стартует не по `onLoad`, а с
появления записи крышки
