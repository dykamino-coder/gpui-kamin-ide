# 17 panel-icon-svg — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\PanelIcon.tsx:46-90 (css-модуля НЕТ, всё в атрибутах SVG)

## JSX-структура (кратко, вложенность)
```
<svg width=14 height=12 viewBox="0 0 14 12" aria-hidden>
  {highlight}   // залитый rect варианта slot (рисуется ПОД рамкой)
  {frame}       // рамка: rect x=1 y=1 w=12 h=10 rx=1.5 fill=none stroke=currentColor stroke-width=1.2
</svg>
```

## Метрики (ИЗ TSX-констант)
- размеры: W=14, H=12 (фикс, не масштабируется css)
- рамка: STROKE_INSET=1 (x=1,y=1,w=12,h=10), FRAME_RADIUS=1.5, STROKE_WIDTH=1.2, stroke=currentColor, fill=none
- highlight: SLOT_RADIUS=1 (rx/ry), fill=currentColor, opacity=0.85 (HIGHLIGHT_OPACITY), SLOT_INSET=1.5
- ширины подсветок: LEFT/RIGHT/CENTER_HIGHLIGHT_W=4.5; RIGHT_HIGHLIGHT_INSET=6 → правый x=8
- нижняя полоса: BOTTOM_HIGHLIGHT_INSET_Y=5; половины правой колонки: RIGHT_QUARTER_HEIGHT=(12−3)/2=4.5, низ y=1.5+4.5=6
- цвета: только currentColor (наследует цвет контейнера); токенов нет

## Состояния (9 вариантов slot — геометрия highlight-rect)
| slot | x | y | w | h |
|---|---|---|---|---|
| main | 1.5 | 1.5 | 4.5 | 9 |
| left | 1.5 | 1.5 | 4.5 | 9 |
| right | 8 | 1.5 | 4.5 | 9 |
| right-top | 8 | 1.5 | 4.5 | 4.5 |
| right-bottom | 8 | 6 | 4.5 | 4.5 |
| center | 4.75 | 1.5 | 4.5 | 9 |
| center-bottom | 4.75 | 7 | 4.5 | 3.5 |
| main-bottom | 1.5 | 6 | 4.5 | 4.5 |
| bottom (fallback else) | 1.5 | 7 | 11 | 3.5 |

`main` и `main-bottom` — горизонтальные зеркала `right`/`right-bottom` (одинаковая
геометрия у left/main). hover/transition/позиционирование — нет (чистый inline SVG).

## Дополнение атрибутов (цикл 10)

- цвета: собственных hex нет — stroke рамки и fill подсветки = `currentColor` (`titlebar/PanelIcon.tsx:56,66-82`), подсветка с opacity 0.85 (`PanelIcon.tsx:26`). Фактический цвет даёт родитель: в меню Layout — `.itemIcon { color: var(--text-muted) }` = #838aa0 (`titlebar/LayoutToggles.module.css:113-119`, disabled → opacity 0.4, `:89`); в плейсхолдере панели — `.glyph { color: var(--text-muted) }` = #838aa0 (`panel-placeholder/PanelPlaceholder.module.css`, блок `.glyph`)
- отступы: CSS-модуля у компонента нет — ни padding, ни margin; «отступы» это SVG-инсеты внутри канвы 14×12: STROKE_INSET 1 (рамка `rect x1 y1 w12 h10`), SLOT_INSET 1.5 (границы подсветки), RIGHT_HIGHLIGHT_INSET 6 → RIGHT_HIGHLIGHT_X = 8, BOTTOM_HIGHLIGHT_INSET_Y 5 (`PanelIcon.tsx:19-20,24-25,34,38-39,48-58`); внешний зазор до label даёт `.menuItem { gap: var(--space-2) }` = 8 (`LayoutToggles.module.css:67`)
