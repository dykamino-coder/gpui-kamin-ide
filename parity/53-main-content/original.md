# 53 main-content — оригинал
Файлы: kamin-ide/src/renderer/components/main/MainContent.tsx (строки 35-58), kamin-ide/src/renderer/components/main/MainContent.module.css

## JSX-структура (кратко, вложенность)
```
main.main [aria-label="Left"] [data-activity-slot="main"]
  style={ height: `${heightPct}%` }
  data-activity-drop = "blocked" | "over" | undefined  (drop-target хук)
  onDragOver / onDragLeave / onDrop
├─ (customize)  → <CustomizePanel />
├─ (noSessions) → <WelcomePlaceholder />
└─ (иначе)
   ├─ <BottomTabBar slot="main" />
   └─ activeId ? <ActivityBody id={activeId} slot="main" />
              : <PanelPlaceholder label="Left" slot="main" />
```
Высота: `customize || noSessions ? 100% : (mainBottomVisible ? mainSplit*100 : 100)%`, `toFixed(2)`.

## Метрики (ИЗ CSS, точные значения)
### .main
- `composes: glint-surface from global` (theme/global.css:96):
  - border: 1px solid transparent
  - background: `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- flex-shrink: 0; display: flex; flex-direction: column
- min-width: 0; min-height: 0; overflow: hidden
- margin: 0 (межпанельные отступы — от gap родителя)
- border-radius: var(--radius-lg)
- position: relative
- height — инлайн-стиль (проценты от mainSplit)

### Drop-индикация (глобально, theme/global.css:53-67)
- `[data-activity-drop="over"]`: background-color `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; outline `1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent)`; outline-offset: -2px; transition: background-color var(--transition-fast), outline-color var(--transition-fast)
- `[data-activity-drop="blocked"]`: background-color `color-mix(in srgb, var(--accent-red) 12%, transparent)`; box-shadow `inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent)`; transition: background-color var(--transition-fast), box-shadow var(--transition-fast)

## Состояния (классы-варианты с метриками)
- customize: тело = CustomizePanel, height 100%, без табов
- noSessions: тело = WelcomePlaceholder, height 100%
- нормальный: BottomTabBar + ActivityBody/PanelPlaceholder; height = mainSplit*100% при видимом Left Bottom, иначе 100%
- data-activity-drop="over"/"blocked" — метрики выше
- hover/focus: нет собственных
