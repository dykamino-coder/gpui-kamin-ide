# 153 sample-placeholders — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_placeholders`), ui/panel_placeholder.rs (`activity_placeholder`)

## Структура/содержание
Карточка-обёртка вокруг `activity_placeholder("terminal", "Terminal", p)`: глиф 36 text-disabled + заголовок + «Nothing to show here yet.».

## Метрики (из кода, точные)
- Обёртка: w-full, max-w 280, min-h 160, radius RADIUS_MD 12, bg `--bg-mantle`, flex-col.
- Плейсхолдер: gap SPACE_2 8, padding SPACE_5 20, центровка; глиф 36 text-disabled; label fs FS_MD 13 / 600 text-primary; hint fs FS_XS 11 text-muted, lh 1.3, max-w 240.

## Отличия от original.md той же папки
Нет.

## Дополнение атрибутов (цикл 10)

- шрифты: label font-size 13 (FS_MD) / font-weight 600 SEMIBOLD (panel_placeholder.rs:179-180); hint font-size 11 (FS_XS), line-height 14.3 = 11×1.3, font-weight 400 (panel_placeholder.rs:186-188); глиф — svg/codicon 36px (panel_placeholder.rs:149)
