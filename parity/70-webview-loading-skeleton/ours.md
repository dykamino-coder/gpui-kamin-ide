# 70 webview-loading-skeleton — наша реализация

Файлы: `crates/shell/src/ui/webview_skeleton.rs` (`skeleton()`, `sk()`);
подключение — `root.rs::webview_body_dyn` (ветка `load`); состояние ожидания
считает `root.rs::tool_body` (`view_resolve_start`, `view_resolve_tries`).

> Цикл 13: РЕАЛИЗОВАНО. До этого скелета не было вовсе — грузящаяся панель
> показывала плейсхолдер ПУСТОГО слота («Open new tool or drag-n-drop tool
> from other panels»), то есть врала о своём состоянии.

## Структура (gpui-дерево)
```
div .absolute .inset_0 .overflow_hidden          // .wrap
  flex col, gap 14, px 18, py 16, bg bg-surface
  ├ div .bar   flex, items_center, gap 10, flex_shrink_0
  │   ├ sk(84×22, rounded 8)                     // .sk.pill
  │   └ sk(flex_1, h 22)                         // .sk.search
  ├ div .rows  flex col, gap 14, min_h 0
  │   └ ×6 div .row  flex, items_center, gap 12
  │        ├ sk(30×30, rounded 8)                // .sk.icon
  │        └ div .lines  flex col, gap 7, flex_1, min_w 0
  │             ├ sk(w = доля,        h 11)      // .sk.line
  │             └ sk(w = доля × .62,  h 9, opacity .6)  // .sk.lineDim
  └ [secs ≥ 3] div .waitNote  mt SPACE_3, text_center, FS_XS, text-disabled
```
`sk()` — бокс `relative + overflow_hidden`, фон text-primary@8 %, внутри
абсолютный бегунок ВО ВСЮ ширину (`::after { inset: 0 }`) с анимацией
`left: −100 % → +100 %`. Пилюля, поиск и иконка — такие же `sk()`, только со
своими боксами и скруглением 8.

## Метрики (из кода, точные)
- Ширины строк **0.90 / 0.70 / 0.80 / 0.60 / 0.75 / 0.50** (`ROW_W`);
  dim-линия — та же доля × **0.62**, `opacity .6`.
- Шиммер **1250 мс**, `repeat()`, easing `ease_in_out`.
- Подпись: порог **3 с** (`EXPLAIN_AFTER_S`), текст
  `Waiting for the extension host to open this panel · {N}s`; при попытке > 1
  добавляется ` · attempt {N}`. Счётчик — `Instant::elapsed()` от первого
  кадра ожидания.
- Фон обёртки `bg_surface` **#3d3f51**; подпись FS_XS **11**, `text_disabled`.
- Скругления: примитивы **6**, пилюля/поиск/иконка **8**.

## Отличия от original.md той же папки
1. **Три стопа градиента собраны из двух двухстоповых половин** — у
   `linear_gradient` в gpui ровно два стопа. Визуально сходится, но перегиб
   в центре бегунка кусочно-линейный, а не сглаженный.
2. **Бегунок двигается через `left`, а не `transform: translateX`** —
   трансформаций в gpui нет; позиция пересчитывается лейаутом каждый кадр,
   в браузере это чистый композитинг.
3. `role="status"` / `aria-label` / `.srOnly` отсутствуют — доступности в
   gpui-порте нет.
4. Секундомер тикает кадрами анимации, а не таймером 1000 мс: значение то же,
   обновление привязано к перерисовке.

## Атрибуты
- отступы: px 18 / py 16; gap 14 между баром и строками и между строками;
  внутри строки 12; между линиями 7; в баре 10; подпись mt SPACE_3 12
- цвета: подложка примитива text-primary@8 %, блик text-primary@9 %,
  фон обёртки bg-surface #3d3f51, подпись text-disabled #60667b
- шрифты: подпись — кегль fs-xs 11 (числа без tabular-nums — фичи шрифта не заданы)
- скругления: примитивы 6; пилюля, поиск и иконка 8
- гэпы: см. отступы
- ховер: N/A — ховер-состояний у скелета нет ни у нас, ни в оригинале
