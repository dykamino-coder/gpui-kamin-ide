# 136 sample-list-item — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:209-254 (`sample_list_item`), 805-809 (`block("List item — active selection (sidebar pattern)", …)`)

## Структура/содержание
```
block «LIST ITEM — ACTIVE SELECTION (SIDEBAR PATTERN)»
└─ list div.flex_col.gap(2).w_full.max_w(280)
   ├─ row «Sessions»          codicon \u{ea83} (folder)
   ├─ row «Settings (active)» codicon \u{eb51} (settings-gear)  ← active
   ├─ row «Extensions»        codicon \u{eae6} (extensions)
   └─ row «Disabled»          codicon \u{ead0}                  ← disabled
row = div.id(label).flex.items_center.gap(8).w_full.px(12).py(8).rounded(8)
```
Три ветки состояний (design_panel.rs:241-250): active → accent-tint + accent-текст + свой hover; disabled → `opacity(0.45)` и НИ ОДНОГО hover; обычная → hover-tint + подъём текста.

## Метрики (из кода, точные)
- отступы: строка px 12 (SPACE_3) / py 8 (SPACE_2); у списка padding/margin нет
- гэпы: список gap 2; строка gap 8 (SPACE_2) между глифом и подписью
- цвета: обычная строка — текст p.text_secondary #adb3c7, фон прозрачный; active — bg p.accent_primary #89b4fa α 0.14 + текст p.accent_primary #89b4fa; disabled — те же цвета обычной строки под `opacity 0.45`
- скругления: строка rounded 8 (RADIUS_SM)
- шрифты: строка font-size 13 (FS_MD), weight 400; глиф — codicon font-size 14; подпись блока 11 (FS_XS) BOLD uppercase
- фоны по ховеру: обычная — p.bg_surface #3d3f51 α 0.5 + текст поднимается до p.text_primary #cfd4e2; active — p.accent_primary α 0.22 (текст остаётся accent_primary); disabled — hover не навешивается вовсе

## Отличия от original.md той же папки
1. Состав (4 строки, те же подписи и codicon-глифы folder / settings-gear / extensions / debug-disconnect), `.itemList` (gap 2, width 100%, max-width 280), `.listItem` (padding 8×12, radius 8, fs-md 13, text-secondary, codicon 14), hover `bg-surface 50%` + text-primary, active `accent-primary 14%` + accent-текст, active-hover 22%, disabled `opacity 0.45` — совпадают с оригиналом 1:1.
2. `cursor: pointer` у строки НЕ задан (в оригинале есть у `.listItem`); `cursor: not-allowed` у disabled тоже нет.
3. `transition: background 150ms ease` отсутствует.
4. Light-вариант `[data-theme="light"] .listItemActive` (сплошной `--accent-primary` фон, текст и глиф `--accent-action-fg`, font-weight 600, hover `--accent-action-hover`) НЕ РЕАЛИЗОВАН: в светлой теме активная строка остаётся accent-tint 14% (цвета берутся из LIGHT-палитры, но формула та же, что в dark).
5. Строки — stateful `div`, а не `<li><button>`: нет роли списка/кнопки, нет атрибута `disabled` (только визуальная opacity), клика тоже нет — образец инертный.
6. Глифы заданы кодпоинтами codicon-шрифта напрямую, без класса `.codicon`; размер 14 совпадает с `:global .listItem .codicon`.
