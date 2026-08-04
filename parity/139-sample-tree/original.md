# 139 sample-tree — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:15-38,145-167, design-sections.module.css:376-383; сам Tree — components/tree/Tree.tsx (описан в зоне FileTree)

## Содержание/структура
`TreeRow()` в Block «Tree (file-explorer pattern)»: живой рекурсивный компонент `<Tree>` в рамке `.treeFrame`.
Данные SAMPLE_TREE: `src/` (dir) → `host/` (index.ts 13 KB, layout-store.ts 2.5 KB, json-file-store.ts 1.8 KB), `exthost/` (api.ts 3.0 KB, loader.ts 8.2 KB); корневые файлы package.json (1.2 KB, icon "json"), README.md (4.1 KB, icon "markdown"). У файлов `meta` = размер.
State: expanded = Set{"src","src/host"}, selected = "src/host/index.ts"; onToggle — переключение папки, onSelect — выбор ноды.

## Метрики
`.treeFrame`:
- width 100%; max-width 380px
- padding `--space-2` (8px)
- border `1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- border-radius `--radius-sm` (8px)
- background `--bg-base` (#313240)

## Состояния/варианты
Интерактивный образец: expand/collapse папок, выделение ноды. Стили строк — из самого Tree (не этого модуля).
