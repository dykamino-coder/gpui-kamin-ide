# 30 session-pin-button — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:293-341` (pin_btn)

## Структура (gpui-дерево кратко)
```
div#pin-{sid} .flex_shrink_0 .w(20) .h(20) .items_center .justify_center
  .rounded(RADIUS_XS=4) .cursor_pointer
  .text_color(pinned ? tab_color : text_muted)
  .hover(bg tint(text_primary,0.12) + text_primary)
  .tooltip("Pin to top bar" | "Unpin from top bar")
  .on_mouse_down(L: kamin:sessions:setPinned !pinned)
└─ fa-thumbtack 10px (контейнер 14×14)
[!pinned] → .invisible().group_hover(srow-{sid}, visible)   ← виден только при ховере строки
```

## Метрики (из кода, точные)
- 20×20, radius 4, fa-thumbtack 10px — 1:1
- pinned: виден всегда, цвет tab_color — 1:1
- unpinned: скрыт до ховера строки (invisible + group_hover) — механика 1:1 (у оригинала display:none → inline-flex)

## Отличия от original.md той же папки
1. **Hover-стиль другой**: оригинал — `opacity: .7` при появлении, `opacity: 1` + `color: var(--tab-color)` на own-hover, БЕЗ фона; у нас — полная непрозрачность сразу, own-hover даёт `bg text_primary@12%` + `text_primary` (белеет, а не красится в tab_color).
2. Промежуточного состояния «opacity 0.7 на row-hover» нет.
3. `aria-label` нет.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ (`crates/shell/src/ui/sessions_list.rs:350-390`); бокс 20×20 (`sessions_list.rs:353-354`), глиф центрируется flex+items_center+justify_center (`:355-357`); собственных margin нет — зазор до соседей даёт `.row` gap SPACE_2 = 8 (`sessions_list.rs:114`); внутренний бокс глифа 14×14 (`sessions_list.rs:388-389`)
- цвета: покой — pinned → tab_color (цвет сессии, дефолт accent_primary #89b4fa, `sessions_list.rs:100-104`), иначе text_muted #838aa0 (`sessions_list.rs:360-364`); hover → text_color tab_color, БЕЗ фона (`sessions_list.rs:366`); фон в любом состоянии не задаётся; непинованная кнопка невидима, по group-ховеру строки проявляется с opacity 0.7 (`sessions_list.rs:391-396`)
- шрифты: `fa(FA_THUMBTACK, 10.0)` — кегль 10, семейство FA_FAMILY (`sessions_list.rs:386-387`); своего text_size у кнопки нет — наследует FS_SM = 12 строки (`sessions_list.rs:122`). Оригинал: `.pin > i { font-size: 10px }` (`sidebar/SessionItem.module.css:98`) — совпадает
