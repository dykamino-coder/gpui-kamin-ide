# 37 customize-contributed-tree — наша реализация
Файлы: crates/shell/src/ui/customize.rs (`customize_nav`, `nav_icon`), host_link.rs (`CzContainer`/`CzPage`, `customize_pages_from`, `customize_pages_from_manifests`)

## Структура (gpui-дерево кратко)
```
для КАЖДОГО контейнера location=customize:
├─ строка-родитель (id "cz-contrib-<id>")
│   ├─ chevron-right|down 12, text-muted
│   ├─ nav_icon(container.icon)
│   └─ container.title
└─ (раскрыт) строки-дети: nav_icon(view.icon ?? circle-small) + view.name
```
Клик по родителю: тоггл группы И, если ни одна его страница не открыта, — открыть `views[0]` (как в оригинале). Родитель подсвечен, когда активна его дочерняя страница (childActive).

## Метрики (из кода, точные)
- Строка: gap SPACE_2 8, px SPACE_3 12, py SPACE_2 8, radius RADIUS_SM 8, fs FS_MD 13, цвет text-secondary; hover bg-surface 50% + text-primary; активная — accent-primary 16% + text-primary.
- Дочерняя строка: `padding-left = SPACE_3 + 18` = 30 (`.child` оригинала), pr 12.
- `nav_icon`: путь/URL (`data:`,`http:`,`https:`,`file:`,`/`) → `img` 16×16; иначе codicon 14 (fallback-глиф gear).
- Фолбэк иконки страницы — `circle-small`, как в оригинале.

## Отличия от original.md той же папки
Титул/иконка контейнера больше не захардкожены — берутся из реестра (`viewContainers[].title/.icon`), причём и из манифестов на диске до прихода снапшота.

## Дополнение атрибутов (цикл 10)

- цвета: строка-родитель контейнера — text_secondary #adb3c7 (`crates/shell/src/ui/customize.rs:144`), hover bg = bg_surface #3d3f51 при альфе 0.5 + text_primary #cfd4e2 (`customize.rs:121,146`), childActive — bg accent_primary #89b4fa при альфе 0.16 + text_primary (`customize.rs:165-166`); chevron text_muted #838aa0 (`customize.rs:159`); дочерние строки — text_secondary #adb3c7, тот же hover, active accent_primary@0.16 + text_primary (`customize.rs:188,190,198-199`); собственного фона в покое нет
- шрифты: строки (и родитель, и дети) text_size FS_MD = 13 (`customize.rs:142,187`); font-weight не задан; chevron `codicon(..., 14.0)` (`customize.rs:157`); `nav_icon` — `codicon(g, 14.0)` либо картинка 16×16 для path/URL-иконок (`customize.rs:41-42,49`); заголовок «CUSTOMIZE» над списком — FS_XS = 11, weight MEDIUM (`customize.rs:77-78`)
