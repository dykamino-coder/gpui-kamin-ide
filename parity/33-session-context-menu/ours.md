# 33 session-context-menu — наша реализация
Файлы: `crates\shell\src\ui\context_menu.rs:28-36,66-107,145-328` (session_menu/menu_item), `crates\shell\src\overlay.rs:929` (рендер), `:174-190` (dropdown_shadow), `crates\shell\src\root.rs:823-827,4890-4891,5054-5073` (open/close/скрим)

## Структура (gpui-дерево кратко)
```
overlay-окно:
div .absolute .left(x) .top(y) .w(MENU_W=208) .flex_col .p(SPACE_1=4)
  .rounded(RADIUS_MD=12) .bg(bg_surface) .border_1(tint(text_primary,0.06))
  .shadow(dropdown: 0 8 24 rgba(0,0,0,0.45))
  + hit_area() + stop_propagation на клик внутри
├─ menu_item codicon-edit "Rename"                       → BeginRename
├─ .when(open) menu_item sparkle "Auto-rename from chat" → claude-bridge.regenerateTitle
├─ menu_item codicon-{pinned-dirty|pin} "{Unpin|Pin} … top bar"
├─ .when(open) menu_item codicon-circle-slash "Deactivate (free memory)"
├─ swatches (элемент 34)
├─ divider .h(1) .mx(4) .my(4) .bg(tint(text_primary,0.06))
└─ menu_item codicon-trash "Delete" (danger)             → ConfirmModal
```
menu_item: gap 8, px 8, py 6, radius SM=8, fs 12, codicon 14px; base `text_secondary`, hover `bg text_primary@10%` + `text_primary`; danger — base/hover `accent_red`, hover bg `accent_red@16%`. Кламп: x/y в вьюпорт с margin 8 (est высота 260). Закрытие: скрим/клик-мимо/Esc в root.

## Метрики (из кода, точные)
- padding 4, radius 12, bg_surface #3d3f51, border text_primary@6%, divider 1px mx4 my4 — 1:1
- item: 6×8, gap 8, radius 8, fs 12, codicon 14 — 1:1
- hover 10% / danger 16% — 1:1; состав и условия пунктов (open-only) — 1:1
- MENU_MARGIN 8 — 1:1 (MENU_MARGIN_PX)

## Отличия от original.md той же папки
1. **Ширина: фикс `MENU_W = 208` vs оригинальный `min-width: 200` (авто-рост под контент)**.
2. Shadow: 0 8 24 @45% (общий overlay dropdown_shadow) vs `--shadow-dropdown` 0 6 24 @30% — темнее и ниже.
3. Кламп по высоте — по оценке est_h=260, а не по фактическому измерению (`visibility:hidden` до измерения у оригинала); при куцем меню у нижнего края позиция чуть выше идеала.
4. z-index-механика не нужна: меню в отдельном overlay-окне (эквивалент `--z-titlebar-popover`).
5. `role="menu"`/`menuitem` нет.

## Дополнение атрибутов (цикл 10)

- шрифты: пункт меню text_size FS_SM = 12 (`crates/shell/src/ui/context_menu.rs:97`); font-weight не задан; глиф пункта `codicon(glyph, 14.0)` (`context_menu.rs:105`); глиф «Clear colour» `codicon(CIRCLE_SLASH, 13.0)` (`context_menu.rs:299`). Заголовков/секций с иным кеглем в меню нет
