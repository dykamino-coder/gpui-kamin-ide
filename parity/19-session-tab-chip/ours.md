# 19 session-tab-chip — наша реализация

Файлы: crates/shell/src/ui/session_tabs.rs:23-332 (chip, chip_action)

## Структура (gpui-дерево кратко)
```
div#tab-{id} (occlude, group, 180×28, r12)
 ├ leading 16×16
 │  ├ dot 4×4 круг (invisible при pinned и на group-hover)
 │  ├ pin-кнопка absolute inset-0 (fa-thumbtack 10px, r3;
 │  │   !pinned: invisible → visible на group-hover; pinned: всегда, цвет tab-color)
 │  └ switching: вместо всего — codicon \u{eb19} 11px accent (спиннер-глиф)
 ├ label flex_1 (ellipsis, nowrap, FontWeight::MEDIUM)
 └ s.open: chip_action disconnect (codicon-debug-disconnect \u{ead0} 12px,
     16×16 r4; скрыт → group-hover; на активном чипе виден всегда)
```
ЛКМ = ChipPress (активация на up без движения), dblclick = BeginRename,
ПКМ = OpenSessionMenu (общее меню сайдбара), move с зажатой ЛКМ = ChipDragOver.

## Метрики (из кода, точные)
- размеры: w px(CHIP_W=180.0) ФИКС, h 28; ml 2; pl 10, pr 6, gap 6
- rounded m::RADIUS_MD (12); fs 12; label weight MEDIUM (500)
- база: bg p.bg_mantle (#262533), color p.text_secondary (#adb3c7);
  hover (без цвета): bg p.bg_surface + text_primary
- active: bg linear_gradient 90° tint(tab_color,0.26)→tint(tab_color,0.14),
  border 1px tint(tab_color,0.45), color text_primary; dot = tab_color
- tinted (color, не active): градиент 0.15→0.08; hover 0.22→0.12 + text_primary
- tab_color = session.color hex | p.accent_primary (#89b4fa)
- dot: 4×4, bg text_muted (active: tab_color)
- pin: fa 10px, rounded 3, hover bg tint(tab_color, 0.16);
  pinned → цвет tab_color, иначе text_secondary
- disconnect (chip_action): 16×16, rounded 4, глиф 12px, color text_muted,
  hover bg tint(text_primary, 0.12) + text_primary
- sleeping (pinned && !open): opacity 0.55, тултип «(sleeping — click to reactivate)»
- drag_over: border_l_2 accent_primary
- switching: leading = codicon-спиннер, тултип «(loading conversation…)»

## Отличия от original.md той же папки
1. Ширина: фикс 180px; оригинал flex 0 1 180px, min 44, max 240 — наши чипы
   не сжимаются/не растут.
2. margin: у нас всем ml 2; оригинал margin-left 2 + `:first-child { margin-left: 6px }`
   — первый чип у нас на 4px левее.
3. close/disconnect: у нас 16×16 r4, hover text_primary 12%; оригинал 18×18
   radius-xs(4), hover 14%; показ у оригинала через opacity 0/1 с transition
   .12s — у нас invisible/visible без анимации.
4. dot↔pin свап: механика та же (group-hover), но у оригинала display-свап,
   у нас visibility (эквивалент).
5. switching: оригинал — пульсация dot (@keyframes tab-switching 1s);
   у нас замена leading на codicon-глиф \u{eb19} без анимации вращения.
6. dndDragging opacity 0.4 — НЕ РЕАЛИЗОВАНО (перетаскиваемый чип не тускнеет);
   вместо dropBar стрипа — border_l_2 на целевом чипе.
7. Light-тема: оригинал усиливает альфы ([data-theme=light]: tinted 26/16,
   active 42/26, border 60%) — у нас те же альфы в обеих темах.
8. keyboard (Enter/Space activate, role=tab, aria) — не применимо/нет.
Остальное (h28, r12, padding 10/6, gap 6, fs 12, weight 500, палитра
градиентов 26/14/45 и 15/8→22/12, dot 4px, pin 10px, sleeping 0.55) — 1:1.
