# 01 Titlebar — наша сторона (gpui-kamin-ide)

Файлы: crates/shell/src/ui/titlebar.rs (+ вызов в root.rs), метрики
crates/metrics/src/lib.rs (TITLEBAR_HEIGHT=42.0, FS_SM...).

## Факт (probe tree, live)
zone: 0,0 2048×42.4 (ЛОГИЧЕСКИЕ px) — ВЫСОТА 42.4 ≠ 42 оригинала.
TITLEBAR_HEIGHT const = 42.0 → лишние 0.4 добавляет что-то в обвязке
(бордер? чип? паддинг контейнера) — найти при фиксе.

## Скрин
ours.png (максимизированное окно, дефолт-тема DARK).

## Структура (gpui-дерево кратко, titlebar.rs:135-407)
div#titlebar (relative, h TITLEBAR_HEIGHT, w_full, flex items-center,
text_size FS_SM, color text_muted; drag = свой DRAG_ARM: down армирует,
move ≥4px → start_native_window_drag; dblclick = zoom)
 ├ probe_area("titlebar")
 ├ .brand 42×42 (flex center, img kaminoid.svg 26×26)
 ├ quick-actions row (gap 1, px SPACE_2): toggle-sidebar [+divider+gear]
 ├ tabs-контейнер (flex, min_w 0, flex_shrink, overflow_hidden, h_full) → session_tabs
 ├ «+» new-session 28×28 круг (НЕТ в оригинальном Titlebar.tsx — там «+» внутри SessionTabs)
 ├ div flex_1 (пустота-drag)
 ├ #command-search (h26, пилюля)
 ├ layout-toggles 26×26 r12 · theme-toggle 28×28 r8
 └ контролы: DevTools + min/max/close (36×36 круги)

## Метрики (из кода)
- h = m::TITLEBAR_HEIGHT (42.0); fs = m::FS_SM (12); color p.text_muted (#838aa0 DARK)
- bg НЕ задан (прозрачный, градиент root просвечивает) — как оригинал
- корень: без padding/gap/radius — как оригинал

## Отличия от original.md
1. Живая высота 42.4px вместо 42 (см. «Факт» выше) — источник лишних 0.4 не найден.
2. leftCluster как контейнер отсутствует: brand и quick-actions — прямые дети корня,
   ширина НЕ пиннится к сайдбару (детали в 02-titlebar-left-cluster/ours.md).
3. Кнопка «+» (new session) — в титлбаре между табами и пустотой; в оригинале
   «+» живёт внутри SessionTabs-стрипа (элемент 18).
4. Drag-механика: свой пороговый native caption-drag вместо -webkit-app-region:
   drag на корне + no-drag на детях (поведенчески эквивалентно; z-index/webkit-токены не применимы).
5. font-family: gpui-дефолт окна (Bricolage задаётся на уровне окна) — совпадает по факту.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_SM = 12 на корне (`crates/shell/src/ui/titlebar.rs:197`); font-family/weight на титлбаре не задаются — наследуются от окна; кегли глифов: window-controls codicon 16 (`titlebar.rs:69`), quick-action svg 14×12 (`titlebar.rs:244-245`), gear fa 12 (`titlebar.rs:275`), search codicon 12 (`titlebar.rs:381`), layout fa 13 (`titlebar.rs:399`), theme fa 12 (`titlebar.rs:415`), DevTools fa 13 + label FS_SM=12 (`titlebar.rs:446-447`)
- ховер: у самого корня `#titlebar` ховера нет (`titlebar.rs:152-199`) — только у детей:
  - control_button: bg bg_surface #3d3f51, fg text_primary #cfd4e2 (`titlebar.rs:43,59`); close (danger): bg accent_red #f38ba8, fg bg_primary #313240 (`titlebar.rs:41`)
  - action_button: bg bg_surface #3d3f51 + text_primary #cfd4e2 (`titlebar.rs:86,108`)
  - search-пилюля: bg bg_surface #3d3f51 + text_secondary #adb3c7 (`titlebar.rs:376`)
  - DevTools: bg bg_surface #3d3f51 + accent_primary #89b4fa (`titlebar.rs:437`)
  - «+»: НЕпрозрачный микс accent_primary 36% + bg_surface 64%, fg accent_primary #89b4fa (`titlebar.rs:326-338`)
