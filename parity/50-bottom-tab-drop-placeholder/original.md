# 50 bottom-tab-drop-placeholder — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/BottomTabBar.tsx:87-89`, `BottomTabBar.module.css` (`.dropPlaceholder`)

## JSX-структура (кратко, вложенность)
```
<span class="dropPlaceholder" aria-hidden="true"/>
```
- Вставляется в `.tabs` на `overIndex` (или в конец при `overIndex === pinned.length`), когда `dragState.overSlot === slot`.

## Метрики (ИЗ CSS, точные значения)
`.dropPlaceholder`:
- `display: inline-block`
- `width: 36px; height: 24px` (высота = высоте таба)
- `border-radius: var(--radius-sm)`
- `border: 1px dashed color-mix(in srgb, var(--accent-primary) 70%, transparent)`
- `background: color-mix(in srgb, var(--accent-primary) 14%, transparent)`
- transition/анимаций нет; flex-item в `.tabs` (gap var(--space-1))

## Состояния (классы-варианты с метриками)
Одно состояние; появляется/исчезает вставкой в DOM во время drag.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.dropPlaceholder` НЕТ (`activity-bar/BottomTabBar.module.css:73-80`) — бокс 36×24 (`:75-76`), рамка 1px dashed (`:78`); внешние зазоры даёт контейнер `.tabs { gap: var(--space-1) }` = 4 (`:17`); сама полоса `.strip` добавляет по краям padding 4px var(--space-2) = 4/8 (`:10`)
