# 38 activity-bar-nav — наша реализация

Файлы: `crates/shell/src/ui/activity_bar.rs` (`activity_bar`, `tile`,
`drop_placeholder`); монтаж и модель — `crates/shell/src/root.rs` (ветка
сайдбара), состояние — `crates/shell/src/activity.rs`.

> Досье переписано в цикле 14 по фактическому коду: прежний текст утверждал
> ширину 44, единый gap 2, отсутствие плейсхолдера и contributed-плиток —
> ничего из этого в коде нет.

## Структура (gpui-дерево)
```
div#activity-bar .flex_shrink_0  w = 48 + 4, pr 4, h_full,
                 flex col, items_center, gap SPACE_2, py SPACE_3
  ├ probe_area("activity-bar")
  ├ tile("customize", gear, active = customize_open)      // фиксирована сверху
  └ div .list  flex col, items_center, w_full, gap 2
      ├ [drop_index == i] drop_placeholder()
      ├ tile(id, icon, active, dragging, drag = Some((slot, id, i, tx)))
      └ [drop_index == len] drop_placeholder()
  └ dots("…")                                             // пикер тулзов
```

## Метрики (из кода, точные)
- Ширина `ACTIVITY_BAR_WIDTH` **48** + 4 к боксу и `pr 4`: в gpui бокс-модель
  border-box, и зазор колонки иначе съедал бы ширину — центр плитки совпадает
  с оригиналом.
- `gap` бара SPACE_2 **8** (между группами), `gap` списка **2** — как
  `.bar { gap: 8 }` и `.list { gap: 2 }`.
- `py` SPACE_3 **12**.
- Бар гаснет вместе со скрытым сайдбаром (`when(sidebar_visible …)`).
- Плитки строятся из `pinned` слота Sidebar через `lookup_any` — builtin И
  contributed.
- Дроп-зоны drag'а: карта сайдбара, сам бар, main, main-bottom,
  central-bottom, обе правые карты и оба правых рейла; цель ЛИПКАЯ.

## Отличия от original.md той же папки
1. Подсветку дропа получает КАРТА, а сам `<nav>` бара/рейла — нет
   (в оригинале `data-activity-drop` висит и на баре).
2. Индекс вставки приходит только с ховера плитки: курсор над картой вне
   плиток даёт вставку В КОНЕЦ, а оригинал отдал бы 0, если курсор выше
   середины первой плитки.
3. У сайдбара слой подсветки навешен на обёртку (шире карты на 8) и со
   скруглением 16, тогда как цель оригинала — `aside.sidebar` без скругления.
4. `aria`-роли отсутствуют.

## Атрибуты
- отступы: бар py 12, pr 4; плитки без собственных паддингов (бокс 32×32)
- цвета: фон бара прозрачный; плитка text-muted #838aa0 → активная
  accent-primary@16 % + text-primary #cfd4e2
- шрифты: подписей нет; глиф тула 18
- скругления: плитка RADIUS_SM 8
- гэпы: 8 между группами, 2 между плитками
- ховер: у неактивной плитки — подложка bg-surface@50 % + text-primary
