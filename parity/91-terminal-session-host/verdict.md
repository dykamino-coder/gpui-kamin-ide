# 91 — verdict (review cycle 1)
VERDICT: DIVERGES (мелочи)
Инсеты/mono13/цвета/Ctrl+C/Ctrl+V/fit/drop подтверждены.
Расхождения: нет cursorBlink; scrollback 10000 vs 5000; смонтирована только
активная сессия (эффект тот же); нет «[process exited with code N]».

## Цикл 5: DIVERGES

Хост сессии терминала: инсеты, mono 13, editor-bg/-fg/-cursor, Ctrl+C с выделением = copy, Ctrl+V, probe-fit, drop путей — подтверждены. Остаток: нет `cursorBlink` (курсор статичный), scrollback не 5000, выделение accent-primary 30% вместо белого 30% (дефолт xterm), нет «[process exited with code N]».

## Цикл 6: DIVERGES

Нет `cursorBlink`, scrollback 5000, цвета выделения xterm, «[process exited…]».

## Цикл 11: DIVERGES

Закрыто: scrollback 5000 (`TerminalSession.tsx:79`) — alacritty по умолчанию держал
10000.

Осталось: `cursorBlink` (курсор статичный); цвет выделения задаётся палитрой, а
оригинал оставляет дефолт xterm; «[process exited with code N]» не пишется.

## Цикл 16: MATCH

Хост терминальной сессии: инсеты 8/22/10/14, моно 13, история 5000, тема из `editor-*`, Ctrl+C/Ctrl+V, буфер неактивных сохраняется.

## Цикл 19: DIVERGES

НОВОЕ: `cursorBlink: true` у оригинала — у нас курсор статичный.

## Цикл 19 (доработка): DIVERGES

Закрыто: курсор терминала мигает периодом 1.2 с (`cursorBlink: true`), был статичным.
Ждёт подтверждения кадром.

## Цикл 23: MATCH

Курсор терминала: `Animation::new(1200ms).repeat()`, `delta < 0.5 → видим`; инсеты 8/22/10/14, кегль 13.
