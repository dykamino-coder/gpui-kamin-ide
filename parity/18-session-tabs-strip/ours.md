# 18 session-tabs-strip — наша реализация

Файлы: crates/shell/src/ui/session_tabs.rs:334-549 (session_tabs,
ordered_chips, overflow_button, tabs_overflow_menu);
«+» и spacer — crates/shell/src/ui/titlebar.rs, ВНУТРИ `.strip` (id "new-session")

## Структура (gpui-дерево кратко)
```
strip (flex items-center, flex_1, min_w 0, overflow_hidden — БЕЗ своих
       боковых отступов; воздух слева даёт `.leftCluster`, досье 02)
 ├ row чипов
 ├ «+» #new-session (26×26, mx 6)
 └ spacer (flex 1 1 auto, min_w 24)
row (flex items-center, min_w 0, overflow_hidden)
 ├ ×fit chip (все ФИКС 180px; активная всегда видима — подтягивается в конец)
 └ hidden не пуст: overflow_button "N ⌄" (h28, px6, r12)
     → tabs_overflow_menu в OVERLAY-окне (w240, max_h 400, список скрытых:
       точка 8px цвета сессии + имя; клик = активация)

titlebar.rs: «+» #new-session — 28×28 круг, bg_surface, fa-plus 12px,
клик → ToggleNewSessionMenu(x,y) (дропдаун folder/no-folder в overlay)
```
Константы: CHIP_W=180.0, CHIP_GAP=2.0; резерв 36px под overflow-кнопку.
Reorder: ChipPress/ChipDragOver/ChipRelease (порог и активация в root),
порядок = user order поверх сортировки last_opened.

## Метрики (из кода, точные)
- strip: flex_1, min_w 0, h_full, overflow_hidden, своих padding нет
- row чипов: min_w 0, overflow_hidden
- chip: 180×28 фикс (элемент 19)
- overflow_button: h 28, px 6, ml 2, gap 2, rounded RADIUS_MD (12), fs 12,
  text_secondary; hover/open: bg p.bg_surface + text_primary; chevron codicon 12
- overflow-меню: w 240, max_h 400, p SPACE_1 (4), rounded RADIUS_MD, bg
  p.bg_surface, border tint(text_primary, 0.06), gap 1; item px SPACE_3
  py SPACE_2 r8 fs SM, hover text_primary 10%, точка 8×8
- «+»: 28×28 rounded_full, ml SPACE_1 (4), bg p.bg_surface,
  color text_secondary, глиф 12px; hover bg p.bg_overlay + text_primary

## Отличия от original.md той же папки
1. Оверфлоу-модель другая: у нас невлезшие чипы уходят в кнопку «N ⌄» с
   поповером; оригинал — горизонтальный скролл (overflow-x auto, скрытый
   скроллбар) без кнопки. Плюс чипы у нас не сжимаются (180 фикс) — у
   оригинала flex 0 1 180 (min 44).
2. dropBar (вертикальная метка вставки 2×22 accent + glow) НЕ РЕАЛИЗОВАНА —
   вместо неё border-left 2px accent на целевом чипе (см. 19).
3. «+»: 28×28 (оригинал 26×26), ml 4 (оригинал margin 0 6), hover — bg
   p.bg_overlay (оригинал color-mix accent 36% + color accent + scale 1.06);
   живёт в titlebar.rs после слота, а не внутри стрипа.
4. Слот не flex:1 (см. 04); spacer-drag — отдельный div.flex_1 титлбара
   (эквивалент .spacer с app-region: drag).
5. pl 48px слева (оригинал padding 0 12) — сознательное отступление.
6. Пикер «+»: пункты те же (folder/no-folder), но рендер в overlay-окне;
   min-width 200 / padding 6 8 оригинала здесь не сверялись (другой файл —
   пикер не в session_tabs.rs).
7. При 0 сессий: оригинал возвращает null; у нас row остаётся (пустой) +
   «+» всегда виден.

## Дополнение атрибутов (цикл 10)

- цвета: сам стрип (`row`) ни background, ни text_color не задаёт (`crates/shell/src/ui/session_tabs.rs:447-454`) — прозрачный, наследует text_color(text_muted #838aa0) и FS_SM=12 корня титлбара (`crates/shell/src/ui/titlebar.rs:197-198`). Цвета — у детей: чип bg bg_mantle #262533, текст text_secondary #adb3c7, резервный border text_primary при альфе 0.0 (`session_tabs.rs:65,68,69`); active — градиент tab_color 0.26 → 0.14 + border tab_color 0.45 + text_primary #cfd4e2 (dark; light 0.42/0.26/0.60) (`session_tabs.rs:72-84`); tinted 0.15 → 0.08, hover 0.22 → 0.12 (`session_tabs.rs:87,99-100`); обычный hover bg bg_surface #3d3f51 + text_primary (`session_tabs.rs:107-109`); drop-bar 2×22 bg accent_primary #89b4fa + glow accent_primary@0.6 blur 4 (`session_tabs.rs:477-483`); overflow-меню bg bg_surface #3d3f51, border text_primary@0.06 (`session_tabs.rs:539-545`)
- шрифты: у стрипа собственного кегля нет; чип text_size 12 (`session_tabs.rs:64`), font-weight не задан; `chip_action` — codicon 16 либо fa-thumbtack 10 (`session_tabs.rs:344,347`); overflow-меню — кегль строк задаётся ниже по файлу (`session_tabs.rs:552+`)
