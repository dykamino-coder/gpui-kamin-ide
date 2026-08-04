# 26 project-sessions-list — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (85-103), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
{!collapsed && (
  <div .sessions>
    active.map(<SessionItem/>)                     ← активные сессии
    {total === 0 && <p .empty>No sessions yet.</p>}
    {inactive.length > 0 && (
      <button .inactiveToggle …/>                  ← элемент 27
      {showInactive && inactive.map(<SessionItem/>)}
    )}
  </div>
)}
```

## Метрики (ИЗ CSS, точные значения)
- `.sessions`: `display: flex; flex-direction: column; gap: 2px`
- `.empty`:
  - `margin: 0; padding: 2px 0 2px 18px` (top/bottom 2 / left 18)
  - `font-size: var(--fs-xs); color: var(--text-muted)`

## Состояния (классы-варианты с метриками)
- `collapsed` (по клику на header группы) → весь `.sessions` не рендерится.
- `total === 0` → абзац `.empty` «No sessions yet.»
- Инактивные сессии видны только при `showInactive` (см. элемент 27).
