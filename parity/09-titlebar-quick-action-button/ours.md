# 09 titlebar-quick-action-button — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:70-105 (fn action_button)

## Структура (gpui-дерево кратко)
```
div#id (occlude, size×size, flex center, rounded(radius), cursor_pointer)
 └ child (svg | fa | codicon)
```
Универсальная: quick-action/theme = 28×28 r8; layout-toggles = 26×26 r12
(вызовы передают size/radius per-кнопка).

## Метрики (из кода, точные)
- w/h = m::ICON_BUTTON_ROUND (28.0) для quick-actions; rounded m::RADIUS_SM (8)
- цвета: color p.text_secondary (#adb3c7), bg прозрачный
- hover: bg p.bg_surface (#3d3f51), color p.text_primary (#cfd4e2)
- active: bg tint(p.accent_primary, 0.16), color p.text_primary
  (hover-стиль при наведении перебивает — как в оригинале, где .btn:hover
  специфичнее .active)

## Отличия от original.md той же папки
1. `.btn :global(.codicon) { font-size: 14px }` — у нас размер глифа задаёт
   вызывающий (gear 13px, theme 12px); для codicon-детей внутри quick-action
   фикс-14 не воспроизводится.
2. transition var(--transition-fast) — нет.
3. aria-pressed — не применимо.
Метрики (28×28, radius 8, палитра base/hover/active 16% accent) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ ни по одной оси (`crates/shell/src/ui/titlebar.rs:87-111`) — центровка глифа через flex + items_center + justify_center (`titlebar.rs:96-99`); бокс w/h = ICON_BUTTON_ROUND = 28 (`crates/metrics/src/lib.rs:25`, вызов `titlebar.rs:225-226`); собственных margin у кнопки нет — внешний зазор даёт gap 1 строки (`titlebar.rs:220`) и mx SPACE_1 = 4 у divider (`titlebar.rs:258`). Совпадает с оригиналом: `.btn` тоже без padding, 28×28 (`titlebar/TitlebarQuickActions.module.css:9-19`, `--layout-icon-button-round: 28px` в `theme/layout-tokens.css:60`)
