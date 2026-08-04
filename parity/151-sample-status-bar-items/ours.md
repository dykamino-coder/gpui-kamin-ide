# 151 sample-status-bar-items — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_status_items`, блок «Status-bar items»)

## Структура/содержание
Четыре элемента статус-бара: «3 active» (ok), «2 failed» (warn), «UTF-8» (нейтральный), «KaminIDE 0.0.1» (brand).

## Метрики (из кода, точные)
- Элемент: flex, items-center, gap 4, px SPACE_2 8, radius RADIUS_XS 4, fs 11; глиф codicon 12.
- ok: accent-green #a6e3a1; warn: accent-yellow #f9e2af; нейтральный: text-muted #838aa0.
- brand: accent-primary #89b4fa, weight 500.

## Отличия от original.md той же папки
Ховера у семпла нет (в живом статус-баре элемент подсвечивается) — семпл статичный.
