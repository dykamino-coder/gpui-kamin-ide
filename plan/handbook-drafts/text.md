# Текст, шрифты, строчная раскладка в браузерных движках — конспект литературы

Ориентир: Rust-движок с собственным line breaker и DirectWrite-бэкендом на Windows.

---

## 1. Text shaping: HarfBuzz, harfrust/rustybuzz, swash

### Модель HarfBuzz

Shaping = преобразование последовательности Unicode-кодпоинтов (+ script, language, direction, features) в последовательность позиционированных глифов. Ядро — OpenType shaping: применение GSUB (substitution) и GPOS (positioning) лукапов по плану (shape plan), составленному из фич, специфичных для скрипта (per-script shapers: Latin/default, Arabic, Indic, USE — Universal Shaping Engine, Hangul, Hebrew, Khmer, Myanmar, Thai/Lao).

Ключевые концепции:

- **hb_buffer_t** — вход/выход шейпера. Вход: кодпоинты + cluster values. Выход: glyph infos (glyph id, cluster) + glyph positions (advance, offset). Направление буфера (LTR/RTL/TTB/BTT) определяет систему координат advance'ов.
- **Clusters** — механизм обратной трассировки глифов к исходным символам (нужен для каретки, выделения, hit-testing). Три уровня: level 0 (legacy, marks сливаются с базой), level 1 (рекомендованный: marks сохраняют отдельные cluster values, лучшая гранулярность), level 2 (характеры-глифы без слияния, монотонность НЕ гарантируется). При level 0/1 гарантирована монотонность cluster values, если вход монотонен — на этом строится line breaking по шейп-результату: разрыв допустим только на границе кластера. https://harfbuzz.github.io/working-with-harfbuzz-clusters.html
- **Normalization внутри шейпера**: HarfBuzz сам делает decompose/compose (не полный Unicode NFC/NFD, а «шрифто-ориентированная» нормализация: выбирает форму, для которой в шрифте есть глифы; `hb-ot-shape-normalize.cc`). Клиенту НЕ нужно нормализовать текст заранее. https://github.com/harfbuzz/harfbuzz/blob/main/src/hb-ot-shape-normalize.cc
- **Features**: default-фичи включаются шейпером (ccmp, liga, kern/dist, mark/mkmk, rlig, calt и скриптовые init/medi/fina для арабского...); пользовательские (`font-feature-settings`) добавляются поверх. Диапазонные фичи (на подстроку буфера) поддерживаются.
- **Пре-контекст/пост-контекст** буфера (`hb_buffer_add` с item_offset/length) — важен для правильного шейпинга на границах сегментов (например, contextual forms арабского при разбиении текста на runs по стилю).

Мануал: https://harfbuzz.github.io/ ; буферы: https://harfbuzz.github.io/harfbuzz-hb-buffer.html

### Rust-порты: состояние 2025–2026

- **rustybuzz** (https://github.com/harfbuzz/rustybuzz) — полный порт шейпинг-алгоритма HarfBuzz на Rust поверх ttf-parser. Перенесён в организацию harfbuzz. Стабильный, но развитие де-факто перешло к harfrust.
- **harfrust** (https://github.com/harfbuzz/harfrust) — официальный «HarfBuzz port to Rust», форк rustybuzz, переведённый с ttf-parser на **read-fonts** (из Google Fonts oxidize / fontations), чтобы делить парсинг шрифтов со **skrifa**. Отслеживает upstream: соответствует HarfBuzz v13 (CHANGELOG: https://github.com/harfbuzz/harfrust/blob/main/CHANGELOG.md). Производительность: <25% медленнее C++ HarfBuzz на типичных шрифтах (https://github.com/harfbuzz/harfrust/blob/main/HARFBUZZ.md). Ограничения: нет интеграций FreeType/CoreText/DirectWrite/ICU/graphite2; экспериментальные boring-expansion фичи не поддержаны. **Выбор по умолчанию для нового Rust-движка.**
- **swash** (https://github.com/dfrg/swash) — независимая реализация шейпинга + introspection + scaling/рендеринг глифов (CFF/glyf/COLR/bitmap) от dfrg. Шейпер не так battle-tested, как порт HarfBuzz; активность ниже. Ценность swash сегодня — скорее glyph scaling/рендеринг, но и это вытесняется skrifa.
- Экосистемный ориентир: **parley** (Linebender, https://github.com/linebender/parley) — rich text layout: стек = fontique (font enumeration/fallback) + **harfrust** (shaping) + **skrifa** (metrics/outlines) + **icu4x** (сегментация). Умеет shaping, line breaking, bidi-reorder, alignment, InlineBox (inline-block-подобные объекты в потоке текста), повторный ре-брейкинг без ре-шейпинга. Хороший референс архитектуры, даже если не использовать напрямую. **cosmic-text** (pop-os) — альтернатива на rustybuzz, ориентирована на редакторы/GUI, а не на браузерный inline layout.

### Применимость

Шейпить на уровне **параграфа/inline formatting context**, не слова: кросс-элементные лигатуры и кернинг (см. §3, LayoutNG). Разрывать строки только по границам кластеров; при разрыве внутри отшейпленного run'а — либо ре-шейп сегмента (safe-to-break точки HarfBuzz: `hb_glyph_info_get_glyph_flags` / UNSAFE_TO_BREAK), либо честный повторный шейпинг обеих половин.

---

## 2. Line breaking: UAX #14, hyphenation, text-wrap: balance/pretty

### UAX #14

https://www.unicode.org/reports/tr14/ — Unicode Line Breaking Algorithm. Каждому кодпоинту — line break class (~50 классов: AL, ID, CJ, BA, BB, GL, SP, ZW, WJ...). Пары классов + контекстные правила (LB1–LB31) дают три исхода между символами: mandatory break (BK, CR/LF/NEL), allowed break, prohibited. Ключевые тонкости:

- **CSS-тейлоринг**: `line-break: strict/normal/loose` меняет трактовку CJ (условно-запрещённые кана-символы), `word-break: break-all/keep-all` переопределяет пары. CSS Text 3 §5: https://www.w3.org/TR/css-text-3/#line-breaking
- **SEA-языки** (тайский, кхмерский, лаосский, бирманский) не имеют разделителей — нужен словарный/ML-брейкер поверх UAX #14.
- **WJ/ZWJ/U+00A0** запрещают разрыв; SOFT HYPHEN (U+00AD) — invisible break opportunity с видимым дефисом при разрыве.

Реализации:

- **ICU4C** `BreakIterator` — историческая база (Gecko/Blink частично); Blink использует собственный `LazyLineBreakIterator` поверх ICU-данных.
- **icu4x `icu_segmenter`** (https://docs.rs/icu_segmenter) — Rust, UAX #14-совместимый LineSegmenter с опциями под CSS `line-break`/`word-break`, + словарные и LSTM-модели для SEA-языков. Данные на 60% компактнее ICU4C, скорость выше (до +47% на китайском): https://blog.unicode.org/2023/04/icu4x-12-now-with-text-segmentation-and.html . Gecko мигрировал свой LineBreaker на icu4x: https://bugzilla.mozilla.org/show_bug.cgi?id=1719535 . **Рекомендуемая база для собственного line breaker.**
- **xi-unicode** (https://docs.rs/xi-unicode/latest/xi_unicode/) — компактный UAX #14-итератор (offset, is_hard_break) из xi-editor; проще, без CSS-тейлоринга и SEA-словарей. Годен как минимальный fallback.

### Hyphenation

- **Knuth–Liang** — паттерновый алгоритм из TeX (нечётные цифры в паттернах = точки переноса). Словари TeX покрывают ~70 языков. Rust: crate **hyphenation** (https://crates.io/crates/hyphenation) — Knuth–Liang по TeX UTF-8 паттернам + расширенный нестандартный перенос (Németh, Libre/OpenOffice). Форк **kl-hyphenate** — то же с бинарными словарями.
- В браузерах `hyphens: auto`: Firefox — libhyphen (Liang-паттерны), Chromium — свои словари (минимальный порт), WebKit — платформенные API. Дефисация даёт дополнительные break opportunities внутри слов; взаимодействует с `hyphenate-limit-*` (CSS Text 4).

### Greedy vs score-based; text-wrap: balance / pretty

Классика браузеров — **greedy first-fit**: класть в строку, пока лезет. TeX — **Knuth–Plass**: глобальная оптимизация параграфа по сумме штрафов (badness) через динамическое программирование. Wikipedia: https://en.wikipedia.org/wiki/Knuth%E2%80%93Plass_line-breaking_algorithm

Chromium (Koji Ishii, 2023) добавил **score-based paragraph-level line breaking** для `text-wrap: balance/pretty`:

- Дизайн-обзор «Score-based Paragraph-level Line Breaking»: https://gwern.net/doc/cs/css/2024-ishii.pdf — перечисляет варианты разбиения, скорит их (штрафы за недозаполнение, hyphen на конце, orphans) и выбирает лучший; вдохновлён Knuth–Plass, но применяется ограниченно из-за стоимости (перебор комбинаций breakpoints).
- **`balance`**: минимизировать разброс длин строк; в Chromium — по сути бисекция доступной ширины (сужать блок, пока число строк не увеличится), лимит 6 строк (Firefox — 10). https://developer.chrome.com/docs/css-ui/css-text-wrap-balance ; фидбек по ранней реализации: https://github.com/w3c/csswg-drafts/issues/8516
- **`pretty`**: анти-orphan + улучшение хвоста параграфа; Chromium перескорит последние ~4 строки, а изначально — только последнюю (если она короче трети ширины). https://developer.chrome.com/blog/css-text-wrap-pretty ; WebKit пошёл дальше (весь параграф, включая ровность правого края и дефисы): https://webkit.org/blog/16547/better-typography-with-text-wrap-pretty/
- Intent to Ship: https://groups.google.com/a/chromium.org/g/blink-dev/c/rwBWqqOB_ag

Применимость: собственный line breaker строить как **итератор break opportunities** (icu4x segmenter + hyphenation как источники кандидатов) поверх измерений шейп-результата; greedy — базовый путь, score-based — отдельный опциональный проход над тем же списком кандидатов (важно: кандидаты и ширины переиспользуются, ре-шейпа быть не должно, кроме unsafe-to-break точек).

---

## 3. Inline layout: line box, vertical-align, line-height, LayoutNG

### Модель CSS

- **Inline formatting context**: содержимое раскладывается в line boxes. Каждый inline box в строке имеет content area (высота = ascent+descent шрифта, A+D) и виртуальный «leading box»: L = line-height − (A+D), половина L (**half-leading**) сверху, половина снизу. Высота line box = охват выровненных inline-боксов (учитывая vertical-align), а НЕ сумма content areas. CSS2 §10.8: https://www.w3.org/TR/CSS2/visudet.html ; css-inline-3: https://drafts.csswg.org/css-inline-3/
- Каноническая статья по механике: Vincent De Oliveira, «Deep dive CSS: font metrics, line-height and vertical-align» — https://iamvdo.me/en/blog/css-font-metrics-line-height-and-vertical-align . Главные грабли: content area ≠ font-size; strut (пустой inline с метриками шрифта блока) участвует в каждой строке; `line-height: normal` зависит от метрик шрифта (см. §6).
- **vertical-align**: baseline (алфавитная), middle (= baseline + x-height/2), sub/super, text-top/text-bottom (края content area корня строки), top/bottom (края line box — «плавающие», вычисляются после остальных, возможны циклы, разрешаемые двумя проходами). css-inline-3 обобщает через `alignment-baseline`/`baseline-source`; для CJK важна **central baseline** (центр em-box) — дефолт в вертикальном письме.
- **text-box-trim/text-box-edge** (бывш. leading-trim, css-inline-3): срезание half-leading + over/under edge (cap/ex/text/ic) у первой/последней строки блока — метрико-зависимый вертикальный тримминг. https://developer.chrome.com/blog/css-text-box-trim ; предыстория: https://medium.com/microsoft-design/leading-trim-the-future-of-digital-typesetting-d082d84b202

### LayoutNG (Blink) — эталонная архитектура

README: https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/core/layout/inline/README.md (историч. путь ng/inline: https://chromium.googlesource.com/chromium/src/+/9d7f9214e689a910456435fd46b01f6731ad190c/third_party/blink/renderer/core/layout/ng/inline/README.md); обзор RenderingNG: https://developer.chrome.com/docs/chromium/layoutng

Пайплайн NGInlineNode (pre-layout, кешируется на параграф):

1. **CollectInlines** — DFS по инлайн-контейнеру: текст и atomic inlines конкатенируются в единую «плоскую» строку параграфа с маркерами открытия/закрытия inline-боксов (objects replacement char для atomics). Это ключ: весь IFC = одна строка + список InlineItem'ов.
2. **SegmentText** — bidi-сегментация (UAX #9 по ICU) + сегментация по script/orientation.
3. **ShapeText** — шейпинг разрешённых bidi/script runs целиком HarfBuzz'ом: **paragraph-level shaping** даёт кернинг и лигатуры через границы элементов (`<span>b</span>o` кернится) и одинаковые арабские формы независимо от разметки.

Затем line breaker режет ShapeResult на строки (переиспользуя shape-результат через `ShapeResult::SubRange`, ре-шейп только на unsafe-to-break), строит фрагменты; строки — anonymous physical fragments.

Текстовый стек Blink (platform/fonts): https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/platform/fonts/README.md — **RunSegmenter** (script/orientation/emoji сегментация) → **HarfBuzzShaper** → **ShapeResult**; **CachingWordShaper** — word-level shape cache (сегментация по словам/пробелам/CJK) как ускоряющий слой для legacy-путей; LayoutNG кеширует целые параграфы и результаты строк.

Применимость: повторить трёхфазную схему — плоский текст параграфа + item list, шейпинг по runs, line breaker поверх готовых advance'ов. Кеш: (шрифт, стиль, текст сегмента) → ShapeResult; инвалидация при изменении доступной ширины НЕ должна трогать шейпинг.

---

## 4. Bidi: UAX #9 и строчная раскладка

https://www.unicode.org/reports/tr9/ — Unicode Bidirectional Algorithm (UBA). Фазы:

1. **По параграфу**: определить paragraph embedding level (P2–P3: первый сильный символ, либо принудительно из `dir`/`unicode-bidi`), разрешить **explicit levels** (X1–X10: LRE/RLE/LRO/RLO/PDF и **isolates** LRI/RLI/FSI/PDI), разрешить weak/neutral типы (W1–W7, N0–N2 — N0 это bracket pairing), implicit levels (I1–I2). Результат — **resolved embedding level** (число) на каждый символ: чётный = LTR, нечётный = RTL.
2. **По строке, после line breaking**: правила L1–L4. L1 — сброс уровней trailing whitespace к paragraph level; **L2 — реордеринг**: найти максимальный уровень, итеративно разворачивать максимальные последовательности уровней ≥ i для i от max до 1. L3 — комбинирующие знаки, L4 — зеркальные глифы (скобки).

Критично для движка: **уровни считаются один раз на параграф, реордеринг — на каждую строку после разбиения** (разбиение делается в **логическом порядке** по логическим ширинам!). Isolates (введены в Unicode 6.3) — предпочтительный механизм CSS (`unicode-bidi: isolate` — дефолт для `dir`), не наследуют контекст наружу.

Интеграция с CSS: https://www.w3.org/TR/css-writing-modes-4/#bidirectionality — каждый inline-элемент с `unicode-bidi` синтезирует управляющие символы вокруг своего текста в плоской строке параграфа (так делает LayoutNG в SegmentText).

Rust: crate **unicode-bidi** (https://crates.io/crates/unicode-bidi, поддерживается Servo) — полная UAX #9 реализация: `BidiInfo::new` → levels, `reorder_line` / `visual_runs` для L1–L2 по диапазону строки. icu4x также имеет properties для bidi. Взаимодействие с шейпингом: каждый level run шейпится своим направлением (RTL run — RTL-буфер HarfBuzz), визуальный порядок runs в строке — по L2, порядок глифов внутри RTL run уже выдан шейпером справа налево.

Гоча: soft hyphen и trailing spaces при реордеринге (L1) — уровни хвостовых пробелов сбрасываются, чтобы пробел «висел» на правильном краю; ellipsis/каретка требуют visual↔logical маппинга по уровням.

---

## 5. Вертикальное письмо: writing-mode, UTR #50, vert/vrt2

### CSS

https://www.w3.org/TR/css-writing-modes-4/ — `writing-mode: vertical-rl / vertical-lr / sideways-rl / sideways-lr`. Понятия: block flow direction, inline base direction, **line-relative** vs **flow-relative** направления. В вертикальных режимах line box'ы — вертикальные колонки; «ширина строки» = высота контейнера. Baseline table меняется: доминирующая базовая линия — **central** (центр em) для vertical-rl/lr, alphabetic — для горизонтали.

### UTR #50 / UAX #50

https://www.unicode.org/reports/tr50/ — свойство **Vertical_Orientation (vo)** на кодпоинт: `U` (upright — CJK, кана), `R` (rotated 90° cw — латиница, пунктуация), `Tu`/`Tr` (typographically transformed — требуют глифовой замены: скобки, кавычки, длинное тире в CJK). CSS `text-orientation: mixed` (дефолт) реализует UTR #50: U-символы прямо, R-символы повёрнуты; `upright` — всё прямо (и bidi принудительно LTR); `sideways` — всё повёрнуто.

### Шрифтовые фичи

- **`vert`** — вертикальные альтернаты для upright-глифов (повёрнутая пунктуация CJK: 「」→ вертикальные формы, точки/запятые в углы). Применяется layout-движком, который сам поворачивает sideways-runs. **`vrtr`** — альтернаты для sideways-глифов.
- **`vrt2`** — «vertical alternates and rotation»: шрифт сам поставляет ПРЕ-повёрнутые глифы для всего; исключает vert. Современные движки (по UTR #50) используют **vert (+vrtr)**, а не vrt2. Adobe, «A Tale of Three (OpenType) Features»: https://blogs.adobe.com/CCJKType/2013/08/tale-of-three-features.html ; спека фич: https://learn.microsoft.com/en-us/typography/opentype/spec/features_uz
- Вертикальные метрики глифов: таблицы `vmtx`/`vhea` (vertical advance/origin), `VORG` (CFF vertical origins). HarfBuzz: буфер с direction=TTB даёт вертикальные advances и применяет vert автоматически.
- **text-combine-upright** (tate-chu-yoko, 縦中横): 2–4 символа (цифры) компонуются горизонтально в один вертикальный em: горизонтальный шейпинг мини-run'а + масштабирование/сжатие (фичи hwid/twid/qwid как первая попытка) до 1em, затем вставка как upright-кластер. css-writing-modes §9.

Применимость: сегментация по orientation (RunSegmenter в Blink делит на upright/rotated по UTR #50) → upright-runs шейпить вертикально (TTB, vert), rotated-runs шейпить горизонтально и поворачивать при отрисовке. Baseline-математика: alphabetic↔central конверсия через метрики.

---

## 6. Font fallback и font matching

### CSS font matching (css-fonts-4 §5)

https://www.w3.org/TR/css-fonts-4/#font-matching-algorithm — **по-символьно** (точнее, по grapheme clusters): для каждого символа идём по `font-family` списку; семья → выбор face по осям в порядке: font-stretch → font-style → font-weight (с правилами «ближайшего» и направлением поиска, weight: 400→500→ниже→выше), затем проверка **cmap содержит символ**. `unicode-range` из @font-face фильтрует ДО скачивания (лениво грузим только нужные сабсеты). Если ни одна семья не покрывает — **system font fallback** (шаг 7, тоже per-character); если и он не дал — .notdef (tofu).

Тонкости:

- Кластерное сопоставление (§5.3): последовательность кодпоинтов (base+marks, эмодзи ZWJ) должна проверяться целиком; если шрифт рендерит NFC-композицию — можно брать его. Вариационные селекторы (emoji VS15/VS16) выбирают text/color шрифт.
- first available font (определяет метрики для ex/ch, strut) = первый шрифт из списка, который существует и покрывает пробел U+0020.
- Chromium на Windows: НЕ использует IDWriteFontFallback как основной механизм — захардкоженная таблица «core fonts» по скриптам + эвристики, затем (для редких скриптов) DirectWrite fallback. Дизайн-док font proxy: https://www.chromium.org/developers/design-documents/directwrite-font-proxy ; Gecko рассматривал IDWriteFontFallback::MapCharacters: https://bugzilla.mozilla.org/show_bug.cgi?id=1238863
- Windows API: **IDWriteFontFallback::MapCharacters** (https://learn.microsoft.com/en-us/windows/win32/api/dwrite_2/nf-dwrite_2-idwritefontfallback-mapcharacters) — системная цепочка fallback (та же, что в IDWriteTextLayout): даёт mappedFont+mappedLength для префикса текста. Плюс кастомные цепочки через IDWriteFontFallbackBuilder. Для Rust-движка на Windows это самый дешёвый путь получить «системный» fallback, согласованный с ОС; кешировать результат (char range → font) обязательно, вызовы дорогие.

### Метрики и единицы

- Вертикальные метрики живут в трёх местах: **hhea** (ascender/descender/lineGap), **OS/2 sTypo\*** (typoAscender/Descender/LineGap), **OS/2 usWin\*** (winAscent/Descent — исторически clipping-box). Платформы читают разное: Windows-браузеры — usWin (или sTypo при флаге **USE_TYPO_METRICS** в fsSelection), macOS — hhea. Отсюда разнобой `line-height: normal` и обрезка глифов между платформами. Разбор: https://www.maxkohler.com/posts/2022-02-19-fixing-vertical-metrics/ , https://glyphsapp.com/learn/vertical-metrics , гайд Google Fonts: http://googlefonts.github.io/gf-docs/VerticalMetrics/
- Для собственного движка: выбрать детерминированную стратегию (например: если USE_TYPO_METRICS → sTypo; иначе usWin для A/D как в Windows-браузерах; lineGap из hhea) и зафиксировать её — это определяет `normal`, strut и совместимость скриншотов с Chrome на Windows.
- **ex** = x-height (OS/2 sxHeight, иначе — измерить глиф 'x'); **ch** = advance ноля '0' (если нет — 0.5em); **ic** = advance '水'; **cap** = capHeight. Метрико-зависимые единицы требуют реального шрифта → зависимость parsing→font loading.
- **@font-face дескрипторы override**: ascent-override/descent-override/line-gap-override/size-adjust — способ выровнять fallback-метрики под веб-шрифт (борьба с CLS): https://developer.chrome.com/blog/font-fallbacks

---

## 7. Font rendering: hinting, gamma, DirectWrite

### Общее

- **Hinting** — инструкции TrueType / autohint, подгоняющие outline к пиксельной сетке. На high-DPI и при DirectWrite «natural» режимах роль минимальна (только вертикальная подгонка или вообще без).
- **Gamma-коррекция** блендинга: текст блендится в нелинейном sRGB; без компенсации тёмный текст на светлом фоне выглядит тоньше/толще. FreeType-эссе про stem darkening и гамму (лучший обзор проблемы): https://freetype.org/freetype2/docs/hinting/text-rendering-general.html — **stem darkening** = утолщение штрихов на малых кеглях как компенсация линейного блендинга (делают CoreText, Adobe CFF-растеризатор; DirectWrite делает похожее через enhanced contrast).

### DirectWrite (Windows-бэкенд)

- **IDWriteRenderingParams** (https://learn.microsoft.com/en-us/windows/win32/api/dwrite/nn-dwrite-idwriterenderingparams): gamma (из реестра ClearType tuner, обычно 1.8–2.2), enhancedContrast, clearTypeLevel (0=grayscale…1=полный субпиксель), pixelGeometry (RGB/BGR), renderingMode. Создание кастомных: IDWriteFactory::CreateCustomRenderingParams (в dwrite_3 — DWRITE_RENDERING_MODE1). Что реально крутит ClearType Tuner: https://blog.yuo.be/2025/05/20/what-does-each-step-in-the-cleartype-tuner-do/
- **Режимы**: DWRITE_RENDERING_MODE_GDI_CLASSIC/GDI_NATURAL (совместимость), NATURAL (вертикальный хинтинг, субпиксельные advance по X), NATURAL_SYMMETRIC (сглаживание по обеим осям — дефолт для современного текста), ALIASED. Measuring mode должен соответствовать rendering mode (NATURAL → IDEAL/NATURAL измерения, GDI_CLASSIC → GDI_CLASSIC), иначе метрики разойдутся с растром.
- **IDWriteTextLayout vs raw glyph runs**: TextLayout — полный «встроенный движок» (itemization → MapCharacters fallback → шейпинг Uniscribe-наследником → line breaking → выравнивание) с колбэком-рендерером. Браузеры его НЕ используют для контента: собственный контроль над шейпингом (HarfBuzz), брейкингом и кешами обязателен; DirectWrite оставляют только **растеризацию и метрики**: IDWriteFontFace::GetDesignGlyphMetrics/GetGlyphRunOutline, DrawGlyphRun у D2D, либо IDWriteGlyphRunAnalysis::CreateAlphaTexture для CPU-растеризации глифов в атлас. Для Rust-движка со своим line breaker путь тот же: harfrust шейпит по glyph id, DirectWrite (или skrifa+свой растеризатор) рисует. Обзор глифов/glyph runs: https://learn.microsoft.com/en-us/windows/win32/directwrite/glyphs-and-glyph-runs
- **Вертикальный текст в DWrite**: у IDWriteTextLayout — SetReadingDirection(TOP_TO_BOTTOM)+SetFlowDirection(RIGHT_TO_LEFT), и тогда рендерер обязан реализовать **IDWriteTextRenderer1**: DrawGlyphRun получает **DWRITE_GLYPH_ORIENTATION_ANGLE** (0/90/180/270) — угол поворота ран-глифов вокруг origin; IDWriteAnalyzer1 (GetGlyphOrientationTransform) выдаёт матрицу+трансляцию. https://learn.microsoft.com/en-us/windows/win32/api/dwrite_2/nf-dwrite_2-idwritetextrenderer1-drawglyphrun , общий гайд: https://learn.microsoft.com/en-us/windows/desktop/DirectWrite/vertical-text . При собственном лейауте это не нужно: сам поворачиваешь transform у DrawGlyphRun (isSideways=true для upright CJK глифов из vmtx-метрик — отдельный флаг DWRITE_GLYPH_RUN::isSideways!).
- Гочи DWrite: (1) glyph run analysis рендерит в texture с уже применённой гаммой/контрастом — при композитинге в linear space двойная гамма; (2) субпиксельный AA несовместим с трансформациями/прозрачными фонами — нужен fallback в grayscale (Chromium так делает при композитинге слоёв); (3) метрики GDI vs Ideal дают разную ширину — не смешивать.

---

## 8. Сводка решений для Rust-движка (DirectWrite на Windows)

| Слой | Рекомендация |
|---|---|
| Shaping | **harfrust** (read-fonts), cluster level 1, paragraph-level шейпинг по runs, UNSAFE_TO_BREAK для ре-брейка |
| Сегментация runs | script (UAX #24 через icu4x properties) + bidi level (unicode-bidi) + orientation (UTR #50) + emoji presentation |
| Line breaking | **icu_segmenter** (UAX #14 + SEA) как источник кандидатов; **hyphenation** crate для `hyphens:auto`; greedy по умолчанию; score-based проход (модель Ishii) для balance/pretty поверх тех же кандидатов |
| Bidi | unicode-bidi: levels на параграф, reorder_line на строку после брейка; isolates по css-writing-modes |
| Inline layout | схема LayoutNG: плоский текст параграфа + InlineItems; half-leading модель CSS2 §10.8; strut; central baseline для vertical |
| Font matching | css-fonts-4 §5 per-cluster; системный fallback через IDWriteFontFallback::MapCharacters с кешем; метрики: USE_TYPO_METRICS ? sTypo : usWin (паритет с Chrome/Windows) |
| Rendering | DWrite только растр: NATURAL_SYMMETRIC + IDWriteGlyphRunAnalysis в атлас; гамма/контраст из системных RenderingParams; grayscale-fallback на трансформы; isSideways для upright vertical |
| Референсы | parley (архитектура стека), Blink platform/fonts README, LayoutNG inline README |

### Ключевые URL одним списком

- HarfBuzz manual: https://harfbuzz.github.io/ (clusters: https://harfbuzz.github.io/working-with-harfbuzz-clusters.html)
- harfrust: https://github.com/harfbuzz/harfrust ; rustybuzz: https://github.com/harfbuzz/rustybuzz ; swash: https://github.com/dfrg/swash ; parley: https://github.com/linebender/parley
- UAX #14: https://www.unicode.org/reports/tr14/ ; icu_segmenter: https://docs.rs/icu_segmenter ; xi-unicode: https://docs.rs/xi-unicode/latest/xi_unicode/ ; hyphenation: https://crates.io/crates/hyphenation
- Score-based line breaking (Ishii): https://gwern.net/doc/cs/css/2024-ishii.pdf ; pretty: https://developer.chrome.com/blog/css-text-wrap-pretty ; WebKit pretty: https://webkit.org/blog/16547/better-typography-with-text-wrap-pretty/
- LayoutNG inline: https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/core/layout/inline/README.md ; Blink text stack: https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/platform/fonts/README.md ; RenderingNG: https://developer.chrome.com/docs/chromium/layoutng
- css-inline-3: https://drafts.csswg.org/css-inline-3/ ; CSS2 visudet: https://www.w3.org/TR/CSS2/visudet.html ; font metrics deep dive: https://iamvdo.me/en/blog/css-font-metrics-line-height-and-vertical-align
- UAX #9: https://www.unicode.org/reports/tr9/ ; unicode-bidi crate: https://crates.io/crates/unicode-bidi
- UTR #50: https://www.unicode.org/reports/tr50/ ; css-writing-modes-4: https://www.w3.org/TR/css-writing-modes-4/ ; vert/vrt2: https://blogs.adobe.com/CCJKType/2013/08/tale-of-three-features.html ; OpenType features u-z: https://learn.microsoft.com/en-us/typography/opentype/spec/features_uz
- css-fonts-4: https://www.w3.org/TR/css-fonts-4/ ; MapCharacters: https://learn.microsoft.com/en-us/windows/win32/api/dwrite_2/nf-dwrite_2-idwritefontfallback-mapcharacters ; DWrite font proxy (Chromium): https://www.chromium.org/developers/design-documents/directwrite-font-proxy ; font fallback metrics overrides: https://developer.chrome.com/blog/font-fallbacks
- Vertical metrics: http://googlefonts.github.io/gf-docs/VerticalMetrics/ ; https://glyphsapp.com/learn/vertical-metrics ; https://www.maxkohler.com/posts/2022-02-19-fixing-vertical-metrics/
- FreeType text rendering essay: https://freetype.org/freetype2/docs/hinting/text-rendering-general.html ; IDWriteRenderingParams: https://learn.microsoft.com/en-us/windows/win32/api/dwrite/nn-dwrite-idwriterenderingparams ; IDWriteTextRenderer1::DrawGlyphRun: https://learn.microsoft.com/en-us/windows/win32/api/dwrite_2/nf-dwrite_2-idwritetextrenderer1-drawglyphrun ; DWrite vertical text: https://learn.microsoft.com/en-us/windows/desktop/DirectWrite/vertical-text ; ClearType tuner разбор: https://blog.yuo.be/2025/05/20/what-does-each-step-in-the-cleartype-tuner-do/
