# Аудит движка crates/html — 2026-08-19 (4 read-only исследования)

Полные отчёты — в транскриптах сессии; здесь консолидат. Ничего не запускалось.

## Числа

render.rs 6807 строк, computed.rs 4922 (Computed: ~188 полей, ~1.7КБ,
Element несёт 4 таких ≈ 7КБ/узел), lines.rs 3111, interact.rs 2167
(12 кастом-элементов). 238 маркеров «KaminIDE patch» в vendored gpui.
Слои по стрелкам здоровые (value/css → computed → dom → render), циклов нет.

## Срочные фиксы корректности (дешёвые, делать первыми)

1. value.rs:212 parse_hex — байтовые срезы без ascii-проверки: `#aфa`
   РОНЯЕТ процесс. + color_space.rs:440 (NBSP в color-mix), render.rs:2771
   (RT_DBG срез не по границе UTF-8).
2. direct_write.rs:1544 — glyphrundescription без null-проверки (AV);
   :1548/1551 unwrap-ы в колбэке отрисовки.
3. direct_write.rs:705 — UB: мутация контекста через *const→*mut от &T
   без UnsafeCell → let mut + *mut.
4. render.rs:2334 — ключ VerticalText коллизит на одинаковых абзацах
   (одинаковый текст+nodes.len) → порядковый счётчик обхода в ключ.
5. doc.rs:390+fonts.rs:52 — iframe чистит ALIASES @font-face хозяина
   посреди кадра → additive-режим load_faces.
6. fonts.rs:85 faces() и color_space.rs:691 — наивный поиск at-rule ловит
   закомментированные правила → общий помощник поверх css-скана.
7. LATE/IFRAME_DEPTH без drop-guard (паника кадра портит их навсегда) →
   guard по образцу DeferGuard (render.rs:1289).
8. render.rs:3741 iframe строится ДВАЖДЫ за кадр (is_some + unwrap).
9. Мёртвое: css.rs:461 split_words; vendored line_layout.rs:491/589 +
   text_system.rs:691 обёртки (pub vendored — пометить, не терять при
   апгрейде). select.rs+lines.rs:2204 — дублированная структура выделения.

## Перф (порядок: безрисковые пиксельно → аккуратные)

Безрисковые: (а) вынести одноразовые нормализации дерева из render в
Document::new / Rc-узлы вместо deep-clone (ГЛАВНЫЙ пожиратель — клоны
поддеревьев на каждом уровне каждый кадр); (б) Rc<Computed> в Piece +
диета Computed (редкие семейства в Option<Box<Rare>>); (в) env::var →
LazyLock (syscall в горячих путях: HTML_MEASURE на каждый measure!);
(г) кэш IDWriteTextFormat по (family,weight,style,stretch,size);
(д) MEASURED: Vec64+remove(0) → HashMap двухпоколенный (образец — gpui
LineLayoutCache); (е) font_family клоны → as_deref/SharedString;
(ж) measure_key — мемо в Cell; (з) dir=auto/text_id — в построение
Document.

Аккуратные (пиксельный риск): (и) КЭШ СПЛИТОВ строк по (ключ ⊕ limit ⊕
все wrap-флаги) — сплит сейчас гоняется 3-5 раз на абзац за кадр; схема
Blink LayoutNG; (к) двухкадровость CellsClipped схлопнуть: пробы пишут в
prepaint, band читает в paint ТОГО ЖЕ кадра — режет и время WPT-прогона;
(л) x_at квадратичен — префикс-суммы, СОХРАНИТЬ порядок сложения f32;
(м) VerticalText: пропуск MaxContent-прохода при попадании в VT_MEASURED.

## Структура (из аудита архитектуры)

1. Слой box tree в doc.rs (table fixup, wrap_floats, reorder,
   place_named_areas, статический collapse_margins — из кадра в разбор).
2. Распил render.rs: table.rs (~1600), positioned.rs, paragraph.rs.
3. Вычистить рудимент rules() (Option мёртв, as_word_row-путь недостижим,
   константные проверки inline.rs:784/793).
4. Единый реестр свойств (имя→parse→поле→coverage) вместо 4 точек
   регистрации; 1900-строчный match apply_one.
5. Типизированная шина проб вместо 4 ad-hoc thread_local.
6. env ANCH_BG → RenderOpts. Доки: lib.rs врёт про отсутствие трансформов,
   шапка interact.rs врёт «resize».
7. PieceAnnotations: единый сбор wrap/letter/word/shift/autospace-спанов.
8. Headless-тесты: фейковый шейпер для lines (образец metrics-щуп),
   голден-дампы box tree. Пиксельный стенд не ловит «оба неверны одинаково».
9. НЕ сливать 4 пути текста (подтверждено откатами). Карта 13 «пробовали
   и откатили» — в отчёте архитектора (транскрипт).

## SOTA-2026: брать / не брать

Брать: (1) conic-градиент по схеме Skia — t=atan2/2π+frac поверх нашего
4-стоп-патча hlsl, дни; гоча — шов 0/2π; (2) крейт selectors (servo) вместо
самописного матчера — bloom-фильтр + корректность, ~неделя на Element-
адаптер; (3) harfrust 0.13 (официальный порт HarfBuzz) + unicode-vo для
ВЕРТИКАЛЬНОГО шейпинга (честный TTB, vert/vmtx; UTR#50 таблицей) —
DirectWrite остаётся на растеризацию; (4) taffy 0.9→0.12: float/clear
из коробки + кэш замеров 4 слота (до 1000×), 3 breaking-волны, отдельная
задача; (5) модель логической геометрии stylo (WritingMode bitflags +
LogicalRect) — фундамент orthogonal flows (код Servo не тянуть — их
vertical сам не готов).

Не сейчас: Vello (sparse strips 0.0.x beta; промежуточно vello_cpu как
офлайн-растеризатор SVG-путей в спрайт), полный parley-стек, lightningcss
целиком (можно подглядывать грамматики). cssparser — токенизатор частями
после selectors.
