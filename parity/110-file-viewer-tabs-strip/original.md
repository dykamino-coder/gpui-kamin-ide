# 110 file-viewer-tabs-strip — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewerTabs.tsx` (155-165, 196-199), `FileViewerTabs.module.css` (7-25, 156-169)

## JSX-структура (кратко, вложенность)
```
div.bar                                     (стрип + условная overflow-кнопка, №112)
├─ div.strip [ref] role=tablist aria-label="Open files" tabIndex=-1
│  │  onPointerMove / onPointerUp (pointer-reorder, НЕ HTML5 drag)
│  ├─ button.tab × N (№111, сортировка pinned-first, стабильный порядок)
│  └─ span.dropIndicator style="left: <x>px" [aria-hidden]   (только во время drag, over >= 0)
└─ (overflow && …) div.overflow (№112)
```
- Порог драга: `DRAG_THRESHOLD_PX = 4`; полусдвиг индикатора `GAP_HALF_PX = 2`.
- Overflow детектится по `scrollWidth > clientWidth + 1` (каждый рендер + ResizeObserver).
- `tabs.length === 0` → компонент возвращает null (стрипа нет).

## Метрики (ИЗ CSS, точные значения)
`.bar`: display: flex; align-items: center; flex-shrink: 0

`.strip`:
- position: relative; display: flex; align-items: center
- gap: var(--space-1)
- padding: 4px var(--space-2) (симметрично по вертикали)
- flex: 1; min-width: 0
- overflow: hidden; scrollbar-width: none; `::-webkit-scrollbar { display: none }`

`.dropIndicator`:
- position: absolute; top: 5px; bottom: 5px
- width: 2px; border-radius: 1px
- background: var(--accent-primary)
- pointer-events: none
- left задаётся inline в px

## Состояния (классы-варианты с метриками)
- Drag активен: перетаскиваемый таб получает `.tabDragging` (opacity: 0.3, см. №111), индикатор вставки показан.
- Overflow: появляется `.overflow`-блок с кнопкой ▾ (№112).
- hover/transition на самом стрипе отсутствуют.
