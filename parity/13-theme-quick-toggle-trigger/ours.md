# 13 theme-quick-toggle-trigger — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:331-345 (вызов action_button
"theme-toggle"), 70-105 (action_button); глиф — TitlebarState.theme_glyph

## Структура (gpui-дерево кратко)
```
action_button#theme-toggle (28×28, r8)
 └ fa(state.theme_glyph, 12.0)   // moon \u{f186} / sun \u{f185} / half \u{f042}
```
Клик → ShellEvent::ToggleAppearancePopover. Tooltip
"Appearance — themes & icons". Логика глифа (dark→moon, light→sun,
system/contributed→half) — снаружи, в state.

## Метрики (из кода, точные)
- w/h = m::ICON_BUTTON_ROUND (28.0); rounded m::RADIUS_SM (8)
- глиф fa 12px в боксе 16×16
- цвета: color p.text_secondary (#adb3c7) — БАЗА action_button
- hover: bg p.bg_surface, color p.text_primary

## Отличия от original.md той же папки
1. Базовый цвет: у нас text_secondary (#adb3c7) через общий action_button;
   оригинал .trigger — var(--text-muted) (#838aa0). Иконка светлее оригинала.
2. transition — нет.
3. .root-обёртка (relative, outside-click) не нужна — поповер в overlay.
Метрики (28×28, radius 8, глиф 12px, hover) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ (`crates/shell/src/ui/titlebar.rs:87-111`), бокс 28×28 = ICON_BUTTON_ROUND (`titlebar.rs:403`, `crates/metrics/src/lib.rs:25`); margin нет. Совпадает с оригиналом `.trigger` 28×28 без padding (`titlebar/ThemeQuickToggle.module.css:8-18`)
- шрифты: своего текста нет; глиф `fa(state.theme_glyph, 12.0)` — кегль 12 (`titlebar.rs:415`); text_size не задан, наследует FS_SM = 12 корня (`titlebar.rs:197`). Оригинал: `.trigger > i { font-size: 12px }` (`ThemeQuickToggle.module.css:20`) — совпадает
