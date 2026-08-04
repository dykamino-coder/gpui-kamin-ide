# 02 titlebar-left-cluster — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:35-40
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:46-52

## JSX-структура (кратко, вложенность)
```
<div class=leftCluster style={width}>   // inline width, см. Состояния
  <div class=brand aria-hidden>         // элемент 03
    <img class=brandLogo .../>
  </div>
  <TitlebarQuickActions />              // элемент 08
</div>
```

## Метрики (ИЗ CSS)
- размеры: height: 100%; width — inline-style: `${sidebarWidth}px` при видимом сайдбаре, `auto` при скрытом
- отступы: нет padding/margin/gap
- скругления: нет
- шрифт: наследуется от .titlebar
- цвета: не заданы (наследуются)
- hover/active/focus: нет
- transition: нет
- позиционирование: display:flex; align-items:center; flex-shrink:0; overflow:hidden

## Состояния
- сайдбар видим (`sidebarVisible || sidebarMode === "customize"`): width = `sidebarWidth.value`px (пиннится к ширине сайдбара)
- сайдбар скрыт: width: auto
Других классов-вариантов нет.

## Дополнение атрибутов (цикл 10)

- цвета: `.leftCluster` своих background/color НЕ задаёт (`titlebar/Titlebar.module.css:46-52`) — прозрачный, наследует от `.titlebar`: background transparent, color var(--text-muted) #838aa0 (`Titlebar.module.css:9,15`); единственный собственный цвет внутри — `.brand { color: var(--accent-primary) }` = #89b4fa (`Titlebar.module.css:26`)
