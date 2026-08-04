# 06 titlebar-window-controls-cluster — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:54-85
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:119-125

## JSX-структура (кратко, вложенность)
```
<div class=controls>
  <TitlebarButton iconSet="fas"     icon="fa-bug"        variant="devtools" label="DevTools" />
  <TitlebarButton iconSet="codicon" icon="chrome-minimize"                  label="Minimize" />
  <TitlebarButton iconSet="codicon" icon={maximized ? "chrome-restore" : "chrome-maximize"}
                                    label={maximized ? "Restore" : "Maximize"} />
  <TitlebarButton iconSet="codicon" icon="chrome-close"  variant="close"    label="Close" />
</div>
```
(метрики кнопок — элемент 07)

## Метрики (ИЗ CSS)
.controls:
- размеры: height: 100%
- отступы: padding-right: var(--space-1); gap не задан (кнопки несут собственный margin 0 var(--space-1))
- скругления: нет
- шрифт: наследуется
- цвета: не заданы
- hover/active/focus: нет (на контейнере)
- transition: нет
- позиционирование: display:flex; align-items:center; -webkit-app-region: no-drag

## Состояния
- maximize-кнопка: иконка `chrome-maximize` ↔ `chrome-restore` по сигналу `isWindowMaximized` (label Maximize ↔ Restore). Стили кластера не меняются.

## Дополнение атрибутов (цикл 10)

- цвета: `.controls` своих background/color НЕ задаёт (`titlebar/Titlebar.module.css:119-125`) — прозрачный, наследует text-muted #838aa0 от `.titlebar`; цвета несут кнопки `TitlebarButton.module.css`: покой color var(--text-muted) #838aa0 (`TitlebarButton.module.css:10`), hover bg var(--bg-surface) #3d3f51 + color var(--text-primary) #cfd4e2 (`TitlebarButton.module.css:25-28`), `.close:hover` bg var(--accent-red) #f38ba8 + color var(--bg-primary) #313240 (`TitlebarButton.module.css:45-48`), `.devtools:hover` color var(--accent-primary) #89b4fa (`TitlebarButton.module.css:37-39`)
