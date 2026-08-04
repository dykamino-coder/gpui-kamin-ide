# 61 file-panel-column — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 91-98), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
aside.filePanel [aria-label="File column"]
  style = fill ? { flex: "1 1 0", minWidth: FILE_PANEL_MIN_WIDTH_PX }
               : { width: filePanelWidth px, minWidth: FILE_PANEL_MIN_WIDTH_PX }
├─ div.resizeHandle (только !fill; элемент 62)
├─ div.card.topCard — элемент 63
├─ div.splitHandle (filePanelBottomVisible) — элемент 64
└─ div.card.bottomCardWithTabs (filePanelBottomVisible) — элемент 65
```
Рендер null при `!filePanelVisible`. FILE_PANEL_MIN_WIDTH_PX = 100 (config/constants.ts:46).

## Метрики (ИЗ CSS, точные значения)
### .filePanel
- display: flex; flex-direction: column
- flex-shrink: 1 (сжимается до min-width, не выталкивая правую панель)
- min-height: 0; position: relative
- фона нет — гейт-фон просвечивает в зазоре между карточками
- width / min-width / flex — инлайн (см. выше)

## Состояния (классы-варианты с метриками)
- fill=true: `flex: 1 1 0`, width-handle не рендерится
- filePanelVisible=false → null
- filePanelBottomVisible=false: только верхняя карточка (flex 1)
- hover/transition собственных нет

## Дополнение атрибутов (цикл 10)

- цвета: `.filePanel` фона не имеет (`FilePanel.module.css:4-12`) — между картами и по краям просвечивает `.appWrapper`: `var(--bg-sidebar)` #1d1d28 / #f4f1ea (`dark-theme.css:13`, `light-theme.css:26`) + radial accent-purple 8% / accent-primary 6% (`AppLayout.module.css:12-14`). Карты внутри — `.card` c `--glint-border` (rgba(255,255,255,.18) на углах) и заливкой `var(--bg-mantle)` #262533 / #fbf7f4 (`FilePanel.module.css:62-70`, `dark-theme.css:12,31`).
- отступы: у колонки padding/margin нет; горизонтальный ритм задаёт родитель `.body { gap: 8px; padding: 0 4px }` (`AppLayout.module.css:31,37`), вертикальный шов между картами — `.splitHandle { height: 10px }` (`FilePanel.module.css:97-106`); собственный padding есть только у `.modeHeader` — `6px 8px 0` (`FilePanel.module.css:83`).
