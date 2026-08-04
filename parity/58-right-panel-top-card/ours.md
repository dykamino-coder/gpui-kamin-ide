# 58 right-panel-top-card — наша реализация
Файлы: crates/shell/src/root.rs:4612-4650; crates/shell/src/ui/right_column.rs:249-266 (card_with_rail), 34-122 (rail_tile), 125-246 (rail); crates/shell/src/ui/glint.rs:122-233

## Структура (gpui-дерево кратко)
```
div h=relative(right_split) .min_h(100) .w_full
└─ card_with_rail(rail_bottom=false): div .flex .size_full .min_w(0)
   ├─ gap_wrap(card, pt=4, pb=0)  (px 4)
   │  └─ glint_surface_wv_holed(
   │       div#right-top .relative .size_full + probe_area("right-top")
   │       └─ tool_body(RightTop) | panel_placeholder("Right Top", …, SlotIcon::RightTop))
   └─ rail(RightTop): div .w(44) .h_full .flex_col .items_center .gap(2) .py(12)
      ├─ rail_tile ×N (pinned): 32×32, rounded 8, иконка 18px (phosphor svg) / 16px (codicon)
      └─ dots «…» 32×32 (codicon ea7c 15px, tooltip "Add or remove items")
```
Карта БЕЗ таб-стрипа (тулы — в рейле), тело = чистое тело активного тула.

## Метрики (из кода, точные)
- Карточка: glint radius 16 / inner 15, кромка edge α.18, заливка bg_mantle (#262533 dark / #fbf7f4 light)
- Высота: relative(right_split), min_h 100
- Rail: ширина 44 (ACTIVITY_BAR_WIDTH), py 12 (SPACE_3), gap 2; плитка 32×32 rounded 8; active bg = accent_primary α.16; hover bg = bg_surface α.5 (#3d3f51/50%); иконка active text_primary (#cfd4e2), иначе text_muted (#838aa0)
- Placeholder: label «Right Top» fs 16 semibold, hint fs 12, глиф SlotIcon::RightTop (scale 2.8)

## Отличия от original.md той же папки
1. Label плейсхолдера «Right Top» — оригинал «Right» (aria-label карточки "Right").
2. Drop-индикация `data-activity-drop="over"/"blocked"` НЕ реализована (нет accent-tint 10% + dashed outline / red 12% + inset shadow).
3. Rail = наша реализация ActivityBar слота: ширина 44 (token), у оригинального `.splitHandle` fallback var = 48px — если фактический CSS-var 44, совпадает; сами метрики плиток (32×32/gap 2/py 12) — наши, сверка с элементом 38 отдельно.
4. `.cardWithBar > aside.card { flex: 1 }` → у нас gap_wrap с size_full + min_w(0), rail flex_shrink_0 — эквивалент.
5. Классы .cardHeader/.empty оригинального модуля не портированы (тела карточек используют свои компоненты).
6. bottomShown=false → height 100% — не поддержано (низ всегда виден, см. 56).

## Дополнение атрибутов (цикл 10)

- шрифты: собственного текста у карты нет (`root.rs:5491-5518` — glint-обёртка + тело тула), `.cardHeader` оригинала (fs-xs 11 / weight 500 / ls .08em) в `RightPanel.tsx:140-149` не используется и у нас не портирован. Кегли приходят от содержимого: плейсхолдер «Right» — заголовок fs-lg 16 + weight 600 и подсказка fs-sm 12 при line-height 1.3 (`panel_placeholder.rs:123-135`), пилюля «Open Tool ▾» fs-sm 12 + глиф FontAwesome 10 (`slot_panel.rs:192,209`), глифы рейла — codicon/phosphor 18 (`right_column.rs:69-78`).
