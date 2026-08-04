# 104 contributed-tree-view-body — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`tree_view_body`, `level`), root.rs (`contributed_tree_section`, состояние `trees`), host_link.rs (`request_tree_children`, `request_tree_meta`, канал `kamin:tree:changed`)

## Структура/содержание
```
div .flex_1 .flex_col .min_h 0                  ← .root
├─ (meta.message) div px 8 / py 4, fs-sm, opacity .75
└─ div .flex_1 .overflow_y_scroll .pt 4 .px 6 .pb 8 .text_size 12   ← .body
    └─ уровни: level("", depth 0) → строки узлов + рекурсия раскрытых
```
Дети тянутся лениво: корень — при первом показе панели, уровень узла — при первом раскрытии (`kamin:tree:getChildren`). `kamin:tree:changed` помечает все известные уровни как «грузится» и перезапрашивает их (аналог перемонтирования по `version`). Состояния: «Loading…» (уровень ещё не пришёл), «(empty)» только на depth 0, «… N more» при > 100 узлов.

## Метрики (из кода, точные)
- `.body`: padding 4 / 6 / 8, fs FS_SM 12, `overflow_y_scroll`, min-h 0.
- `.loading`/`.emptyChild`: fs FS_XS 11, text-muted, py 2 + `paddingLeft = depth*12 + 8`.
- message-баннер: px SPACE_2 8, py SPACE_1 4, fs FS_SM 12, opacity 0.75.
- Кап уровня TREE_CHILD_CAP = 100, без кнопки догрузки — только счётчик остатка.

## Отличия от original.md той же папки
1. Вью выбирается по `contributes.views[].type != "webview"` (`DynTool.webview`); tree-вью больше не регистрируются как вебвью и не ждут html.
2. Customize-страницы contributed-контейнеров по-прежнему рендерятся только вебвью (Bridge объявляет их `type: webview`); tree-страница в Customize не поддержана.
3. DnD (`TreeDragAndDropController`) не портирован — в gpui нет HTML5-DnD.

## Дополнение атрибутов (цикл 10)

- цвета: собственного фона у тела нет (`contributed_tree.rs:439-450`) — просвечивает карта bg_mantle #262533 dark / #fbf7f4 light (`palette.rs:55,93`); строки «Loading…» / «(empty)» / «… N more» — text_muted #838aa0 / #6e685d (`contributed_tree.rs:160`, `palette.rs:65,103`); message-баннер цвета не задаёт, наследует текст карты при opacity 0.75 (`contributed_tree.rs:435`). Совпадает с оригиналом (`.body` без фона, `.loading`/`.emptyChild { color: var(--text-muted) }`, `FileTreeView.module.css:8-15,155-160`).
