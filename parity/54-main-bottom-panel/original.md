# 54 main-bottom-panel — оригинал
Файлы: kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.tsx (строки 57-86), kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.module.css

## JSX-структура (кратко, вложенность)
```
section.panel [aria-label="Left Bottom"]  style={ height: `${heightPct}%` }
├─ div.resizeHandle [role=separator aria-orientation=horizontal] [data-tooltip="Drag to resize"]  (элемент 55)
│  └─ span.resizeHandleBar [aria-hidden]
└─ div.card [data-activity-slot="mainBottom"] [data-activity-drop=over|blocked|undefined]
   ├─ <BottomTabBar slot="mainBottom" />
   └─ <Body/>: activeId ? <ActivityBody id slot="mainBottom" />
                        : <PanelPlaceholder label="Left Bottom" slot="main-bottom" activitySlot="mainBottom" />
```
Рендер null при `!mainBottomVisible`. Высота: `mainVisible ? (1 - mainSplit)*100 : 100`%, `toFixed(2)`. mainSplit клампится в [MAIN_SPLIT_LOWER=0.2, MAIN_SPLIT_UPPER=0.85] (config/constants.ts:75-76).

## Метрики (ИЗ CSS, точные значения)
### .panel
- flex-shrink: 0; display: flex; flex-direction: column; position: relative
- фона нет (гейт-фон просвечивает); height — инлайн-процент

### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### Drop-индикация — глобальные `[data-activity-drop="over"/"blocked"]` (см. 53, theme/global.css:53-67)

## Состояния (классы-варианты с метриками)
- mainBottomVisible=false → null
- mainVisible=false → height 100% колонки
- data-activity-drop="over"/"blocked" на .card — глобальные метрики
- hover/transition собственных нет (у card)
