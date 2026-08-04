# 17 panel-icon-svg — наша реализация

Файлы: crates/shell/src/ui/panel_placeholder.rs:12-80 (SlotIcon, glyph,
slot_glyph 2.8×, slot_glyph_small 1.0×)

## Структура (gpui-дерево кратко)
```
frame div (relative, 14s×12s, rounded 1.5s, border_1 text_muted)
 └ bar div (absolute, rounded 1s, bg text_muted α0.85)   // подсвеченный слот
```
Не SVG — нативные div (рамка + залитый прямоугольник), масштаб параметром
(placeholder 2.8, layout-меню 1.0).

## Метрики (из кода, точные)
- база: W=14, H=12; frame radius 1.5·s; slot radius 1.0·s
- геометрия слотов (x, y, w, h) при s=1:
  Main 1.5,1.5,4.5,9 · MainBottom 1.5,6,4.5,4.5 · Center 4.75,1.5,4.5,9 ·
  CenterBottom 4.75,7,4.5,3.5 · Right 8,1.5,4.5,9 · RightTop 8,1.5,4.5,4.5 ·
  RightBottom 8,6,4.5,4.5
- цвета: рамка border p.text_muted (#838aa0); highlight p.text_muted α0.85
- hover/active: нет

## Отличия от original.md той же папки
1. Вариантов 7 из 9: НЕТ `bottom` (fallback 1.5,7,11,3.5) и отдельного `left`
   (у оригинала left ≡ main геометрически — покрыто Main; bottom
   НЕ РЕАЛИЗОВАН).
2. Рамка: border 1px (gpui border_1) вместо stroke-width 1.2; и рамка
   рисуется по краю бокса 14×12 — у оригинала rect с инсетом 1
   (x=1,y=1,w=12,h=10), т.е. наша рамка на 1px «шире» по каждой стороне.
3. Цвет: захардкожен text_muted (+α0.85 у слота) вместо currentColor —
   иконка не перекрашивается с контейнером (в hover/active кнопок оригинал
   светлеет вместе с текстом).
4. Масштабирование параметром s (оригинал фикс 14×12; наш slot_glyph 2.8×
   для плейсхолдеров — расширение, не расхождение в титлбаре).

## Дополнение атрибутов (цикл 10)

- гэпы: N/A: гэпы — иконка рисуется абсолютными барами внутри рамки-канвы 14×12 (`crates/shell/src/ui/panel_placeholder.rs`, `fn glyph`), flex-детей нет, gap разделять нечего
