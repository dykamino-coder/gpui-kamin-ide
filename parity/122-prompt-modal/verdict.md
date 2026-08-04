# 122 — verdict (review cycle 1)
VERDICT: DIVERGES
input 8/4 vs 8/12, bg surface60% vs bg-base, border overlay vs bg-surface; нет
focus/invalid-бордера, .error, disabled OK; лишний .body-параграф; кнопка не OK.
360/520 + Enter — подтверждены.

## Цикл 2: DIVERGES
Инпут стили (8/12, bg-base, border bg-surface, focus/invalid); нет .error/disabled OK/«OK»; лишний body.

## Цикл 5: DIVERGES

Prompt-модалка: 360/520 и Enter-сабмит — исправлены. Остаток: инпут px8/py4 + `bg_surface .6` + border `bg_overlay` вместо padding 8/12 + bg-base + border bg-surface, нет focus-бордера accent и `.invalid`; нет строки ошибки и disabled OK; confirm-label «Save»/«Rename» вместо «OK»; лишний body-параграф. Кадр не снялся (файл совпал с другим) — по скрину не проверено.

## Цикл 6: DIVERGES

Инпут не по рецепту (px8/py4 + bg-surface .6 вместо 8/12 + bg-base + border bg-surface + focus accent + `.invalid`); нет строки ошибки и disabled OK; лейбл «Save»/«Rename» вместо «OK»; лишний body-параграф.

## Цикл 7: DIVERGES

Инпут 8/12 + bg-base + focus accent + invalid red против px8/py4 + bg-surface .6; нет строки ошибки и disabled OK; лейбл Save вместо OK; лишний body_el.

## Цикл 13: DIVERGES

Закрыто: скрим (см. 121) — один слой, тем-зависимый.

Осталось: паддинги инпута 8/12, фон bg-base, рамка bg-surface, focus- и
invalid-рамка, строка ошибки, disabled OK, лейбл «OK», лишний абзац между
заголовком и полем, select-all при фокусе.

## Цикл 13 (добивка): DIVERGES

Закрыто по ревью: инпут — padding 8/12, фон `bg-base`, рамка (фокус
accent-primary, `.invalid` accent-red), кегль fs-md через `Size::Size`;
появилась строка ошибки (mt 8, fs-xs, accent-red) и заблокированный OK
(opacity .5, клик не проходит); абзаца между заголовком и полем в prompt-режиме
больше нет; `actions` получил `margin-top: space-4`; подпись кнопки — «OK», как
у `showPrompt` оригинала; плейсхолдеры «name» / «Layout name».

Валидатор портирован дословно (`nameError`): «Name required», «Name cannot
contain path separators», «Invalid name».

Проверено на живом окне: «New File…» открывает модалку с красной рамкой,
строкой «Name required» и погашенным OK.

Осталось: select-all значения при фокусе; анимация появления.

## Цикл 15: DIVERGES

Осталось: ряд инпута ~50 лог. px против ~34 (собственный бокс `Input`), фокус-бордер вместо «валидный → accent», явная `width: 100 %`.

## Цикл 17: DIVERGES

Закрыто: центрирование модалки.
Осталось: рамка accent только на `:focus` (сейчас всегда при валидном значении), высота ряда инпута ~50 против ~34, `select()` при фокусе, явная `width: 100 %`.

## Цикл 20: DIVERGES

Закрыто: ряд ввода получил `--fs-md` 13 и нулевой собственный бокс `Input`.
Осталось: рамка accent только на `:focus`, `focus() + select()` при открытии.

## Цикл 22 (правка): DIVERGES

Закрыто: рамка инпута prompt-модалки красится accent ТОЛЬКО в фокусе
(`.input:focus`), иначе `--bg-surface`; состояние читается у самого
`InputState` через `focus_handle(cx).is_focused(window)`. Раньше accent
стоял всегда, когда значение валидно.

Осталось: `focus() + select()` при открытии модалки; высота ряда ввода
(собственный бокс `Input` уже погашен, но line-box отличается).

## Цикл 22 (правка 2): DIVERGES

Закрыто: при открытии prompt-модалки значение выделяется целиком
(`inputRef.current?.select()`, `PromptModal.tsx:42-45`) — тем же
`SelectAll`, что и rename-инпут сайдбара.

Осталось: высота ряда ввода (line-box `Input` отличается от CSS-паддингов).

## Цикл 23: MATCH

Prompt-модалка: рамка инпута accent в фокусе, значение выделено целиком (`window.focus` + `SelectAll`) — подтверждено кадром.
