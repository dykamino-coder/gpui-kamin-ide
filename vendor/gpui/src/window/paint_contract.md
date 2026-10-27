
## Граница механизма и политики хозяина

GPUI исполняет упорядоченные команды рисования в заданном состоянии окна.
Хозяин определяет семантический порядок, атомарные поддеревья и нужные эффекты.
Группа композиции — область вывода в отдельную сцену, а не семантический
контекст хозяина. В частности, правила CSS stacking, выбор подложки текста
и fill policy принадлежат HTML-движку.

### Обязательства вызывающего и сохранение поведения

- Layout и prepaint ребёнка исполняются ровно один раз на проход. Отложить
  paint не означает повторить измерения, регистрацию hit-test или эффекты.
  Identity hit-test, scroll/selection и порядок регистрации остаются частью
  контракта хозяина и сверяются с окончательным paint order отдельно.
- Отложенный ребёнок и снимок живут только в текущем проходе paint. Нельзя
  хранить `AnyElement` между кадрами или документами.
- Порядок исполнения задаёт хозяин. Replay сохраняет порядок команд,
  дробные координаты и существующую растровую привязку без нового округления.
- Задание должно завершиться внутри своей открытой композиционной группы:
  его нельзя вынести за границу opacity, blend, clip, mask или offscreen.
  Произвольный Element — непрозрачная граница; прозрачный адаптер явно
  разрешает перенос ребёнка, сохраняя его окружение и область композиции.
- Typed host contexts принадлежат окну, не глобальному потоку. Снимок
  удерживает тот же `Rc<T>`; значения внутри него не копируются глубоко.
  Отсутствующий тип скрывает одноимённый контекст вызывающего на время replay.

### Снимки и проверяемый replay

`paint_ctx` сохраняет element/entity ids, text style, typed host contexts,
element offsets и глубину exact offsets, transformation stack, mask scale/map,
opacity, content masks и image caches. `raster_paint_ctx` дополнительно сохраняет
весь `RasterFrame`: glyph offset, actual/exact fill transforms, rounded/exact
bounds и непрозрачные typed host raster data. Обычный `PaintCtx` этих полей
не содержит; общий replay наследует frame вызывающего. `RasterPaintCtx` позволяет
вызывающему явно выбрать fill transform перед replay.

`snapshot_paint_state` сохраняет все эти поля, включая generic `RasterFrame`,
точную fill matrix, device scale и стек rem overrides. `with_raster_frame` принимает явные placement
и unsnapped fill transforms, не выбирая подложку или CSS policy. Существующий
общий consumer наследует raster state вызывающего; inline consumer сохраняет
прежний набор glyph/background полей и наследует fill transform. Все три пути
исполняются одним механизмом replay и не меняют layout/prepaint или paint order.

`with_raster_context` временно заменяет только один типизированный слот этого
же frame и восстанавливает его через общий guard, включая unwind. Остальные
host values и raster-геометрия сохраняются без клонирования всей карты.
`ChildPaintContext` заимствуется только на время callback; хозяин завершает
выбор геометрии перед рекурсивным рисованием детей.

Replay проверяет identity окна, прохода и живой композиционной области до
изменения состояния и вызова closure. `PaintError` различает чужое окно/проход,
завершённый проход, другую открытую и уже закрытую группу; fallback нет.
`PaintPassId` и `PaintScopeId` не продлевают жизнь области при клонировании.
Prepaint и paint одного draw разделяют pass; изолированные effect/plane/capture
сцены имеют собственные scope. Transform/clip сохраняются в снимке, поэтому
их лексическое закрытие само по себе не запрещает отложенный replay.

Guards восстанавливают caller environment, typed contexts и raster state также
при unwind; scopes геометрии и scene swaps используют тот же Drop-механизм.
Unwind помечает проход недействительным: продолжать частично нарисованный кадр
нельзя. Внешний draw при abort очищает незавершённый frame и возвращает внешнее
окружение. HTML coordinator владеет очередями прохода и освобождает их через Drop,
в том числе при пойманном unwind. Глобального reset нет.

### Capture и instrumentation

`Window::set_frame_capture` передаёт backend единственный `FrameCaptureSink`
этого окна без клонирования. Backend доставляет завершённые BGRA pixels и
длительность readback. Хозяин владеет сессией, sequence, подписками и выбором
first/latest; endpoint не смешивает кадры разных окон. Замена или уничтожение
endpoint закрывает сессию и завершает ожидающие подписки. Readiness и выбор
результата принадлежат WPT, а не renderer. Production offscreen scene остаётся
общим механизмом GPUI/backend и не зависит от подписок на оконные кадры.

`Application::with_instrumentation` принимает типизированные настройки хозяина
до запуска платформы. Измерения prepaint и GPU принадлежат приложению и явно
передаются backend, включая пересоздание устройства. Toolkit не разбирает
диагностические переменные окружения и не хранит глобальное состояние измерений.

### Инварианты scoped API

- frame-local pass token идентифицирует окно и кадр; scope token дополнительно
  идентифицирует открытую композиционную область, включая offscreen capture;
- snapshot включает всё окружение рисования, generic raster frame, typed host
  contexts и entity/id environment; raster frame хранит исходную дробную
  геометрию и device grid, а не решение хозяина о подложке текста;
- replay принимает только token того же окна, кадра и открытой группы;
  несовпадение возвращает типизированную ошибку без fallback на прямой paint;
- clip/transform/effect scopes исполняются в порядке вызова, не пересортировывают
  jobs и не пропускают отложенную работу через свою границу;
- guards восстанавливают полное внешнее состояние при unwind. Abort уничтожает
  текущий проход, освобождает jobs и не загрязняет следующий документ/кадр;
- очередь jobs принадлежит хозяину; стабильная ограниченная сортировка и
  однократное исполнение не требуют знания CSS внутри GPUI. Нельзя вводить
  повторный minimum scan или бесконечный drain при добавлении новых jobs.

Проверки HTML фиксируют порядок текущих consumers, nested scopes, identity
contexts, raster origin, отклонение неверного replay и восстановление после abort.
CSS очередь живёт в HTML coordinator, который задаёт порядок child visitor
внутри прежних scopes GPUI. Opaque boundary guard и метаданные прозрачных
holders являются общими hooks; GPUI их не интерпретирует. Text-background
и fill policy принадлежат HTML. GPUI передаёт host callback геометрию и
непрозрачные метаданные без интерпретации; snapshot не принимает решений хозяина.

### Immutable command records

`Window::record_paint(source_clip, cx, paint)` consumes source painting once.
The explicit source clip replaces the ambient fragment clip before culling.
An optional host `PaintRecording` lifecycle opens and finishes its own deferred
work inside capture; guard destruction restores or aborts host state.
No element or callback is retained by `PaintRecord` or `PaintPlan`.

A record shares immutable canonical Scene commands and frame-local resources.
It outlives its capture scope, but only in the same live window/draw and raster
scale. This does not relax snapshot checks: PaintState/PaintCtx still require
their original live composition scope. Stale identity or invalid placement is
a typed error before destination writes.

The host composes ordered records and explicit Clip/Group descriptors, including
shared effect ancestors. GPUI does not choose CSS order, source identity,
fragment scheduling, layout generation or hit geometry. Plan playback accepts
no App and invokes no element/host callback. Each invocation adopts fresh group
ids through the existing compositor. Source transforms/raster positions remain
resolved; an explicit source-to-destination translation moves commands once.
The final destination clip follows effects, preserving off-clip filter inputs.
Pending plane contexts, raster state and host contexts are restored on return
or unwind; an abort invalidates the draw rather than granting replay rights.
