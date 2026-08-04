# 65 file-panel-bottom-card — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 143-155), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.card.bottomCardWithTabs [aria-label="Bottom card"]
  style={ height: `${filePanelBottomHeight}px`, flexShrink: 0 }  — ФИКС-высота в px
  [data-activity-slot="centralBottom"] [data-activity-drop=over|blocked|undefined]
  onDragOver/onDragLeave/onDrop
├─ <BottomTabBar slot="centralBottom" />
└─ <BottomCardBody/>: activeId ? <ActivityBody id slot="centralBottom" />
     : <PanelPlaceholder label="Central Bottom" slot="center-bottom" activitySlot="centralBottom" />
```
Рендерится только при `filePanelBottomVisible`. Высота — пиксели из filePanelBottomHeight (мин 100, BOTTOM_PANE_MIN_HEIGHT_PX).

## Метрики (ИЗ CSS, точные значения)
### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### .bottomCardWithTabs
- display: flex; flex-direction: column (BottomTabBar фикс-высоты сверху, тело flex)

## Состояния (классы-варианты с метриками)
- data-activity-drop="over": background `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; outline `1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent)`; outline-offset -2px
- data-activity-drop="blocked": background `color-mix(in srgb, var(--accent-red) 12%, transparent)`; box-shadow `inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent)`
