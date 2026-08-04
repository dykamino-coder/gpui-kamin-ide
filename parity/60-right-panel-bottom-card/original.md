# 60 right-panel-bottom-card — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 166-184), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.cardWithBar  style={ height: bottomPct }  onDragOver/onDragLeave/onDrop
├─ aside.card [aria-label="Right Bottom"] [data-activity-slot="rightBottom"]
│    [data-activity-drop=over|blocked|undefined]
│  └─ bottomActive ? <ActiveTool slot="rightBottom" /> (→ ActivityBody)
│                 : <PanelPlaceholder label="Right Bottom" slot="right-bottom" activitySlot="rightBottom" />
└─ <ActivityBar slot="rightBottom" align="bottom" />   (зеркальный: пикер сверху)
```
Рендерится только при `rightPanelBottomVisible`. bottomPct = ((1 - rightPanelSplit)*100).toFixed(2)%.

## Метрики (ИЗ CSS, точные значения)
Идентичны верхней карточке (те же классы .cardWithBar / .card):
### .cardWithBar
- display: flex; flex-direction: row; min-height: 0; height — инлайн-процент
- `.cardWithBar > aside.card { flex: 1; min-width: 0; }`

### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

## Состояния (классы-варианты с метриками)
- data-activity-drop="over"/"blocked" — глобальные (theme/global.css:53-67): accent-tint 10% + dashed outline 60% / red-tint 12% + inset box-shadow 2px red 60%
- Отличие от top-card: ActivityBar с `align="bottom"` (зеркальная раскладка бара), aria-label "Right Bottom"
