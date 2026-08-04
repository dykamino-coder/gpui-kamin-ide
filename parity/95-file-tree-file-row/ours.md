# 95 file-tree-file-row — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:171-327` (`rows()`, ветка `!is_dir`, drag 240-245), `crates/shell/src/ui/file_list.rs:71-98` (`DraggedFile`, `FileDragGhost`), `crates/shell/src/icon_theme.rs:119-126` (file_img)

## Структура (gpui-дерево кратко)
```
div #"{panel_key}:{path}"  (тот же контейнер, что 94)
    on_mouse_down(Left): Ctrl → select-toggle, иначе select + OpenFile(path)
    on_mouse_down(Right): OpenFileMenu(path, false, x, y)
    on_drag(DraggedFile{path}) → ghost: FileDragGhost (пилюля с именем:
        px SPACE_2, py 2, radius SM, bg_surface, border text_primary 15%, FS_XS)
├── chevron-спейсер: div 16×16 (пустой)
├── icon_theme::file_img(name) 16×16 .flex_shrink_0
├── label (flex_1, ellipsis, deco-цвет)
└── badge (элемент 97)
```

## Метрики (из кода, точные)
- Все метрики строки идентичны 94: gap 6, `pl depth*12+8`, pr 8, py 2, radius 4, hover `bg_surface` 55%, selected градиент accent 26%→14% + бордер 45%.
- Спейсер 16×16 (оригинал: width 16, font-size 13 — визуально эквивалент).
- Drag-ghost: `SPACE_2`/2px паддинги, `RADIUS_SM` 8, `bg_surface` #3d3f51, бордер `text_primary` 15%, текст `FS_XS` 11 `text_primary`.

## Отличия от original.md той же папки
1. Все пункты 1-4, 8-10 из 94 (высота py2 vs 22px, нет резервного бордера, нет text-secondary→primary, hover перекрывает selected, нет Shift-select/клавиатуры/тултипа/flash, mouse_down).
2. **Drag** — внутренний gpui `on_drag` (drop: редактор → открыть, терминал → путь) вместо нативного `beginNativeDrag`; в ОС файл унести нельзя. Ghost-пилюля — наша добавка (в оригинале нативный drag-image).
3. **Selected не синхронизирован с активным файлом редактора** — только клик/Ctrl-клик по дереву (оригинал: explorer-selection ← активный таб).
