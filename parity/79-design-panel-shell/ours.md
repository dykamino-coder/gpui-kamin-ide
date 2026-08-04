# 79 design-panel-shell — наша реализация

Файлы: `crates/shell/src/ui/design_panel.rs:26-71` (`section`),
`design_panel.rs:77-110` (`block` / `block_hint`),
`design_panel.rs:540-969` (`design_panel` — сборка 6 секций).

## Структура (gpui-дерево)

```
#design-panel — flex-col, size_full, min_h 0, overflow_y_scrollbar
                + probe_area("design-panel")
└─ 6 × section(title, subtitle, body): flex-col, gap 12, mb 24
   ├─ sectionHeader — flex-col, gap 2
   │  ├─ title    — FS_LG(16), Semibold, text_primary
   │  └─ subtitle — FS_SM(12), line-height 12×1.3, text_muted
   └─ sectionBody — border 1px bg_surface 60 %, rounded 12 (RADIUS_MD),
        bg bg_mantle, p 16

block / block_hint (внутри Components): flex-col, gap 8
├─ compLabel — FS_XS(11), Bold (UA-дефолт `<h3>`), text_muted, UPPERCASE
├─ compHint (опц.) — mb 4, FS_XS(11), line-height 11×1.3, text_muted
└─ compInline — flex, flex_wrap, gap 8 (иначе одиночный ребёнок
     растягивался колонкой на всю ширину панели)

Секции: Colors (4 группы токенов), Typography, Spacing, Radius, Shadows,
Components. Тексты title/subtitle 1:1 с оригиналом.
```

## Что закрыто (циклы 10-14)

`--fs-lg` 16 у заголовка и `--fs-sm` 12 у подзаголовка; карточка
`.sectionBody` (рамка `bg-surface 60 %` + radius-md + `--bg-mantle` +
padding 16); `gap: 12` внутри секции и `gap: 2` в её хедере; межсекционный
ритм 24.

## Осталось

1. Скролл живёт на самой панели, а в оригинале скроллит `.body` панели
   Customize — при вложенном скролле полоса рисуется на другом уровне.
2. `letter-spacing` у `.compLabel` и `.sectionHeader` — ограничение gpui.

## Атрибуты (сверка ц.15)

- ховер: оболочка статична — `section()` и сборка секций hover-правил не
  задают, как и `.root`/`.section`/`.sectionBody` оригинала; ховеры живут
  внутри семплов (135-153).
- цвета: title `--text-primary` #cfd4e2, subtitle `--text-muted` #838aa0,
  рамка `.sectionBody` — `--bg-surface` #3d3f51 при 60 %, фон
  `--bg-mantle` #262533.
