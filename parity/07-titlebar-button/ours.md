# 07 titlebar-button — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:31-68 (control_button — default/close),
346-377 (devtools-вариант inline), crates/shell/src/ui/icon.rs:44-55 (codicon)

## Структура (gpui-дерево кратко)
```
control_button: div#id (occlude, window_control_area, круг)
 └ codicon(glyph, 14.0)      // бокс 16×16

devtools (inline в titlebar.rs):
 div#devtools ├ fa(FA_BUG, 13.0) └ div fs SM "DevTools"
```

## Метрики (из кода, точные)
default (.btn):
- w/h = m::ICON_BUTTON_TITLEBAR (36.0); mx m::SPACE_1 (4)
- rounded_full (50%)
- иконка: codicon в боксе 16×16, глиф 14px
- цвета: color p.text_muted (#838aa0), bg прозрачный
- hover: bg p.bg_surface (#3d3f51), color p.text_primary (#cfd4e2)

close (danger=true):
- hover: bg p.accent_red (#f38ba8), color p.bg_primary (#313240)

devtools:
- h 36, width auto, px m::SPACE_3 (12), mx m::SPACE_1 (4), gap m::SPACE_1 (4)
- rounded m::RADIUS_MD (12); label text_size m::FS_SM (12)
- color p.text_muted; hover: bg p.bg_surface + color p.accent_primary (#89b4fa)

## Отличия от original.md той же папки
1. Размер глифа default-кнопок: у нас 14px, оригинал `.btn > i` font-size 13px.
2. transition var(--transition-fast) — нет.
3. devtools-вариант у нас не переиспользует control_button (отдельная вёрстка),
   но метрики (padding 0 12, gap 4, radius 12, fs-sm label, hover accent) — 1:1.
Остальное (36×36, круг, margin 0 4, палитра hover, close-danger) — 1:1.
