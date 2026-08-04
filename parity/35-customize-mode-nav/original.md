# 35 customize-mode-nav — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\CustomizeMode.tsx` (79-97), `CustomizeMode.module.css`

## JSX-структура (кратко, вложенность)
```
<div .root>
  <header .header>
    <span .title>CUSTOMIZE</span>
  </header>
  <ul .list>
    PANELS.map(<NavItem/>)                 ← 5 встроенных: Settings(settings-gear),
                                             Design(symbol-color), Extensions(extensions),
                                             Logs(output), System(pulse)
    containers.map(<ContributedTree/>)     ← контрибьютнутые customize-контейнеры (элемент 37)
  </ul>
</div>
```
Контейнеры: `registry.viewContainers.filter(location === "customize")`.

## Метрики (ИЗ CSS, точные значения)
- `.root`: `display: flex; flex-direction: column; padding: var(--space-3) 0; gap: var(--space-2)`
- `.header`: `padding: 8px 12px; display: flex; align-items: center`
- `.title`:
  - `font-size: var(--fs-xs); font-weight: 500; letter-spacing: 0.08em`
  - `color: var(--text-muted); font-feature-settings: "ss01"`
  - (текст «CUSTOMIZE» — uppercase литералом, `text-transform` не задан; рецепт хедера совпадает с PROJECTS из SessionsMode)
- `.list`:
  - `list-style: none; margin: 0; padding: 0 var(--space-2)`
  - `display: flex; flex-direction: column; gap: 2px`

## Состояния (классы-варианты с метриками)
- Собственных состояний у контейнера нет; строки — элементы 36/37.
