# 24 project-group-header — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (43-52), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
<div .group>                        ← flex column, вся группа
  <div .header ref={headerRef} onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
    <button .headerMain onClick={toggle collapsed}>
      <i .codicon.codicon-chevron-{right|down} .chevron aria-hidden/>
      <TreeIcon .icon name={name} type="dir" expanded={!collapsed}/>
      <span .name data-tooltip={project.folderPath ?? "Sessions without a folder"}>{name}</span>
      <span .count>{total}</span>     ← active.length + inactive.length
```
Chevron: `codicon-chevron-right` при collapsed, `codicon-chevron-down` при раскрытом.

## Метрики (ИЗ CSS, точные значения)
- `.group`: `display: flex; flex-direction: column`
- `.header`: `display: flex; align-items: center; height: 26px`
- `.headerMain`:
  - `display: flex; align-items: center; gap: 6px; flex: 1; min-width: 0; height: 100%`
  - `padding: 0 4px 0 6px` (right 4 / left 6)
  - `background: transparent; border: none`
  - `color: var(--text-secondary)`
  - `text-align: left; font: inherit; font-size: var(--fs-sm); font-weight: 500`
  - `cursor: pointer; white-space: nowrap; overflow: hidden`
- `.chevron`: `flex-shrink: 0; font-size: 13px; width: 16px; text-align: center; color: var(--text-muted)`
- `.icon` (TreeIcon): `flex-shrink: 0; width: 16px; height: 16px`
- `.name`: `flex: 1; overflow: hidden; text-overflow: ellipsis`
- `.count` (бейдж-счётчик):
  - `flex-shrink: 0; min-width: 16px; height: 16px; padding: 0 5px`
  - `display: inline-flex; align-items: center; justify-content: center`
  - `border-radius: 9px`
  - `background: var(--bg-surface); color: var(--text-muted); font-size: var(--fs-xs)`

## Состояния (классы-варианты с метриками)
- `.headerMain:hover`: `color: var(--text-primary)` (только цвет текста, фон не меняется)
- collapsed → chevron `codicon-chevron-right`, `TreeIcon expanded=false`, `.sessions` не рендерится
- hover по `.header` → показывает портал-попап `actionsPop` (элемент 25)
