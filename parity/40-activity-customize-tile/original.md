# 40 activity-customize-tile — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:131-148`, `ActivityBar.module.css` (`.btn`, `.btnActive`)

## JSX-структура (кратко, вложенность)
```
<button type="button"
        class="btn [btnActive]"
        aria-pressed={isActive}          // isActive = sidebarMode === "customize"
        aria-label="Customize"
        data-tooltip="Customize"
        onClick={isActive ? leaveCustomize() : openCustomize("settings")}>
  <ToolIcon icon="gear"/>                // встроенный Phosphor-токен "gear"
</button>
```
- Рендерится ТОЛЬКО в sidebar-баре (`fixedHead`), первым элементом при `align="top"`, последним при reverse.
- Не в `pinned[]`: нельзя перетащить, скрыть, переместить. Нет onPointerDown/onContextMenu — обычный onClick.
- Без обёртки `<li>` — кнопка прямо в `<nav>` (вне `.list`).

## Метрики (ИЗ CSS, точные значения)
Тот же `.btn`, что у элемента 39:
- `width: 32px; height: 32px; display: grid; place-items: center`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-muted); font: inherit; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`
- SVG-иконка (ToolIcon default): 18×18, `fill="currentColor"`

## Состояния (классы-варианты с метриками)
- hover: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.btnActive` (customize открыт): `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)`
- Пока customize активен, остальные плитки бара НЕ подсвечиваются (`customizeOwnsBar`).

## Дополнение атрибутов (цикл 10)

- отступы: та же `.btn`-правило — padding/margin нет, бокс 32×32, центровка grid/place-items (`activity-bar/ActivityBar.module.css:53-58`). Отличие от обычной плитки: gear стоит ПРЯМЫМ ребёнком `.bar` (`ActivityBar.tsx:131-148`), а не внутри `.list`, поэтому зазор до списка задаёт `.bar { gap: var(--space-2) }` = 8 (`ActivityBar.module.css:9`), а не 2px `.list`-гэп; верхний отступ до края колонки — `.bar { padding: var(--space-3) 0 }` = 12 (`:10`)
