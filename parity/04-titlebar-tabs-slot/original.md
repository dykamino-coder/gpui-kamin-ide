# 04 titlebar-tabs-slot — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:41
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:57-64

## JSX-структура (кратко, вложенность)
```
<div class=tabsSlot aria-label="Open sessions">
  <SessionTabs />   // элемент 18; при 0 сессий SessionTabs возвращает null — слот пуст
</div>
```

## Метрики (ИЗ CSS)
- размеры: не заданы; flex: 1; min-width: 0
- отступы: padding: 0 var(--space-3)
- скругления: нет
- шрифт: наследуется
- цвета: не заданы
- hover/active/focus: нет
- transition: нет
- позиционирование: display:flex; align-items:center; -webkit-app-region: no-drag

## Состояния
Нет вариантных классов.

## Дополнение атрибутов (цикл 10)

- цвета: `.tabsSlot` своих background/color НЕ задаёт (`titlebar/Titlebar.module.css:57-64`) — прозрачный, наследует color var(--text-muted) #838aa0 и background transparent от `.titlebar` (`Titlebar.module.css:9,15`)
