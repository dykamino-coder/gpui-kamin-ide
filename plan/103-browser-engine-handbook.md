# Справочник: инженерия браузерного движка (уровень ядра)

Свод литературы и практики современных движков (Blink/Servo/WebKit/Ladybird) с привязкой
к нашему `crates/html` (retained gpui + taffy, WPT-reftest стенд). Собран 2026-08-20
из шести исследовательских отчётов. Разделы: I Архитектура и метод · II Раскладка ·
III Рендеринг/композитинг · IV Текст и шрифты · V CSS/каскад/парсинг.

---

## I. Архитектура и метод (как строят движки малыми силами)

### I.1 Главный учебник — «Web Browser Engineering» (Panchekha & Harrelson)

https://browser.engineering/ — полный текст бесплатно (OUP 2024). Соавтор Chris Harrelson —
техлид рендеринга Blink и автор серии RenderingNG. Базовый, но полный браузер на ~2000 строк
Python, каждая глава — работающий инкремент.

Структура = разумный порядок реализации фич:
- **Part 1 Loading**: URL/HTTP → окно/canvas/скролл → форматирование текста (wrap, метрики).
- **Part 2 Viewing**: HTML-дерево (error recovery — центральная идея парсинга) → block/inline
  layout (layout-дерево ОТДЕЛЬНО от DOM) → CSS (matching, cascade, inheritance) → hit testing.
- **Part 3 Applications**: формы → JS-биндинги/события → приватность.
- **Part 4 Modern** (самое ценное): визуальные эффекты (blending/clipping/stacking, surfaces)
  → scheduling/threads (пайплайн кадра явным конвейером) → анимации мимо layout →
  accessibility tree → embedded content → **глава 16: Reusing Previous Computation** —
  инвалидация и кэширование как отдельная дисциплина, вводится ПОСЛЕДНЕЙ поверх корректной
  наивной версии.

Три дерева: DOM → layout → paint/display list, явные фазы между ними.

### I.2 Ladybird / LibWeb — движок с нуля малой командой

- https://ladybird.org/ · https://github.com/LadybirdBrowser/ladybird
- Kling: https://awesomekling.github.io/Ladybird-a-new-cross-platform-browser-project/
- Слайды Web Engines Hackfest 2023: webengineshackfest.org/2023/slides/ladybird_*.pdf

~7-10 оплачиваемых инженеров + волонтёры; >90% WPT-сабтестов к 10.2025 (2.07M к 04.2026).

**Спека-как-код — главный урок**: алгоритм спеки копируется в комментарии построчно, под
каждой строкой — код ровно этого шага; функции называются как в спеке
(`run_the_focusing_steps`). Эффекты: ревью против спеки, а не против чужого движка;
обновление спеки = grep по тексту шага. «Не читать чужие движки — читать спеку»; дыры
спеки эскалировать в whatwg/csswg issues (молодой движок = лучший фаззер спеки).

**Тест-дривен**: WPT импортирован в репозиторий, тест коммитится вместе с фиксом;
публичный счётчик — компас приоритизации и метрика. Приоритет: топ-сайты реального веба
как milestone; перф — ПОСЛЕ корректности (inline caches LibJS взяли после 90% WPT).

### I.3 Servo (LF Europe, Igalia)

https://servo.org/ · https://servo.org/wpt/ — Rust, параллельный layout, WebRender.
0.1.0 (04.2026). Ставка на embedding (`libservo`, delegate-based WebView API, альтернатива
CEF). **Методологический урок метрики**: 92% subtest-rate при взвешенном скоре 62%
(Chromium ~89%) — сырой счётчик сабтестов льстит; честнее взвешенный срез по фичам.

### I.4 RenderingNG (Blink) — пайплайн кадра

- https://developer.chrome.com/docs/chromium/renderingng-architecture
- …/renderingng-data-structures · …/blinkng · …/layoutng
- «Inside look at modern web browser» (Kosaka, 4 части): developer.chrome.com/blog/inside-browser-part1…4
- «Life of a Pixel» (Chrome University) — лучший видеообзор конвейера.
- Классика: web.dev/articles/howbrowserswork (Garsiel).

12 стадий: animate → style → layout → pre-paint → scroll → paint → commit → layerize →
raster/decode → activate → aggregate → draw. Ключевое:
- **Чистые фазы с неизменяемыми выходами**: LayoutNG выдаёт immutable fragment tree; paint —
  display lists. Иммутабельность = кэшируемость + потокобезопасность (весь BlinkNG —
  многолетний рефакторинг «нельзя читать грязное состояние между фазами»).
- **Property trees** (transform/clip/effect/scroll) отдельно от слоёв: compositor мутирует
  только их — скролл/анимации пропускают style/layout/paint.
- Стадии пропускаемы: пайплайн-инвалидация решает, какие фазы нужны кадру.

### I.5 Immediate vs retained: где наш движок

Браузер — предельный retained: инвалидация автоматическая и консервативно-корректная.
GPUI (наша основа) — гибрид: retained Entity-state + element tree, пересоздаваемый каждый
кадр; layout через taffy; paint кладёт примитивы в Scene (quads/shadows/glyphs/paths),
сортировка по stacking order, батчи по типу примитива. GPUI повторяет ХВОСТ браузерного
конвейера (paint→display list→raster), но выбрасывает инвалидацию style/layout, пересобирая
дерево кадра целиком — потолок кадра = taffy-замеры при тысячах узлов (наша память:
layout-diet). Родственная система координат: Raph Levien
(raphlinus.github.io — «Towards principled reactive UI», «Xilem: an architecture for UI in
Rust»; linebender.org/blog/xilem-backend-roadmap/): immediate-описание + retained-исполнение
+ diff между ними.

**Вывод для нас**: наша "однокадровость + probes в prepaint" — самодельный аналог
пайплайн-фаз; двигаться к явным фазам (style→layout→paint) с иммутабельными промежуточными
структурами; инвалидацию (кэши measure/split/VT) наращивать поверх корректной наивной
версии — ровно так уже и идём.

### I.6 WPT и метрики

- web-platform-tests.org/writing-tests/ · wpt.fyi · github.com/web-platform-tests/interop
- reftest: `<link rel="match">`/`mismatch`, `<meta name="fuzzy">` — допуск (N пикселей,
  отклонение каналов) для антиалиасинга; reference строится из «заведомо простых» примитивов.
- crashtests — pass = не упал (дешёвый корпус).
- Interop 2025: 19 областей, индустрия меряет курируемые СРЕЗЫ, не всё WPT.
- Стратегия молодого движка: кластеризовать провалы по ОБЩЕМУ reference (одна примитивная
  фича закрывает пачку — наша практика «дифф против общего эталона» это подтверждает);
  expectations-файлы (Servo/Gecko) либо тест-рядом-с-фиксом (Ladybird); шумовой пол
  существует даже у Chrome.

### I.7 Где живёт истина

- WHATWG living standards: https://html.spec.whatwg.org/multipage/ (+ infra.spec.whatwg.org).
- CSSWG: **drafts.csswg.org** (Editor's Drafts, НЕ w3.org/TR — тот устаревший снапшот);
  индекс свойств: drafts.csswg.org/indexes/.
- **csswg-drafts issues = прецедентное право CSS**: неоднозначная семантика почти всегда
  разрешена в issue с «RESOLVED: …» — искать issue по имени свойства ПЕРЕД реализацией
  спорного поведения.

---

## II. Раскладка (layout)

Полный конспект: [handbook-drafts/layout.md](handbook-drafts/layout.md). Главное:

- **LayoutNG**: контракт «(node, style, constraint space) → immutable fragment result»;
  constraint space (available size, новый FC, margin strut, floats, fragmentainer) = cache
  key; measure pass (min/max-content) кэшируется ОТДЕЛЬНО от layout pass — это убрало
  экспоненту O(2^n) вложенных flex/grid. Выход — физические координаты, внутри — логические
  оси. developer.chrome.com/docs/chromium/layoutng
- **Servo Layout 2020**: box tree (структура по спеке, enum-инварианты) ≠ fragment tree
  (результат); rayon точечно — только доказуемо независимые дети; BFC с floats/margin
  collapsing — последовательно. Grid в Servo — через taffy (PR #32619: 18.6% → 38.3% css-grid).
- **Taffy low-level API**: traits LayoutPartialTree/CacheTree + compute_*_layout на ОДИН
  узел — своё дерево, свой dispatch по display, свои алгоритмы рядом с taffy-шными;
  RunMode::ComputeSize = дешёвый measure; RoundTree — пиксельный снап отдельным проходом.
- **Спековые алгоритмы**: flexbox §9 (resolve flexible lengths — итеративный freeze-цикл),
  grid track sizing §11 (base/growth limit → intrinsic вклады span=1,2,… → maximize → fr →
  stretch; до 2 прогонов на ось), margin collapsing через margin strut в constraint space.
- **css-grid-3 / grid-lanes** (наш фронт!): резолюция 31.01.2025 «grid-lanes»; РАЗДЕЛ 4.4
  драфта — точная модель: running positions, flow-tolerance (normal=1em!), grid-lanes-direction,
  **track sizing при auto-placement: ВСЕ auto-элементы контрибутят во ВСЕ треки ДО
  placement**; content-alignment stacking-оси двигает столбец целиком; dense backfill
  надёжен только при равных span'ах. webkit.org/blog/17660 · gridlanes.webkit.org (Field
  Guide!) · drafts.csswg.org/css-grid-3/
- **Инкрементальность**: два канала dirty (needs_layout и intrinsic_dirty с разным
  всплытием), relayout boundaries (фикс-размер + свой FC), contain как контракт;
  LayoutNG-подход «кэш вместо битов» — diff constraint space централизован.

**Для нас**: сводка из 10 пунктов в конце drafts/layout.md — constraint-space-как-ключ,
два dirty-канала, box/fragment разделение, low-level taffy, grid-lanes поверх готового
track sizing, rayon точечно, margin strut, RoundTree-снап, enum-инварианты,
content-visibility-аналог для длинных списков.

## III. Рендеринг и композитинг

Полный конспект: [handbook-drafts/rendering.md](handbook-drafts/rendering.md). Главное:

- **CSS 2.1 Appendix E** (w3.org/TR/CSS21/zindex.html) — точный алгоритм порядка отрисовки
  stacking context, шаги 1-10: фон SC → отрицательные z-index SC (НАД фоном родителя, ПОД
  его блоковым контентом) → фоны in-flow блоков → floats (pseudo-SC) → inline-контент →
  positioned z:auto/0 → положительные z-index → outlines ПОВЕРХ ВСЕГО. z-index:auto =
  pseudo-SC (атомарен для рисования, но НЕ изолирует positioned потомков) — важное отличие
  от z-index:0.
- Движки НЕ сортируют глобально: рекурсивный painter в порядке Appendix E эмитит плоский
  display list; стабильная сортировка только детям каждого SC по (z_bucket, z_index,
  tree_order). Blink: PaintLayer с кэшированными negZOrderList/posZOrderList.
- **SC = граница offscreen-слоя только при opacity/filter/blend** — иначе чисто логическая
  скобка порядка (наша paint_group-механика ровно об этом).
- WebRender: display list → scene building → frame building → GPU; picture caching
  (doc.servo.org/webrender/picture/); spatial/clip trees отдельно от примитивов.
- Blink: property trees (transform/clip/effect/scroll) — компоновка мимо paint.
- Полный современный список создателей SC: opacity<1, transform, filter, backdrop-filter,
  clip-path, mask, isolation, mix-blend-mode, will-change, contain:paint, fixed/sticky.

## IV. Текст и шрифты

Полный конспект: [handbook-drafts/text.md](handbook-drafts/text.md). Главное:

- **Shaping**: harfrust (github.com/harfbuzz/harfrust — официальный порт HarfBuzz v13 на
  read-fonts, <25% медленнее C++) — выбор по умолчанию; rustybuzz — предшественник; swash
  вытесняется skrifa. Шейпить на уровне ПАРАГРАФА (кросс-элементные лигатуры/кернинг);
  clusters level 1 — граница разрыва строк; UNSAFE_TO_BREAK флаги — где нужен ре-шейп
  при разрыве. Нормализовать текст заранее НЕ нужно (шейпер сам).
- **Line breaking**: UAX #14 (unicode.org/reports/tr14/) — классы + правила LB1-31;
  icu4x icu_segmenter — рекомендуемая база (CSS line-break/word-break опции, SEA-словари,
  Gecko уже мигрировал); xi-unicode — минимальный fallback. Hyphenation: Knuth-Liang,
  crate hyphenation (TeX-паттерны ~70 языков). text-wrap:balance/pretty — score-based
  (Knuth-Plass) поверх greedy.
- **parley** (Linebender) — референс-архитектура rich text: fontique (fallback) + harfrust +
  skrifa + icu4x; re-break без re-shape; InlineBox в потоке.
- Остальные темы в драфте: inline layout/line box construction, bidi UAX #9, вертикальное
  письмо UTR #50, CSS font matching, DirectWrite-специфика.

## V. CSS: каскад, селекторы, парсинг

### V.1 Selector matching — как ускоряют все движки

- **Right-to-left**: матчить с rightmost compound, влево по комбинаторам; descendant — бэктрекинг по предкам, `>` — один шаг.
- **Rule bucketing**: правила бакетируются по rightmost compound (id → class → attr → tag →
  универсальные); для элемента проверяются только бакеты его id/классов/тега (Blink/WebKit
  `RuleSet`, Servo `SelectorMap`: doc.servo.org/style/selector_map/).
- **Ancestor Bloom filter** (WebKit `SelectorFilter`, Servo `StyleBloom`): при обходе дерева
  push/pop хешей id/class/tag предков (counting bloom для pop); probe перед точным
  descendant-матчем; «нет» = точный отказ. WebKit: +25% общий, 2× на descendant.
  bugs.webkit.org/show_bug.cgi?id=53880 · doc.servo.org/style/bloom/struct.StyleBloom.html
  **Для нашего Vec<Ancestor>-walk это фикс №1.**
- `:has()` (Blink 2022-23): поиск вниз от anchor + кэш на время recalc
  (`CheckPseudoHasCacheScope`); инвалидация — обход вверх от мутации.
  blogs.igalia.com/blee/posts/2023/05/31/how-blink-invalidates-styles-when-has-in-use.html

### V.2 Stylo как донор

- Обзор: hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/ ·
  book.servo.org/architecture/style.html
- Rule tree: matched declarations = путь в дереве правил, общие префиксы шарятся.
- **Style sharing cache**: LRU ~31 сиблинга/кузена; тот же parent style + классы →
  computed style переиспользуется без матчинга — крупнейшая экономия на реальных страницах.
- Custom properties: отдельный ранний проход var()-подстановки, циклы → guaranteed-invalid.
- **Крейт `selectors` встраиваем**: имплементируешь trait `Element` (parent/prev_sibling/
  has_class/attr) + `SelectorImpl` → `matches_selector()` с MatchingContext (bloom + nth-кэш
  в комплекте). docs.rs/selectors — вариант замены нашего матчера (пункт task #93).
- `cssparser` (токенизатор css-syntax-3) — тоже встраиваемый: docs.rs/cssparser.

### V.3 Каскад точно по спеке (css-cascade-5/6)

- Порядок: origin+importance → context → **@layer** (important-слои в ОБРАТНОМ порядке) →
  specificity → порядок появления. Style attribute — отдельный шаг выше селекторов.
  @scope proximity (cascade-6) — tie-break после specificity.
- Value stages: declared → cascaded → specified → **computed** (наследуется ИМЕННО computed)
  → used (layout) → actual.
- Проценты: остаются процентами на computed везде, где база зависит от layout
  (width/margin/padding — от containing block); резолвятся на computed только где база
  известна без layout (font-size, line-height). drafts.csswg.org/css-cascade-5/#value-stages

### V.4 HTML-парсинг

- html5ever: Tokenizer (state machine спеки) → TreeBuilder (insertion modes) → trait
  `TreeSink` — свой DOM подключается имплементацией sink. github.com/servo/html5ever
  (у нас уже vendored ✓).
- Tree construction (html.spec.whatwg.org/multipage/parsing.html): ~23 insertion modes,
  stack of open elements + active formatting elements; adoption agency (misnested `<b><i>`),
  foster parenting (нетабличное из `<table>` — выносится перед таблицей), implied end tags.

### V.5 Инкрементальный restyle

- Blink invalidation sets: из каждого селектора извлекается «фича слева» → set фич справа;
  мутация класса X поднимает descendant/sibling set X — dirty только затронутые.
  chromium.googlesource.com/chromium/src/+/master/third_party/blink/renderer/core/css/style-invalidation.md
- Servo: snapshot до мутации + Dependency-based invalidation + restyle hints.
- Минимум для нас: dirty-биты self/descendants → потом invalidation sets по классам.

### V.6 Значения (css-values-4)

- calc() = дерево (calc-sum/product/value), упрощается на computed где возможно, иначе
  доживает до used; min/max/clamp — узлы того же дерева.
- Единицы ch/ic (ширина «0»/«水» — нужны метрики), cap, lh/rlh (цикл font-size↔line-height!).

**Приоритеты для нас**: (1) bloom поверх Vec<Ancestor>; (2) rule bucketing по rightmost
compound; (3) рассмотреть `selectors`+`cssparser` вместо самописных (уже в task #93);
(4) dirty-биты → invalidation sets.

---

## VI. Готовые решения с GitHub (что копировать)

Полный конспект: [handbook-drafts/github.md](handbook-drafts/github.md). Главное:

- **Blitz (DioxusLabs)** — движок нашего класса (Stylo+Taffy+Parley+Vello, без JS).
  **Калибровка ожиданий**: их WPT ≈47.5% overall (без JS доступно лишь ~20% сабтестов);
  топ: css-variables 97.8%, css-color 90.5%. `blitz-dom` — рабочий образец навешивания
  Stylo-трейтов на свой DOM; `stylo_taffy` — конвертер computed→taffy Style.
  blitz.is/status/wpt · github.com/DioxusLabs/blitz-wpt-results.
- **Крейт `selectors` — лучшая инвестиция списка**: Element-трейт ~20 методов → корректный
  матчинг всей грамматики + Bloom-фильтр + nth-index cache в комплекте (MPL-2.0 — ок при
  статической линковке). `cssparser` снимает класс багов токенизации (escapes, an+b,
  unicode-range).
- **Текстовый стек**: parley (0.11, июнь 2026) > cosmic-text для HTML (есть InlineBox;
  быстрее в shaping+layout). **fontique целиком** — дешёвый корректный font fallback/
  matching вместо ручного IDWriteFontFallback. **harfrust целиком** — детерминированный
  кроссплатформенный шейпер (WPT-тексты стабильнее, чем DirectWrite-шейпинг).
  skrifa — «новый freetype» (метрики/аутлайны/hinting/COLR).
- **taffy** — тот же мейнтейнер, что Blitz; лёг в основу grid Servo; смотреть low-level
  API + CHANGELOG 0.9-0.12.
- У cosmic-text подсматривать читаемые реализации shape run cache и glyph cache.

---

## VII. Чек-лист «перед следующим заходом» (синтез всех разделов)

1. **grid-lanes**: сверить нашу модель с Field Guide (gridlanes.webkit.org) и драфтом §4.4:
   flow-tolerance normal = 1em (у нас учтён?); track sizing — все auto-элементы контрибутят
   во ВСЕ треки ДО placement; content-alignment двигает столбец целиком.
2. **Bloom-фильтр предков** над walk_children — фикс №1 по скорости стайлинга.
3. **Слот-выравнивание/пады**: сверять с Appendix E пошагово при любом paint-order баге.
4. **Кэш-протокол layout**: constraint-space-как-ключ (у нас: measure_key/split_key — уже
   в этом духе; расширять к явному constraint space).
5. **harfrust/fontique** — кандидаты на замену DirectWrite-шейпинга (детерминизм на WPT);
   вертикальное письмо у нас уже своё — сверить с UTR #50 из text.md.
6. **contain/content-visibility** — дешёвые обрезатели инвалидации для IDE-использования движка.
7. Спорная семантика CSS → сначала grep issue в github.com/w3c/csswg-drafts.
8. Метрика: следить и за сабтест-процентом, и за взвешенным по семьям (урок Servo 92%/62%).
