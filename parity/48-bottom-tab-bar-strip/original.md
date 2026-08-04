# 48 bottom-tab-bar-strip — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/BottomTabBar.tsx:70-84`, `BottomTabBar.module.css` (`.strip`, `.tabs`, `.pickerSlot`)

## JSX-структура (кратко, вложенность)
```
<div class="strip"
     data-activity-strip="1"
     data-activity-slot={slot}                 // main | mainBottom | centralBottom
     data-activity-orientation="horizontal">
  <div class="tabs" role="tablist" aria-label="{slot} tabs">
    {tabs}                                     // элемент 49 + плейсхолдеры (50)
  </div>
  <div class="pickerSlot">
    <ActivityPicker slot={slot} popDirection="up"/>   // «…» dots, элемент 42
  </div>
</div>
```
- Те же данные, что у вертикального ActivityBar; drop владеет карточка-приёмник (`useActivityDropTarget`), стрип — только drag-start и контекст-меню.
- Рецепт портирован из Bridge `FileViewerTabs.tsx`.

## Метрики (ИЗ CSS, точные значения)
`.strip`:
- `display: flex; align-items: center; gap: var(--space-1)`
- `flex-shrink: 0`
- `padding: 4px var(--space-2)` (вертикаль 4px, горизонталь var(--space-2))
- `border-radius: var(--radius-sm)`
- фон: не задан (прозрачный)

`.tabs`:
- `display: flex; align-items: center; gap: var(--space-1)`
- `flex: 1; min-width: 0`
- `overflow-x: auto; scrollbar-width: none` (скрытый скроллбар)

`.pickerSlot`:
- `flex-shrink: 0; display: flex; align-items: center; margin-left: auto` (пикер прижат к правому краю)

## Состояния (классы-варианты с метриками)
Вариантов у самого стрипа нет; состояния несут табы (49) и плейсхолдер (50).

## Дополнение атрибутов (цикл 10)

- цвета: `.strip` ни background, ни color НЕ задаёт (`activity-bar/BottomTabBar.module.css:5-12`) — прозрачная полоса поверх фона карты. Hex — у детей: `.tab` color var(--text-secondary) #adb3c7 (`:33`), hover bg color-mix(var(--bg-surface) #3d3f51 50%, transparent) + color var(--text-primary) #cfd4e2 (`:43-44`), `.tabActive` bg color-mix(var(--accent-primary) #89b4fa 16%, transparent) + color #cfd4e2 (`:65-66`), `.dropPlaceholder` border 1px dashed accent-primary #89b4fa 70% + bg accent-primary 14% (`:78-79`), `.tabDragging { opacity: 0.3 }` (`:69`)
