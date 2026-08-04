# 129 tooltip — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\tooltip.rs (весь: `KaminTooltip`, `tooltip_box_at`, `half_width`, `tooltip`); crates\shell\src\overlay.rs:719-722 (`tooltip_live` → `tooltip_box_at`), overlay.rs:191 (`tooltip_region`); crates\shell\src\root.rs:170, 667-673 (состояние `tooltip_live`), 2405-2407 (overlay-окно не прячется, пока тултип жив)

## Структура/содержание
```
элемент.tooltip(tooltip("текст")) → gpui показывает KaminTooltip по ховеру (задержка gpui)
MAIN-окно: KaminTooltip::render НЕ рисует бокс (WebView2-чайлды перекрыли бы) →
  шлёт ShellEvent::TooltipShow(text, mouse.x, mouse.y) с позицией мыши ЭТОГО окна
overlay-окно: tooltip_box_at(text, x, y):
  half = shape_line(text, 11px, UI_FONT «Bricolage Grotesque»).width.min(640) / 2 + 8
  left = (x − half).clamp(4, max(vw − 2·half − 4, 4))     ← кламп по X к вьюпорту
  top  = (y − 14 − box_h).max(4)                          ← НАД курсором
  box_h = 11 × 1.3 + 8 = 22.3
Drop KaminTooltip → TooltipHide (гасит overlay-копию)
```
Палитра — `kamin_theme::current_palette()` (следует активной теме). Отдельная ветка `KaminTooltip::render` внутри самого overlay-окна ставит бокс относительным `absolute` (left −half, top −(box_h + 14)) без клампа — боевой путь main → overlay идёт через `tooltip_box_at`.

## Метрики (из кода, точные)
- отступы: px 8 (SPACE_2), py 4
- гэпы: N/A: гэпы — у бокса ровно один ребёнок (текст), `gap` не задан
- цвета: bg p.bg_surface #3d3f51 (dark) / #e6e1d4 (light); текст p.text_primary #cfd4e2 / #322e28; тень 0 2 8 rgba(0,0,0,0.3) (= `--shadow-mini`)
- скругления: rounded 4 (RADIUS_XS)
- шрифты: font-size 11 (FS_XS), line-height 14.3 (11 × 1.3), font-weight 400 NORMAL, семейство `crate::root::UI_FONT` = «Bricolage Grotesque»
- ховер: N/A: ховер — тултип сам не hoverable; hit-регион ставится только на бокс (`tooltip_region`), состояний ховера нет
- прочее: max-width 640, `whitespace_nowrap` + `overflow_hidden`; смещение от курсора 14px вверх; горизонтальный кламп 4px от краёв

## Отличия от original.md той же папки
1. Якорь — КУРСОР, а не элемент: центр по X от позиции мыши, верх бокса = y − 14 − box_h. Оригинал считает от `getBoundingClientRect` элемента с `{ side: "top", offset: 8 }`.
2. Кламп есть по X (4 … vw − 2·half − 4) и по верхней границе (`.max(4)`); переворота под курсор при нехватке места сверху и клампа по правому/нижнему краю в стиле `clampToViewport` нет.
3. Fade-in 0.1s и двухпроходное «невидимое измерение» (opacity 0 → layout-измерение → opacity 1) отсутствуют: ширина меряется шейпером ДО рендера.
4. `text-overflow: ellipsis` нет — только `overflow_hidden` (обрезка без многоточия); `max-width: 640` есть, но без `min(640px, calc(100vw − 16px))`.
5. Тултипы из вебвью (сигнал `webviewTooltip`) не принимаются.
6. Рисуется в ОТДЕЛЬНОМ overlay-окне поверх WebView2, а не в DOM того же документа; показ — по gpui-ховеру с его задержкой, а не по `pointerenter` на `[data-tooltip]`.
7. Скрытие — только через `Drop` у `KaminTooltip`; явных подписок на `mousedown` / `visibilitychange` / `window blur` / `scroll (capture)` нет.
8. Палитра берётся из `current_palette()` — light-тема поддержана (прежний хардкод DARK устранён).
9. Метрики покоя (bg-surface, padding 4×8, radius-xs 4, fs-xs 11, lh-snug 1.3, shadow-mini, nowrap, max-width 640, pointer-events нет) — совпадают 1:1.
