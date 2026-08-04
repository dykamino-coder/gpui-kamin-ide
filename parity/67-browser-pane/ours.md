# 67 browser-pane — наша реализация
Файлы: crates/shell/src/ui/browser_pane.rs:66-145 (wry-вариант), 151-218 (visual_frame, windows), 26-38 (normalize_url), 40-63 (nav_btn); root.rs:4118-4308 (web-ветка, форвардинг ввода)

## Структура (gpui-дерево кратко)
```
div#browser-pane .flex_1 .min_h(0) .flex_col
├─ навбар: div .flex .items_center .gap(4) .flex_shrink_0 .h(32) .px(8)
│    .border_b_1 .border_color(tint(text_primary, 0.06))
│  ├─ nav_btn "br-back"   codicon ea9b (Back)
│  ├─ nav_btn "br-fwd"    codicon ea9c (Forward)
│  ├─ nav_btn "br-reload" codicon eb37 (Reload)
│  └─ адрес: div .flex_1 .min_w(0) .ml(4) .px(8) .h(24) .rounded(8)
│       .bg(tint(bg_surface,0.5)) .border_1(tint(bg_overlay,0.4))
│       .on_key_down(Enter → normalize_url → load_url/navigate)
│     └─ Input(address).appearance(false)
└─ вьюпорт: div#browser-viewport .relative .flex_1 .min_h(0) .px(8) .pb(8)
   └─ webview (wry) | composition-визуал (дыра в кадре, canvas sync_zone)
```
normalize_url: схема как есть; «домен.tld» → https://; иначе Google-поиск. Visual-режим: мышь/скролл форвардятся SendMouseInput, курсор страницы мапится на gpui CursorStyle, back/forward/reload через wv_visual.

## Метрики (из кода, точные)
- Навбар: h 32, px 8, gap 4 (SPACE_1), border-bottom 1px tint(text_primary,0.06)
- nav_btn: 26×26, rounded 8 (RADIUS_SM), цвет text_secondary; hover bg tint(text_primary,0.1) + text_primary
- Адрес: h 24, px 8, rounded 8, bg bg_surface α.5, border 1px bg_overlay α.4
- Вьюпорт: px 8, pb 8 («воздух» вокруг вебвью); скругление зоны — угловые маски R=12 (RADIUS_MD) в glint-канвасе

## Отличия от original.md той же папки
1. Навбар: оригинал `padding: 4px 6px`, БЕЗ border-bottom и без фикс-высоты; у нас h 32, px 8 + разделительная линия снизу.
2. Адресная строка: оригинал h 26, px 10, bg --bg-base, border --divider-soft, focus → border accent-primary; у нас h 24, px 8, bg bg_surface/50%, border bg_overlay/40%, focus-подсветки НЕТ.
3. nav_btn hover: оригинал bg --bg-surface-hover (#3b3b52); у нас tint(text_primary, 0.1).
4. Вьюпорт-инсет: оригинал margin 0 6px 6px + border-radius 12; у нас px 8 / pb 8 (8 против 6), радиус 12 совпадает (маски).
5. Скрытие webview при перекрытии поповерами (MutationObserver + POPUP_SELECTOR) не нужно: наши поповеры живут в overlay-окне НАД вебвью (feedback_all_popovers_overlay).
6. form/onSubmit → on_key_down(Enter); placeholder «Search or enter address» задаётся InputState вне этого файла (не проверено здесь).
7. Forward: оригинал browser.navigate-API; у нас history.forward()/evaluate_script (wry) или wv_visual::forward().

## Дополнение атрибутов (цикл 10)

- шрифты: адресная строка fs-sm 12 (`browser_pane.rs:128`, дублируется в visual-варианте `:208`) = `.addr { font-size: var(--fs-sm) }` (`BrowserPane.module.css:41`); глифы nav-кнопок codicon 16 (`browser_pane.rs:63`) — у оригинала `.navBtn` кегля не задаёт, наследует 16px `.codicon` (`skeleton.css:2-4`), т.е. совпадает; собственных font-weight/семейства панель не ставит.
