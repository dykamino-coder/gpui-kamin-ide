# 24 project-group-header — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:548-655` (project_header), `:770-812` (сборка группы)

## Структура (gpui-дерево кратко)
```
div#grp-{pid} .flex .items_center .gap(6) .w_full .h(26) .pl(6) .pr(4)
  .text_size(FS_SM=12) .font_weight(MEDIUM=500) .text_color(text_secondary)
  .cursor_pointer .overflow_hidden
  .hover(text_primary) .on_hover(HoverPill grp:{pid})
  .on_mouse_down(L: ToggleProjectCollapse) .on_mouse_down(R: Delete-project modal)
├─ codicon chevron-{right|down} 13px, text_muted, flex_shrink_0
├─ icon_theme::folder_img(name, expanded) 16×16       ← Catppuccin по имени папки
├─ div .flex_1 .min_w(0) .text_ellipsis {name}
├─ count-badge: .min_w(16) .h(16) .px(5) .rounded(9) .bg(bg_surface) .text(FS_XS=11, text_muted)
└─ .when(hovered) anchor_probe()                      ← якорь для overlay-пилюли (эл. 25)
```

## Метрики (из кода, точные)
- Высота 26, gap 6, padding 0 4 0 6 — 1:1
- fs 12, weight 500, `text_secondary` #adb3c7, hover → `text_primary` #cfd4e2 (только цвет, без фона) — 1:1
- chevron: codicon 13px, `text_muted` #838aa0; folder-icon 16×16
- count-badge: min-w 16, h 16, px 5, radius 9, `bg_surface` #3d3f51, fs 11, `text_muted` — 1:1

## Отличия от original.md той же папки
1. Chevron: у оригинала фикс `width: 16px; text-align: center` — у нас глиф без фиксированной ширины 16 (интринсик ~13px); текст группы стартует на пару px левее.
2. Тултип имени (`data-tooltip = folderPath ?? "Sessions without a folder"`) не реализован.
3. Доп. поведение: right-click по хедеру открывает модал «Delete project» — в оригинале RMB на группе ничего не делает (удаление только из hover-попапа).
4. Header — один div (клик по всей строке), у оригинала `.header` + вложенная кнопка `.headerMain`; визуально эквивалентно.
5. Группа без единой сессии не рендерится вовсе (`continue`), см. эл. 26.
