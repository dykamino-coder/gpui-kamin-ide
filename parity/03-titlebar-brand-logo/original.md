# 03 titlebar-brand-logo — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:36-38
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:18-40

## JSX-структура (кратко, вложенность)
```
<div class=brand aria-hidden="true">
  <img class=brandLogo src={kaminoid.svg} alt="" draggable={false} />
</div>
```

## Метрики (ИЗ CSS)
.brand:
- размеры: width: var(--layout-titlebar-height); height: var(--layout-titlebar-height) (квадрат = высоте титлбара)
- отступы: нет
- скругления: нет
- шрифт: `.brand :global(.codicon) { font-size: 18px !important; }` (запасной путь; в текущем JSX рендерится img, не codicon)
- цвета: color: var(--accent-primary)
- позиционирование: display:flex; align-items:center; justify-content:center; flex-shrink:0; -webkit-app-region: no-drag

.brandLogo:
- размеры: width: 26px; height: 26px
- object-fit: contain; display: block
- -webkit-user-drag: none; user-select: none
- hover/active/focus: нет
- transition: нет

## Состояния
Нет вариантных классов. Не интерактивен (aria-hidden, draggable=false).
