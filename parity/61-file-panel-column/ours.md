# 61 file-panel-column — наша реализация
Файлы: crates/shell/src/root.rs:4113-4602 (file_column), 4734-4739 (file_wrap), 3970-3975 (file_w из ratio); crates/metrics/src/lib.rs:61,70-73

## Структура (gpui-дерево кратко)
```
file_wrap: div .w(file_w) .flex_shrink_0 .h_full
└─ file_column: div .flex .flex_col .size_full .min_w(0)
   ├─ div h=relative(1 − bottom_ratio) .min_h(100) .w_full
   │  └─ gap_wrap_v_top( glint(top_card) )          — элемент 63
   ├─ h_handle("file-bottom-handle", pr=0)          — элемент 64
   └─ div .flex_1 .min_h(0) .min_w(0)
      └─ gap_wrap_v(pb=4)( glint(slot_panel CentralBottom) ) — элемент 65
```
Показ гейтится `layout.file_panel_visible`; ширинная ручка (между main и file) — сиблинг `main_file_handle` в body (элемент 62).

## Метрики (из кода, точные)
- Ширина: file_w = width_from_ratio(file_panel_width_ratio, PANEL_MIN_SIZE=100, viewport_w).round(); дефолт FILE_PANEL_DEFAULT=360 (px до первой конвертации)
- bottom_ratio: кламп [BOTTOM_RATIO_MIN 0.1, BOTTOM_RATIO_MAX 0.8]; дефолт из FILE_BOTTOM_DEFAULT=180px→ratio
- Колонка без фона; flex_shrink_0 на wrap; min_h(100) у верхней секции

## Отличия от original.md той же папки
1. Ширина хранится как ratio от вьюпорта (filePanelWidthRatio) — оригинал хранит px (filePanelWidth); при resize окна наша колонка масштабируется, оригинальная остаётся фикс-px.
2. `flex-shrink: 1` (сжатие до min-width при тесноте) → у нас flex_shrink_0 — не сжимается.
3. fill-режим (`flex: 1 1 0` при скрытом main) НЕ реализован.
4. `filePanelBottomVisible`-гейт отсутствует: split-handle и нижняя карточка всегда рендерятся.
5. Раскладка высот инвертирована: оригинал — низ фикс-px (flexShrink 0), верх flex 1; у нас верх = relative(1−ratio), низ = flex_1. Итоговые доли совпадают, но семантика ресайза иная (см. 64).
6. aside/aria-label «File column» — нет DOM.

## Дополнение атрибутов (цикл 10)

- цвета: колонка без фона (`root.rs:5399-5413`) — просвечивает bg_sidebar #1d1d28 dark / #f4f1ea light (`root.rs:6060`, `palette.rs:56,94`) + radial-спрайты accent_purple α .08 / accent_primary α .06 (`radial_bg.rs:96-97`); карты внутри — glint: заливка bg_mantle #262533 / #fbf7f4 (`palette.rs:55,93`), mid #262533 / #e6e1d4 (`palette.rs:87,125`), кромка glint_edge #ffffff α .18 / #3c2814 α .18 (`palette.rs:86,124`, `glint.rs:28-40`).
- гэпы: flex-`gap` у колонки нет — `file_col` это `div().flex().flex_col()` без gap (`root.rs:5399`); межпанельные 8px собираются из `gap_wrap` каждой карты (`px(4)` слева и справа, `root.rs:3023-3031`) → 4+4; вертикальный «gap» между верхней и нижней картой = хит-зона `h_handle` 10px (`splitter.rs:121`, вызов `root.rs:5416-5420`).
- ховер: N/A: ховер — сама колонка (`root.rs:5399-5413`) hover-стилей не имеет, как и `.filePanel`; ховеры внутри принадлежат mode-табам (66), split-ручке (64) и строкам дерева (94/95).
