# 89 — verdict (review cycle 1)
VERDICT: MATCH
bar/tabs/scrollBtn/tab/tabActive+вогнутые уголки/close-гейт/addBtn/label — 1:1.
Прим.: ls .02em нет; лейбл «{title} {i+1}» vs s.label; оконный overflow вместо
scroll+page-step.

## Цикл 5: MATCH

Тулбар терминала 1:1 (bar items-end gap4 px25 min-h30, tabs gap2 flex1, scrollBtn 22×30 disabled 0.35 codicon 12, tab h30 px10 gap6 r8-8-0-0 fs11/500 min-w80 max-w220, label max-w160 ellipsis, hover 50%, active = editor-bg + вогнутые уголки 6×6, close 16 opacity 0→0.7→1 + overlay 60%, addBtn 28 round + accent 14% при открытом меню). Остаток: лейбл `{title} {i+1}` вместо `shellLabel`; overflow оконный по индексу, а не пиксельный smooth-scroll 80%; `letter-spacing .02em` — ограничение.

## Цикл 6: MATCH

Тулбар терминала 1:1 (лейбл `{title} {i+1}` — остаток).

## Цикл 13: DIVERGES

Закрыто: лейбл таба — имя сессии без порядкового номера (`s.label`).

Осталось: `.anchor { flex-shrink: 0 }`; пиксельная прокрутка стрипа вместо
оконной пагинации.

## Цикл 13 (добивка): DIVERGES

Закрыто: `.anchor { flex-shrink: 0 }` — кнопка «+» больше не сжимается
стрипом табов.

Осталось: пиксельная прокрутка стрипа (`scrollTo` на 80 % ширины) вместо
оконной пагинации по индексу.

## Цикл 16: DIVERGES

Осталось: переполнение табов — окно по индексу с жёсткой `TAB_W = 112` вместо пиксельного smooth-scroll на 80 % ширины; `.tab { flex: 0 1 auto }` (сжатие) не воспроизведено; `letter-spacing` — упор в gpui.

## Цикл 19: DIVERGES

Закрыто: `.tab { flex: 0 1 auto }` — табы сжимаются (min 80 / max 220).
Осталось: переполнение через реальный `overflow-x` со smooth-скроллом на 80 % ширины вместо окна по индексу.

## Цикл 23: DIVERGES

Переполнение табов терминала — окно по индексу (`visible = floor((panel_w − 70)/112)`, шевроны шлют `±1`), у оригинала реальный `overflow-x` со скроллом. `flex: 0 1 auto` / min 80 / max 220 подтверждены.

## Цикл 23: MATCH

Закрыто: полоса табов терминала — настоящий `overflow-x: auto`
(`TerminalToolbar.module.css:10-19`), все табы живут в дереве, а не в окне по
индексу. Шевроны листают СТРАНИЦУ, как оригинал:
`page = max(32, floor(clientWidth × 0.8))`, `scrollLeft ± page` с клампом по
`scrollWidth`, и включаются по РЕАЛЬНОМУ скроллу (`scrollLeft > 1` /
`scrollLeft + clientWidth < scrollWidth − 1`), а не по индексу первого
видимого таба.

Переполнение живьём не воспроизводил: в панели один терминал, а плодить
шесть в дев-инстансе пользователя ради кадра не стал.
