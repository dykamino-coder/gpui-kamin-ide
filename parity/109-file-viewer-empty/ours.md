# 109 file-viewer-empty — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:5352-5357 (вызов), crates\shell\src\ui\panel_placeholder.rs:37-89 (glyph), 92-94 (slot_glyph), 103-144 (шаблон)

## Структура/содержание
```
panel_placeholder("File",
  "Click a file in any panel, or drag-and-drop one from outside",
  SlotIcon::Center, p)
└─ div.size_full.flex_col.items_center.justify_center.gap(8).p(20).overflow_hidden
   ├─ div.mb(4) └─ slot_glyph(Center) = glyph(scale 2.0), канва 28×24
   │     ├─ frame_rect: absolute left 2 top 2, 24×20, border_2, rounded 3
   │     └─ bar (Center): absolute left 9.5 top 3, 9×18, rounded 2
   ├─ label «File»
   └─ hint (text_center; max-width НЕТ)
```
Глиф — не SVG, а нативные div: канва 14×12 в исходных координатах PanelIcon.tsx, умноженная на scale 2.0 (`slot_glyph`, panel_placeholder.rs:92-94). Бар кламплется во внутреннюю область рамки (SLOT_INSET 1.5). При s ≥ 2.0 рамка рисуется `border_2` (2px), при s = 1.0 — `border_1`.

## Метрики (из кода, точные)
- отступы: контейнер p 20 (SPACE_5); обёртка глифа mb 4 (SPACE_1); у label и hint собственных padding/margin нет
- гэпы: контейнер gap 8 (SPACE_2)
- цвета: базовый цвет контейнера p.text_muted #838aa0; label p.text_primary #cfd4e2; hint p.text_muted #838aa0; рамка глифа p.text_muted #838aa0 (α 1.0); слот-бар p.text_muted α 0.85
- скругления: рамка глифа 3.0 (1.5 × scale 2.0); слот-бар 2.0 (1.0 × 2.0); у контейнера и текстов скруглений нет
- шрифты: label font-size 16 (FS_LG), font-weight 600 SEMIBOLD; hint font-size 12 (FS_SM), line-height 15.6 (12 × 1.3), font-weight 400; семейство — UI «Bricolage Grotesque» (наследуется от окна); моно-шрифта нет
- ховер: N/A: ховер — одно статичное состояние, ни одного `.hover(...)` в panel_placeholder.rs (совпадает с оригиналом)

## Отличия от original.md той же папки
1. Другой глиф: слот-рамка PanelIcon (Center) 28×24 вместо `codicon-file` 36px цветом `--text-disabled`.
2. Другой текст: «Click a file in any panel, or drag-and-drop one from outside» вместо «Pick a file from the tree, or press Ctrl+P to open one by name.»; подсказки про Ctrl+P и `<kbd>`-чипа (padding 2×6, bg-surface, text-primary, radius-xs, font-mono fs-xs, border text-muted 30%) нет вовсе.
3. Добавлен заголовок «File» fs 16 semibold — в оригинале только глиф + `<p>`.
4. Совпадают: gap 8 (space-2), padding 20 (space-5), flex-column + центрирование по обеим осям, text-align center, базовый цвет text-muted.
5. max-width у hint нет ни у нас, ни в CSS оригинала — совпадает (240 есть только в соседнем `activity_placeholder`, panel_placeholder.rs:189).
6. Добавлен `overflow_hidden` у контейнера (в CSS оригинала его нет).
7. `flex: 1` у `.empty` → у нас `size_full` внутри уже растянутой карты.
