# 100 file-context-menu — наша реализация
Файлы: `crates/shell/src/ui/file_menu.rs:114-547` (`file_menu()`, `item`/`icon_slot`/`divider`), `crates/shell/src/root.rs:828-847` (открытие/multi), `:4892,5055-5066` (закрытие Esc/click-away), `crates/shell/src/overlay.rs:174+` (`dropdown_shadow`). Рендер в overlay-окне (единый слой), `hit_area` в корне.

## Структура (gpui-дерево кратко)
```
layer: div .absolute .top_0 .left_0 .size_full
└── col: div #file-menu .occlude .absolute .left(x) .top(y) .min_w(200)
      .flex .flex_col .gap(1) .p(SPACE_1) .rounded(RADIUS_MD)
      .bg(bg_surface) .border_1(text_primary 6%) .shadow(dropdown)
    ├── hit_area()
    ├── «Open In ▸» (hover → каскад, элемент 101) + divider
    ├── [dir] New File… / New Folder… + divider
    ├── Cut / Copy / Paste + divider          // multi>1 → операция над выбором
    ├── Rename… / Delete («Delete N items» при multi) + divider
    ├── Copy Path / Copy Relative Path
    └── contributed explorer/context (when-движок, сортировка групп
        navigation-first, divider на смене группы, без иконок;
        клик → kamin:command:execute с Uri {$mid:1})
item: .flex .items_center .gap(SPACE_2) .px(SPACE_3) .py(SPACE_2) .rounded(RADIUS_SM)
      .text_size(FS_SM); icon_slot 16px (FA-глиф 12px, muted | red)
```
Позиция: `x = clamp(cursor, MARGIN..viewport−200−8)`, `y = clamp(cursor, ..viewport−est_h−8)`, est_h = 380 (dir) / 330 (file).

## Метрики (из кода, точные)
- Меню: min-width **200**, padding `SPACE_1` 4, gap 1, radius `RADIUS_MD` 12, bg `bg_surface` #3d3f51, бордер 1px `text_primary` a=0.06, тень 0/8/24 rgba(0,0,0,.45).
- Item: gap 8, px 12, py 8, radius 8, `FS_SM` 12, `text_primary` #cfd4e2; hover `text_primary` 10%.
- Danger: `accent_red` #f38ba8 (текст+иконка), hover red 16%.
- Иконки: FontAwesome solid (weight 900), слот 16px, глиф 12px, `text_muted` #838aa0.
- Divider: h 1, mx `SPACE_2` 8, my `SPACE_1` 4, `text_primary` 6%.

## Отличия от original.md той же папки
1. **min-width 200 vs 180** (SUB тоже шире — см. 101).
2. **Позиционирование эвристикой** est_h (380/330) вместо двухпроходного измерения (visibility hidden→visible); при contributed-пунктах est_h занижен → меню у нижнего края может вылезти. **Нет max-height/overflow-y** (`calc(100vh-16px)`).
3. **danger-цвет**: `accent_red` #f38ba8 vs `var(--accent-danger, #e5484d)` оригинала.
4. **Бордер/сепаратор** из `text_primary 6%` vs `var(--divider-soft)` — сверить фактическое значение токена.
5. **Порядок**: «Open In» вынесен первым фикс-пунктом; в оригинале порядок = state.extra (tab-actions) → builtinActions → contributed. **tab-actions (extra) не поддержаны.**
6. **Закрытие**: Esc + click-away (root.rs) есть; закрытия по scroll(capture) нет; `role=menu/menuitem`-семантики нет.
7. Contributed-пункты без иконок (слот пустой) — как в оригинале иконка тоже отсутствует? В оригинале i.fas рендерится всегда фикс-слотом — совпадает.
8. Наша добавка: «Delete N items» при мультиселекте (в original.md не описано).
