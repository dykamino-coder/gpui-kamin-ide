# 113 monaco-editor-host — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/MonacoEditor.tsx` (168-349, рендер 345-348), `MonacoEditor.module.css`; опции создания редактора — `MonacoEditor.tsx:183-208` (+ `monaco-loader.ts`)

## JSX-структура (кратко, вложенность)
```
error ? div.error "Failed to open: {error}"
      : div.host [ref]        (Monaco монтируется в него императивно)
```

## Метрики (ИЗ CSS, точные значения)
`.host`: flex: 1; min-height: 0

`.host .monaco-editor .scrollbar .slider` (:global):
- border-radius: var(--radius-xs) (только геометрия; цвета — через темы scrollbarSlider.*)

`.error`:
- flex: 1; display: flex; align-items: center; justify-content: center
- padding: var(--space-5)
- color: var(--accent-red)
- font-family: var(--font-mono); font-size: var(--fs-sm)

## Опции Monaco (из TSX — определяют вид редактора)
- automaticLayout: true; scrollBeyondLastLine: false; smoothScrolling: true
- stickyScroll: { enabled: true }; minimap: { enabled: true }
- scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 }
- fixedOverflowWidgets: true (hover/suggest в body-слое, не клипаются карточкой)
- largeFileOptimizations: true; maxTokenizationLineLength: 20000
- fontFamily: getComputedStyle(documentElement) `--font-mono` || "monospace"
- fontSize: 13

## Состояния (классы-варианты с метриками)
- `.error`: показывается вместо `.host` при неудачном чтении файла.
- hover/transition в CSS-модуле отсутствуют (всё внутри Monaco).
