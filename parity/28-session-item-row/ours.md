# 28 session-item-row — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:90-269` (session_row), `:39-58` (relative_time), `crates\shell\src\root.rs:804-822` (ActivateSession)

## Структура (gpui-дерево кратко)
```
div#{sid} .group(srow-{sid}) .flex .items_center .gap(SPACE_2=8) .w_full
  .h(24) .pl(16) .pr(8) .border_1(transparent) .rounded(RADIUS_XS=4)
  .text_size(FS_SM=12) .text_color(text_secondary) .cursor_pointer .overflow_hidden
  [active]  → bg linear-gradient(90°, tab_color@26% → tab_color@14%), border tab_color@45%, text_primary
  [tinted]  → bg linear-gradient(90°, tab_color@24% → tab_color@13%)
  [else]    → hover: bg tint(bg_surface,0.55) + text_primary
  [!open]   → opacity 0.6
  .on_mouse_down(L: ActivateSession) (R: OpenSessionMenu x,y) .on_hover(HoverPill)
├─ dot (элемент 29)
├─ div .flex_1 .min_w(0) .text_ellipsis {name}
├─ time: .text(FS_XS=11, SEMIBOLD=600, text_muted) .opacity(0.7) relative_time()
├─ pin_btn (элемент 30)
└─ .when(hovered) anchor_probe()      ← якорь overlay-пилюли (эл. 32)
```
relative_time 1:1 c relative-time.ts: now / Nm / Nh / Nd.

## Метрики (из кода, точные)
- h 24, gap 8, padding 0 8 0 16, border 1 transparent, radius 4, fs 12 — 1:1
- tab_color = `session.color` hex, дефолт `accent_primary` #89b4fa — 1:1
- active: 26%→14% + border 45% + text_primary; tinted: 24%→13%; hover (plain): bg_surface@55% + text_primary — 1:1
- time: fs 11, weight 600, text_muted #838aa0, opacity 0.7 — 1:1
- inactive: opacity 0.6 — 1:1

## Отличия от original.md той же папки
1. **`.tinted:hover` (30%/17%) НЕ РЕАЛИЗОВАН** — цветные неактивные строки не реагируют на ховер фоном/цветом.
2. **`.inactive:hover { opacity: 1 }` НЕ РЕАЛИЗОВАН** — неактивная строка остаётся 0.6 при ховере.
3. **Light-варианты НЕ РЕАЛИЗОВАНЫ** (tinted 26/16, hover 34/22, active 42/26 + border 60%, inactive 0.8) — одни dark-проценты в обеих темах.
4. dblclick → rename и F2 → rename НЕ РЕАЛИЗОВАНЫ (rename только из пилюли/контекст-меню).
5. Тултип абсолютного времени на `.time` отсутствует (сознательно: строка уже несёт ховер-механику пилюли).
6. `role="button"`/`tabIndex=0`/keyboard-активация — нет.
