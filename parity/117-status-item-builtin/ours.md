# 117 status-item-builtin — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:132-156 (item), 168-199 (варианты)

## Структура (gpui-дерево кратко)
```
div#{tip}: flex.items_center.gap(4).px(8).rounded(4)
├─ (glyph) codicon 12px
└─ div {label}
tooltip(tip)
```
Варианты: «N active» (circle-filled \u{ea71}, accent_green), «N failed» (warning \u{ea6c}, accent_yellow), «N off» (circle-slash \u{eabd}, text_muted), «N cmds» (symbol-keyword \u{eb62}, text_muted).

## Метрики (из кода, точные)
- gap 4, px 8 (SPACE_2), rounded 4 (RADIUS_XS), fs 11 (FS_XS), codicon 12px
- Tone-цвета: ok p.accent_green #a6e3a1, warn p.accent_yellow #f9e2af, muted p.text_muted #838aa0
- Hover: bg p.bg_surface #3d3f51 a=.6 + text p.text_primary #cfd4e2

## Отличия от original.md той же папки
1. Метрики и hover совпадают (gap 4 / px space-2 / radius-xs / fs-xs / codicon 12 / bg-surface 60%).
2. Не `<button tabIndex=-1>` — обычный div; aria-label нет; тултип — наш gpui-тултип (overlay), не data-tooltip.
3. `.brand`-тона в item() нет — бренд-элемент собран отдельно (№120).
4. Вертикальный padding отсутствует у обоих; высота элемента тянется до
   высоты бара (`h_full` на обёртке, `status_bar.rs:403`) — пункт был
   написан до этой правки и утверждал обратное (ревью ц.35).
5. Клики по встроенным счётчикам («N active» → палитра, MCP-счётчики →
   настройки) — ЗАФИКСИРОВАННОЕ отступление по прямой просьбе пользователя;
   у оригинала эти элементы некликабельны.
