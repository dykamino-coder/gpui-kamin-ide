# 23 sessions-mode-root — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:699-861` (sessions_sidebar), `:484-509` (action_row), `crates\metrics\src\lib.rs:42-56`

## Структура (gpui-дерево кратко)
```
div#sidebar .relative .size_full .flex_col .pt(SPACE_2=8) .text_size(FS_SM=12)
├─ div .flex_col .px(8) .pb(8) .pt(4)                  ← actions
│  ├─ action_row("No folder session")
│  └─ action_row("New session")
├─ div .pl(12) .pr(8) .py(8) .text(FS_XS=11, MEDIUM, text_muted) "PROJECTS"
├─ (нет снапшота) → div .px(12) .py(12) text_muted "Loading sessions…"
└─ list: div .flex_1 .min_h(0) .flex_col .pl(SPACE_1=4) .pr(15) .pb(8) .overflow_y_scrollbar
   ├─ группы проектов (элементы 24-27)
   └─ (пусто) → div .px(12) .py(12) text_muted "No projects yet. Open a folder or start a session."
```
action_row: `.gap(10) .w_full .px(8) .py(6) .rounded(RADIUS_SM=8) .text_size(FS_MD=13) .text_color(text_secondary)` + иконка `fa-circle-plus 16px, w 20, text_muted`.

## Метрики (из кода, точные)
- root: padding-top 8; actions: 4/8/8 (top/lr/bottom) — 1:1
- action: gap 10, padding 6×8, radius 8, fs 13, hover `bg tint(bg_surface,0.6)` + `text_primary` — 1:1 (60% bg-surface)
- header: pl 12 / pr 8 / py 8, fs 11, weight 500, text_muted (dark #838aa0)
- list: pl 4 / **pr 15** / pb 8; empty: px 12 / py 12, text_muted, fs 12

## Отличия от original.md той же папки
1. **«New session» вызывает `new_no_folder_session` — тот же хендлер, что «No folder session»** (оригинал: пикер папки, затем сессия). Функциональная заглушка.
2. list `padding-right: 15px` vs оригинальные 4px (`--space-1`) — намеренно, чтобы скроллбар не перекрывал count-badge; строки справа на 11px короче.
3. Header «PROJECTS»: нет `letter-spacing: 0.08em` и `font-feature-settings: "ss01"` (текст уже uppercase-литерал, как в оригинале).
4. `.action:hover > i { color: text-primary }` не реализовано — иконка остаётся text_muted при ховере строки.
5. Тултипы кнопок («Start without a folder» / «Pick a folder, then start a session») отсутствуют.
6. Доп. состояние «Loading sessions…» (пока нет снапшота) — в оригинале его нет.
7. empty: оригинал `padding: 12 12 12 12` (space-3 + left 12) — у нас `px 12 / py 12`, совпадает.
