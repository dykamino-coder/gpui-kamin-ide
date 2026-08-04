# 137 sample-input — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:84-97, design-sections.module.css:237-250

## Содержание/структура
`InputRow()` в Block «Input»: контролируемый `<input type=text class=input placeholder="Sample input">` (useState + onInput).

## Метрики
`.input`:
- width 100%; max-width 360px
- padding `var(--space-2) var(--space-3)` = 8px 12px
- border `1px solid var(--bg-surface)` (#3d3f51); border-radius `--radius-sm` (8px)
- background `--bg-base` (#313240); color `--text-primary`
- font inherit; font-size `--fs-md` (13px)
- outline none; transition `border-color var(--transition-fast)` (150ms ease)

## Состояния/варианты
- `:focus` → border-color `--accent-primary` (#89b4fa dark)
