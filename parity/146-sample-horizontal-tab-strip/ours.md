# 146 sample-horizontal-tab-strip — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_tab_strip`, блок «Horizontal tab strip»)

## Структура/содержание
Стрип-рецепт `BottomTabBar`: три пилюли — Terminal (активна), Problems, Output; иконка + подпись.

## Метрики (из кода, точные)
- Стрип: flex, items-center, gap SPACE_1 4, w-full, max-w 360, px SPACE_2 8, py 4, radius RADIUS_SM 8.
- Таб: h 24, px 10, gap 6, radius RADIUS_SM 8, fs 11, weight 500, цвет text-secondary #adb3c7; глиф codicon 13.
- Активный таб: фон accent-primary #89b4fa при alpha 0.16, текст text-primary #cfd4e2.

## Отличия от original.md той же папки
1. Семпл статичный: активная вкладка не переключается кликом.
2. Иконки — codicon вместо Phosphor-ассетов `ToolIcon` живого стрипа.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер
