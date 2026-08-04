# 149 sample-context-menu — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_context_menu`)

## Структура/содержание
Статичное превью поверхности ActivityContextMenu: «Hide» (codicon-eye-closed) и «Move to» (codicon-arrow-right + chevron-right справа).

## Метрики (из кода, точные)
- `.menu`: min-w 180, bg `--bg-surface`, рамка 1px divider-soft (text-primary 6%), radius 12, padding 4, gap 1, без тени.
- `.item`: gap 8, w-full, px 12 / py 8, radius 8, fs FS_SM 12, text-primary; hover — text-primary 10%.
- `.chevron`: fs 12, text-muted.

## Отличия от original.md той же папки
Состояние `.itemMoveTo[aria-expanded=true]` (accent 16%) в статичном превью не показывается — как и в оригинале.

## Дополнение атрибутов (цикл 10)

- шрифты: `.item` font-size 12 (FS_SM), font-weight 400 (design_samples.rs:529); глиф пункта — codicon font-size 16 (база `.codicon`, design_samples.rs:534); chevron — codicon font-size 12 (FS_SM) (design_samples.rs:538)
