# 56 right-panel-column — наша реализация
Файлы: crates/shell/src/root.rs:4605-4705 (right_column_el), 4759-4764 (right_wrap), 3976 (right_w); crates/shell/src/ui/right_column.rs:249-266 (card_with_rail), 125-246 (rail)

## Структура (gpui-дерево кратко)
```
right_wrap: div .w(right_w + ACTIVITY_BAR_WIDTH=44) .flex_shrink_0 .h_full
└─ right_column_el: div .flex .flex_col .size_full .min_w(0)
   ├─ div h=relative(right_split) .min_h(100) → card_with_rail(top)   — элемент 58
   ├─ h_handle("right-split-handle", pr=44)                            — элемент 59
   └─ div .flex_1 .min_h(0) → card_with_rail(bottom, rail_bottom=true) — элемент 60
```
Показ гейтится `layout.right_panel_visible` (when(rv) в body). Ручка ширины (между file и right) — сиблинг в body: `file_right_handle` (элемент 57).

## Метрики (из кода, точные)
- Ширина: right_w = layout.right_panel_width_px.round() (дефолт RIGHT_PANEL_DEFAULT=280) + 44 rail; min при драге PANEL_MIN_SIZE=100
- right_split: поле RootView, дефолт RIGHT_SPLIT_DEFAULT=0.55, кламп [0.15, 0.85]
- Колонка без фона (просвечивает bg_sidebar+radial), rail width 44 (ACTIVITY_BAR_WIDTH)
- flex_shrink_0 на right_wrap; min_w(0) внутри

## Отличия от original.md той же папки
1. fill-режим (`flex: 1 1 0` при скрытом центре) НЕ реализован — всегда фикс-ширина.
2. `flex-shrink: 1` + min-width 100 оригинала → у нас flex_shrink_0: при тесноте колонка не сжимается (сжимается main).
3. bottomShown-гейт отсутствует: нижняя карточка и split-handle рендерятся всегда (rightPanelBottomVisible нет в нашей layout-модели).
4. right_split НЕ персистится (end_drag сохраняет только sidebar/file/right width + mainSplit + fileBottom) — оригинал сохраняет rightPanelSplit.
5. Ширина колонки включает rail (+44): оригинальная rightPanelWidth задаёт всю колонку вместе с ActivityBar; у нас right_w — контентная часть, rail добавляется сверху. При равных сохранённых числах наша колонка на 44px шире.
6. width-handle не absolute внутри колонки, а нулевой сиблинг в body (см. 57).

## Дополнение атрибутов (цикл 10)

- цвета: колонка фона не имеет (`root.rs:5470-5590`) — просвечивает корневая заливка bg_sidebar #1d1d28 dark / #f4f1ea light (`root.rs:6060`, `palette.rs:56,94`) плюс два запечённых radial-спрайта: accent_purple #cba6f7 / #8a5fc8 при peak α 0.08 и accent_primary #89b4fa / #da8343 при peak α 0.06 (`radial_bg.rs:96-97`, `palette.rs:76,83,114,121`).
- отступы: у `right_wrap`/`right_column_el` padding нет (`root.rs:5644-5650`, `:5470`); горизонтальный полузазор даёт `gap_wrap` карты — `pl 4`, справа 0, карта вплотную к рейлу (`right_column.rs:21-31`); рейл — `py 12` (SPACE_3, `right_column.rs:145`).
- гэпы: flex-`gap` у колонки нет; 8px до соседа собираются из `pl 4` двух смежных `gap_wrap`; вертикальный зазор между картами = хит-зона `h_handle` 10px (`splitter.rs:121`, вызов `root.rs:5528-5532` с `pr = 48`); внутри рейла gap 8 между группами (`right_column.rs:144`) и gap 2 между плитками (`right_column.rs:155`).
- ховер: N/A: ховер — `right_column_el` (`root.rs:5470-5590`) и `card_with_rail` (`right_column.rs:210-227`) hover-стилей не задают; ховер есть только у плиток рейла (`right_column.rs:96` — `bg_surface` α .5) и у split-ручки (элемент 59).
