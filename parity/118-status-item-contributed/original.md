# 118 status-item-contributed — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (69-83), `StatusBar.module.css` (23-45)

## JSX-структура (кратко, вложенность)
```
button.item [.clickable] type=button disabled={!command}
  style={item.color ? { color } : undefined}
  [data-tooltip={tooltip} aria-label={tooltip}]
  onClick → hostRpc.commands.execute(command)
└─ renderCodiconText(item.text)     ($(icon) → codicon-спаны + текст; общий парсер с QuickPick)
```
- clickable = есть `item.command`; иначе `disabled`.
- `item.color` — произвольный цвет расширения, inline.

## Метрики (ИЗ CSS, точные значения)
`.item` (общее с №117):
- display: flex; align-items: center; gap: 4px
- padding: 0 var(--space-2)
- color: var(--text-muted); border-radius: var(--radius-xs); font-size: var(--fs-xs)
- `.item .codicon`: font-size: 12px !important

`.clickable`: cursor: pointer

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.item:disabled`: cursor: default
- `.item:disabled:hover`: background: transparent; color: var(--text-muted) (не реагирует на hover)
