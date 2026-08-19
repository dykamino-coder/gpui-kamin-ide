# Конспект: рендеринг и композитинг браузерных движков

Ориентир: маленький движок на Rust поверх GPU-фреймворка со сценой примитивов (quad/sprite/path), слоями и стабильной сортировкой по order.

---

## 1. Paint order: CSS 2.1 Appendix E, stacking contexts, z-index

**Спека:** CSS 2.1 Appendix E "Elaborate description of Stacking Contexts" — https://www.w3.org/TR/CSS21/zindex.html (та же глава в CSS 2.2: https://www.w3.org/TR/CSS22/zindex.html). Краткая версия — CSS 2.1 §9.9.1.

### Краткий порядок (7 слоёв, §9.9.1)
Внутри одного stacking context (SC), сзади вперёд:
1. background + borders элемента, образующего SC;
2. дочерние SC с отрицательным z-index (сначала самый отрицательный);
3. in-flow, non-inline-level, non-positioned потомки (фоны/рамки блоков);
4. non-positioned floats;
5. in-flow, inline-level, non-positioned потомки (текст, inline-blocks, inline-tables);
6. дочерние SC с z-index: 0 и positioned потомки с z-index: auto/0;
7. дочерние SC с положительным z-index (сначала наименьший).

### Полный алгоритм Appendix E (шаги 1–10)
Рекурсивная функция «paint stacking context»:
1. Если элемент — root: фон canvas (background color/image root'а распространяется на весь canvas).
2. Если элемент block-level (block/list-item/…): его background color → background image → border.
3. Stacking contexts потомков с отрицательным z-index, по возрастанию z-index, при равенстве — tree order. Каждый рисуется атомарно (рекурсивный вызов этого же алгоритма).
4. Для in-flow, non-positioned, block-level потомков (в tree order): их background color → background image → border.
5. Все non-positioned floats (в tree order); каждый float рисуется как **pseudo-stacking-context**: атомарно по этому же алгоритму, но его positioned потомки и вложенные настоящие SC всплывают в родительский SC.
6. Если элемент — inline, образующий SC: рисуется только его собственный inline-контент внутри line boxes.
7. Иначе: сначала для самого элемента, затем для всех in-flow, non-positioned потомков в tree order — текст, decoration, replaced content, inline backgrounds/borders (детализация по line boxes).
8. Positioned потомки с z-index: auto или 0 (в tree order). z-index:auto → pseudo-stacking-context (атомарен для рисования, но не изолирует positioned потомков); z-index:0 → настоящий SC.
9. Stacking contexts потомков с положительным z-index, по возрастанию, при равенстве — tree order.
10. Outlines элемента и всех потомков (поверх всего в данном SC).

### Ключевые концепции
- **Атомарность SC**: содержимое SC не может перемежаться с содержимым другого SC. Отрицательный z-index ребёнка ставит его **над** background родительского SC, но **под** его блоковым контентом (шаг 3 после шага 2).
- **z-index работает только у positioned** элементов (в CSS 2.1) и у flex/grid items (CSS3).
- **Создание SC (современный список, не только CSS 2.1):** root; positioned + z-index ≠ auto; `opacity < 1`; `transform`, `filter`, `backdrop-filter`, `perspective`, `clip-path`, `mask`; `isolation: isolate`; `mix-blend-mode` ≠ normal; `will-change` соответствующих свойств; `contain: paint/layout`; `position: fixed/sticky` (в большинстве движков). Хорошее объяснение ловушки с opacity: Philip Walton, "What No One Told You About Z-Index" — https://philipwalton.com/articles/what-no-one-told-you-about-z-index/
- Актуализация спеки: CSS Positioned Layout Module Level 3 (painting order) — https://www.w3.org/TR/css-position-3/

### Как движки строят display list
- Движки не сортируют весь DOM глобально: рекурсивный обход дерева (layout tree / layer tree) в порядке Appendix E **эмитит плоский display list** — порядок элементов списка и есть paint order.
- **Gecko**: layout строит `nsDisplayList` напрямую из фреймов, с сортировкой positioned-детей по z-index; затем список конвертируется в WebRender display list. (Retained display lists: диффятся между кадрами.) Обзор: https://firefox-source-docs.mozilla.org/gfx/RenderingOverview.html
- **Blink/WebKit**: дерево `PaintLayer`/`RenderLayer`; у каждого SC — кэшированные списки `negZOrderList` / `posZOrderList` (positioned дети, отсортированные по z-index стабильно). Paint обходит: собственный фон → neg-list → normal flow → pos-list. См. README paint: https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/paint/README.md

### Применимость к маленькому движку
- Не нужен глобальный sort-key: **рекурсивный painter в порядке Appendix E**, эмитящий примитивы в плоский список, даёт корректный порядок бесплатно. Стабильная сортировка нужна только детям каждого SC: ключ `(z_bucket, z_index, tree_order)`, где z_bucket ∈ {negative, zero/auto, positive}.
- Практичное упрощение: каждый элемент рисовать в 3 фазы — background/border, content, outline; float'ы и inline-уровень можно свести к «фоны блоков раньше текста того же поддерева».
- SC = граница слоя, если у него opacity/filter/blend — тогда нужен offscreen (см. §2 pictures). Иначе SC — чисто логическая скобка порядка.

---

## 2. WebRender (Servo/Firefox)

**Ключевые ссылки:**
- Репозиторий: https://github.com/servo/webrender
- Обзор архитектуры (wiki): https://github.com/servo/servo/wiki/Webrender-Overview
- Firefox Rendering Overview (WebRender в Gecko, лучший общий док): https://firefox-source-docs.mozilla.org/gfx/RenderingOverview.html
- Lin Clark, "The whole web at maximum FPS: How WebRender gets rid of jank" (2017, идеология «весь paint на GPU каждый кадр, как игровой движок»): https://hacks.mozilla.org/2017/10/the-whole-web-at-maximum-fps-how-webrender-gets-rid-of-jank/
- Документация модуля pictures/picture caching (rustdoc с большими комментариями): https://doc.servo.org/webrender/picture/index.html
- PR picture caching: https://github.com/servo/webrender/pull/3379
- Блог gfx-команды (newsletters, детали батчинга/кэшей): https://mozillagfx.wordpress.com/2018/11/29/webrender-newsletter-32/ и https://mozillagfx.wordpress.com/2018/12/13/webrender-newsletter-33/
- Блог nical (инженер WebRender): https://nical.github.io/ ; про аллокацию атласов: https://nical.github.io/posts/etagere.html ; про GPU-память и инстансинг: https://nical.github.io/posts/rust-2d-graphics-02.html

### Конвейер
```
Display List (сериализованный, self-contained blob, IPC из content-процесса)
  → Scene Building (поток scene builder: флаттенинг, spatial tree, дерево Pictures, интернирование примитивов)
  → Frame Building (per-frame: видимость, разрешение clip chains, batching, GPU cache update)
  → Renderer (GL: инстансированные quad'ы, render task graph, composite)
```
- **Display list**: плоский бинарный список item'ов (rect, text run, image, gradient, box-shadow, iframe, push/pop stacking context, push/pop clip, scroll frame). Полностью самодостаточен — переживает IPC и переход C++→Rust.
- **Scene building** в отдельном потоке: тяжёлая работа (построение дерева Pictures, интернирование) не блокирует кадры. Одна сцена → много кадров (скролл/анимация transform не требуют rebuild сцены — только frame building).
- **Frame building**: вычисление видимости, назначение примитивов тайлам, построение **render task graph** (offscreen-задачи: blur, clip-маски, box-shadow, свёртки) с аллокацией в пул render targets, генерация батчей.
- **Renderer**: почти вся геометрия — axis-aligned quad'ы, «умные» шейдеры; тысячи прямоугольников одним instanced draw call. Два прохода: **opaque pass спереди назад с z-buffer** (экономия fill rate) и **alpha pass сзади вперёд** с блендингом. Батчи ломаются сменой шейдера/текстуры.

### Spatial tree и clip chains (clip/scroll tree)
- **Spatial tree**: узлы = reference frames (transform), scroll frames, sticky frames. Каждый примитив ссылается на spatial node id. Скролл = изменение оффсета узла, без пересборки сцены и без repaint.
- **Клипы отдельно от иерархии**: примитив несёт **clip chain** — список клипов (rounded rect, image mask), каждый со своим spatial node. Простые rounded-rect клипы применяются прямо в шейдере, сложные — рендерятся в маску render task'ом.

### Pictures и picture caching
- **Picture** = поддерево примитивов, которое может рендериться в offscreen surface (нужно для opacity как группы, filter, mix-blend-mode) либо инлайниться (pass-through), если эффект можно применить на месте.
- **Picture caching**: сцена режется на небольшое число **slices** (по признаку «скроллится вместе»), каждый slice — сетка крупных тайлов (~2048×512 для контента; исторически 128×128 для UI-slice). Для каждого тайла на каждый кадр строится **список зависимостей** (примитивы, их transform'ы, клипы, анимируемые значения) и сравнивается с прошлым кадром: не изменилось → тайл не перерастеризуется. Кэшируется результат **растеризации**, а композиция тайлов дешёвая — и может быть отдана OS-композитору (DirectComposition / CoreAnimation), тогда скролл вообще не будит GL.
- **Interning**: примитивы интернируются между сценами — стабильные id для сравнения зависимостей.

### Применимость
- Модель «примитив = quad + шейдер по типу» идеально ложится на движок quad/sprite. Брать: spatial tree отдельно от примитивов (скролл/анимация transform без пересборки), clip chain как ссылка, а не вложенность.
- Opaque front-to-back с depth buffer — дешёвый большой выигрыш, если много непрозрачных прямоугольников.
- Picture caching в мини-варианте: per-слой тайлы + список зависимостей (id + хэш стиля + transform) вместо пиксельного диффа.

---

## 3. Chromium compositing: cc, Slimming Paint / CompositeAfterPaint, property trees

**Ключевые ссылки:**
- "How cc Works" (главный док по композитору): https://chromium.googlesource.com/chromium/src/+/lkgr/docs/how_cc_works.md
- "Life of a Pixel" (слайды-введение во весь конвейер, обновляются): http://bit.ly/lifeofapixel (обзор: https://www.bram.us/2022/01/16/life-of-a-pixel/)
- RenderingNG обзор: https://developer.chrome.com/docs/chromium/renderingng ; архитектура: https://developer.chrome.com/docs/chromium/renderingng-architecture ; анонс: https://blog.chromium.org/2021/10/renderingng.html ; BlinkNG deep-dive: https://developer.chrome.com/docs/chromium/blinkng
- Blink paint README (paint chunks, display items, PaintArtifactCompositor): https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/paint/README.md
- cc README: https://chromium.googlesource.com/chromium/src/+/HEAD/cc/README.md
- Легаси-док (полезен историей layerization): https://www.chromium.org/developers/design-documents/gpu-accelerated-compositing-in-chrome/

### Конвейер (main thread → impl thread → viz)
```
DOM → Style → Layout → Pre-paint (построение property trees + invalidation)
→ Paint (display items / PaintOps → PaintArtifact: чанки + property tree state)
→ Layerize (PaintArtifactCompositor: paint chunks → cc::Layers)  [CompositeAfterPaint]
→ Commit (main → compositor thread) → Tiling/Raster (Skia, пул воркеров)
→ Activate (pending tree → active tree) → Draw (quads) → viz (display compositor, агрегация) → GPU
```

### Paint artifacts (PaintOps)
- Paint эмитит **display items** — по сути записанные Skia-команды (`cc::PaintOp` в `PaintOpBuffer` — хромовская обёртка над идеей SkPicture: сериализуемая, с анализом). Paint ничего не рисует — только записывает.
- Display items группируются в **paint chunks**; каждый чанк несёт ссылку на состояние property trees (transform/clip/effect node id). Итог paint'а — **PaintArtifact**.

### Property trees
- Четыре дерева вместо одной иерархии слоёв: **transform**, **clip**, **effect** (opacity, filter, blend, маски), **scroll**. Развязывают «геометрию/эффекты» от «порядка рисования»: список слоёв — плоский, отсортированный по paint order, а каждый слой/чанк указывает узлы деревьев.
- Решают экспоненциальные краевые случаи старой модели (клип от одного предка + transform от другого). Compositor thread по деревьям сам двигает скролл и анимации transform/opacity без main thread.
- Blink строит свои property trees в pre-paint; при коммите они конвертируются в cc property trees; некомпозитные узлы «спекаются» в display items (`PaintChunksToCcLayer`).

### Slimming Paint → CompositeAfterPaint (CAP)
- Исторически (до ~2021): решения о layerization принимались **до** paint (по «compositing triggers»: transform 3D, video, canvas, will-change…), паинт шёл в GraphicsLayer-дерево — источник багов и лишних слоёв.
- **Slimming Paint v1**: paint переведён на display items. **v2 / CompositeAfterPaint**: сначала полный paint в единый PaintArtifact с property trees, потом **layerization как оптимизация**: `PaintArtifactCompositor` группирует чанки в cc::Layers (что анимируется/скроллится — отдельный слой, остальное сливается). Композитинг стал чистой пост-обработкой paint'а. Shipped в 2021 (RenderingNG).

### cc (compositor)
- Две стороны: `LayerTreeHost` (main) и `LayerTreeHostImpl` (impl thread) с **pending tree** (куда коммитят и растят тайлы) и **active tree** (что рисуется) — активация только когда нужные тайлы готовы (иначе checkerboarding).
- Impl thread обслуживает скролл и композиторные анимации даже когда main thread занят JS.
- Выход cc — **CompositorFrame**: render passes + draw quads (tile, texture, solid color, render-pass quad). Процесс **viz** агрегирует surfaces от всех фреймворков/iframe'ов (SurfaceAggregator) и рисует единый кадр GPU.

### Применимость
- Property trees — главная идея на вынос: держать transform/clip/effect как отдельные деревья с id, примитивы — в плоском списке в paint order со ссылками на узлы. Это даёт скролл/анимации без пересборки сцены и без вложенных матриц в каждом примитиве.
- CAP-урок: не решать про слои заранее; сначала полный список примитивов, затем группировка в слои как оптимизация.

---

## 4. Skia в браузерах: recording, Ganesh/Graphite, градиенты/тени/blur

**Ключевые ссылки:**
- Сайт: https://skia.org (API/архитектура)
- Анонс Graphite: https://blog.google/chromium/introducing-skia-graphite-chromes/ (обзор: https://www.phoronix.com/news/Chromium-Skia-Graphite)
- Обсуждение blur-алгоритма: https://groups.google.com/g/skia-discuss/c/mL2iaiwulmc ; box blur теория: https://en.wikipedia.org/wiki/Box_blur
- WebKit-баг про переход FEGaussianBlur на скиевский blur (описан алгоритм 3 box passes): https://bugs.webkit.org/show_bug.cgi?id=73949

### Как браузеры используют Skia
- Chrome/Blink: paint **записывает** команды (PaintOpBuffer, наследник идеи `SkPicture`/`SkPictureRecorder`) — replay потом, на растер-воркерах, тайлами, в GPU-бэкенд Skia. Запись = дёшево на main thread, растр = параллелен и отложен.
- **Ganesh** — классический GPU-бэкенд: op-based, батчинг/слияние draw ops, GL-центричный дизайн (много специализированных путей). **Graphite** — замена: заточен под Metal/Vulkan/D3D12(Dawn/WebGPU); отложенная запись команд с реордерингом для GPU-throughput, **многопоточные Recorder'ы**, прекомпиляция pipeline'ов на фоне (убирает shader-compilation jank). Уже в Chrome на Apple Silicon (~+15% MotionMark).

### Градиенты
- Linear/radial/conic — шейдерные: вычисление t по позиции + lookup. Малое число стопов — аналитически в шейдере (uniform-ы), много стопов — 1D-текстура рампы. Важно: интерполяция в правильном цветовом пространстве и premultiplied alpha.

### Blur (Gaussian)
- Гауссов blur **сепарабелен**: два 1D-прохода (H затем V), O(r) вместо O(r²).
- CPU-путь Skia (`SkBlurMask`/`SkMaskBlurFilter`): **три последовательных box blur'а** — по центральной предельной теореме тройной box ≈ Gaussian; box blur реализуется скользящей суммой за O(1) на пиксель независимо от радиуса. Размер box вычисляется из sigma; итоговый вес = box³.
- GPU-путь: для больших sigma сначала **downscale** (blur низкочастотен), потом сепарабельная свёртка, потом upscale — радиус в шейдере остаётся малым.
- WebRender делает так же: blur как render task на пониженном разрешении.

### Box-shadow (outset/inset)
- Наивно: растеризовать прямоугольник → blur → tint цветом тени. Дорого для больших элементов.
- Оптимизация (и в Skia, и в WebRender): blur скруглённого прямоугольника **однороден вдоль рёбер** — достаточно посчитать один размытый угол/маленький rrect и растянуть **nine-patch'ем** (9 сеточных quad'ов: углы без масштаба, рёбра тянутся). Skia дополнительно имеет аналитическую оценку blurred rrect. Кэшировать по ключу (corner radii, sigma) — тени одинакового стиля переиспользуются.
- **Inset** shadow = та же маска, инвертированная: рисуется внутри padding-box, blur той же техникой, клип по границе элемента; спред меняет размер исходного rrect до blur.

### Применимость
- Для path-примитива: не писать растеризатор — CPU-растр (tiny-skia / скиевская модель) в текстуру + кэш, либо GPU-тесселяция (lyon). Blur: только сепарабельный + downscale; тени rrect: nine-patch + кэш углов — это 95% реальных CSS-теней почти бесплатно.

---

## 5. Текстовая отрисовка: glyph atlas, subpixel AA, GPU text

**Ключевые ссылки:**
- WebRender text-rendering.md (лучший разбор блендинга текста: grayscale / subpixel / subpixel-с-известным-фоном, вывод формул): https://github.com/servo/webrender/blob/main/webrender/doc/text-rendering.md
- Issue про subpixel AA в WebRender: https://github.com/servo/webrender/issues/464
- Аллокация glyph-атласа в WebRender (shelf packing, крейт etagere): https://nical.github.io/posts/etagere.html
- Subpixel-позиционирование через варианты глифа в атласе: https://rasmusbarr.github.io/blog/subpixelglyph.html
- Практика glyph atlas + kerning (Warp, terminal на GPU): https://www.warp.dev/blog/adventures-text-rendering-kerning-glyph-atlases
- Dual-source blending для subpixel-текста: https://arkanis.de/weblog/2023-08-14-simple-good-quality-subpixel-text-rendering-in-opengl-with-stb-truetype-and-dual-source-blending/
- SDF-текст: Green (Valve, SIGGRAPH 2007) "Improved Alpha-Tested Magnification…": https://steamcdn-a.akamaihd.net/apps/valve/2007/SIGGRAPH2007_AlphaTestedMagnification.pdf ; MSDF: https://github.com/Chlumsky/msdfgen ; вектора на GPU: https://wdobbie.com/post/gpu-text-rendering-with-vector-textures/ ; Evan Wallace: https://medium.com/@evanwallace/easy-scalable-text-rendering-on-the-gpu-c3f4d782c5ac

### Растровый glyph atlas (путь браузеров)
- Глифы растеризуются платформенным растеризатором (FreeType / DirectWrite / CoreText) — с хинтингом и гаммой, идентично нативному тексту — и пакуются в texture atlas. Отрисовка глифа = textured quad из атласа → батчится в один draw call на весь текст.
- **Ключ кэша**: font id + размер + вариации + флаги рендера + **квантованный subpixel-оффсет** (WebRender: 4 горизонтальных позиции — 0, ¼, ½, ¾ px; т.е. до 4 растров одного глифа) + иногда компоненты transform (skew/scale для повёрнутого текста растрят глиф в экранном разрешении).
- Аллокация в атласе: shelf packing (etagere) — простой, с дефрагментацией по поколениям; eviction по LRU. Chromium/Ganesh держит собственный glyph atlas (GrAtlas/SkStrike) и точно так же батчит текст; при Graphite — то же с новым менеджментом.
- Zoom-анимации: растровый атлас не масштабируется красиво — движки либо перерастеризуют по окончании жеста, либо терпят blur во время pinch-zoom.

### Subpixel AA (ClearType-стиль)
- Маска глифа с **отдельной альфой на R/G/B** (LCD-геометрия). Блендинг требует dual-source blending (`SRC1` в GL/Vulkan) — цвет и per-channel альфа из шейдера двумя выходами; либо two-pass (сначала «вырезать» фон, потом добавить цвет), либо — если фон известен (opaque тайл) — можно предскомпозить. Всё это выведено в webrender text-rendering.md.
- Subpixel AA валиден только поверх непрозрачного фона; на прозрачных слоях/при transform движки откатываются на grayscale AA. macOS давно отключил subpixel AA системно; Windows/DirectWrite ещё использует.
- Гамма: блендинг текста перцептуально корректен только с учётом gamma correction; платформенные растеризаторы запекают contrast/gamma в маску.

### SDF vs растровый атлас
- **SDF** (signed distance field): в атласе — поле расстояний; шейдер получает резкий край на любом масштабе через smoothstep; один растр на все размеры, дёшево масштабировать/обводить/тенить. Минусы: скруглённые углы и потеря тонких деталей на малых размерах (лечится MSDF — multi-channel), **нет хинтинга и subpixel AA** → на мелком UI-тексте заметно хуже растра.
- Браузеры выбирают растровый атлас ради fidelity малых кеглей; SDF — выбор игр/карт (mapbox) и UI с постоянным зумом.

### Применимость
- Брать: растровый атлас (swash/ab_glyph + etagere в Rust), ключ с квантованным subpixel-X, grayscale AA по умолчанию; dual-source blending — опционально позже. Кэш shaped runs (rustybuzz) отдельно от кэша глифов — шейпинг дороже растра.

---

## 6. Тайлинг, растеризация, invalidation / damage tracking

**Ключевые ссылки:**
- how_cc_works (тайлинг, raster priorities, pending/active): https://chromium.googlesource.com/chromium/src/+/lkgr/docs/how_cc_works.md
- Picture caching WebRender (per-tile dependencies): https://doc.servo.org/webrender/picture/index.html и https://github.com/servo/webrender/pull/3379
- Blink paint invalidation: https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/paint/README.md
- RenderingNG (какие стадии конвейера скипаются): https://developer.chrome.com/docs/chromium/renderingng-architecture

### Tile-based rasterization (модель cc)
- Каждый растрируемый слой режется на тайлы (типично 256×256 или 512×512). Плюсы: (a) растр параллелится по воркерам; (b) память ограничена видимым + prepaint-каймой, а не размером всего слоя; (c) инвалидция перерастеризует только задетые тайлы; (d) при скролле новые тайлы доращиваются, старые выкидываются.
- **Приоритеты**: тайлы во viewport → кайма в направлении скролла → остальное; при нехватке бюджета — low-res заглушки (или checkerboard). Активация pending→active дерева ждёт готовности видимых тайлов, чтобы не мигать.

### Invalidation (что перерисовать)
- **Blink**: изменение стиля/лейаута помечает paint invalidation; после re-paint display items **диффятся** (кэш display items) — реально изменившиеся регионы дают invalidation rects, которые мапятся в тайлы слоя.
- **Gecko**: retained display lists — новый частичный display list мёржится со старым, diff даёт damage.
- **WebRender**: без пиксельных rect'ов — **сравнение списков зависимостей тайла** (интернированные id примитивов + их значения + transform'ы + клипы): тайл валиден, пока все зависимости не изменились. Плюс per-tile dirty rect для частичного перерастра внутри тайла.

### Damage / partial present
- Damage агрегируется до самого экрана: viz DamageTracker собирает damage от слоёв/сюрфейсов в damage rect кадра; при поддержке — **partial swap** (scissor + буфер с сохранением) или dirty-rects OS-композитору (DirectComposition, EGL_KHR_partial_update / buffer_age на EGL). Скролл при picture caching = сдвиг готовых тайлов OS-композитором, GPU-растр не просыпается.
- Отдельный трюк: анимации transform/opacity/scroll вообще не создают damage растра — меняются только property-tree узлы, recomposite без re-raster (главный смысл разделения paint/composite; в RenderingNG стадии конвейера явно скипаются).

### Применимость к маленькому движку
Минимально достаточная схема:
1. Retained-сцена примитивов с stable id и generation counter.
2. Слои = поверхности со своим transform-узлом; внутри слоя — тайловая сетка только если слой большой/скроллится (маленькие виджеты — один тайл).
3. Инвалидция в стиле WebRender: per-tile список (prim id, hash(стиль+геометрия), spatial node epoch); дифф списков вместо трекинга rect'ов — сильно проще в реализации и не имеет багов «забыли инвалидировать».
4. Композиция тайлов каждый кадр — дёшево (quad'ы); отдельно копить frame damage rect для partial present, если фреймворк позволяет.
5. Скролл/анимации — только через узлы spatial tree, никогда через мутацию примитивов.

---

## Сквозные выводы для Rust-движка

1. **Порядок**: рекурсивный обход в порядке Appendix E → плоский список; стабильный sort по (z-bucket, z-index, tree order) только среди детей SC. Существующая «стабильная сортировка по order» уже совместима: order = позиция в эмитированном списке.
2. **Три дерева вместо вложенности**: spatial (transform+scroll), clip chain, effect. Примитив = данные + 3 ссылки. Это одновременно модель WebRender и property trees Chromium — сошлись независимо.
3. **Offscreen только когда нужно**: группа с opacity/filter/blend → render task в текстуру; всё остальное — inline. Render task graph с пулом таргетов.
4. **Кэшировать растр, не композицию**: тайлы + дифф зависимостей (WebRender-модель проще хромовской и лучше ложится на retained-сцену примитивов).
5. **Текст**: растровый glyph atlas + квантованный subpixel-X + shelf packing; SDF не брать для основного UI-текста.
6. **Дорогие эффекты**: blur = downscale + separable; box-shadow = кэшированный blurred corner + nine-patch; градиенты = шейдер/1D-рампа.
