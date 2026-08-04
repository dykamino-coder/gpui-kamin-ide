# 97 — WebView2 Visual Hosting (замена wry-чайлдов)

Цель: вебвью как DirectComposition-визуал ВНУТРИ дерева main-окна.
Убивает: лаг ресайза (композитим сами в кадре), GDI-скругление (clip визуала
со сглаживанием), overlay-окно поповеров (обычные элементы рисуются НАД вебвью).

## Шаги
1. vendored gpui: экспорт dcomp: pub-метод у DirectXRenderer/Window —
   `attach_child_visual(bounds, radius) -> IDCompositionVisual` (device приватен:
   directx_renderer.rs DirectComposition {comp_device, comp_visual}).
   Дерево: root visual → [gpui swapchain visual, wv-визуалы..., popover-слой?]
   ВАЖНО: gpui-контент должен рисоваться и НАД вебвью → структура:
   root → [wv-визуалы (снизу), gpui-визуал (сверху, прозрачный фон в зонах wv)].
   Прозрачные «дыры» под вебвью в gpui-кадре: main сейчас blit непрозрачный —
   перейти на dcomp+premultiplied и для main (мы теперь умеем: env per-window),
   а фон приложения рисовать самим (радиал уже наш).
2. Хостинг: webview2-com: CreateCoreWebView2CompositionController(hwnd main),
   controller.SetRootVisualTarget(наш IDCompositionVisual); Bounds = зона панели.
3. Ввод: WM_MOUSE*/WM_POINTER в зоне вебвью → controller.SendMouseInput
   (форвардинг из gpui wndproc — vendored hook или subclass main);
   фокус: controller.MoveFocus; курсор: CursorChanged event → SetCursor.
4. Мост протокола: заменить wry-обвязку (custom protocol kamin://, ipc
   postMessage) на WebView2 API: AddWebResourceRequestedFilter +
   WebMessageReceived (эквиваленты 1:1).
5. Поэтапность: первым перевести БРАУЗЕР (Web-режим, независим от ipc-моста),
   затем Bridge-вью; wry-путь держать фичефлагом до паритета.
6. После паритета: удалить overlay-окно (поповеры в main над вебвью),
   NCHITTEST-сабкласс, sync/recreate, регион-код, freeze-код.

## Референсы
- research-отчёт (задача a46a32bb): visual hosting официально задокументирован
  MS (windowed-vs-visual-hosting), обязанности ввода перечислены там же.
- vendored gpui: crates.io 0.2.2 + наши патчи (env per-window, last_layout,
  minimap-геттеры) в vendor/gpui, vendor/gpui-component.


## Статус (2026-07-26)
- [x] Шаг 1 ФУНДАМЕНТ: vendored gpui экспортирует dcomp:
  - дерево пересобрано: root-КОНТЕЙНЕР → [underlay..., gpui-визуал сверху]
    (directx_renderer.rs DirectComposition: comp_root, create_underlay_visual, commit)
  - DirectXRenderer::create_underlay_visual_raw() -> Option<*mut c_void> (AddRef)
  - PlatformWindow::dcomp_underlay_visual (default None) + Windows impl
  - gpui::Window::dcomp_underlay_visual() — публичный вход для shell
  Всё компилируется, 64 теста зелёные. В blit-режиме возвращает None.
- [ ] Шаг 2 (СЛЕДУЮЩАЯ СЕССИЯ): webview2-com (свежая версия с crates.io, сверить
  версию windows-crate с vendored gpui) → CreateCoreWebView2EnvironmentWithOptions
  + CreateCoreWebView2CompositionController(hwnd main) →
  controller.SetRootVisualTarget(visual из шага 1); Navigate; Bounds по зоне
  браузер-панели. Прототип за env KAMIN_VISUAL_WV=1: main запускать С dcomp
  (env-флаг off) — wry-вью в этом режиме невидимы, это ожидаемо для прототипа.
- [ ] Шаг 3: ввод (SendMouseInput из forwarded WM_MOUSE* в зоне), фокус, курсор.
- [ ] Шаг 4: мост kamin:// + postMessage на WebResourceRequested/WebMessageReceived.
- [ ] Шаг 5: перевод Bridge-вью, снос overlay-окна/NCHITTEST/sync/региона/масок.


## Прогресс шага 2 (та же сессия)
- [x] webview2-com 0.38 подключён (типы через `windows061` — у webview2-com
  windows 0.61, у shell 0.62; raw COM-указатели совместимы, IUnknown::from_raw)
- [x] wv_visual.rs: environment → CompositionController → SetRootVisualTarget
  (визуал из vendored gpui) → Navigate; SetBounds по probe-зоне
  «browser-viewport» (в visual-режиме web-панель рендерит каркас с зоной)
- [x] ДЫМ ПРОШЁЛ: `[vwv] composition webview LIVE` — контроллер живой,
  bounds применяются
- [!] Визуал ПОД gpui НЕ ВИДЕН: main-окно на dcomp рисует непрозрачный
  корневой фон (root bg + карты) поверх underlay. ВРЕМЕННО AddVisual
  insertAbove=true (поверх gpui) для проверки картинки — ВЕРНУТЬ false.
- [ ] СЛЕДУЮЩЕЕ: «дыра» в gpui-фоне под зоной вебвью: (а) корневой bg
  красить НЕ на root, а по-панельно; (б) в web-зоне не рисовать фон карты;
  тогда underlay-вариант (false) станет видимым. После этого ввод (шаг 3).


## MILESTONE (2026-07-26): РЕНДЕР В ПАНЕЛИ РАБОТАЕТ
Google рисуется composition-визуалом ровно в зоне web-панели (скрин-пруф).
Ключи: (1) позиция = visual.SetOffsetX2/Y2, размер = controller.SetBounds
(0,0,w,h) — SetBounds в composition-режиме НЕ позиционирует; (2) после
SetRootVisualTarget обязателен Commit dcomp-device (Window::dcomp_commit,
зовётся в root render web-ветки каждый кадр); (3) визуал ВРЕМЕННО поверх
gpui (insertAbove=true) — для финала вернуть под + «дыра» в фонах.
Ввод: ресёрч WebView2SampleWinComp запущен (SendMouseInput/cursor/focus/DPI)
— шаг 3 строить строго по его отчёту.


## Шаг 3 (ввод) — конспект ресёрча (полный отчёт: task ac3d5ae8)
- SendMouseInput(eventKind=каст WM-кода, virtualKeys=GET_KEYSTATE_WPARAM,
  mouseData, POINT локально-вебвью в ФИЗ px). Референс: WebView2APISample/
  ViewComponent.cpp (OnMouseMessage/TrackMouseEvents), НЕ WinComp-семпл.
- wheel: WM_MOUSEWHEEL в SCREEN-координатах → ScreenToClient → минус offset;
  mouseData = signed delta (±120).
- down → SetCapture + флаг; up → ReleaseCapture; фильтр: в ректе ИЛИ capture
  ИЛИ WM_MOUSELEAVE. Выход из зоны → синтетический LEAVE (иначе залипшие
  hover/tooltip/курсор). TrackMouseEvent(TME_LEAVE).
- Курсор: add_CursorChanged → get_Cursor(HCURSOR) → хранить, ставить SetCursor
  пока hover над вебвью (НЕ SetClassLongPtr — конфликт с gpui).
- Клавиатура НЕ форвардится: composition-контроллер имеет скрытый child-HWND,
  клавиши идут туда сами при фокусе. Tab-выход: MoveFocusRequested → Handled;
  хоткеи хоста: AcceleratorKeyPressed (async-действие!). Вход: клик или
  Controller.MoveFocus(PROGRAMMATIC).
- DPI 1.25: default ShouldDetectMonitorScaleChanges=TRUE — НИЧЕГО не делать,
  Bounds в физ px (BoundsMode=USE_RAW_PIXELS).
- ГРАБЛИ: put_Bounds обязателен для hit-test ввода (у нас уже);
  NotifyParentWindowPositionChanged на WM_MOVE/WM_MOVING (дропдауны страницы);
  IsVisible(false)+TrySuspend на minimize; teardown: RootVisualTarget(null) →
  дерево → Close; dblclick — Chromium считает сам по двум down;
  app-region:drag → GetNonClientRegionAtPoint (ICoreWebView2CompositionController4).
- Rust-примеров composition нет — портировать ViewComponent.cpp напрямую;
  второй референс: microsoft-ui-xaml/dev/WebView2/WebView2.cpp.

## Лаг ресайза — РЕШЕНИЕ (проверить юзером)
sync_zone(): offset+SetClip визуала каждый кадр (мгновенно, наш композитор,
вызывается из canvas-prepaint каркаса = bounds ТЕКУЩЕГО кадра);
SetBounds (relayout Chromium) — рост шагами 256px с запасом при драге,
финализация точным размером на отпускании.


## Синхронизация кадра (2026-07-26, финал итераций ресайза)
Итог трёх итераций семантики драга:
1. Гистерезис 256px — ОТКЛОНЁН (страница relayout-ится «больше, чем есть»,
   скачет ступенями).
2. Заморозка поверхности — ОТКЛОНЕНА (весь ресайз ждёт отпускания).
3. ФИНАЛ: живой SetBounds каждый кадр + покадровый SetClip/Offset визуала
   (край всегда = сплиттер) + **Commit dcomp В present() vendored-рендерера**
   (не в prepaint! иначе вебвью опережает панель на долю кадра — изменения
   dcomp применяются к экрану мгновенно, а свопчейн-кадр gpui приходит позже).
   canvas каркаса только ставит offset/clip/bounds, Commit убран.

Остаток: вероятностный 1-кадровый разъезд Present↔Commit (разные vblank при
занятом потоке) — юзер видит редкое «двоение» при драге. Кандидаты шлифовки
(после ввода, если мешает): IDCompositionDevice2::WaitForCommitCompletion
перед Present (цена — блокировка кадра) или DCompositionWaitForCompositorClock.


## Шаг 3 ввод — ПЕРВАЯ ВЕРСИЯ СОБРАНА (2026-07-26)
- Мышь через gpui-события каркаса «visual-webview»: move (с MK-маской нажатой
  кнопки), L/R/M down/up, wheel+hwheel (Lines→±120), hover(false)→WM_MOUSELEAVE.
  Координаты: (event − zone origin) × scale (локальные физ. px).
  wv_visual::send_mouse(kind=WM-код, vk, data, x, y).
- Клавиатура: должна заработать сама после клика (скрытый child-HWND берёт
  win32-фокус) — проверить набором в поиске google.
- НЕ сделано: курсор (CursorChanged→SetCursor при hover), Tab-выход
  (MoveFocusRequested), хоткеи (AcceleratorKeyPressed),
  NotifyParentWindowPositionChanged на WM_MOVE (дропдауны страницы),
  IsVisible(false) на minimize, teardown-порядок.

## Шаг 3 ввод — ДОДЕЛАН (2026-07-26, вторая итерация)

Подтверждено юзером: мышь, курсор, клавиатура, тулбар — работают.

- **Клавиатура (корень)**: цикл gpui `GetMessageW→DispatchMessageW` БЕЗ
  `TranslateMessage` (gpui транслирует только свои клавиши в хендлере).
  Фокус в Chromium-hwnd был (лог MoveFocus), WM_KEYDOWN доходил, WM_CHAR
  не генерился. Патч platform.rs: TranslateMessage только для сообщений
  child-окон (`GetParent(msg.hwnd).is_ok()`) — toplevel-окна gpui не
  трогаем (иначе двойной WM_CHAR в наших инпутах).
- **Курсор без промига**: SetCursor из mouse-move гонялся с WM_SETCURSOR
  gpui. Решение: HCURSOR из CursorChanged → маппинг на gpui::CursorStyle
  (сравнение с LoadCursorW(IDC_*)) → `.cursor(style)` на элементе +
  `window.refresh()` при смене.
- **Дефолтные попапы «хрен пойми куда»** (статус ссылок, title-тултип):
  composition-вебвью позиционирует их относительно РОДИТЕЛЯ контроллера.
  Статус-бар выключен (`IsStatusBarEnabled=false`); для остальных —
  невидимый якорный child-HWND (класс KaminVwvAnchor, пустой GDI-регион:
  не рисуется, мышь насквозь), контроллер создан на нём, sync_zone двигает
  его SetWindowPos + NotifyParentWindowPositionChanged.
- **Тулбар**: visual_frame в browser_pane.rs (те же стили) — back/forward/
  reload/Navigate через wv_visual; URL-синк через add_SourceChanged →
  take_url_change() в render. Клик по навбару → focus_host() (SetFocus на
  main hwnd), иначе клавиатура остаётся в вебвью и gpui-инпут молчит.

## Двоение при ресайзе — GPU-синхронизация (2026-07-26, проверить)

Скрин юзера: вебвью УЕХАЛ РАНЬШЕ панели → dcomp-коммит применён на
компоузе, где флип буфера ещё не произошёл (GPU не дорендерил кадр — DWM
берёт старый буфер, коммиты же применяются всегда). Фиксы в vendored:
1. `commit()`: `WaitForCommitCompletion` ПЕРЕД `Commit` — глубина очереди
   коммитов ≤1 (в steady state ожидание пустое).
2. `present()` при `underlay_active`: после Present — D3D11_QUERY_EVENT +
   Flush + спин `GetData` до готовности GPU (кап 100k итераций), затем
   commit. Буфер гарантированно готов к моменту коммита → флип и оффсет
   на одном компоузе.

## Остаток
- визуал ПОД gpui (AddVisual true→false) + «дыра» в фоне панели
- Tab/хоткеи (MoveFocusRequested/AcceleratorKeyPressed), IsVisible на
  minimize, teardown
- скругление углов вебвью: IDCompositionRectangleClip (SetCornerRadius) —
  замена GDI-региону, без лесенок
- мост kamin:// + postMessage; перевод Bridge-вью; снос overlay-окна

## Вебвью ПОД gpui — РАБОТАЕТ (2026-07-26, день дебага «слоя-невидимки»)

Гугл виден в панели через «дыру», вебвью под всем интерфейсом gpui —
поповеры main-окна теперь могут рисоваться поверх вебвью (путь к сносу
overlay-окна открыт).

Два убийцы, найденные по дороге:
1. **SetDefaultBackgroundColor (непрозрачный кастомный) в composition-
   режиме ГЛУШИТ рендер контента** — WebView2 рисует только заливку.
   Убран навсегда; фон догоняющего relayout даёт backdrop-визуал.
2. **gpui-component Root красит ВСЁ окно `theme.background`**
   (root.rs:413, theme_sync ставит его = editor_bg) — слой ПОД нашим
   корневым div, дыры наших фонов его не касались. Фикс: в visual-режиме
   `root.transparent = true` (main.rs), фон окна рисует корневой canvas
   RootView (с дырой).

Методика, которая раскрыла обоих (спасибо юзеру): КРАСИТЬ КАЖДОГО
кандидата в свой яркий цвет одним билдом (гейт KAMIN_VWV_PAINTDBG=1):
красный=корневой canvas, синий=glint web-карточки, зелёный=backdrop,
оранжевый=подложка дин-вебвью, жёлтый=рамка редактора, маджента=sticky.
Зона осталась 1d1c25 → grep 1d1c25/editor_bg по ВСЕМУ дереву → theme_sync
→ Root. Диагностические env: KAMIN_VWV_ABOVE / RECTCLIP / NO_BACKDROP /
GREEN / SHIFT / MEGAHOLE / PAINTDBG (все выключены по умолчанию; снести
при финальной чистке вместе с логами).

Осталось в шаге: скруглённый IDCompositionRectangleClip проверен юзером?
(запущен), угловые маски/GDI для composition-вью не нужны; затем мост
kamin://, перевод Bridge-вью, снос overlay.

## Контент-ресайз (идея юзера) — ПРИНЯТ, ДЕФОЛТ (2026-07-26)

«Менять при ресайзе не размер окна, а размер контента»: во время драга
поверхность WebView2 не пере-аллоцируется — вьюпорт страницы меняется через
CDP `Emulation.setDeviceMetricsOverride` (перевёрстка в стабильную
поверхность, кадры не рвутся); при росте поверхность увеличивается скачками
с запасом +256 (один realloc на серию кадров; layout при активном override
от размера поверхности не зависит); когда размер замер (>=120мс + доводчик
150мс после отпускания сплиттера) — один точный SetBounds.
`KAMIN_VWV_CONTENT_RESIZE=0` — откат.

Скругление углов вебвью: маски-четвертькруги ПОЛИЛИНИЕЙ (12 сегментов) в
glint-канвасе. ГОЧА gpui: `Path::curve_to` с ctrl-точкой в вершине угла
вырождает ЗАЛИВКУ в треугольник (веерная триангуляция от старта + loop-blinn
сегмент совпадают) — дуга «прямая». Полилиния + MSAA — гладко.

## СЛЕДУЮЩИЙ ЭТАП: мульти-вью + мост + перевод Bridge + снос overlay
1. wv_visual → мульти-инстанс (Host-мапа по id вью; свой визуал/anchor/зона
   на каждое Bridge-вью: chat, console, plan, dyn-tools).
2. Мост: AddScriptToExecuteOnDocumentCreated (THEME_BLOCK) +
   WebMessageReceived (замена wry ipc), URL — те же localhost:3456.
3. Перевод вью по одному под гейтом; wry-путь остаётся фолбеком.
4. Снос overlay-окна: поповеры/тултипы обратно в main (рисуются НАД
   вебвью — вебвью в подвале), удалить NCHITTEST/sync/recreate.

## Перевод Bridge-вью на composition — СТАТУС (2026-07-26, поздно)

Работает: все 5 вью LIVE (chat/plan/console/czShared/browser), мост
двусторонний ([vwv:res] отдаёт HTML 1.6MB, [vwv:in] качает invoke'и),
SourceChanged только у browser (bridge-вью перетирали URL-строку).
webview_body + webview_body_dyn имеют visual-ветки (static_view_id =
кэш-leak динамических id для probe/fwd).

ОТКРЫТЫЙ БАГ: чат-зона рендерится БЕЛОЙ и шире карточки (скрин probe).
JS вью живёт (inbound идёт). Кандидаты: (1) surface/клип рассинхрон
(рост +256 запასом при первом sync? клип должен резать); (2)
setDeviceMetricsOverride на bridge-вью — поля вокруг override-вьюпорта
белые; (3) вью-визуал перекрывает карточку → зона считана шире. Дебаг:
KAMIN_VWV_CONTENT_RESIZE=0 (исключить override), лог sync_zone_view
размеров чата, скрин угла. Затем: dyn-тулы (не-KNOWN) в visual-режиме не
создаются вовсе (webviews.get→None→placeholder) — добавить init_view по
resolve; фокус-возврат хосту при клике вне вью; снос overlay.

## ОТКРЫТО (2026-07-26, вечер): plan/console composition-вью не präsентуют

Факты: DOM живой (задачи «молоко» в plan-DOM, консольный xterm с канвасами),
first sync_zone у всех прошёл (SetBounds верные), дыры фонов открыты — но
ВИЗУАЛЬНО зоны пусты (виден только backdrop). Chat и browser рисуются.
fixed-red-инъекция в plan не видна в его зоне; полноэкранный скан нашёл
одиночный красный спот на (2180,260) экрана — либо UI-артефакт, либо
вью рисуется в чужом месте (подозрение: ДВА рендера с одним viewId зовут
sync_zone_view с разными bounds — offset скачет; проверить логом смены
offset >5px). SetIsVisible(false)-старт откатен (вью всегда visible,
скрытие только клипом) — не помогло plan/console.

Гипотезы к проверке: (1) дубль-рендер slot-тулов с одним id (activity
двух слотов?); (2) präsентация N-ного composition-вью на общем env
(chat=1й рисуется, browser=5й рисуется — против); (3) override
setDeviceMetricsOverride на не-загруженной странице.

Также сегодня: тосты (нулевой anchor → сплющивание + мёртвые клики)
починены; каретка над bridge-вью; титлбар-драг с порогом 4px (клик в
maximized больше не restore-ит); dev-токен обновлён из прод-конфига
(«Invalid token» был корнем «Reconnecting to bridge» ВО ВСЕХ режимах).

## РЕШЕНО: пустые plan/console — параллельный init composition-вью

Корень: 4 init_view в ОДНОМ кадре — CreateCoreWebView2Environment/
CreateCompositionController ПАМПЯТ message-loop (та же гоча, что у wry
create_controller) → вложенные пампы create-цепочек, у «серединных» вью
(2..4-й) präsентация кадров умирает НАВСЕГДА при живом DOM/вводе (red-тест
невидим). Rebind SetRootVisualTarget НЕ лечит; user_data-изоляция НЕ лечит;
SetIsVisible-игры НЕ лечат. ФИКС: сериализация — один init_view за кадр,
следующий только когда предыдущий LIVE (all_started_live-гейт + WvLive-пинок
рендера из comp-callback). Проверено: консоль и план появились.

## ОТКРЫТ: обрезка composition-вью ИМЕННО в right-bottom слоте

Числа согласованы (wvstat: surface 445x527 = физ. зона, override 356x422 css,
anchor по зоне) — но в right-bottom рисуется только верх (~2 строки плана),
низ = backdrop. ПРИ ПЕРЕНОСЕ ТУЛА В ДРУГУЮ ПАНЕЛЬ РИСУЕТСЯ ПОЛНОСТЬЮ (факт
юзера) — бага геометрии/перекрытия конкретного слота, не контента и не
размеров. Чат с surface 1211 физ рисуется весь — не размер. Кандидаты:
перекрытие куском чужого непрозрачного слоя ровно в right-bottom (проверить
PAINTDBG-заливкой область под зоной); клэмп рендера по видимой части
якоря-child (проверить: не позиционировать якорь / без якоря); сравнить
wvstat в «хорошей» панели vs right-bottom (probe emit pinTool).
Доводчик settle_pending добавлен (пинок кадра 170мс) — общий ресайз доезжает.

## РЕШЕНО: обрезка right-bottom = разлив backdrop-а соседнего вью

dcomp-клип применяется ДО transform: backdrop (16×16 × scale 65536) не
резался клипом → editor_bg-плита от offset консоли заливала вправо-вниз и
в дыре плана закрывала его низ (высота закрытия = высота консоли — заметил
юзер). Фикс: SetTransform2 точный (w/16, h/16) в sync_zone_view.
Также: якорь 0×0 в точке зоны (полный размер не нужен).

## ИТОГ ДНЯ (2026-07-26): ЦЕЛЬ E2E ДОСТИГНУТА, ПОДТВЕРЖДЕНА ЮЗЕРОМ

Полностью рабочий Bridge на visual hosting: сообщение через композер чата →
видно в чате И в консоли (живой CLI), план покупок через TaskCreate → виден
в Plan-панели. Все вью composition, под gpui, с дырами/масками/вводом.

Финальные фиксы: hide_unzoned (неактивные табы слота прячутся — клип 0 +
нулевой transform бэкдропа); дисковый кэш HTML вью (webview-html/{id}.html:
UI рисуется ~1с вместо ожидания 10-15с активации extension; свежий HTML
перегружает только при отличии); тосты (лишний .relative() после .absolute()
перезаписывал позиционирование — gpui: последний вызов выигрывает).

ОСТАЛОСЬ по плану: снос overlay-окна (поповеры в main поверх подвала);
дин-тулы не из KNOWN (init по resolve); ускорение активации extension
(BridgeHost/MCP — код kamin-ide); чистка диагностических env/логов
(KAMIN_VWV_* ABOVE/RECTCLIP/GREEN/SHIFT/MEGAHOLE/PAINTDBG, [vwv:*] логи).
