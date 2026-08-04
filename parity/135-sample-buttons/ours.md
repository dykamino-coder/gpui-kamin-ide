# 135 sample-buttons — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_samples.rs:71-78 (`DsBtn`), 84-152 (`ds_btn`), 155-165 (`sample_buttons`); crates\shell\src\ui\design_panel.rs:804 (`block("Buttons", …)`), 78-123 (обёртка `block`)

## Структура/содержание
```
block «BUTTONS»
└─ div.flex.flex_wrap.gap(8)
   ├─ div#ds-btn-primary   «Primary»    bg accent_action, fg accent_action_fg, weight 600
   ├─ div#ds-btn-secondary «Secondary»  прозрачный, border bg_overlay, fg text_primary
   ├─ div#ds-btn-danger    «Danger»     bg accent_red, fg bg_primary, weight 600
   └─ div#ds-btn-ghost     «Ghost»      прозрачный, border rgba(0,0,0,0), fg text_secondary
```
Все 4 варианта реализованы (`DsBtn::{Primary, Secondary, Danger, Ghost}`), каждый — stateful div с `cursor_pointer()` и `.hover(...)`. У Ghost бордер прозрачный, а не отсутствующий — иначе кнопка была бы на 2px уже соседей.

## Метрики (из кода, точные)
- отступы: кнопка px 16 (SPACE_4) / py 4 (SPACE_1) — у всех 4 вариантов одинаково
- гэпы: ряд кнопок gap 8 (SPACE_2); блок `block()` — колонка gap 8 (SPACE_2), тело-ряд flex-wrap gap 8
- цвета: Primary — bg p.accent_action #89b4fa, текст p.accent_action_fg #313240; Secondary — фон прозрачный, текст p.text_primary #cfd4e2, border 1px p.bg_overlay #515567; Danger — bg p.accent_red #f38ba8, текст p.bg_primary #313240; Ghost — фон прозрачный, текст p.text_secondary #adb3c7, border 1px rgba(0,0,0,0); подпись блока p.text_muted #838aa0
- скругления: все 4 кнопки rounded 8 (RADIUS_SM)
- шрифты: все кнопки font-size 12 (FS_SM); Primary и Danger — weight 600 SEMIBOLD, Secondary и Ghost — weight 400; подпись блока 11 (FS_XS) weight 700 BOLD uppercase
- фоны по ховеру: Primary — p.accent_action_hover #74c7ec; Secondary — p.bg_surface #3d3f51; Danger — p.accent_maroon #eba0ac; Ghost — p.bg_surface #3d3f51 + текст поднимается до p.text_primary #cfd4e2

## Отличия от original.md той же папки
1. Все 4 варианта, их цвета, паддинги 4×16, radius-sm 8, fs-sm 12, weight 600 у Primary/Danger и все 4 hover-состояния — совпадают с оригиналом 1:1.
2. `transition: background var(--transition-fast)` (150ms ease) отсутствует — в gpui переходов нет, смена фона мгновенная.
3. Кнопки — stateful `div`, а не `<button type=button>`: `cursor: pointer` есть, но нет клавиатурной активации, фокус-кольца и роли button.
4. У Ghost бордер задан явно прозрачным (rgba 0,0,0,0) — то же, что `1px solid transparent` в оригинале.
5. `font: inherit` оригинала → у нас семейство наследуется от окна («Bricolage Grotesque»), размер задан явно 12.
