# 134 design-shadow-tokens — наша реализация

Файлы: `crates/shell/src/ui/design_panel.rs` (блок Shadows), значения —
`crates/shell/src/ui/shadows.rs` (девять токенов × две темы).

> Досье переписано в цикле 13: прежний текст описывал «три бокса 96×56 с
> именем ВНУТРИ бокса» и утверждал, что словаря `shadow-*` не существует —
> и то, и другое неверно (ревью зоны 130-159).

## Структура (gpui-дерево)
```
div (flex, flex_wrap, gap SPACE_4)                // .shadowGrid
  └ ×9 div (w 140, flex col, items_center, gap SPACE_2)   // .shadowItem
      ├ бокс 100×64, rounded RADIUS_SM, bg bg-primary, shadow = токен
      └ token_name("--shadow-…")                  // моно FS_XS
```

## Метрики (из кода, точные)
- Порядок токенов = `SHADOW_TOKENS` оригинала: mini, card, bar, tab, dropdown,
  card-popup, toast, lg, modal.
- Ячейка **140**, бокс **100×64**, `rounded` RADIUS_SM **8**, фон `bg_primary`,
  `gap` внутри ячейки SPACE_2 **8**, между ячейками SPACE_4 **16**.
- Значения теней — `ui/shadows.rs`: тёмная тема 1:1 с `dark-theme.css`,
  светлая — ink-tinted `rgba(27, 26, 22, …)` 1:1 с `light-theme.css`.

## Отличия от original.md той же папки
1. `grid auto-fill minmax(140px, 1fr)` заменён на flex-wrap с фиксированной
   ячейкой 140 — последний ряд не растягивается.
2. `letter-spacing` у подписи отсутствует (ограничение движка).

## Атрибуты
- отступы: ячейка gap 8, сетка gap 16
- цвета: бокс `bg_primary` #1e1e2e, подпись text-secondary #adb3c7; сами тени — чёрные alpha в тёмной теме и ink rgba(27, 26, 22, …) в светлой
- шрифты: подпись моно FS_XS 11
- скругления: бокс RADIUS_SM 8
- гапы: 8 внутри ячейки, 16 между ячейками
- ховер: N/A — витрина токенов, ховера нет ни у нас, ни в оригинале
