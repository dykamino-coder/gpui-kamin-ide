# 73 contributed-container-body — наша реализация

НЕ РЕАЛИЗОВАНО как контейнер-стек нескольких views.

Замена: каждый contributed view = отдельный «dyn tool» активити-реестра; тело одного тула рендерит `tool_body` (dyn-ветка).
Файлы: `crates/shell/src/root.rs` (tool_body → dyn_tool branch), `root.rs:3492-3560` (webview_body_dyn), `root.rs:3281-3420` (visual_wv_body), welcome-ветка там же

## Структура (gpui-дерево кратко)
```
div (flex-col, size_full, min_h 0)
├─ div — секц-титул: d.view_name.to_uppercase()   (см. 74)
└─ div (flex_1, min_h 0) → webview_body_dyn(view_id, wv, alive, placeholder, p)
```
- Нет группировки по containerId: контейнер расширения разворачивается в плоский список тулов, каждый живёт в своём слоте самостоятельно.
- resolve-ретрай: `view_resolve_at` — повторный `resolve_webview` не чаще раза в 5с, пока вью не alive (root.rs:3024-3033).
- Welcome-замещение: при отсутствии активной сессии welcome заменяет ВСЮ панельную область (все колонки), а не только chat-view (root.rs:5366).

## Метрики (из кода, точные)
- Обёртка: `flex-col`, `size_full`, `min_h 0` — совпадает с `.root` оригинала (без отступов/фона).
- Титул: px 12, pt 4 (SPACE_1), pb 2, FS_XS(11), weight Medium, text_muted `#838aa0`, uppercase.

## Отличия от original.md той же папки
1. Нет multi-view стека: оригинал рендерит `views.map → ViewSection` внутри одного `.root`; у нас один view = один тул = одна панель.
2. Нет `ActivityPlaceholder "No views"` для пустого контейнера — пустой контейнер просто не порождает тулов.
3. Welcome-фоллбек шире оригинала: заменяет всю область панелей, а не тело chat-view.
4. `ContributedViewBody` (flush-вариант для Customize) у нас — отдельная ветка root.rs:5304-5333 (czShared-вебвью, один переиспользуемый wv2 на все contributed Customize-страницы; в оригинале — свой iframe на view).

## Дополнение атрибутов (цикл 10)

- скругления: подложка динамического вебвью — `--radius-lg` 16, как `.frame` оригинала (`ContributedContainerBody.module.css:57`); в цикле 14 радиус протащен и в боевой путь visual hosting (клип зоны + вырез фона `glint.rs:194`), где раньше был 0/12; бейдж хедера radius 9 (`root.rs:3547`) = `.viewBadge { border-radius: 9px }` (`:39`).
- ховер: N/A: ховер — ни контейнер (`root.rs:3661-3674`), ни хедер вью (`root.rs:3523-3557`) hover-стилей не имеют, как и `.root`/`.view`/`.title`/`.frame` оригинала; у бейджа только tooltip (`root.rs:3555`).
