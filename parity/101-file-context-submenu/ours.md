# 101 file-context-submenu — наша реализация
Файлы: `crates/shell/src/ui/file_menu.rs:272-303` (строка «Open In» + hover-открытие), `:549-641` (каскад), `:114-116` (константы), `crates/shell/src/root.rs:592-596` (`FileMenuOpenIn`)

## Структура (gpui-дерево кратко)
```
[menu.open_in] → sub: div #file-menu-sub .occlude .absolute .left(sub_x) .top(sub_y)
    .min_w(260) .flex .flex_col .gap(1) .p(SPACE_1) .rounded(RADIUS_MD)
    .bg(bg_surface) .border_1(text_primary 6%) .shadow(dropdown)
  ├── hit_area()
  ├── «Reveal in File Explorer» (explorer.exe [/select,path])
  └── dir → «Open in Terminal» | file → «Open in Associated Application»
sub_x = x + 200 + 2 (влево при нехватке: x − 260 − 2); sub_y = clamp(y, ..vh−120)
Открытие: on_hover строки «Open In ▸» → ShellEvent::FileMenuOpenIn(true);
активная строка подсвечена bg text_primary 10%.
```

## Метрики (из кода, точные)
- SUB_W min-width **260**; offset от родительского меню 2px ✓; бокс/item-метрики идентичны 100 (p 4, gap 1, radius 12, bg_surface, бордер 6%, item px12/py8/radius8/FS_SM, hover 10%).
- Строка-родитель: chevron-right codicon `\u{eab6}` 12px `text_muted` справа.

## Отличия от original.md той же папки
1. **Grace-закрытие 250мс НЕ реализовано, и хуже — каскад вообще не закрывается при ховере других пунктов root-меню**: замыкание `close_sub` создано, но мёртвое (`let _ = &close_sub;`, file_menu.rs:307-313) — sub живёт до закрытия всего меню.
2. **min-width 260 vs 180**.
3. **Привязка по вертикали**: `sub_y = y` (top root-меню), не rect строки `.hasSub`; совпадает только пока «Open In» — первая строка; кламп по низу `vh−120` эвристикой, без измерения.
4. Пункты каскада захардкожены (Reveal / Terminal / Associated App) — contributed-детей нет; в оригинале children строятся динамически.
5. Нет `role=menu`/`tabIndex=-1`, нет visibility-двухпроходности.

## Дополнение атрибутов (цикл 10)

- цвета: фон каскада bg_surface #3d3f51 dark / #e6e1d4 light (`file_menu.rs:613`, `palette.rs:57,95`), бордер `text_primary 6%` = #cfd4e2 α .06 / #322e28 α .06 (`file_menu.rs:615`, `--divider-soft`, `variables.css:151`), тень — `overlay::dropdown_shadow()` (`file_menu.rs:616`); пункт: текст text_primary #cfd4e2 / #322e28 (`file_menu.rs:547`), hover-фон text_primary α .10 (`file_menu.rs:189`), у danger-пункта hover #e5484d α .16 (`file_menu.rs:186-188`), иконка слота text_muted #838aa0 / #6e685d (`file_menu.rs:151`), danger-цвет — фолбэк-хекс #e5484d (`file_menu.rs:142`; токен `--accent-danger` в темах не объявлен, проверено grep).
- шрифты: пункт fs-sm 12 (`file_menu.rs:546`, `metrics/lib.rs:43`), глиф FontAwesome 12 в боксе 16 (`file_menu.rs:159`), шеврон «Open In ▸» codicon 12 (`file_menu.rs:306`); собственного font-weight у пунктов нет.
