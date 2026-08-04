# 38 activity-bar-nav — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:117-128`, `ActivityBar.module.css` (`.bar`, `.barReverse`)

## JSX-структура (кратко, вложенность)
```
<nav class="bar [barReverse]"
     aria-label="{slot} activities"
     data-activity-strip="1"
     data-activity-slot={slot}            // sidebar | rightTop | rightBottom …
     data-activity-orientation="vertical"
     data-activity-drop="blocked|over|undefined">   // от useActivityDropTarget
  // align="top" (default):    {fixedHead}{buttons}{picker}
  // align="bottom" (reverse): {picker}{buttons}{fixedHead}
  fixedHead = <CustomizeTile/>            // только slot === "sidebar" (элемент 40)
  buttons   = <ul class="list"> {tiles} </ul>   // элемент 39
  picker    = <ActivityPicker slot popDirection={reverse ? "down" : "up"}/>  // элемент 42
</nav>
```
- `slot === "sidebar" && !sidebarVisible` → возвращает `null` (бар исчезает целиком).
- `customizeOwnsBar` (sidebar + sidebarMode === "customize") → ни одна плитка не подсвечена активной.
- Сам `<nav>` — drop target (`useActivityDropTarget(slot)`), позиционная вставка между иконками.

## Метрики (ИЗ CSS, точные значения)
`.bar`:
- `display: flex; flex-direction: column; align-items: center`
- `gap: var(--space-2)`
- `padding: var(--space-3) 0` (top/bottom var(--space-3), left/right 0)
- `width: var(--layout-activity-bar-width, 44px)`
- `flex-shrink: 0`
- фон: НЕТ собственного (прозрачная колонка, просвечивает app-backdrop градиент)
- border/border-radius: нет; шрифт: не задаёт (иконки)

`.list` (внутренний `<ul>`):
- `list-style: none; margin: 0; padding: 0`
- `display: flex; flex-direction: column; gap: 2px`
- `width: 100%; align-items: center`

## Состояния (классы-варианты с метриками)
- `.barReverse` (align="bottom"): добавляет `justify-content: flex-end`; DOM-порядок в JSX перевёрнут на {picker, list, fixedHead} — пара «пикер+иконки» прижата к низу, пикер прямо НАД верхней иконкой.
- `data-activity-drop="over" | "blocked"` — атрибут ставится, стилей в этом css-модуле для него нет (подсветку рисует карточка-приёмник).

## Дополнение атрибутов (цикл 10)

- цвета: `.bar` ни background, ни color НЕ задаёт (`activity-bar/ActivityBar.module.css:5-13`) — прозрачная колонка, под ней видно градиент приложения (комментарий `:1-4`). Hex — у детей: `.btn`/`.picker` color var(--text-muted) #838aa0 (`ActivityBar.module.css:62`), hover bg color-mix(var(--bg-surface) #3d3f51 50%, transparent) + color var(--text-primary) #cfd4e2 (`:87-88`), active bg color-mix(var(--accent-primary) #89b4fa 16%, transparent) + color #cfd4e2 (`:95-96`), `.dropPlaceholder` border accent-primary 70% + bg accent-primary 14% (`:27-28`)
