# 98 file-tree-header-toolbar — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:376-409` (`tool_btn`), `:458-520` (header), `:112-146` (`flat_row_index`); `crates/shell/src/root.rs:550-591` (`LocateSelectedFile`), `:665-673` (Refresh/Collapse), `:1171-1173` (IndexStatus)

## Структура (gpui-дерево кратко)
```
header: div .flex .items_center .gap(SPACE_1=4) .pl(12) .pr(8) .py(8) .flex_shrink_0
├── title: div .flex_1 .min_w(0) .ellipsis .nowrap .text_size(FS_XS)
│     .font_weight(MEDIUM) .text_color(text_muted) .child(имя_папки.to_uppercase())
├── [indexing] div .px(8) .text_size(FS_XS) .text_color(text_muted) .opacity(0.85)
│     "Indexing…"                       // без спиннера
└── actions: div .flex .items_center .gap(2)
    ├── tool_btn "tree-locate"   "\u{ebf8}" "Locate selected file"
    ├── tool_btn "tree-collapse" "\u{eac5}" "Collapse all folders"   // collapse-all
    └── tool_btn "tree-refresh"  "\u{eb37}" "Refresh"

tool_btn: div 22×22 .flex center .rounded(RADIUS_XS) .text_color(text_muted)
    codicon(glyph, 14) ; hover: bg_surface 60% + text_primary; disabled → opacity 0.4
```
Locate (root.rs): цель = selected | активный таб редактора → раскрыть предков до корня (+дозапрос листингов) → select → скролл `set_offset(idx*24 − 140)`.
Refresh: пере-листинг всех expanded директорий (без ремаунта). Collapse: `expanded = {root}`.

## Метрики (из кода, точные)
- header: gap 4, `padding: 8px 8px 8px 12px` ✓, flex_shrink_0 ✓.
- title: `FS_XS` 11, weight 500, `text_muted` #838aa0, uppercase, ellipsis.
- indexing: FS_XS, text_muted, opacity 0.85.
- btn: 22×22, глиф 14px, radius 4, hover `bg_surface` #3d3f51 a=0.6 + `text_primary` #cfd4e2.

## Отличия от original.md той же папки
1. **Title без `letter-spacing: 0.08em` и `font-feature-settings: "ss01"`**.
2. **Indexing без спиннера** codicon-loading (spin, 12px) и gap 4 — только текст; тултип «Building the search index…» отсутствует.
3. **Collapse/Expand не тумблер**: всегда collapse-all (\u{eac5}) с одной подписью; состояния `treeAllCollapsed` → codicon-expand-all/«Expand all folders» нет.
4. **Disabled-логика отсутствует**: все 3 кнопки всегда активны (`disabled=false`); в оригинале locate гаснет без root/selectedFile, остальные без root.
5. **Locate без флеша и smooth-скролла**: мгновенный `set_offset` по расчётной высоте строки 24px (реальная ~20-22 → накапливается неточность на длинных списках); `.flash` (treeFlash 0.9s accent 40%→transparent) не реализован нигде в gpui-порте; поллинга data-tree-id нет (расчёт синхронный — листинги предков могут ещё не прийти → скролл до подгрузки промахивается).
6. **Refresh** — пере-листинг expanded-директорий, а не полный ремаунт (null→восстановить); кэш deco не сбрасывается.
7. Глиф locate `\u{ebf8}` — сверить с оригинальным `codicon-target`.
