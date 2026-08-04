# 78 customize-content-panel — наша реализация

Файлы: `crates/shell/src/ui/customize.rs:216-244` (`title_for`),
`customize.rs:460-584` (`customize_panel`), `crates/shell/src/root.rs`
(обёртка glint-картой на всю панельную область + contrib-ветка czShared).

## Структура (gpui-дерево)

```
glint_surface_wv_holed — карта на ВСЮ панельную область
└─ customize_panel: div size_full, flex-col, min_h 0
   ├─ header — flex_shrink_0, pt 20 / px 24 / pb 12,
   │    border-bottom 1px bg_overlay 30 %
   │    ├─ title — FS_XL(22), Semibold, text_primary
   │    └─ subtitle — mt 4, FS_MD(13), text_muted
   └─ #cz-body-scroll — flex_1, min_h 0, overflow_y_scroll, py 16 / px 24,
        + probe_area("cz-body") для поэлементного кропа страниц
      body по `active`:
        settings   → секции + pref_row (см. 82) и LegacyBridgeCard (83)
        design     → design_panel (79)
        extensions → extensions_panel (86)
        logs       → logs_panel (80)
        system     → system_panel (81)
        _          → `.placeholder`: центр, gap 8, p 28,
                     fa-screwdriver-wrench 32 opacity .5 + «Phase B»
   contrib-страница (root.rs): вместо customize_panel — czShared-вебвью
     (`div#cz-contrib` relative size_full + probe_area + wv) либо центр «Loading…»
```

## Что закрыто (циклы 10-14)

Кегли `--fs-xl` 22 / `--fs-md` 13, `border-bottom` под хедером, раздельные
паддинги header `20 24 12` и body `16 24`, централизованный скролл тела,
`ComingSoon` с иконкой и подписью «Phase B», тексты subtitle 1:1
(`title_for`).

## Осталось

1. Панель обёрнута в glint-карту на всю панельную область — в оригинале
   `CustomizePanel` живёт ВНУТРИ main-карты (архитектурное следствие того,
   что у нас карта рисуется слоем выше).
2. Contrib-страница не выводит хедер с subtitle «Contributed by an
   extension.»: вебвью занимает карту целиком.

## Атрибуты (сверка ц.15)

- цвета: заголовок `--text-primary` #cfd4e2, подзаголовок `--text-muted`
  #838aa0, разделитель хедера `--bg-overlay` #515567 при 30 %, подложка —
  glint-карта (`--bg-mantle` #262533 → `--glint-edge` white 18 %).
- ховер: у шелла панели hover-правил нет — как и у `.panel`/`.header`/`.body`
  оригинала; ховеры принадлежат наву Customize (36) и телу страницы (79-86).
- скругления: у шелла нет; радиус даёт карта слота `--radius-lg` 16.
- шрифты: title `--fs-xl` 22/600, subtitle `--fs-md` 13/400,
  `.placeholder` — подпись `--fs-sm` 12.
