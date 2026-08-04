# 157 activity-drop-highlight — наша реализация
Файлы: crates/shell/src/root.rs:4946-4973 (hit-test дроп-зон по probe_registry: sidebar/right-top/main-bottom/right-bottom), root.rs:5411-5441 (ghost у курсора: α 0.45 без цели / 0.85 над целью), root.rs:2176-2184 + ui/slot_panel.rs:111-113 (accent-полоса border_l_2 у таба-цели вставки)

## Структура/содержание
ЧАСТИЧНО. При drag плитки тула: (1) hit-test 4 дроп-зон по probe-bounds; (2) ghost-пилюля у курсора (label тула, bg accent_primary, α 0.45→0.85 когда над валидной зоной) — единственная индикация «зона примет дроп»; (3) внутри стрипа зоны — вставочная accent-полоса `border_l_2` accent_primary у таба под курсором. Подсветки САМОЙ карточки-приёмника (fill + dashed outline) нет; состояния «blocked» (красный, назначение уже содержит активность) нет.

## Метрики (из кода, точные)
Ghost: offset +10/+8 от курсора; px SPACE_3 / py 4; radius SM 8; fs SM 12; bg accent_primary α 0.85 (над зоной) / 0.45 (вне); текст accent_action_fg. Вставочная метка: border-left 2px accent_primary. Порог начала drag 4px (1:1 с activity-dnd).

## Отличия от original.md той же папки
- `[data-activity-drop="over"]` (bg accent 10% + dashed outline 1px accent 60%, offset −2, transition 150ms) — НЕ РЕАЛИЗОВАНО: карточка-приёмник не подсвечивается, сигнал перенесён на α ghost'а.
- `[data-activity-drop="blocked"]` (red tint 12% + inset box-shadow 2px red 60%) — НЕ РЕАЛИЗОВАНО, blocked-состояние не вычисляется.
- Insertion-метка у нас — сплошная полоса 2px (в оригинале strip-placeholder — dashed прямоугольник 32/36px, зона 41/50); совпадает только идея accent-цвета.
- Ghost-пилюля с label — наша замена ActivityDragGhost (элемент 47), в оригинале ghost — иконка, а не подпись.

## Дополнение атрибутов (цикл 10)

- гэпы: N/A: гэпы — и ghost, и drop-placeholder'ы это одиночные боксы без внутренних рядов; расстояния задают контейнеры (`.list` gap 2 в activity_bar.rs, стрип gap 4 в slot_panel.rs)
- цвета: ghost — фон = непрозрачная смесь p.accent_primary #89b4fa 22% над p.bg_surface #3d3f51, border 1px p.accent_primary #89b4fa α .5, глиф p.accent_primary #89b4fa, shadow 0 4 14 rgba(0,0,0,.35), opacity .92 (root.rs:6482-6528); drop-placeholder бара — border 1px dashed p.accent_primary α .7, bg p.accent_primary α .14 (activity_bar.rs:141-155); drop-placeholder стрипа — те же α .7 / α .14 (slot_panel.rs:126-136); подсветки самой карты-приёмника и `blocked` (p.accent_red #f38ba8) нет
- ОШИБКА В ours.md: ghost описан как «пилюля с label, px SPACE_3 / py 4, fs 12, bg accent_primary α 0.85 / 0.45, текст accent_action_fg» — в коде ghost это квадрат 28×28 с одним глифом (без подписи), rounded 8, фон-смесь accent 22% + bg_surface, opacity .92, вид от наличия цели НЕ зависит (root.rs:6470-6530)
