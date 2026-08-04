# 71 webview-load-error — наша реализация

Файлы: `crates/shell/src/ui/webview_skeleton.rs` (`load_error()`); подключение —
`root.rs::webview_body_dyn` (ветка `load` при `exhausted`); бюджет ретраев —
`root.rs::tool_body` (backoff 350 мс × 1.5 до 3 с, 45 попыток); сброс бюджета —
`ShellEvent::RetryView` в `root.rs`.

> Цикл 13: РЕАЛИЗОВАНО. До этого исчерпанный бюджет resolve не показывал
> ничего — панель оставалась в загрузочном состоянии навсегда, ручного Retry
> не было.

## Структура (gpui-дерево)
```
div .absolute .inset_0                              // .errWrap
  flex col, items_center, justify_center, gap SPACE_2, p 24, text_center,
  bg bg-surface
  ├ fa-triangle-exclamation 22, accent-yellow @ .85, mb SPACE_1   // .errIcon
  ├ div  FS_MD, weight SEMIBOLD, text-primary  "This panel didn't load"
  ├ div  FS_SM, text-muted, max_w 280
  │      "The extension host may still be starting up."
  └ div#wv-retry  flex, items_center, gap 6, px 16, py 6,          // .retry
        rounded RADIUS_SM, border 1px text-primary@0.14,
        bg text-primary@0.06, FS_SM, text-primary, cursor_pointer,
        hover bg text-primary@0.12
        ├ fa-rotate (кегль наследуется)
        └ "Retry"
```

## Метрики (из кода, точные)
- Обёртка: `p` **24**, `gap` SPACE_2 **8**, фон `bg_surface` **#3d3f51**.
- Иконка: FontAwesome solid **22**, `accent_yellow` при alpha **0.85**,
  `mb` SPACE_1 **4**; бокс = advance глифа (инлайновый `<i>` оригинала).
- Заголовок: FS_MD **13**, `SEMIBOLD` (600), `text_primary`.
- Подсказка: FS_SM **12**, `text_muted`, `max-w` **280**.
- Кнопка: px **16** / py **6**, `gap` **6**, rounded RADIUS_SM **8**,
  рамка 1px `text_primary`@**0.14** (= `--divider-soft`),
  фон `text_primary`@**0.06**, ховер **0.12**.
- Триггер: `tries >= 45` при мёртвом вью. Retry сбрасывает счётчик попыток,
  метку времени и секундомер — следующий кадр шлёт `resolve_webview` заново.

## Отличия от original.md той же папки
1. `transition: background .15s ease` у кнопки — переходов в gpui нет,
   подложка меняется мгновенно.
2. `role="alert"` отсутствует — доступности в gpui-порте нет.
3. Line-height подсказки (1.4) движком не задаётся отдельно: используется
   общий межстрочный интервал текста.

## Атрибуты
- отступы: обёртка p 24; кнопка px 16 / py 6; иконка mb 4
- цвета: фон bg-surface #3d3f51, иконка accent-yellow #f9e2af @85 %,
  заголовок text-primary #cfd4e2, подсказка text-muted #838aa0,
  кнопка — фон text-primary@6 %, рамка text-primary@14 %, текст text-primary
- шрифты: заголовок FS_MD 13 / 600; подсказка FS_SM 12; кнопка FS_SM 12;
  иконка предупреждения 22
- скругления: кнопка RADIUS_SM 8
- гапы: обёртка 8; внутри кнопки 6
- ховер: только у кнопки — подложка text-primary 6 % → 12 %
