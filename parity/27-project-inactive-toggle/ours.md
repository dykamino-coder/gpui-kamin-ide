# 27 project-inactive-toggle — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:511-546` (inactive_toggle), `crates\shell\src\root.rs:799-803` (ToggleInactive)

## Структура (gpui-дерево кратко)
```
div#inact-{pid} .flex .items_center .gap(6) .w_full
  .pl(18) .pr(8) .py(3)
  .text_size(FS_SM=12) .text_color(text_disabled)
  .cursor_pointer .hover(text_secondary)
  .on_mouse_down(L: ToggleInactive)
├─ codicon chevron-{down|open ? : right} 12px
└─ "{count} inactive session{s}"
```

## Метрики (из кода, точные)
- gap 6, padding 3 8 3 18 (top/bottom 3, right 8, left 18) — 1:1
- fs 12 (`FS_SM`), цвет `text_disabled` #60667b, hover → `text_secondary` #adb3c7 — 1:1
- chevron codicon 12px — 1:1; down при открытом / right при закрытом — 1:1
- Плюрализация «1 inactive session» / «N inactive sessions» — 1:1

## Отличия от original.md той же папки
Расхождений по метрикам нет. Единственное: у оригинала это `<button>` с `font: inherit`, у нас div (в gpui разницы нет); класса-модификатора `.inactiveOpen` нет и у оригинала он пустой.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_SM = 12 (`crates/shell/src/ui/sessions_list.rs:606`); font-weight не задан (нормальный); chevron `codicon(..., 12.0)` (`sessions_list.rs:613-616`). Текст — `"{count} inactive session{s}"` (`sessions_list.rs:617-620`)
