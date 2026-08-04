# 112 file-viewer-tabs-overflow — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewerTabs.tsx` (200-232), `FileViewerTabs.module.css` (28-81)

## JSX-структура (кратко, вложенность)
```
div.overflow [ref]                          (рендерится только когда scrollWidth > clientWidth + 1)
├─ button.overflowBtn aria-label="Open files menu" data-tooltip="More open files" aria-expanded
│  └─ i.codicon.codicon-chevron-down
└─ (menuOpen) div.overflowMenu role=menu
   └─ button.overflowItem [.overflowItemActive] role=menuitem title={путь} × N
      ├─ (pinned) i.codicon.codicon-pinned.pinIcon
      ├─ <TabIcon>.tabIcon
      ├─ span.overflowLabel
      └─ (dirty) span.dirty "●"
```
- Клик по item → выбрать таб + scrollIntoView в стрипе; закрытие по mousedown вне / Escape.

## Метрики (ИЗ CSS, точные значения)
`.overflow`: position: relative; flex-shrink: 0; padding-right: var(--space-1)

`.overflowBtn`:
- inline-flex центр; width: 24px; height: 24px
- border: none; border-radius: var(--radius-sm)
- background: transparent; color: var(--text-secondary); cursor: pointer

`.overflowMenu`:
- position: absolute; top: calc(100% + 2px); right: 0; z-index: 30
- min-width: 200px; max-width: 360px; max-height: 60vh; overflow-y: auto
- padding: var(--space-1)
- border-radius: var(--radius-md)
- background: var(--bg-surface); border: 1px solid var(--divider-soft)
- box-shadow: 0 6px 24px rgb(0 0 0 / 30%)

`.overflowItem`:
- display: flex; align-items: center; gap: 6px; width: 100%
- padding: 5px 8px; border: none; border-radius: var(--radius-sm)
- background: transparent; color: var(--text-secondary)
- font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer

`.overflowLabel`: flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis

## Состояния (классы-варианты с метриками)
- `.overflowBtn:hover`: background: var(--bg-surface-hover); color: var(--text-primary)
- `.overflowItem:hover`: background: var(--bg-surface-hover); color: var(--text-primary)
- `.overflowItemActive`, `.overflowItemActive:hover`: background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)
