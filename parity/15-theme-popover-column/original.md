# 15 theme-popover-column — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:121-128
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:91-114

## JSX-структура (кратко, вложенность)
```
<div class=column>
  <div class=colTitle>{Dark|Light|Icons}</div>
  <div class=colList role=listbox aria-label={title}>
    <Item … />                            // элемент 16
    …contributed-темы/icon-темы
  </div>
</div>
```
Содержимое колонок: Dark → «Kamin Dark» + contributed dark; Light → «Kamin Light»
+ contributed light; Icons → «Catppuccin» + contributed iconThemes.

## Метрики (ИЗ CSS)
.column:
- размеры: min-width: 0 (ширина колонки от грида родителя: minmax(140px, 1fr))
- отступы: gap: var(--space-1) (flex-column)
- позиционирование: display:flex; flex-direction:column

.colTitle:
- padding: var(--space-1) var(--space-2)
- font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em
- color: var(--text-muted)

.colList:
- max-height: 320px; overflow-y: auto
- gap: 1px (flex-column)
- display:flex; flex-direction:column

## Состояния
Вариантных классов нет.
