# Конспект: движки CSS-раскладки (layout engines)

Обзор литературы по архитектуре layout-движков: Chromium LayoutNG, Servo Layout 2020, Taffy, спековые алгоритмы (BFC, flexbox, grid, css-grid-3 / grid-lanes), Ladybird LibWeb, инкрементальный relayout. Дата среза: август 2026.

---

## 1. Chromium LayoutNG

### Ключевые концепции

1. **Immutable fragment tree.** Legacy Blink мутировал layout tree in-place (вход и выход layout жили в одних объектах). LayoutNG оставил layout tree лишь как держатель входов/выходов, а результат layout — совершенно новый **иммутабельный** fragment tree (`PhysicalFragment`). Иммутабельность позволяет переиспользовать большие поддеревья предыдущего дерева при инкрементальном layout и убирает целый класс багов «hysteresis» (результат зависит от предыдущего состояния).

2. **Constraint space.** Формализованный объект «все входы от родителя к ребёнку»: available size, установлен ли новый formatting context, промежуточное состояние margin collapsing от предыдущего контента, позиции float'ов, для фрагментации — block-size fragmentainer'а и текущий block offset (где ломать). Инвариант: алгоритм layout не имеет права читать ничего, кроме кортежа (node, style, children, constraint space) — «The current layout should not access any information outside this set, this will break invariants in the system».

3. **NGLayoutResult / кэширование.** Constraint space, породивший фрагмент, хранится рядом с результатом и служит **cache key**. Результат переиспользуется, если: constraint space идентичен, нет break token, узел не помечен dirty. `CachedLayoutResult()` клонирует фрагмент без offset'а — родитель позиционирует его заново.

4. **Физические координаты на выходе.** Фрагменты хранят чисто физические left/top offset'ы от родителя; все writing modes / direction разрешаются во время layout. Внутри алгоритмов — логическая геометрия, на выходе — физическая.

5. **Min/max sizing (measure pass).** Intrinsic sizes (min-content / max-content) считаются отдельным проходом и кэшируются отдельно от layout pass. Legacy имел экспоненциальную сложность O(2^n) на вложенных flex/grid из-за повторных measure+layout проходов; явное кэширование measure- и layout-результатов в LayoutNG вернуло линейность.

6. **Break tokens и фрагментация.** Multicol/печать реализованы как естественная часть layout: узел может выдать несколько фрагментов, между ними — break token (сколько контента уже уложено), а не пост-обработка. Constraint space знает геометрию fragmentainer'а.

7. **Inline layout** переписан на иммутабельный плоский список (items) с кэшированием на уровне параграфа, шейпингом на весь параграф и корректным bidi.

8. **Четыре класса багов, которые чинила архитектура:** correctness (чистые контракты компонентов), under-invalidation (централизованный diff «нужен ли relayout»), hysteresis (идемпотентность через явные входы/выходы), over-invalidation/perf (кэши).

### Ссылки
- https://developer.chrome.com/docs/chromium/layoutng — RenderingNG deep-dive: LayoutNG.
- https://developer.chrome.com/docs/chromium/renderingng-fragmentation — deep-dive: block fragmentation.
- https://chromium.googlesource.com/chromium/src/third_party/+/refs/heads/main/blink/renderer/core/layout/layout_ng.md — README LayoutNG в дереве исходников.
- Design docs (из README): 
  - https://docs.google.com/document/d/1uxbDh4uONFQOiGuiumlJBLGgO4KDWB8ZEkp7Rd47fw4/edit
  - https://docs.google.com/document/d/1RjH_Ofa8O_ucGvaDCEgsBVECPqUTiQKR3zNyVTr-L_I/edit
  - https://docs.google.com/document/d/1EJOdFesZKspvrU7uWtGl-8ab2jIrzRF6NKJhwYOs6hU/
- https://chromium.googlesource.com/chromium/src.git/+/62.0.3178.1/third_party/WebKit/Source/core/layout/ng/inline/README.md — inline layout NG.
- https://developer.chrome.com/docs/chromium/blinkng — BlinkNG (фазы пайплайна).

### Применимо к малому Rust-движку
- Кортеж «(node, style, constraint space) → immutable result» как единственный контракт; constraint space = cache key. Taffy уже так делает (`Cache` на узле), но стоит расширить свой constraint space (percent base, доступное место, bfc-состояние) и хранить его в кэше явно.
- Разделять measure pass (intrinsic sizes) и layout pass с независимыми кэшами и независимой инвалидацией.
- Выход в физических координатах, вся логика внутри — в логических (inline/block осях): дёшево получить vertical writing modes потом.

---

## 2. Servo layout: 2013 → 2020

### Ключевые концепции

1. **Почему отказались от Layout 2013.** Он строился вокруг жёсткого разделения «многопоточный обход дерева» vs «операции на узле» (parallel traversal driver). Фичи, не ложащиеся в модель (floats, counters, фрагментация), реализовывались мучительно и криво — web-compat страдал. Код сильно отклонялся от терминологии спек.

2. **Layout 2020 — «две трети» LayoutNG-подхода: два дерева.** **Box tree** (вложенные formatting contexts, как в спеке) строится из styled DOM; layout превращает его в **fragment tree** (куски после line breaking / columns / pagination); из fragment tree строится display list для WebRender. Три фазы, чистое разделение.

3. **Типизация спеки enum'ами.** Box tree — вложенные Rust-enum'ы, гарантирующие на уровне типов, что внутри контекста может лежать только допустимый спекой контент (например, дети inline box — только inline-level).

4. **Оппортунистический параллелизм (rayon).** Вместо обязательного параллелизма на каждом уровне — параллелятся отдельные циклы (children независимых formatting contexts) через rayon. Встретили floats/counters — секция считается последовательно; за границей BFC, содержащего floats, параллелизм возобновляется. Этого хватает, чтобы насыщать ядра, не ломая семантику.

5. **Persistent-структуры для инкрементальности.** Узлы box/fragment tree — Arc, почти иммутабельные; обновление создаёт новые узлы, переиспользуя неизменённые поддеревья.

6. **Без промежуточного display list.** Дизайн после WebRender: fragment tree конвертируется прямо в WebRender display items + spatial/clip trees; скролл и transform обновляются без relayout (двигается offset spatial node).

7. **Позиционированные элементы** — placeholder-фрагменты (`AbsoluteOrFixedPositioned`) в потоке ради правильного painting order; сами укладываются от containing block.

8. **Grid в Servo — через Taffy** (PR #32619, nicoburns, 2024): интеграция стороннего раскладчика в браузерный движок через trait-интерфейс, pass rate css-grid 18.6% → 38.3%.

### Ссылки
- https://servo.org/blog/2023/04/13/layout-2013-vs-2020/ — сравнение движков и решение.
- https://github.com/servo/servo/wiki/Layout-2020 — цели и принципы дизайна.
- https://book.servo.org/design-documentation/layout.html — актуальный дизайн-док (3 фазы, rayon, floats).
- https://github.com/servo/servo/wiki/Servo-Layout-Engines-Report — детальный отчёт-сравнение.
- https://github.com/servo/servo/pull/32619 — CSS Grid через taffy.

### Применимо
- Box tree ≠ fragment tree: даже в малом движке хранить «структуру» (стили, контексты) отдельно от «результата» (позиции, куски) — это и есть предпосылка кэшей и переиспользования.
- Rayon-параллелизм только там, где независимость доказуема (дети flex-контейнера после разрешения размеров, независимые поддеревья) — не строить универсальный parallel driver.
- Enum-моделирование инвариантов спеки — дёшево в Rust и убивает класс невозможных состояний.

---

## 3. Taffy

### Ключевые концепции

1. **Два уровня API.** High-level: `TaffyTree` — своё хранилище узлов (SlotMap + generational indices), стили, кэш, dispatch. Low-level: набор trait'ов (`TraversePartialTree`, `LayoutPartialTree`, `CacheTree`, `RoundTree` + `LayoutFlexboxContainer` / `LayoutGridContainer` / `LayoutBlockContainer`) и свободные функции `compute_flexbox_layout` / `compute_grid_layout` / `compute_block_layout` / `compute_leaf_layout` / `compute_root_layout` / `compute_hidden_layout`, считающие ОДИН узел. Хранение, кэширование и диспетчеризацию по display берёт на себя встраивающий (так Taffy живёт внутри Servo, Blitz, Bevy, Zed, Lapce, Slint).

2. **Алгоритмы:** CSS Block, Flexbox, CSS Grid (feature-complete по css-grid-1, включая minmax/fr/auto-fill/auto-fit/dense), `calc()`. Grid placement внутри — occupancy matrix с поддержкой отрицательных/implicit-треков; track sizing — по §11 css-grid-1.

3. **Модель вызова:** узел получает `known_dimensions`, `available_space` (`Definite(px) | MinContent | MaxContent`), `RunMode` (`ComputeSize` — только размер, measure; `PerformLayout` — полный layout) и `SizingMode`. Это упрощённый аналог constraint space LayoutNG.

4. **Intrinsic sizing / measure functions.** Листья (текст, картинки, канвас) меряются пользовательской measure-функцией: ей передают known_dimensions + available_space, она возвращает размер. Intrinsic sizes контейнеров выводятся рекурсивными `ComputeSize`-вызовами min-/max-content.

5. **Кэш.** Per-node cache на несколько слотов (по комбинации known_dimensions/available_space/run mode), т.к. родитель может опросить ребёнка несколько раз (measure + final). `CacheTree` позволяет встраивающему хранить кэш у себя. Плюс `RoundTree` — снап к целым пикселям отдельным проходом (без накопления ошибок).

6. **Ограничения:** нет текстового layout (принципиально вне scope), нет inline formatting context, нет floats, нет таблиц, нет фрагментации; absolute positioning — в объёме, нужном flex/grid. Т.е. Taffy — «раскладчик боксов», а не движок документа.

7. **Производительность** — уровня/лучше Yoga (например, 38.6ms vs 45.8ms на 100k глубоко вложенных узлов).

### Ссылки
- https://github.com/DioxusLabs/taffy — README, benchmark'и, roadmap.
- https://docs.rs/taffy — доки: traits, compute_*, модули.
- https://github.com/DioxusLabs/taffy/issues/204 — история grid-реализации.
- https://github.com/bevyengine/bevy/pull/8026 — grid в Bevy (GridTrack/GridPlacement поверх taffy).

### Применимо
- Строить свой движок на **low-level API**: своё дерево реализует trait'ы, диспетчер по `display` свой — тогда можно добавить собственные алгоритмы (inline, grid-lanes) рядом с taffy-шными, с общим кэшем.
- `RunMode::ComputeSize` — дешёвый measure: не изобретать отдельный протокол intrinsic sizing.
- Измерение текста — своя measure-функция с кэшем шейпинга по (текст, ширина).

---

## 4. Спековые алгоритмы и их реализация

### 4.1 Block formatting context и margin collapsing

- BFC укладывает block-level детей последовательно по block-оси; смежные block-осевые margin'ы **схлопываются** (max положительных + min отрицательных): sibling-sibling, parent-first/last child (если нет border/padding/clearance между), пустые блоки схлопывают свои top+bottom.
- Схлопывание НЕ пересекает границу нового BFC (`overflow != visible`, floats, flex/grid items, contain: layout). Практика реализации (LayoutNG): в constraint space передаётся «margin strut» — накопленное множество ещё не разрешённых margin'ов; позиция ребёнка неизвестна, пока strut не разрешится (например, первым border/контентом). Это причина, почему BFC-layout принципиально последовательный (см. Servo floats).
- Floats: укладываются в BFC с exclusion-областями; line boxes и margin box'ы обтекают; `clear` сдвигает ниже float'ов. В движках это отдельное состояние BFC (float context), протаскиваемое через constraint space.
- Спека: https://www.w3.org/TR/CSS22/visuren.html#block-formatting и https://www.w3.org/TR/CSS22/box.html#collapsing-margins ; https://drafts.csswg.org/css2/#collapsing-margins

### 4.2 Flexbox (css-flexbox-1 §9)

Алгоритм из спеки, как его реализуют (taffy/LayoutNG следуют почти дословно):
1. Определить available space; посчитать **flex base size** и hypothetical main size каждого item (из flex-basis / width / content).
2. Собрать items в **flex lines** (при wrap — жадно по hypothetical size).
3. **Resolve flexible lengths** (§9.7): итеративный цикл — заморозить inflexible items, распределять free space пропорционально flex-grow/shrink (shrink — взвешенный на base size), нарушивших min/max — clamp и заморозить, повторять до стабилизации.
4. Cross size: hypothetical cross size каждого item (layout с известным main size — вот откуда «двойной проход» и экспонента без кэша), высота линии = max, `align-content` распределяет линии, `align-items/self` (+ baseline, stretch) — внутри линии.
5. `justify-content` распределяет по main-оси; auto margins съедают free space до justify.
- Спека: https://drafts.csswg.org/css-flexbox-1/#layout-algorithm
- Гоча реализации: п.4 требует relayout ребёнка со stretch-высотой — обязателен кэш measure vs final (LayoutNG measure/layout passes, taffy cache slots).

### 4.3 Grid track sizing (css-grid-1 §11)

1. Placement (§8.5): explicit placement → авто-размещение курсором («sparse» по умолчанию, `dense` — с начала каждый раз), implicit-треки создаются по мере надобности.
2. `repeat(auto-fill/auto-fit)` разворачивается до sizing по available size; auto-fit коллапсирует пустые треки.
3. **Track sizing algorithm** (§11): каждый трек имеет base size и growth limit из min/max track sizing function (`minmax()`, fr, auto, min-/max-content, длины/проценты).
   - Init: base = 0/длина, limit = ∞/длина.
   - **Resolve intrinsic sizes**: вклад items span=1, затем span=2… (spanning items распределяют вклад по пересекаемым intrinsic-трекам — «distribute extra space»), с учётом min-content/max-content contribution.
   - **Maximize tracks**: раздать free space до growth limits.
   - **Expand flexible tracks (fr)**: find the size of an fr — пропорционально долям, с re-run при нарушении min.
   - **Stretch auto tracks** (align/justify-content: stretch).
   - Алгоритм гоняется до 2 раз на ось (перекрёстная зависимость осей: сначала columns, потом rows, при необходимости re-resolve columns).
- Спека: https://drafts.csswg.org/css-grid-1/#layout-algorithm (§11 — https://drafts.csswg.org/css-grid-1/#algo-track-sizing ).
- Taffy реализует именно этот алгоритм; интеграция в Servo (PR #32619) показала главные сложности: проценты/indefinite sizes, baseline, subgrid (css-grid-2, в taffy нет).

### 4.4 CSS Grid Level 3: masonry → grid-lanes (состояние 2025–2026)

История и статус:
- Дебат синтаксиса: WebKit — «masonry = display:grid» (Firefox так с 2020 за флагом), Chrome — отдельный `display: masonry` (в Chromium 140 экспериментально). 2024-11 — обострение спора; **31.01.2025 CSSWG резолюция: «Re-use grid templating and placement properties for masonry layout»**, нейтральное имя — **`display: grid-lanes`** (+ `inline-grid-lanes`). Итоговая модель: отдельный display-тип, но переиспользующий grid-свойства (шаблоны треков, placement, gap, alignment).
- Editor's draft активно меняется (текущая редакция — май 2026, unpublished WIP): `item-tolerance` переименован в `flow-tolerance`; семейство `item-*` (item-flow / item-direction / item-pack / item-slack) как единые flow-контролы для flex/grid/masonry — предложено и затем **выкинуто** из grid-3 (осталось отдельным эскизом css-display-4 / item-flow); добавлен `grid-lanes-direction`.
- Реализации: Safari Technology Preview 234 — «CSS Grid Lanes» (самая полная), WWDC26 сессия; Chromium/Firefox переделывают свои прототипы под резолюцию. Native masonry по-прежнему experimental во всех движках.

Модель:
1. **Оси.** Одна ось — **grid axis** (полноценные grid-треки: repeat/auto-fill/minmax/fr), другая — **stacking axis** (items складываются свободно, без строк). Ориентация определяется тем, какой grid-template-* задан: `grid-template-columns` → колонки-«lanes», поток вниз (waterfall); `grid-template-rows` → строки, поток вбок (bricks). У `grid-auto-flow` появилось значение `normal` для авто-определения.
2. **Placement.** Для каждого трека — **running position**. Definite-placed items идут по explicit-позиции; auto-placed — в «самый короткий» трек: item ставится на max(running positions пересекаемых треков), после укладки running positions обновляются. Спан — `grid-column: span N`, негативные индексы работают.
3. **`flow-tolerance`** (бывш. item-tolerance): `normal` (=1em) | `<length-percentage>` | `infinite`. Треки, чьи running positions отличаются меньше порога, считаются «равными» — тогда заполняются по порядку документа, а не прыжками; `infinite` = строгий порядок. Убирает визуальный шум от мелких разниц высот.
4. **`grid-lanes-direction`**: `normal | [ row | column ] [ fill-reverse || track-reverse ]?`. Управляет: какой из «равных» треков выбирается (track-reverse — с конца), и в каком направлении items заполняют трек (fill-reverse — снизу/справа). Waterfall = column, bricks = row.
5. **Track sizing с неопределённым размещением.** Ключевое отличие от grid-1: авто-размещаемый item может попасть в ЛЮБОЙ трек, поэтому **все auto-placed items вносят вклад в sizing всех треков** (spanning items — во все возможные позиции). Только после sizing выполняется placement. Это делает intrinsic-треки дорогими (O(items × tracks) вкладов) — практический совет спеки: использовать minmax(длина, fr).
6. **Dense packing**: backfill дырок ограничен — надёжно работает при равных track-span'ах; спека помечает часть поведений TBD.
7. **Alignment.** Grid axis — обычные grid-выравнивания. Stacking axis: `align-content/justify-content` двигают весь «столбец» контента как единое целое (по max running position); self-alignment применим только к items у краёв/зазоров; baseline в stacking axis не поддержан.
8. **Subgrid** в stacking axis невозможен — превращается во вложенный grid-lanes контейнер.
9. **Graceful degradation**: в неподдерживающих браузерах grid-lanes деградирует до обычного grid (с бо́льшими дырами) — сознательное свойство дизайна.

Ссылки:
- https://drafts.csswg.org/css-grid-3/ — editor's draft (основной источник).
- https://www.w3.org/TR/css-grid-3/ — TR-снапшот.
- https://webkit.org/blog/17660/introducing-css-grid-lanes/ — WebKit: модель, примеры, STP 234.
- https://gridlanes.webkit.org/ — «Field Guide to CSS Grid Lanes».
- https://webkit.org/blog/16026/css-masonry-syntax/ — история дебата синтаксиса.
- https://css-tricks.com/masonry-layout-is-now-grid-lanes/ — резолюция 31.01.2025, таймлайн.
- https://github.com/w3c/csswg-drafts/issues/12803 — живой спор row/column в item-flow.
- https://developer.apple.com/videos/play/wwdc2026/314/ — Learn CSS Grid Lanes (WWDC26).

### 4.5 Ladybird LibWeb

- Пайплайн: DOM → StyleComputer → **layout tree** (от ICB) → painting (display list). Layout организован как классы `FormattingContext` (BFC, IFC, FlexFC, GridFC, TableFC, SVGFC): родительский контекст создаёт дочерний и передаёт `AvailableSpace`; used values пишутся в `LayoutState`, коммитятся по завершении.
- IFC не самостоятелен — всегда работает в паре с родительским BFC (floats, ширины line box'ов). Margin collapsing — в BFC.
- Хороший образец «читаемого» движка: маленькие FC-классы, близкие к тексту спеки, без агрессивных кэшей (инкрементальность у Ladybird пока слабая — полные relayout'ы).
- Ссылки: https://github.com/LadybirdBrowser/ladybird ; обзор архитектуры: https://deepwiki.com/LadybirdBrowser/ladybird/1-overview

---

## 5. Инкрементальный relayout

### Ключевые концепции

1. **Dirty bits (классика WebKit/Blink).** Три бита на renderer: `m_needsLayout` (сам грязный), `m_normalChildNeedsLayout` (грязный in-flow потомок), `m_posChildNeedsLayout` (грязный positioned потомок). `setNeedsLayout()` помечает себя и восходит по ancestor chain, ставя соответствующий бит (позиционные потомки — отдельный бит, чтобы уметь перекладывать только их). `layout()` спускается только в грязные поддеревья (`layoutIfNeeded()`), после — биты сбрасываются.

2. **Relayout boundaries.** Blink стартует layout не с корня, а с ближайшей «границы relayout» (элемент с фиксированным размером, не зависящим от контента, свой formatting context): `MarkContainerChain` поднимает пометку только до boundary. Это ручной предок `contain`.

3. **Intrinsic size invalidation — отдельный канал.** `SetIntrinsicLogicalWidthsDirty` инвалидирует кэш min/max-content ширин независимо от позиционного layout. Критично: изменение контента глубоко внутри может менять intrinsic size предков (shrink-to-fit, auto-треки) — эта инвалидация ползёт вверх по своей цепочке, отдельной от needs-layout. Обратно: смена available size родителя НЕ инвалидирует intrinsic-кэши детей.

4. **LayoutNG-подход: кэш вместо битов.** Иммутабельные результаты + constraint space как ключ превращают «нужен ли relayout» в «промахнулся ли кэш». Diff constraint space'ов централизован (одно место решает, можно ли переиспользовать результат, включая быстрые пути: «изменилась только block-size availability, а узел от неё не зависит»). Это лечит и under-, и over-invalidation.

5. **CSS containment как контракт от автора.** `contain: layout` — элемент = независимый formatting context, layout внутри не влияет наружу и наоборот (движок может резать пометки на границе); `contain: size` — размер элемента не зависит от потомков (можно вообще не заходить внутрь при measure); `contain: paint` — клип + скип отрисовки невидимого; `content-visibility: auto` — скип layout+paint вне вьюпорта. Реально измеренные выигрыши: layout нормально масштабируется от размера всего DOM, containment сокращает объём до поддерева.

6. **Гранулярность инвалидации по типу изменения:** transform/scroll — не layout (compositor/spatial tree, см. Servo); смена цвета — только paint; смена контента — intrinsic dirty + needs layout. Пайплайн фаз (style → layout → paint) с чёткими границами (BlinkNG) не даёт фазам читать недосчитанное состояние.

### Ссылки
- https://webkit.org/blog/116/webcore-rendering-iii-layout-basics/ — dirty bits, ancestor chain, layout roots.
- https://chromium.googlesource.com/chromium/src/+/master/third_party/blink/renderer/core/layout/README.md — Blink layout: инвалидация, фазы, геометрия.
- https://developer.chrome.com/docs/chromium/blinkng — фазы пайплайна и инварианты.
- https://drafts.csswg.org/css-contain-2/ — спека containment.
- https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Containment/Using — практическое руководство.
- https://developer.chrome.com/blog/css-containment — containment в Chrome (оригинальный анонс).
- https://csswizardry.com/2026/04/what-is-css-containment-and-how-can-i-use-it/ — свежий практический разбор.

---

## 6. Сводка: что брать в малый Rust-движок поверх taffy-подобного раскладчика

1. **Constraint-space-как-ключ.** Один struct «входы layout» (known/available size, percent base, режим), он же ключ per-node кэша. Иммутабельный результат. Это самая дешёвая по коду идея LayoutNG с максимальным выигрышем — убирает экспоненту вложенных flex/grid автоматически.
2. **Два канала dirty:** `needs_layout` (позиции/размеры) и `intrinsic_dirty` (min/max-content), с разной логикой всплытия. Всплытие останавливать на relayout boundary: фиксированный размер + свой FC — то же, что `contain: layout size`, можно завести явный флаг узла.
3. **Box tree / результат раздельно** (Servo): дерево стилей/структуры живёт долго, результаты — заменяемые снапшоты; Arc-шаринг неизменённых поддеревьев даёт инкрементальность бесплатно.
4. **Low-level API taffy** вместо TaffyTree: своё хранилище, свой dispatch по display — тогда рядом с flex/grid легко поставить свои алгоритмы (inline-контекст, grid-lanes) с общим кэшем и общим measure-протоколом (`ComputeSize` vs `PerformLayout`).
5. **grid-lanes реализуем малой кровью поверх готового grid track sizing:** (а) прогнать track sizing по grid-оси, где все auto-placed items контрибутят во все треки; (б) placement циклом running positions + flow-tolerance + grid-lanes-direction; (в) stacking-axis alignment — сдвиг столбцов целиком. Не требует строк/областей grid-1 — по сути проще полного grid.
6. **Rayon — точечно:** параллелить только независимые дочерние layout'ы (flex/grid items после разрешения размеров треков/линий), BFC с margin collapsing и floats — последовательно.
7. **Margin collapsing — через margin strut в constraint space**, а не пост-фиксапами: единственный способ не сломать кэшируемость.
8. **Пиксельный снап — отдельным RoundTree-проходом** по float-результатам, чтобы ошибки не накапливались и кэши не зависели от округления.
9. **Enum-инварианты спеки** (Servo): «в inline box только inline children» и т.п. — на типах, не assert'ами.
10. **Спец-фичи для UI-движка:** `content-visibility: auto`-аналог (скип layout вне вьюпорта) даёт больше, чем любой микро-кэш, на длинных списках/деревьях.

---

## Полный список источников

**LayoutNG:** [deep-dive](https://developer.chrome.com/docs/chromium/layoutng) · [fragmentation](https://developer.chrome.com/docs/chromium/renderingng-fragmentation) · [layout_ng.md](https://chromium.googlesource.com/chromium/src/third_party/+/refs/heads/main/blink/renderer/core/layout/layout_ng.md) · design docs: [1](https://docs.google.com/document/d/1uxbDh4uONFQOiGuiumlJBLGgO4KDWB8ZEkp7Rd47fw4/edit), [2](https://docs.google.com/document/d/1RjH_Ofa8O_ucGvaDCEgsBVECPqUTiQKR3zNyVTr-L_I/edit), [3](https://docs.google.com/document/d/1EJOdFesZKspvrU7uWtGl-8ab2jIrzRF6NKJhwYOs6hU/) · [inline README](https://chromium.googlesource.com/chromium/src.git/+/62.0.3178.1/third_party/WebKit/Source/core/layout/ng/inline/README.md) · [BlinkNG](https://developer.chrome.com/docs/chromium/blinkng)

**Servo:** [2013 vs 2020](https://servo.org/blog/2023/04/13/layout-2013-vs-2020/) · [wiki Layout 2020](https://github.com/servo/servo/wiki/Layout-2020) · [Servo Book: Layout](https://book.servo.org/design-documentation/layout.html) · [Layout Engines Report](https://github.com/servo/servo/wiki/Servo-Layout-Engines-Report) · [grid via taffy PR](https://github.com/servo/servo/pull/32619)

**Taffy:** [GitHub](https://github.com/DioxusLabs/taffy) · [docs.rs](https://docs.rs/taffy) · [grid issue #204](https://github.com/DioxusLabs/taffy/issues/204) · [Bevy grid PR](https://github.com/bevyengine/bevy/pull/8026)

**Спеки:** [css-flexbox-1 §9](https://drafts.csswg.org/css-flexbox-1/#layout-algorithm) · [css-grid-1 §11](https://drafts.csswg.org/css-grid-1/#algo-track-sizing) · [CSS2 margin collapsing](https://drafts.csswg.org/css2/#collapsing-margins) · [css-grid-3 ED](https://drafts.csswg.org/css-grid-3/) · [css-grid-3 TR](https://www.w3.org/TR/css-grid-3/) · [css-contain-2](https://drafts.csswg.org/css-contain-2/)

**grid-lanes:** [WebKit intro](https://webkit.org/blog/17660/introducing-css-grid-lanes/) · [Field Guide](https://gridlanes.webkit.org/) · [syntax debate](https://webkit.org/blog/16026/css-masonry-syntax/) · [CSS-Tricks](https://css-tricks.com/masonry-layout-is-now-grid-lanes/) · [issue #12803](https://github.com/w3c/csswg-drafts/issues/12803) · [WWDC26](https://developer.apple.com/videos/play/wwdc2026/314/)

**Инкрементальность:** [WebKit dirty bits](https://webkit.org/blog/116/webcore-rendering-iii-layout-basics/) · [Blink layout README](https://chromium.googlesource.com/chromium/src/+/master/third_party/blink/renderer/core/layout/README.md) · [MDN containment](https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Containment/Using) · [Chrome containment](https://developer.chrome.com/blog/css-containment) · [CSS Wizardry 2026](https://csswizardry.com/2026/04/what-is-css-containment-and-how-can-i-use-it/)

**Ladybird:** [GitHub](https://github.com/LadybirdBrowser/ladybird) · [DeepWiki overview](https://deepwiki.com/LadybirdBrowser/ladybird/1-overview)
