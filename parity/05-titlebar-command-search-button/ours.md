# 05 titlebar-command-search-button — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:289-315 (id "command-search")

## Структура (gpui-дерево кратко)
```
div#command-search (occlude, flex items-center, пилюля)
 ├ codicon(SEARCH \u{ea6d}, 12.0)   // бокс 16×16, глиф 12px
 └ "Type a command…"
```
Клик → open_palette() (ShellEvent → командная палитра). Тултип
"Open command palette (Ctrl+Shift+P)".

## Метрики (из кода, точные)
- h px(26.0); ширина авто по контенту
- px m::SPACE_3 (12), mr m::SPACE_2 (8), gap m::SPACE_2 (8)
- rounded m::RADIUS_SM (8)
- шрифт: text_size m::FS_XS (11)
- цвета: bg = tint(p.bg_surface, 0.6) (#3d3f51 60%), border 1px
  tint(p.bg_overlay, 0.3) (#515567 30%), color p.text_muted (#838aa0)
- hover: bg p.bg_surface (#3d3f51), color p.text_secondary (#adb3c7)

## Отличия от original.md той же папки
1. .searchHint имеет свой `padding: 0 var(--space-2)` → фактический зазор
   иконка-текст в оригинале = gap 8 + 8 = 16px, справа от текста +8px;
   у нас только gap 8. Пилюля уже на ~16px.
2. transition var(--transition-fast) — нет (gpui hover мгновенный).
Остальное (h26, radius 8, fs 11, color-mix 60%/30%, hover) — 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_XS = 11 (`crates/shell/src/ui/titlebar.rs:373`); font-weight не задан; глиф codicon SEARCH 12.0 (`titlebar.rs:381`); подпись «Type a command…» тем же кеглем 11 в div с px SPACE_2 = 8 (`titlebar.rs:383`). Отклонение: у оригинала есть `.kbd` — font-mono 10px (`titlebar/Titlebar.module.css:110-117`), у нас kbd-чипа нет
