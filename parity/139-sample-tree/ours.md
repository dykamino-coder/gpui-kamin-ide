# 139 sample-tree — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_tree`, блок «Tree (file-explorer pattern)»)

## Структура/содержание
Четыре строки-рецепта файлового дерева: `src` (папка, раскрыта), `components` (папка, свёрнута), `App.tsx` (выделен), `main.tsx`. Колонка без собственного скролла, `w-full`, `max-w 280`.
```
row (flex, items-center, gap 6, h 22, border 1px transparent, radius-xs)
├─ бокс шеврона 16 (глиф 13, text-muted; у листа — пустой спейсер)
├─ иконка 16 (папка/файл codicon)
└─ имя (fs-sm)
```

## Метрики (из кода, точные)
- Строка: h 22, gap 6, `padding-left = depth*12 + 8`, pr SPACE_2 8, radius RADIUS_XS 4, рамка 1px transparent (резерв под выделение), fs FS_SM 12, цвет text-secondary #adb3c7.
- Шеврон: бокс 16, глиф 13, text-muted #838aa0.
- Иконка: codicon 16.
- Выделенная строка: линейный градиент 90° accent-primary 26% → 14%, рамка accent-primary 45%, текст text-primary #cfd4e2.
- Ховер невыделенной: bg-surface #3d3f51 при alpha 0.55 + text-primary.

## Отличия от original.md той же папки
1. Семпл статичный: раскрытие и выбор не переключаются кликом (в оригинале — `useState` на expanded/selected).
2. Иконки — codicon-папка/файл, а не иконочная тема Catppuccin, как в живом дереве оригинала.
3. Hint у блока убран — в оригинале у `TreeRow` его нет.
