# 37 customize-contributed-tree — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\CustomizeMode.tsx` (42-72), `CustomizeMode.module.css`

## JSX-структура (кратко, вложенность)
```
<>
  <li>
    <button .item [.active при childActive] aria-expanded={open}
            onClick={toggle open; если ни одна страница не открыта → открыть views[0]}>
      <span .codicon.codicon-chevron-right .chevron [.chevronOpen при open]/>
      <NavIcon icon={container.icon}/>
      <span>{container.title}</span>
    </button>
  </li>
  {open && views.map(<NavItem … icon={v.icon ?? "circle-small"} child/>)}   ← элемент 36 с .child
</>
```
`childActive = views.some(v.id === active)` — родитель подсвечен `.active`, когда открыта любая его страница. Дефолт `open = true`.

## Метрики (ИЗ CSS, точные значения)
- Родительская строка — те же `.item`/`.active`, что у элемента 36:
  - `.item`: `display: flex; align-items: center; gap: var(--space-2); width: 100%; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: transparent; color: var(--text-secondary); font-size: var(--fs-md); text-align: left`
  - `.item .codicon`: `font-size: 14px !important`
- `.chevron` (ведёт перед иконкой):
  - `flex: 0 0 auto`
  - `font-size: 12px !important`
  - `color: var(--text-muted)`
  - `transition: transform 120ms ease`
- Дочерние строки: `.child` → `padding-left: calc(var(--space-3) + 18px)`

## Состояния (классы-варианты с метриками)
- `.chevronOpen`: `transform: rotate(90deg)` (иконка всегда `codicon-chevron-right`, поворот через CSS)
- `.item:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.active, .active:hover` (childActive): `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)`
- Свёрнут (`!open`) — дочерние `NavItem` не рендерятся.
