# 130 design-color-tokens — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\theme\src\palette.rs:1-144 (`Palette`, const `DARK` / `LIGHT`, тесты инвариантов); crates\shell\src\ui\design_panel.rs:452-481 (`swatch`), 483-493 (`group_label`), 496-560 (4 группы, 26 токенов), 26-72 (`section`), 882-887 (секция «Colors»)

## Структура/содержание
Палитра — Rust-структура `Palette` (const `DARK` / `LIGHT`), без CSS-переменных; contributed-темы позже лягут рантайм-мапой поверх.
Секция «Colors»: колонка групп (gap 16) из 4 групп (gap 8), каждая = `group_label` + flex-wrap ряд свотчей (gap 8).
```
colors div.flex_col.gap(16)
└─ группа × 4: div.flex_col.gap(8)
   ├─ group_label «SURFACE» / «TEXT» / «ACCENT» / «SEMANTIC»
   └─ swatches div.flex.flex_wrap.gap(8)
      └─ swatch × N: div.flex.items_center.gap(8).min_w(180).flex_grow.p(8).rounded(4).bg(bg_surface α .3)
         ├─ чип div 28×28.rounded(4).border_1(text_primary α .12).bg(токен)
         └─ подпись «--{token}» (mono, FS_XS, text_secondary)
```
Группы и токены — тот же порядок, что `COLOR_GROUPS` оригинала: Surface (bg-primary, bg-base, bg-mantle, bg-sidebar, bg-surface, bg-overlay), Text (text-primary, text-subtext, text-secondary, text-muted, text-disabled), Accent (blue, sapphire, teal, green, yellow, orange, red, maroon, pink, purple, rosewater), Semantic (accent-primary, accent-action, accent-action-hover, accent-action-fg) — 26 токенов.

## Метрики (из кода, точные)
- отступы: свотч p 8 (SPACE_2); у групп и контейнера padding нет; внешний отступ даёт `section()` — тело p 16 (SPACE_4), секция mb 24 (SPACE_6)
- гэпы: контейнер групп gap 16 (SPACE_4); группа gap 8 (SPACE_2); ряд свотчей gap 8 (SPACE_2); внутри свотча gap 8 (SPACE_2)
- цвета: подложка свотча p.bg_surface #3d3f51 α 0.3; бордер чипа p.text_primary #cfd4e2 α 0.12; подпись p.text_secondary #adb3c7; `group_label` p.text_muted #838aa0; тело секции bg p.bg_mantle #262533 + border 1px p.bg_surface α 0.6; заголовок секции p.text_primary #cfd4e2, сабтайтл p.text_muted #838aa0
- скругления: свотч 4 (RADIUS_XS); чип 4 (RADIUS_XS); тело секции 12 (RADIUS_MD)
- шрифты: подпись свотча «JetBrains Mono» 11 (FS_XS) weight 400; `group_label` 11 (FS_XS) weight 700 BOLD (UA-дефолт `<h3>`), текст через `to_uppercase()`; заголовок секции 16 (FS_LG) / 600; сабтайтл 12 (FS_SM), line-height 15.6
- ховер: N/A: ховер — витрина статична, в секции Colors нет ни одного `.hover(...)` (совпадает с оригиналом)

Полная таблица наш-токен → значение, DARK (palette.rs:52-88):

| Токен | Значение |
|---|---|
| bg_primary | #313240 |
| bg_base | #313240 |
| bg_mantle | #262533 |
| bg_sidebar | #1d1d28 |
| bg_surface | #3d3f51 |
| bg_overlay | #515567 |
| editor_bg | #1d1c25 |
| editor_fg | #dcdce4 |
| editor_cursor | #a0a0d0 |
| text_primary | #cfd4e2 |
| text_subtext | #afb6ca |
| text_secondary | #adb3c7 |
| text_muted | #838aa0 |
| text_disabled | #60667b |
| text_muted_2 | #7f849c |
| text_muted_light | #acb2d2 |
| accent_blue | #89b4fa |
| accent_sapphire | #74c7ec |
| accent_red | #f38ba8 |
| accent_maroon | #eba0ac |
| accent_green | #a6e3a1 |
| accent_yellow | #f9e2af |
| accent_pink | #f5c2e7 |
| accent_purple | #cba6f7 |
| accent_orange | #fab387 |
| accent_teal | #94e2d5 |
| accent_rosewater | #f5e0dc |
| accent_action | #89b4fa (= blue) |
| accent_action_hover | #74c7ec |
| accent_action_fg | #313240 |
| accent_primary | #89b4fa |
| bg_surface_hover | #3b3b52 |
| bg_overlay_hover | #3e3e56 |
| glint_edge | rgba(255,255,255,0.18) |
| glint_mid | #262533 (= bg_mantle) |

LIGHT (palette.rs:90-126): bg_primary #f6efeb; bg_base #fbf8f1; bg_mantle #fbf7f4; bg_sidebar #f4f1ea; bg_surface #e6e1d4; bg_overlay #d6d0c0; editor_bg #fcfaf6; editor_fg #48433c; editor_cursor #48433c; text_primary #322e28; text_subtext #463f37; text_secondary #524c43; text_muted #6e685d; text_disabled #938e82; text_muted_2 #524c43; text_muted_light #524c43; accent_blue #3b6fc4; sapphire #3a8aa3; red #ca3939; maroon #d35a5a; green #5e9855; yellow #c89a3f; pink #c46598; purple #8a5fc8; orange #da8343; teal #4a9999; rosewater #c08571; accent_action #da8343 (= orange); action_hover #b16527; action_fg #ffffff; accent_primary #da8343; bg_surface_hover #d8d4c4; bg_overlay_hover #c2bcab; glint_edge rgba(60,40,20,0.18); glint_mid #e6e1d4 (= bg_surface).

Инварианты закрыты тестами (palette.rs:132-143): dark action = blue, light action = orange; glint_mid = bg_mantle (dark) / bg_surface (light).

## Отличия от original.md той же папки
Значения всех присутствующих токенов совпадают токен-в-токен (dark): bg-primary/base/mantle/sidebar/surface/overlay, все 5 text-*, все 11 accent-*, accent-primary/action/action-hover/action-fg, editor-bg/fg/cursor, bg-surface-hover #3b3b52, bg-overlay-hover #3e3e56, text-muted-2 #7f849c, text-muted-light #acb2d2, glint (edge rgba(255,255,255,.18), mid = bg-mantle как в dark-theme.css; `:root`-фоллбек с mid = bg-base не воспроизводим — не нужен). Расхождений в ЗНАЧЕНИЯХ НЕТ.

Отсутствуют в палитре (в оригинале есть, у нас нет полей):
- `--overlay-modal` rgba(0,0,0,.5) / `--overlay-soft` .35 / `--overlay-deep` .6 — у нас скрим модалки захардкожен `rgba(0,0,0,0.6)` в `modal.rs:64-71` (это ровно overlay-deep; overlay-modal и overlay-soft не используются нигде);
- вся семья `--bg-tint-*` (red / red-soft / green / green-soft / orange / blue);
- `--accent-blue-soft/-2/-3`, `--accent-purple-soft`, `--accent-green-soft`, `--accent-red-dark/-2/-3`, `--accent-orange-dark`, `--accent-yellow-dark`;
- `--divider-soft` (color-mix text-primary 6%) — у нас собирается ad-hoc `tint(text_primary, 0.06)` по месту (напр. editor_tabs.rs:272);
- все `--tint-*` color-mix токены — каждое место делает `tint()` со своей α;
- семейство `--accent-primary-soft` / `--bg-tint-primary` / `--tint-primary-*` алиасов.

Отличия витрины (секция Colors):
- состав и порядок 26 токенов, 4 группы, `group_label` uppercase — совпадают 1:1 с `COLOR_GROUPS`;
- чип 28×28, border text-primary 12%, свотч p 8 + bg `bg-surface 30%` + radius-xs, подпись mono FS_XS — совпадают;
- layout: CSS-grid `repeat(auto-fill, minmax(180px, 1fr))` заменён на flex-wrap с `min_w 180 + flex_grow` (в gpui нет grid) — перенос и растяжка последнего ряда ведут себя иначе;
- `letter-spacing: 0.06em` у `.groupLabel` в gpui недоступен;
- у подписи нет `word-break: break-all` (длинные имена не рвутся);
- подпись — обычный div с моно-семейством, а не `<code>` (визуально эквивалентно, семантики нет);
- цвет подписи `text_secondary` совпадает с оригиналом.
