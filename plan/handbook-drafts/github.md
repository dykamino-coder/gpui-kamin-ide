# GitHub/crates.io: готовые решения и приёмы для мини HTML/CSS-движка

Контекст: retained-рендер поверх GPU-фреймворка (сцена примитивов), taffy-подобный flexbox, свой CSS-парсер, свой line breaker, DirectWrite на Windows, цель — WPT reftests. Приоритет — скорость разработки и скорость рантайма.

---

## 1. Blitz / dioxus-native (DioxusLabs/blitz)

**Что это.** «Radically modular HTML/CSS rendering engine» на Rust: Stylo (каскад/селекторы) + Taffy (box layout) + Parley (текст) + Vello (2D GPU через абстракцию AnyRender). Рендерит Wikipedia, old Reddit. Нет JS-движка — по сути тот же класс движка, что и наш.

**Лицензия.** Apache-2.0 / MIT (dual). Исключение: крейт `stylo_taffy` дополнительно MPL-2.0 (интероп с Servo).

**Зрелость.** Beta; 0.1 Alpha на crates.io — июнь 2025 (18 месяцев работы). Активная разработка, NLnet-грант. WPT-инфраструктура есть: репо `DioxusLabs/blitz-wpt-results` (обновлялось апрель 2026), статус-страница blitz.is/status/wpt.

**WPT-цифры Blitz (эталон достижимого без JS):** overall interop ~47.5% (12 242/26 038 тестов), subtests 48.1%. Топ: css-variables 97.8%, WOFF2 94.9%, css-style-attr 93.7%, css-ui 89.5%, css-color 90.5%. Провалы: css-counter-styles 3.3%, css-gaps 6.8%, css-shadow 7.0%, css-content 9.3%, cssom 8.3%. Важно: без JS доступно только ~20% subtests — прямая калибровка ожиданий для нашего стенда.

**ЧТО брать.**
- **Архитектурный образец №1.** `blitz-dom` — готовый пример, как навесить Stylo-трейты (TElement/TNode/TDocument) на собственный DOM и как склеить Stylo-стили с Taffy-стилями (крейт `stylo_taffy` — конвертер computed values → taffy Style; можно брать целиком или портировать, MPL-2.0 допускает статическую линковку без заражения).
- `blitz-html` (html5ever-обвязка), `blitz-net` — переиспользуемы отдельно, но у нас парсер уже есть.
- **WPT-раннер без JS**: их подход к прогону только runnable-тестов и подсчёту относительных процентов — копировать методику для нашего стенда.

**Риски.** Bleeding edge, API ломается; тянет весь Stylo (большая компиляция). Если не берём Stylo — ценность Blitz в основном как reference/методика, не как код.

---

## 2. Linebender-стек: parley, fontique, skrifa, harfrust, swash, vello

**parley** (rich text layout) — Apache-2.0/MIT. Версия 0.11.0 (июнь 2026) — активно живёт. Четыре ключевые зависимости: **fontique** (font enumeration + fallback), **harfrust** (шейпинг), **skrifa** (чтение TrueType/OpenType + scaled/hinted outlines), **ICU4X** (сегментация/bidi). Умеет inline boxes (place-холдеры под инлайновые виджеты/картинки — ровно то, что нужно inline-layout HTML). Это текстовый стек Blitz.

**harfrust** — порт HarfBuzz на Rust (репо harfbuzz/harfrust, поддерживается самими HarfBuzz-людьми). Заменил swash-шейпинг в parley. НЕ имеет интеграции с DirectWrite/CoreText — чистый шейпер по данным шрифта. Для нас: шейпинг делаем harfrust, а enumeration/fallback — fontique (у которого есть системный бекенд, на Windows через DWrite/GDI-реестр шрифтов) либо наш DirectWrite.

**skrifa** (из googlefonts/fontations) — зрелый (0.43.x), это «новый freetype» Google Fonts; glyph outlines, hinting, метрики, COLR. Брать целиком для метрик/аутлайнов, если хотим уйти от DirectWrite-растеризации или нужен кроссплатформенный путь.

**swash** — предыдущее поколение (shaping+scaling); ещё жив (cosmic-text на нём), но вектор развития Linebender — harfrust+skrifa. Новый код лучше строить не на swash.

**vello** — GPU 2D (compute-shader rasterizer). Для нас нерелевантен (свой GPU-рендер сцены примитивов), но их glyph-pipeline и damage-подходы можно подсматривать.

**ЧТО брать.**
- **fontique целиком** — самый дешёвый способ получить корректный font fallback/matching (family matching по CSS-правилам, script-based fallback), вместо ручной возни с IDWriteFontFallback. Совместим со своим шейпером.
- **harfrust целиком** как шейпер (вместо/в дополнение к DirectWrite shaping) — детерминированный, кроссплатформенный, WPT-тексты будут стабильнее.
- **parley: порт алгоритма, не крейт**, если line breaker свой: смотреть их разбивку на runs (style runs × script runs × font fallback runs × bidi runs), greedy line breaking с ICU4X segmenter, и их кэш шейпинга. Если line breaker не принципиален — можно взять parley целиком как inline-layout и сэкономить месяцы.

**Риски.** Агрессивный MSRV (бампают в патчах); parley API до 1.0 нестабилен; ICU4X — заметный вес в бинаре (данные сегментации).

---

## 3. cosmic-text (pop-os)

**Что это.** Текстовый стек «всё-в-одном»: шейпинг (rustybuzz/harfrust-класс), layout, font fallback, редактирование, растеризация (swash). MIT/Apache-2.0. Зрелый — production в COSMIC desktop, iced, zed-ранних версиях.

**Сравнение с parley (по обсуждениям 2025, egui PR #5784, bevy #21765):**
- parley быстрее в shaping+layout (зависит от длины текста/числа span'ов);
- parley имеет inline boxes — cosmic-text нет (критично для HTML inline layout);
- cosmic-text слабее документирован; API Buffer-центричный, заточен под текстовые виджеты/редакторы, не под HTML-поток;
- общие зависимости у обоих (harfrust, swash, skrifa) — по качеству шейпинга паритет.

**ЧТО брать.** Для HTML-движка — **parley предпочтительнее**. У cosmic-text полезно подсматривать: их shape run cache и glyph cache (простая, читаемая реализация кэшей — см. §8), обработку fallback-цепочек. Крейт целиком брать не стоит: нет inline boxes, придётся костылять.

---

## 4. Stylo (servo/stylo) как отдельный крейт

**Что это.** CSS-движок Firefox/Servo: парсинг CSS, каскад, селекторы, параллельный restyle. С 2024 публикуется на crates.io отдельными крейтами (`stylo`, `selectors`, `cssparser`, `servo_arc`, `stylo_atoms`...). MPL-2.0.

**Реально ли встроить только каскад/селекторы.** Да — Blitz это доказал (blitz-dom — рабочий пример интеграции). Механика: реализуешь трейты TDocument/TElement/TNode + SelectorImpl на своём DOM, получаешь весь каскад, specificity, инвалидацию, style sharing, CSS variables, media queries. Но: стоимость — большой компайл-тайм, MPL-2.0 (file-level copyleft — ок для проприетарного продукта при статической линковке, изменённые файлы Stylo надо публиковать), громоздкие трейты, computed values в формате Stylo (нужен конвертер как stylo_taffy).

**Модульные куски (Apache/MIT-совместимые, легче всего брать):**
- **`cssparser`** (MPL-2.0) — токенайзер/парсер CSS-синтаксиса, battle-tested; наш «свой CSS-парсер» может остаться, но cssparser снимает весь класс багов токенизации (escapes, an+b, unicode-range).
- **`selectors`** (MPL-2.0) — парсинг и матчинг селекторов С Bloom-фильтром и nth-index cache в комплекте. Самый выгодный отдельный кусок: реализуем `Element`-трейт (~20 методов) на своём DOM — получаем корректный матчинг всей селекторной грамматики + fast-reject. Это, вероятно, лучшая инвестиция из всего списка при своём каскаде.
- `servo_arc` — Arc без weak, с thin-указателями (микрооптимизация, брать по желанию).

**Вердикт.** Два пути: (а) весь Stylo по образцу Blitz — максимум WPT-корректности каскада за минимум своего кода, ценой компайла и жёсткой архитектуры; (б) `selectors` + `cssparser` + свой каскад — сохраняем контроль/размер, получаем корректный матчинг. Для «маленького движка» рекомендую (б), с апгрейдом до (а) если каскад станет ботлнеком по WPT.

---

## 5. Taffy: свежие версии

MIT/Apache-2.0 (частично Yoga-производный код). Активен, тот же мейнтейнер (nicoburns), что и Blitz; Taffy лёг в основу CSS Grid в самом Servo (PR servo#32619) — знак качества.

**Хронология фич (CHANGELOG):**
- 0.6 — block layout, box-sizing, «traitification» Style (LayoutPartialTree — можно подключать свой стор стилей без копирования в taffy Style).
- 0.7.x — detailed grid info (доступ к вычисленным трекам).
- 0.8 — **calc() в low-level API**: calc-значения как opaque pointer, резолвит колбек хоста — идеально для своего CSS-парсера (мы храним своё calc-дерево, taffy дергает резолвер).
- 0.9 — named grid lines/areas; Style generic над строками.
- 0.10 — direction/RTL, **float и clear**, парсинг CSS-строк (`parse` feature).
- 0.11 — safe alignment keywords.
- 0.12 — align-content для block, переработанный layout cache (корректность).
- 0.13 (текущая стабильная) — display: flow-root, self-start/self-end, пачка фиксов grid auto-placement/margin collapsing.
- unreleased — flexbox balance (Flexbox L2), containment.

**Grid intrinsic sizing** — рабочий, но с открытыми issues по распределению intrinsic contributions/growth limits (0.9.1 чинил grid placement после регрессии — bevy#21672).

**Перф.** Свежие версии переработали кэширование (0.12); известная боль — число measure-вызовов на кадр (наша память: 3100–4700 замеров/кадр в gpui-порте). Лечится: LayoutPartialTree + собственный measure-кэш + cache-slots taffy.

**ЧТО брать.** Если наш flexbox «taffy-подобный» — рассмотреть **замену на настоящий taffy 0.13+**: получаем block layout, float/clear, grid, calc-хук — всё нужное для WPT css-flexbox/css-grid бесплатно, тесты taffy генерируются из Chrome (gentest) — то есть его поведение уже сверено с браузером. Форки смотреть не нужно — upstream живее всех форков.

**Риски.** Наши vendored-патчи (см. память reference_gpui_vendor_patches) придётся переносить; float в taffy молодой.

---

## 6. Быстрые техники: bloom filter, invalidation sets, style sharing

- **Bloom filter (WebKit-приём).** Готовая переносимая реализация — в крейте `selectors`: counting/non-counting bloom filters, tuned под ancestor filter, + функция матчинга с fast-reject. Если селекторный матчинг свой — портировать их `bloom.rs` (один файл, MPL) или переписать по нему (алгоритм тривиален: хэши tag/id/class/attr предков, селектор проверяет свои правые compound-части против фильтра до полного матчинга). Есть и независимый пример в крейте `css` (модуль selectors::bloom).
- **Style sharing cache (Stylo).** Реализация внутри `stylo` (модуль sharing): кольцо последних N (31) стилёванных элементов, кандидат делит стиль если совпали tag/class/attrs/state и «revalidation selectors». Мини-реализации отдельным крейтом НЕТ — портировать идею: для reftests это главный ускоритель повторяющихся DOM-структур. Порт средней сложности; отложить до появления перф-проблемы.
- **Invalidation sets (Blink) / Stylo invalidation.** Переносимой мини-реализации нет; Stylo-вариант вшит в их restyle. Для reftest-движка (статические страницы, полный restyle одноразовый) инвалидация НЕ нужна — не тратить время. Понадобится при динамике: тогда читать stylo `invalidation/` (MPL) и Blink design doc «Invalidation Sets».

Приоритет: bloom filter (дёшево, всегда полезен) > style sharing (по необходимости) > invalidation sets (не сейчас).

---

## 7. Движки-доноры алгоритмов

- **litehtml** (C++, **New BSD** — читать и портировать легально). «document_container»-архитектура: движок не рисует и не трогает шрифты сам — всё через интерфейс хоста; идентично нашей схеме с GPU-сценой. Зрелый (используется в почтовиках/JUCE-портах), CSS2/частично CSS3, gumbo-парсер (Apache-2.0). WPT не гоняет — точность ниже нашей цели. Полезен как донор простых алгоритмов (table layout, borders collapse) когда спека мутна, но код местами «до-спековый». Средний приоритет.
- **Ultralight** — закрытый (форк WebKit). Только идеи из маркетинга/доков: CPU+GPU dual renderer, маленький footprint. Кода нет — практической ценности нет.
- **Ladybird / LibWeb** (C++, **BSD-2-Clause** — можно читать И портировать с копирайт-нотисом). Главный донор алгоритмов: код написан «по спеке» с комментариями-цитатами шагов спеки, читается как учебник; WPT — 2.07 млн passing subtests (апрель 2026), т.е. алгоритмы проверены. Портировать точечно: их `Layout/` (BFC/IFC, line builder, table fixup), `CSS/` (StyleComputer — каскад без Stylo-масштаба). **Лучший источник для «как правильно реализовать X по спеке»**, выше по ценности, чем litehtml.
- **servo/servo** (MPL-2.0) — читать можно, портировать = MPL-обязательства на файлы. Ценен layout2020 (fragment tree, инкрементальность) как архитектурный reference. С апреля 2026 servo 0.1.0 на crates.io — но встраивать целиком это уже «не маленький движок».
- **yoga** (facebook, MIT) — только flexbox, taffy исторически из него вырос и давно обогнал по CSS-соответствию. Не нужен.

---

## 8. Перф-приёмы: кэши, атласы, damage

**Шейпинг/measure-кэши.**
- cosmic-text: `ShapeRunCache` — ключ (текст run'а + attrs), значение — отшейпленные глифы; LRU по «возрасту». Простая читаемая реализация — портируется за день.
- parley: кэширует на уровне layout context (переиспользование аллокаций) + шейп-кэш; смотреть их `shape.rs`.
- Наш аналог: кэш measure для taffy-measure-функций с ключом (run text, font, size, width-bucket) — «багет-кэш»: квантовать available width по бакетам, иначе кэш не попадает при каждом ресайзе.

**Glyph atlas.**
- **etagere** (nical, MIT/Apache) — shelf-packing аллокатор, написан для WebRender ИМЕННО под глифы («high number of items with similar sizes»); брать целиком, это стандарт де-факто. Статья nical «Improving texture atlas allocation in WebRender» — обязательное чтение.
- **guillotiere** (nical) — guillotine-алгоритм с fast dealloc + rectangle coalescing; лучше для разнокалиберных картинок/слоёв, хуже для глифов. Брать вторым атласом под изображения, если нужно.

**Damage tracking.** Готового крейта нет. Практичные схемы: (а) WebRender: retained display list + picture caching (тайлы, инвалидация по diff display items) — тяжело; (б) для reftests не нужно вовсе — кадр статичен; (в) для IDE-embed достаточно dirty-rect на уровне «стиль/лейаут изменился у поддерева → union bounding box» — идея из litehtml/старого WebKit, кода на 100 строк.

---

## Сводные рекомендации (по ROI)

1. **etagere** — взять целиком, сразу. Нулевой риск.
2. **taffy 0.13+ вместо самописного flexbox-клона** — block+float+grid+calc-хук, тесты сгенерены из Chrome. Крупнейший выигрыш по WPT-скорости.
3. **selectors (+cssparser)** — корректный селекторный матчинг + bloom filter бесплатно; свой каскад оставить.
4. **fontique + harfrust** — fallback и шейпинг; DirectWrite оставить для растеризации/системных шрифтов или заменить skrifa.
5. **Ladybird LibWeb** — настольная книга алгоритмов (BSD-2, портируемо): line builder, tables, каскад.
6. **Blitz** — методика WPT-стенда без JS + образец Stylo/Taffy-склейки; их 47.5% — реалистичная планка.
7. **cosmic-text ShapeRunCache** — образец кэша шейпинга; сам крейт не брать.
8. Bloom filter сейчас; style sharing по нужде; invalidation sets — не для reftests.

**Лицензии, кратко:** MIT/Apache — taffy, parley/fontique/harfrust/skrifa, etagere, cosmic-text, yoga. MPL-2.0 (file-copyleft, статически линкуемо) — stylo, selectors, cssparser, servo. BSD — litehtml (New BSD), Ladybird (BSD-2). Закрыто — Ultralight.

*Источники: github.com/DioxusLabs/blitz, blitz.is/status/wpt, github.com/DioxusLabs/taffy (CHANGELOG), docs.rs/selectors, github.com/servo/stylo, github.com/linebender/parley, github.com/harfbuzz/harfrust, github.com/pop-os/cosmic-text, github.com/litehtml/litehtml, github.com/LadybirdBrowser/ladybird, ladybird.org/newsletter/2026-04-30, nical.github.io/posts/etagere.html, servo.org/blog/2026/04/13/servo-0.1.0-release, egui#5784, bevy#21765, servo#32619.*
