# 137 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: MATCH

Инпут 360×32.8 лог. = max-width 360 / padding 8 12 / border 1. Минор: у оригинала настоящий <input> с :focus border accent-primary, у нас статичный div.

## Цикл 15: DIVERGES

Осталось: живой `<input>` с `:focus` accent-бордером; цвет плейсхолдера — UA, у нас жёсткий text-muted.

## Цикл 18: DIVERGES

Осталось: живой `<input>` с `:focus` accent-рамкой (у нас статичный div) и UA-цвет плейсхолдера вместо зашитого text-muted. Геометрия замерена и совпала: 360.00 × 33.6, p 8/12, r-sm, bg-base.

## Цикл 23: DIVERGES

Семпл-инпут по-прежнему статичный `div`: ни `InputState`, ни каретки, ни `:focus`-рамки; цвет строки зашит вместо UA-плейсхолдера. Геометрия совпадает.

## Цикл 24: MATCH

Семпл стал ЖИВЫМ `<input>`, как в оригинале (`component-samples.tsx:88-94`):
`InputState` рождается при показе страницы Design, `Input::new(...)
.appearance(false)` внутри нашей коробки, кегль через `Size::Size(FS_MD/0.875)`.
`.input:focus { border-color: accent-primary }` — рамка считается по фокусу
инпута кадром (снимок в `render`, как у фильтра логов).

Плейсхолдер больше не зашит в `text-muted`: до появления состояния рисуется
UA-цветом `currentColor 54 %` от `--text-primary` (Chromium `::placeholder` =
`color-mix(in srgb, currentColor 54%, transparent)`; правила `::placeholder`
в `design-sections.module.css` нет).

Проверено кадром: клик → синяя accent-рамка + каретка, ввод «hex #7aa2f7»
показан цветом `--text-primary`. Геометрия прежняя (360 × 33.6, p 8/12, r-sm,
bg-base).
