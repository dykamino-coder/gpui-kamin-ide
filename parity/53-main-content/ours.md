# 53 main-content — наша реализация
Файлы: crates/shell/src/root.rs:3988-4012 (chat_content), 4046-4110 (main_column), 4709-4714 (main_wrap); crates/shell/src/ui/glint.rs:122-233 (glint_surface_wv_holed); crates/shell/src/ui/slot_panel.rs:187-237

## Структура (gpui-дерево кратко)
```
main_wrap: div .flex_1 .min_w(PANEL_MIN_SIZE=100) .h_full
└─ main_column (при main_bottom_visible — flex-col из 53+55+54; иначе gap_wrap(chat_content))
   └─ div h=relative(main_split) .min_h(100) → gap_wrap_v(pt4, pb0)
      └─ chat_content = glint_surface_wv_holed(
           div#main-slot .relative .size_full
           └─ slot_panel(Main, state, "Left", SlotIcon::Main, picker_up=false, drag_over, body))
```
slot_panel: если pinned>0 — стрип табов (h24-пилюли + «…») сверху, затем тело активного тула (tool_body) либо panel_placeholder("Left").

## Метрики (из кода, точные)
- Карточка (glint): внешний радиус RADIUS_LG=16, кромка 1px из двух 2-стоповых 135°-градиентов glint_edge (dark #ffffff α.18 / light #3c2814 α.18) 0→22% и 78%→100%, mid = glint_mid (dark #262533=bg_mantle / light #e6e1d4=bg_surface), внутренняя заливка inset 1px радиус 15 = bg_mantle (dark #262533 / light #fbf7f4)
- Высота: `relative(main_split)`, main_split кламп [MAIN_SPLIT_MIN 0.2, MAIN_SPLIT_MAX 0.85], дефолт 0.7; без нижнего ящика — 100% (gap_wrap с pt и pb 4)
- min-width 100 (PANEL_MIN_SIZE), min_h 100 у верхней секции
- Стрип: px 8, pt 4, gap 2; таб h 24, px 12, rounded 8, fs 12; active bg accent_primary α.16
- Дыры под composition-вебвью: glint рисуется paint_quad-сегментами вокруг зон + угловые маски R=12

## Отличия от original.md той же папки
1. Drop-индикация `data-activity-drop="over"/"blocked"` (accent-tint 10% + dashed outline / red-tint 12% + inset shadow) НЕ реализована — drag тулов подсвечивает только позицию вставки в стрипе (border_l 2 accent).
2. Оригинал: BottomTabBar + ActivityBody как отдельные дети `.main`; у нас единый slot_panel (стрип+тело) — структура эквивалентна, но таб-метрики свои (см. 65).
3. Высота: оригинал — инлайн `height: N%` (toFixed(2)); у нас `relative(main_split)` — та же доля, без строкового округления.
4. Customize/Welcome не внутри main-content: обрабатываются на уровне body (см. 52); оригинал рендерит CustomizePanel/WelcomePlaceholder внутри `.main`.
5. glint-кромка: CSS 4-стоповый linear-gradient → два наложенных 2-стоповых слоя (лимит gpui 0.2.2); пиксельно эквивалентно.
6. aria (`main[aria-label="Left"]`, data-activity-slot) отсутствует — в gpui нет DOM.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — карта `chat_content` (`root.rs:4845-4891`, glint + `slot_panel`) hover-стилей не имеет, у `.main` оригинала их тоже нет; drag-подсветка `data-activity-drop` не портирована (уже отмечено в «Отличиях»), а ховер табов стрипа — элемент 49/65.
