# 89 terminal-toolbar — наша реализация

Файлы: `crates/shell/src/ui/term_toolbar.rs:114-414` (`term_toolbar`),
`term_toolbar.rs:25-76` (`concave_corner` — вогнутые уголки активного таба),
`term_toolbar.rs:78-110` (`scroll_btn`), `term_toolbar.rs` (`TAB_W`).

## Структура (gpui-дерево)

```
bar — flex, items_end, gap 4, flex_shrink_0, px 25, min_h 30
      + probe_area("term-toolbar")
├─ overflow → scroll_btn #term-tabs-left (chevron-left `eab5`):
│    22×30, rounded 4, text_secondary, глиф codicon 12,
│    enabled → hover bg bg_surface + text_primary, иначе opacity .35
├─ tabs — flex, items_end, gap 2, flex_1, min_w 0, overflow_hidden;
│  окно [first..last], visible = ⌊(panel_w − 70) / TAB_W⌋
│  └─ #term-tab-{i}: flex, gap 6, h 30, px 10, min_w 80, max_w 220,
│     rounded 8 8 0 0, кегль 11, Medium (`letter-spacing: .02em` — нет в gpui)
│     ├─ codicon-terminal `ea85` 12
│     ├─ label — max_w 160, ellipsis
│     └─ #term-tabx-{i} — 16×16, rounded 4, codicon-close `ea76` 11,
│          opacity 0 → group_hover .7 (у активного таба виден всегда)
│     активный → сливается с `--editor-bg` тела ниже + вогнутые уголки 6
│                (радиус рисуется путём: `::before/::after` с
│                 radial-gradient в gpui нет)
│     иначе → hover bg bg_surface 50 % + text_primary
├─ overflow → scroll_btn #term-tabs-right (chevron-right `eab6`)
└─ #term-add — relative, flex_shrink_0, h 30, items_center
   ├─ #term-add-btn — 28×28, rounded_full, text_secondary,
   │    codicon-add `ea60` 15, тултип «New terminal»;
   │    меню открыто → bg accent_primary 14 % + accent_primary
   │    иначе → hover bg bg_surface + text_primary
   └─ menu_open → deferred(menu).with_priority(60)   (см. 90)
```

## Что закрыто (циклы 10-14)

Активный таб сливается с телом и получает вогнутые уголки; h 30, min-w 80 /
max-w 220 с ellipsis; кегль 11/500; крестик скрыт до ховера; «+» 28×28
круглая с подсветкой открытого меню; шевроны 22×30 с disabled .35; бар
`align-end` + боковые 25 + min-height 30.

## Осталось

1. Скролл табов — постраничное окно по индексу (шаг 1 таб, эвристика
   `TAB_W`), в оригинале пиксельный smooth-scroll на 80 % ширины.
2. `letter-spacing: .02em` у подписи таба — ограничение gpui.
3. Лейбл берётся из `TermSession`; per-shell иконка таба (`s.icon`) не
   поддержана — всегда `codicon-terminal`.

## Атрибуты (сверка ц.15)

- скругления: таб — `border-radius: 8px 8px 0 0` (`radius-sm` сверху),
  крестик и шеврон прокрутки — `--radius-xs` 4, кнопка «+» — круг 50 %,
  вогнутые уголки активного таба — радиус 6.
