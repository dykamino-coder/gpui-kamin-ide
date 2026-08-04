# 23 sessions-mode-root — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionsMode.tsx` (9-29), `SessionsMode.module.css`

## JSX-структура (кратко, вложенность)
```
<div .root>
  <div .actions>
    <button .action data-tooltip="Start without a folder">
      <i .fas.fa-circle-plus aria-hidden/> No folder session
    <button .action data-tooltip="Pick a folder, then start a session">
      <i .fas.fa-circle-plus aria-hidden/> New session
  <div .header><span>PROJECTS</span></div>
  <div .list>
    groups.length === 0 ? <p .empty>No projects yet. Open a folder or start a session.</p>
                        : groups.map(<ProjectGroup/>)
</div>
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: `display: flex; flex-direction: column; flex: 1; min-height: 0; padding-top: var(--space-2)`
- `.actions`: `display: flex; flex-direction: column; padding: 4px 8px 8px` (top 4 / lr 8 / bottom 8)
- `.action`:
  - `display: flex; align-items: center; gap: 10px; width: 100%`
  - `padding: 6px 8px`
  - `background: transparent; border: none; border-radius: var(--radius-sm)`
  - `color: var(--text-secondary)`
  - `font: inherit; font-size: var(--fs-md); text-align: left; white-space: nowrap`
  - `cursor: pointer`
  - `transition: background var(--transition-fast), color var(--transition-fast)`
- `.action > i`: `width: 20px; text-align: center; font-size: var(--fs-lg); color: var(--text-muted)`
- `.header`:
  - `padding: 8px 8px 8px 12px` (left 12 — инсет как у FileTreeHeader)
  - `font-size: var(--fs-xs); font-weight: 500; letter-spacing: 0.08em`
  - `text-transform: uppercase; font-feature-settings: "ss01"`
  - `color: var(--text-muted); flex-shrink: 0`
- `.list`: `flex: 1; min-height: 0; overflow: auto; padding: 0 var(--space-1) var(--space-2)` (top 0 / lr space-1 / bottom space-2)
- `.empty`: `margin: 0; padding: var(--space-3) var(--space-3) var(--space-3) 12px; color: var(--text-muted); font-size: var(--fs-sm)`

## Состояния (классы-варианты с метриками)
- `.action:hover`: `background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)`
- `.action:hover > i`: `color: var(--text-primary)`
- Пустой список групп → `.empty` абзац вместо `ProjectGroup`-ов.
