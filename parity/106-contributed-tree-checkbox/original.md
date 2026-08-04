# 106 contributed-tree-checkbox — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:162-174`, CSS: `file-tree/FileTreeView.module.css` (`.treeCheckbox`)

## JSX-структура (кратко, вложенность)
```
{node.checkboxState !== undefined && (
  span.treeCheckbox [role=checkbox, aria-checked = (checkboxState === 1), tabIndex=0]
    (data-tooltip = node.checkboxTooltip — только если задан;
     onClick → toggleCheckbox (stopPropagation, reportCheckbox с инвертированным состоянием);
     onKeyDown: " " или "Enter" → preventDefault + toggle)
  └── {checked} → i.codicon.codicon-check (aria-hidden)   // unchecked — пустой бокс
)}
```
TreeItemCheckboxState: CHECKED=1, UNCHECKED=0. Тоггл независим от клика по строке; провайдер обновляет модель и re-fetch возвращает перевёрнутое состояние.

## Метрики (ИЗ CSS, точные значения)
`.treeCheckbox`:
- display: inline-flex; align-items: center; justify-content: center
- width: 14px; height: 14px; margin-right: 4px; flex-shrink: 0
- border: 1px solid var(--border-strong, currentColor)
- border-radius: 3px
- font-size: 11px (размер codicon-галки)
- cursor: pointer
- background не задан (прозрачный)

## Состояния (классы-варианты с метриками)
- checked: внутри рендерится codicon-check (11px); unchecked: пусто. CSS-вариантов (hover/checked-классов) нет — различие только контентом и aria-checked.
- Фокусируем (tabIndex=0), клавиатурный toggle Space/Enter.

## Дополнение атрибутов (цикл 10)

- цвета: заливки нет; рамка `border: 1px solid var(--border-strong, currentColor)` (`FileTreeView.module.css:114`) — токен `--border-strong` в темах НЕ объявлен (grep по `variables.css`/`dark-theme.css`/`light-theme.css` пуст), поэтому реально работает `currentColor` = цвет строки: `--text-secondary` #adb3c7 dark / #524c43 light в покое (`:75`; `dark-theme.css:36`, `light-theme.css:46`), `--text-primary` #cfd4e2 / #322e28 на hover и у выделенной строки (`:87,93`); галка-codicon тоже currentColor.
