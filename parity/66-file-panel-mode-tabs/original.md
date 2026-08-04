# 66 file-panel-mode-tabs — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanelModeTabs.tsx (строки 10-29), kamin-ide/src/renderer/components/file-panel/FilePanelModeTabs.module.css

## JSX-структура (кратко, вложенность)
```
div.switcher [role="tablist"] [aria-label="File panel mode"]
├─ button.tab.left(.active при mode="files") [role="tab"] [aria-selected]
│  ├─ i.codicon.codicon-files [aria-hidden]
│  └─ span "Files"
└─ button.tab.right(.active при mode="web") [role="tab"] [aria-selected]
   ├─ i.codicon.codicon-globe [aria-hidden]
   └─ span "Web"
```
Клик переключает `filePanelMode` ("files" | "web", персистится).

## Метрики (ИЗ CSS, точные значения)
### .switcher
- display: inline-flex; flex-shrink: 0

### .tab
- display: inline-flex; align-items: center; gap: 5px
- height: 24px; padding: 0 10px
- border: 1px solid var(--divider-soft)
- background: var(--bg-surface)
- color: var(--text-secondary)
- font: inherit; font-size: var(--fs-sm)
- cursor: pointer

### .left (склейка в центре)
- border-radius: var(--radius-md) 0 0 var(--radius-md)
- border-right: none (шов без двойного бордера)

### .right
- border-radius: 0 var(--radius-md) var(--radius-md) 0

## Состояния (классы-варианты с метриками)
- `.tab:hover`: color var(--text-primary)
- `.active, .active:hover` (рецепт выбранной строки file/tree):
  - background: `linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent))`
  - border-color: `color-mix(in srgb, var(--accent-primary) 45%, transparent)`
  - color: var(--text-primary)
- transition не объявлен
