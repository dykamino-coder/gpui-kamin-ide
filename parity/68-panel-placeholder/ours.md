# 68 panel-placeholder — наша реализация
Файлы: crates/shell/src/ui/panel_placeholder.rs:84-124 (panel_placeholder_ex / panel_placeholder), 34-80 (glyph — нативная PanelIcon); crates/shell/src/ui/slot_panel.rs:150-183 (open_tool_btn «Open Tool ▾»)

## Структура (gpui-дерево кратко)
```
panel_placeholder_ex(label, hint, slot, extra):
div .size_full .flex_col .items_center .justify_center
    .gap(8) .p(20) .overflow_hidden .text_color(text_muted)
├─ div .mb(4) → slot_glyph(slot)  (glyph scale 2.8: рамка 14×12 → 39.2×33.6)
├─ label: fs 16 (FS_LG) semibold text_primary
├─ hint:  fs 12 (FS_SM) text_muted, max_w 240, text_center
└─ when_some(extra): open_tool_btn — пилюля «Open Tool ▾»
```
Глиф — нативные div вместо SVG: рамка border 1px text_muted, rounded 1.5·s, внутри бар подсвеченного слота (text_muted α.85, rounded 1·s); 7 вариантов SlotIcon (Main/MainBottom/Center/CenterBottom/Right/RightTop/RightBottom), геометрия PanelIcon.tsx (SLOT_INSET 1.5, ширины 4.5, RIGHT_X 8 и т.д.).

## Метрики (из кода, точные)
- Контейнер: gap 8 (SPACE_2), padding 20 (SPACE_5), цвет text_muted (#838aa0 / #6e685d)
- Глиф: масштаб 2.8 → 39.2×33.6px; mb 4 (SPACE_1)
- label: 16px semibold text_primary (#cfd4e2 / #322e28)
- hint: 12px text_muted, max-width 240, по центру
- Пилюля (slot_panel::open_tool_btn): px 12 (SPACE_3), py 4 (SPACE_1), mt 4, rounded 8 (RADIUS_SM), gap 8, fs 12, текст text_primary; bg tint(accent_primary, 0.16), hover 0.26; «Open Tool» + fa chevron-down 10px

## Отличия от original.md той же папки
1. Размер глифа: у нас 39.2×33.6 (scale 2.8) против 28×24 оригинала (scale 2 от 14×12) — наш заметно крупнее.
2. hint: max-width 240 добавлен (у оригинального PanelPlaceholder.hint ограничения ширины нет — только line-height lh-snug; 240 — это метрика ActivityPlaceholder).
3. Пилюля: метрики 1:1 (py 4 ≈ padding space-1, px 12 = space-3, bg 16%/hover 26%, radius sm, mt space-1, иконка 10px); transition var(--transition-fast) не воспроизведён (мгновенный hover).
4. Глиф нативными div (currentColor→text_muted α.85 у слота) — оригинал SVG PanelIcon с opacity 0.85; визуально эквивалентно.
5. Пилюля рендерится только там, где caller передал extra (слоты со стрипом: Left/Left Bottom/Central Bottom); правые карты и центр «File» — без неё, что соответствует «без activitySlot пикер не рендерится».
6. h2/p/aria — нет DOM.

## Дополнение атрибутов (цикл 10)

- шрифты: заголовок fs-lg 16 + weight 600 SEMIBOLD (`panel_placeholder.rs:123-124`, `metrics/lib.rs:45`), подсказка fs-sm 12 с line-height 1.3 = 15.6px (`panel_placeholder.rs:131-133`), пилюля «Open Tool ▾» fs-sm 12 + шеврон FontAwesome 10 (`slot_panel.rs:192,209`). Оригинал: `.label { font-size: var(--fs-lg); font-weight: 600 }`, `.hint { var(--fs-sm); line-height: var(--lh-snug) 1.3 }`, `.trigger { var(--fs-sm) }`, `.trigger > i { 10px }` (`PanelPlaceholder.module.css:30-64`) — 1:1.
