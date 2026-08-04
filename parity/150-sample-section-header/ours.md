# 150 sample-section-header — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_section_header`, блок «Section header»)

## Структура/содержание
Ландмарк-заголовок сайдбара: одна строка «SECTION».

## Метрики (из кода, точные)
- px 12, py SPACE_2 8.
- fs FS_XS 11, weight 500 через `typo::ss01(MEDIUM)` (то же начертание, что у PROJECTS/CUSTOMIZE).
- Цвет text-muted #838aa0.
- Фона и скругления нет — прозрачная строка.

## Отличия от original.md той же папки
`letter-spacing: 0.08em` в gpui недоступен — единственное расхождение.
