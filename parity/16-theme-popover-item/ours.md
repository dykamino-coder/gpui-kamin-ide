# 16 theme-popover-item — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:449-514 (closure `item`
внутри appearance_popover)

## Структура (gpui-дерево кратко)
```
div#id (flex items-center, строка-option)
 ├ icon-слот w16 (fa moon/sun/icons 12px, text_muted)
 ├ label flex_1 (ellipsis, nowrap)
 └ tick-слот w12 (codicon-check \u{eab2} 10px, accent_primary;
                  invisible при !on — ширина стабильна)
```

## Метрики (из кода, точные)
- px SPACE_3 (12), py SPACE_2 (8), gap SPACE_2 (8), rounded RADIUS_SM (8)
- fs m::FS_SM (12), color p.text_primary
- icon-слот: w px(16.0), глиф fa 12px, color p.text_muted
- tick: w px(12.0), глиф 10px, color p.accent_primary (#89b4fa),
  invisible (не удалён из вёрстки) при !on
- hover (только при !on): bg tint(text_primary, 0.08)
- picked (on): bg tint(accent_primary, 0.16) постоянный, hover не перебивает

## Отличия от original.md той же папки
1. hover-фон: у нас text_primary 8%; оригинал 10%.
2. Цвет иконки: у нас text_muted; оригинал .itemIcon без цвета → наследует
   var(--text-primary) от .item. Иконка тусклее оригинала.
3. Галка: codicon-check вместо fas fa-check (другой глиф-шрифт, тот же смысл).
4. picked не задаёт color: у нас текст остаётся text_primary (совпадает,
   оригинал тоже text-primary).
Метрики (padding 8/12, gap 8, r8, fs 12, icon w16/12px, tick w12/10px accent,
picked accent 16% с visibility-галкой) — 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_SM = 12 (`crates/shell/src/ui/layout_popover.rs:499`); font-weight не задан; иконка темы `fa(glyph, 12.0)` в слоте w 16 (`layout_popover.rs:503,508`); галка `fa("\u{f00c}", 10.0)` в слоте w 12 (`layout_popover.rs:537,544`). Оригинал: `.item { font-size: var(--fs-sm) }` 12, `.itemIcon { width:16; font-size:12 }`, `.itemTick { width:12; font-size:10 }` (`titlebar/ThemeQuickToggle.module.css:124,141-146,150-157`) — совпадает
