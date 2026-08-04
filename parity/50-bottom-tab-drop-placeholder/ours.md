# 50 bottom-tab-drop-placeholder — наша реализация
Файлы: `crates/shell/src/ui/slot_panel.rs` (`fn drop_placeholder`, вставка внутрь `.tabs`), `crates/shell/src/root.rs` (`tool_drag_over_index(slot)`, событие `ToolDragOverTab`)

## Структура (gpui-дерево кратко)
```
tabs (flex, items-center, gap 4, flex-1, min-w 0, overflow-x auto)
├─ [drag_over == i] drop_placeholder       ← пустой бокс 36×24
├─ tab(...)
└─ [drag_over == pinned.len()] drop_placeholder   ← вставка в конец
```
Прежняя индикация (левая accent-рамка 2px на целевом табе) убрана.

## Метрики (из кода, точные)
- 36×24 (высота = высоте таба), `flex-shrink: 0`.
- radius RADIUS_SM 8.
- Рамка 1px **dashed**, accent-primary #89b4fa при alpha 0.7.
- Фон accent-primary при alpha 0.14.
- Flex-item в `.tabs` с gap SPACE_1 4; собственных паддингов нет.

## Отличия от original.md той же папки
Кадра состояния drag в досье нет — вердикт по коду.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — плейсхолдер пустой (36×24 без содержимого), собственных паддингов нет; зазоры вокруг даёт `.tabs` gap SPACE_1 = 4 (`crates/shell/src/ui/slot_panel.rs`)
