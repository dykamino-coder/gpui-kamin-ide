# 10 layout-toggles-trigger — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:316-330 (вызов action_button
"layout-toggles"), 70-105 (action_button)

## Структура (gpui-дерево кратко)
```
action_button#layout-toggles (26×26, r12)
 └ fa(FA_TABLE_COLUMNS \u{f0db}, 13.0)
```
Клик → ShellEvent::ToggleLayoutPopover (поповер в overlay-окне, элемент 11).
Tooltip "Layout panels".

## Метрики (из кода, точные)
- w/h px(26.0); rounded m::RADIUS_MD (12)
- глиф fa-table-columns 13px (бокс 16×16)
- цвета: color p.text_secondary (#adb3c7); hover bg p.bg_surface + text_primary

## Отличия от original.md той же папки
1. Open-состояние НЕ РЕАЛИЗОВАНО: `active` захардкожен false — оригинал при
   `aria-expanded="true"` красит триггер в accent-primary 16% + text-primary.
2. .anchor-обёртка не нужна (поповер позиционируется от вьюпорта в overlay).
3. transition — нет.
Метрики триггера (26×26, radius 12, глиф 13px, base/hover цвета) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ (`crates/shell/src/ui/titlebar.rs:87-111`), бокс 26×26 задан явно вызовом (`titlebar.rs:387`); margin нет — стоит прямым ребёнком корня титлбара между search-пилюлей (её mr SPACE_2 = 8, `titlebar.rs:367`) и theme-кнопкой
- шрифты: собственного текста нет; глиф `fa(FA_TABLE_COLUMNS, 13.0)` — кегль 13 (`titlebar.rs:399`); text_size у кнопки не задан, наследует FS_SM = 12 корня (`titlebar.rs:197`). Оригинал: `.trigger > i { font-size: 13px }` (`titlebar/LayoutToggles.module.css:23`) — совпадает
