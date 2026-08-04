# 20 sidebar-root — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\Sidebar.tsx` (52-63), `Sidebar.module.css`

## JSX-структура (кратко, вложенность)
```
<aside .sidebar aria-label="Primary side bar"
       data-activity-slot="sidebar"
       data-activity-drop={blocked→"blocked" | over→"over" | undefined}
       style={width: sidebarWidth px; min-width: SIDEBAR_MIN_WIDTH_PX px}
       onDragOver/onDragLeave/onDrop (drop-target активностей)>
  {mode === "customize" ? <CustomizeMode/> : <SidebarBody/>}
  <div .resizeHandle …/>   ← элемент 21
</aside>
```
Не рендерится вовсе (`return null`), если `!sidebarVisible && mode !== "customize"` — customize-режим пинит сайдбар видимым.

## Метрики (ИЗ CSS, точные значения)
- `.sidebar`:
  - `background: transparent` (радиальный градиент appWrapper просвечивает)
  - `display: flex; flex-direction: column`
  - `flex-shrink: 1` (ужимается до inline min-width вместо выталкивания соседей)
  - `min-height: 0`
  - `position: relative`
  - ширина — inline: `width: ${sidebarWidth}px`, `min-width: ${SIDEBAR_MIN_WIDTH_PX}px` (константа из `config/constants.js`)
- padding/margin/border/border-radius: нет (все 0/none)
- шрифт: наследуется
- hover/active/focus: нет собственных
- transition/анимации: нет
- z-index: нет

## Состояния (классы-варианты с метриками)
- `data-activity-drop="over"` / `"blocked"` — подсветка задаётся глобально в `theme/global.css` (элемент 157 инвентаря), не в этом модуле.
- Режимы `sessions` / `customize` — переключают тело, сам `<aside>` не меняется.

## Дополнение атрибутов (цикл 10)

- цвета: `.sidebar { background: transparent }` (`sidebar/Sidebar.module.css:5`), color не задаётся — наследуется от `.body`; фон под сайдбаром — радиальный backdrop приложения (комментарий `Sidebar.module.css:1-3`). Ближайшие hex — у ручки: `.resizeHandleBar` градиент transparent → var(--bg-overlay) #515567 30..70% → transparent (`Sidebar.module.css:39-45`), hover/active → var(--tint-primary-strong) = color-mix(accent-blue #89b4fa 25%, transparent) (`Sidebar.module.css:52-60`; токен `theme/variables.css:128` → `:110` = color-mix(accent-blue 25%, transparent))
