# 08 titlebar-quick-actions-row — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:198-244

## Структура (gpui-дерево кратко)
```
div (flex items-center, gap 1, px SPACE_2, flex_shrink_0)
 ├ action_button#toggle-sidebar (28×28 r8, active=sidebar_visible)
 │  └ svg("icons/panel-left.svg") 14×12, color text_secondary
 └ when !sidebar_visible:
    ├ divider div 1×14, mx SPACE_1, bg p.bg_surface
    └ action_button#customize-gear (28×28 r8) └ fa(FA_GEAR \u{f013}, 13.0)
```

## Метрики (из кода, точные)
- row: gap px(1.0), px m::SPACE_2 (8), размеры по контенту
- divider: w 1 × h 14, mx m::SPACE_1 (4), bg p.bg_surface (#3d3f51)
- кнопки — элемент 09; иконка сайдбар-тумблера svg 14×12; gear 13px

## Отличия от original.md той же папки
1. Gear: НЕ ФУНКЦИОНАЛЕН — on_click пустой `|_, _| {}` (оригинал открывает
   Customize), active всегда false (оригинал: active = sidebarMode==="customize").
2. Иконка тумблера — статичный файл panel-left.svg вместо PanelIcon slot="left"
   (визуально тот же 14×12 глиф, но не параметризован currentColor-вариантами).
3. Условие показа gear: у нас только `!sidebar_visible`; в оригинале то же —
   совпадает. Tooltip: у нас Hide/Show sidebar — 1:1; у gear нет варианта
   "Close Customize".
Метрики row (gap 1, padding 0 8, divider 1×14 margin 0 4 bg-surface) — 1:1.

## Дополнение атрибутов (цикл 10)

- скругления: N/A: скругления — у самой строки rounded не задан (`crates/shell/src/ui/titlebar.rs:217-222`); скругления только у детей: кнопки RADIUS_SM = 8 (`titlebar.rs:226,264`), divider 1×14 без скругления (`titlebar.rs:255-259`)
- ховер: у строки ховера нет; у кнопок внутри bg bg_surface #3d3f51 + text_primary #cfd4e2 (`titlebar.rs:86,108`); active-состояние (не ховер) — accent_primary #89b4fa при альфе 0.16 + text_primary (`titlebar.rs:114-115`)
