# 108 file-viewer-wrapper — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:4315-4495 (ветка редактора), 4498-4549 (top_card + glint-обёртка), crates\metrics\src\lib.rs, crates\theme\src\palette.rs

## Структура (gpui-дерево кратко)
```
gap_wrap_v_top(glint_surface_wv_holed(          — карта File-панели (glint-рамка)
  top_card: div.flex_col.size_full
  ├─ on_drop(ExternalPaths) + on_drop(DraggedFile)   — drop-zone редактора
  ├─ div (mode-header: justify_end, pt 6, px 8) → file_panel_mode_tabs
  └─ top_content =
     ├─ editor_tabs непусты: div.flex_col.size_full
     │  ├─ ряд: editor_tabs_bar (№110) + (dirty) кнопка «Save  Ctrl+S»
     │  └─ рамка редактора: div.flex_col.flex_1.mx(4).mt(4).mb(4)
     │     .rounded(12).overflow_hidden().bg(editor_bg)
     │     ├─ breadcrumb-строка 24px (путь ~-сокращённый, mono)
     │     └─ ряд: [Input редактора + sticky-overlay] + minimap
     └─ пусто: panel_placeholder (№109)
))
```
Лимит табов `MAX_EDITOR_TABS = 12` (root.rs:112) — эвикт старейшего un-pinned неактивного (root.rs:3648-3661), pinned-first сортировка (root.rs:2074-2081).

## Метрики (из кода, точные)
- Рамка редактора: `mx 4` (SPACE_1), `mt 4`, `mb 4`, `rounded 12` (RADIUS_MD), `bg p.editor_bg` #1d1c25, overflow hidden
- Breadcrumb: h 24, px 12 (SPACE_3), fs 11 (FS_XS), font «JetBrains Mono», цвет p.text_muted #838aa0, ellipsis
- Mode-header: pt 6, px 8 (SPACE_2), justify_end
- Кнопка Save: mx 8, px 12, py 3, rounded 8, fs 11 semibold, bg p.accent_action #89b4fa, fg p.accent_action_fg #313240, hover opacity .9
- Внешняя карта — glint-рамка (не bg-mantle-контейнер)

## Отличия от original.md той же папки
1. Нет контейнера `.viewer` (margin 0 6px 6px, bg-mantle, radius-md) — вместо него общая glint-карта File-панели; редакторная рамка получает mx 4 / mt 4 / mb 4 вместо паддингов `.body` 8px 0 10px.
2. Нет `.bodyFlush` и retained-слоя `retainLayer`: webview-панели у нас НЕ открываются как редакторские табы (см. №114/№115), путей `webview://<id>` нет.
3. Добавлен breadcrumb-заголовок с путём внутри рамки (в оригинале FileViewer его нет).
4. Добавлена кнопка «Save  Ctrl+S» при dirty (в оригинале нет).
5. Mode-header (file/web-переключатель) — часть этой обвязки; в оригинале это отдельный элемент file-panel-top-card (№63).
6. Drop-zone есть (ExternalPaths + внутренний drag из дерева), атрибут-семантики `data-drop-zone` нет.
