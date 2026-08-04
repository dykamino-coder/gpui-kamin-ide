# 54 main-bottom-panel — наша реализация
Файлы: crates/shell/src/root.rs:4046-4110 (ветка main_bottom_visible; сам ящик 4078-4106); crates/shell/src/ui/glint.rs:122-233; crates/shell/src/ui/slot_panel.rs:187-237

## Структура (gpui-дерево кратко)
```
main_column (flex-col, при layout.main_bottom_visible)
├─ [верх: chat_content, h=relative(main_split)]        — элемент 53
├─ h_handle("main-bottom-handle")                      — элемент 55
└─ div .flex_1 .min_h(0) .min_w(0)
   └─ gap_wrap_v(pt=false, pb=true)  (px 4, pb 4)
      └─ glint_surface_wv_holed(
           div#main-bottom .relative .size_full + probe_area("main-bottom")
           └─ slot_panel(MainBottom, state, "Left Bottom",
                SlotIcon::MainBottom, picker_up=true, drag_over, body))
```
`main_bottom_visible=false` → ветка else: только gap_wrap(chat_content).

## Метрики (из кода, точные)
- Высота: `flex_1` — остаток колонки после верха relative(main_split); эквивалент (1 − mainSplit)·100%
- Карточка: glint radius 16 / inner 15, кромка edge α.18 (dark #ffffff / light #3c2814), заливка bg_mantle (#262533 / #fbf7f4)
- gap_wrap_v: px 4, pb 4, pt 0 (смежный с ручкой паддинг убран — вертикальный зазор = 8px ручки)
- Пустое состояние: panel_placeholder «Left Bottom» + пилюля «Open Tool ▾» (accent α.16, hover α.26), пикер открывается ВВЕРХ (picker_up=true)
- Стрип табов как у 53 (h24, px12, rounded 8, fs 12)

## Отличия от original.md той же папки
1. Drop-индикация `data-activity-drop="over"/"blocked"` на карточке НЕ реализована.
2. Оригинал: ручка (элемент 55) — ребёнок section.panel; у нас ручка — сиблинг между верхом и ящиком в main_column. Итоговая геометрия та же.
3. Высота: оригинал инлайн-процент `(1-mainSplit)*100%` на section.panel; у нас flex_1 при фиксированном верхе — та же доля.
4. `mainVisible=false → height 100%` не поддержано: у нас при скрытом main скрыт весь main_wrap (ящик исчезает вместе с колонкой).
5. section/aria-label «Left Bottom» → нет DOM; label живёт в плейсхолдере.
