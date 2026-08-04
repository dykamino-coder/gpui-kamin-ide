# 97 file-tree-row-badge — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/file-tree-helpers.tsx:62-65` (RowBadge), `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
{deco?.badge ? (
  span.badge
    (style.color = decorationColor(deco.color);  // ThemeColor → css-цвет
     data-tooltip = deco.tooltip)
  {deco.badge}   // короткая строка-статус: git "M"/"U" и т.п.
) : null}
```
Данные — из FileDecorationProvider через `useFileDecoration(path)` (hostRpc.fileDecorations.get; ре-запрос по path-scoped tick или глобальной версии).

## Метрики (ИЗ CSS, точные значения)
`.badge`:
- flex-shrink: 0
- margin-left: auto (прижат к правому краю строки)
- padding-left: 6px
- font-size: var(--fs-xs); font-weight: 600
- color — инлайн из `decorationColor(deco.color)` (ThemeColor decoration'а); background/border нет

## Состояния (классы-варианты с метриками)
- Вариантов нет; цвет полностью определяется decoration. При отсутствии `deco.badge` элемент не рендерится.

## Дополнение атрибутов (цикл 10)

- цвета: `.badge` собственного цвета не задаёт (`FileTreeView.module.css:147-153`) — красится из `decorationColor(id)` (`signals/file-decorations.ts:41-60`): modified → `--accent-orange` #fab387 dark (`dark-theme.css:49`) / #da8343 light (`light-theme.css:65`); untracked/added/stageModified → `--accent-green` #a6e3a1 / #5e9855 (`:45`, `:61`); deleted/conflicting → `--accent-red` #f38ba8 / #ca3939 (`:43`, `:59`); ignored → `--text-disabled` #60667b / #938e82 (`:38`, `:48`); submodule и фоллбэк неизвестного id → `--accent-blue` #89b4fa / #3b6fc4 (`:41`, `:57`); list.warning/problemsWarning → `--accent-yellow` #f9e2af / #c89a3f (`:46`, `:62`). Без ThemeColor цвет наследуется от строки — `--text-secondary` #adb3c7 / #524c43 (`FileTreeView.module.css:75`).
