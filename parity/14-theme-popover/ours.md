# 14 theme-popover — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:433-670 (appearance_popover);
рендер — overlay-слой

## Структура (gpui-дерево кратко)
```
div#appearance-popover (absolute в OVERLAY: top 46, right 8; ширина по контенту)
 ├ hit_area()
 ├ header (flex, px SPACE_1, pb SPACE_2)
 │  ├ title "Appearance" flex_1 (fs SM, SEMIBOLD, text_primary)
 │  └ #ap-system тумблер (fa-circle-half-stroke \u{f042} 11px + "System")
 └ columns row (flex, gap SPACE_2)
    ├ column "Dark"  (Kamin Dark + contributed dark)
    ├ column "Light" (Kamin Light + contributed light)
    └ column "Icons" (Catppuccin + contributed icon-темы)
```
Пики НЕ закрывают поповер (stop_propagation).

## Метрики (из кода, точные)
- фрейм: top px(TITLEBAR_HEIGHT+4)=46, right px(8), p SPACE_2 (8),
  gap SPACE_2 (8), rounded RADIUS_MD (12), bg p.bg_surface,
  border 1px tint(text_primary, 0.06), shadow dropdown_shadow()
- header: px SPACE_1 (4), pb SPACE_2 (8)
- title: fs FS_SM (12), FontWeight::SEMIBOLD, text_primary
- sysToggle: px SPACE_2 (8), py SPACE_1 (4), gap SPACE_2 (8), rounded RADIUS_SM (8),
  fs FS_XS (11); off: bg tint(text_primary, 0.06), color text_secondary;
  on (sysOn): bg tint(accent_primary, 0.16), color text_primary;
  hover: bg tint(accent_primary, 0.22)

## Отличия от original.md той же папки
1. sysToggle off-состояние: у нас bg tint(text_primary, 0.06); оригинал —
   transparent. Плюс цвет off: text_secondary vs var(--text-muted).
2. sysToggle hover: у нас accent_primary 22%; оригинал text_primary 10% +
   color text-primary.
3. columns: flex row gap 8 (колонки min-width 140) вместо grid
   `repeat(3, minmax(140px, 1fr))` — колонки не равноширинные, каждая по
   контенту (сознательно: фикс-ширина резала имена тем).
4. Позиция: right 8 от вьюпорта overlay; оригинал right:0 от .root триггера
   (совпадает с точностью до пары px), top 46 vs anchor+4 (~76 от верха окна
   у оригинала top: calc(100%+4) от триггера ≈ 39) — у нас поповер выше/ниже
   на несколько px, сверить скринами.
5. header gap var(--space-3) между title и toggle — у нас flex_1 у title
   (эквивалент по раскладке).
Фрейм (padding 8, gap 8, radius 12, bg-surface, border 6%, shadow) — 1:1.
