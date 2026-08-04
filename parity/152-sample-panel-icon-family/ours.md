# 152 sample-panel-icon-family — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_panel_icons`), `crates/shell/src/ui/panel_placeholder.rs` (`slot_glyph_small`, `enum SlotIcon`)

## Структура/содержание
Восемь подписанных иконок слотов в порядке оригинала: left, main, main-bottom, center, center-bottom, right, right-top, right-bottom. Каждая — рамка-канва с подсвеченным слотом плюс подпись под ней.

## Метрики (из кода, точные)
- Ряд: flex-wrap, gap SPACE_3 12.
- Ячейка: flex-col, items-center, gap 4, цвет text-secondary #adb3c7.
- Подпись: fs FS_XS 11, text-muted #838aa0.
- Иконка: канва 14×12, рамка rect 12×10 rx 1.5 штрих 1.2, подсвеченный бар — text-muted при alpha 0.85, инсет слота 1.5 (`SLOT_INSET` оригинала).

## Отличия от original.md той же папки
1. Иконка рисуется нативными div-барами, а не SVG (в gpui нет inline-SVG с произвольной геометрией) — форма выверена по `PanelIcon.tsx`.
2. `left` и `main` в оригинале дают одну и ту же фигуру — у нас обе подписи используют один вариант `SlotIcon::Main`; в перечислении есть ещё legacy-вариант `Bottom`, в витрине не показанный.
3. Подпись у нас — обычный текст, в оригинале `<code class=codeInline>` моно 10px.

## Дополнение атрибутов (цикл 10)

- скругления: у бара-подсветки слота radius 1×scale, у самой рамки-канвы — rx 1.5 штриха (`crates/shell/src/ui/panel_placeholder.rs`, `fn glyph`); внешнего скругления у ячейки нет
- ховер: N/A: ховер
