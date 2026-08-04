# 93 file-tree-empty-state — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:429-448` (ранний return в `file_tree()` при `workspace=None`)

## Структура (gpui-дерево кратко)
```
div #panel_key .size_full .flex .flex_col .items_center .justify_center
    .gap(SPACE_2=8) .text_color(text_muted)
├── codicon "\u{ea83}" (folder) 32px в боксе 16×16
├── div .text_size(FS_SM) "No active session with a folder."
└── probe_area(panel_key)
```
Хедер (элемент 98) при этом НЕ рендерится (ранний return до его сборки).

## Метрики (из кода, точные)
- gap `SPACE_2` = 8px; цвет контейнера `text_muted` #838aa0; глиф 32px; текст `FS_SM` = 12px.
- padding не задаётся (центрирование flex-ом).

## Отличия от original.md той же папки
1. **Вторая подсказка отсутствует**: только «No active session with a folder.», нет «Pick a session in Projects, or start one with a folder.»
2. **Цвет иконки**: наследует `text_muted` #838aa0; в оригинале `.emptyIcon` = `--text-disabled` (#60667b) — иконка у нас светлее.
3. `padding: var(--space-5)` (20px) не воспроизведён — при узкой панели текст ляжет ближе к краям.
4. **Хедер дерева не рендерится** в empty-состоянии; в оригинале `<FileTreeHeader />` есть и тут (title «PROJECT», disabled-кнопки).
5. Бокс глифа 16×16 при font 32px — глиф вылезает за бокс (центрирован, визуально ок, но геометрия не 1:1).

## Дополнение атрибутов (цикл 10)

- скругления: у самого блока их нет; в пустом состоянии рисуется хедер, и скругления есть у трёх его кнопок — RADIUS_XS 4 (`ui/file_list.rs`, `tool_btn`)
- шрифты: подсказка fs-sm 12 (`file_list.rs:498`) = `.emptyHint { font-size: var(--fs-sm) }` (`FileTreeView.module.css:34-37`); глиф codicon 32 (`file_list.rs:495`) = `.emptyIcon { font-size: 32px }` (`:29-32`); текст кнопки (fs-sm 12 / weight 600 у оригинала, `:46-47`) отсутствует вместе с кнопкой.
- ховер: N/A: ховер — hover есть только у `.openBtn:hover` оригинала (фон `--accent-action-hover` #74c7ec dark / #b16527 light, `FileTreeView.module.css:52-55`); кнопки у нас нет, у остальной разметки пустого состояния hover-правил нет.
