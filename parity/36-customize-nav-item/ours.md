# 36 customize-nav-item — наша реализация
Файлы: `crates\shell\src\ui\customize.rs:56-86` (builtin-строки), `:123-158` (child-строки contributed)

## Структура (gpui-дерево кратко)
```
div#cz-{id} .flex .items_center .gap(SPACE_2=8) .px(8) .py(6)
  .rounded(RADIUS_SM=8) .text_size(FS_SM=12) .text_color(text_secondary)
  .cursor_pointer
  .hover(bg tint(text_primary,0.08) + text_primary)
  [active] → bg tint(accent_primary,0.16) + text_primary
  .on_mouse_down(L: SetCustomizePanel)
├─ codicon(codicon_by_name(icon), 15px) .text_color(text_muted)
└─ {label}
child-вариант (contributed-страница): .pl(40) .pr(8), остальное то же
```

## Метрики (из кода, точные)
- padding 6×8, gap 8, radius 8, fs 12
- Иконка codicon 15px, `text_muted`
- hover: `text_primary@8%` + text_primary; active: `accent_primary@16%` + text_primary
- child: padding-left 40

## Отличия от original.md той же папки
1. **font-size 12 (`FS_SM`) vs оригинальный `var(--fs-md)` = 13**.
2. Паддинг 6×8 vs оригинальный `8×12` (`space-2 space-3`) — строки ниже и уже.
3. Иконка 15px vs `14px !important`; цвет у нас всегда `text_muted` (оригинал наследует цвет строки: text-secondary → text-primary на hover/active).
4. hover-фон: `text_primary@8%` vs `color-mix(bg-surface 50%, transparent)` — другой рецепт (у оригинала полупрозрачный серый поверхностный, у нас белёсый).
5. active: `accent_primary@16%` + text_primary — 1:1.
6. child-инсет 40 vs `calc(space-3 + 18px)` = 30.
7. `<img width=16 height=16>`-иконки (contributed image icon) НЕ ПОДДЕРЖАНЫ — только codicon по имени с фоллбеком `\u{eb51}`.
8. `aria-pressed` нет.

## Дополнение атрибутов (цикл 10)

- цвета: покой — фона нет, text_secondary #adb3c7 (`crates/shell/src/ui/customize.rs:100`); hover — bg = bg_surface #3d3f51 при альфе 0.5 + text_primary #cfd4e2 (`customize.rs:89,102`); active — bg = accent_primary #89b4fa при альфе 0.16 + text_primary #cfd4e2 (`customize.rs:112-113`); иконка своего цвета не имеет, наследует цвет строки (`customize.rs:106-108`)
