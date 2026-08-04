# 119 status-editor-encoding-eol — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (55-64), `StatusBar.module.css` (23-45)

## JSX-структура (кратко, вложенность)
```
selectedFile отсутствует → null (не рендерится)
иначе:
├─ Item label="UTF-8" title="Encoding"
└─ (eol) Item label="LF"|"CRLF" title="End of line"
```
- Item = встроенный item №117 (button.item, tabIndex=-1, data-tooltip).
- Кодировка всегда "UTF-8" (host читает/пишет UTF-8); EOL — реактивно из активной Monaco-модели (`activeEditorEol`), обновляется при смене файла, null → строка EOL скрыта.

## Метрики (ИЗ CSS, точные значения)
Использует `.item` без tone-классов:
- display: flex; align-items: center; gap: 4px
- padding: 0 var(--space-2)
- color: var(--text-muted); border-radius: var(--radius-xs); font-size: var(--fs-xs)

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- Отображается ТОЛЬКО при активном текстовом редакторе (VS Code-парити).
