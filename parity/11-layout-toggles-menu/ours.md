# 11 layout-toggles-menu — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:28-46 (popover_frame),
48-139 (toggle_row/menu_label), 141-225 (layout_popover);
иконки слотов: crates/shell/src/ui/panel_placeholder.rs:78-80 (slot_glyph_small)

## Структура (gpui-дерево кратко)
```
popover_frame#layout-popover (absolute в OVERLAY-окне, top 46, w 220)
 ├ hit_area()
 ├ menu_label "LAYOUT"
 ├ ×6 toggle_row (Left / Left Bottom / File / Center Bottom / Right / Right Bottom)
 │   ├ checkbox 16×16 r3 (on: accent bg + codicon-check 12px accent_action_fg)
 │   ├ slot_glyph_small (PanelIcon-мини, text_muted; disabled → opacity 0.4)
 │   ├ label flex_1
 │   └ disabled: hint fs XS "Requires X"
 ├ divider 1px
 └ presets_section (элемент 12)
```
Клик по строке НЕ закрывает поповер (stop_propagation). Дети без родителя —
disabled (effective_on = on && !disabled).

## Метрики (из кода, точные)
- фрейм: top px(TITLEBAR_HEIGHT + 4.0)=46, left = vw − right(210) − 220,
  w px(POP_W=220.0), p m::SPACE_1 (4), rounded m::RADIUS_MD (12)
- цвета фрейма: bg p.bg_surface (#3d3f51), border 1px tint(text_primary, 0.06),
  shadow dropdown_shadow()
- menu_label: px SPACE_3 (12) / py SPACE_1 (4), fs m::FS_XS (11), text_muted, uppercase
- toggle_row: gap SPACE_2 (8), px SPACE_3 (12), py SPACE_2 (8), rounded SPACE_SM→RADIUS_SM (8),
  fs m::FS_SM (12), color text_primary (disabled → text_muted)
- hover: tint(text_primary, 0.10)
- check: 16×16, rounded 3, border 1px p.bg_overlay (#515567);
  on: bg/border p.accent_primary (#89b4fa), галка accent_action_fg (#313240)
- hint: fs XS, text_muted, opacity 0.7
- divider: h 1, mx SPACE_2 (8), my SPACE_1 (4), bg tint(text_primary, 0.06)

## Отличия от original.md той же папки
1. Ширина фикс 220px; оригинал min-width 220 + рост по контенту.
2. max-height calc(100vh−16px) + overflow-y:auto — НЕТ (длинный список
   пресетов не скроллится).
3. gap 1px между пунктами (flex-column) — НЕТ на фрейме (есть только внутри
   presets_section).
4. .menuLabel letter-spacing 0.04em — нет.
5. .itemHint цвет: у нас text_muted + opacity 0.7; оригинал var(--text-disabled) (#60667b).
6. Позиционирование: фикс top 46 от вьюпорта (оригинал: anchor-bottom + 6px
   offset ≈ 40, clampToViewport); у нас на 6px ниже.
7. border/divider: tint(text_primary,0.06) вместо var(--divider-soft) —
   численно совпадает по дизайн-решению, сверить токен.
8. disabled: cursor not-allowed — нет (просто нет cursor_pointer).

## Дополнение атрибутов (цикл 10)

- шрифты: `menu_label` FS_XS = 11 + `to_uppercase()` (`crates/shell/src/ui/layout_popover.rs:163-166`; letter-spacing 0.04em оригинала в gpui недоступен); строка тумблера text_size FS_SM = 12 (`layout_popover.rs:122`); hint отключённой строки FS_XS = 11 (`layout_popover.rs:142`); галка чекбокса codicon 12 (`layout_popover.rs:108`); `menu_item` секции Layouts — FS_SM = 12 (`layout_popover.rs:453`) + codicon 16 (`layout_popover.rs:463`); font-weight нигде в меню не задаётся (нормальный)
