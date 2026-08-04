

## Цикл 4: DIVERGES

Иконка узла contributed-дерева не реализована; светлый фильтр иконок
(как в 99) тоже отсутствует.

## Цикл 8: DIVERGES

Иконка узла contributed-дерева не реализована.

## Цикл 10: DIVERGES

Не реализовано (см. 104).

## Цикл 7: DIVERGES

Порядок веток и данные верны (codicon → resourceUri basename → generic
circle-outline/folder; `file_img`/`folder_img(name, expanded)`).

Исправлено по ревью: `.icon` задаёт только БОКС 16×16, кегль глифа наследуется от
строки — было `codicon(glyph, 16.0)` (бокс = кегль), стало бокс 16 / шрифт fs-sm 12;
неизвестный ThemeIcon даёт пустой бокс, а не circle-outline.

Осталось: light-фильтр `saturate(3.2) brightness(0.7)` для `<img>`-иконок (см. 99).

## Цикл 13: DIVERGES

Закрыто: кегль обеих codicon-веток — 16, а не FS_SM 12. Модуль
(`FileTreeView.module.css:131-135`) кегль у `.icon` не задаёт, значит работает
база `.codicon` 16; вердикт цикла 7 «кегль наследуется от строки» был неверен.

Осталось: light-фильтр `saturate/brightness` у `<img>`-ветки.

## Цикл 13 (ревью зоны): DIVERGES

Ревью подтвердило порядок веток (codicon → resourceUri по basename → generic),
бокс 16×16 и кегль глифа 16 в обеих codicon-ветках.

Осталось: светлотемный фильтр `saturate(3.2) brightness(0.7)` у `<img>`-ветки
(общий пробел с элементом 99).

## Цикл 15: MATCH

Иконка узла: ветвление ThemeIcon → resourceUri → circle-outline/folder, бокс 16×16, кегль 16.

## Цикл 18: MATCH

Иконка узла: ThemeIcon → resourceUri → generic, бокс 16×16, кегль 16, неизвестный ThemeIcon = пустой бокс.

## Цикл 21: MATCH

Иконка узла: ThemeIcon → resourceUri → generic, бокс 16×16, кегль 16, неизвестный ThemeIcon = пустой бокс.

## Цикл 22: MATCH

Общий с 99 пробел закрыт: `<img>`-ветка в светлой теме проходит фильтр
`saturate(3.2) brightness(0.7)` (см. 99, замер там же).
