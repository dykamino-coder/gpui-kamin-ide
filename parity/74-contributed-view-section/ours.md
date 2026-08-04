# 74 contributed-view-section — наша реализация
Файлы: crates/shell/src/root.rs (`contrib_view_header`, ветка `dyn_tool` в `tool_body`), ui/contributed_tree.rs (мета вью)

## Структура (gpui-дерево кратко)
```
div .flex_col .size_full .min_h 0                     ← .view
├─ contrib_view_header                                ← .title
│   ├─ титул: meta.title ?? contributed name, uppercase
│   ├─ (meta.description) .viewDescription
│   └─ (meta.badge) .viewBadge (ml auto)
└─ тело: вебвью (type=webview) либо tree_view_body (TreeDataProvider)
```
Мета берётся из `kamin:tree:getMeta` и broadcast'а `kamin:tree:meta` (createTreeView).

## Метрики (из кода, точные)
- `.title`: flex, items-center, padding SPACE_1 4 / SPACE_3 12 (симметрично, как в оригинале), fs FS_XS 11, text-muted, flex-shrink 0.
- `.viewDescription`: margin-left SPACE_2 8, font-weight 400, opacity 0.55.
- `.viewBadge`: margin-left auto, min-w 18, px 5, radius 9, bg `--accent-primary`, цвет `--bg-base`, fs 0.75em (11×0.75), line-height 16, по центру; tooltip = `badge.tooltip`.

## Отличия от original.md той же папки
1. `letter-spacing .04em` в gpui недоступен; uppercase делается в Rust (`to_uppercase`).
2. Несколько `.view` в одном контейнере (стек с flex:1) не поддержано — панель показывает ПЕРВОЕ вью контейнера (см. 73).

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — секция (`contrib_view_header`, `root.rs:3514-3559` + тело) не интерактивна и hover-правил не задаёт; у `ViewSection` оригинала (`ContributedContainerBody.tsx:62-77`, стили `.view`/`.title`) их тоже нет — сворачивания по клику в этой версии нет.
