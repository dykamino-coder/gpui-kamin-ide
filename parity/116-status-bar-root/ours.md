# 116 status-bar-root — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:160-280 (status_bar), crates\shell\src\root.rs:5370-5380 (вызов), crates\metrics\src\lib.rs:20

## Структура (gpui-дерево кратко)
```
div#status-bar: relative.flex_shrink_0.h(24).w_full.flex.items_center.px(8).gap(4)
├─ probe_area("status-bar")
├─ left: flex.items_center.gap(2)
│  ├─ item «N active» (circle-filled, green)
│  ├─ (failed>0) «N failed» (warning, yellow)
│  ├─ (disabled>0) «N off» (circle-slash, muted)
│  ├─ «N cmds» (symbol-keyword, muted)
│  └─ contrib × N (alignment=1, priority desc)
└─ right: flex.items_center.gap(2).ml_auto
   ├─ contrib × N (alignment=2)
   ├─ (update) пилюля «Update {ver}» (№120)
   ├─ (eol) «UTF-8» + «LF|CRLF» (№119)
   └─ бренд «KaminIDE {version}» (№120)
```

## Метрики (из кода, точные)
- Высота 24 (STATUS_BAR_HEIGHT), px 8 (SPACE_2), gap контейнера 4 (SPACE_1), fs 11 (FS_XS), базовый цвет p.text_muted #838aa0, фон прозрачный (градиент окна просвечивает)
- Группы left/right: gap 2, right = ml_auto

## Отличия от original.md той же папки
1. `items_center` вместо `align-items: stretch` (item'ы не тянутся на всю высоту бара; при нашем py у item'ов hit-зона ниже).
2. Правые contributed отсортированы по УБЫВАНИЮ priority — общая сортировка перед разбором; в оригинале правые по возрастанию (asc).
3. Порядок правой группы: contributed → update-пилюля → encoding/EOL → бренд; в оригинале contributed → EncodingItems → VersionUpdateItem (версия/апдейт — ПОСЛЕДНИЙ, единый item).
4. Тег `<footer>` и aria отсутствуют (div + probe_area).
5. gap 2 у групп и gap 4 контейнера — совпадают с оригиналом.

## Дополнение атрибутов (цикл 10)

- шрифты: контейнер font-size 11 (FS_XS) (status_bar.rs:253), font-weight 400, семейство — UI-шрифт окна «Bricolage Grotesque» (собственного font_family бар не задаёт); группы left/right своих шрифтовых правил не задают — наследуют 11
