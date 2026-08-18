# План: полное покрытие WPT-рефтестов (все не-JS зелёные)

Дата: 2026-08-19. Владелец плана: стенд wptrun (`crates/html/examples/wptrun.rs`).

## Текущее состояние

- css-writing-modes: **175/187 = 93.6%** (12 красных в 8 блоках; ещё +1 valign-003
  в незакоммиченном пакете таблиц).
- Полный не-JS свод старых семейств: `target/all-nojs.txt` — **3144 пары**
  (сумма pairs-{flexbox,gabs,grid,pos,text,wm}-nojs). Прогон идёт, отчёт
  сохранится в `target/rep-all-nojs.txt`, таблица по семействам — по завершении.
- Контроли на момент плана: mix-nojs 112/140, mix160 ~120/160, юниты 128.

## Довендорено 2026-08-19 (из апстримного WPT, sparse-клон)

Новые каталоги в `vendor/wpt-parsing/`: css-images, css-text-decor,
css-variables, css-masking, filter-effects, css-pseudo, css-contain,
css-nesting, css-logical, css-conditional, mediaqueries, compositing,
css-scrollbars, css-transitions, css-animations, css-shapes, css-break,
css-inline, css-page, quirks/, svg/, common/.

Списки пар построены (`target/pairs-<семейство>-nojs.txt`, рефтесты с
`rel=match` без `<script>`), **итого +3784 пары**:

| Семейство | Пар | Семейство | Пар |
|---|---|---|---|
| css-transforms | 678 | css-conditional | 139 |
| css-images | 387 | css-inline | 138 |
| css-masking | 269 | css-pseudo | 95 |
| css-text-decor | 263 | compositing | 48 |
| css-multicol | 229 | quirks | 17 |
| css-shapes | 223 | css-position-sticky | 14 |
| css-page | 217 | css-animations | 14 |
| css-contain | 237 | css-scrollbars | 11 |
| css-break | 193 | css-nesting | 8 |
| css-variables | 177 | mediaqueries | 7 |
| filter-effects | 261 | css-logical | 5 |
| svg | 153 | css-transitions | 1 |

Полный фронт: 3144 + 3784 ≈ **6900 не-JS пар**.

## Осталось сделать по инфраструктуре (ДО первых прогонов новых семейств)

1. **Слить `css/support` апстрима** в vendor (сейчас отложено — шёл прогон;
   мержить БЕЗ перезаписи существующих файлов, только новые: старые support
   выверены под текущие зелёные).
2. Проверить `resolve_links`/`rebase_css_urls` на новых каталогах: пути
   `/common/...`, `/svg/support/...`, `/fonts/...` должны разрешаться.
3. Прогнать каждое семейство ПО ОДНОМУ (гоча: один прогон за раз), сохранить
   `rep-<семейство>-base.txt` — это нулевые замеры.
4. Пары с `<link rel="stylesheet" href="/fonts/ahem.css">` работают; у svg
   проверить, как стенд ест `.svg`-эталоны (если эталон — svg-файл, стенд
   может не уметь: сначала посчитать долю таких).

## Порядок атаки (по частоте в реальном вебе × близость к готовому)

1. **css-variables (177)** — var() реализован, ожидаемо высокий базовый
   процент; дешёвые победы.
2. **css-text-decor (263)** — подчёркивания: underline рисуем; добрать
   style/thickness/offset/color.
3. **css-images (387)** — object-fit/position (частично есть), градиенты
   (линейный/радиальный есть, conic — писать в шейдер/полосами), image-set.
4. **compositing (48)** — mix-blend-mode/isolation реализованы (PaintGroup);
   маленькое семейство, быстро в зелень.
5. **css-conditional (139)** — @supports: в основном разбор; дёшево.
6. **css-masking (269)** — clip-path есть частично; mask-image — новая
   механика (маска в PaintGroup).
7. **css-transforms (678)** — самый большой; у нас только поворот текста.
   Нужен общий transform на коробках: матрица в div-отрисовку (gpui умеет
   TransformationMatrix — расширить с текста на поддеревья; Z-порядок уже
   чинён по экранным AABB).
8. **css-multicol (229)** — колонки: column_flow есть для текста; добрать
   блочные колонки/балансировку.
9. **filter-effects (261)** — blur есть (PaintGroup); grayscale/brightness/
   contrast/drop-shadow — пошейдерно в группу.
10. **css-shapes (223), css-inline (138), css-break (193), css-page (217)** —
    третий эшелон (печать/фигуры реже в вебе).
11. **svg (153)** — отдельный трактат: парсер svg-дерева + пути. Начать с
    подмножества rect/circle/path/fill/viewBox (иконки).
12. **css-pseudo (95), sticky (14), quirks (17), мелочь** — добирать между
    крупными.

## Методология (без изменений, обязательна)

- Решения только спеко-корректные с цитатами; не подгонять под тесты.
- Нетто-замер семейства + контроли mix-nojs/mix160 + wm-all после каждой
  правки; реверт нетто-минусов; юниты на каждом коммите.
- Стенд не модифицировать (кроме инфраструктуры загрузки).
- Дампы читать только свежими (WPT_DUMP=all), «медленных пар» следить.
- Флак-лист: css-flexbox-height-animation-stretch (0.27..1.63).

## Отсутствует в вендоре и осознанно отложено

- css-transitions/css-animations: скриптовая часть (нужно время/JS) — взяли
  только 15 статичных пар.
- Полный SVG-каталог урезан скриптовой фильтрацией (64 пропущено).
- css-fonts/css-ui/css-color и прочие УЖЕ БЫЛИ в вендоре — их не-JS пары
  входят в старые списки частично; при желании построить списки и по ним
  той же командой (скрипт в истории: построитель pairs-*-nojs).
