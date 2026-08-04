# 107 contributed-tree-node-icon — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:189-197` (NodeIcon), CSS: `file-tree/FileTreeView.module.css` (`.icon`); при resourceUri — `file-tree/TreeIcon.tsx` + `TreeIcon.module.css`

## JSX-структура (кратко, вложенность)
Три взаимоисключающие ветки (приоритет сверху вниз):
```
1. node.codicon (ThemeIcon)   → i.codicon.codicon-{node.codicon}.icon (aria-hidden)
2. node.resourceUri           → <TreeIcon className={icon} name={basename(resourceUri)}
                                  type={collapsibleState === 0 ? "file" : "dir"} expanded />
                                  // = img.img.icon (см. элемент 99)
3. иначе (generic)            → i.codicon.{collapsibleState === 0 ? "codicon-circle-outline" : "codicon-folder"}.icon
```
basename: `resourceUri.split(/[\\/]/).pop() ?? ""`.

## Метрики (ИЗ CSS, точные значения)
`.icon` (FileTreeView.module.css):
- flex-shrink: 0; width: 16px; height: 16px
- цвета для codicon-веток не переопределяются классом `.icon` (наследование от строки: обычно var(--text-secondary), hover/selected var(--text-primary))

Для ветки TreeIcon дополнительно `.img` (TreeIcon.module.css):
- display: block; light-тема: filter: saturate(3.2) brightness(0.7)

## Состояния (классы-варианты с метриками)
- Вариантных классов нет; иконка меняется по данным узла (codicon / resourceUri / generic) и по expanded (open/closed глиф папки в ветке TreeIcon).

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding/margin у `.icon` нет — только фиксированный бокс `width: 16px; height: 16px; flex-shrink: 0` (`FileTreeView.module.css:131-135`); зазор до лейбла даёт строка `.row { gap: 6px }` (`:65`), правый край — `.row { padding-right: 8px }` (`:68`), отступ уровня — `indentPx(depth) = depth*12 + 8`; чекбокс перед иконкой добавляет `margin-right: 4px` (`:111`).
