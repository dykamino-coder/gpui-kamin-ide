# 36 customize-nav-item — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\CustomizeMode.tsx` (18-37), `CustomizeMode.module.css`

## JSX-структура (кратко, вложенность)
```
<li>
  <button .item [.child][.active] aria-pressed={active}
          onClick={activeCustomizePanel.value = id}>
    <NavIcon/>          ← isImageIcon ? <img width=16 height=16 alt=""> : <span .codicon.codicon-{icon}>
    <span>{label}</span>
  </button>
</li>
```

## Метрики (ИЗ CSS, точные значения)
- `.item`:
  - `display: flex; align-items: center; gap: var(--space-2); width: 100%`
  - `padding: var(--space-2) var(--space-3)`
  - `border-radius: var(--radius-sm); background: transparent`
  - `color: var(--text-secondary)`
  - `font-size: var(--fs-md); text-align: left`
  - (border/cursor в модуле не заданы)
- `.item .codicon` (`:global`): `font-size: 14px !important`
- `<img>`-иконка: атрибуты `width=16 height=16`

## Состояния (классы-варианты с метриками)
- `.item:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.active, .active:hover`: `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` (faint accent tint, без fill; одинаково в light и dark)
- `.child` (вложенная страница контрибьютнутого контейнера): `padding-left: calc(var(--space-3) + 18px)`
