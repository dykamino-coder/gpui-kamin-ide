# 145 sample-block-wrapper — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn block`, `fn block_hint`)

## Структура/содержание
Обёртка каждого семпла — 1:1 с `.compRow` / `.compLabel` / `.compHint` / `.compInline` оригинала:
```
div (flex-col, gap SPACE_2 8)              ← .compRow
├─ подпись (uppercase)                     ← .compLabel
├─ [hint]                                  ← .compHint
└─ div (flex, flex-wrap, gap SPACE_2 8)    ← .compInline
    └─ тело семпла
```

## Метрики (из кода, точные)
- `.compRow`: flex-col, gap SPACE_2 8.
- `.compLabel`: fs FS_XS 11, weight 700 (UA-дефолт `<h3>`), цвет text-muted #838aa0, текст в верхнем регистре (`to_uppercase` в Rust).
- `.compHint`: mb SPACE_1 4, fs FS_XS 11, line-height 1.3, text-muted.
- `.compInline`: flex, flex-wrap, gap SPACE_2 8 — без него одиночный ребёнок (меню, дерево) растягивался на всю ширину панели.
- `.compStack` (между блоками): flex-col, gap SPACE_4 16.

## Отличия от original.md той же папки
`letter-spacing: 0.06em` у подписи в gpui недоступен.

## Дополнение атрибутов (цикл 10)

- отступы: собственных паддингов у обёртки нет — вертикальные интервалы задают `gap SPACE_2` 8 внутри блока и `gap SPACE_4` 16 между блоками; единственный отступ — `mb SPACE_1` 4 у hint (`crates/shell/src/ui/design_panel.rs`, `fn block_hint`)
