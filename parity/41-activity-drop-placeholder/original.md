# 41 activity-drop-placeholder — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:150-152`, `ActivityBar.module.css` (`.dropPlaceholder`)

## JSX-структура (кратко, вложенность)
```
<li class="dropPlaceholder" aria-hidden="true"/>
```
- Вставляется в `<ul class="list">` на позицию `overIndex` (перед плиткой i, либо в конец при `overIndex === pinned.length`), когда `dragState.overSlot === slot`.
- Пустой элемент, без содержимого.

## Метрики (ИЗ CSS, точные значения)
`.dropPlaceholder`:
- `width: 32px; height: 32px` (повторяет форму живой плитки)
- `border-radius: var(--radius-sm)`
- `border: 1px dashed color-mix(in srgb, var(--accent-primary) 70%, transparent)`
- `background: color-mix(in srgb, var(--accent-primary) 14%, transparent)`
- transition/анимации: нет; позиционирование: обычный flex-item в `.list` (gap 2px)

## Состояния (классы-варианты с метриками)
Одно состояние; появляется/исчезает только через вставку/удаление из DOM во время drag.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.dropPlaceholder` НЕТ (`activity-bar/ActivityBar.module.css:23-29`) — бокс ровно 32×32 (`:24-25`) в размер живой плитки, рамка 1px dashed (`:27`); внешние зазоры даёт `.list { gap: 2px }` (`:45`); вставляется в `<ul class="list">` на позицию `overIndex` (`ActivityBar.tsx:150-152`)
