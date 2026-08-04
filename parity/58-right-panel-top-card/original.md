# 58 right-panel-top-card — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 133-151), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.cardWithBar  style={ height: topPct }  onDragOver/onDragLeave/onDrop
├─ aside.card [aria-label="Right"] [data-activity-slot="rightTop"]
│    [data-activity-drop=over|blocked|undefined]
│  └─ topActive ? <ActiveTool slot="rightTop" /> (→ ActivityBody)
│              : <PanelPlaceholder label="Right" slot="right-top" activitySlot="rightTop" />
└─ <ActivityBar slot="rightTop" align="top" />
```
topPct = bottomShown ? (rightPanelSplit*100).toFixed(2)% : "100%".

## Метрики (ИЗ CSS, точные значения)
### .cardWithBar (обёртка card + activity bar)
- display: flex; flex-direction: row; min-height: 0
- height — инлайн-процент
- `.cardWithBar > aside.card { flex: 1; min-width: 0; }`

### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### Прочие классы модуля (используются телами карточек)
- .cardHeader: padding 8px 12px; text-transform uppercase; font-size var(--fs-xs); font-weight 500; letter-spacing 0.08em; color var(--text-muted)
- .empty: flex 1; flex-direction column; align-items center; justify-content center; gap var(--space-1); padding var(--space-4); color var(--text-muted); text-align center
- .empty > i: font-size 24px; opacity 0.4; margin-bottom var(--space-1)
- .empty > p: margin 0; font-size var(--fs-sm)

## Состояния (классы-варианты с метриками)
- data-activity-drop="over": background `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; outline `1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent)`; outline-offset -2px (theme/global.css:53)
- data-activity-drop="blocked": background `color-mix(in srgb, var(--accent-red) 12%, transparent)`; box-shadow `inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent)` (theme/global.css:63)
- bottomShown=false → height 100%
