# 12 layout-presets-section — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:227-399 (presets_section),
401-431 (menu_item); данные — crate::layout_store::load_presets()

## Структура (gpui-дерево кратко)
```
div (flex col, gap 1)
 ├ menu_label "LAYOUTS"
 ├ menu_item codicon-save \u{eb4b} "Save current layout…"
 ├ menu_item codicon-desktop-download \u{ea78} "Export current layout…"
 ├ menu_item codicon-cloud-upload \u{eac3} "Import layout…"
 ├ presets.is_empty(): div "No saved layouts yet"
 └ ×N presetRow (flex, gap 1)
     ├ apply-кнопка flex_1 (codicon \u{ebeb} layout 14px muted + имя ellipsis;
     │   ЛКМ apply, ПКМ rename-prompt)
     ├ icon_btn save-as \u{eb4a} (overwrite) · desktop-download \u{ea78} (export)
     ├ icon_btn star \u{eb59} full / \u{ea6a} empty (default toggle)
     └ icon_btn trash \u{ea81} (delete)
```

## Метрики (из кода, точные)
- menu_item / apply: gap SPACE_2 (8), px SPACE_3 (12), py SPACE_2 (8),
  rounded RADIUS_SM (8), fs FS_SM (12), color text_primary,
  hover tint(text_primary, 0.10); иконка codicon 14px text_muted
- presetEmpty: px SPACE_3, py SPACE_1, fs FS_XS (11), text_muted
- presetRow: gap px(1.0)
- icon_btn: 26×26, rounded RADIUS_SM (8), глиф codicon 13px, color text_muted
  (star-default → accent_primary), hover tint(text_primary, 0.10) + text_primary

## Отличия от original.md той же папки
1. Иконка в menu_item/apply — 14px; оригинал не форсит размер (наследует
   fs-sm 12px у codicon в тексте). +2px.
2. Save/Export/Import у оригинала — .menuItem БЕЗ ведущей иконки-чекбокса,
   с codicon — совпадает; но у нас нет letter-spacing у label секции (см. 11).
3. star-глифы: full \u{eb59} / empty \u{ea6a} — соответствуют
   codicon-star-full/star-empty; aria-pressed → цвет accent — 1:1.
4. transition — нет.
Метрики (26×26 icon-btn r8 глиф 13, padding 8/12, hover 10%, gap 1) — 1:1.

## Дополнение атрибутов (цикл 10)

- цвета: заголовок «LAYOUTS» text_muted #838aa0 (`crates/shell/src/ui/layout_popover.rs:165`, вызов `:269`); пункты Save/Export/Import — text_primary #cfd4e2, иконка text_muted #838aa0, hover bg = text_primary при альфе 0.10 (`layout_popover.rs:444,454,456,463`); пустое состояние «No saved layouts yet» text_muted #838aa0 (`layout_popover.rs:300`); строка пресета text_primary #cfd4e2 + hover bg text_primary@0.10 (`layout_popover.rs:356,359-360`), иконка папки text_muted #838aa0 (`layout_popover.rs:382`); `presetIconBtn` — покой text_muted #838aa0, активная (star=default) accent_primary #89b4fa, hover bg text_primary@0.10 + fg text_primary #cfd4e2 (`layout_popover.rs:314,324-328,330`); divider над секцией bg = text_primary@0.06 (`layout_popover.rs:252`); собственного фона у секции нет — bg_surface #3d3f51 поповера (`layout_popover.rs:70`)
