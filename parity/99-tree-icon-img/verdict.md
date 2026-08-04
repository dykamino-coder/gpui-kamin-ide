

## Цикл 4: DIVERGES

TreeIcon: бокс 16×16 — MATCH. Нет светлого фильтра
(`saturate(3.2) brightness(0.7)`) и карт `rootFolder*`/`isRoot`.

## Цикл 8: DIVERGES

Бокс 16×16 верен. Нет светлого фильтра `saturate(3.2) brightness(0.7)` и карт `rootFolder*` для корня.

## Цикл 10: DIVERGES

Бокс 16×16 верен. Нет светлого фильтра saturate(3.2) brightness(0.7) (grep = 0). Нет rootFolder-иконок: в IconTheme нет полей rootFolder/rootFolderNames, флаг isRoot в folder_img не передаётся.

## Цикл 13: DIVERGES

★ Закрыто: составные расширения. Резолв брал ТОЛЬКО последний сегмент
(`rsplit('.').next()`), из-за чего 119 составных ключей таблицы Catppuccin
(`css.ts`, `g.dart`, `docker-compose.yml`, `azure-pipelines.yml`…) были
недостижимы. Теперь суффиксы перебираются от ДЛИННОГО к короткому, как
`file-icons.ts:73-107`; на это есть юнит-тест `composite_extensions_resolve`.

Осталось: light-фильтр `saturate(3.2) brightness(0.7)`; `isRoot`-карты
(`rootFolder*`); `languageIds` и слой `doc.light` у contributed-тем.

## Цикл 13 (ревью зоны): DIVERGES

Осталось: светлотемный фильтр `saturate(3.2) brightness(0.7)` (в gpui нет
CSS-фильтров — нужна отдельная обработка ассета); карты `rootFolder*` и флаг
`isRoot` (у нас `folder_img(name, open)` такого признака не принимает).

Бокс 16×16, порядок резолва и составные расширения ревью подтвердило.

## Цикл 15: DIVERGES

Осталось: светлотемный `filter: saturate(3.2) brightness(.7)` у иконок и карты `rootFolder*` для корня.

## Цикл 18: DIVERGES

Осталось: светлотемный `filter: saturate(3.2) brightness(.7)`, карты `rootFolder*` для корня, шаг `EXT_TO_LANGUAGE_ID → languageIds`, слой `doc.light`.

## Цикл 21: DIVERGES

Осталось: светлотемный `filter`, карты `rootFolder*`, `languageIds`, слой `doc.light`.

## Цикл 22 (правка 3): MATCH

★ Закрыто всё оставшееся по иконкам дерева.

1. **Светлый фильтр** `saturate(3.2) brightness(0.7)` (`TreeIcon.module.css:6`).
   CSS-фильтров в gpui нет — ту же матрицу применяем к hex-цветам ВНУТРИ SVG
   (`icon_light.rs`), результат кэшируется по ключу источника. Замер: строка
   `.android`, обводка `#cad3f5` → на экране `#7f93b3` (127,147,179); формула
   даёт (127,147,178) — расхождение в 1/255 от округления.
2. **`rootFolder*`** — корень воркспейса берёт `rootFolderNames(Expanded)` →
   `rootFolder(Expanded)` → `folder(Expanded)` и НЕ проваливается в обычные
   `folderNames` (`fileIconThemeData.ts:278-330`); `folder_img` получил
   `is_root`, юнит-тест `root_folder_maps`.
3. **`languageIds`** — слой после расширений, по последнему расширению через
   `EXT_TO_LANGUAGE_ID` (тест `language_id_layer`). Для встроенного Catppuccin
   слой ничего не меняет: все 59 расширений таблицы уже покрыты
   `fileExtensions` (проверено скриптом по вендорному `fileIcons.ts` — пересечение пустое).
4. **Слой `doc.light`** — карты светлого оверрайда идут перед базовыми
   (`layers()`, тест `light_override_wins`).

★ По пути найден баг вендора: SVG-ветка `Image::to_image_data` (в отличие от
всех прочих форматов и от ветки `Resource`) не делала RGBA→BGRA — иконка из
байтов рисовалась с переставленными R и B (коричневая вместо синей). Патч
`KaminIDE patch` в `vendor/gpui/src/platform.rs`, там же
`SMOOTH_SVG_SCALE_FACTOR` вместо 1.0 против замыливания.

## Цикл 23: MATCH

Иконки: светлотемный фильтр (saturate 3.2 → brightness 0.7 + кэш), `rootFolder*`, `languageIds`, слой `light` — с юнит-тестами.
