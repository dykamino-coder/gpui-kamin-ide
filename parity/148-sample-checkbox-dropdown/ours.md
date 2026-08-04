# 148 sample-checkbox-dropdown — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_checkbox_dropdown`), root.rs (`DesignAction::ToggleCheck`)

## Структура/содержание
Статично встроенное меню (position static, без тени — как превью оригинала): «SAMPLE» + Option A/B/C со стартовыми true/false/true. Клик тогглит только свой пункт и НЕ закрывает меню.

## Метрики (из кода, точные)
- `.menu`: min-w 220, bg `--bg-surface` #3d3f51, рамка 1px divider-soft (text-primary 6%), radius RADIUS_MD 12, padding 4, gap 1.
- `.menuLabel`: px 12 / py 4, fs 11, uppercase, text-muted.
- `.menuItem`: gap 8, w-full, px 12 / py 8, radius 8, fs 12, text-primary; hover — text-primary 10%.
- `.check`: 16×16, radius 3, рамка 1px `--bg-overlay`; включённый — заливка и рамка accent-primary, галка codicon 12 цветом `--accent-action-fg`.

## Отличия от original.md той же папки
`letter-spacing .04em` у label недоступен в gpui. Скролл (`max-height: calc(100vh - 16px)`) статичному превью не нужен.

## Дополнение атрибутов (цикл 10)

- шрифты: `.menuLabel` font-size 11 (FS_XS), font-weight 400, текст через `to_uppercase()` (design_samples.rs:173-175); `.menuItem` font-size 12 (FS_SM), font-weight 400 (design_samples.rs:501); галка — codicon font-size 12 (FS_SM) (design_samples.rs:489)
