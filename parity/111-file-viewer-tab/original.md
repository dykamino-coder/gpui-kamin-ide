# 111 file-viewer-tab — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewerTabs.tsx` (166-195), `FileViewerTabs.module.css` (83-173)

## JSX-структура (кратко, вложенность)
```
button.tab [.tabActive] [.tabDragging] role=tab data-tab-path aria-selected data-tooltip={полный путь}
├─ (pinned) i.codicon.codicon-pinned.pinIcon
├─ <TabIcon>.tabIcon      (webview → иконка расширения-владельца / файл → TreeIcon)
├─ span.label             (basename / live-title webview-панели)
├─ (dirty) span.dirty "●" aria-label="Unsaved changes"
└─ button.close aria-label="Close" data-tooltip="Close"|"Discard & close"
   └─ i.codicon.codicon-close
```
- pointerdown (левая) → press-bookkeeping; выбор — на pointerup стрипа (клик без сдвига ≥4px = select).
- middle-click (`onAuxClick` button===1) закрывает; right-click — контекст-меню (Close / Close Others / Close to the Right / Close All + файловое меню, для webview `builtin:false`).

## Метрики (ИЗ CSS, точные значения)
`.tab`:
- display: inline-flex; align-items: center; gap: 6px
- padding: 4px 6px 4px 10px; height: 24px
- background: transparent; border: none
- border-radius: var(--radius-sm)
- color: var(--text-secondary)
- font-size: 11px; font-weight: 500; letter-spacing: 0.02em
- white-space: nowrap; cursor: pointer; flex-shrink: 0
- transition: background var(--transition-fast), color var(--transition-fast)

`.tabIcon`: flex-shrink: 0; width: 14px; height: 14px
`.label`: white-space: nowrap (без усечения)
`.dirty`: color: var(--accent-orange); font-size: 10px; line-height: 1
`.pinIcon`: font-size: 11px; opacity: 0.7

`.close`:
- width: 16px; height: 16px; inline-flex центр; padding: 0
- background: transparent; border: none; border-radius: var(--radius-xs)
- color: inherit; opacity: 0
- transition: opacity var(--transition-fast), background var(--transition-fast)
- `.close .codicon`: font-size: 11px

## Состояния (классы-варианты с метриками)
- `.tab:hover`: background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)
- `.tabActive`, `.tabActive:hover`: background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary) — без рамки/кольца
- `.tabDragging`: opacity: 0.3
- `.tab:hover .close`, `.tabActive .close`: opacity: 0.7
- `.close:hover`: opacity: 1; background: color-mix(in srgb, var(--bg-overlay) 60%, transparent)
