# Обновление vendored-крейтов

`vendor/` содержит upstream-код с необходимыми KaminIDE изменениями. Версии и
назначение форков закреплены в комментариях `[workspace.dependencies]` и
`[patch.crates-io]` корневого `Cargo.toml`; HTML-парсер подключается путями из
`crates/html/Cargo.toml`. История `git log -- vendor/` объясняет отдельные
изменения, но не заменяет полную разницу с upstream.

| Каталог | Чистая база патча |
| --- | --- |
| `gpui` | crates.io `gpui-pre` 0.3.8, снимок zed `279fe070bb389b79652e52065b2f001edcc0b11b` |
| `gpui-windows` | crates.io `gpui-pre-windows` 0.3.8, тот же снимок |
| `gpui-base` | crates.io `gpui-base` 0.7.1, longbridge/gpui-kit `87d10ae5e1299d1be18670a37570c5402f952e34`, `crates/base` |
| `gpui-component` | crates.io `gpui-component` 0.7.1, тот же commit, `crates/component` |
| `taffy` | `cargo package` из DioxusLabs/taffy `2d936b7701f64b0fe95e50f1bfdcfc65a9f88f6f` (manifest version 0.14.0) |
| `html5ever` | crates.io `html5ever` 0.40.1 |
| `markup5ever_rcdom` | crates.io `markup5ever_rcdom` 0.39.0+unofficial |
| `resvg`, `usvg` | crates.io 0.48.1 с заменой `src/` из linebender/resvg commit `77b12f4951a7b75d759c42e8c9497cff7c839c3c` |

Taffy с версией manifest 0.14.0 уже содержит изменения upstream после релиза.
Релизный архив 0.14.0 не является базой этого форка. Для подготовки базы
checkout указанного commit, затем `cargo package --allow-dirty --no-verify`;
распаковать полученный `.crate`, проверить `git.sha1` в `.cargo_vcs_info.json`.
Не запускать обновление зависимостей при подготовке базы.

Архив `.crate` нужен целиком: нормализованный Cargo manifest, lockfile и
упаковка отличаются от Git checkout. Для SVG сначала распаковать оба
архива 0.48.1, затем заменить их `src/` каталогами `crates/resvg/src/` и
`crates/usvg/src/` указанного commit. Обновлять resvg и usvg вместе:
рендерер использует варианты enum и поля дерева парного парсера.

Полный патч также фиксирует состав принятого пакета: например, в GPUI-связке
архивные `Cargo.lock` удалены, поскольку приложение использует корневой
lockfile. Эти отличия входят в восстановление vendor-дерева вместе с кодом;
временный lockfile отдельного тестового запуска туда попадать не должен.

HTML-связка также обновляется совместно: локальный rcdom из архива
0.39.0+unofficial переведён на `html5ever`, `markup5ever` и `xml5ever` 0.40.
Версия самого rcdom сохранена; она не означает совместимость с интерфейсами
markup5ever 0.39. Проверять manifests и `Cargo.lock`, чтобы DOM и парсер
использовали один тип `TreeSink` из markup5ever 0.40.

GPUI, Windows backend и Taffy также образуют согласованный набор: измерение
текста и baseline передаются в layout, а структуры paint primitives должны
совпадать с Windows renderer и HLSL shaders. Обновление одного крейта требует
проверки всей связки. `gpui-base` и `gpui-component` используют расширенные
типы текста GPUI; проверять их вместе с IDE, а не только с HTML runner.
Windows-изменения DirectWrite/DirectX не являются переносимым backend.

## Политика изменений

1. Исправления общего назначения сначала предлагать upstream. В локальном
   патче записывать причину и ссылку на upstream issue/PR, когда она есть.
   При обновлении удалять локальный патч, уже принятый upstream.
2. Каждый vendored-крейт имеет один полный `vendor/<crate>-kaminide.patch`.
   Он содержит нумерованный список исправлений с причинами, изменения
   manifest, добавления и удаления файлов. Это разница с указанной чистой
   базой, а не `git diff` последнего рабочего изменения.
3. Локальные участки помечать `KaminIDE patch:` и объяснять необходимость
   изменения. Для целиком нового модуля использовать содержательный `//!`
   заголовок. Маркеры помогают при ревью, но воспроизводимость проверяется
   применением полного патча, а не количеством комментариев.
4. Сохранять upstream-форматирование. Обычный `cargo fmt` форматирует
   workspace `crates/*`; явный список каталогов `workspace.exclude` предотвращает
   автоматическое включение vendored path-зависимостей в его члены.
   Не запускать массовый rustfmt по `vendor/` и не добавлять его в workspace.
   Форматировать только изменённые участки с настройками upstream.

Тестировать исключённый крейт через `cargo test --manifest-path
vendor/<crate>/Cargo.toml`: запуск `cargo test -p <crate>` из workspace не
подключает его dev-dependencies. При отдельном manifest явно передавать
корневые `[patch.crates-io]` через Cargo `--config`, чтобы тесты использовали
согласованный набор локальных форков; родительские manifest patches не
наследуются. Для HTML-связки дополнительно передавать patches для html5ever
и markup5ever_rcdom: приложение подключает их напрямую через paths.
Не сохранять побочные изменения lockfile отдельного запуска.

Features приложения также не наследуются из корневого workspace при таком
запуске. Для тестов самого GPUI включать `--features stacker,test-support`: второй
feature подключает upstream proptest и FakeHttpClient. Для автономных
тестов Windows backend также включать его `test-support`, чтобы его
тестовый `PlatformWindow` совпадал с интерфейсом GPUI. Для base/component/windows
добавлять `--features gpui/stacker`: локальная рекурсивная раскладка использует
StackSafe. Для component передавать также список tree-sitter features из
`workspace.dependencies.gpui-component`, иначе проверки подсветки могут не
попасть в сборку.

## Taffy: поддерживаемый форк

Taffy — поддерживаемый форк, а не временная замена до следующего релиза.
Общие исправления и расширения сначала предлагать upstream отдельными
PR с минимальными примерами и тестами. Это не требует ждать принятия PR,
чтобы исправить локальную корректность. Специфичные для движка адаптеры
оставлять локально; общие механизмы layout не считать специфичными только
потому, что первым потребителем оказался KaminIDE.

Синхронизация Taffy — полный трёхсторонний импорт по процедуре ниже:
старая чистая база, весь текущий форк и новая чистая база. Разбирать результат
семантическими группами: baselines; float/block; flex/grid intrinsic sizing
и calc; оптимизации; остальные изменения API, упаковки и инструментов.
Для каждой группы проверять замыкание зависимостей: вызывающие функции,
структуры данных, кеши, новые локальные модули и потребителей в GPUI.
Пересечение списков изменённых upstream и локальных файлов не ограничивает
объём проверки. Изменение pin завершает импорт всего выбранного upstream
среза; не объявлять частичный набор перенесённых исправлений полной
синхронизацией.

Для каждого upstream-изменения фиксировать решение: взять, адаптировать,
отклонить по спецификации либо удалить доказанно эквивалентный локальный
дубль. Расхождение решается по применимой спецификации CSS: записывать
раздел и ссылку, минимальный тест, сравнение механизмов Blink, Gecko и Servo,
upstream/CSSWG issue при наличии и условие пересмотра. Совпадение браузеров
служит данными об интероперабельности, но само по себе не отменяет норму.
Не удалять расширенные локальные контракты ради совпадения с upstream API.

Контракт Taffy проверять совместно с `vendor/gpui/src/taffy.rs` и
`vendor/gpui/src/taffy/`: reference box и координаты baseline, first/last,
обе физические оси, relative offsets, измерение текста и масштаб,
intrinsic probes и кеш полного layout, calc handles и detailed layout info.
Для переноса поведения требуются тесты Taffy, затронутого GPUI-адаптера и
соответствующие WPT-семейства; оптимизации должны сохранять геометрию
и метаданные измерения. Политика разрывов, continuation tokens и fragment
tree принадлежат движку. При доказанной необходимости допустимы общие
solver hooks без зависимостей Taffy от DOM или GPUI; одна готовая геометрия
ещё не доказывает полноту поддержки фрагментации.

`vendor/taffy-kaminide.patch` остаётся полной воспроизводимой разницей
с закреплённой чистой базой и provenance форка. Его round-trip проверяет
`vendor-patches` в `.github/workflows/pr-checks.yml` через
`python vendor/check-patches.py`; отдельный `vendor-taffy` проверяет clippy
и тесты Taffy. Round-trip подтверждает восстановление дерева, а корректность
поведения подтверждают тесты и совместная проверка с GPUI. Шапку патча
генерирует список `FIXES` в `vendor/regenerate-patch.py`: темы и их группы
менять там при регенерации, затем запускать проверку, а не редактировать
сгенерированную шапку отдельно.

## Обновление через трёхстороннее слияние

Сохранить три дерева: старый чистый upstream (base), текущий vendor (ours),
новый чистый upstream (theirs). Импортировать их в отдельный временный Git
репозиторий: base как общий предок, ours и theirs как две ветки, затем слить
theirs в ours. Так Git видит исходную базу, удаления и добавления файлов.
Не заменять конфликтующий файл одной стороной целиком. Проверить каждый
локальный механизм: upstream мог исправить ту же проблему иначе.

Перенести результат в `vendor/<crate>/`, обновить pin в manifests и таблице
выше, а также `PINS` и причины в `vendor/regenerate-patch.py`. Удалить
устаревшие обходы upstream и регенерировать патч относительно **новой**
чистой базы. Приложить unit tests для изменений поведения; проверить
тесты затронутых vendor-крейтов, HTML, clippy, workspace check и WPT.

## Регенерация и проверка

Из корня репозитория, с чистым распакованным upstream-крейтом вне worktree:

```sh
python vendor/regenerate-patch.py gpui /tmp/upstream/gpui-pre-0.3.8
python vendor/regenerate-patch.py gpui /tmp/upstream/gpui-pre-0.3.8 --check
python vendor/regenerate-patch.py taffy /tmp/upstream/taffy-2d936b7
```

`python vendor/check-patches.py [crate ...]` сам собирает чистые базы из
закреплённых источников (архивы crates.io со сверкой SHA-256, Git-источники по
полному commit id, `cargo package` Taffy toolchain из `rust-toolchain.toml`) и
выполняет `--check` для всех крейтов; CI запускает его при изменениях
`vendor/`. Lockfile `cargo package` зависит от даты registry, поэтому для Taffy
он восстанавливается обратным применением секции `Cargo.lock` из патча; все
остальные файлы базы проверяются независимо.

Повторить для каждого изменённого крейта. `--scratch` задаёт внешний каталог
для временного дерева проверки. Генератор нормализует CRLF в LF, учитывает
все tracked и неигнорируемые новые файлы крейта, проверяет применение патча
к копии upstream и побайтовое совпадение результата после нормализации.
Он не изменяет upstream. Для Taffy и gpui-kit генератор также проверяет
commit из `.cargo_vcs_info.json` и отвергает другой snapshot. Бинарные изменения записываются как Git binary patch; round-trip также
проверяет точные байты этих ресурсов. Текстовые файлы с trailing whitespace
в любой стороне также сохраняются как binary patch: это сохраняет upstream
байты, не внося trailing whitespace в сам патч.

Патчи имеют нулевой контекст, чтобы пустые строки контекста не создавали
trailing whitespace. Применять из корня дерева с подготовленным vendor:

```sh
git apply --check --unidiff-zero vendor/gpui-kaminide.patch
git apply --unidiff-zero vendor/gpui-kaminide.patch
```

Для resvg/usvg порядок подготовки фиксирован: архивы → upstream `src/` →
`usvg-kaminide.patch` → `resvg-kaminide.patch`. Два последних патча меняют
разные каталоги и применяются однократно. Старый `resvg-kaminide.patch`
содержал последовательные изменения обоих крейтов; его нельзя применять
вместе с новыми патчами. Новый файл описывает только resvg, полный usvg
патч находится рядом.

`scripts/vendor_patches.py --export` экспортирует только незакоммиченный
`git diff` и использует исторический реестр путей. Для восстановления
форков и их обновления использовать полный генератор выше.

## Тестовые ресурсы

`vendor/gpui/tests/fonts/` содержит неизменённые шрифты и лицензии из
точного snapshot Zed для SVG regression tests; ревизия и исходные пути
записаны в README рядом. Генератор сохраняет их как Git binary patch.

### gpui-component

Опубликованный crate ссылается на ресурсы upstream-монорепозитория,
которые не входят в архив. Неизменённые иконки, markdown и тема сохранены
в `vendor/gpui-component/tests/fixtures/`; их исходные пути и точная ревизия
перечислены в README рядом. Тест экспорта dock читает парный
`vendor/gpui-base/src/dock/mod.rs`. При обновлении сверять эти ресурсы
с ревизией `.cargo_vcs_info.json`, сохраняя сами проверки.

## Поддерживаемые диагностические переменные

| Переменная | Потребитель и назначение |
| --- | --- |
| `WPT_FIRST_FRAME=1` | `crates/html/examples/wptrun`: снимок первого GPU-кадра документа без ожидания сходимости; диагностический режим, CI его не включает |
| `KAMIN_FORCE_SCALE` | `crates/html/examples/wptrun.rs`: стабильный scale 1 для WPT; Windows display читает положительный scale |
| `KAMIN_PREPAINT_PROF=1` | профилировщик GPUI и `crates/shell/src/web/diag*.rs`: время prepaint по типам элементов |
| `KAMIN_DIAG_GPU_TIMING=1` | Windows batch-cost timestamps; `gpui::batch_cost::take_line()` читается диагностикой shell |
| `KAMIN_DEBUG_D3D_DEVICE=warp\|d3d11on12` | `vendor/gpui-windows/src/debug_device.rs`, только debug-сборки: устройство WARP или D3D11On12 для native acceptance (`scripts/start_native_acceptance.ps1`, `docs/NATIVE_ACCEPTANCE_SEAMS.md`); release переменную не читает |

`ORD_DBG`, `VERT_DBG` и `IMG_DBG` были локальными печатными пробами без
потребителей и удалены. `GPUI_NATIVE_TREE` (печать дерева Taffy в
stderr) удалён по той же причине. Новая диагностическая переменная требует реального
потребителя и записи в этой таблице.
