# 100 file-context-menu — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileContextMenu.tsx:133-146` (компонент 21-161), `kamin-ide/src/renderer/components/file-tree/FileContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
div.menu [role=menu] (style: left/top px из clampToViewport(anchor=курсор, side:"bottom", offset:0); visibility hidden→visible после измерения)
└── items.flatMap:  // порядок: state.extra (tab-actions) → builtinActions → explorerContextItems
    ├── {смена a.group} → div.separator [role=separator]
    └── row(a):
        ├── без children → button.item[.danger] [role=menuitem]
        │   ├── i.fas.{a.icon}.itemIcon (aria-hidden; фикс. слот и без иконки)
        │   └── span.label {a.label}
        └── с children → button.item.hasSub [role=menuitem, aria-haspopup=menu]
            ├── i.fas.itemIcon
            ├── span.label
            └── i.codicon.codicon-chevron-right.chevron
```
Поведение: закрытие по outside-mousedown (capture) / Esc / scroll(capture); ре-открытие на новой позиции ресетит submenu; hover leaf-строки root-меню → scheduleSubClose (grace 250мс), hover `.hasSub` → открыть submenu (элемент 101). Иконки — FontAwesome (`fas`), chevron — codicon.

## Метрики (ИЗ CSS, точные значения)
`.menu`:
- position: fixed; z-index: var(--z-dropdown)
- min-width: 180px; max-height: calc(100vh - 16px); max-width: calc(100vw - 16px); overflow-y: auto
- background: var(--bg-surface); border: 1px solid var(--divider-soft)
- border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)
- margin: 0; padding: var(--space-1)
- display: flex; flex-direction: column; gap: 1px

`.item`:
- display: flex; align-items: center; gap: var(--space-2); width: 100%
- padding: var(--space-2) var(--space-3)
- background: transparent; border: none; border-radius: var(--radius-sm)
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer

`.itemIcon`:
- width: 16px; font-size: 12px; text-align: center; flex-shrink: 0; color: var(--text-muted)

`.label`: flex: 1; white-space: nowrap

`.hasSub`: position: relative

`.chevron`: font-size: 12px; color: var(--text-muted); margin-left: var(--space-2)

`.separator`: height: 1px; margin: var(--space-1) var(--space-2); background: var(--divider-soft)

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--text-primary) 10%, transparent)
- `.danger`: color: var(--accent-danger, #e5484d); `.danger .itemIcon`: color: inherit
- `.danger:hover`: background: color-mix(in srgb, var(--accent-danger, #e5484d) 16%, transparent)
- visibility: hidden до измерения bounding rect (двухпроходное позиционирование), затем visible.
