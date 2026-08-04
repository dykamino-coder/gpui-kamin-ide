# 27 project-inactive-toggle — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (91-98), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
<button .inactiveToggle [+ .inactiveOpen при открытом] onClick={toggle showInactive}>
  <i .codicon.codicon-chevron-{down|right} aria-hidden/>
  {inactive.length} inactive session{s}          ← «1 inactive session» / «N inactive sessions»
</button>
```
Chevron: `codicon-chevron-down` при открытом, `codicon-chevron-right` при закрытом.

## Метрики (ИЗ CSS, точные значения)
- `.inactiveToggle`:
  - `display: flex; align-items: center; gap: 6px; width: 100%`
  - `padding: 3px 8px 3px 18px` (top/bottom 3 / right 8 / left 18)
  - `background: transparent; border: none`
  - `color: var(--text-disabled)`
  - `font: inherit; font-size: var(--fs-sm); text-align: left`
  - `cursor: pointer`
- `.inactiveToggle .codicon` (`:global`): `font-size: 12px`
- `.inactiveOpen`: класс вешается, но в CSS-модуле отдельных правил для него НЕТ (визуальное различие — только chevron-иконка из JSX).

## Состояния (классы-варианты с метриками)
- `.inactiveToggle:hover`: `color: var(--text-secondary)` (только цвет, фона нет)
- Открыт (`showInactive`) → chevron down + ниже рендерятся `SessionItem` инактивных.
