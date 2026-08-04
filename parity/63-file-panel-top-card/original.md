# 63 file-panel-top-card — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 114-129), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.card.topCard [aria-label="File card"]
├─ div.modeHeader
│  └─ <FilePanelModeTabs />   (элемент 66)
└─ (filePanelMode === "web") → <BrowserPane />          (элемент 67)
   (selectedFile)            → <FileViewer />
   (иначе)                   → <PanelPlaceholder label="File" slot="center"
                                 hint="Click a file in any panel, or drag-and-drop one from outside" />
```
Drop-target нет (верхняя карточка — editor-поверхность, без пикера и без drops).

## Метрики (ИЗ CSS, точные значения)
### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### .topCard
- flex: 1; min-height: 0 (занимает остаток высоты при открытой нижней карточке)

### .modeHeader
- display: flex; justify-content: flex-end; align-items: center
- padding: 6px 8px 0
- flex-shrink: 0

## Состояния (классы-варианты с метриками)
- Тело: web-режим / файл выбран / placeholder — переключается контентом, без классов-вариантов
- hover/active/transition собственных нет
