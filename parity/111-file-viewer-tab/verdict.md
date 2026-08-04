# 111 — verdict (review cycle 1)
VERDICT: DIVERGES
pl8/pr4 vs 10/6; нет ls .02em; порядок pin/icon/label; dirty XOR close (оригинал ОБА);
close всегда видим (нет 0-.7-1 + bg-overlay60%), r3 vs 4; нет tabDragging .3.
h24/gap6/fs11/hover/active — 1:1.

## Цикл 2: DIVERGES
Нет ls .02em; нет tabDragging .3; тултип close не «Discard & close» при dirty.

## Цикл 5: DIVERGES

Таб вьюера: pl10/pr6, pin `codicon-pinned` op .7, dirty И close вместе, close 0→.7→1 + overlay 60%, r-xs — исправлено. Остаток: нет `.tabDragging` opacity .3; тултип close всегда «Close» (у оригинала «Discard & close» при dirty); letter-spacing .02em — ограничение gpui.

## Цикл 6: DIVERGES

Нет `.tabDragging`; тултип close без варианта «Discard & close» при dirty.

## Цикл 7: DIVERGES

Нет tabDragging opacity .3; тултип close всегда «Close» вместо «Discard & close»; dirty квад 6x6 вместо глифа fs10.

## Цикл 12: DIVERGES

Закрыто: dirty-маркер — глиф «●» кеглем 10 (`.dirty::before`), а не нарисованный
квадрат 6×6.

Осталось: `.tabDragging { opacity: .3 }`; `letter-spacing .02em` — нет в gpui.

## Цикл 13: DIVERGES

Закрыто: `.tab { flex-shrink: 0 }` — при переполнении табы больше не сжимаются.

Осталось: `letter-spacing: .02em` и `transition` (ограничения движка);
`.tabDragging` для файловых табов; состав RMB-меню.

## Цикл 14: DIVERGES

Закрыто: кегль пина 11 → **16** (`.pinIcon` стоит на самом `<i class="codicon
…">` → каскад отдаёт базу); та же ошибка ушла в `tab_width` — там же
исправлено, плюс ширина глифа ● уточнена до ≈0.8 em.

Осталось: `letter-spacing .02em`; состав RMB-меню (у оригинала это меню
дерева с добавочной группой «tab»).

## Цикл 15: DIVERGES

Осталось: `letter-spacing .02em`, `.tabDragging` opacity .3, `line-height: 1` у dirty-точки, иконка расширения у webview-таба.

## Цикл 20: DIVERGES

Закрыто: `.tabDragging { opacity: .3 }` подтверждён кадром.
Осталось: ховер должен красить и подпись (лечится `group_hover`), `line-height: 1` у dirty-точки, иконка вебвью-таба, `letter-spacing` и `transition` — упоры.

## Цикл 23: DIVERGES

Ховер таба подтверждён замером (#adb3c7 → #cfd4e2, фон #323242). Осталось: `line-height: 1` у dirty-точки, иконка вебвью-таба (см. 115); letter-spacing и transition — упоры движка.

## Цикл 24: DIVERGES

Закрыто: `.dirty { line-height: 1 }` (`FileViewerTabs.module.css:127-131`) —
у глифа ● теперь своя строчная высота 10, а не наследуемая от таба; то же и в
меню переполнения.

Осталось: иконка вебвью-таба (см. 115 — подсистема
`createWebviewPanel`-как-таб, нужно решение), `letter-spacing .02em` и
`transition` — упоры движка.

## Цикл 26: DIVERGES

Закрыто: `.label { white-space: nowrap }`. Каскад пина (16) и крестика (11)
ревью пересчитало независимо — обе ветки в порте разрешены верно.

Осталось: состав RMB-меню — оригинал открывает МЕНЮ ДЕРЕВА с доп-группой `tab`
и `builtin: !isWebview`, а у нас своё меню шириной 220 с пунктом Pin/Unpin,
которого в UI оригинала нет вообще, и пунктом «File actions…»; дроп-индикатор
рисуется на no-op позициях и не рисуется после последнего таба; иконка
вебвью-таба (115); `letter-spacing` и `transition` — упоры движка

## Цикл 32: DIVERGES

`letter-spacing` БОЛЬШЕ НЕ УПОР ДВИЖКА — трекинг реализован вендорным патчем
(план 99): `shape_text_spaced` → `layout_wrapped_line_spaced` →
`layout_line_spaced` → сдвиг глифов рядом с проходом `force_width`, плюс
`TextStyle.letter_spacing` и `Styled::letter_spacing()`. Значение проставлено
в этом элементе.

Проверено новой probe-командой `shape` (шейпит строку ТЕМ ЖЕ шейпером, что и
рендер — пиксельный поиск по кадру трижды ловил соседнюю строку):
«NOTIFICATIONS» 83.2 → 91.78 при spacing 0.66 (13 символов × 0.66 = 8.58),
«TERMINAL» 54.99 → 60.27 (8 × 0.66 = 5.28). Прирост совпадает с моделью
глиф-в-глиф на обеих строках.

Осталось: сверить АБСОЛЮТНУЮ ширину с оригиналом тем же методом. Эталон 89.6
из прошлых циклов снят с КАДРА (ink extents), а не шейпером, и сравнивать его
с 91.78 нельзя — методы разные. Нужен прогон настоящего KaminIDE с той же
командой `shape`

## Цикл 34: MATCH

Упор «сверить АБСОЛЮТНУЮ ширину с оригиналом» снят: оригинальный KaminIDE
поднят с CDP (`KAMIN_EXE=…/debug/kaminide.exe CDP_PORT=9223 node
scripts/launch-cdp.mjs`; release-сборка отладочный порт НЕ открывает), и
метрики сняты прямо из его DOM новым скриптом `scripts/orig_measure.mjs`
(`Runtime.evaluate` по WebSocket, без зависимости `ws` — глобальный WebSocket
Node 22).

Сверка «строка → ширина» оригинал / наш `probe shape` (тот же шейпер, что и
рендер):

| строка | 11px | вес | трекинг | оригинал | наш | Δ |
|---|---|---|---|---|---|---|
| PROJECTS | 11 | 500 | 0.88px (.08em) | 59.73 | 59.46 | 0.27 |
| CONSOLE | 11 | 400 | 0.44px (.04em) | 53.14 | 53.17 | 0.03 |
| CLAUDE | 11 | 400 | 0.44px | 44.66 | 44.64 | 0.02 |
| PLAN | 11 | 400 | 0.44px | 29.77 | 29.47 | 0.30 |
| Terminal | 11 | 500 | 0.22px (.02em) | 46.50 | 46.38 | 0.12 |

Расхождение ≤0.3px на всех пяти строках и на трёх разных значениях трекинга —
это округление раскладки браузера, а не разница шейпинга. Прежний «эталон
89.6» был снят с КАДРА (ink extents) и к сравнению не годился вовсе.

Значение трекинга у этого элемента взято из CSS оригинала напрямую (список
всех 24 правил `letter-spacing` вытащен grep-ом по `src/renderer/**/*.css`).
