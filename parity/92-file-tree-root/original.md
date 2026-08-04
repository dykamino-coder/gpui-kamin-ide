# 92 file-tree-root — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:55-74`, `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root (+ optional className из пропа, конкатенация)
├── <FileTreeHeader />
└── div.body [data-file-tree] (onContextMenu: если e.target === e.currentTarget → openFileContextMenu корня {path: root, name: basename(root), type: "dir"})
    └── <FolderNode key={root} path={root} depth={0} initiallyExpanded />
```
- `key={root}` — смена workspace-папки полностью ремаунтит дерево.
- Right-click по пустой области (сами строки делают stopPropagation через собственные обработчики) = контекст-меню корневой папки.

## Метрики (ИЗ CSS, точные значения)
`.root`:
- flex: 1; display: flex; flex-direction: column; min-height: 0
- цвета/шрифт не задаёт (наследует)

`.body`:
- flex: 1; overflow: auto
- padding: 4px 6px 8px (top 4, право/лево 6, низ 8; горизонтальный inset чтобы скруглённый highlight строк не прилипал к краям панели)
- font-size: var(--fs-sm)
- background не задан (прозрачный)

## Состояния (классы-варианты с метриками)
- Вариантных классов нет; состояние «нет папки» — отдельный элемент 93 (`.empty`).

## Дополнение атрибутов (цикл 10)

- цвета: собственных фонов у `.root`/`.body` нет (`FileTreeView.module.css:1-15`) — просвечивает карта-хозяин с `--bg-mantle` #262533 dark (`dark-theme.css:12`) / #fbf7f4 light (`light-theme.css:25`); цвет текста приходит от строк `.row { color: var(--text-secondary) }` #adb3c7 / #524c43 (`:75`; `dark-theme.css:36`, `light-theme.css:46`), у пустого состояния `.empty { color: var(--text-muted) }` #838aa0 / #6e685d (`:26`).
