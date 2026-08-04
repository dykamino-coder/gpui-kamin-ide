# 60 right-panel-bottom-card — наша реализация
Файлы: crates/shell/src/root.rs:4671-4703; crates/shell/src/ui/right_column.rs:249-266 (card_with_rail rail_bottom=true), 141-194 (bottom-rail порядок)

## Структура (gpui-дерево кратко)
```
div .flex_1 .min_h(0) .min_w(0)
└─ card_with_rail(rail_bottom=true)
   ├─ gap_wrap(card, pt=0, pb=4)
   │  └─ glint_surface_wv_holed(
   │       div#right-bottom .relative .size_full + probe_area("right-bottom")
   │       └─ tool_body(RightBottom) | panel_placeholder("Right Bottom", …, SlotIcon::RightBottom))
   └─ rail(RightBottom, bottom=true): .justify_end; порядок детей: «…»-пикер НАД плитками
```
Пикер «…» открывается вверх (up=true). Тело rightBottom-тула = как правило вебвью плана (webview_body_dyn).

## Метрики (из кода, точные)
- Высота: flex_1 (остаток после верха relative(right_split)) — эквивалент (1−split)·100%
- Карточка: glint radius 16 / inner 15, заливка bg_mantle; те же цвета, что 58
- Rail: 44px, justify_end, gap 2, py 12; dots 32×32 сверху, затем плитки 32×32 (зеркало align=bottom оригинала: пикер над плитками)
- gap_wrap: pt 0 (смежный с ручкой), pb 4

## Отличия от original.md той же папки
1. `rightPanelBottomVisible`-гейт НЕ реализован — нижняя карточка всегда рендерится.
2. Drop-индикация `data-activity-drop` НЕ реализована.
3. Label «Right Bottom» совпадает с оригиналом (в отличие от top-card, где у нас «Right Top» vs «Right»).
4. Зеркальный ActivityBar align="bottom" → наш rail(bottom=true): justify_end + пикер над плитками — DOM-порядок оригинала {picker, list} воспроизведён.
5. Высота: оригинал инлайн `(1-split)*100%`; у нас flex_1 — та же доля без округления toFixed(2).

## Дополнение атрибутов (цикл 10)

- цвета: карта — `glint_surface_wv_holed` (`root.rs:5554-5580`, `glint.rs:28-40`): заливка glint_mid #262533 dark / #e6e1d4 light (`palette.rs:87,125`), внутренний rect bg_mantle #262533 / #fbf7f4 (`palette.rs:55,93`), кромка glint_edge #ffffff α .18 / #3c2814 α .18 (`palette.rs:86,124`). Рейл снизу: плитка idle прозрачная, hover bg_surface α .5 = #3d3f51 / #e6e1d4 (`right_column.rs:52-56`), active accent_primary α .16 = #89b4fa / #da8343 (`right_column.rs:57-61`), иконка активной text_primary #cfd4e2 / #322e28, неактивной text_muted #838aa0 / #6e685d (`right_column.rs:62-66`).
