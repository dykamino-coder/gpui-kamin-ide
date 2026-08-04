# 147 sample-vertical-icon-column — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_icon_column`, блок «Vertical icon column»)

## Структура/содержание
Рецепт `ActivityBar`: колонка из трёх квадратных плиток (первая активна) и «…»-пикер под ними.

## Метрики (из кода, точные)
- Бар: ширина `ACTIVITY_BAR_WIDTH` 48, flex-col, items-center, gap SPACE_2 8, py SPACE_3 12.
- Список плиток: flex-col, items-center, gap 2.
- Плитка: 32×32, radius RADIUS_SM 8, глиф codicon 18, цвет text-muted #838aa0.
- Активная плитка: фон accent-primary при alpha 0.16, текст text-primary #cfd4e2.
- Пикер «…»: та же плитка 32×32, глиф 18, text-muted.

## Отличия от original.md той же папки
1. Семпл статичный: активная плитка не переключается кликом.
2. Тултипов у плиток нет (в оригинале `data-tooltip` на каждой).

## Дополнение атрибутов (цикл 10)

- шрифты: текстовых узлов нет — только глифы codicon 18 в плитках и в «…»-пикере (`crates/shell/src/ui/design_panel.rs`, `fn sample_icon_column`); кегль наследуется от панели FS_MD 13, но на отрисовку не влияет
- ховер: N/A: ховер
