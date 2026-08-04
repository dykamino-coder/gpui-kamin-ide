# 140 sample-chips-kbd-code-badge — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (блок «Chips · Kbd · Code · Badge»)

## Структура/содержание
Ряд flex-wrap: три чипа (active/idle/error), kbd, inline-code, badge.

## Метрики (из кода, точные)
- Чип: px SPACE_2 8, py 1, radius RADIUS_XS 4, fs FS_XS 11; фон — цвет чипа при alpha 0.14, рамка 1px того же цвета при alpha 0.30, текст — сам цвет. active = accent-green #a6e3a1, error = accent-red #f38ba8.
- `idle` (muted-вариант): фон text-muted #838aa0 при alpha 0.12, рамка при 0.25.
- kbd: JetBrains Mono, fs 11, text-secondary #adb3c7, фон bg-overlay #515567 при alpha 0.5, px 6, py 2, radius 4, рамка bg-surface #3d3f51 при alpha 0.7.
- code: JetBrains Mono, fs 11, accent-primary #89b4fa, фон accent-primary при alpha 0.10, px 6, py 1, radius 4.
- badge: min-w 18, h 18, px 6, radius 9, fs 11, weight 600, фон accent-red #f38ba8, текст bg-primary #313240.
- Ряд: gap SPACE_2 8.

## Отличия от original.md той же папки
Ховера у элементов ряда нет — в оригинале его тоже нет.
