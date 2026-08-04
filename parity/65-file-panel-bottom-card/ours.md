# 65 file-panel-bottom-card — наша реализация
Файлы: crates/shell/src/root.rs:4570-4600; crates/shell/src/ui/slot_panel.rs:27-115 (tab), 187-237 (slot_panel); crates/shell/src/ui/glint.rs:122-233

## Структура (gpui-дерево кратко)
```
div .flex_1 .min_h(0) .min_w(0)
└─ gap_wrap_v(pt=0, pb=4)
   └─ glint_surface_wv_holed(
        div#central-bottom .relative .size_full + probe_area("central-bottom")
        └─ slot_panel(CentralBottom, state, "Central Bottom",
             SlotIcon::CenterBottom, picker_up=true, drag_over, body))
```
slot_panel: стрип (pinned>0) + тело активного тула (напр. консоль/терминал) либо panel_placeholder «Central Bottom» с «Open Tool ▾» (пикер вверх).

## Метрики (из кода, точные)
- Высота: flex_1 = доля bottom_ratio колонки (верх занял 1−ratio); кламп ratio [0.1, 0.8]
- Карточка: glint radius 16 / inner 15, заливка bg_mantle
- Стрип: px 8 (SPACE_2), pt 4 (SPACE_1), gap 2; таб: h 24, px 12 (SPACE_3), gap 4 (SPACE_1), rounded 8 (RADIUS_SM), fs 12, иконка 13px (phosphor/codicon); idle text_muted, hover bg text_primary α.08 + text_primary; active bg accent_primary α.16 + text_primary; drag-over: border_l 2 accent_primary
- «…» dots 24×24 rounded 8, codicon ea7c 15px, справа (flex_1-спейсер перед ним)

## Отличия от original.md той же папки
1. Высота: оригинал — ФИКС-px (`height: filePanelBottomHeight px, flexShrink 0`, мин 100); у нас доля колонки (см. 64) — при resize окна поведение расходится.
2. Drop-индикация `data-activity-drop="over"/"blocked"` НЕ реализована; вместо неё — только индикатор вставки в стрипе.
3. BottomTabBar оригинала (элемент 48/49: TAB_ICON_SIZE 13, свои паддинги) заменён нашим стрипом slot_panel — иконка 13px совпадает, остальные метрики (h24/px12/rounded 8) требуют сверки с 48-bottom-tab-bar-strip отдельно.
4. aria-label «Bottom card», data-activity-slot — нет DOM.
5. Рендерится всегда (filePanelBottomVisible-гейта нет).

## Дополнение атрибутов (цикл 10)

- цвета: карта «Central Bottom» — `glint_surface_wv_holed` (`root.rs:5440-5459`): заливка bg_mantle #262533 dark / #fbf7f4 light (`palette.rs:55,93`), mid glint #262533 / #e6e1d4 (`palette.rs:87,125`), кромка glint_edge #ffffff α .18 / #3c2814 α .18 (`palette.rs:86,124`). Стрип-табы: текст text_secondary #adb3c7 / #524c43 (`slot_panel.rs:50`), hover bg_surface α .5 #3d3f51 / #e6e1d4 + text_primary #cfd4e2 / #322e28 (`slot_panel.rs:36,54`), active accent_primary α .16 (`slot_panel.rs:119`), drop-плейсхолдер — бордер accent α .7 + фон accent α .14 (`slot_panel.rs:134-135`), «…»-пикер text_muted #838aa0 / #6e685d (`slot_panel.rs:152`).
