# 52 app-shell — наша реализация
Файлы: crates/shell/src/root.rs:3387 (render), 5091-5130 (корневой фон+radial), 5131-5189 (titlebar), 5190-5369 (body), 5370-5380 (status_bar), 2565-2587 (gap_wrap); crates/shell/src/ui/radial_bg.rs:49-125; crates/metrics/src/lib.rs:14,20,32-33

## Структура (gpui-дерево кратко)
```
div (root, .relative, track_focus, key_context "Root")
├─ canvas: корневой фон bg_sidebar, paint_quad по hole_segments_multi
│    (дыры под composition-вебвью вместо сплошного .bg())
├─ radial_bg.layers(viewport): 2 absolute img (baked PNG)
├─ titlebar(...)                                   (высота TITLEBAR_HEIGHT=42)
├─ div#body .relative .flex_1 .min_h(0) .flex .pl(4)
│  ├─ activity_bar(...)
│  ├─ when(sidebar_visible): div w(sidebar_w) + gap_wrap + v_handle("sidebar-handle")
│  ├─ when(customize_open): gap_wrap(glint(customize_panel)) на всю ширину
│  ├─ when(!customize && has_active):
│  │    when(main_visible)  → main_wrap (flex_1, min_w 100)
│  │    when(file_visible)  → main_file_handle + file_wrap (w=file_w, shrink 0)
│  │    when(right_visible) → file_right_handle + right_wrap (w=right_w+44, shrink 0)
│  └─ when(!customize && !has_active): welcome_full (flex_1)
├─ status_bar(...)                                 (высота STATUS_BAR_HEIGHT=24)
└─ скрим/оверлеи (palette/quickopen/fif/modal)
```
Межпанельный зазор — НЕ flex-gap: каждая колонка оборачивается `gap_wrap` (px 4 + условные pt/pb 4), смежные 4+4 = 8px.

## Метрики (из кода, точные)
- Корневой фон: `p.bg_sidebar` — dark #1d1d28, light #f4f1ea (canvas-заливка сегментами вокруг вебвью-дыр)
- Radial-слои (radial_bg.rs): бейк в PNG, alpha = A·(1 − d/0.6):
  - purple: эллипс 1200×600, центр 20%/10% вьюпорта, accent_purple (#cba6f7 dark / #8a5fc8 light), peak α 0.08
  - primary: эллипс 800×500, центр 90%/90%, accent_primary (#89b4fa dark / #da8343 light), peak α 0.06
- body: `pl(px(BODY_GUTTER_X=4))` — гаттер ТОЛЬКО слева; flex-row; min_h 0
- gap_wrap: px 4, pt/pb 4 (условно) — эквивалент body gap 8 + гуттер 4
- Titlebar 42 / StatusBar 24; текст-цвет задают дети (нет общего color на корне)

## Отличия от original.md той же папки
1. Гуттер: оригинал `padding: 0 var(--space-1)` (4px с ОБЕИХ сторон body); у нас только `pl(4)` — справа роль гуттера играет rail правой колонки (44px). При скрытой правой панели правый край без 4px-гуттера.
2. Механизм зазоров: оригинал — flex `gap: 8` на .body; у нас — паддинги gap_wrap каждой колонки. Визуально то же 8px, но зазор принадлежит колонке (hit-зоны сплиттеров живут в нём).
3. Радиальный градиент: CSS radial-gradient → бейк PNG-спрайтов (линейный спад до 0.6 — математически совпадает с `transparent 60%`), но без color-mix — прямая альфа поверх bg_sidebar; при resize спрайты не перегенерируются (фикс-размер эллипса — как в CSS).
4. `color: var(--text-primary)` на appWrapper не переносился — цвет задаётся точечно в каждом компоненте.
5. Welcome: у нас заменяет все три колонки (welcome_full flex_1); оригинал: welcome внутри mainColumn + FilePanel/RightPanel опущены — итоговая площадь совпадает.
6. Customize: у нас одна glint-карта на всю область (колонки не рендерятся); оригинал держит mainColumn (CustomizePanel внутри него) — визуально эквивалентно.
7. fill-режимы (fileFills/rightFills — колонка растягивается при скрытом main) НЕ реализованы: file_wrap/right_wrap всегда фикс-ширины.
8. Дыры под composition-вебвью в корневом фоне — наша специфика (в оригинале нет; там DOM-слои).

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — у корня (`root.rs` render, `div().relative().track_focus()`) hover-стилей нет, как и у `.appWrapper`/`.body` оригинала; ховер живёт только у ручек (55/57/62) и детей.
