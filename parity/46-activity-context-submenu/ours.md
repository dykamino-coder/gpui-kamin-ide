# 46 activity-context-submenu — наша реализация
Файлы: `crates/shell/src/overlay.rs:1240-1318` (сабменю внутри `tool_tab_menu()`); иконки слотов `crates/shell/src/ui/panel_placeholder.rs:34-80` (`slot_glyph_small`, аналог PanelIcon); обработчик `root.rs:1194-1198` (`MoveToolTo` → `move_activity(..., usize::MAX)`).

## Структура (gpui-дерево кратко)
```
[sub_open] div#tool-tab-submenu — тот же box_style, что корень (45):
  left = x + 180 + 2; top = (y + 40).min(vh-240).max(8)
  └ строка × (SLOT_ENTRIES минус текущий slot):
      div#ttm-<slot> flex gap SPACE_2 px SPACE_3 py SPACE_2 rounded SM
        ├ slot_glyph_small(SlotIcon)     // нативная рамка 14×12 (scale 1.0)
        └ div flex_1 label
      клик → MoveToolTo(src, id, dst) + CloseToolTabMenu
```
- Порядок и подписи: Sidebar / Left / Left Bottom / Center Bottom / Right / Right Bottom (`centralTop` исключён) — как оригинал.
- Move = append в конец целевого слота (`usize::MAX`) — как оригинал.

## Метрики (из кода, точные)
- Контейнер: идентичен 45 — min-w **180**, p 4, gap 1, rounded **12**, bg #3d3f51, border text_primary@0.06, **без тени**.
- Строка: gap **8**, px **12**, py **8**, rounded **8**, FS_SM **12px**, текст `p.text_primary` #cfd4e2; hover bg `p.text_primary`@0.10.
- Иконка слота: `slot_glyph_small` — рамка **14×12**, border 1px `p.text_muted` #838aa0, подсвеченный слот `p.text_muted`@0.85, скругления 1.5/1.0.

## Отличия от original.md той же папки
1. **Иконка «Sidebar» = иконке «Left»**: обе строки используют `SlotIcon::Main` (полный левый столбец) — у оригинала различаются варианты `left` и `main`; у нашего `SlotIcon` варианта для sidebar нет.
2. **Позиционирование фиксированное**: `x+MENU_W+2`, `y+40` (кламп) — оригинал якорит к rect строки `.itemMoveTo` c `clampToViewport(side:"right", offset:4)`; вертикальное смещение у нас всегда +40 от верха меню.
3. Нет тени (как 45 п.1); нет ролей `menu`/`menuitem`.
4. Сабменю не закрывается при уходе ховера на «Hide» (см. 45 п.3).
5. Иконка слота muted (рамка text_muted) — совпадает с оригинальным `.subItemIcon { color: text-muted }`.
6. Строки/hover/фильтр текущего слота/append-семантика — 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: строки сабменю — text_size FS_SM = 12 (`crates/shell/src/overlay.rs:1322`); font-weight не задан; иконка слота — не шрифт, а нативный div-глиф `slot_glyph_small` масштаба 1.0 (14×12) (`overlay.rs:1331`, реализация `crates/shell/src/ui/panel_placeholder.rs:97-99`), поэтому кегля у неё нет
