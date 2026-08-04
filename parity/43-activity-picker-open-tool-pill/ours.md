# 43 activity-picker-open-tool-pill — наша реализация
Файлы: `crates/shell/src/ui/slot_panel.rs:150-183` (`open_tool_btn()`), передаётся как `extra` в `crates/shell/src/ui/panel_placeholder.rs:84-119` (`panel_placeholder_ex`).

## Структура (gpui-дерево кратко)
```
panel_placeholder_ex(label, hint, SlotIcon, extra = open_tool_btn):
  колонка по центру: slot_glyph → label → hint → пилюля

open_tool_btn:
  div#opentool-<slot> inline-строка
    ├ "Open Tool"
    └ icon::fa("\u{f078}" fa-chevron-down, 10px)   // FontAwesome solid, бокс 16×16
on_mouse_down(Left) → ShellEvent::OpenToolPicker(slot, cursor_x, cursor_y, up)
```
- Открывает тот же пикер (элемент 44), что и «…»; используется во всех пустых слотах (`slot_panel` без body).

## Метрики (из кода, точные)
- Паддинг: `px(SPACE_3)` **12px** гориз., `py(SPACE_1)` **4px** верт.; `mt(SPACE_1)` **4px**.
- `gap(SPACE_2)` **8px** между текстом и шевроном; rounded `RADIUS_SM` **8px**.
- Фон: `p.accent_primary` #89b4fa @ alpha **0.16**; hover @ **0.26**.
- Текст: `p.text_primary` #cfd4e2, `FS_SM` **12px**.
- Шеврон: FontAwesome solid (weight 900) **10px** в боксе 16×16.

## Отличия от original.md той же папки
1. Метрики совпадают 1:1 (padding 4/12, gap 8, mt 4, radius 8, fs-sm, accent 16%→26%, текст primary, шеврон 10px).
2. Открытие: оригинал — тот же `ActivityPicker` с anchor-обёрткой `.pickerAnchorInline`, clamp от rect кнопки; у нас меню позиционируется от координат клика (см. 42 п.2-3).
3. Нет `aria-haspopup`/`aria-expanded`.
4. Шеврон у нас в flex-боксе 16×16 (`icon::fa`) — в оригинале голый `<i>` 10px; на геометрию строки не влияет (высота задаётся паддингом).
