# 41 activity-drop-placeholder — наша реализация
Файлы: `crates/shell/src/ui/activity_bar.rs` (`fn drop_placeholder`, вставка в `list`), `crates/shell/src/root.rs` (`tool_drag_over_index(PanelSlot::Sidebar)` — индекс вставки), hit-тест дроп-зон по probe-bounds там же.

## Структура (gpui-дерево кратко)
```
list (flex-col, items-center, gap 2)
├─ [drop_index == i] drop_placeholder      ← пустой бокс 32×32
├─ tile(...)
└─ [drop_index == entries.len()] drop_placeholder   ← вставка в конец
```
Пустой элемент без содержимого, как `<li class="dropPlaceholder">` оригинала.

## Метрики (из кода, точные)
- 32×32, `flex-shrink: 0` — повторяет форму живой плитки.
- radius RADIUS_SM 8.
- Рамка 1px **dashed**, цвет accent-primary #89b4fa при alpha 0.7.
- Фон accent-primary при alpha 0.14.
- Позиционирование — обычный flex-item в `.list` (gap 2), собственных отступов нет.

## Отличия от original.md той же папки
1. Индекс вставки берётся из общей drag-модели, а не из позиционного расчёта по `clientY` (см. элемент 38) — для вертикального бара это значит, что место вставки может отличаться от того, куда реально наведён курсор.
2. Кадра состояния drag в досье пока нет — вердикт по коду.
