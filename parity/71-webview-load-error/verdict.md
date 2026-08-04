# 71 — verdict (review cycle 1)
VERDICT: DIVERGES
Не реализован: .errWrap + errIcon 22 accent-yellow + «This panel didn't load»
(13/600) + hint (12 muted max-w 280 lh1.4) + Retry (6/16 r8 border divider-soft
bg 6%/hover 12%). При исчерпании ретраев порт остаётся на Loading…

## Цикл 5: DIVERGES

Экран ошибки загрузки вебвью не реализован (grep `didn't load|Retry` = 0): нет errWrap, errIcon 22 accent-yellow op .85, title 13/600, hint 12 max-w 280 lh1.4, кнопки Retry (6/16, r8, divider-soft, bg 6%/hover 12%). Ретрай бесконечный раз в 5 с, терминального состояния нет. Кадра нет.

## Цикл 6: DIVERGES

Экран ошибки загрузки вебвью не реализован.

## Цикл 13: DIVERGES

РЕАЛИЗОВАНО (`ui/webview_skeleton.rs::load_error`). Раньше исчерпанный бюджет
resolve (45 попыток) не показывал НИЧЕГО — панель навсегда оставалась в
загрузочном состоянии, ручного Retry не было.

Портированы: обёртка absolute inset-0, p 24, gap 8, bg-surface; иконка
fa-triangle-exclamation 22 accent-yellow @ .85 с mb 4; заголовок fs-md/600;
подсказка fs-sm text-muted max-w 280; кнопка Retry px 16 / py 6, radius-sm,
рамка text-primary@14 %, фон 6 % → ховер 12 %, глиф fa-rotate + текст.
Retry сбрасывает счётчик попыток и метку времени — следующий кадр шлёт
`resolve_webview` заново.

Осталось: `transition .15s` у кнопки (в gpui переходов нет); `role="alert"`;
кадры обеих сторон (состояние достигается только при мёртвом extension host).

## Цикл 13 (добивка): DIVERGES

Закрыто: рамка Retry — `--divider-soft` (text-primary 6 %), а не 14 %; 14 % это
CSS-fallback `.retry`, который при определённой переменной не срабатывает.

Осталось: `line-height: 1.4` у подсказки; `transition` кнопки; `role="alert"`.

## Цикл 14: MATCH

Закрыто: `line-height: 1.4` у подсказки.

Ревью сверило элемент посвойственно и подтвердило всё остальное, включая
рамку кнопки Retry: `--divider-soft` определён как text-primary 6 %, поэтому
CSS-фолбэк 14 % недостижим — наши 6 % верны.

## Цикл 16: MATCH

Экран ошибки загрузки: gap 8, p 24, fa-triangle 22 accent-yellow .85, retry 6/16 с рамкой `text-primary 6 %`. Достижим только на динамических вью.

## Цикл 19: MATCH

Экран ошибки загрузки вебвью: gap 8, p 24, fa-triangle 22 accent-yellow .85, retry 6/16.
