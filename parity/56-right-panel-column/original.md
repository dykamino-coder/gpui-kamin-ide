# 56 right-panel-column — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 102-110), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.column [aria-label="Right activity column"]  ref=columnRef
  style = fill ? { flex: "1 1 0", minWidth: RIGHT_PANEL_MIN_WIDTH_PX }
               : { width: rightPanelWidth px, minWidth: RIGHT_PANEL_MIN_WIDTH_PX }
├─ div.resizeHandle (только !fill; элемент 57)
├─ div.cardWithBar (topPct)      — элемент 58
├─ div.splitHandle (bottomShown) — элемент 59
└─ div.cardWithBar (bottomPct, bottomShown) — элемент 60
```
Рендер null при `!rightPanelVisible`. RIGHT_PANEL_MIN_WIDTH_PX = 100 (config/constants.ts:51). topPct = bottomShown ? split*100% : "100%"; bottomPct = (1-split)*100% (toFixed(2)); split клампится [0.15, 0.85].

## Метрики (ИЗ CSS, точные значения)
### .column
- display: flex; flex-direction: column
- flex-shrink: 1 (сжимается до min-width при тесноте)
- min-height: 0; position: relative
- фона нет — гейт-фон просвечивает между двумя карточками
- width / min-width / flex — инлайн (см. выше)

## Состояния (классы-варианты с метриками)
- fill=true: `flex: 1 1 0` вместо фикс-ширины; width-handle не рендерится
- rightPanelVisible=false → null
- bottomShown=false: только верхняя карточка, height 100%
- hover/transition собственных нет

## Дополнение атрибутов (цикл 10)

- цвета: собственного фона у `.column` нет (`RightPanel.module.css:4-12` — только flex/min-height/position), сквозь зазор между картами просвечивает подложка `.appWrapper`: `var(--bg-sidebar)` = #1d1d28 dark (`dark-theme.css:13`) / #f4f1ea light (`light-theme.css:26`) плюс два radial-слоя (accent-purple 8% / accent-primary 6%, `AppLayout.module.css:12-14`).
- отступы: у `.column` и `.cardWithBar` padding/margin нет (`RightPanel.module.css:4-22`); всё межпанельное расстояние даёт родитель `.body { gap: var(--space-2) 8px; padding: 0 var(--space-1) 4px }` (`AppLayout.module.css:31,37`); ширину/минимум колонка получает инлайн (width = rightPanelWidth, min-width 100).
