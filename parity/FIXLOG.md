# FIXLOG — бэклог расхождений и статусы фиксов

## Titlebar (из ours-агента, цикл 1)
- [ ] 02 leftCluster: ширина = сайдбару (табы стартуют от кромки сайдбара)
- [ ] 04/18 tabs-slot: pl 48→12, flex:1; чипы flex 0 1 180 (min44/max240);
      оверфлоу = скролл (сейчас кнопка «N ⌄» — структурная замена)
- [x] 18 «+»: 26×26 внутри стрипа; hover accent-mix 36% (scale 1.06 нет в gpui-hover)
- [x] 10 layout-триггер: accent 16% при открытом поповере
- [x] 08 customize-gear: ToggleCustomize + active
- [x] 11/15 поповеры: colList max-h 320 + скролл (100vh−16 меню — TODO, нет vh в фрейме)
- [x] 13 theme-триггер: text_muted (не secondary)
- [x] 14 sysToggle: off=transparent+muted; hover 10%+primary; sysOn hover не перебивает
- [ ] 17 PanelIcon: currentColor; stroke 1.2 инсет 1; вариант bottom
- [ ] 19 чип: light-альфы, dndDragging 0.4, switching-пульс, close 18×18/14%
- [x] 16 item hover 10%, иконка text_primary
- [x] 07 глиф window-контролов 13px (сейчас 14)
- [ ] 01 высота 42.4 (жив вопрос: агент подтвердил на живом — перепроверить
      после остальных фиксов; вероятно dpi-артефакт, см. 01/verdict)

## Sidebar (цикл 1)
- [ ] 22 резолвер тела: sidebar_activity + ActivityPlaceholder fallback
- [ ] 31 rename-инпут: bg-base + рамка accent + editing bg-surface; blur=commit
- [ ] 36/37 customize-nav: fs 13, паддинг 8×12, child-инсет 30, hover-рецепт
- [ ] 26 empty «No sessions yet.» для пустых проектов
- [ ] 37 contributed-узел: title/icon из container; active; клик→views[0]
- [ ] 28 hover-вариации (tinted 30/17%, inactive opacity 1); dblclick/F2 rename
- [ ] 29 пульс working-точки
- [ ] 20/21 min-width 200; clampGrowth; customize-nav при скрытом сайдбаре
- [ ] 25/32 пилюли: text-secondary; цветные hover (add/rename accent, disconnect blue); 14px; clamp
- [ ] 33/34 меню: min-width 200 (не фикс 208), shadow 0 6 24@30%; свотч scale-hover; light-цвета
Чистые: 27; почти: 23, 24 (pr15, letter-spacing, тултипы)

## ActivityBar (цикл 1)
- [ ] 47 drag-ghost: 28×28 иконка-призрак по центру (accent-рамка, тень, .92);
      lookup_any для contributed
- [ ] 41 drop-placeholder вертикальных баров: dashed 32×32 + позиционная вставка
- [ ] 50 drop-placeholder стрипа: dashed 36×24 вставка (не border_l)
- [ ] 39 сайдбарные плитки: drag + RMB-меню + tileDragging 0.3
- [ ] 51 ToolIcon: единый компонент, img-ветка (data:/URL), размер 18
- [ ] Phosphor-svg hover-перекраска (все места)
- [ ] 45/46 меню: shadow; сабменю якорем к строке; toggle/закрытие; иконка Sidebar
- [ ] 44 пикер: 8/12, иконки 18, text-primary, галка наследуемая, uppercase-титул,
      max-height+scroll, flip
- [ ] 48/49: gap 4, overflow-x scroll, 11/500/.02em, hover bg-surface-mix, px10
- [ ] 38: gap 8 секций, скрытие с сайдбаром, contributed в сайдбар-бар, «…» 18px

## FileTree (цикл 1)
- [ ] 102-107 НЕ РЕАЛИЗОВАНЫ: generic Tree + contributed TreeDataProvider-рендер
- [ ] 101 каскад Open In: закрытие по ховеру других пунктов + grace 250мс
- [ ] Locate: флеш .flash 0.9s + скролл по реальной высоте строки (22)
- [ ] 94/95 строка: фикс-высота 22, border 1px transparent резерв, hover
      text-secondary→primary, chevron 13, selected-градиент не перекрывать
- [ ] 92 RMB по пустоте=меню корня; клавиатура; Shift-select; drag папок; dropTarget
- [ ] 96 children: «(empty)», Loading на всех уровнях, кап 100+шаг 200, «(M hidden)»
- [ ] 98 header: спиннер indexing, collapse/expand-тумблер, disabled, letter-spacing
- [ ] 100 меню: измерение вместо est_h, max-height+scroll, min-w 180,
      danger #e5484d, tab-actions, вращение спиннера
- [ ] 99 light-фильтр иконок; isRoot-карты; 93 empty вторая подсказка

## Panels 52-71 (цикл 1)
- [ ] 70/71 НЕ РЕАЛИЗОВАНЫ: шиммер-скелет вебвью + «This panel didn't load»+Retry
      (сейчас вечный Loading… и молчаливый авторетрай 5с)
- [x] 69 ActivityPlaceholder реализован (36 icon + Nothing to show; заменил «Coming soon»)
- [ ] 53/54/58/60/63/65 drop-индикация карт: over=accent 10%+dashed;
      blocked=red 12%+inset shadow
- [x] 55/59/64 горизонтальные ручки 10px
- [x] 56/59-61/64/65 гейты rightPanelBottomVisible/filePanelBottomVisible (низ+сплиттер не рендерятся)
- [x] right_split не персистится — КАК В ОРИГИНАЛЕ (layout-autosave.ts не имеет
      effect для rightPanelSplit; находка агента ложная, не менять)
- [ ] 64/65 file-panel bottom: фикс-px (min 100, no-shrink) вместо ratio
- [ ] 52/56/61 fill-режимы (flex:1 при скрытом центре); shrink:1+min-width колонок
- [ ] 57 handle: гейт по file-панели, clampGrowth(MAIN_MIN=100), hit-зона −8..0
- [x] 67 browser-pane 1:1 (навбар 4/6, addr h26 bg-base focus-accent, nav hover surface-hover, инсет 6); 68 глиф 28×24 + hint 360
- [x] 58 label «Right»

## Panels 72-91 (цикл 1)
- [ ] 83 legacy-bridge-card ОТСУТСТВУЕТ (детект Electron Bridge + uninstall-флоу)
- [x] 81/87 warning-иконки: accent_yellow #f9e2af
- [ ] 86 problems: хедер счётчики-фильтры err/warn, collapse по файлу,
      TreeIcon+dirname, капы 100/200 + «Show more»
- [x] 89 terminal-toolbar: editor-bg актив + вогнутые уголки (canvas-дуга),
      h 30, close hover/active, addBtn 28 круг, scrollBtn 22×30, меню 1:1
- [x] 88/91 терминал: editor-bg тело + инсеты 8/22/10/14, fg/cursor editor-*, fs 13 (cell 7.8×17)
- [ ] 72 chat-switch-skeleton: брендовая шторка (лого+glow+sweep)
- [x] 80 logs: Copy+Clear 26×26, фильтр flex:1 bg-base, body bg-base+wrap, item-рецепт 1:1, empty-state (auto-scroll TODO: нужен ScrollHandle-стейт)
- [x] 85 extensions 1:1: хедер EXTENSIONS+Install (accent 14/26+border), группы «— N», Enable/Disable-кнопка, uninstall (новый ShellEvent::UninstallExtension), dim 0.55 (data-URL иконки TODO — фаза расширений)
- [x] 77 welcome: primary #fff + hover-mix 86%·black, secondary hover 12%; welcome ТОЛЬКО в main-карте (панели живут) (translateY/drop-shadow — нет в gpui, deviation)
- [ ] 79 design-shell: карточки .sectionBody (border+mantle+r12+p16), титулы 16/12
- [ ] сквозное 78/80/81/84/90: title 20→22, меню py 8, box-shadow дропдаунов, hover

## Editor+Status+Overlays 108-129 (цикл 1)
- [ ] 115 webview-tab-icon НЕТ (webview не открываются табами редактора)
- [x] 129 tooltip: current_palette() (новый kamin_theme::set_current_kind); над элементом + clamp
- [ ] 120 version-update: downloading «Updating N%», клик idle-бренда=check,
      пилюля скрывает бренд
- [ ] 114 webview-panel: watchdog 20s, crash-ping, retry-карточка, fade-cover
- [~] 122 prompt-modal: размеры 360/520 + Enter-сабмит СДЕЛАНЫ; live-валидация TODO (нужен validate-коллбек)
- [~] 124/126 pt 12vh реального вьюпорта СДЕЛАН; стрелочная навигация + debounce — TODO (нужен selected-стейт в RootView)
- [x] 126 reveal: SymbolHit.line из range.startLine → OpenFileAt (клик и Enter)
- [ ] 123 quick-pick: detail-рендер, prompt-строка, «OK (N)»+Cancel
- [x] 112 overflow-меню: dirty orange, Esc в CloseOverlay (active в меню недостижим: активный таб всегда видим; клик-вне был)
- [x] 128 toasts: bg 50% (blur нет в gpui — deviation), shadow-card-popup, max-w 360; 121 shadow-modal + confirm hover action-hover/maroon (slide-анимации TODO)

## Misc 130-159 (цикл 1)
- [x] 131 design-шкала: m::FS_LG/FS_XL (16/22)
- [ ] 156 focus-visible ring отсутствует во всём shell
- [ ] 157 drop-highlight карточек (over/blocked) не реализован ← дубль Panels
- [x] 134 словарь ui/shadows.rs (9 токенов 1:1); применены modal/dropdown/card-popup/mini
- [ ] 136-153 (15 сэмпл-блоков Design-страницы) не реализованы
- [ ] 158 cursor:grabbing при tool-drag; гашение hover/tooltip во время drag
- [ ] 140 Chip: r4, pad 8×1, accent-green 14%+border; kbd/codeInline/badge; без dot
- [ ] 130 палитра-витрина: 26 свотчей в 4 группах (сейчас 12 плоских)
- [x] 154 скроллбар: ScrollbarShow::Always, thumb bg-overlay/hover text-disabled (global.css 1:1)
- [ ] 135 кнопки: Danger+Ghost, hover у Primary/Secondary;
      155 light glint-mid: сверить bg-mantle по light-theme.css

## Вне аудита (запросы юзера)
- [x] Закреплённые чипы сессий — влево, отдельной группой (stable-sort в ordered_chips)

# ── ЦИКЛ 1: вердикты ревьюверов (2026-07-26) ──
Детали — в parity/NN-*/verdict.md (единственный источник для волны фиксов №2).
Счёт: Titlebar 5M/14D · Panels72-91 3M/17D · Overlays108-129 4M/18D ·
Misc130-159 3M/27D · Panels52-71 — ревьювер ещё в работе.
MATCH: 01,03,06,07,08 · 84,85,89 · 117,121,128,129 · 154,155,159.

## Волна 2 — бэклог из вердиктов (сводно)
Titlebar (34 пункта в вердиктах): action_button secondary-цвет; svg тумблера
currentColor; gear/codicon 12; searchHint pad; newTab muted+ml6+opaque-hover-mix;
leftCluster=ширина сайдбара; спейсер min-w24; layout/appearance-поповеры позиция
anchor+6/gap1/max-h/1fr-колонки/shadow-dropdown; itemHint disabled; fa-check;
PanelIcon инсет-рамка 1.2 + вариант bottom; чипы: border-резерв, close 18/10/14%,
pin r4, sleeping muted, switching-точка, dnd .4, light-альфы, dropBar-полоса;
theme-глиф contributed=sun/moon (root.rs:5259).
72-91: problems хедер+counts+collapse+TreeIcon+пилюля+showMore+капы+row-метрики;
system-log levels+time+grid+mono; customize header/body/sectionTitle/row 4-0;
терминал empty-state с картой+тулбаром, scrollback 5000, cursorBlink, exit-строка;
welcome divider-soft 6%/glow 220@26%/text-center/lh1.3; term-menu shadow-dropdown+
menuEmpty; design .sectionBody+титулы 16/12 (витрины токенов переписаны в wave2 —
перепроверка в цикле 2); 72/83 не реализованы.
108-129: file-viewer .viewer/.body-рамка; empty-state 109; strip pb4+dropIndicator;
tab pl10/pr6+dirty+close-гейт 0-.7-1+порядок; overflow-меню shadow/min-max/item-
рецепт/chevron-кнопка; status-bar stretch+порядок+asc; 118 r-xs; 119 гейт/тултип;
120 downloading+взаимоискл+кликабельный бренд; 122 input bg-base+focus/invalid+
error+disabled-OK+«OK»; 123 скрим+shadow+prompt/empty/detail/Cancel/OK(N)+item-
рецепт; 124/125/126 shadows::dropdown()+50vh+стрелки+text-right; 126 SymbolKind-
карта; 127 60vh+«{query}»; 129 max-w кламп+ellipsis; 113 error-вью+fs13; 114
loader/Retry/watchdogs; 115 webview-табы.
Misc: сделано в wave2 — light-тени (shadows theme-aware), design-витрины
(130/131/132/133/134 переписаны 1:1) — перепроверка в цикле 2; остаются 135-153/
156/157/158 (сэмплы, focus-ring, drop-highlight, grabbing) + адопция shadows::
в 9 файлах с инлайн BoxShadow.

## Цикл 1 — зона Panels 52-71 (3M/17D; вердикты в папках)
Применено сразу по ревью: v_handle left(-8)→left(-4) (ревьювер опроверг мой
wave1-фикс — для нашей эмуляции зазора верен -4); hint плейсхолдера lh 1.3,
max_w(360) удалён (в CSS его нет).
Бэклог: body pr(4); гэп activity/sidebar 8; ширина right-колонки = persisted−rail;
shrink1+min-w100 колонок; file-низ в px (min100 no-shrink); min_h(100) убрать
у карт; pr ручки 48 (ACTIVITY_BAR_WIDTH 44 vs токен 48 — сверить зону activity);
Open Tool в правых картах; drop-индикация всех карт; BottomTabBar всегда;
noSessions-гейт колонок; fill-режимы; codicon 16 в mode-tabs/navBtn; рамка
PanelIcon rect-инсет+stroke2.4; rail gap8/codicon18; 70 скелет+шиммер+waitNote;
71 errWrap+Retry.

## Волна 2 — сделано (компилится, 38 тестов, app перезапущен)
Titlebar: action_button text-secondary; svg тумблера currentColor; gear 12;
searchHint px8; newTab muted+mx6+opaque hover-mix; спейсер min-w24; theme-глиф
contributed=sun/moon. Чипы: border-резерв 1px, first ml6, close 18/глиф10/hover14,
pin r-xs, sleeping muted, switching-точка with_animation (пульс 1s), light-альфы
(tinted 26/16, active 42/26+60), dropBar-полоса 2×22+glow. Поповеры: layout top40/
right84/gap1/min-w; appearance top39/right250/без pb/колонки flex-1+gap4/gap1;
itemHint disabled; пресет-codicon 12; fa-check; dropdown_shadow=словарь; «+»-меню
1:1 (иконки 14, item 6/8, labels оригинала, hover10, shadow, min-w200).
Оверлеи: shadows::dropdown в quick_open/fif/ws/терм-меню; fif 10vh/76vh без
клампов; ws SymbolKind-карта 1:1 + иконка без цвета. Welcome: divider-soft 6%,
glow 220@26%, text-center, lh1.3.
НОВОЕ в бэклог: буты-табы редактора при старте достаются ТЕКУЩЕЙ активной сессии
(персист пока глобальный — нужен пер-сессионный персист openFiles);
editor_tabs (111/112) рецепт close/dirty/paddings — цикл 3.

## Волна 3 (частично) — сделано
editor_tabs 111: pl10/pr6, порядок pin(codicon-pinned 11 op.7)→icon→label,
dirty И close оба, close-гейт 0→.7(group-hover/active)→1+bg-overlay60% r-xs;
110 py4 симметрично; 112 меню shadow 0/6/24/30% + min200/max360 + item 5/8
gap6 secondary hover surface-hover. status_bar 116: stretch (без items_center),
правые contributed ASC, порядок contributed→encoding→update→brand; 118 r-xs без
py; 119 tooltip «Encoding»; 120 hover accent34% + тултип «Update to KaminIDE
{v} — you have {cur}». Остаток цикла 3 — по списку выше (108/109/113/114/115/
120-downloading/122/123/127, problems/system-log, 70/71, 52-71 геометрия,
136-153, LegacyBridgeCard, chat-skeleton, adoption shadows в overlay.rs/
command_palette/sessions_list/sticky_scroll/tooltip).

## Волна 3 (продолжение): problems 86/87 переписаны 1:1
Хедер PROBLEMS + countBtn-фильтры (active accent18+border40, disabled .8,
иконки красятся при count>0), collapse по файлу (chevron 13/w16), TreeIcon 16,
fileName primary (не bold), fileDir flex1+tooltip, пилюля min-w16 h16 r9,
строки min-h22 pl26 gap6 sev14 hover surface60+primary, hint=lightbulb,
origin source(code), [Ln N, Col M] (character парсится), капы 100/200 +
«Show more» + «… N more problems in this file», empty-текст оригинала.
Стейт в RootView: problems_filter/collapsed/file_cap + 3 ShellEvent.
Ревью-цикл №2 запущен по Titlebar и Overlays (фоново).

# ── ЦИКЛ 2: итоги (Titlebar 14/19 M, Overlays 6/22 M; вердикты дописаны в папки)
Волна 4 по остаткам ц.2 — СДЕЛАНО: 04 слот pr12; 13 theme-триггер muted
(action_button muted-флаг); 19 dndDragging .4 (chip_drag.src → chip) + mr1;
11 max-h vh−16 + скролл всего меню + right 75; 124/126 max-h min(50vh,480) +
path text-right; 126 items_baseline. Проблемы 86/87 1:1 (см. волну 3).
ОСТАЛОСЬ (вердикты ц.2): 17 PanelIcon rect-инсет+bottom; 108/109/110-112
file-viewer блоки (60vh меню, chevron-триггер, overflowItemActive, Discard&close,
dropIndicator-полоса, ls .02em); 113/114/115; 118 css-цвета; 120 downloading;
122/123 модалки-инпуты/скрим; 124-127 стрелочная навигация + light-active +
127 60vh/{query}; 129 микро-ellipsis. Плюс зоны без ц.2: 52-71 (геометрия),
72-91 (перепроверка после problems/терминала), Sidebar/ActivityBar/FileTree
(фиксы ещё не начаты), Misc-сэмплы 136-153/156/157/158.

## Волна 4 (продолжение): Sidebar-старт
- [x] 29 пульс working-точки (with_animation sin 1s, как switching-чип)
- [x] 33/34 session-меню min-w 200 (не фикс 208; кламп по 200)
- [x] 26 «No sessions yet." у проекта без открытых сессий (pl26 fs-xs muted)
Остальной Sidebar (31 rename-инпут, 36/37 customize-nav, 28 hover-вариации,
20/21 min-width/clampGrowth, 25/32 пилюли) + ActivityBar/FileTree — след. волны.

## Волна 5: геометрия 52-71 (по вердиктам ц.1)
- [x] ACTIVITY_BAR_WIDTH 44→48 (= --layout-activity-bar-width; тест обновлён)
- [x] Правая колонка width = persisted (БЕЗ +rail)
- [x] body pr(4) — правый гуттер (rail не вплотную к краю)
- [x] card↔rail зазор 0 (gap_wrap pl-only)
- [x] rail: .bar gap 8; codicon плиток 18; «…» 18
- [x] min_h(100) снят с трёх карт (main/file-top/right-top)
- [x] Sidebar: пульс working-точки; меню min-w200; «No sessions yet.»
Осталось 52-71: shrink1+min-w100 колонок, file-низ px-персист, fill-режимы,
noSessions-гейт, drop-индикация, BottomTabBar-всегда, 70/71 скелет/Retry,
66/67 codicon 16, PanelIcon rect-инсет.

# ── Скрин-верификация (после разноса от юзера) — ПРОЦЕСС ИЗМЕНЁН
Каждая пачка правок теперь проверяется скринами main+overlay ДО отчёта.
Пойманы и исправлены регрессы моих же волн: Scrollable растягивал layout-
поповер на всю высоту (убран, max-h остался); flex_1 колонок appearance
резал имена тем (убран — интринсик); gpui svg() без text_color рисует
ПУСТО (иконка сайдбара; цвет возвращён, hover-lift svg = ограничение gpui);
«No sessions yet.» спамился при inactive-only (сужен до пустых проектов);
палочки PanelIcon вылезали за рамку (кламп в inner 2..12×2..10).
По фидбеку юзера: «New session» теперь открывает пикер папки (раньше =
No folder session); disconnect-глиф чипа 16 (эффективный размер прода);
чипы flex 0 1 180 / min 44 / max 240 со сжатием и ellipsis (окно
оверфлоу-кнопки считается по min 44). Скрины: _layout-pop-ovl.png (чисто),
_chips-check.png (чисто).
НОВЫЙ БАГ в бэклог: live-тултип в overlay рисуется НЕ у курсора (внизу по
центру при probe-вводе) — проверить координаты tooltip_live (mx,my) при
SendInput; вероятно, экранные против клиентских или лаг hover-состояния.
Побочка моих слепых кликов: создана лишняя «Session 8», file-панель
переключена на Web — юзеру решать, убирать ли сессию.

## Тултип-баг закрыт (скрин-верификация нашла)
TooltipShow теперь несёт (x,y) = window.mouse_position() В МОМЕНТ показа
(last_mouse запаздывал при SendInput-телепорте курсора → тултип улетал).
Чипы: flex 0 1 180/min 44/max 240 + ellipsis; disconnect 16 — подтверждено
скрином _chips-check.png.

# ── Стабилизация сплиттеров (запрос юзера: «дёргается», полупиксели)
Диагноз: во время драга размеры гонялись через ratio и округлялись НЕЗАВИСИМО;
при dpi 1.25 шаг мыши = 0.8 логического px → round() дробного результата
прыгал туда-сюда; в паре file↔right сумма не сохранялась → центральная
колонка (flex-остаток) дёргалась от чужого драга.
Best practice (Zed/VS Code): единственный источник — ЦЕЛЫЕ логические px,
дельта указателя квантуется, гибкая середина = остаток, ratio только для
персиста.
Сделано в drag_move/end_drag (root.rs):
- все дельты .round(); init тоже округляется;
- MainFile: ratio из целых px (round-trip width_from_ratio → тот же px);
- FileRight: СУММА ПАРЫ инвариант (nr = total − nf), и nf прогоняется через
  ту же клампованную конверсию (FILE_RATIO_MIN/MAX), что и рендер — иначе на
  упоре ratio сумма уплывала;
- вертикали (MainBottom/RightSplit/FileBottom): ratio = round(px)/body_h →
  relative() даёт целые высоты, без полупиксельных швов;
- end_drag округляет px-поля перед персистом (дробные из JSON давали ±1px
  скачок на первом кадре).
ИЗМЕРЕНО probe-драгами (не на глаз):
- file|right ×6 (в упор и обратно): дрейф чата 0.00 px, сумма 756.8 неизменна,
  шаг ровно ±40, возврат в исходное 437.6;
- sidebar|main: sidebar 256→288, file_w/right_w/right_x НЕ изменились;
- right-split ×3: сумма 812.00 неизменна, чат не тронут;
- file-bottom ×2: правая колонка не тронута (452.80 = 452.80).
Остаток: file-bottom не строго симметричен (±2 device px на реверсе) — низ
хранится ratio (как в оригинале); при желании перевести на px-персист.

# ── Клик по файлу в дереве (запрос юзера)
reveal_file_panel(): OpenFile/OpenFileAt включают file-панель, если скрыта,
и переключают Web→Files с персистом обоих полей. Работает для дерева, поиска,
Quick Open и symbols (единая точка).

# ── Env-переменные законстанчены (запрос юзера)
KAMIN_VISUAL_WV: дефолт ON (=0 — аварийный wry). ПРИЧИНА БАГА «вебвью
некликабельны»: main.rs требовал ==1 отдельной проверкой → без env окно
поднималось без dcomp, а root шёл по visual-ветке = полурежим. Теперь
единый гейт wv_visual::enabled() в обоих местах.
Диагностические флаги вырезаны в константы (13 мест): KAMIN_VWV_PAINTDBG,
_GREEN, _NO_BACKDROP, _RECTCLIP, _SHIFT, _MEGAHOLE, _ABOVE → false;
KAMIN_VWV_CONTENT_RESIZE → true.
Осталось живыми (по делу): KAMIN_PROBE_PORT/TOKEN (debug-RPC),
KAMIN_DEV_REPO, KAMIN_GPUI_UPDATE_URL, KAMIN_TOKEN_MAP, KAMIN_OVERLAY_ALPHA,
GPUI_DISABLE_DIRECT_COMPOSITION (ставим сами), GPUI_SIZE.

# ── Минимум центральной колонки: 550 → 100 (запрос юзера)
Диагноз по коду ОРИГИНАЛА:
- constants.ts: MAIN_MIN_WIDTH_PX = 100 (это и есть предел сжатия центра);
- centre-column-width.ts clampGrowth(desired, current, MAIN_MIN=100) — растущая
  панель стопорится, когда центру осталось 100;
- CHAT_MIN_WIDTH_PX = 550 живёт ТОЛЬКО в layout-ratios.computeSideArea (опора
  для пропорционального масштабирования при ресайзе окна) и НЕ ограничивает драг;
- живая ширина file-панели = px-сигнал filePanelWidth; ratioFromWidth с клампом
  0.05..0.6 применяется лишь в autosave (персист) → драг не упирается в 0.6.
Наш порт делал наоборот: ratio был ЕДИНСТВЕННЫМ источником (кламп 0.6 душил
драг), а sidebar-драг клампился по vw−550 → чат не сжимался.
Сделано:
- RootView.file_w_live: Option<f32> — живые px file-панели (ratio → только
  персист + пересчёт при смене ширины вьюпорта: last_viewport_w);
- layout_math::max_growth_width(vw, occupied) = vw − (гуттеры 8 + activity 48)
  − occupied − PANEL_MIN_SIZE(100), floor 100 — порт clampGrowth;
- Sidebar/MainFile используют его; FileRight сохраняет сумму пары.
ИЗМЕРЕНО probe-драгами: чат жмётся до 74.4 measured (≈MAIN_MIN 100 логических)
и упирается ровно там; обратный драг стопорится на file min (90.4 measured);
right-колонка при этом не шевелится. Юнит-тесты kamin-metrics: 6 passed
(growth_limit_is_centre_min_not_chat_min + floor).

# ── СКРИНЫ по элементам (пробел «скрин» из goal закрыт)
parity/_original-full.png — прод KaminIDE (Playwright/CDP 9222, 2560×1380);
parity/_ours-full.png — наш порт (probe screenshot).
Нарезка по зонам → в КАЖДУЮ папку элемента положены original.png + ours.png
(101 элемент; скрипт scratchpad/crop_zones.py, ректы зон логические,
масштабируются на dpr 1.25). Зональные эталоны: parity/_zone-*-{original,ours}.png.
Зоны: titlebar(01-19), sidebar+tree(20-37,92-107), activity(38-51),
main(52-55,72-77), filepanel(61-68,108-113), right(56-60), status(116-120).

## Что скрин-сверка сразу выявила и что исправлено
Эллипсис в чипах сессий: оригинал «reload-skills-com…», у нас текст обрезался
НАСУХО. Корень — gpui `truncate()`/`text_ellipsis()` НЕ рисует «…» для
`.child(SharedString)` внутри flex-элемента: Text кэширует шейп с первого
measure-прохода (MaxContent) и не перерезает по итоговой ширине.
Решение: ui/text_fit.rs — усечение ПО ИЗМЕРЕНИЮ (shape_line, fallback по
средней ширине глифа 0.62·size) + ширина чипа теперь ДЕТЕРМИНИРОВАННАЯ
(считаем сами: (available − pl48 − 4 − резерв36)/n − gap, кламп 44..180)
вместо flex-basis+shrink, иначе бюджет усечения не совпадал с реальным клипом.
Проверено зумом: «Create new d…» (было «Create new dc»).

## Волна 6 — остатки цикла 3 (Sidebar 20-37, ActivityBar 38-51, FileTree 92-107)

Исправлено (сверено с CSS оригинала, не по памяти):

**Рейлы активностей** — `ActivityBar.module.css`: `.bar` gap 8 только МЕЖДУ группами
(gear / list / picker), `.list` внутри = 2. Было единое: 8 на всех детей в
`right_column.rs`, 2 на всех в `activity_bar.rs`. Тайлы завёрнуты во внутренний
список (`list`), рейл собирается как `{list, picker}` / `{picker, list}`.

**Сайдбар** (`SessionsMode/ProjectGroup/SessionItem.module.css`):
- chevron группы — бокс `w 16 + justify_center`, глиф 13 (был голый 12);
- `.list` padding-right 4 (был 15 «под скроллбар» — в оригинале 4);
- «No sessions yet.» padding 2px 0 2px 18px (был 26/4);
- пилюли действий: база `text-secondary`, ховер-фон всегда `text-primary 12%`,
  цвет по действию (rename/add → accent-primary, disconnect → accent-blue,
  delete → accent-red); красный ФОН 15% только у delete проекта;
- `.tinted:hover` 30/17 (ховера у цветной строки не было вовсе);
- `.inactive:hover` → opacity 1;
- пульс working-точки: 1.1 s, opacity 0.5↔1, «scale» 1→1.5 через абсолютный
  внутренний кружок (в gpui нет transform — лейаут не должен дёргаться).

**Customize-нав** (`CustomizeMode.module.css`): `.root` padding space-3 0 + gap
space-2, `.header` 8×12 + ss01, `.list` px space-2/gap 2, `.item` fs-md 13 +
padding 8×12 + глиф 14 БЕЗ жёсткого цвета (наследует), ховер `bg-surface 50%`,
`.child` pl = space-3 + 18 = 30 (было 40).

**Дерево файлов** (`FileTreeView/FileTreeHeader.module.css`): строка h 22 +
резерв `border 1px transparent`, ховер красит текст, chevron 13 в боксе 16
(у выделенной наследует цвет), `.chevronSpacer` 16, badge `ml auto + pl 6 +
600`, «Show N more» gap 6 / py 3 / ховер bg-surface 55% + text-primary,
корневая строка h 22 + бокс chevron, `.body` padding-top 4, заголовок ss01,
«Indexing…» со спиннер-глифом 12 и gap 4.

**Стрип нижней панели** (`BottomTabBar.module.css`): `.strip` gap space-1 +
padding 4px по вертикали (было только сверху), `.tab` 11px/500/text-secondary/
px 10/gap 6, ховер `bg-surface 50%`, иконка неактивного таба text-secondary.

**tool_picker** (`ActivityBar.module.css .menu/.menuItem/.menuLabel`):
`min-width 220` вместо фиксированной ширины, gap 1, `max-height 100vh − 16`,
метка «TOOLS» uppercase + padding 4×12, пункты padding 8×12 + база
`text-primary` + ховер `text-primary 10%`, иконки 16 (codicon) / 18 (svg).

### Отмеченные отклонения (не забытые правки)
- **`letter-spacing` в gpui отсутствует** (`TextStyle` не имеет поля) — заголовки
  PROJECTS / CUSTOMIZE / имя дерева выходят ~6 CSS-px уже оригинала на 8 знаках.
  `font-feature-settings: "ss01"` при этом ВКЛЮЧЁН через `Font.features`
  (`ui/typo.rs::ss01`). Вариант «дописать tracking в вендоренный gpui» —
  отдельная задача (правка line_layout: пост-шейповый сдвиг advance).
- **Нет `transform`** → ховер-scale свотчей цвета и scale drag-ghost эмулируются
  геометрией либо отсутствуют; пульс точки эмулирован абсолютным кружком.
- **`svg().text_color()` не наследует hover** → перекраска svg-иконок плиток по
  ховеру недоступна (нужен group_hover-вариант рендера).

### Пере-кроп скринов (дефект моего же досье)
Папки 92-107 держали кроп САЙДБАРА: дерево файлов у обеих сторон живёт в правой
колонке (карта right-top), а не в файловой панели. Оригинал переснят в режиме
Files (`parity/_original-full-files.png`, Playwright по CDP 9222 + клик по табу
«Files»), зоны `tree` (orig 1741,42,307×560 / ours 1058,42,296×470) и `editor`
(orig 990,42,751×560 / ours 596,42,466×470) нарезаны заново — 30 папок
(92-107, 61-68, 108-113). Скрипт: `scratchpad/crop_zones_w6.py`.

### Волна 7 (остатки цикла 3, ещё не сделано)
- сайдбар: пин-ховер (0.7 по ховеру строки, цвет tab-color без фона),
  rename-инпут (bg-base + бордер accent + radius 4 + padding 1/4, коммит по
  blur), dblclick/F2 → rename, светлая палитра `resolveSessionColor`;
- customize: титул и иконка contributed-контейнера из реестра (сейчас
  захардкожены «Claude Bridge» / comment-discussion);
- activity: drag-ghost 28×28 и dashed-плейсхолдеры 32×32 / 36×24, подменю
  «Move to ▸» по dropdown-рецепту, ToolIcon img-ветка (VSIX-иконки);
- дерево: generic TreeView + contributed TreeDataProvider (элементы 102-104),
  checkbox 14×14, контекст-меню min-w 180 + danger #e5484d + позиция по
  измерению, живой grace-close 250 мс, фильтр иконок для светлой темы.

### Волна 6б — два СИСТЕМНЫХ расхождения (замеры, не на глаз)

**1. line-height.** gpui по умолчанию 1.618; браузерный `normal` для Bricolage =
**1.169** (замер оригинала через CDP: `.item` fs 13, padding 8+8 → height
31.2 ⇒ строка 15.2). Из-за этого каждая строка, чья высота идёт от контента,
была ~5px выше оригинала (нав Customize: pitch 39 против 33). Фикс — одна
строка на корне `root.rs`: `.line_height(gpui::relative(1.169))` (наследуется
всем). После: pitch 34.8/33.6 против 32.8/34.4 у оригинала, action-строки
сайдбара 28 против 28.

**2. Вертикальный гаттер `.body`.** У оригинала
`.body { gap: var(--space-2); padding: 0 var(--space-1) }` — вертикального
паддинга НЕТ: карты вплотную к титлбару и статус-бару, шов между стопками
закрывает ручка. Наш `gap_wrap` (и его копия в `right_column.rs`) добавлял
pt/pb 4 → ВСЕ панели были на 4.8px ниже (замер края карты: наш 47.2 против
42.4 оригинала). Фикс: `gap_wrap` стал горизонтальным (`px 4`), флаги
`pad_top/pad_bottom` и функции `gap_wrap_v`/`gap_wrap_v_top` удалены.
После: `right-top` y = 43.2 (probe-область внутри 1px бордера карты) против
42.4 у оригинала.

Осталось из этой темы на волну 7: у оригинала ручка v-сплита абсолютная (шов
между стопками = 0), у нас она занимает место в потоке (шов 10px).

## Досье: добор скринов (147/159) и как снимались состояния

Механика съёмки состояний (обе стороны, одинаковый DPR 1.25 → пиксель-в-пиксель):
- **оригинал** — Playwright по CDP 9222: клики по нав-пунктам, `scrollTop` для
  прокрутки Design-панели, синтетические `KeyboardEvent` (ctrl+p / ctrl+shift+p /
  ctrl+shift+f / ctrl+t — подтверждено в `QuickOpen.tsx:34`, `global-input.ts:72`,
  `FindInFiles.tsx:34`, `WorkspaceSymbols.tsx:30`), `contextmenu`-событие на строке
  сессии;
- **мы** — probe: `emit {kind}` (не `event` — на этом я потерял заход: девять
  `ok:false`), `scroll` (WM_MOUSEWHEEL — `key pagedown` панель НЕ скроллит),
  `screenshot {target:"overlay"}` для меню/модалок, `emit pinTool` для
  включения тула в слот.

Добрано в этот заход: Customize-экраны (78-85), токены и семплы Design
(130-153), оверлеи 124-127 + 121/123/128 + 90(наш), терминал 88/89/91,
Problems 86/87, вебвью-карта 114/115, скроллбар 154, glint-ring 155.

### Остались 12 — и почему
| Элемент | Причина |
|---|---|
| 69 activity-placeholder | нужен слот БЕЗ активного тула на обеих сторонах |
| 70 webview-loading-skeleton | транзиентное состояние загрузки вебвью |
| 71 webview-load-error | нужен искусственный сбой загрузки |
| 90 terminal-shell-menu | наш кадр есть; у оригинала нужно открыть «…» тулбара |
| 122 prompt-modal | у оригинала это НЕ rename (он инлайновый), а «New File…» дерева |
| 123 quick-pick | нужна команда расширения, открывающая QuickPick |
| 128 toasts-stack | триггеры-семплы в Design-панели |
| 129 tooltip | ховер + выдержка задержки на обеих сторонах |
| 156 focus-visible-ring | нужен keyboard-focus |
| 157 activity-drop-highlight | нужен активный drag |
| 158 dragging-body-classes | у оригинала это CSS-классы на `<body>` — визуала нет |
| 159 legacy-app-shell-css | мёртвый legacy-CSS оригинала — визуала нет |

158/159 фиксирую как **N/A по природе** (нечего снимать), остальные 10 — следующая
партия.

## ЦИКЛ 5 — зона токенов/семплов/глобальных стилей (130-159)

Итог: **MATCH 8 / DIVERGES 22.** MATCH: 131 типографика, 132 отступы,
133 радиусы, 134 тени, 154 скроллбар, 155 glint-ring, 158/159 (N/A по природе —
legacy CSS в порт не тащили).

### Главный положительный результат
**Расхождений в HEX-значениях НЕТ НИ ОДНОГО.** Сверены все 34 поля тёмной
палитры и 34 светлой против `dark-theme.css` / `light-theme.css` — совпадают
побайтно, включая `glint_edge` rgba(255,255,255,.18) / rgba(60,40,20,.18) и
`glint_mid` (#262533 = bg-mantle / #e6e1d4 = bg-surface). Числовые токены
(`FS_*` 11/12/13/16/22, `SPACE_1..7` = 4..28, `RADIUS_*` 4/8/12/16) и все
9×2 тени — 1:1.

Не заведены как поля палитры (собираются ad-hoc через `tint()`):
`overlay-modal/soft/deep`, семья `bg-tint-*`, `accent-*-soft/-dark`,
`divider-soft`, все `--tint-*`. Значения от этого не врут, но `overlay-soft .35`
и `overlay-deep .6` недоступны — скрим захардкожен одним α 0.5. Волна 8.

### Что чинить (волна 8, всё в `ui/design_panel.rs` + мелочи)
1. **Секция Components обрывается**: 16 из 18 sample-блоков отсутствуют
   (list item, input, dropdown, tree, in-app toasts, modals, external toasts,
   tooltip, tab strip, icon column, checkbox dropdown, context menu, section
   header, status-bar items, panel icon family, placeholders) — примерно два
   экрана контента. Побочный признак: `_zone-design-s2-ours.png` и `-s3-ours.png`
   получились одинаковыми, потому что прокручивать дальше нечего.
2. **Нет карточки `.sectionBody`** вокруг каждой секции (border 1px
   bg-surface 60% + radius-md + bg-mantle + padding 16) — у оригинала это 6
   обрамлённых карточек, у нас содержимое лежит плоско.
3. **Заголовки секций мельче**: `.sectionTitle` fs-lg 16/600 (у нас FS_MD 13),
   `.sectionSubtitle` fs-sm 12 (у нас FS_XS 11); ритм секций `gap 24 + pb 24`
   (у нас `mb 20`, нижнего паддинга нет); связки title↔subtitle 2 и
   subtitle↔body 12 (у нас 0 и 8).
4. **Нет `Block`-враппера** и подписей блоков (`.compLabel` uppercase fs-xs
   muted, `.compHint`, `.compInline` wrap gap 8) — семплы висят без названий.
5. **Свотч фиксированной ширины 180**: у оригинала grid-ячейка тянется
   (`minmax(180px,1fr)`) → у нас длинное `--accent-action-hover` упирается в
   правый край карточки. Чинить `min_w(180) + flex_grow`.
6. **`.btnGhost` без прозрачного бордера** → Ghost на 2px ниже и уже соседей.
7. **`.chipMuted`** должен быть bg 12% / border 25% (у нас общий 14%/30%);
   тексты семплов не совпадают («chip/muted/danger» вместо «active/idle/error»,
   «Ctrl+K» вместо «Ctrl+Shift+P», «code()» вместо «npm run check»).
8. **Focus-ring не реализован вовсе** (`outline 2px accent-primary, offset 2`).
9. **Drop-подсветка карточки-приёмника** отсутствует: нужны `over`
   (bg accent 10% + dashed 1px accent 60%, offset −2) и `blocked`
   (red 12% + inset 2px red 60%); есть только ghost и полоса вставки.
10. Поведение (не визуал): при драге тула не ставится `cursor: grabbing`,
    ховеры/тултипы во время драга не подавляются.

## ЦИКЛ 5 — итоги трёх зон и волна 8

| Зона | MATCH | DIVERGES |
|---|---|---|
| панели и экраны 52-91 | 14 | 26 |
| оверлеи, статус, модалки 108-129 | 3 | 19 |
| токены, семплы, глобальные стили 130-159 | 8 | 22 |

### Корневая находка цикла: overlay-слою не был задан line-height
`overlay.rs` ставил корню `.font()` и `.text_color()`, но НЕ `line_height` — и
всё содержимое overlay-окна (палитра, QuickOpen, find-in-files, symbols,
модалки, тосты) считалось с gpui-дефолтом 1.618. Замеры цикла 5: строка
QuickOpen 31.2 против 26.4 у оригинала, диалог модалки 136 против 125, кнопки
30 против 24, карточка тоста 45.6 против 38.4.

**Волна 8 (сделано и проверено замером после сборки):**
1. `overlay.rs` — `.line_height(gpui::relative(1.169))` на корне слоя.
2. Инпут-ряды QuickOpen / find-in-files / workspace-symbols: вместо паддинга
   фиксированная высота ряда 40 (`Input` несёт собственную высоту, паддингом
   в 39-40 не попасть). Замер после правки: ряд **39.2**, шаг строк **26.0**
   против 26.4 у оригинала — совпало.
3. Скрим по типу оверлея: модалка `--overlay-deep` .6, палитра и QuickPick .5,
   списочные QO/FiF/WS `--overlay-soft` .35; **QuickPick добавлен в скрим**
   (раньше фон под ним не темнел вовсе).
4. `.body gap 8` между activity-баром и сайдбаром: бару добавлен `pr 4`
   (половину зазора несёт `pl 4` сайдбара, вторую половину не давал никто).
5. Заголовок Problems — `ss01`.

### Не сделано (волна 9, по убыванию заметности)
- Design-панель: карточка `.sectionBody` (border 1px bg-surface 60% + r12 +
  bg-mantle + p16), заголовки секций 16/600 и 12, ритм 24 + pb 24, `Block`-
  враппер с подписями, 16 из 18 sample-блоков.
- System log: сегменты all/error/warning/info, колонка времени, моно-строки,
  Clear иконкой 28×28, поле фильтра 28px.
- Settings: заголовки секций 11/600/uppercase/muted, строка `gap 10; padding
  4px 0` без фона и ховера, label 13, `line-height 1.5` у описания, нативный
  disabled; карточка Legacy Bridge (элемент 83) не реализована вовсе.
- Customize-хедер: `padding 20 24 12` + нижняя линия, тело `16 24` со скроллом,
  сабтайтл 13 + mt 4 и **пять текстов сабтайтлов из оригинала**.
- Правые карты (58/60): пилюля «Open Tool» в пустом состоянии; у main-карты
  (53) она, наоборот, лишняя.
- Drop-подсветка карточки-приёмника `over`/`blocked` — для всех карт.
- QuickPick: тень, max-h 60vh, обрамлённый инпут, рецепт строк, Cancel, Esc.
- Палитра: `max_h` 60vh вместо 0.75 остатка.
- Скелеты и ошибки вебвью (70/71/72), иконка таба вебвью (115).

### Волна 9 (сделано, проверено кадрами `_w9b-settings` / `_w9c-design`)
- **Customize-оболочка**: `.header` = `padding 20 24 12` + нижняя линия
  (`bg-overlay` 30%), `.body` = `padding 16 24` со скроллом; сабтайтл
  fs-md 13 + `mt space-1`. Раньше был единый `p 20` без линии.
- **Пять текстов сабтайтлов** заменены на оригинальные
  (`CustomizePanel.tsx:73-79`), дословно.
- **Settings**: `.sectionTitle` — 11/600/**uppercase**/text-muted (было
  12/Semibold/text-secondary mixed-case); `.row` — `gap 10; padding 4px 0`
  без фона, радиуса и ховера; label 13; `.rowDesc` — `line-height 1.5`
  без собственной `max-width` (описание перестало переноситься на вторую
  строку); клик по строке до загрузки префов больше не уходит в `SetPref`
  (аналог `disabled` у оригинала).
- **Design-секции**: карточка `.sectionBody` (border 1px `bg-surface` 60% +
  radius-md + `bg-mantle` + padding 16), заголовок fs-lg 16/600, сабтайтл
  fs-sm 12 + lh 1.3, ритм `gap 12` внутри секции, `gap 2` в хедере,
  `mb 24` между секциями.

### Волна 9б
- **Пилюля «Open Tool»** приведена к оригиналу по слотам: правые карты
  (`RightPanel.tsx:148,181` передают `activitySlot`) теперь получают её через
  `panel_placeholder_ex`, а ЦЕНТРАЛЬНАЯ карта — больше нет
  (`MainContent.tsx:55` отдаёт placeholder без `activitySlot`).
  `open_tool_btn` стал `pub(crate)`.
- **Палитра команд**: `max_h` = 60vh (было `(vh − 84 − 48) × 0.75` — панель
  уезжала на ~67px ниже оригинала).
- **QuickPick**: `max_h` панели = 60vh (был жёсткий 420); в функцию добавлен
  параметр `vh`.

Проверка пилюли ОТЛОЖЕНА: в текущем состоянии в правых картах активны тулы,
пустого состояния на экране нет. Проверять — открепив тулы слота (probe
`emit pinTool` наоборот) либо на чистом профиле.

## ЦИКЛ 6 — повторная сверка после волн 8-9

| Зона | MATCH | DIVERGES | Закрыто пунктов ц.5 |
|---|---|---|---|
| панели 52-91 | 15 | 25 | 9 |
| оверлеи 108-129 | 3 | 19 | 6 |

Перешли в MATCH: **79** (Design-панель целиком), **58/60** (пилюля правых карт),
**86** (Problems + ss01). Подтверждено замерами: сегменты System log 20.8 против
21.6 лог., поле фильтра 28.8 против 28, шаг строк QuickOpen 25.6 против 26.4.

### Волна 10 — три правки по находкам ц.6 (две из них — МОИ регрессии)
1. **Глиф Clear в System log был `\u{eb80}` = `codicon-word-wrap`**, а нужен
   `\u{eabf}` = `clear-all`. Ошибка волны 9, видна на кадре.
2. **Регрессия волны 9 в Settings**: сняв `max_w 560` у описания, я не сделал
   текстовую колонку сжимаемой — строка перестала переноситься и обрезалась
   краем панели. Добавлены `flex_1() + min_w(0)`.
3. **Побочка волны 8 в activity-баре**: `w(48).pr(4)` при border-box съел 4px
   из ширины — плитки уехали на 2px влево. Теперь `w(48+4).pr(4)`.
4. Инпут-ряды QuickOpen/FiF/symbols получили `flex_shrink_0` (замер ц.6 показал
   34.4 лог. вместо 40.8 — фиксированная высота сжималась); палитра переведена
   с `py 6` на фиксированные 44 (замер оригинала 43.7).
5. QuickPick: `max_h 60vh` перенесён с СПИСКА на ПАНЕЛЬ + `overflow_hidden` +
   `shadow::modal()`.

### Осталось крупное (волна 11)
- 16 sample-блоков Design + `Block`-враппер с подписями.
- Карточка Legacy Bridge (83) — первый блок Settings у оригинала.
- Drop-подсветка карточки-приёмника `over`/`blocked` (53/54/58/60/65).
- Скелет загрузки вебвью, экран ошибки с Retry, шторка переключения чата
  (70/71/72), иконка таба вебвью (115).
- QuickPick: обрамлённый инпут, рецепт строк, Cancel, «OK (N)», Escape.
- `Input` не наследует размер/паддинг обёртки — задавать на самом компоненте
  (фильтры Logs и System).
- Скрим захардкожен чёрным: в светлой теме оригинал даёт rgba(27,26,22).
- Альфа детей overlay (тултип ≈0.40, тост ≈0.20 против 1.0/0.5) и правый отступ
  тоста 8.8 против 16 — проверить живьём, похоже на сам overlay-слой.

### Волна 11 (начата) — sample-блоки Design
- **`Block`-враппер реализован** (`design_panel.rs::block`): `.compRow`
  (колонка gap 8) + `.compLabel` (fs-xs, uppercase, text-muted). Стек семплов
  переведён с `gap 12` на `.compStack` = `gap space-4` 16.
- Подписи получили существующие блоки: «BUTTONS», «CHIPS · KBD · CODE · BADGE».
- **Новые блоки 1:1**: «LIST ITEM — ACTIVE SELECTION (SIDEBAR PATTERN)»
  (список gap 2 / max-w 280; строка 8×12, r-sm, fs-md, text-secondary, hover
  bg-surface 50% + primary; active accent 14% + accent-primary, hover активной
  22%; disabled opacity .45) и «INPUT» (8×12, border bg-surface, r-sm, bg-base,
  fs-md, max-w 360).
- Проверено кадром `_w11-design-samples.png`: блоки с подписями на месте,
  active-строка тинтована, disabled приглушён.

Осталось из зоны семплов: dropdown, tree, in-app toasts, modals, external
toasts, tooltip, tab strip, icon column, checkbox dropdown, context menu,
section header, status-bar items, panel icon family, placeholders (14 блоков;
часть требует состояния — открытый dropdown, триггеры тостов/модалок).

### Волна 11б — ещё три блока семплов + hint
- `block_hint` — вариант блока с `.compHint` (fs-xs, lh-snug 1.3, text-muted,
  отбивка снизу 4).
- **Section header** (150): padding 8/12, fs-xs/500, text-muted, `ss01`
  (letter-spacing 0.08em — ограничение gpui) + hint оригинала.
- **Status-bar items** (151): рецепт `status_bar.rs` — gap 4, px 8, r-xs,
  fs 11, глиф 12; «3 active» green, «2 failed» yellow, «UTF-8» muted,
  бренд secondary.
- **Panel icon family** (152): все 8 слотов `slot_glyph_small` + подписи
  fs-xs muted + hint оригинала.
Проверено кадром `_chk-w11b.png` — блоки на месте, подписи и хинты видны.

Итого в секции Components реализовано 7 из 18 блоков (buttons, list item,
input, chips, section header, status-bar items, panel icon family).
Осталось 11: dropdown, tree, in-app toasts, modals, external toasts, tooltip,
tab strip, vertical icon column, checkbox dropdown, context menu, placeholders.

### Волна 11в — ещё три блока семплов
- **Horizontal tab strip** (146): `.strip` gap space-1 + padding 4/8,
  `.tab` h24 px10 gap6 r-sm fs11/500 text-secondary, active accent 16% +
  text-primary + хинт оригинала.
- **Vertical icon column** (147): бар 48 + py space-3, `.list` gap 2, плитка
  32×32 r-sm, active accent 16%, «…»-пикер отделён gap space-2 (замер по
  кадру: шаг плиток 34 лог. = 32 + 2 ✓).
- **Tree (file-explorer pattern)** (139): строка h22 gap6 + резерв бордера,
  chevron 13 в боксе 16, иконка 16, выделение = градиент accent 26→14% +
  бордер 45%, ховер bg-surface 55% + text-primary.
Проверено кадром `_chk-w11c.png`.

Реализовано 10 блоков из 18. Осталось 8: dropdown (открытое состояние),
in-app toasts, modals, external toasts, tooltip, checkbox dropdown,
context menu, placeholders.

### Волна 11г — карточка Legacy Electron Bridge (элемент 83)
- Модуль `src-tauri/src/bridge_uninstall.rs` оригинала **портирован 1:1** в
  `crates/shell/src/legacy_bridge.rs` (снята Tauri-обвязка, `tracing` →
  `eprintln`): обнаружение (Squirrel-установка, ключи контекст-меню HKCU,
  конфиг) и удаление (uninstaller + чистка реестра и конфига). Внешних
  крейтов не требует — только `reg`/`cmd`.
- Карточка по `LegacyBridgeCard.module.css`: flex-start gap 12, padding 12,
  bg-surface + divider-soft + r-md; иконка 32×32 r-sm accent-primary глиф 16
  (`fa-box-archive`); титул 13/600; описание 12 с `line-height 1.5`; кнопка
  4/12 с бордером accent-red, текст 12/600 accent-red, ховер — красная
  заливка + белый текст. Рендерится ТОЛЬКО при реальном обнаружении.
- Кнопка ведёт себя как оригинал: сначала danger-подтверждение, и лишь после
  него реимпорт сессий (`kamin:bridge:reimportSessions`) → uninstall,
  оба в отдельном потоке (блокирующий `cmd /C` иначе вешает окно).
- **Проверено кадром `_chk-w11d.png` на живой машине**: карточка появилась и
  честно перечислила найденное — «folder “Open with” menu entry, saved config».
  Кнопку НЕ нажимал: действие необратимое.

### Волна 12 — по жалобам юзера + ревью цикла 7
1. **Тул «Extensions» в сайдбаре не работал**: тело сайдбара всегда рисовало
   `sessions_sidebar`, а активный тул влиял только на подсветку плитки.
   Теперь `ActivityClicked` ставит активный тул слоту Sidebar, а тело
   диспатчится как в оригинале (`Sidebar.tsx:81-85` → `ActivityBody`):
   projects → сессии, иначе `tool_body(Sidebar)`, при пустом слоте —
   `ActivityPlaceholder «No tool selected»`. Добавлена ветка `extensions`
   (`ActivityBody.tsx:33`) и подгрузка списка при активации тула.
   Проверено кадром: заголовок EXTENSIONS + Install, группа «BUILT-IN — 4»,
   строки с версией/статусом и кнопкой Disable.
2. **Иконки управления окном были мельче оригинала.** Замер живого оригинала
   через CDP: computed `font-size` глифа = **16px** (правило `.codicon` бьёт
   `.btn > i` 13px), ink 10.4×11.2 лог.; у нас стояло 13 → ink 6.4×8.0.
   Поставил 16 — высота сошлась (11.2), ширину нужно перемерить чистым
   кропом (в последнем замере окно захватило все три кнопки).
3. Ревью ц.7 (зона 130-159, MATCH 10 / DIVERGES 20; **HEX-токены не
   разъехались** — пересверены все 6 bg / 5 text / 11 accent / 4 semantic /
   hover / glint в обеих темах + метрики + 9×2 тени): исправлены мои же
   ошибки — глиф «Extensions» был `\u{eae1}` (= diff) вместо `\u{eae6}`,
   «Output» `\u{eb9c}` (= library) вместо `\u{eb9d}`; brand в семпле
   статус-бара → accent-primary + weight 500; `.chipMuted` → 12%/25% своим
   рецептом; тексты семплов → «active/idle/error», «Ctrl+Shift+P»,
   «npm run check»; `.btnGhost` получил прозрачный бордер; свотч токена
   стал тянущимся (`min_w 180 + flex_grow` вместо жёсткой ширины).

Осталось по ц.7: 8 нереализованных блоков (dropdown, toasts, modals,
external toasts, tooltip, checkbox dropdown, context menu, placeholders),
порядок блоков не как в оригинале, дерево-семпл собрано по метрикам
FileTreeView вместо `Tree.module.css` (нужны `.treeFrame` max-w 380 + p8 +
bg-base, gap 8, chevron 14/10, иконки 12 с accent-yellow у папок, indent 14,
колонка meta), в 146/147 нужны Phosphor-иконки вместо codicon, в 152 —
подписи `.codeInline`-чипами 10px и слот `main` вместо `bottom`.

### Волна 13 — по ревью цикла 7 (панели, MATCH 13 / DIVERGES 27, закрыто 6)
1. **РЕГРЕССИЯ волны 10 (моя)**: один `InputState` на Logs и System поставил
   в Logs чужой плейсхолдер «Filter logs…». Оригинал: Logs — «Filter…»
   (`LogsPanel.tsx:103`), System — «Filter logs…» (`SystemLogPanel.tsx:33`).
   Инпут теперь пересоздаётся при смене панели (`log_filter_panel`).
2. **Карточка Legacy Bridge**: кнопка «Remove old Bridge» прижималась к верху
   (у родителя `items_start`, а в CSS `.remove { align-self: center }`;
   замер ц.7 — центр на ~15 CSS выше оригинала). В нашей версии gpui у
   `Stateful` нет `self_center()`, поэтому центрируем обёрткой на всю высоту.
3. **`detect_electron_bridge()` вызывался в рендере** — реестр опрашивался
   каждый кадр. Теперь разовый детект при открытии Customize, результат
   лежит в `RootView.legacy_bridge`.
4. Волна 12 (по жалобам юзера) подтверждена ревьюером: левый край плиток
   activity-бара 12.0 CSS у обеих сторон (сдвиг 2px устранён), глиф Clear
   `clear-all`, плейсхолдер System дословный, перенос описания в Settings.

Остаток ТОП-5 по зоне: `noSessions` не гасит File/Right/MainBottom и нет
fill-режимов; drop-индикация `over`/`blocked` не портирована ни на одну карту;
`Input` не наследует паддинг/кегль обёртки (инсет 20 против 8, кегль 13.7
против 12); стрип скрыт при пустом `pinned`; busy-состояние кнопки Legacy.
- Кнопка Legacy-карточки: центрирование проверено кадром `_chk-w13b-card.png`
  (кнопка стоит по центру карточки справа, а не прижата к верху).
- Стрип панели теперь рисуется ВСЕГДА (`slot_panel.rs`), как `BottomTabBar.tsx`
  оригинала: при пустом `pinned` остаётся пикер «…».

### Волна 14 — размер `Input` берётся из самого компонента
`gpui_component::Input` кегль и паддинги берёт из СВОЕГО `Size`
(`input.rs:360 input_text_size(self.size)`), а не из обёртки — поэтому
`text_size`/`px` на родителе не действовали (ревью ц.7: инсет 20 CSS против 8,
кегль 13.7 против 12). Фильтрам Logs и System выставлен
`with_size(Size::Size(px(FS_SM / 0.875)))` — `input_text_size` для
`Size::Size(x)` даёт `x * 0.875`, то есть ровно 12.

⚠ Чистого замера ПОСЛЕ правки пока нет: полоса, по которой я мерил инсет,
захватила ещё и сайдбар, поэтому числа (35 против 27 dev) невалидны.
Перемерить кропом строго по внутренней грани поля.

## Цикл 9 — титлбар: 4 MATCH / 6 DIVERGES

Волна 16 закрыла 04 (зазор чип→«+» 6.40 = оригинал), 05 (пилюля 159.20 против 157.60), 09 (ink 207,212,226 = --text-primary), 12 (иконки 16/16/13/12 = каскад). Ложных претензий ц.8 в зоне нет — все десять подтвердились. Осталось: 02 и 18 — только отмеченные отклонения, 11 и 14 — по одному-двум остаткам, 15 (1fr-равнение колонок), 17 (кламп бара 2.0 против инсета 1.5).

### Новые находки цикла 9 (вне списка ц.8)

1. Резерв 580 в `root.rs:5626` (`tabs_w = viewport_w − 580`) занижен на ~45: фактический правый блок = 86 + 452.8 + 24 + 38 + 24 = 624.8 → при полном стрипе последний чип КЛИПАЕТСЯ (`flex_shrink` + `overflow_hidden`), а не уходит в «N ⌄». Считать резерв от фактических ширин кластеров.
2. `anchor_below(pop_h = 0.0)` — ветка flip'а «нет места снизу → сторона top» из `clamp-popup.ts:88-89` недостижима.
3. `.tabsSlot` у оригинала контентной высоты, у нас `.h_full()` → hit/drag-область слота 42 вместо ~28 (визуально невидимо).
4. Правый офсет appearance-поповера без гаттера (`.max(0.0)`) — но у оригинала `right:0` тоже без клампа, паритет соблюдён.
5. Для протокола: ширина чипа 180.00 против 173.60 — в кадрах разное число сессий (5 против 7); шаг межчиповых зазоров 3.20 совпадает.

Probe в этом прогоне был недоступен (порт 9333, ECONNREFUSED) — 11/14/15 проверены по коду, остальное пиксельно (рамка захвата подтверждена: контент с (9,9) физ., шаг 1.25).

## Цикл 9 — сайдбар: 1 MATCH / 6 DIVERGES

Закрыто 22 (резолвер тела). У 20 закрыты ширина и нав-в-Customize, остался `flex-shrink`.

**★ Дефект МЕТОДИКИ, найден ревьюером:** в папках 25, 28, 31, 34, 35, 36, 37 файлы
`original.png` и `ours.png` — ПОБАЙТОВО один и тот же кадр sessions-сайдбара
(проверено numpy). Кадров поповера, rename-инпута, свотчей и Customize-нава
оригинала не существует вовсе — эти вердикты держатся только на сверке CSS с
кодом. Гейт считает такие элементы «с парой кадров», то есть врёт: нужна
проверка НЕидентичности кадров пары, а сами состояния надо снять.

Снята ложная претензия из `20-sidebar-root/ours.md` п.1: `SIDEBAR_MIN_WIDTH_PX = 100`
(`constants.ts:45`) и наш `PANEL_MIN_SIZE = 100` совпадают; кламп драга тоже есть
(`root.rs:3193`).

## Цикл 13 — тулы-одиночки (сверх оригинала)

`activity::is_singleton(id)` = тул есть в реестре contributed → один экземпляр.
`ActivityModel::slot_of(id)` даёт его «дом». `pin()` в чужой слот — no-op.
Пикер (`ui/tool_picker.rs`) метит одиночек замком, а занятые — именем панели и
приглушением 0.5. Перетаскивание не ограничено: это перенос, а не копия.

## Цикл 15 — правки по пяти ревью (1-19, 38-51, 92-107, 108-129, 130-159)

Ревью цикла: 40 MATCH / 61 DIVERGES на 101 перепроверенном элементе.
Ниже — что закрыто сразу после отчётов.

### Патч vendored gpui: `repeat(auto-fill, minmax(<min>, 1fr))`

`Style::grid_cols_min` + `Styled::grid_cols_min(px)` + маппинг в
`taffy::RepetitionCount::AutoFill` (`vendor/gpui/src/{style,styled,taffy}.rs`).
Без него три CSS-грида рисовались flex-wrap'ом: неполный последний ряд
растягивался на всю ширину (замер: ячейка `--bg-overlay` 995 px вместо 193),
шаг дорожек не совпадал. Переведены `.swatches` (130, minmax 180),
`.radiusGrid` (133, 120), `.shadowGrid` (134, 140). Проверено кадром:
осиротевшие ряды держат ширину дорожки.

### Геометрия и якоря

- **128** тосты: слот получил ФИКСИРОВАННУЮ ширину 360 + `items_end` —
  у absolute-бокса без ширины она схлопывалась по контенту, и выравнивать
  было нечего. Замер: правый зазор 16.0 лог. (было 245), нижний 36.
- **44** пикер тулов: якорь — РЕКТ триггера (`probe_area("picker-anchor-<slot>")`
  у «…» стрипа, «…» бара и пилюли «Open Tool»), центровка по кросс-оси и флип
  по правилам `clamp-popup.ts`; была сырая точка клика (до 108 px мимо).
- **45** контекст-меню тула: `left = x − w/2` (нулевой якорь в курсоре) + флип
  вверх; было `left = x` (до 90 px мимо).
- **46** сабменю: центровка по строке «Move to»
  (`top = row.top + row.h/2 − sub.h/2`); было верхом на верх строки (~71 px).
- **11** поповер Layout: центровка и флип по ИЗМЕРЕННОМУ размеру (замер
  прошлого кадра через `probe_registry`), лишний вертикальный кламп снят.
- **14/15** Appearance: три колонки равны (`repeat(3, minmax(140px,1fr))` при
  `width: max-content`), у хедера `gap: space-3`.

### Состояния и цвета

- **38** полоса activity-бара: вернулось `blocked` (красное) состояние,
  ширина тинта — ровно `<nav>` 48, а не бокс 52.
- **39/40/49/51** ховер осветляет ГЛИФ: `tool_glyph_group_hover` через
  `group_hover` (у `svg()` собственный `.hover()` цвет не меняет).
- **136** `.listItem { cursor: pointer }` / `not-allowed` у disabled.
- **138** светлая ветка `[data-theme=light] .dropdownItemPicked`.
- **119** `UTF-8` рисуется при любом открытом файле, EOL — отдельно.
- **123** невыбранный чекбокс QuickPick — `circle-large-outline` (ebb5).
- **120** пилюля апдейта — `cloud-download` (eac2), был `arrow-down`.

### Метрики

- **43** бокс FontAwesome: `1.25em × 1em` (FA7 `--fa-width`), были жёсткие 16×16.
- **97/94/95** бейдж декорации без значения больше НЕ рендерится: пустой `div`
  оставался flex-элементом и съедал `gap 6` у лейбла.
- **146/147** семплы берут токены тулов (`terminal`/`warning`/`output`,
  `folders`/`tree-view`/`search`) → Phosphor-ветка `ToolIcon`; у плиток и
  пикера появились `data-tooltip`.
- **149** шеврон контекст-меню 16 (правило `.chevron{12px}` проигрывает
  вендорной базе).
- **75** радиус вью на боевом пути visual hosting: клип зоны и вырез фона
  `--radius-lg` 16 (были 0 и 12).

### Досье переписаны с живого кода

78, 79, 80, 81, 82, 85, 86, 87, 89, 90 (+ ссылки на строки в 73, 75, 88).

### Инструментальное

`probe screenshot` отдаёт кадр 181×24, когда ГЛАВНОЕ окно свёрнуто:
`find_window` берёт минимизированный HWND (rect −32000). Лечится
`ShowWindow(SW_RESTORE)`; кадр экрана (`cmd: "screen"`) при этом ловит чужие
окна поверх.

## Цикл 16 — ревью зон 20-37 и 52-91

Ревью: 12 MATCH / 6 DIVERGES (20-37) и 20 MATCH / 20 DIVERGES (52-91).
Правки сразу после отчётов:

- **33** глиф «Unpin from top bar»: `codicon-pinned-dirty` = `ebb2`; стоял
  `eba1` = `codicon-github-inverted` — в меню закреплённой сессии рисовался
  значок GitHub.
- **31** цвет текста инпута переименования: `theme.colors.foreground`
  синхронизируется с `--text-primary` (оставался крейтовый `#fafafa`).
- **26** порядок сессий и проектов: активные — по времени СОЗДАНИЯ
  (`sessions.ts:65`: выбор сессии не переставляет строку), проекты — по
  `created_at`; алфавитная сортировка была отсебятиной.
- **23** пустое состояние сайдбара лежит внутри `.list` → левый инсет 16.
- **67/76 ★ регрессия ц.15**: радиус выреза вебвью стал общей константой
  `RADIUS_LG` 16 и скруглил ещё и `.viewport` браузера, которому положено 12.
  Теперь радиус хранится ПО ЗОНЕ (`wv_visual::set_zone_mask_radius`,
  `zones_logical_r`), маски в `glint.rs` берут его у зоны: браузер 12,
  contributed-вью 16.
- **80** ховер строки канала Logs поднимает и ЦВЕТ ИМЕНИ (у него собственный
  `text_color`, `.hover()` строки до него не доходил) — через `group_hover`.
- **81** тексты пустого состояния системного лога дословно из
  `SystemLogPanel.tsx:57`.

### Известный системный остаток

`text_ellipsis()` в gpui не рисует «…» — это всплывает в 24, 28, 94, 95, 98,
105, 111. Лечится `ui::text_fit::fit`, но ему нужны `&mut Window` и ширина
контейнера: нужен отдельный проход с прокидыванием окна в
`sessions_list`/`file_list`/`editor_tabs`.

## Замечания пользователя (ц.16)

- **Терминальная карта выезжала за правый край панели.** `.root { margin: 0 6 6 }`
  был перенесён как `mx/mb` на бокс с `size_full`: ширина = 100 % родителя,
  а margin добавлялся СНАРУЖИ. Замер до: `central-bottom` right 1685.6,
  `term-toolbar` right 1716.8 (+31.2). Отступы переехали на внешнюю обёртку
  как padding — теперь ширина карты = ширина панели − 12.
- **В меню «+» было 3 шелла вместо 6, все с одним глифом.** Обнаружение
  переписано 1:1 с `kamin-host/services/shells.ts`: Windows PowerShell
  (`System32\WindowsPowerShell\v1.0`), PowerShell 7 (`ProgramFiles\PowerShell\7`,
  иначе x86), Command Prompt, Git Bash (три кандидата), затем КАЖДЫЙ
  WSL-дистрибутив из `wsl -l -q` (вывод UTF-16LE). У профиля появилось поле
  `icon` → `terminal-powershell` / `-cmd` / `-bash` / `-linux`, меню рисует
  глиф по нему. Список кэшируется на процесс (`OnceLock`).
- **Monaco в порт не переносится** — курс на собственный редактор. Критерий
  сверки элемента 113 сужен до внешнего вида хоста (моно 13, цвета
  `--editor-*`, ползунок, поля, прокрутка); опции Monaco вне сверки.

## Цикл 17 — правки до отчётов ревьюверов

- **124/125/126** двойная подсветка снята: у строк QuickOpen / Find in Files /
  Workspace Symbols больше нет `:hover`-заливки — наведение ПЕРЕНОСИТ активный
  индекс (`on_mouse_move` → `ShellEvent::OverlayRowHover(kind, idx)`), как
  `onMouseEnter` оригинала. Подсвечена ровно одна строка.
- **90** строка меню шеллов перестроена по оригиналу: `.menuRow { gap: 2 }`
  с `.menuItem { flex: 1; padding: 8/12 }` и звездой СНАРУЖИ пункта; ховер
  красит только пункт.
- **100/101** файловое контекст-меню и его каскад анкорятся по ИЗМЕРЕННОЙ
  коробке (probe-регион `file-menu` с прошлого кадра) — были оценки
  `est_h` 380/330 и минимальная ширина 180.
- **77** свечение под лого welcome гаснет на 68 % радиуса
  (`bake_glow_edge`), а не у самого края.
- **89** табы терминала ужимаются при тесноте (`flex: 0 1 auto`).

Осталось из известного: `busy`-состояние карточки Legacy Bridge (83),
нативный чекбокс настроек (82), эллипсис «…» (системный, 7 элементов).

## ★ Цикл 17 — корневая причина шести расхождений

Overlay-окно живёт на ВЕСЬ ЭКРАН, а его корневой слой стоял `size_full()` —
контейнер получался 2049.6×1153.6 (экран / 1.25) вместо main-вьюпорта.
Всё, что центрируется `justify_center`/`items_center`, съезжало: палитра,
QuickOpen, Symbols — x 704.8 вместо 380; Find in Files — 664.8 вместо 340;
Confirm/Prompt-модалка — центр 1031/577 вместо 700/450.

Правка: слой получает явные `w(vw) / h(vh)` по main-вьюпорту.
Проверка: `ov-palette` x 380.8, w 638.4 → центр **700.0** при окне 1400.
Закрывает 121, 122, 124, 125, 126, 127 одним изменением.

Заодно снят мёртвый `let hover` в `quick_open.rs`, `find_in_files.rs`,
`workspace_symbols.rs` (остатки двойной подсветки).

## Цикл 17 — зона 38-51 (9 MATCH / 5 DIVERGES)

Подтверждено пиксельно: якорь пикера (центр меню = центр триггера, Δ ≤ 1 px),
контекст-меню `x − w/2` + кламп, blocked-полоса и тинт ровно 48, ховер глифа
плиток, бокс FontAwesome `1.25em × 1em` (пилюля 100.8 × 21.6 вместо 24 по
высоте), drag-ghost и плейсхолдеры.

Правки по отчёту:
- **42/49** ховер красит ГЛИФ и подпись у «…» бара, «…» стрипа и таба стрипа
  (`group_hover`); глиф бара переведён на общий `icon::codicon` — бокс = кегль.
- **46** высота строки сабменю 30 (там `PanelIcon` 14×12, а не codicon 16):
  центровка по строке «Move to» промахивалась на 4.8 px; левый край
  `x + MENU_W − 1` (было +4 лишних).

Осознанные отклонения (в оригинале иначе, оставлено намеренно):
- **44** замок и имя слота-владельца у тулов-одиночек — понятия singleton в
  оригинале нет вовсе, добавлено по просьбе пользователя.
- **51** `tree-view` у оригинала не резолвится (класса нет в вендоре) и даёт
  ПУСТОЙ бокс; неизвестное имя — тоже пустота. У нас Phosphor-глиф и фолбэк
  `codicon-file`.

## Цикл 17 — добивка оверлеев

- **127** палитра: первая строка под курсором остаётся 12 % (её
  `li:first-child .row` (0,3,1) перебивает `.row:hover` (0,2,0)); текст
  пустого состояния — ASCII-кавычки и ВСЕГДА с запросом; высота панели ровно
  60vh (`.scrim` без `align-items` ⇒ stretch), пол 320 снят.
  Замер после: `ov-palette` h 538.4 при ожидании 540.
- **123** QuickPick: описание и детали участвуют в фильтре ТОЛЬКО при
  `matchOnDescription` / `matchOnDetail`, добавлен `alwaysShow`; «No matching
  items» показывается, когда фильтр отсёк всё; высота панели
  `min(100vh − 84, 60vh)`.

## Цикл 17 — хвост правок по зонам 108-129

- **118** цвет contributed-элемента статус-бара: добавлен `parse_css_color`
  (hex 3/6, `rgb()/rgba()`, базовые именованные) — `style={{ color }}` в
  оригинале пропускает любую CSS-строку, у нас проходил только `#rrggbb`.
- **111** `.tabDragging { opacity: 0.3 }`: индекс перетаскиваемого таба
  протянут в `editor_tabs_bar`, исходный таб гаснет.

Проверено выборочно и снято как НЕ расхождение (ревьювер читал устаревший
код): гейты `matchOnDescription`/`matchOnDetail` и `alwaysShow` в QuickPick
уже реализованы (`quick_pick.rs:163-167`), пустое состояние по `shown == 0`
на месте (`:246`), первая строка палитры под ховером остаётся 12 %
(`command_palette.rs:103`).

Осталось в зоне: фокус-рамка инпута prompt-модалки (нужна привязка к
состоянию фокуса `InputState`), высота ряда инпута (`Input` несёт свой бокс),
`WebviewPanelView` + иконка расширения + `downloading`-состояние апдейта.

## Цикл 18 — зоны 92-107 (6/10) и 130-159 (21/9)

Подтверждено замерами: гриды Design (8 дорожек 206.40 / 12 дорожек 137.6 /
10 дорожек 165.92 — ровно `auto-fill minmax`), Phosphor-иконки и тултипы
семплов, шеврон контекст-меню 16, курсоры листа, скроллбар-токены,
glint-кромка, семейство иконок панелей.

Правки:
- **100** файловое контекст-меню: центрирование по курсору (`left = x − w/2`)
  + переворот вверх при нехватке места снизу + кламп по ВНЕШНЕЙ коробке
  (padding 4×2 + рамка 1×2). Замеры ревьювера: Δ 94 px по X и 106 px по Y,
  свисание на 10 px — закрыто.
- **92** Ctrl+клик по КОРНЮ больше не сворачивает дерево
  (`applyClickSelection` вернул true → раскрытие не трогаем).
- **98** `.btn[disabled] { cursor: not-allowed }`.
- **104** тело contributed-дерева получило видимый ползунок
  (`overflow_y_scrollbar`), как у файлового.
- **138** пункт дропдауна растягивается на ширину меню: `w_full` внутри
  absolute-родителя taffy не резолвит — заливка выбранного была короче на
  58 px, а hint с галкой не прижимались вправо; hint выбранного пункта в
  светлой теме красится в `accent-action-fg`.
- **157** карта остаётся подсвеченной при самодропе: в оригинале
  `isOver` истинно и когда `sourceSlot === slot`.

## Цикл 18 — доработка после отчётов

- **106** рамка чекбокса contributed-дерева следует `currentColor` строки на
  ховере: `.row:hover { color: text-primary }` перекрашивает и её. Строка
  объявляет группу, чекбокс красится `group_hover`.
- **139** семпл дерева стал интерактивным, как в оригинале: состояние
  (`tree_expanded`, `tree_selected`) переехало в `DesignState`, добавлено
  `DesignAction::TreeClick`; клик по папке раскрывает её, по файлу — переносит
  выделение (было захардкожено, колбэк пустой).

## Цикл 19 — зоны 1-19 (12/7) и 52-91 (19/21)

Закрыто ревьюверами по замерам: ширина и правый край поповера Appearance
(561.6 = 3 × 176 + гэпы + паддинги; колонки равны), анкор layout-поповера по
измеренному размеру, радиусы зон 12/16, спад свечения welcome 68 %, инсеты
карты терминала, `flex: 0 1 auto` у табов терминала, звезда и ховеры в меню
шеллов, дебаунс фильтра логов и сброс при смене канала.

Правки:
- **Мёртвый код**: `root.rs` `webview_panel` + `webview_body` (68 строк) без
  единого вызова — удалены; всё идёт через `webview_body_dyn`.
- **09** ховер quick-action поднимает и цвет SVG (`group_hover` от группы
  кнопки) — `.btn:hover { color: text-primary }`.
- **19.1** при `switching` пульсирует ТОЛЬКО точка, пин остаётся в слоте
  (оригинал анимирует `.switching .dot`, а `.pinnedTab .pin` продолжает
  показывать пин).
- **19.2** бюджет метки чипа: у СПЯЩЕГО чипа кнопки disconnect в DOM нет
  (`SessionTab.tsx:53`) — метке достаётся на 24 px больше; плюс учтены 2 px
  рамки (`CHIP_CHROME_W` 62 → 64 / 40).
- **07** бокс `fa-bug` в кнопке DevTools пиннится к 16×16: правило
  `.btn > i { width:16px }` (0,1,1) перебивает FA-шное `width: 1.25em`.
- **80** пустой буфер логов больше не подписывается — у оригинала `<pre>`
  просто пуст.
- **81** «нет логов» определяется по `entries.is_empty()`, а не по фильтру:
  при пустом логе с выбранным уровнем показывался текст про фильтр.

## Цикл 19 — доработка: файловая колонка живёт в пикселях (63/64/65)

Оригинал ведёт ВЫСОТОЙ НИЖНЕЙ КАРТЫ: `height = max(100, round(ratio ·
innerHeight))` + `flex-shrink: 0`, а верх — `flex: 1`. У нас ведущей была
доля от body_h (vh − 42 − 24), поэтому карта недобирала ~37 px, а минимум
получался 83.4 вместо 100.

- `m::BOTTOM_PANE_MIN_HEIGHT = 100` (новая метрика).
- `file_bottom_px` считается от высоты ОКНА и клампится минимумом 100.
- Верхняя карта — `flex_1`, нижняя — `h(px)` + `flex_shrink_0`.
- Драг ведёт пикселями (`max(100, start − delta)`), ratio — только персист.

Замер после правки: `central-bottom` h = **352.8** (было 317.6, расчёт
оригинала ≈355) — остаток ~2 px даёт гуттер карты.

- **56** правая колонка масштабируется вместе с окном (`max(100,
  round(w · factor))` при смене ширины вьюпорта) — раньше ждала драга.
- **70/78** contributed-страница Customize показывает скелет загрузки вместо
  голого «Loading…».
- **91** курсор терминала мигает периодом 1.2 с (`cursorBlink: true`).
- **90** меню шеллов центрируется по якорю (`left = a.left + a.width/2 −
  p.width/2` + кламп гуттером 8), а не прижато `right: 0` — было ~86 px мимо.
- **83** карточка Legacy Bridge получила состояние `busy`: «Removing…» и
  погашенная кнопка на время удаления.

## Цикл 20 — зона 20-37 (12 MATCH / 6 DIVERGES)

Правки:
- **31** rename-инпут: `gpui_component::Input` ставит СВОИ `px 8 / py 2 / h 24`
  ДО `refine_style`, поэтому текст уезжал на 13 px вместо 5, а поле высотой 24
  в строке 24 срезало горизонтальные кромки и скругление. Погашены
  (`px_0 / py_0 / h 16`) — геометрию задаёт обёртка `.renameInput`.
- **28** активация строки по mouse-UP (`onClick` оригинала) + двойной клик =
  переименование.
- **24** тултип пути висит только на имени группы, а не на всей строке 26 px.
- **33** кламп контекст-меню сессии по ИЗМЕРЕННОЙ коробке (замер живьём
  202.4 × 177.6) вместо фиксированных 200 × 260 — у краёв промах был 4 и 81 px.

Пробный тост probe: `severity` теперь читается и из ключа `level`, дефолт
`info`, а `sticky` по умолчанию **false** — тестовый тост «Compile error»
приезжал с зелёной галкой и висел до перезапуска.

## Цикл 20 — зона 108-129 (5 MATCH / 17 DIVERGES): три сквозные причины

Ревьювер свёл 9 расхождений к двум механизмам и снял три ложных вердикта
прошлых циклов (глифы `symbol-*` слеплены в САМОМ шрифте; `.empty`
файл-вьювера недостижим и у оригинала; ховер contributed-элемента с
`item.color` у нас корректен). Заодно вскрыл два ошибочно закрытых:
меню переполнения табов на 4 px левее (112) и ховер-цвет бренда (120).

Правки:
- **`.hover().text_color()` до дочерних элементов не доходит** (доказано
  тремя живыми ховерами: фон меняется, цвет нет). Статус-бар красит подпись
  и глиф через `group_hover` — закрывает 117, 118, 120.
- **`gpui_component::Input` навязывал свой бокс (`px 8 / py 2 / h 24`) и
  кегль 14**: ряды ввода оверлеев выходили 42-50 лог. px вместо ≈34.
  Во всех пяти оверлеях (QuickPick, QuickOpen, Find in Files, Symbols,
  палитра) выставлены `--fs-md` 13 и нулевой собственный бокс — закрывает
  часть 122, 123, 124, 125, 126, 127.
- **123** `.list { flex: 1 }`: ряд Cancel/OK прижат к низу панели, под ним
  больше нет ~290 px пустой мантии.

## Цикл 20 — хвост (зона 108-129)

- **111** ховер таба редактора красит и подпись (`group_hover`) — собственный
  `.hover()` до дочернего текста не доходит.
- **112** меню переполнения: `.overflow { padding-right: space-1 }` оборачивает
  И кнопку, И меню, поэтому `right: 0` меню считается от правого края РЯДА, а
  кнопка стоит на 4 внутрь. У нас `mr` висел на кнопке и меню уезжало на 4 px —
  теперь кнопка с `mr 4`, меню с `right: −4`.
- **108** инсеты рамки редактора 6/6 (`.viewer { margin: 0 6px 6px }`) вместо
  4.8/4.0.

## Цикл 21 — зона 92-107 (5 MATCH / 11 DIVERGES)

★ Ревьювер ЗАМЕРИЛ то, что раньше проходило как «упор в движок»: у строки
дерева ink лейбла (173,179,199) не менялся под ховером, у кнопки тулбара —
(128.5,134.9,156.9) → (128.5,135.5,157.5), при том что ФОНЫ совпали с
оригиналом до единицы (51,51,67 и 52,53,69). Итого `.hover().text_color()`
не достаёт вложенные элементы — это чинимо через `group_hover`.

Правки:
- **94/95/92** ховер строки дерева красит лейбл; **96** — «Show N more» и её
  глиф; **98** — глиф кнопки тулбара; **105** — лейбл строки
  contributed-дерева.
- **98** `cursor: not-allowed` у disabled-кнопок: вердикт ц.18 объявил его
  закрытым, но правка тогда НЕ применилась (regex промахнулся) — теперь на
  месте.
- **100** `probe_area` меряет PADDING-box, а мы прибавляли паддинги ещё раз:
  меню стояло на 5.2 px левее и переворачивалось на 9 px раньше. Плюс снят
  лишний вертикальный кламп — у side bottom оригинал клампит только
  поперечную ось.
- **101** каскад «Open In» центрируется по строке-якорю (было +18 px), X
  считается от неразутой ширины, добавлены `overflow-y: auto` и `max-width`.

## Цикл 21 — зона 38-51 (7 MATCH / 7 DIVERGES)

Правки:
- **38 ★ откат моей ошибки ц.18**: подсветку цели при самодропе вернул.
  Источник — `activity-dnd.ts:69-71`: при дропе на свою же позицию сигнал
  обнуляет `overSlot`, значит `isOver` там false. В ц.18 я прочитал только
  `useActivityDropTarget.ts:36` и «починил» верное поведение.
- **39/49** тултипы: плитки берут лейбл из ОБЩЕГО реестра (`lookup_any`) —
  contributed-тулы оставались без подсказки; у таба стрипа тултипа не было
  вовсе.
- **45** строка «Move to» с открытым сабменю держит accent 16 % даже под
  курсором (`.itemMoveTo[aria-expanded]` стоит ПОСЛЕ `.item:hover` при равной
  специфичности).
- **46** сабменю переворачивается ВЛЕВО, когда справа не влезает — раньше
  кламп по X накрывал им родительское меню.
- **49** неизвестный id таба больше не рисует фантомный «Tool» с шестерёнкой.
- **51** `ActivityPlaceholder` перешёл на единый резолв `tool_glyph_split`:
  свой код падал в `codicon-play` и не знал `<img>`-ветки — один и тот же
  тул давал «play» в теле панели и «file» в баре.

### ★ Спорное: работает ли `group_hover` для текста и кодиконов

Ревьювер ц.21 замерил, что ink подписи/глифа не меняется ни под `hover`, ни
под `group_hover`, и утверждает, что `group_hover` красит только `svg()`.
Ревьювер ц.20 на тех же основаниях предлагал `group_hover` как ЛЕЧЕНИЕ.
Мой контрольный замер не состоялся: probe-hover не изменил даже ФОН, то есть
событие до элемента не дошло — вывод сделать нельзя.

Разбор кода `vendor/gpui/src/elements/div.rs:2497-2505`: `group_hover`
применяется, только если у элемента есть hitbox, и он создаётся как раз при
наличии `group_hover_style` (`:1700-1701`) — то есть механизм должен
работать. Следующему ревьюверу: проверять РЕАЛЬНЫМ курсором (`SetCursorPos`),
а не синтетическим `WM_MOUSEMOVE`.

## Цикл 21 — хвост

- **100** файловое контекст-меню невидимо до первого замера
  (`visibility: hidden` оригинала) — первый кадр больше не рисуется по оценке
  с последующим «прыжком».
- **121** заголовки модалок со знаком вопроса («Delete session?» /
  «Delete project?»), пустой проект получил свой текст без «This cannot be
  undone» (`sessions-ui.ts:16,27-29`).

### Протокол проверки ховера для следующего цикла

Ни синтетический `WM_MOUSEMOVE` (probe `hover`), ни `SetCursorPos` в моих
замерах не сдвинули даже ФОН статус-элемента, а `screen`-кроп по прежним
координатам окна попал мимо (окно переехало). Порядок, который надо
соблюсти: (1) `GetWindowRect` + `ClientToScreen` СВЕЖИМ вызовом,
(2) `SetCursorPos` в центр элемента, (3) пауза ≥300 мс,
(4) кадр через `screen` (BitBlt экрана), НЕ через `screenshot` (PrintWindow
может рендерить кадр без hover-состояния).
- **110** модель переполнения стрипа табов: рисуем ВСЕ табы в исходном
  порядке и режем их `overflow: hidden`, как `FileViewerTabs.tsx:166` + css:21;
  кнопка «▾» лишь включается по факту переполнения и НИЧЕГО из стрипа не
  изымает. Раньше не влезшие выкидывались, а последний видимый подменялся
  активным — замер ц.20 дал 161 px пустоты и порядок «2-м стоит 6-й таб».

## Цикл 22 — системный остаток «…» закрыт

Многоточие при усечении в порте рисует НЕ движок: `text_ellipsis()` только
обрезает, а «…» дописывает `text_fit::fit`, которому нужны `&mut Window` и
ширина. До панелей `window` не дотянут (`tool_body` его не получает), поэтому
добавлен `text_fit::fit_approx` — та же логика без шейпера, ширина по числу
символов (`AVG_GLYPH_RATIO`, точность ±1 символ).

Бюджеты берутся из probe прошлого кадра:
- строка сессии — `sidebar_w − 98` (pl 16 + pr 8 + точка 4 + гэпы + время + пин),
- имя группы — `sidebar_w − 84`,
- строка файлового и contributed-дерева — `panel_w − indent − 52`.

Проверено кадром: в сайдбаре «Create new docx d…» вместо прежнего обрыва.
Закрывает пункт в 24, 28, 94, 95, 105 (в 98 и 111 остаётся — там свои
контейнеры).

## ★ Цикл 22 — корень ховер-цвета найден и закрыт патчем vendored gpui

Ревьювер ц.22 доказал замерами (фон меняется, ink — ни на пиксель) и указал
причину в коде: цвет текста запекается на LAYOUT (`div.rs:1618`
`compute_style_internal(None, …)`, `div.rs:1334` оборачивает детей базовым
стилем, `text.rs:339` кладёт цвет в шейпленные раны), а `hover_style` и
`group_hover_style` применяются только внутри ветки с hitbox
(`div.rs:2489-2505`), то есть на PAINT. У `svg()` цвет читается на paint
(`svg.rs:100-110`) — поэтому там ховер всегда работал, и мои правки
«через `group_hover`» лечили только svg-ветки.

Патч (`vendor/gpui/src/elements/div.rs`):
- `InteractiveElementState.last_bounds` — bounds элемента с последнего paint;
- новый глобал `GroupBounds` рядом с `GroupHitboxes` — bounds групп;
- в `Interactivity::paint` оба пишутся;
- в `compute_style_internal` при `hitbox == None` (layout) «наведено ли»
  решается по позиции курсора и этим bounds, после чего рефайнятся
  `group_hover_style` и `hover_style`.

Отставание — один кадр, на глаз незаметно.

**Проверка на живом окне:** статус-элемент «3 active», ink
`#a6e3a1` → `#cfd4e2` (207,212,226 = `--text-primary`); до патча цвет не
менялся. Закрывает корень у 39, 42, 49, 92, 94, 95, 96, 98, 105, 111, 112,
117, 118, 120, 135, 136, 144, 146, 147, 151.

## Цикл 22 — сайдбар при сужении

Список сессий и группы переведены на ширину КОНТЕЙНЕРА (`w_full` +
`min_w 0` у списка, групп, строк и заголовков): в вертикальном скроллере
дети берут max-content, поэтому при сужении сайдбара строки оставались
широкими и бейдж-счётчик уезжал от правого края (баг, пойман юзером).

Там же: якорь hover-пилюли и общий `probe_area` переведены с
`absolute + size_full` на `absolute + inset 0` — канвас с `size_full`
участвовал в раскладке flex-строки и отжимал соседей.

Тултип: `max-width`/`overflow`/`text-ellipsis` перенесены с абсолютной
коробки на вложенный текст в ОБОИХ рендерерах (`tooltip_box_at` и
`KaminTooltip::render`) — коробка схлопывалась, и от подписи оставалось «…».

## Цикл 22 — накрывашка переключения чата (72/76)

Портирован `ChatSwitchSkeleton`: `ui/chat_switch_skeleton.rs`. Соответствия
и обходы ограничений движка:
- `.brand` 96×96, `.glow` 150 — запечённый спрайт `bake_glow_edge(150, accent,
  .28, .66)` вместо `radial-gradient` + `blur(8px)`;
- `kaminSwitchBreathe` (2.4 с): `opacity .5→1` и «масштаб» .94→1.06 — через
  анимацию РАЗМЕРА бокса с сохранением центра (нет `transform: scale`);
- `kaminSwitchFloat` (2.4 с): подъём логотипа на 4 px — через `top`
  (нет `translateY`);
- `.caption` «Loading conversation…» fs-sm text-muted;
- `.bar` 180×3 r999 на `text-primary 8 %`, `.barFill` — два градиентных
  полубокса (у `linear_gradient` в gpui ровно два стопа), пробег 1.15 с.

Подключено в `root.rs`: рисуется поверх тела чат-вью, пока `switching_to`
не сброшен.
- **98** Locate: строка центрируется в видимой области (высота тела из probe)
  вместо фиксированных 140 px от верха; если листинги предков ещё не пришли —
  повтор через 120 мс, раньше скролл просто не выполнялся.
- **122** рамка инпута prompt-модалки: accent только в фокусе
  (`focus_handle(cx).is_focused(window)`), иначе `--bg-surface`.
- **122** при открытии prompt-модалки значение выделяется целиком (`SelectAll`,
  как `inputRef.current?.select()` оригинала).
- **98** `.flash` после Locate: вспышка найденной строки 0.9 с (accent 40 % →
  прозрачность). Ключ анимации содержит счётчик — повторный Locate по тому же
  файлу перезапускает вспышку, а не игнорируется.

## Цикл 22 — апдейтер внутри приложения (120)

По просьбе юзера: клик по пилюле больше не открывает браузер, а качает и
ставит апдейт сам (`crates/shell/src/updater.rs::install` — `ureq` со
стримом и тиком прогресса каждые 256 КБ, затем инсталлятор `/S` и
`process::exit(0)`, иначе он не перезапишет exe).

Пилюля получила состояние `downloading` = `.progressFill` оригинала:
заливка `accent-primary 32 %` шириной по проценту, при неизвестном
`Content-Length` — полная заливка с `opacity .5`; подпись «Updating N%» /
«Updating N.N MB» лежит поверх заливки (`.progressLabel { position:
relative }`). Ошибка возвращает пилюлю в обычный вид и поднимает
sticky-тост.
- **98** кнопка Refresh = полный ремаунт (`RefreshTreeHard`: сброс кэша и
  `child_cap`, папки снова в «Loading…»), watcher остался на мягком
  `RefreshTree` без сброса кэша (иначе панель мигает на каждое событие ФС).
- **94/95** `.dropTarget`: папка под перетаскиваемым файлом подсвечивается
  accent 22 % + рамка accent (`drag_over::<DraggedFile>`) — состояния не было.
- **94/95** клавиатура строк дерева: Delete / F2 / Ctrl+X / Ctrl+C / Ctrl+V.
  В gpui у div-ов нет фокуса, поэтому обработчик на общем key-handler окна и
  работает по `tree.selected`; инпут-оверлеи и rename имеют приоритет.
- **99/107** иконки дерева: светлый фильтр `saturate(3.2) brightness(0.7)`
  (`icon_light.rs`, матрица по hex внутри SVG + кэш), карты `rootFolder*`,
  слой `languageIds` и оверрайд `doc.light`. Вендор-патч: SVG из байтов не
  свизлился RGBA→BGRA (`platform.rs`), из-за чего иконки были коричневыми.
- **73** «контейнер → N вью»: `DynTool.views` + стопка секций, «No views» /
  «No view». Попутно: скелет вебвью-вью снимается по html (а не по сообщению
  от страницы) и `root_requested` вместо «есть ли состояние» — иначе дерево
  висело на «Loading…».
- **104/105** contributed-дерево: DnD (`hasDnd` + `handleDrag`/`handleDrop`,
  подсветка цели) и `treeReveal` (expandPath + select + nearest-скролл по
  bounds строки). Строка переведена на `on_click`, чтобы драг не выделял узел.
- **114** вебвью: loader-cover (bg-surface + спиннер-кольцо 22 с вращающейся
  дугой, фейд 180 мс, готовность = пинг либо 1200 мс) и load-watchdog 20 с →
  Retry. Спиннер — SVG + `with_animation`, поворот текста в gpui невозможен.
- **98** спиннер Indexing вращается: глиф `codicon-loading` вынут из шрифта в
  SVG-ассет, `steps(30)` за 1.5 с.
- **156** `:focus-visible`: вендорный патч gpui (`Window::focus_visible` —
  клавиатурная модальность) + `ui/focus_ring.rs` (кольцо 2 px accent, offset 2).
  Первая волна таб-стопов: activity bar и тулбар дерева.
- **156** волна 2-3 таб-стопов: титлбар, Logs, терминал, браузер-панель,
  пилюли сессий, фильтры Problems, overflow табов.
- **156** волна 4-5: статус-бар, чипы сессий, строки сессий/файлового и
  contributed-дерева. 124 таб-стопа на типовом экране.
- **Цикл 23, волны правок** (после отчётов семи ревьюверов): ховер цветной
  строки сессии; тексты удаления и заголовок модалки; абзац в модалке legacy;
  корневая строка дерева (ховер лейбла, dropTarget, кольцо, `fit_approx`);
  design-панель без своего скроллера; кольца фокуса титлбара/чипа; пол 320 у
  четырёх оверлеев; QuickPick `flex_1` + accent-рамка; ховер глифа
  статус-бара; пилюля апдейта (гэп/тултип/курсор); рейлы (глиф + фантом);
  схемы иконок расширений + `data:`; шторка чата (условие, таймаут, фейд,
  кэш свечения); перепекание свечения welcome при смене темы; реальные
  счётчики скелета Customize; `on_click` и драг папок в дереве; якорь
  сабменю; нативный чекбокс настроек; карточка ошибки чтения файла; хедер и
  `.bodyFlush` contributed-страницы; `:focus-within` в Logs/System log +
  тултип абсолютного времени.
