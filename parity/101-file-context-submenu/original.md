# 101 file-context-submenu — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileContextMenu.tsx:147-157`, `kamin-ide/src/renderer/components/file-tree/FileContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
{sub && createPortal(document.body):
  div.menu [role=menu, tabIndex=-1]
    (style: left/top из clampToViewport(anchor = rect родительской .hasSub строки, side: "right", offset: 2); visibility hidden→visible;
     onMouseEnter → cancelSubClose; onMouseLeave → scheduleSubClose)
  └── sub.action.children.map → leaf(a, inSub=true):
      button.item[.danger] [role=menuitem] (onMouseEnter → cancelSubClose)
      ├── i.fas.{a.icon}.itemIcon
      └── span.label {a.label}
}
```
Каскад «Open In ▸»: открывается hover'ом по `.hasSub` в root-меню; закрытие с grace-задержкой SUB_CLOSE_DELAY_MS = 250мс (диагональный проход курсора через соседние строки не убивает submenu); вход в submenu или возврат на родителя отменяет таймер. Клик по leaf: a.run() + закрытие всего меню.

## Метрики (ИЗ CSS, точные значения)
Использует те же классы, что 100 (тот же модуль):
- `.menu`: position fixed; z-index var(--z-dropdown); min-width 180px; background var(--bg-surface); border 1px solid var(--divider-soft); border-radius var(--radius-md); box-shadow var(--shadow-dropdown); padding var(--space-1); flex column; gap 1px; max-height calc(100vh - 16px); max-width calc(100vw - 16px); overflow-y auto
- `.item`: flex; gap var(--space-2); padding var(--space-2) var(--space-3); border-radius var(--radius-sm); color var(--text-primary); font-size var(--fs-sm)
- `.itemIcon`: width 16px; font-size 12px; color var(--text-muted)
- `.label`: flex 1; white-space nowrap
- Позиционирование: справа от anchor-строки, offset 2px, кламп во viewport.

## Состояния (классы-варианты с метриками)
- `.item:hover`: background color-mix(in srgb, var(--text-primary) 10%, transparent)
- `.danger` / `.danger:hover` — как в 100
- visibility: hidden до измерения, потом visible; таймер закрытия 250мс.
