# ДОСЬЕ визуальной сверки — все 159 элементов в одном файле

Для каждого элемента: описание и код ОРИГИНАЛА, наш код, оба кадра и вся история вердиктов по циклам. Источник правды по каждому элементу — одноимённая папка рядом; здесь всё сведено вместе.

Генерируется: `python parity/dossier.py`. Краткая таблица — [INDEX.md](INDEX.md), перечень с файлами и классами — [INVENTORY.md](INVENTORY.md), гейт — `python parity/gate.py`, полнота атрибутов — `python parity/attrs.py`.

**Сводка: 66 MATCH / 93 DIVERGES из 159.**

## Оглавление по зонам

- **1-19 Титлбар** — 19 элементов, 13 MATCH
- **20-37 Сайдбар — сессии и Customize-нав** — 18 элементов, 13 MATCH
- **38-51 Activity-бар, рейлы, пикеры, стрипы** — 14 элементов, 3 MATCH
- **52-91 Панели, карты, экраны Customize, терминал** — 40 элементов, 17 MATCH
- **92-107 Дерево файлов и его меню** — 16 элементов, 1 MATCH
- **108-129 Редактор, оверлеи, статус-бар, модалки** — 22 элементов, 3 MATCH
- **130-159 Токены дизайна, sample-компоненты, глобальные стили** — 30 элементов, 16 MATCH

# Зона 1-19 — Титлбар

## 1. titlebar — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:MATCH, ц11:MATCH*

![оригинал](01-titlebar/original.png)
![наш](01-titlebar/ours.png)

### Оригинал

# 01 Titlebar — оригинал (KaminIDE 0.2.87, host renderer)

Файлы: src/renderer/components/titlebar/Titlebar.tsx (+ .module.css),
TitlebarButton.*, TitlebarQuickActions.*, LayoutToggles.*, ThemeQuickToggle.*,
PanelIcon.tsx, LayoutPresetsSection.tsx.

## Структура (Titlebar.tsx)
.titlebar (flex row, height var(--layout-titlebar-height)=42px, bg transparent,
drag-region, fs var(--fs-sm)=12px, color var(--text-muted))
 ├ .brand 42×42 (flex center, no-drag, color var(--accent-primary));
 │   .brandLogo 26×26 (лого-марка), codicon 18px
 ├ .leftCluster (flex, ширина = сайдбар, flex-shrink 0, overflow hidden, h 100%)
 ├ .tabsSlot (flex:1, min-width:0) — таб-стрип сессий
 └ правый кластер: quick-actions (поиск-команда, layout-toggles, theme toggle,
   DevTools), затем window controls (min/max/close)

## Computed (живой прод, ВНИМАНИЕ: снято в contributed-теме GitHub-dark)
titlebar: height 42px; font 12px "Bricolage Grotesque Variable"; weight 400;
color = var(--text-muted); bg transparent; flex; align-items center.
brand: 42×42; flex center; color var(--accent-primary).

## Тема-независимые метрики (сверять)
- высота 42px; brand-слот 42×42; лого 26×26; codicon 18px
- fs 12px (fs-sm), font Bricolage Grotesque, weight 400
- leftCluster ширина = ширине сайдбара; tabsSlot flex:1 min-width:0
- корневые токены (дефолт-тема сверяется по kamin_theme::DARK):
  fs-xs 11 / fs-sm 12 / fs-md 13; radius xs4 sm8 md12 lg16; space 4/8/12/16

## Скрин
original.png (contributed-тема; при цветовой сверке переключить прод в
дефолтную тёмную тему или сверять цвета по палитре, не по пикселям скрина)

## Метрики .titlebar (ИЗ CSS, Titlebar.module.css:5-16 — дополнение)
- размеры: height: var(--layout-titlebar-height); width — нет (flex-строка на всю ширину)
- отступы: нет padding/margin/gap на корне
- скругления: нет
- шрифт: font-size: var(--fs-sm); family/weight/letter-spacing не заданы (наследуются)
- цвета: color: var(--text-muted); background: transparent
- hover/active/focus: нет (корень не интерактивен)
- transition/анимации: нет
- позиционирование: display:flex; align-items:center; position:relative;
  z-index: var(--z-toast-lower); flex-shrink:0; -webkit-app-region: drag

## Состояния
Нет вариантных классов у корня. Drag-region сплошной; no-drag выставляют
дети (.brand, .tabsSlot, .controls, кнопки).

## Мёртвые классы в том же css (в JSX Titlebar.tsx не используются)
`.welcomeTab`, `.kbd` — присутствуют в Titlebar.module.css:66-117, не рендерятся.

### Наша реализация

# 01 Titlebar — наша сторона (gpui-kamin-ide)

Файлы: crates/shell/src/ui/titlebar.rs (+ вызов в root.rs), метрики
crates/metrics/src/lib.rs (TITLEBAR_HEIGHT=42.0, FS_SM...).

## Факт (probe tree, live)
zone: 0,0 2048×42.4 (ЛОГИЧЕСКИЕ px) — ВЫСОТА 42.4 ≠ 42 оригинала.
TITLEBAR_HEIGHT const = 42.0 → лишние 0.4 добавляет что-то в обвязке
(бордер? чип? паддинг контейнера) — найти при фиксе.

## Скрин
ours.png (максимизированное окно, дефолт-тема DARK).

## Структура (gpui-дерево кратко, titlebar.rs:135-407)
div#titlebar (relative, h TITLEBAR_HEIGHT, w_full, flex items-center,
text_size FS_SM, color text_muted; drag = свой DRAG_ARM: down армирует,
move ≥4px → start_native_window_drag; dblclick = zoom)
 ├ probe_area("titlebar")
 ├ .brand 42×42 (flex center, img kaminoid.svg 26×26)
 ├ quick-actions row (gap 1, px SPACE_2): toggle-sidebar [+divider+gear]
 ├ tabs-контейнер (flex, min_w 0, flex_shrink, overflow_hidden, h_full) → session_tabs
 ├ «+» new-session 28×28 круг (НЕТ в оригинальном Titlebar.tsx — там «+» внутри SessionTabs)
 ├ div flex_1 (пустота-drag)
 ├ #command-search (h26, пилюля)
 ├ layout-toggles 26×26 r12 · theme-toggle 28×28 r8
 └ контролы: DevTools + min/max/close (36×36 круги)

## Метрики (из кода)
- h = m::TITLEBAR_HEIGHT (42.0); fs = m::FS_SM (12); color p.text_muted (#838aa0 DARK)
- bg НЕ задан (прозрачный, градиент root просвечивает) — как оригинал
- корень: без padding/gap/radius — как оригинал

## Отличия от original.md
1. Живая высота 42.4px вместо 42 (см. «Факт» выше) — источник лишних 0.4 не найден.
2. leftCluster как контейнер отсутствует: brand и quick-actions — прямые дети корня,
   ширина НЕ пиннится к сайдбару (детали в 02-titlebar-left-cluster/ours.md).
3. Кнопка «+» (new session) — в титлбаре между табами и пустотой; в оригинале
   «+» живёт внутри SessionTabs-стрипа (элемент 18).
4. Drag-механика: свой пороговый native caption-drag вместо -webkit-app-region:
   drag на корне + no-drag на детях (поведенчески эквивалентно; z-index/webkit-токены не применимы).
5. font-family: gpui-дефолт окна (Bricolage задаётся на уровне окна) — совпадает по факту.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_SM = 12 на корне (`crates/shell/src/ui/titlebar.rs:197`); font-family/weight на титлбаре не задаются — наследуются от окна; кегли глифов: window-controls codicon 16 (`titlebar.rs:69`), quick-action svg 14×12 (`titlebar.rs:244-245`), gear fa 12 (`titlebar.rs:275`), search codicon 12 (`titlebar.rs:381`), layout fa 13 (`titlebar.rs:399`), theme fa 12 (`titlebar.rs:415`), DevTools fa 13 + label FS_SM=12 (`titlebar.rs:446-447`)
- ховер: у самого корня `#titlebar` ховера нет (`titlebar.rs:152-199`) — только у детей:
  - control_button: bg bg_surface #3d3f51, fg text_primary #cfd4e2 (`titlebar.rs:43,59`); close (danger): bg accent_red #f38ba8, fg bg_primary #313240 (`titlebar.rs:41`)
  - action_button: bg bg_surface #3d3f51 + text_primary #cfd4e2 (`titlebar.rs:86,108`)
  - search-пилюля: bg bg_surface #3d3f51 + text_secondary #adb3c7 (`titlebar.rs:376`)
  - DevTools: bg bg_surface #3d3f51 + accent_primary #89b4fa (`titlebar.rs:437`)
  - «+»: НЕпрозрачный микс accent_primary 36% + bg_surface 64%, fg accent_primary #89b4fa (`titlebar.rs:326-338`)

### Вердикты

# 01 — verdict (review cycle 1)
VERDICT: MATCH
h42/flex/fs-sm/text-muted/прозрачный фон — 1:1. Структурные вопросы учтены в 02/04/18.

## Цикл 2: MATCH

## Цикл 8: MATCH

h42 (низ 42.40 у обеих), фон прозрачный, fs-sm/text-muted (метка DevTools #838aa0, ink 50.40 дословно).

## Цикл 11: MATCH

h 42.40; метка DevTools ink (131,138,160) = --text-muted.

---

## 2. titlebar-left-cluster — **DIVERGES** (цикл 11)

*История: ц2:MATCH, ц8:DIVERGES, ц9:DIVERGES, ц11:DIVERGES*

![оригинал](02-titlebar-left-cluster/original.png)
![наш](02-titlebar-left-cluster/ours.png)

### Оригинал

# 02 titlebar-left-cluster — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:35-40
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:46-52

## JSX-структура (кратко, вложенность)
```
<div class=leftCluster style={width}>   // inline width, см. Состояния
  <div class=brand aria-hidden>         // элемент 03
    <img class=brandLogo .../>
  </div>
  <TitlebarQuickActions />              // элемент 08
</div>
```

## Метрики (ИЗ CSS)
- размеры: height: 100%; width — inline-style: `${sidebarWidth}px` при видимом сайдбаре, `auto` при скрытом
- отступы: нет padding/margin/gap
- скругления: нет
- шрифт: наследуется от .titlebar
- цвета: не заданы (наследуются)
- hover/active/focus: нет
- transition: нет
- позиционирование: display:flex; align-items:center; flex-shrink:0; overflow:hidden

## Состояния
- сайдбар видим (`sidebarVisible || sidebarMode === "customize"`): width = `sidebarWidth.value`px (пиннится к ширине сайдбара)
- сайдбар скрыт: width: auto
Других классов-вариантов нет.

## Дополнение атрибутов (цикл 10)

- цвета: `.leftCluster` своих background/color НЕ задаёт (`titlebar/Titlebar.module.css:46-52`) — прозрачный, наследует от `.titlebar`: background transparent, color var(--text-muted) #838aa0 (`Titlebar.module.css:9,15`); единственный собственный цвет внутри — `.brand { color: var(--accent-primary) }` = #89b4fa (`Titlebar.module.css:26`)

### Наша реализация

# 02 titlebar-left-cluster — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:183-244 (brand + quick-actions как прямые дети корня)

## Структура (gpui-дерево кратко)
НЕ РЕАЛИЗОВАНО как отдельный контейнер. У оригинала `<div class=leftCluster
style=width:sidebarWidth>` оборачивает brand + TitlebarQuickActions; у нас
brand (42×42) и quick-actions row — два самостоятельных ребёнка div#titlebar,
без общей обёртки.

## Метрики (из кода, точные)
- обёртки нет → нет width-пиннинга; суммарная ширина = 42 (brand) + контент quick-actions
- brand: flex_shrink_0; quick-actions row: flex_shrink_0

## Отличия от original.md той же папки
1. ГЛАВНОЕ: ширина кластера НЕ равна ширине сайдбара (оригинал: inline
   `width: ${sidebarWidth}px` при видимом сайдбаре, `auto` при скрытом).
   Следствие: таб-стрип сессий у нас начинается сразу после quick-actions
   (+48px pl в session_tabs), а не от правой кромки сайдбара.
2. Нет height:100% / overflow:hidden контейнера (нечему — контейнера нет).
3. Режим `sidebarMode === "customize"` (кластер остаётся пиннутым) не воспроизведён.

## Дополнение атрибутов (цикл 10)

- гэпы: контейнера-кластера НЕТ; фактические зазоры внутри области — quick-actions row gap 1 (`crates/shell/src/ui/titlebar.rs:220`), её px SPACE_2 = 8 (`titlebar.rs:221`), divider mx SPACE_1 = 4 (`titlebar.rs:258`); между brand 42×42 и row гэпа нет (`titlebar.rs:202-214`)
- скругления: N/A: скругления — контейнера-кластера нет, скруглять нечего; у детей rounded: quick-action RADIUS_SM = 8 (`titlebar.rs:226,264`), divider 1×14 без скругления (`titlebar.rs:255-259`)
- ховер: N/A: ховер — кластера как элемента нет; ховер несут кнопки внутри: bg bg_surface #3d3f51 + text_primary #cfd4e2 (`titlebar.rs:86,108`)

### Вердикты

# 02 — verdict (review cycle 1)
VERDICT: DIVERGES
.leftCluster отсутствует: нет width=ширине сайдбара (auto при скрытом), h100%,
overflow hidden; brand и quick-actions — прямые дети корня. (компенсация pl48 — deviation)

## Цикл 2: MATCH
(в пределах deviation pl48)

## Цикл 8: DIVERGES

`.leftCluster` у оригинала шириной sidebarWidth → первый чип на 280.80; у нас `STRIP_PL = 48` фиксировано (140.80) и за шириной сайдбара не следует. Отмеченное отклонение (юзер трижды просил больше воздуха), но расхождение фиксируем.

## Цикл 9: DIVERGES

`.leftCluster` шириной sidebarWidth (`Titlebar.module.css:46-52`, `Titlebar.tsx:29-31`) → первый чип 280.80. У нас обёртки нет вовсе (brand и quick-actions — прямые дети корня, `titlebar.rs:200-278`), отступ = константа `STRIP_PL = 48` (`session_tabs.rs:380`) → первый чип 152.20. Волна 16 добавила +12 паддингом слота, но связи с шириной сайдбара по-прежнему нет. ОТМЕЧЕННОЕ ОТКЛОНЕНИЕ (юзер трижды просил больше воздуха).

## Цикл 11: DIVERGES

Обёртки .leftCluster нет, STRIP_PL = 48: первый чип 152.40 против правого края сайдбара 240.0 → рассинхрон 87.6. СОГЛАСОВАННОЕ отклонение.

---

## 3. titlebar-brand-logo — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:MATCH, ц11:MATCH*

![оригинал](03-titlebar-brand-logo/original.png)
![наш](03-titlebar-brand-logo/ours.png)

### Оригинал

# 03 titlebar-brand-logo — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:36-38
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:18-40

## JSX-структура (кратко, вложенность)
```
<div class=brand aria-hidden="true">
  <img class=brandLogo src={kaminoid.svg} alt="" draggable={false} />
</div>
```

## Метрики (ИЗ CSS)
.brand:
- размеры: width: var(--layout-titlebar-height); height: var(--layout-titlebar-height) (квадрат = высоте титлбара)
- отступы: нет
- скругления: нет
- шрифт: `.brand :global(.codicon) { font-size: 18px !important; }` (запасной путь; в текущем JSX рендерится img, не codicon)
- цвета: color: var(--accent-primary)
- позиционирование: display:flex; align-items:center; justify-content:center; flex-shrink:0; -webkit-app-region: no-drag

.brandLogo:
- размеры: width: 26px; height: 26px
- object-fit: contain; display: block
- -webkit-user-drag: none; user-select: none
- hover/active/focus: нет
- transition: нет

## Состояния
Нет вариантных классов. Не интерактивен (aria-hidden, draggable=false).

### Наша реализация

# 03 titlebar-brand-logo — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:183-197

## Структура (gpui-дерево кратко)
```
div (.brand-аналог, 42×42, flex center, flex_shrink_0)
 └ img("icons/kaminoid.svg") 26×26
```

## Метрики (из кода, точные)
- бокс: w/h = m::TITLEBAR_HEIGHT (42.0) — квадрат = высоте титлбара
- лого: img 26×26 (px(26.0))
- отступы/скругления: нет
- цвета: не заданы (img самодостаточен)
- hover/active: нет (не интерактивен)

## Отличия от original.md той же папки
1. color: var(--accent-primary) на .brand не задан — у нас img, цвет не нужен
   (оригинальный codicon-fallback 18px тоже не реализован — некритично, в
   проде рендерится img).
2. draggable=false / user-select — не применимо (gpui img не перетаскивается).
3. aria-hidden — не применимо (нет accessibility-дерева).
Метрики (42×42 бокс, лого 26×26, flex center) — совпадают полностью.

## Дополнение атрибутов (цикл 10)

- шрифты: N/A: шрифты — brand-слот содержит только `img("icons/kaminoid.svg")` 26×26, текста и глифов нет (`crates/shell/src/ui/titlebar.rs:202-213`); своего text_size нет, наследует FS_SM = 12 корня (`titlebar.rs:197`). Отклонение от оригинала: у него в слоте есть codicon-ветка 18px (`titlebar/Titlebar.module.css:29`), у нас её нет

### Вердикты

# 03 — verdict (review cycle 1)
VERDICT: MATCH
.brand 42×42 + logo 26×26 — 1:1.

## Цикл 2: MATCH

## Цикл 8: MATCH

Лого совпало ПОБИТОВО: ink x 9.60..32.80, y 8.00..34.40.

## Цикл 11: MATCH

ink лого 26 лог. по вертикали (8.8..34.4), правый край 32.0.

---

## 4. titlebar-tabs-slot — **MATCH** (цикл 11)

*История: ц2:DIVERGES, ц8:DIVERGES, ц9:MATCH, ц11:MATCH*

![оригинал](04-titlebar-tabs-slot/original.png)
![наш](04-titlebar-tabs-slot/ours.png)

### Оригинал

# 04 titlebar-tabs-slot — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:41
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:57-64

## JSX-структура (кратко, вложенность)
```
<div class=tabsSlot aria-label="Open sessions">
  <SessionTabs />   // элемент 18; при 0 сессий SessionTabs возвращает null — слот пуст
</div>
```

## Метрики (ИЗ CSS)
- размеры: не заданы; flex: 1; min-width: 0
- отступы: padding: 0 var(--space-3)
- скругления: нет
- шрифт: наследуется
- цвета: не заданы
- hover/active/focus: нет
- transition: нет
- позиционирование: display:flex; align-items:center; -webkit-app-region: no-drag

## Состояния
Нет вариантных классов.

## Дополнение атрибутов (цикл 10)

- цвета: `.tabsSlot` своих background/color НЕ задаёт (`titlebar/Titlebar.module.css:57-64`) — прозрачный, наследует color var(--text-muted) #838aa0 и background transparent от `.titlebar` (`Titlebar.module.css:9,15`)

### Наша реализация

# 04 titlebar-tabs-slot — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:245-255 (контейнер),
crates/shell/src/ui/session_tabs.rs:392-399 (row внутри)

## Структура (gpui-дерево кратко)
```
div (flex, items-center, min_w 0, flex_shrink, overflow_hidden, h_full)
 └ session_tabs row (flex, items-center, min_w 0, overflow_hidden,
                     pl 48px, pr SPACE_3)
```
После слота отдельно идут «+» (28×28) и div.flex_1 (пустота-drag).

## Метрики (из кода, точные)
- контейнер: min_w 0, flex_shrink (НЕ flex:1), overflow_hidden, h_full
- row: pl px(48.0), pr m::SPACE_3 (12)
- скругления/шрифт/цвета: не заданы на слоте (несут чипы)

## Отличия от original.md той же папки
1. flex:1 отсутствует — слот сжимается по контенту (чипы фикс-180px),
   остаток ширины забирает соседний div.flex_1. В оригинале слот тянется.
2. padding: оригинал `0 var(--space-3)` (12/12); у нас pl=48 (сознательно —
   «воздух после quick-actions», юзер просил трижды), pr=12. Слева +36px
   к оригиналу.
3. aria-label="Open sessions" — не применимо.
4. -webkit-app-region: no-drag — заменяет occlude()/stop_propagation на чипах.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — сам слот не интерактивен (`crates/shell/src/ui/titlebar.rs:284-305`); ховер только у «+» внутри слота: bg = микс accent_primary 36% + bg_surface 64% (непрозрачный), fg accent_primary #89b4fa (`titlebar.rs:326-338`)

### Вердикты

# 04 — verdict (review cycle 1)
VERDICT: DIVERGES
Слот без padding 0 12 (pl48 deviation, pr12 внутри стрипа); flex:1 у соседа-спейсера;
«+» и drag-спейсер — сиблинги слота, зазор чип-плюс = 16 vs 6.

## Цикл 2: DIVERGES
Нет padding 0 12 у слота (правые 12 к search пропали с pr стрипа); flex:1 у спейсера (deviation-структура).

## Цикл 8: DIVERGES

`.tabsSlot { padding: 0 12 }` — у нас `pr(SPACE_3)` на СТРИПЕ, поэтому 12 ушли между последним чипом и «+»: зазор 18.00 против 6.40, а слева от search-пилюли 12 потеряны. Чинить обёрткой [стрип+«+»+спейсер] в слот с `px(12)`.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Слот табов переписан в структуру оригинала: `div` с `flex_1 min_w(0) px(12)` (= `.tabsSlot{padding:0 12}`), внутри `.strip` (чипы), затем «+», затем спейсер `flex_1 min_w(24)`. Раньше 12 висели `pr` на самом стрипе, из-за чего уходили в зазор чип→«+». `titlebar.rs:283-340`. Замер после правки: «+» x=871.2, правый край последнего чипа 865.2 → зазор ровно 6.0 (было 18.0).

## Цикл 9: MATCH

Закрыто. `titlebar.rs:284-351`: `flex_1().min_w(0).px(SPACE_3)`, внутри `.strip` → «+» → спейсер `flex_1().min_w(24)`; `pr` со стрипа снят. Замер: правый край чипа 1064.80, «+» 1071.20 → зазор 6.40; у оригинала 1514.40/1521.60 → 6.40. Было 18.00. Кружок 25.60, фон (61,63,81) = #3d3f51.

## Цикл 11: MATCH

правый край последнего чипа 860.00, «+» 866.40 → зазор 6.40 (как в ц.9); спейсер 29.2 ≥ min 24.

---

## 5. titlebar-command-search-button — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:DIVERGES, ц9:MATCH, ц11:MATCH*

![оригинал](05-titlebar-command-search-button/original.png)
![наш](05-titlebar-command-search-button/ours.png)

### Оригинал

# 05 titlebar-command-search-button — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:43-51
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:83-108

## JSX-структура (кратко, вложенность)
```
<button class=searchButton aria-label="Open command palette (Ctrl+Shift+P)"
        data-tooltip="Open command palette (Ctrl+Shift+P)"
        onClick=execute("workbench.action.showCommands")>
  <span class="codicon codicon-search" />
  <span class=searchHint>Type a command…</span>
</button>
```

## Метрики (ИЗ CSS)
.searchButton:
- размеры: height: 26px; width — авто по контенту
- отступы: padding: 0 var(--space-3); margin-right: var(--space-2); gap: var(--space-2)
- скругления: border-radius: var(--radius-sm)
- шрифт: font-size: var(--fs-xs)
- цвета: color: var(--text-muted);
  background: color-mix(in srgb, var(--bg-surface) 60%, transparent);
  border: 1px solid color-mix(in srgb, var(--bg-overlay) 30%, transparent)
- иконка: `.searchButton :global(.codicon) { font-size: 12px !important; }`
- hover: background: var(--bg-surface); color: var(--text-secondary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:flex; align-items:center; -webkit-app-region: no-drag

.searchHint:
- padding: 0 var(--space-2)

## Состояния
Только hover (см. выше). Вариантных классов нет.

### Наша реализация

# 05 titlebar-command-search-button — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:289-315 (id "command-search")

## Структура (gpui-дерево кратко)
```
div#command-search (occlude, flex items-center, пилюля)
 ├ codicon(SEARCH \u{ea6d}, 12.0)   // бокс 16×16, глиф 12px
 └ "Type a command…"
```
Клик → open_palette() (ShellEvent → командная палитра). Тултип
"Open command palette (Ctrl+Shift+P)".

## Метрики (из кода, точные)
- h px(26.0); ширина авто по контенту
- px m::SPACE_3 (12), mr m::SPACE_2 (8), gap m::SPACE_2 (8)
- rounded m::RADIUS_SM (8)
- шрифт: text_size m::FS_XS (11)
- цвета: bg = tint(p.bg_surface, 0.6) (#3d3f51 60%), border 1px
  tint(p.bg_overlay, 0.3) (#515567 30%), color p.text_muted (#838aa0)
- hover: bg p.bg_surface (#3d3f51), color p.text_secondary (#adb3c7)

## Отличия от original.md той же папки
1. .searchHint имеет свой `padding: 0 var(--space-2)` → фактический зазор
   иконка-текст в оригинале = gap 8 + 8 = 16px, справа от текста +8px;
   у нас только gap 8. Пилюля уже на ~16px.
2. transition var(--transition-fast) — нет (gpui hover мгновенный).
Остальное (h26, radius 8, fs 11, color-mix 60%/30%, hover) — 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_XS = 11 (`crates/shell/src/ui/titlebar.rs:373`); font-weight не задан; глиф codicon SEARCH 12.0 (`titlebar.rs:381`); подпись «Type a command…» тем же кеглем 11 в div с px SPACE_2 = 8 (`titlebar.rs:383`). Отклонение: у оригинала есть `.kbd` — font-mono 10px (`titlebar/Titlebar.module.css:110-117`), у нас kbd-чипа нет

### Вердикты

# 05 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет .searchHint padding 0 8 (зазор иконка-текст 8 vs 16; правый 12 vs 20).
Остальное (h26, px12, mr8, gap8, r-sm, fs-xs, bg 60%, border 30%, hover) — 1:1.

## Цикл 2: MATCH

## Цикл 8: DIVERGES

`codicon()` всегда даёт бокс 16×16, а в пилюле оригинала `<span class=codicon>` шириной advance = font-size = 12 → пилюля 163.20 против 157.60, зазор иконка→текст 19.20 против 16.80 (текст совпал). Нужен параметр размера бокса.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

`codicon()` больше не рисует жёсткий бокс 16×16: бокс = кегль (`icon.rs:45-56`), потому что `<i class=codicon>` — инлайн шириной в advance глифа, а у codicon-шрифта advance = 1em. Замер пилюли после правки: 158.4 лог. против 163.2 до (оригинал 157.6).

## Цикл 9: MATCH

Закрыто. `icon.rs:48-58`: бокс кодикона = кегль. Пилюля 159.20 против 157.60 у оригинала (было 163.20). Внутренние офсеты совпали дословно: ink иконки +13.60 у обоих, зазор иконка→текст 17.60 у обоих (ц.8 давал 19.20/16.80). Остаток 1.6 — advance глифов DirectWrite, не геометрия.

## Цикл 11: MATCH

пилюля 159.20; правый край + mr8 = 1106.40 = x триггера layout, стык дословный.

---

## 6. titlebar-window-controls-cluster — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:MATCH, ц11:MATCH*

![оригинал](06-titlebar-window-controls-cluster/original.png)
![наш](06-titlebar-window-controls-cluster/ours.png)

### Оригинал

# 06 titlebar-window-controls-cluster — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.tsx:54-85
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\Titlebar.module.css:119-125

## JSX-структура (кратко, вложенность)
```
<div class=controls>
  <TitlebarButton iconSet="fas"     icon="fa-bug"        variant="devtools" label="DevTools" />
  <TitlebarButton iconSet="codicon" icon="chrome-minimize"                  label="Minimize" />
  <TitlebarButton iconSet="codicon" icon={maximized ? "chrome-restore" : "chrome-maximize"}
                                    label={maximized ? "Restore" : "Maximize"} />
  <TitlebarButton iconSet="codicon" icon="chrome-close"  variant="close"    label="Close" />
</div>
```
(метрики кнопок — элемент 07)

## Метрики (ИЗ CSS)
.controls:
- размеры: height: 100%
- отступы: padding-right: var(--space-1); gap не задан (кнопки несут собственный margin 0 var(--space-1))
- скругления: нет
- шрифт: наследуется
- цвета: не заданы
- hover/active/focus: нет (на контейнере)
- transition: нет
- позиционирование: display:flex; align-items:center; -webkit-app-region: no-drag

## Состояния
- maximize-кнопка: иконка `chrome-maximize` ↔ `chrome-restore` по сигналу `isWindowMaximized` (label Maximize ↔ Restore). Стили кластера не меняются.

## Дополнение атрибутов (цикл 10)

- цвета: `.controls` своих background/color НЕ задаёт (`titlebar/Titlebar.module.css:119-125`) — прозрачный, наследует text-muted #838aa0 от `.titlebar`; цвета несут кнопки `TitlebarButton.module.css`: покой color var(--text-muted) #838aa0 (`TitlebarButton.module.css:10`), hover bg var(--bg-surface) #3d3f51 + color var(--text-primary) #cfd4e2 (`TitlebarButton.module.css:25-28`), `.close:hover` bg var(--accent-red) #f38ba8 + color var(--bg-primary) #313240 (`TitlebarButton.module.css:45-48`), `.devtools:hover` color var(--accent-primary) #89b4fa (`TitlebarButton.module.css:37-39`)

### Наша реализация

# 06 titlebar-window-controls-cluster — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:346-406 (кластер),
control_button 31-68, DevTools 353-377

## Структура (gpui-дерево кратко)
```
div (flex items-center h_full, pr SPACE_1)
 ├ div#devtools (fa-bug 13px + label "DevTools", radius 12)
 ├ control_button win-min  (CHROME_MINIMIZE \u{eaba})
 ├ control_button win-max  (CHROME_MAXIMIZE \u{eab9} ↔ CHROME_RESTORE \u{eabb}
 │                          по window.is_maximized(); tooltip Maximize↔Restore)
 └ control_button win-close (CHROME_CLOSE \u{eab8}, danger)
```
Кнопки: window_control_area(Min/Max/Close) + on_mouse_down →
minimize_window()/zoom_window()/remove_window().

## Метрики (из кода, точные)
- контейнер: h_full, pr m::SPACE_1 (4), gap нет (кнопки несут mx SPACE_1)
- кнопки — элемент 07
- цвета контейнера: не заданы

## Отличия от original.md той же папки
Структурно и метрически 1:1 (h100%, padding-right 4, порядок
DevTools→min→max→close, смена иконки maximize↔restore). Расхождения
внутри самих кнопок — см. 07-titlebar-button/ours.md.

## Дополнение атрибутов (цикл 10)

- ховер: у контейнера кластера ховера нет (`crates/shell/src/ui/titlebar.rs:419-423`); у детей: control_button bg bg_surface #3d3f51 + fg text_primary #cfd4e2 (`titlebar.rs:43,59`), close (danger) bg accent_red #f38ba8 + fg bg_primary #313240 (`titlebar.rs:41,59`), DevTools bg bg_surface #3d3f51 + fg accent_primary #89b4fa (`titlebar.rs:437`)

### Вердикты

# 06 — verdict (review cycle 1)
VERDICT: MATCH
.controls + порядок DevTools/Min/Max/Close — 1:1.

## Цикл 2: MATCH

## Цикл 8: MATCH

Кластер контролов: порядок и шаг 44.0 логических, close на 19.6 от правого края окна.

## Цикл 11: MATCH

центры min/max/close 1286/1330/1374 → шаг ровно 44.0; правый край close 20.0 от края окна.

---

## 7. titlebar-button — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:MATCH, ц11:MATCH*

![оригинал](07-titlebar-button/original.png)
![наш](07-titlebar-button/ours.png)

### Оригинал

# 07 titlebar-button — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarButton.tsx:30-35
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarButton.module.css

## JSX-структура (кратко, вложенность)
```
<button type=button class={btn [close|devtools]} data-tooltip={label} aria-label={label}>
  <i class={"codicon codicon-<name>" | "fas fa-<name>"} aria-hidden />
  {variant==="devtools" && <span class=devtoolsLabel>DevTools</span>}
</button>
```

## Метрики (ИЗ CSS)
.btn (default):
- размеры: width: var(--layout-icon-button-titlebar); height: var(--layout-icon-button-titlebar)
- отступы: margin: 0 var(--space-1); padding не задан
- скругления: border-radius: 50%
- шрифт: не задан на кнопке
- иконка `.btn > i`: inline-flex центр; width 16px; height 16px; font-size 13px; line-height 1
- цвета: color: var(--text-muted); background не задан (прозрачный)
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:inline-flex; align-items:center; justify-content:center; -webkit-app-region: no-drag

## Состояния
.devtools (variant="devtools"):
- width: auto; padding: 0 var(--space-3); gap: var(--space-1); border-radius: var(--radius-md)
- hover: color: var(--accent-primary) (фон — от базового .btn:hover: var(--bg-surface))
- .devtoolsLabel: font-size: var(--fs-sm)

.close (variant="close"):
- hover: background: var(--accent-red); color: var(--bg-primary)

### Наша реализация

# 07 titlebar-button — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:31-68 (control_button — default/close),
346-377 (devtools-вариант inline), crates/shell/src/ui/icon.rs:44-55 (codicon)

## Структура (gpui-дерево кратко)
```
control_button: div#id (occlude, window_control_area, круг)
 └ codicon(glyph, 14.0)      // бокс 16×16

devtools (inline в titlebar.rs):
 div#devtools ├ fa(FA_BUG, 13.0) └ div fs SM "DevTools"
```

## Метрики (из кода, точные)
default (.btn):
- w/h = m::ICON_BUTTON_TITLEBAR (36.0); mx m::SPACE_1 (4)
- rounded_full (50%)
- иконка: codicon в боксе 16×16, глиф 14px
- цвета: color p.text_muted (#838aa0), bg прозрачный
- hover: bg p.bg_surface (#3d3f51), color p.text_primary (#cfd4e2)

close (danger=true):
- hover: bg p.accent_red (#f38ba8), color p.bg_primary (#313240)

devtools:
- h 36, width auto, px m::SPACE_3 (12), mx m::SPACE_1 (4), gap m::SPACE_1 (4)
- rounded m::RADIUS_MD (12); label text_size m::FS_SM (12)
- color p.text_muted; hover: bg p.bg_surface + color p.accent_primary (#89b4fa)

## Отличия от original.md той же папки
1. Размер глифа default-кнопок: у нас 14px, оригинал `.btn > i` font-size 13px.
2. transition var(--transition-fast) — нет.
3. devtools-вариант у нас не переиспользует control_button (отдельная вёрстка),
   но метрики (padding 0 12, gap 4, radius 12, fs-sm label, hover accent) — 1:1.
Остальное (36×36, круг, margin 0 4, палитра hover, close-danger) — 1:1.

### Вердикты

# 07 — verdict (review cycle 1)
VERDICT: MATCH
.btn 36×36 r50% + devtools + close-hover red — 1:1 (глиф 13 подтверждён).

## Цикл 2: MATCH

## Цикл 8: MATCH

**Глиф 16 подтверждён пиксельно**: ink max/close 11.20 у обеих сторон, штрих min 1.60. Каскад оригинала объяснён: `.codicon[class*=codicon-]` (0,2,0) бьёт `.btn > i {13px}` (0,1,1). Остаток: центр на 1 физ. px ниже (растеризация).

## Цикл 11: MATCH

ink close 11.20 лог.

---

## 8. titlebar-quick-actions-row — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:MATCH, ц11:MATCH*

![оригинал](08-titlebar-quick-actions-row/original.png)
![наш](08-titlebar-quick-actions-row/ours.png)

### Оригинал

# 08 titlebar-quick-actions-row — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.tsx:27-51
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.module.css:1-7,35-40

## JSX-структура (кратко, вложенность)
```
<div class=row>
  <ActionBtn title={"Hide sidebar"|"Show sidebar"} active={sidebarVisible}>
    <PanelIcon slot="left" />          // элемент 17
  </ActionBtn>
  {sidebar скрыт && (
    <span class=divider aria-hidden />
    <ActionBtn title={"Close Customize"|"Customize"} active={customizeMode}>
      <i class="fas fa-gear" />
    </ActionBtn>
  )}
</div>
```
(ActionBtn — элемент 09)

## Метрики (ИЗ CSS)
.row:
- размеры: не заданы (по контенту)
- отступы: gap: 1px; padding: 0 var(--space-2)
- скругления: нет
- шрифт: наследуется
- цвета: нет
- hover/active/focus: нет (на контейнере)
- transition: нет
- позиционирование: display:inline-flex; align-items:center; -webkit-app-region: no-drag

.divider:
- размеры: width: 1px; height: 14px
- отступы: margin: 0 var(--space-1)
- цвета: background: var(--bg-surface)

## Состояния
- Шестерёнка + divider рендерятся ТОЛЬКО при скрытом сайдбаре (`!sidebarVisible`).
- Тумблер сайдбара: active = сайдбар видим. Шестерёнка: active = sidebarMode === "customize".

### Наша реализация

# 08 titlebar-quick-actions-row — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:198-244

## Структура (gpui-дерево кратко)
```
div (flex items-center, gap 1, px SPACE_2, flex_shrink_0)
 ├ action_button#toggle-sidebar (28×28 r8, active=sidebar_visible)
 │  └ svg("icons/panel-left.svg") 14×12, color text_secondary
 └ when !sidebar_visible:
    ├ divider div 1×14, mx SPACE_1, bg p.bg_surface
    └ action_button#customize-gear (28×28 r8) └ fa(FA_GEAR \u{f013}, 13.0)
```

## Метрики (из кода, точные)
- row: gap px(1.0), px m::SPACE_2 (8), размеры по контенту
- divider: w 1 × h 14, mx m::SPACE_1 (4), bg p.bg_surface (#3d3f51)
- кнопки — элемент 09; иконка сайдбар-тумблера svg 14×12; gear 13px

## Отличия от original.md той же папки
1. Gear: НЕ ФУНКЦИОНАЛЕН — on_click пустой `|_, _| {}` (оригинал открывает
   Customize), active всегда false (оригинал: active = sidebarMode==="customize").
2. Иконка тумблера — статичный файл panel-left.svg вместо PanelIcon slot="left"
   (визуально тот же 14×12 глиф, но не параметризован currentColor-вариантами).
3. Условие показа gear: у нас только `!sidebar_visible`; в оригинале то же —
   совпадает. Tooltip: у нас Hide/Show sidebar — 1:1; у gear нет варианта
   "Close Customize".
Метрики row (gap 1, padding 0 8, divider 1×14 margin 0 4 bg-surface) — 1:1.

## Дополнение атрибутов (цикл 10)

- скругления: N/A: скругления — у самой строки rounded не задан (`crates/shell/src/ui/titlebar.rs:217-222`); скругления только у детей: кнопки RADIUS_SM = 8 (`titlebar.rs:226,264`), divider 1×14 без скругления (`titlebar.rs:255-259`)
- ховер: у строки ховера нет; у кнопок внутри bg bg_surface #3d3f51 + text_primary #cfd4e2 (`titlebar.rs:86,108`); active-состояние (не ховер) — accent_primary #89b4fa при альфе 0.16 + text_primary (`titlebar.rs:114-115`)

### Вердикты

# 08 — verdict (review cycle 1)
VERDICT: MATCH
.row gap1 px8 + divider 1×14 + состояния — 1:1.

## Цикл 2: MATCH

## Цикл 8: MATCH

Ряд quick-actions: старт 50.0 (brand 42 + px8), gap 1; дивайдер 1×14 mx4 — по коду 1:1.

## Цикл 11: MATCH

старт 50.40 (brand 42.4 + px8), кнопка 28×28.

---

## 9. titlebar-quick-action-button — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:DIVERGES, ц9:MATCH, ц11:MATCH*

![оригинал](09-titlebar-quick-action-button/original.png)
![наш](09-titlebar-quick-action-button/ours.png)

### Оригинал

# 09 titlebar-quick-action-button — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.tsx:54-66
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.module.css:9-33

## JSX-структура (кратко, вложенность)
```
<button type=button class={btn [active]} data-tooltip={title} aria-label={title}
        aria-pressed={active ?? false}>
  {children}   // PanelIcon slot="left" ЛИБО <i class="fas fa-gear">
</button>
```

## Метрики (ИЗ CSS)
.btn:
- размеры: width: var(--layout-icon-button-round); height: var(--layout-icon-button-round)
- отступы: не заданы
- скругления: border-radius: var(--radius-sm)
- шрифт: не задан; `.btn :global(.codicon) { font-size: 14px !important; }`
- цвета: color: var(--text-secondary); background: transparent
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:inline-flex; align-items:center; justify-content:center

## Состояния
.active:
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)
(при hover активной кнопки: `.btn:hover` специфичнее (0,2,0 против 0,1,0 у `.active`) → фон на hover = var(--bg-surface))

### Наша реализация

# 09 titlebar-quick-action-button — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:70-105 (fn action_button)

## Структура (gpui-дерево кратко)
```
div#id (occlude, size×size, flex center, rounded(radius), cursor_pointer)
 └ child (svg | fa | codicon)
```
Универсальная: quick-action/theme = 28×28 r8; layout-toggles = 26×26 r12
(вызовы передают size/radius per-кнопка).

## Метрики (из кода, точные)
- w/h = m::ICON_BUTTON_ROUND (28.0) для quick-actions; rounded m::RADIUS_SM (8)
- цвета: color p.text_secondary (#adb3c7), bg прозрачный
- hover: bg p.bg_surface (#3d3f51), color p.text_primary (#cfd4e2)
- active: bg tint(p.accent_primary, 0.16), color p.text_primary
  (hover-стиль при наведении перебивает — как в оригинале, где .btn:hover
  специфичнее .active)

## Отличия от original.md той же папки
1. `.btn :global(.codicon) { font-size: 14px }` — у нас размер глифа задаёт
   вызывающий (gear 13px, theme 12px); для codicon-детей внутри quick-action
   фикс-14 не воспроизводится.
2. transition var(--transition-fast) — нет.
3. aria-pressed — не применимо.
Метрики (28×28, radius 8, палитра base/hover/active 16% accent) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ ни по одной оси (`crates/shell/src/ui/titlebar.rs:87-111`) — центровка глифа через flex + items_center + justify_center (`titlebar.rs:96-99`); бокс w/h = ICON_BUTTON_ROUND = 28 (`crates/metrics/src/lib.rs:25`, вызов `titlebar.rs:225-226`); собственных margin у кнопки нет — внешний зазор даёт gap 1 строки (`titlebar.rs:220`) и mx SPACE_1 = 4 у divider (`titlebar.rs:258`). Совпадает с оригиналом: `.btn` тоже без padding, 28×28 (`titlebar/TitlebarQuickActions.module.css:9-19`, `--layout-icon-button-round: 28px` в `theme/layout-tokens.css:60`)

### Вердикты

# 09 — verdict (review cycle 1)
VERDICT: DIVERGES
Базовый цвет text-muted, должен text-secondary; svg тумблера жёстко text-secondary
(hover не поднимает до primary); fa-gear 13 vs 12.

## Цикл 2: MATCH

## Цикл 8: DIVERGES

`.active { color: text-primary }` съедается жёстким `text_secondary` на самом `svg()` (`titlebar.rs:234`): у оригинала иконка panel-left (208,212,225), у нас (173,179,199). Единственное расхождение ЦВЕТА в зоне.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Цвет svg тумблера теперь следует active-состоянию, как `currentColor` в `TitlebarQuickActions.module.css` (`.btn` secondary → `.active` primary): `titlebar.rs:236-244`. Замер ink после правки (207,212,226) против оригинала (208,212,225) — разница в пределах сглаживания. Подъём цвета по ХОВЕРУ остаётся недостижим (svg в gpui не реагирует).

## Цикл 9: MATCH

Закрыто. `titlebar.rs:246-250`: цвет svg следует active. Самый яркий ink тумблера (207,212,226) = `--text-primary #cfd4e2` (`dark-theme.css:34`) точно в токен; было (173,179,199) = secondary. Подъём цвета по ХОВЕРУ недостижим — ограничение gpui.

## Цикл 11: MATCH

svg активного тумблера (207,212,226) = --text-primary.

---

## 10. layout-toggles-trigger — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:MATCH, ц11:MATCH*

![оригинал](10-layout-toggles-trigger/original.png)
![наш](10-layout-toggles-trigger/ours.png)

### Оригинал

# 10 layout-toggles-trigger — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.tsx:165-180
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.module.css:1-33

## JSX-структура (кратко, вложенность)
```
<div class=anchor>                         // relative-обёртка для outside-click
  <button type=button class=trigger aria-haspopup=menu aria-expanded={open}
          aria-label="Layout panels" data-tooltip="Layout panels">
    <i class="fas fa-table-columns" aria-hidden />
  </button>
  {open && портал-меню в <body>}           // элемент 11
</div>
```

## Метрики (ИЗ CSS)
.anchor:
- position: relative; -webkit-app-region: no-drag

.trigger:
- размеры: width: 26px; height: 26px
- отступы: padding: 0
- скругления: border-radius: var(--radius-md)
- шрифт: `.trigger > i { font-size: 13px; line-height: 1; }`
- цвета: color: var(--text-secondary); background: transparent; border: none
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:grid; place-items:center; cursor:pointer

## Состояния
`.trigger[aria-expanded="true"]` (popover открыт):
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)

### Наша реализация

# 10 layout-toggles-trigger — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:316-330 (вызов action_button
"layout-toggles"), 70-105 (action_button)

## Структура (gpui-дерево кратко)
```
action_button#layout-toggles (26×26, r12)
 └ fa(FA_TABLE_COLUMNS \u{f0db}, 13.0)
```
Клик → ShellEvent::ToggleLayoutPopover (поповер в overlay-окне, элемент 11).
Tooltip "Layout panels".

## Метрики (из кода, точные)
- w/h px(26.0); rounded m::RADIUS_MD (12)
- глиф fa-table-columns 13px (бокс 16×16)
- цвета: color p.text_secondary (#adb3c7); hover bg p.bg_surface + text_primary

## Отличия от original.md той же папки
1. Open-состояние НЕ РЕАЛИЗОВАНО: `active` захардкожен false — оригинал при
   `aria-expanded="true"` красит триггер в accent-primary 16% + text-primary.
2. .anchor-обёртка не нужна (поповер позиционируется от вьюпорта в overlay).
3. transition — нет.
Метрики триггера (26×26, radius 12, глиф 13px, base/hover цвета) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ (`crates/shell/src/ui/titlebar.rs:87-111`), бокс 26×26 задан явно вызовом (`titlebar.rs:387`); margin нет — стоит прямым ребёнком корня титлбара между search-пилюлей (её mr SPACE_2 = 8, `titlebar.rs:367`) и theme-кнопкой
- шрифты: собственного текста нет; глиф `fa(FA_TABLE_COLUMNS, 13.0)` — кегль 13 (`titlebar.rs:399`); text_size у кнопки не задан, наследует FS_SM = 12 корня (`titlebar.rs:197`). Оригинал: `.trigger > i { font-size: 13px }` (`titlebar/LayoutToggles.module.css:23`) — совпадает

### Вердикты

# 10 — verdict (review cycle 1)
VERDICT: DIVERGES
Базовый цвет text-muted vs text-secondary. Остальное (26×26 r12, active accent16%) — 1:1.

## Цикл 2: MATCH

## Цикл 8: MATCH

Триггер layout-toggles: 26×26 r12, база text-secondary, глиф fa 13 — совпало, форма глифа сверена при 12× зуме.

## Цикл 11: MATCH

триггер 25.6×26.4 r12, fa ink = --text-secondary, глиф центрирован (1119.6 против 1119.2).

---

## 11. layout-toggles-menu — **DIVERGES** (цикл 11)

*История: ц2:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц11:DIVERGES*

![оригинал](11-layout-toggles-menu/original.png)
![наш](11-layout-toggles-menu/ours.png)

### Оригинал

# 11 layout-toggles-menu — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.tsx:117-163
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.module.css:38-133

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<ul class=menu role=menu style={left,top,visibility}>   // fixed, clampToViewport, offset 6px (POPUP_OFFSET_PX)
  <li class=menuLabel>Layout</li>
  ×6 <li><button role=menuitemcheckbox aria-checked aria-disabled disabled class=menuItem>
       <span class="check [checkOn]">{on && <i class="codicon codicon-check">}</span>
       <span class=itemIcon><PanelIcon slot=… /></span>
       <span class=itemLabel>Left|Left Bottom|File|Center Bottom|Right|Right Bottom</span>
       {disabled && hint && <span class=itemHint>Requires …</span>}
     </button></li>
  <li class=divider role=separator />
  <LayoutPresetsSection />                               // элемент 12
</ul>
```
Клик по item НЕ закрывает меню; закрытие — outside-click / Esc.

## Метрики (ИЗ CSS)
.menu:
- размеры: min-width: 220px; max-height: calc(100vh - 16px); overflow-y: auto
- отступы: padding: var(--space-1); margin: 0; gap: 1px (flex-column)
- скругления: border-radius: var(--radius-md)
- цвета: background: var(--bg-surface); border: 1px solid var(--divider-soft)
- тень: box-shadow: var(--shadow-dropdown)
- позиционирование: position: fixed; z-index: var(--z-dropdown); display:flex; flex-direction:column; list-style:none

.menuLabel:
- padding: var(--space-1) var(--space-3); font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted)

.menuItem:
- width: 100%; padding: var(--space-2) var(--space-3); gap: var(--space-2)
- border-radius: var(--radius-sm); background: transparent; border: none
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer
- display:flex; align-items:center
- hover (`:hover:not([disabled])`): background: color-mix(in srgb, var(--text-primary) 10%, transparent)

.check:
- width: 16px; height: 16px; border-radius: 3px; border: 1px solid var(--bg-overlay); flex-shrink: 0
- inline-flex центр; `.check .codicon { font-size: 12px; line-height: 1; }`

.checkOn:
- background: var(--accent-primary); border-color: var(--accent-primary); color: var(--accent-action-fg)

.itemIcon: inline-flex центр; color: var(--text-muted); flex-shrink: 0
.itemLabel: flex: 1
.itemHint: font-size: var(--fs-xs); color: var(--text-disabled)
.divider: height: 1px; margin: var(--space-1) var(--space-2); background: var(--divider-soft)

## Состояния
- `[disabled]` (child-строка при скрытом родителе): cursor: not-allowed; color: var(--text-muted); `.itemIcon { opacity: 0.4 }`; hover-фон не применяется; aria-checked=false принудительно (effectiveOn = isOn && !disabled)
- checked (`checkOn`): см. выше; рендерится codicon-check
- позиция: side "bottom" от анкора, offset 6px; visibility:hidden до первого замера

### Наша реализация

# 11 layout-toggles-menu — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:28-46 (popover_frame),
48-139 (toggle_row/menu_label), 141-225 (layout_popover);
иконки слотов: crates/shell/src/ui/panel_placeholder.rs:78-80 (slot_glyph_small)

## Структура (gpui-дерево кратко)
```
popover_frame#layout-popover (absolute в OVERLAY-окне, top 46, w 220)
 ├ hit_area()
 ├ menu_label "LAYOUT"
 ├ ×6 toggle_row (Left / Left Bottom / File / Center Bottom / Right / Right Bottom)
 │   ├ checkbox 16×16 r3 (on: accent bg + codicon-check 12px accent_action_fg)
 │   ├ slot_glyph_small (PanelIcon-мини, text_muted; disabled → opacity 0.4)
 │   ├ label flex_1
 │   └ disabled: hint fs XS "Requires X"
 ├ divider 1px
 └ presets_section (элемент 12)
```
Клик по строке НЕ закрывает поповер (stop_propagation). Дети без родителя —
disabled (effective_on = on && !disabled).

## Метрики (из кода, точные)
- фрейм: top px(TITLEBAR_HEIGHT + 4.0)=46, left = vw − right(210) − 220,
  w px(POP_W=220.0), p m::SPACE_1 (4), rounded m::RADIUS_MD (12)
- цвета фрейма: bg p.bg_surface (#3d3f51), border 1px tint(text_primary, 0.06),
  shadow dropdown_shadow()
- menu_label: px SPACE_3 (12) / py SPACE_1 (4), fs m::FS_XS (11), text_muted, uppercase
- toggle_row: gap SPACE_2 (8), px SPACE_3 (12), py SPACE_2 (8), rounded SPACE_SM→RADIUS_SM (8),
  fs m::FS_SM (12), color text_primary (disabled → text_muted)
- hover: tint(text_primary, 0.10)
- check: 16×16, rounded 3, border 1px p.bg_overlay (#515567);
  on: bg/border p.accent_primary (#89b4fa), галка accent_action_fg (#313240)
- hint: fs XS, text_muted, opacity 0.7
- divider: h 1, mx SPACE_2 (8), my SPACE_1 (4), bg tint(text_primary, 0.06)

## Отличия от original.md той же папки
1. Ширина фикс 220px; оригинал min-width 220 + рост по контенту.
2. max-height calc(100vh−16px) + overflow-y:auto — НЕТ (длинный список
   пресетов не скроллится).
3. gap 1px между пунктами (flex-column) — НЕТ на фрейме (есть только внутри
   presets_section).
4. .menuLabel letter-spacing 0.04em — нет.
5. .itemHint цвет: у нас text_muted + opacity 0.7; оригинал var(--text-disabled) (#60667b).
6. Позиционирование: фикс top 46 от вьюпорта (оригинал: anchor-bottom + 6px
   offset ≈ 40, clampToViewport); у нас на 6px ниже.
7. border/divider: tint(text_primary,0.06) вместо var(--divider-soft) —
   численно совпадает по дизайн-решению, сверить токен.
8. disabled: cursor not-allowed — нет (просто нет cursor_pointer).

## Дополнение атрибутов (цикл 10)

- шрифты: `menu_label` FS_XS = 11 + `to_uppercase()` (`crates/shell/src/ui/layout_popover.rs:163-166`; letter-spacing 0.04em оригинала в gpui недоступен); строка тумблера text_size FS_SM = 12 (`layout_popover.rs:122`); hint отключённой строки FS_XS = 11 (`layout_popover.rs:142`); галка чекбокса codicon 12 (`layout_popover.rs:108`); `menu_item` секции Layouts — FS_SM = 12 (`layout_popover.rs:453`) + codicon 16 (`layout_popover.rs:463`); font-weight нигде в меню не задаётся (нормальный)

### Вердикты

# 11 — verdict (review cycle 1)
VERDICT: DIVERGES
gap строк 0 vs 1; фикс w220 vs min-width; нет max-h vh-16+scroll; позиция top46/left
vw-430 vs anchor.bottom+6/anchor.left; shadow 0/8/24/.45 vs dropdown 0/4/16/.5;
itemHint muted+op.7 vs text-disabled; нет ls .04em (gpui). Рецепты пунктов/чеков — 1:1.

## Цикл 2: DIVERGES
Нет max-h vh-16+scroll; X = оценка right84 vs anchor.left (~vw-295 → right 75).

## Цикл 8: DIVERGES

Поповер layout: оригинал центрируется по анкору (left ≈ 1658), у нас правый анкор 75 → left ≈ 1753; `left` считается от фиксированных `POP_W = 220` вместо min-width; нет `overflow-y: auto`. Пиксельно не проверено — нет кадра с открытым поповером.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Анкор перестал быть зашитым офсетом 75: `anchor_below()` считает `left = a.left + a.width/2 − pop_w/2`, `top = a.bottom + 6`, оба клампа гаттером 8 — арифметика `clampToViewport(side:"bottom")`. Bounds триггера берутся из probe-реестра (`action_button` теперь `relative()` + `probe_area(id)`), замер: layout-toggles x=1106.4 w=25.6. ОСТАЛОСЬ: ширина для центровки — константа `POP_W=220` вместо измеренной, и нет `overflow-y:auto` (у нас `overflow_hidden`).

## Цикл 9: DIVERGES

Анкор ЗАКРЫТ: `anchor_below` (`layout_popover.rs:33-40`) = `left = a.left + a.width/2 − p.width/2`, `top = a.bottom + 6`, кламп гаттером 8 — совпадает с `clamp-popup.ts:100-110` + offset 6 (`LayoutToggles.tsx:39`). ОСТАЛОСЬ: ширина для центровки — константа `POP_W = 220` вместо измеренной `m.width` (при пресете шире 220 центр уедет); `.overflow_hidden()` (`:61`) вместо `overflow-y:auto` (`LayoutToggles.module.css:53`). Плюс `anchor_below` вызывается с `pop_h = 0.0` → ветка flip'а на сторону top недостижима.

## Цикл 11: DIVERGES

POP_W 220 (живьём поповер вышел ровно 220.0 — дефект латентный); .overflow_hidden() вместо overflow-y:auto; anchor_below вызывается с pop_h=0 → ветка flip недостижима. Позиция верна: left 1009.6 против расчётных 1009.2.

---

## 12. layout-presets-section — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:DIVERGES, ц9:MATCH, ц11:MATCH*

![оригинал](12-layout-presets-section/original.png)
![наш](12-layout-presets-section/ours.png)

### Оригинал

# 12 layout-presets-section — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutPresetsSection.tsx:98-167
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.module.css:56-62,64-89,113-126,135-192 (общий css с меню)

## JSX-структура (кратко, вложенность)
```
<>                                        // внутри <ul class=menu> элемента 11
  <li class=menuLabel>Layouts</li>
  <li><button class=menuItem> codicon-save             + "Save current layout…"</button></li>
  <li><button class=menuItem> codicon-desktop-download + "Export current layout…"</button></li>
  <li><button class=menuItem> codicon-cloud-upload     + "Import layout…"</button></li>
  {presets.length===0 && <li class=presetEmpty>No saved layouts yet</li>}
  ×N <li class=presetRow onContextMenu=rename>
       <button class=presetApply data-tooltip="Apply this layout · right-click to rename">
         <span class=itemIcon><i class="codicon codicon-layout"></span>
         <span class=itemLabel>{name}</span>
       </button>
       <button class=presetIconBtn> codicon-save-as </button>            // overwrite
       <button class=presetIconBtn> codicon-desktop-download </button>   // export
       <button class=presetIconBtn aria-pressed={default}> codicon-star-full|star-empty </button>
       <button class=presetIconBtn> codicon-trash </button>
     </li>
</>
```

## Метрики (ИЗ CSS)
.menuLabel, .menuItem, .itemIcon, .itemLabel — как в элементе 11 (тот же css).

.presetEmpty:
- padding: var(--space-1) var(--space-3); font-size: var(--fs-xs); color: var(--text-muted)

.presetRow:
- display:flex; align-items:center; gap: 1px

.presetApply:
- flex: 1; min-width: 0; padding: var(--space-2) var(--space-3); gap: var(--space-2)
- background: transparent; border: none; border-radius: var(--radius-sm)
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer
- display:flex; align-items:center
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent)
- `.presetApply .itemLabel`: overflow:hidden; text-overflow:ellipsis; white-space:nowrap

.presetIconBtn:
- размеры: width: 26px; height: 26px; flex-shrink: 0
- display:grid; place-items:center; background: transparent; border: none
- border-radius: var(--radius-sm); color: var(--text-muted); cursor: pointer
- `> i { font-size: 13px; line-height: 1; }`
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)

## Состояния
- `.presetIconBtn[aria-pressed="true"]` (star = default-пресет): color: var(--accent-primary); иконка codicon-star-full (иначе star-empty)
- пустой список: строка .presetEmpty
- right-click по .presetRow → rename-prompt (стилей не меняет)

### Наша реализация

# 12 layout-presets-section — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:227-399 (presets_section),
401-431 (menu_item); данные — crate::layout_store::load_presets()

## Структура (gpui-дерево кратко)
```
div (flex col, gap 1)
 ├ menu_label "LAYOUTS"
 ├ menu_item codicon-save \u{eb4b} "Save current layout…"
 ├ menu_item codicon-desktop-download \u{ea78} "Export current layout…"
 ├ menu_item codicon-cloud-upload \u{eac3} "Import layout…"
 ├ presets.is_empty(): div "No saved layouts yet"
 └ ×N presetRow (flex, gap 1)
     ├ apply-кнопка flex_1 (codicon \u{ebeb} layout 14px muted + имя ellipsis;
     │   ЛКМ apply, ПКМ rename-prompt)
     ├ icon_btn save-as \u{eb4a} (overwrite) · desktop-download \u{ea78} (export)
     ├ icon_btn star \u{eb59} full / \u{ea6a} empty (default toggle)
     └ icon_btn trash \u{ea81} (delete)
```

## Метрики (из кода, точные)
- menu_item / apply: gap SPACE_2 (8), px SPACE_3 (12), py SPACE_2 (8),
  rounded RADIUS_SM (8), fs FS_SM (12), color text_primary,
  hover tint(text_primary, 0.10); иконка codicon 14px text_muted
- presetEmpty: px SPACE_3, py SPACE_1, fs FS_XS (11), text_muted
- presetRow: gap px(1.0)
- icon_btn: 26×26, rounded RADIUS_SM (8), глиф codicon 13px, color text_muted
  (star-default → accent_primary), hover tint(text_primary, 0.10) + text_primary

## Отличия от original.md той же папки
1. Иконка в menu_item/apply — 14px; оригинал не форсит размер (наследует
   fs-sm 12px у codicon в тексте). +2px.
2. Save/Export/Import у оригинала — .menuItem БЕЗ ведущей иконки-чекбокса,
   с codicon — совпадает; но у нас нет letter-spacing у label секции (см. 11).
3. star-глифы: full \u{eb59} / empty \u{ea6a} — соответствуют
   codicon-star-full/star-empty; aria-pressed → цвет accent — 1:1.
4. transition — нет.
Метрики (26×26 icon-btn r8 глиф 13, padding 8/12, hover 10%, gap 1) — 1:1.

## Дополнение атрибутов (цикл 10)

- цвета: заголовок «LAYOUTS» text_muted #838aa0 (`crates/shell/src/ui/layout_popover.rs:165`, вызов `:269`); пункты Save/Export/Import — text_primary #cfd4e2, иконка text_muted #838aa0, hover bg = text_primary при альфе 0.10 (`layout_popover.rs:444,454,456,463`); пустое состояние «No saved layouts yet» text_muted #838aa0 (`layout_popover.rs:300`); строка пресета text_primary #cfd4e2 + hover bg text_primary@0.10 (`layout_popover.rs:356,359-360`), иконка папки text_muted #838aa0 (`layout_popover.rs:382`); `presetIconBtn` — покой text_muted #838aa0, активная (star=default) accent_primary #89b4fa, hover bg text_primary@0.10 + fg text_primary #cfd4e2 (`layout_popover.rs:314,324-328,330`); divider над секцией bg = text_primary@0.06 (`layout_popover.rs:252`); собственного фона у секции нет — bg_surface #3d3f51 поповера (`layout_popover.rs:70`)

### Вердикты

# 12 — verdict (review cycle 1)
VERDICT: DIVERGES
codicon в menuItem/presetApply 14 vs 12 (наследование). Остальное — 1:1.

## Цикл 2: MATCH

## Цикл 8: DIVERGES

Секция пресетов: у оригинала иконки 16 (каскад codicon), у нас 13/12/12; плюс бокс 16×16 при шрифте 12 добавляет 4px к слоту.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Иконки `.itemIcon` подняты 12 → 16: у `.itemIcon` своего `font-size` нет, каскад отдаёт базовый `.codicon{font-size:16px}` (`skeleton.css:2`). `layout_popover.rs` — обе точки (строка пресета codicon-layout и строка меню). Плюс исчезли лишние 4px слота — бокс кодикона больше не 16 при кегле 12 (см. 05).

## Цикл 9: MATCH

Закрыто. Каскад оригинала: `.itemIcon` кегля не задаёт → базовый `.codicon{16px}` (`skeleton.css:1-5`); `.presetIconBtn > i {13px}`; `.check .codicon {12px}`. У нас 16/16/13/12 против 16/16/13/12 (`layout_popover.rs:459`, `:378`, `:332`, `:104`). Лишние 4px слота ушли вместе с фиксом 05.

## Цикл 11: MATCH

ink иконки пункта 14.4×14.4 (кегль 16), зазор иконка→текст 8.8.

---

## 13. theme-quick-toggle-trigger — **MATCH** (цикл 11)

*История: ц2:DIVERGES, ц8:MATCH, ц11:MATCH*

![оригинал](13-theme-quick-toggle-trigger/original.png)
![наш](13-theme-quick-toggle-trigger/ours.png)

### Оригинал

# 13 theme-quick-toggle-trigger — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:51-66
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:1-25

## JSX-структура (кратко, вложенность)
```
<div class=root ref>                       // relative-обёртка, outside-click / blur / Esc
  <button type=button class=trigger data-tooltip="Appearance — themes & icons"
          aria-label="Appearance — themes & icons" aria-haspopup=dialog aria-expanded={open}>
    <i class="fas {fa-circle-half-stroke | fa-sun | fa-moon}" aria-hidden />
  </button>
  {open && <Menu />}                       // элемент 14
</div>
```
Логика иконки: contributed-тема light → fa-sun, dark → fa-moon; без contributed:
choice "system" → fa-circle-half-stroke, иначе по resolvedTheme (light → fa-sun, dark → fa-moon).

## Метрики (ИЗ CSS)
.root:
- position: relative; display:inline-flex; align-items:center; -webkit-app-region: no-drag

.trigger:
- размеры: width: 28px; height: 28px
- отступы: не заданы
- скругления: border-radius: var(--radius-sm)
- шрифт: `.trigger > i { font-size: 12px; line-height: 1; }`
- цвета: background: transparent; color: var(--text-muted)
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:inline-flex; align-items:center; justify-content:center

## Состояния
Вариантных классов нет; aria-expanded меняется, но css-правила на него в этом модуле нет.

### Наша реализация

# 13 theme-quick-toggle-trigger — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:331-345 (вызов action_button
"theme-toggle"), 70-105 (action_button); глиф — TitlebarState.theme_glyph

## Структура (gpui-дерево кратко)
```
action_button#theme-toggle (28×28, r8)
 └ fa(state.theme_glyph, 12.0)   // moon \u{f186} / sun \u{f185} / half \u{f042}
```
Клик → ShellEvent::ToggleAppearancePopover. Tooltip
"Appearance — themes & icons". Логика глифа (dark→moon, light→sun,
system/contributed→half) — снаружи, в state.

## Метрики (из кода, точные)
- w/h = m::ICON_BUTTON_ROUND (28.0); rounded m::RADIUS_SM (8)
- глиф fa 12px в боксе 16×16
- цвета: color p.text_secondary (#adb3c7) — БАЗА action_button
- hover: bg p.bg_surface, color p.text_primary

## Отличия от original.md той же папки
1. Базовый цвет: у нас text_secondary (#adb3c7) через общий action_button;
   оригинал .trigger — var(--text-muted) (#838aa0). Иконка светлее оригинала.
2. transition — нет.
3. .root-обёртка (relative, outside-click) не нужна — поповер в overlay.
Метрики (28×28, radius 8, глиф 12px, hover) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ (`crates/shell/src/ui/titlebar.rs:87-111`), бокс 28×28 = ICON_BUTTON_ROUND (`titlebar.rs:403`, `crates/metrics/src/lib.rs:25`); margin нет. Совпадает с оригиналом `.trigger` 28×28 без padding (`titlebar/ThemeQuickToggle.module.css:8-18`)
- шрифты: своего текста нет; глиф `fa(state.theme_glyph, 12.0)` — кегль 12 (`titlebar.rs:415`); text_size не задан, наследует FS_SM = 12 корня (`titlebar.rs:197`). Оригинал: `.trigger > i { font-size: 12px }` (`ThemeQuickToggle.module.css:20`) — совпадает

### Вердикты

# 13 — verdict (review cycle 1)
VERDICT: DIVERGES
Глиф: contributed должна давать sun/moon по её uiTheme (half-stroke ТОЛЬКО при
system без contributed) — у нас half-stroke при любой contributed (root.rs:5259-5266).
Метрики триггера — 1:1 (text_muted фикс подтверждён).

## Цикл 2: DIVERGES
Регресс: триггер темы должен быть text-muted (CSS .trigger), а action_button теперь красит все в secondary. Глиф-логика верна.

## Цикл 8: MATCH

Триггер темы: регресс цикла 2 закрыт — цвет вернулся к text-muted (131,138,160); 28×28 r8, глиф 12, логика sun/moon/half-stroke 1:1.

## Цикл 11: MATCH

28×28, moon ink = --text-muted.

---

## 14. theme-popover — **DIVERGES** (цикл 11)

*История: ц2:MATCH, ц8:DIVERGES, ц9:DIVERGES, ц11:DIVERGES, ц11:DIVERGES, ц11:DIVERGES*

![оригинал](14-theme-popover/original.png)
![наш](14-theme-popover/ours.png)

### Оригинал

# 14 theme-popover — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:82-118
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:32-89

## JSX-структура (кратко, вложенность)
```
<div class=menu role=dialog aria-label="Appearance">   // НЕ портал: absolute внутри .root
  <div class=header>
    <span class=title>Appearance</span>
    <button class="sysToggle [sysOn]" aria-pressed data-tooltip="Follow the OS light/dark setting">
      <i class="fas fa-circle-half-stroke" /><span>System</span>
    </button>
  </div>
  <div class=columns>
    <Column title="Dark">…</Column>       // элементы 15/16
    <Column title="Light">…</Column>
    <Column title="Icons">…</Column>
  </div>
</div>
```
Пики НЕ закрывают popover; закрытие — outside-click / Esc / window blur.

## Метрики (ИЗ CSS)
.menu:
- размеры: width: max-content
- отступы: padding: var(--space-2); margin: 0; gap: var(--space-2) (flex-column)
- скругления: border-radius: var(--radius-md)
- цвета: background: var(--bg-surface); border: 1px solid var(--divider-soft)
- тень: box-shadow: var(--shadow-dropdown)
- позиционирование: position: absolute; top: calc(100% + 4px); right: 0; z-index: var(--z-overlay); display:flex; flex-direction:column

.header:
- display:flex; align-items:center; justify-content:space-between; gap: var(--space-3); padding: 0 var(--space-1)

.title:
- font-size: var(--fs-sm); font-weight: 600; color: var(--text-primary)

.sysToggle:
- padding: var(--space-1) var(--space-2); gap: var(--space-2)
- background: transparent; color: var(--text-muted); border-radius: var(--radius-sm)
- font-size: var(--fs-xs); white-space: nowrap; display:inline-flex; align-items:center
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent); color: var(--text-primary)

.columns:
- display: grid; grid-template-columns: repeat(3, minmax(140px, 1fr)); gap: var(--space-2)

## Состояния
.sysOn (+ .sysOn:hover) — System активен:
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)

### Наша реализация

# 14 theme-popover — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:433-670 (appearance_popover);
рендер — overlay-слой

## Структура (gpui-дерево кратко)
```
div#appearance-popover (absolute в OVERLAY: top 46, right 8; ширина по контенту)
 ├ hit_area()
 ├ header (flex, px SPACE_1, pb SPACE_2)
 │  ├ title "Appearance" flex_1 (fs SM, SEMIBOLD, text_primary)
 │  └ #ap-system тумблер (fa-circle-half-stroke \u{f042} 11px + "System")
 └ columns row (flex, gap SPACE_2)
    ├ column "Dark"  (Kamin Dark + contributed dark)
    ├ column "Light" (Kamin Light + contributed light)
    └ column "Icons" (Catppuccin + contributed icon-темы)
```
Пики НЕ закрывают поповер (stop_propagation).

## Метрики (из кода, точные)
- фрейм: top px(TITLEBAR_HEIGHT+4)=46, right px(8), p SPACE_2 (8),
  gap SPACE_2 (8), rounded RADIUS_MD (12), bg p.bg_surface,
  border 1px tint(text_primary, 0.06), shadow dropdown_shadow()
- header: px SPACE_1 (4), pb SPACE_2 (8)
- title: fs FS_SM (12), FontWeight::SEMIBOLD, text_primary
- sysToggle: px SPACE_2 (8), py SPACE_1 (4), gap SPACE_2 (8), rounded RADIUS_SM (8),
  fs FS_XS (11); off: bg tint(text_primary, 0.06), color text_secondary;
  on (sysOn): bg tint(accent_primary, 0.16), color text_primary;
  hover: bg tint(accent_primary, 0.22)

## Отличия от original.md той же папки
1. sysToggle off-состояние: у нас bg tint(text_primary, 0.06); оригинал —
   transparent. Плюс цвет off: text_secondary vs var(--text-muted).
2. sysToggle hover: у нас accent_primary 22%; оригинал text_primary 10% +
   color text-primary.
3. columns: flex row gap 8 (колонки min-width 140) вместо grid
   `repeat(3, minmax(140px, 1fr))` — колонки не равноширинные, каждая по
   контенту (сознательно: фикс-ширина резала имена тем).
4. Позиция: right 8 от вьюпорта overlay; оригинал right:0 от .root триггера
   (совпадает с точностью до пары px), top 46 vs anchor+4 (~76 от верха окна
   у оригинала top: calc(100%+4) от триггера ≈ 39) — у нас поповер выше/ниже
   на несколько px, сверить скринами.
5. header gap var(--space-3) между title и toggle — у нас flex_1 у title
   (эквивалент по раскладке).
Фрейм (padding 8, gap 8, radius 12, bg-surface, border 6%, shadow) — 1:1.

### Вердикты

# 14 — verdict (review cycle 1)
VERDICT: DIVERGES
Анкор right 8 vs правый край триггера (~250); top 46 vs ~39; лишний pb8 у header
(зазор 16 vs 8); колонки не 1fr; shadow 0/8/24/.45 vs 0/4/16/.5.
sysToggle-рецепт подтверждён 1:1.

## Цикл 2: MATCH

## Цикл 8: DIVERGES

Поповер appearance: `.menu { right: 0 }` от `.root` → правый край триггера 239 от края окна, у нас `right(250)` (11 логических влево); колонки без 1fr-равнения.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Правый край поповера привязан к правому краю ТРИГГЕРА (`.menu{top:calc(100% + 4px); right:0}` от `.root`): `right = vw − (trigger.x + trigger.w)`, `top = trigger.y + trigger.h + 4` по bounds из probe-реестра. Замер: theme-toggle right=1160 при vw=1400 → офсет 240 (был зашит 250, то есть 10 мимо). ОСТАЛОСЬ: колонки без 1fr-равнения (нужен замер текста).

## Цикл 9: DIVERGES

Позиция ЗАКРЫТА: `right = vw − (trigger.x + trigger.w)`, `top = trigger.y + trigger.h + 4` по probe-bounds = `.menu{top:calc(100% + 4px); right:0}` от `.root` (`ThemeQuickToggle.module.css:32-35`); зашитый 250 убран. Правый кластер живьём совпал целиком, расхождение ≤1.6 (layout 1761.60 против 1762.40, moon 1788.80/1789.60, min/max/close 1929.60/1973.60/2016.80 против 1928.80/1972.80/2016.80). Тень `shadows::dropdown()` = 0 4 16 α.5 = `--shadow-dropdown` ✓. ОСТАЛОСЬ: колонки без 1fr (см. 15).

## Цикл 11: DIVERGES

★ РЕГРЕССИЯ НАЙДЕНА И ИСПРАВЛЕНА: right-анкор резолвился к вьюпорту OVERLAY-окна (шире main) — поповер уезжал за правый край, левый край 1316.8 при окне 1400, видна одна колонка. Тем же объясняется пропажа тоста. Правка: left-анкор `trigger_right − THEME_POP_W` + явная ширина 444 = 3×140 + 2×4 + 2×8. Требует пересверки кадром.

## Цикл 11: DIVERGES

Ширина пересчитана: `THEME_POP_W = 140×3 + space-2×2 (гэпы) + space-2×2 (падинг) + 2 (рамка)` = **454** (было 444 — гэпы считались по 4, трём колонкам не хватало 8px).

Осталось: кадр состояния (probe занят ведущим).

## Цикл 11: DIVERGES

Ширина пересчитана: `THEME_POP_W = 140×3 + space-2×2 (гэпы) + space-2×2 (падинг) + 2 (рамка)` = **454** (было 444 — гэпы считались по 4, трём колонкам не хватало 8px).

Осталось: кадр состояния (probe занят ведущим).

---

## 15. theme-popover-column — **DIVERGES** (цикл 11)

*История: ц2:MATCH, ц8:DIVERGES, ц9:DIVERGES, ц11:DIVERGES, ц11:DIVERGES*

![оригинал](15-theme-popover-column/original.png)
![наш](15-theme-popover-column/ours.png)

### Оригинал

# 15 theme-popover-column — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:121-128
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:91-114

## JSX-структура (кратко, вложенность)
```
<div class=column>
  <div class=colTitle>{Dark|Light|Icons}</div>
  <div class=colList role=listbox aria-label={title}>
    <Item … />                            // элемент 16
    …contributed-темы/icon-темы
  </div>
</div>
```
Содержимое колонок: Dark → «Kamin Dark» + contributed dark; Light → «Kamin Light»
+ contributed light; Icons → «Catppuccin» + contributed iconThemes.

## Метрики (ИЗ CSS)
.column:
- размеры: min-width: 0 (ширина колонки от грида родителя: minmax(140px, 1fr))
- отступы: gap: var(--space-1) (flex-column)
- позиционирование: display:flex; flex-direction:column

.colTitle:
- padding: var(--space-1) var(--space-2)
- font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em
- color: var(--text-muted)

.colList:
- max-height: 320px; overflow-y: auto
- gap: 1px (flex-column)
- display:flex; flex-direction:column

## Состояния
Вариантных классов нет.

### Наша реализация

# 15 theme-popover-column — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:515-530 (closure `column`),
532-588 (наполнение Dark/Light/Icons)

## Структура (gpui-дерево кратко)
```
div (flex col, min_w 140)
 ├ colTitle (uppercase, fs XS, muted)
 └ rows: item… (элемент 16)
```
Dark = «Kamin Dark» + contributed c dark_ui; Light = «Kamin Light» +
contributed light; Icons = «Catppuccin» (built-in) + contributed icon-темы.

## Метрики (из кода, точные)
- column: min_w px(140.0), flex col; gap между строками — НЕ задан
- colTitle: px SPACE_2 (8), py SPACE_1 (4), fs m::FS_XS (11),
  color p.text_muted, .to_uppercase()

## Отличия от original.md той же папки
1. colList max-height: 320px + overflow-y:auto — НЕ РЕАЛИЗОВАНО (длинный
   список тем растянет поповер).
2. gap 1px между item'ами (.colList) — нет.
3. letter-spacing 0.04em у colTitle — нет.
4. Ширина: min_w 140 по контенту вместо грид-ячейки minmax(140px, 1fr) —
   колонки разной ширины (см. 14, п.3).
5. role=listbox / aria — не применимо.

## Дополнение атрибутов (цикл 10)

- цвета: у самой колонки фона нет — прозрачная поверх bg_surface #3d3f51 поповера (`crates/shell/src/ui/layout_popover.rs:720`); заголовок колонки text_muted #838aa0 (`layout_popover.rs:564`); строки внутри text_primary #cfd4e2 (`layout_popover.rs:500`), picked-фон accent_primary #89b4fa при альфе 0.16 (`layout_popover.rs:524`), hover bg text_primary@0.10 (`layout_popover.rs:490`)
- шрифты: заголовок колонки FS_XS = 11 + `to_uppercase()` (`layout_popover.rs:563,565`); строки FS_SM = 12 (`layout_popover.rs:499`); font-weight в колонке не задан (SEMIBOLD только у заголовка «Appearance» самого поповера, `layout_popover.rs:661`)

### Вердикты

# 15 — verdict (review cycle 1)
VERDICT: DIVERGES
gap колонки 0 vs 4; gap colList 0 vs 1; нет ls .04em. min-w140 + max-h320+scroll — 1:1.

## Цикл 2: MATCH

## Цикл 8: DIVERGES

Колонка поповера: `min_w(140)` без равнения — у оригинала все три колонки равны `max(140, широчайшая)`. gap 4/1 и max-h 320 со скроллом закрыты.

## Цикл 9: DIVERGES

`repeat(3, minmax(140px, 1fr))` при `width:max-content` (`ThemeQuickToggle.module.css:85-89`) → все три колонки равны `max(140, широчайшая)`. У нас три независимых `min_w(140)` без `flex_1` (`layout_popover.rs:547-554`; `flex_1` снят намеренно — резал длинные имена тем). gap 4/1 и `max_h 320` + скролл закрыты.

## Цикл 11: DIVERGES

Равнение колонок: min_w(140) без flex_1 против repeat(3, minmax(140px,1fr)). Правкой 14 ширина зафиксирована на 444, то есть три колонки по 140 — требует подтверждения кадром.

## Цикл 11: DIVERGES

Ширина контейнера исправлена (см. 14), но колонки по-прежнему `min_w(140)` без `flex_1` против `grid-template-columns: repeat(3, minmax(140px, 1fr))` — при широком поповере колонки не растянутся.

---

## 16. theme-popover-item — **MATCH** (цикл 11)

*История: ц2:MATCH, ц8:MATCH, ц11:MATCH*

![оригинал](16-theme-popover-item/original.png)
![наш](16-theme-popover-item/ours.png)

### Оригинал

# 16 theme-popover-item — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:130-152
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:116-157

## JSX-структура (кратко, вложенность)
```
<button type=button class="item [picked]" role=option aria-selected={picked ?? false}>
  <i class="fas {fa-moon|fa-sun|fa-icons} itemIcon" aria-hidden />
  <span class=itemName>{name}</span>
  <i class="fas fa-check itemTick" style={visibility: picked ? visible : hidden} aria-hidden />
</button>
```
Галка ВСЕГДА в DOM (visibility-toggle, не conditional) — резервирует ширину,
чтобы max-content popover не прыгал при смене пика.

## Метрики (ИЗ CSS)
.item:
- размеры: width: 100%
- отступы: padding: var(--space-2) var(--space-3); gap: var(--space-2)
- скругления: border-radius: var(--radius-sm)
- шрифт: font-size: var(--fs-sm); text-align: left
- цвета: background: transparent; color: var(--text-primary)
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent)
- transition: нет
- позиционирование: display:flex; align-items:center

.itemIcon:
- width: 16px; font-size: 12px; text-align: center; flex-shrink: 0

.itemName:
- flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis

.itemTick:
- width: 12px (фикс-слот); flex-shrink: 0; text-align: center
- font-size: 10px; color: var(--accent-primary)

## Состояния
.picked (+ .picked:hover):
- background: color-mix(in srgb, var(--accent-primary) 16%, transparent)
- color: var(--text-primary)
- .itemTick visible (inline visibility)

### Наша реализация

# 16 theme-popover-item — наша реализация

Файлы: crates/shell/src/ui/layout_popover.rs:449-514 (closure `item`
внутри appearance_popover)

## Структура (gpui-дерево кратко)
```
div#id (flex items-center, строка-option)
 ├ icon-слот w16 (fa moon/sun/icons 12px, text_muted)
 ├ label flex_1 (ellipsis, nowrap)
 └ tick-слот w12 (codicon-check \u{eab2} 10px, accent_primary;
                  invisible при !on — ширина стабильна)
```

## Метрики (из кода, точные)
- px SPACE_3 (12), py SPACE_2 (8), gap SPACE_2 (8), rounded RADIUS_SM (8)
- fs m::FS_SM (12), color p.text_primary
- icon-слот: w px(16.0), глиф fa 12px, color p.text_muted
- tick: w px(12.0), глиф 10px, color p.accent_primary (#89b4fa),
  invisible (не удалён из вёрстки) при !on
- hover (только при !on): bg tint(text_primary, 0.08)
- picked (on): bg tint(accent_primary, 0.16) постоянный, hover не перебивает

## Отличия от original.md той же папки
1. hover-фон: у нас text_primary 8%; оригинал 10%.
2. Цвет иконки: у нас text_muted; оригинал .itemIcon без цвета → наследует
   var(--text-primary) от .item. Иконка тусклее оригинала.
3. Галка: codicon-check вместо fas fa-check (другой глиф-шрифт, тот же смысл).
4. picked не задаёт color: у нас текст остаётся text_primary (совпадает,
   оригинал тоже text-primary).
Метрики (padding 8/12, gap 8, r8, fs 12, icon w16/12px, tick w12/10px accent,
picked accent 16% с visibility-галкой) — 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_SM = 12 (`crates/shell/src/ui/layout_popover.rs:499`); font-weight не задан; иконка темы `fa(glyph, 12.0)` в слоте w 16 (`layout_popover.rs:503,508`); галка `fa("\u{f00c}", 10.0)` в слоте w 12 (`layout_popover.rs:537,544`). Оригинал: `.item { font-size: var(--fs-sm) }` 12, `.itemIcon { width:16; font-size:12 }`, `.itemTick { width:12; font-size:10 }` (`titlebar/ThemeQuickToggle.module.css:124,141-146,150-157`) — совпадает

### Вердикты

# 16 — verdict (review cycle 1)
VERDICT: DIVERGES
Галка codicon-check vs fa-check. Остальное (hover 10%, picked 16%, слоты) — 1:1.

## Цикл 2: MATCH

## Цикл 8: MATCH

Пункт поповера 1:1 по коду (8/12, gap 8, бокс иконки 16 + fa 12, тик 12 + fa-check 10, hover 10%, picked 16%).

## Цикл 11: MATCH

строка 29.6, picked-фон (73,82,108) = accent 16% над bg-surface, слот иконки 16 + fa 12, тик 12/fa 10.

---

## 17. panel-icon-svg — **DIVERGES** (цикл 11)

*История: ц2:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц11:DIVERGES, ц11:DIVERGES*

![оригинал](17-panel-icon-svg/original.png)
![наш](17-panel-icon-svg/ours.png)

### Оригинал

# 17 panel-icon-svg — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\PanelIcon.tsx:46-90 (css-модуля НЕТ, всё в атрибутах SVG)

## JSX-структура (кратко, вложенность)
```
<svg width=14 height=12 viewBox="0 0 14 12" aria-hidden>
  {highlight}   // залитый rect варианта slot (рисуется ПОД рамкой)
  {frame}       // рамка: rect x=1 y=1 w=12 h=10 rx=1.5 fill=none stroke=currentColor stroke-width=1.2
</svg>
```

## Метрики (ИЗ TSX-констант)
- размеры: W=14, H=12 (фикс, не масштабируется css)
- рамка: STROKE_INSET=1 (x=1,y=1,w=12,h=10), FRAME_RADIUS=1.5, STROKE_WIDTH=1.2, stroke=currentColor, fill=none
- highlight: SLOT_RADIUS=1 (rx/ry), fill=currentColor, opacity=0.85 (HIGHLIGHT_OPACITY), SLOT_INSET=1.5
- ширины подсветок: LEFT/RIGHT/CENTER_HIGHLIGHT_W=4.5; RIGHT_HIGHLIGHT_INSET=6 → правый x=8
- нижняя полоса: BOTTOM_HIGHLIGHT_INSET_Y=5; половины правой колонки: RIGHT_QUARTER_HEIGHT=(12−3)/2=4.5, низ y=1.5+4.5=6
- цвета: только currentColor (наследует цвет контейнера); токенов нет

## Состояния (9 вариантов slot — геометрия highlight-rect)
| slot | x | y | w | h |
|---|---|---|---|---|
| main | 1.5 | 1.5 | 4.5 | 9 |
| left | 1.5 | 1.5 | 4.5 | 9 |
| right | 8 | 1.5 | 4.5 | 9 |
| right-top | 8 | 1.5 | 4.5 | 4.5 |
| right-bottom | 8 | 6 | 4.5 | 4.5 |
| center | 4.75 | 1.5 | 4.5 | 9 |
| center-bottom | 4.75 | 7 | 4.5 | 3.5 |
| main-bottom | 1.5 | 6 | 4.5 | 4.5 |
| bottom (fallback else) | 1.5 | 7 | 11 | 3.5 |

`main` и `main-bottom` — горизонтальные зеркала `right`/`right-bottom` (одинаковая
геометрия у left/main). hover/transition/позиционирование — нет (чистый inline SVG).

## Дополнение атрибутов (цикл 10)

- цвета: собственных hex нет — stroke рамки и fill подсветки = `currentColor` (`titlebar/PanelIcon.tsx:56,66-82`), подсветка с opacity 0.85 (`PanelIcon.tsx:26`). Фактический цвет даёт родитель: в меню Layout — `.itemIcon { color: var(--text-muted) }` = #838aa0 (`titlebar/LayoutToggles.module.css:113-119`, disabled → opacity 0.4, `:89`); в плейсхолдере панели — `.glyph { color: var(--text-muted) }` = #838aa0 (`panel-placeholder/PanelPlaceholder.module.css`, блок `.glyph`)
- отступы: CSS-модуля у компонента нет — ни padding, ни margin; «отступы» это SVG-инсеты внутри канвы 14×12: STROKE_INSET 1 (рамка `rect x1 y1 w12 h10`), SLOT_INSET 1.5 (границы подсветки), RIGHT_HIGHLIGHT_INSET 6 → RIGHT_HIGHLIGHT_X = 8, BOTTOM_HIGHLIGHT_INSET_Y 5 (`PanelIcon.tsx:19-20,24-25,34,38-39,48-58`); внешний зазор до label даёт `.menuItem { gap: var(--space-2) }` = 8 (`LayoutToggles.module.css:67`)

### Наша реализация

# 17 panel-icon-svg — наша реализация

Файлы: crates/shell/src/ui/panel_placeholder.rs:12-80 (SlotIcon, glyph,
slot_glyph 2.8×, slot_glyph_small 1.0×)

## Структура (gpui-дерево кратко)
```
frame div (relative, 14s×12s, rounded 1.5s, border_1 text_muted)
 └ bar div (absolute, rounded 1s, bg text_muted α0.85)   // подсвеченный слот
```
Не SVG — нативные div (рамка + залитый прямоугольник), масштаб параметром
(placeholder 2.8, layout-меню 1.0).

## Метрики (из кода, точные)
- база: W=14, H=12; frame radius 1.5·s; slot radius 1.0·s
- геометрия слотов (x, y, w, h) при s=1:
  Main 1.5,1.5,4.5,9 · MainBottom 1.5,6,4.5,4.5 · Center 4.75,1.5,4.5,9 ·
  CenterBottom 4.75,7,4.5,3.5 · Right 8,1.5,4.5,9 · RightTop 8,1.5,4.5,4.5 ·
  RightBottom 8,6,4.5,4.5
- цвета: рамка border p.text_muted (#838aa0); highlight p.text_muted α0.85
- hover/active: нет

## Отличия от original.md той же папки
1. Вариантов 7 из 9: НЕТ `bottom` (fallback 1.5,7,11,3.5) и отдельного `left`
   (у оригинала left ≡ main геометрически — покрыто Main; bottom
   НЕ РЕАЛИЗОВАН).
2. Рамка: border 1px (gpui border_1) вместо stroke-width 1.2; и рамка
   рисуется по краю бокса 14×12 — у оригинала rect с инсетом 1
   (x=1,y=1,w=12,h=10), т.е. наша рамка на 1px «шире» по каждой стороне.
3. Цвет: захардкожен text_muted (+α0.85 у слота) вместо currentColor —
   иконка не перекрашивается с контейнером (в hover/active кнопок оригинал
   светлеет вместе с текстом).
4. Масштабирование параметром s (оригинал фикс 14×12; наш slot_glyph 2.8×
   для плейсхолдеров — расширение, не расхождение в титлбаре).

## Дополнение атрибутов (цикл 10)

- гэпы: N/A: гэпы — иконка рисуется абсолютными барами внутри рамки-канвы 14×12 (`crates/shell/src/ui/panel_placeholder.rs`, `fn glyph`), flex-детей нет, gap разделять нечего

### Вердикты

# 17 — verdict (review cycle 1)
VERDICT: DIVERGES
Рамка на боксе 14×12 vs rect-инсет 1 (12×10) stroke 1.2 — рамка шире на 2px, сдвиг
хайлайта ~1px; нет варианта bottom; крупный масштаб 2.0 — у оригинала PanelIcon
не масштабируется. Геометрия 7 слотов — 1:1. panel-left.svg — полное совпадение.

## Цикл 2: DIVERGES
Рамка border на боксе vs rect-инсет 1 (12×10) stroke 1.2; нет варианта bottom; масштаб 2.0.

## Цикл 8: DIVERGES

SVG panel-icon совпадает с `PanelIcon.tsx` дословно, масштаб 2.0 **оправдан** (`.glyph svg{28×24}`) — претензия цикла 2 была ложной. Остаток: в div-варианте кламп бара к x≥2/y≥2 даёт зазор 0.4/0.8 под штрихом, у оригинала бар вплотную с 1.5.

## Цикл 9: DIVERGES

Нит, волной 16 не тронут: `panel_placeholder.rs:47` клампит бар к `max(2.0)`, у оригинала `SLOT_INSET = 1.5` (`PanelIcon.tsx:25`) и штрих рамки 1.2 при инсете 1 занимает 0.4..1.6 → бар со штрихом соприкасается, у нас зазор 0.5 при scale 1.

## Цикл 11: DIVERGES

Кламп бара к 2.0 против SLOT_INSET 1.5: на зуме 14× бар начинается по внутренней кромке рамки, у оригинала заходит на штрих.

## Цикл 11: DIVERGES

Закрыто: порядок отрисовки — подсвеченный бар рисуется ПЕРВЫМ, рамка поверх него (`PanelIcon.tsx:86-87`); раньше бар перекрывал штрих рамки по внутренней кромке.

Осталось: `rx 1.5` штриха воспроизводится бордером бокса (в gpui нет SVG-штриха) — согласованное отклонение.

---

## 18. session-tabs-strip — **DIVERGES** (цикл 11)

*История: ц2:MATCH, ц8:DIVERGES, ц9:DIVERGES, ц11:DIVERGES*

![оригинал](18-session-tabs-strip/original.png)
![наш](18-session-tabs-strip/ours.png)

### Оригинал

# 18 session-tabs-strip — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\session-tiles\SessionTabs.tsx:98-138
- %PROJECTS%\kamin-ide\src\renderer\components\session-tiles\SessionTabs.module.css

## JSX-структура (кратко, вложенность)
```
<div class=strip role=tablist aria-label="Open sessions"
     onPointerDown/Move/Up>                       // press=activate, drag ≥4px = reorder
  ×N [{drag && dropBefore===id && <span class=dropBar aria-hidden />}
      <SessionTab session dragging />]            // элемент 19
  {drag && dropBefore===null && <span class=dropBar />}   // drop в конец
  <button class=newTab aria-label="New session" data-tooltip="New session…" aria-expanded>
    <i class="fas fa-plus" />
  </button>
  <div class=spacer data-tauri-drag-region />
  {picker && <div class=picker role=menu data-session-picker style={left,top}>   // fixed, y = bottom("+")+4
    <button role=menuitem class=pickerItem> codicon-folder-opened  "New session (folder…)"</button>
    <button role=menuitem class=pickerItem> codicon-circle-large-outline "No folder session"</button>
  </div>}
</div>
```
При 0 сессий компонент возвращает null. Константы: PICKER_GAP_PX=4, DRAG_THRESHOLD_PX=4.

## Метрики (ИЗ CSS)
.strip:
- размеры: height: 100%; flex: 1; min-width: 0
- overflow-x: auto; overflow-y: hidden; scrollbar-width: none; `::-webkit-scrollbar { display: none; }`
- display:flex; align-items:center

.dropBar (метка вставки при drag):
- flex: 0 0 2px; width: 2px; height: 22px; align-self: center
- margin: 0 1px; border-radius: 1px
- background: var(--accent-primary)
- box-shadow: 0 0 4px color-mix(in srgb, var(--accent-primary) 60%, transparent)
- pointer-events: none

.spacer (drag-регион окна — НА СПЕЙСЕРЕ, не на стрипе):
- flex: 1 1 auto; align-self: stretch; min-width: 24px; -webkit-app-region: drag

.newTab:
- размеры: width: 26px; height: 26px
- отступы: margin: 0 6px; padding: 0
- скругления: border-radius: 50%
- шрифт: `> i { font-size: 12px; line-height: 1; }`
- цвета: background: var(--bg-surface); color: var(--text-muted); border: none
- hover: background: color-mix(in srgb, var(--accent-primary) 36%, var(--bg-surface)); color: var(--accent-primary); transform: scale(1.06)
- transition: background var(--transition-fast), color var(--transition-fast), transform var(--transition-fast)
- flex-shrink: 0; align-self: center; display:flex центр; cursor:pointer; -webkit-app-region: no-drag

.picker:
- position: fixed; z-index: var(--z-titlebar-popover, 10001)
- min-width: 200px; padding: var(--space-1)
- border-radius: var(--radius-md); background: var(--bg-surface)
- border: 1px solid var(--divider-soft)
- box-shadow: var(--shadow-dropdown, 0 6px 24px rgb(0 0 0 / 30%))
- -webkit-app-region: no-drag

.pickerItem:
- width: 100%; padding: 6px 8px; gap: 8px
- border: none; border-radius: var(--radius-sm); background: transparent
- color: var(--text-secondary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer
- display:flex; align-items:center; `:global(.codicon) { font-size: 14px; }`
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent); color: var(--text-primary)

## Состояния
- drag: dropBar рендерится перед целевым табом либо в конце
- picker открыт: aria-expanded=true на "+"; закрытие — outside mousedown (capture) / Esc

### Наша реализация

# 18 session-tabs-strip — наша реализация

Файлы: crates/shell/src/ui/session_tabs.rs:334-549 (session_tabs,
ordered_chips, overflow_button, tabs_overflow_menu);
«+» — crates/shell/src/ui/titlebar.rs:256-286 (id "new-session")

## Структура (gpui-дерево кратко)
```
row (flex items-center, min_w 0, overflow_hidden, pl 48, pr SPACE_3)
 ├ ×fit chip (все ФИКС 180px; активная всегда видима — подтягивается в конец)
 └ hidden не пуст: overflow_button "N ⌄" (h28, px6, r12)
     → tabs_overflow_menu в OVERLAY-окне (w240, max_h 400, список скрытых:
       точка 8px цвета сессии + имя; клик = активация)

titlebar.rs: «+» #new-session — 28×28 круг, bg_surface, fa-plus 12px,
клик → ToggleNewSessionMenu(x,y) (дропдаун folder/no-folder в overlay)
```
Константы: CHIP_W=180.0, CHIP_GAP=2.0; резерв 36px под overflow-кнопку.
Reorder: ChipPress/ChipDragOver/ChipRelease (порог и активация в root),
порядок = user order поверх сортировки last_opened.

## Метрики (из кода, точные)
- row: pl px(48.0), pr m::SPACE_3 (12), min_w 0, overflow_hidden
- chip: 180×28 фикс (элемент 19)
- overflow_button: h 28, px 6, ml 2, gap 2, rounded RADIUS_MD (12), fs 12,
  text_secondary; hover/open: bg p.bg_surface + text_primary; chevron codicon 12
- overflow-меню: w 240, max_h 400, p SPACE_1 (4), rounded RADIUS_MD, bg
  p.bg_surface, border tint(text_primary, 0.06), gap 1; item px SPACE_3
  py SPACE_2 r8 fs SM, hover text_primary 10%, точка 8×8
- «+»: 28×28 rounded_full, ml SPACE_1 (4), bg p.bg_surface,
  color text_secondary, глиф 12px; hover bg p.bg_overlay + text_primary

## Отличия от original.md той же папки
1. Оверфлоу-модель другая: у нас невлезшие чипы уходят в кнопку «N ⌄» с
   поповером; оригинал — горизонтальный скролл (overflow-x auto, скрытый
   скроллбар) без кнопки. Плюс чипы у нас не сжимаются (180 фикс) — у
   оригинала flex 0 1 180 (min 44).
2. dropBar (вертикальная метка вставки 2×22 accent + glow) НЕ РЕАЛИЗОВАНА —
   вместо неё border-left 2px accent на целевом чипе (см. 19).
3. «+»: 28×28 (оригинал 26×26), ml 4 (оригинал margin 0 6), hover — bg
   p.bg_overlay (оригинал color-mix accent 36% + color accent + scale 1.06);
   живёт в titlebar.rs после слота, а не внутри стрипа.
4. Слот не flex:1 (см. 04); spacer-drag — отдельный div.flex_1 титлбара
   (эквивалент .spacer с app-region: drag).
5. pl 48px слева (оригинал padding 0 12) — сознательное отступление.
6. Пикер «+»: пункты те же (folder/no-folder), но рендер в overlay-окне;
   min-width 200 / padding 6 8 оригинала здесь не сверялись (другой файл —
   пикер не в session_tabs.rs).
7. При 0 сессий: оригинал возвращает null; у нас row остаётся (пустой) +
   «+» всегда виден.

## Дополнение атрибутов (цикл 10)

- цвета: сам стрип (`row`) ни background, ни text_color не задаёт (`crates/shell/src/ui/session_tabs.rs:447-454`) — прозрачный, наследует text_color(text_muted #838aa0) и FS_SM=12 корня титлбара (`crates/shell/src/ui/titlebar.rs:197-198`). Цвета — у детей: чип bg bg_mantle #262533, текст text_secondary #adb3c7, резервный border text_primary при альфе 0.0 (`session_tabs.rs:65,68,69`); active — градиент tab_color 0.26 → 0.14 + border tab_color 0.45 + text_primary #cfd4e2 (dark; light 0.42/0.26/0.60) (`session_tabs.rs:72-84`); tinted 0.15 → 0.08, hover 0.22 → 0.12 (`session_tabs.rs:87,99-100`); обычный hover bg bg_surface #3d3f51 + text_primary (`session_tabs.rs:107-109`); drop-bar 2×22 bg accent_primary #89b4fa + glow accent_primary@0.6 blur 4 (`session_tabs.rs:477-483`); overflow-меню bg bg_surface #3d3f51, border text_primary@0.06 (`session_tabs.rs:539-545`)
- шрифты: у стрипа собственного кегля нет; чип text_size 12 (`session_tabs.rs:64`), font-weight не задан; `chip_action` — codicon 16 либо fa-thumbtack 10 (`session_tabs.rs:344,347`); overflow-меню — кегль строк задаётся ниже по файлу (`session_tabs.rs:552+`)

### Вердикты

# 18 — verdict (review cycle 1)
VERDICT: DIVERGES
(deviation: pl48, «N v»). dropBar: полоса 2×22 r1 accent+glow vs border_l чипа;
newTab: цвет secondary vs muted, ml4+pr12=16 vs 6, hover alpha .36 vs НЕПРОЗРАЧНЫЙ
микс accent36%+surface, нет scale (deviation); спейсер без min-w24;
плюс-меню: нет иконок folder-opened/circle-large-outline 14+gap8, item 4/12 vs 6/8,
labels другие, hover 8% vs 10%, нет shadow-dropdown, w210 vs min-w200.

## Цикл 2: MATCH
(deviations: N-кнопка, scale)

## Цикл 8: DIVERGES

Стрип чипов: рецепты strip/dropBar/spacer/newTab/«+»-меню 1:1 (кружок 26×26 #3d3f51, лейблы дословны). Расхождение — зазор чип→«+» 18.00 против 6.40 (см. 04). Отклонения: STRIP_PL 48, «N ⌄» вместо скролла стрипа, нет `scale(1.06)`.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Зазор чип→«+» закрыт вместе с 04 (кнопка внутри стрипа перед спейсером, весь зазор = её `margin 0 6`). Отклонения прежние: `STRIP_PL 48`, «N ⌄» вместо скролла стрипа, нет `scale(1.06)`.

## Цикл 9: DIVERGES

Зазор чип→«+» ЗАКРЫТ (6.40 = оригинал, см. 04). Остаются отклонения: `STRIP_PL 48`; «N ⌄» вместо `.strip{overflow-x:auto}`; нет `transform:scale(1.06)` у `.newTab:hover` — в gpui нет transform. НОВОЕ: у нас стрип `flex_shrink()` без `flex_1`, у оригинала `.strip{flex:1}` — стрип не забирает свободное место, поэтому даже с `overflow-x:auto` скроллиться будет нечему.

## Цикл 11: DIVERGES

STRIP_PL 48; «N ⌄» вместо .strip{overflow-x:auto} (у оригинала оверфлоу-кнопки нет вовсе); нет scale(1.06); стрип flex_shrink() без flex_1. Арифметика бюджета ПРОВЕРЕНА и сходится: «N ⌄» с 16 чипов, клипа до порога нет, разница ширины чипа против оригинала ровно 9.6 = STRIP_PL/5.

---

## 19. session-tab-chip — **MATCH** (цикл 11)

*История: ц2:DIVERGES, ц8:MATCH, ц11:MATCH*

![оригинал](19-session-tab-chip/original.png)
![наш](19-session-tab-chip/ours.png)

### Оригинал

# 19 session-tab-chip — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\session-tiles\SessionTab.tsx:26-65
- %PROJECTS%\kamin-ide\src\renderer\components\session-tiles\SessionTab.module.css

## JSX-структура (кратко, вложенность)
```
<div class="tab [active] [tinted] [sleeping] [switching] [pinnedTab] [dndDragging]"
     style="--tab-color:<resolved|var(--accent-primary)>"
     role=tab tabIndex=0 data-session-id aria-selected aria-busy={switching}
     data-tooltip={name | name (sleeping…) | name (loading conversation…)}
     onKeyDown(Enter/Space=activate) onContextMenu=openSessionMenu>
  <span class=leading>
    <span class=dot aria-hidden />
    <button class="pin [pinned]" aria-label="Pin session|Unpin session">
      <i class="fas fa-thumbtack" /></button>
  </span>
  <span class=label>{name}</span>
  {session.open && <button class=close aria-label="Disconnect session"
      data-tooltip="Disconnect (free from memory)">
    <i class="codicon codicon-debug-disconnect" /></button>}
</div>
```

## Метрики (ИЗ CSS)
.tab:
- размеры: height: 28px; flex: 0 1 180px; min-width: 44px; max-width: 240px
- отступы: padding: 0 6px 0 10px; margin: 6px 1px, затем margin-left: 2px (перекрывает); gap: 6px; `:first-child { margin-left: 6px; }`
- скругления: border-radius: var(--radius-md)
- шрифт: font-size: 12px; label font-weight: 500
- цвета: background: var(--bg-mantle); border: 1px solid transparent; color: var(--text-secondary)
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: нет на .tab (только на .close)
- позиционирование: display:flex; align-items:center; overflow:hidden; cursor:pointer; -webkit-app-region: no-drag

.leading (слот dot↔pin):
- position: relative; width: 16px; height: 16px; inline-flex центр; flex-shrink: 0

.dot:
- position: absolute; inset: 0; margin: auto; width: 4px; height: 4px; border-radius: 50%
- background: var(--text-muted); в .active: background: var(--tab-color)

.pin:
- position: absolute; inset: 0; display: none (flex по состояниям); центр
- background: transparent; border: none; border-radius: var(--radius-xs)
- color: var(--text-secondary); font-size: 10px; padding: 0; cursor: pointer
- hover: background: color-mix(in srgb, var(--tab-color) 16%, transparent)

.label:
- flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500

.close:
- размеры: width: 18px; height: 18px; flex-shrink: 0
- скругления: border-radius: var(--radius-xs)
- цвета: background: transparent; border: none; color: var(--text-muted); font-size: 10px; padding: 0
- opacity: 0 по умолчанию; transition: opacity .12s, background .12s, color .12s
- показ: `.tab:hover .close, .active .close { opacity: 1; }`
- hover: background: color-mix(in srgb, var(--text-primary) 14%, transparent); color: var(--text-primary)

## Состояния
.active (+ .active:hover):
- background: linear-gradient(90deg, color-mix(in srgb, var(--tab-color) 26%, transparent), color-mix(in srgb, var(--tab-color) 14%, transparent))
- border-color: color-mix(in srgb, var(--tab-color) 45%, transparent)
- color: var(--text-primary); dot окрашивается в var(--tab-color)

.tinted (есть session.color, не active):
- background: linear-gradient(90deg, color-mix(in srgb, var(--tab-color) 15%, transparent), color-mix(in srgb, var(--tab-color) 8%, transparent))
- hover: 22% / 12%

Light-тема (`[data-theme="light"]`):
- .tinted: градиент 26% / 16%
- .active: градиент 42% / 26%; border-color 60%

dot↔pin свап:
- `.tab:hover .pin { display:flex }` + `.tab:hover .dot { display:none }`
- .pinnedTab: `.pin { display:flex; color: var(--tab-color) }`, `.dot { display:none }` (постоянно)

.sleeping (pinned + деактивирована): opacity: 0.55; `.label { color: var(--text-muted) }`
.switching (active, чат ещё не догнал): `.dot { animation: tab-switching 1s ease-in-out infinite }`
  @keyframes tab-switching: 0%,100% opacity 1; 50% opacity 0.25
  prefers-reduced-motion: animation none; opacity 0.45
.dndDragging (drag-reorder): opacity: 0.4

### Наша реализация

# 19 session-tab-chip — наша реализация

Файлы: crates/shell/src/ui/session_tabs.rs:23-332 (chip, chip_action)

## Структура (gpui-дерево кратко)
```
div#tab-{id} (occlude, group, 180×28, r12)
 ├ leading 16×16
 │  ├ dot 4×4 круг (invisible при pinned и на group-hover)
 │  ├ pin-кнопка absolute inset-0 (fa-thumbtack 10px, r3;
 │  │   !pinned: invisible → visible на group-hover; pinned: всегда, цвет tab-color)
 │  └ switching: вместо всего — codicon \u{eb19} 11px accent (спиннер-глиф)
 ├ label flex_1 (ellipsis, nowrap, FontWeight::MEDIUM)
 └ s.open: chip_action disconnect (codicon-debug-disconnect \u{ead0} 12px,
     16×16 r4; скрыт → group-hover; на активном чипе виден всегда)
```
ЛКМ = ChipPress (активация на up без движения), dblclick = BeginRename,
ПКМ = OpenSessionMenu (общее меню сайдбара), move с зажатой ЛКМ = ChipDragOver.

## Метрики (из кода, точные)
- размеры: w px(CHIP_W=180.0) ФИКС, h 28; ml 2; pl 10, pr 6, gap 6
- rounded m::RADIUS_MD (12); fs 12; label weight MEDIUM (500)
- база: bg p.bg_mantle (#262533), color p.text_secondary (#adb3c7);
  hover (без цвета): bg p.bg_surface + text_primary
- active: bg linear_gradient 90° tint(tab_color,0.26)→tint(tab_color,0.14),
  border 1px tint(tab_color,0.45), color text_primary; dot = tab_color
- tinted (color, не active): градиент 0.15→0.08; hover 0.22→0.12 + text_primary
- tab_color = session.color hex | p.accent_primary (#89b4fa)
- dot: 4×4, bg text_muted (active: tab_color)
- pin: fa 10px, rounded 3, hover bg tint(tab_color, 0.16);
  pinned → цвет tab_color, иначе text_secondary
- disconnect (chip_action): 16×16, rounded 4, глиф 12px, color text_muted,
  hover bg tint(text_primary, 0.12) + text_primary
- sleeping (pinned && !open): opacity 0.55, тултип «(sleeping — click to reactivate)»
- drag_over: border_l_2 accent_primary
- switching: leading = codicon-спиннер, тултип «(loading conversation…)»

## Отличия от original.md той же папки
1. Ширина: фикс 180px; оригинал flex 0 1 180px, min 44, max 240 — наши чипы
   не сжимаются/не растут.
2. margin: у нас всем ml 2; оригинал margin-left 2 + `:first-child { margin-left: 6px }`
   — первый чип у нас на 4px левее.
3. close/disconnect: у нас 16×16 r4, hover text_primary 12%; оригинал 18×18
   radius-xs(4), hover 14%; показ у оригинала через opacity 0/1 с transition
   .12s — у нас invisible/visible без анимации.
4. dot↔pin свап: механика та же (group-hover), но у оригинала display-свап,
   у нас visibility (эквивалент).
5. switching: оригинал — пульсация dot (@keyframes tab-switching 1s);
   у нас замена leading на codicon-глиф \u{eb19} без анимации вращения.
6. dndDragging opacity 0.4 — НЕ РЕАЛИЗОВАНО (перетаскиваемый чип не тускнеет);
   вместо dropBar стрипа — border_l_2 на целевом чипе.
7. Light-тема: оригинал усиливает альфы ([data-theme=light]: tinted 26/16,
   active 42/26, border 60%) — у нас те же альфы в обеих темах.
8. keyboard (Enter/Space activate, role=tab, aria) — не применимо/нет.
Остальное (h28, r12, padding 10/6, gap 6, fs 12, weight 500, палитра
градиентов 26/14/45 и 15/8→22/12, dot 4px, pin 10px, sleeping 0.55) — 1:1.

### Вердикты

# 19 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет постоянного border transparent у неактивных (сдвиг 1px); фикс w180 vs flex 0 1 180
min44 max240 (следствие deviation); first-child ml2 vs 6; close 16×16/глиф12/hover12%
vs 18×18/10/14%; pin r3 vs 4; sleeping label не muted; switching = спиннер vs
анимированная точка; нет dndDragging .4; light-альфы не портированы.
Ядро (h28, 0 6 0 10, gap6, r12, градиент active 26-14+45%, tinted, dot/pin-свап,
тултипы) — 1:1.

## Цикл 2: DIVERGES
Нет dndDragging .4; потерян margin-right 1px (зазор 2 vs 3).

## Цикл 8: MATCH

Чип сессии совпал целиком: h28, радиус-профиль, ml6/2 + mr1 (шаг 3.20), градиент active 26→14 + бордер 45% ПОД фоном (замер 134 = расчёт), неактивный bg-mantle побитово, точка 4×4, пин fa 10 в слоте 16, disconnect 18×18 глиф 16, метка fs12/500 (line_height 1.169 её не сдвинул), sleeping .55, dndDragging .4. Ниты: точка на 1px ниже; бюджет усечения `CHIP_CHROME_W=62` не учитывает бордер и всегда вычитает место под disconnect.

## Цикл 11: MATCH

чип с пином, меткой и disconnect — как в ц.8.

---

# Зона 20-37 — Сайдбар — сессии и Customize-нав

## 20. sidebar-root — **DIVERGES** (цикл 9)

*История: ц3:MATCH, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](20-sidebar-root/original.png)
![наш](20-sidebar-root/ours.png)

### Оригинал

# 20 sidebar-root — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\Sidebar.tsx` (52-63), `Sidebar.module.css`

## JSX-структура (кратко, вложенность)
```
<aside .sidebar aria-label="Primary side bar"
       data-activity-slot="sidebar"
       data-activity-drop={blocked→"blocked" | over→"over" | undefined}
       style={width: sidebarWidth px; min-width: SIDEBAR_MIN_WIDTH_PX px}
       onDragOver/onDragLeave/onDrop (drop-target активностей)>
  {mode === "customize" ? <CustomizeMode/> : <SidebarBody/>}
  <div .resizeHandle …/>   ← элемент 21
</aside>
```
Не рендерится вовсе (`return null`), если `!sidebarVisible && mode !== "customize"` — customize-режим пинит сайдбар видимым.

## Метрики (ИЗ CSS, точные значения)
- `.sidebar`:
  - `background: transparent` (радиальный градиент appWrapper просвечивает)
  - `display: flex; flex-direction: column`
  - `flex-shrink: 1` (ужимается до inline min-width вместо выталкивания соседей)
  - `min-height: 0`
  - `position: relative`
  - ширина — inline: `width: ${sidebarWidth}px`, `min-width: ${SIDEBAR_MIN_WIDTH_PX}px` (константа из `config/constants.js`)
- padding/margin/border/border-radius: нет (все 0/none)
- шрифт: наследуется
- hover/active/focus: нет собственных
- transition/анимации: нет
- z-index: нет

## Состояния (классы-варианты с метриками)
- `data-activity-drop="over"` / `"blocked"` — подсветка задаётся глобально в `theme/global.css` (элемент 157 инвентаря), не в этом модуле.
- Режимы `sessions` / `customize` — переключают тело, сам `<aside>` не меняется.

## Дополнение атрибутов (цикл 10)

- цвета: `.sidebar { background: transparent }` (`sidebar/Sidebar.module.css:5`), color не задаётся — наследуется от `.body`; фон под сайдбаром — радиальный backdrop приложения (комментарий `Sidebar.module.css:1-3`). Ближайшие hex — у ручки: `.resizeHandleBar` градиент transparent → var(--bg-overlay) #515567 30..70% → transparent (`Sidebar.module.css:39-45`), hover/active → var(--tint-primary-strong) = color-mix(accent-blue #89b4fa 25%, transparent) (`Sidebar.module.css:52-60`; токен `theme/variables.css:128` → `:110` = color-mix(accent-blue 25%, transparent))

### Наша реализация

# 20 sidebar-root — наша реализация
Файлы: `crates\shell\src\root.rs:5245-5296` (обвязка), `crates\shell\src\root.rs:2561-2587` (gap_wrap), `crates\shell\src\root.rs:2925,2944-2951` (drag-кламп), `crates\metrics\src\lib.rs:59-61`

## Структура (gpui-дерево кратко)
```
body (flex row, pl BODY_GUTTER_X=4)
└─ .when(sidebar_visible)
   └─ div .relative .w(sidebar_w) .flex_shrink_0 .h_full
      ├─ probe_area("sidebar")            ← CDP-замена (замер rect)
      └─ gap_wrap (px 4, pt 4, pb 4)      ← эмуляция .body{gap:8; padding:0 4}
         └─ customize_open ? customize_nav(...) : sessions_sidebar(...)
   └─ v_handle("sidebar-handle", ...)     ← элемент 21
```
`sidebar_w = layout.sidebar_width_px.round()` (дефолт `SIDEBAR_DEFAULT = 270`). Фон прозрачный — радиальный градиент фона просвечивает (как оригинал).

## Метрики (из кода, точные)
- Ширина: `layout.sidebar_width_px` px, персист в layout_store; drag-кламп `PANEL_MIN_SIZE = 100 .. viewport_w − 550`
- `flex_shrink_0` (не ужимается)
- Обёртка gap_wrap: `px(4)` + `pt(4)` + `pb(4)`, `min_w/min_h 0`, `overflow_hidden`
- Собственных bg/border/radius нет

## Отличия от original.md той же папки
1. **min-width: у нас кламп 100 (`PANEL_MIN_SIZE`), в оригинале `SIDEBAR_MIN_WIDTH_PX` (и `PRIMARY_SIDEBAR_MIN_WIDTH = 200` в metrics)** — сайдбар можно ужать до 100px.
2. `flex_shrink_0` vs оригинальный `flex-shrink: 1` (у оригинала сайдбар ужимается до inline min-width, у нас — жёсткая ширина).
3. Оригинал: customize-режим ПИНИТ сайдбар видимым (`return null` только если `!sidebarVisible && mode !== "customize"`). У нас `.when(self.sidebar_visible, …)` — при скрытом сайдбаре customize-навигация не рендерится вовсе.
4. Drop-target активностей (`data-activity-drop` over/blocked, onDragOver/Drop) НЕ РЕАЛИЗОВАН.
5. Нет aria (`aria-label="Primary side bar"`) — в gpui нет аналога.
6. gap_wrap даёт свои 4px паддинга внутри колонки (оригинал — нулевой паддинг, зазор из `.body { gap }`); визуально эквивалентно, но паддинг «внутри» ширины сайдбара, т.е. полезная ширина контента на 8px меньше при той же sidebar_width.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — у корня сайдбара ховер-стиля нет (`crates/shell/src/root.rs:6236-6244`: только relative/w/flex_shrink_0/h_full/probe_area). Ховер-реакция есть у смежной ручки: `v_handle` показывает полосу 3px цвета accent_primary #89b4fa при альфе 0.25 (`crates/shell/src/ui/splitter.rs:62,86`), а видимость гонит state `hovered_handle`, не CSS-ховер (`root.rs:6286,6292-6299`)

### Вердикты

## Цикл 3: MATCH

Каркас sessions-mode совпал; flex_shrink на колонке и «customize держит сайдбар видимым» — на волне 7.

## Цикл 4: DIVERGES

`.sidebar` = `flex-shrink: 1`, у нас `flex_shrink_0`; и `Sidebar.tsx:52` рисует нав при `customize` даже со скрытым сайдбаром — у нас `when(sidebar_visible)`. Волна 8.

## Цикл 8: DIVERGES

`flex-shrink: 1` против нашего `flex_shrink_0`; нав Customize скрывается вместе с сайдбаром (оригинал держит его видимым). **Ширина исправлена волной 15**: `gap_wrap` съедал 8px тела — теперь `w(sidebar_w + 8)`.

## Цикл 9: DIVERGES

Ширина ЗАКРЫТА: probe `sidebar` w=222.4 = 215+8; активная строка 4.0-210.4 (W-8), у оригинала 4.0-216.0 (W-8 при 220); левый край контента лог. 60 у обеих. Нав Customize при скрытом сайдбаре ЗАКРЫТ: `root.rs:5727` `.when(sidebar_visible || customize_open)` = `Sidebar.tsx:24`. ОСТАЛОСЬ: `Sidebar.module.css:10` `flex-shrink:1` против нашего `.flex_shrink_0()` (`root.rs:5735`) — нужен `flex_shrink(1.)` + `min_w(PANEL_MIN_SIZE)`. Drop-таргет сайдбара (`Sidebar.tsx:58-61`, `data-activity-drop`) не реализован.

---

## 21. sidebar-resize-handle — **MATCH** (цикл 8)

*История: ц3:MATCH, ц4:MATCH, ц8:MATCH*

![оригинал](21-sidebar-resize-handle/original.png)
![наш](21-sidebar-resize-handle/ours.png)

### Оригинал

# 21 sidebar-resize-handle — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\Sidebar.tsx` (27-49, 64-73), `Sidebar.module.css`

## JSX-структура (кратко, вложенность)
```
<div .resizeHandle [+ .resizeHandleActive при drag]
     data-tooltip="Drag to resize"
     role="separator" aria-orientation="vertical" aria-label="Resize sidebar"
     onMouseDown={начало drag}>
  <span .resizeHandleBar aria-hidden="true"/>
</div>
```
Логика drag: `useDragHandler().begin({cursor:"col-resize"})`; `desired = max(SIDEBAR_MIN_WIDTH_PX, clientX - leftX)`; затем `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX)` — рост ограничен min-width центральной колонки. Ref ресинкается с сигналом перед drag (иначе jump).

## Метрики (ИЗ CSS, точные значения)
- `.resizeHandle`:
  - `position: absolute; top: 0; right: calc(-1 * var(--space-2))` — сидит ЦЕЛИКОМ в gap `--space-2` справа от сайдбара
  - `width: var(--space-2); height: 100%`
  - `cursor: col-resize`
  - `z-index: var(--z-resize-handle)`
  - `user-select: none; pointer-events: auto`
  - `display: flex; align-items: stretch; justify-content: center` (грип центрирован в gap)
- `.resizeHandleBar`:
  - `display: block; width: 2px; height: 100%`
  - `opacity: 0` (невидим в покое)
  - `background: linear-gradient(to bottom, transparent 0%, var(--bg-overlay) 30%, var(--bg-overlay) 70%, transparent 100%)`
  - `transition: opacity 0.15s, background 0.15s, width 0.15s`
  - `pointer-events: none`

## Состояния (классы-варианты с метриками)
- `.resizeHandle:hover .resizeHandleBar` и `.resizeHandleActive .resizeHandleBar` (во время drag):
  - `opacity: 1; width: 3px`
  - `background: linear-gradient(to bottom, transparent 0%, var(--tint-primary-strong) 30%, var(--tint-primary-strong) 70%, transparent 100%)`

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin НЕТ. Позиционирование — absolute: top 0, right calc(-1 * var(--space-2)) = −8px, width var(--space-2) = 8px, height 100% (`sidebar/Sidebar.module.css:19-25`) — хит целиком в 8-пиксельном зазоре `.body`, без захода на кромки соседей (комментарий `Sidebar.module.css:15-18`); грип центрируется `justify-content: center` (`:31`), ширина полосы 2px в покое → 3px по hover/active (`Sidebar.module.css:35,52`)

### Наша реализация

# 21 sidebar-resize-handle — наша реализация
Файлы: `crates\shell\src\ui\splitter.rs:22-86` (v_bar/v_handle), `crates\shell\src\root.rs:5278-5295` (монтаж), `crates\shell\src\root.rs:2916-2951,2997-3008,3376-3383` (drag/persist/handle_show)

## Структура (gpui-дерево кратко)
```
v_handle("sidebar-handle"):
div .relative .w(0) .h_full .flex_shrink_0        ← нулевая ширина в потоке
└─ div#sidebar-handle .absolute .left(-4) .top_0
     .w(SPACE_2=8) .h_full .items_center .justify_center
     .cursor_col_resize .tooltip("Drag to resize")
     .on_mouse_down(begin_drag DragKind::Sidebar) .on_hover(hovered_handle)
   └─ .when(show) v_bar(tint(accent_primary,0.25), 3.0)
        ← 3 сегмента: fade-in 30% / solid 40% / fade-out 30% (linear_gradient 180°)
```
`show = hover ручки ИЛИ активный drag` (state-driven через `RootView.hovered_handle`, occlude не используется — mouse-up должен пузыриться до корня).

## Метрики (из кода, точные)
- Hit-зона: 8px (`SPACE_2`), absolute `left: -4px` — сидит в межколоночном зазоре
- Грип: ширина 3px, высота 100%, цвет `tint(p.accent_primary, 0.25)` (dark: #89b4fa @ 25% ≈ tint-primary-strong)
- Градиент растворения: transparent→color на 0–30%, solid 30–70%, color→transparent 70–100%
- Idle: пусто (ничего не рисуется)
- Drag-кламп: `100 .. viewport_w − 550`; персист `sidebarWidthPx` одним патчем на mouse-up

## Отличия от original.md той же папки
1. Idle-полоса 2px (`opacity: 0`, bg-overlay градиент) не рендерится вовсе — визуально идентично (у оригинала она невидима), но transition width 2→3px и opacity 0.15s отсутствуют (в gpui нет transition; появление грипа мгновенное).
2. Кламп роста: у нас хардкод `viewport − 550`; оригинал — `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX)` от фактической min-width центральной колонки.
3. Минимум: 100 (`PANEL_MIN_SIZE`) vs `SIDEBAR_MIN_WIDTH_PX` оригинала.
4. Нет `role="separator"` / `aria-orientation` / `aria-label` (нет аналога в gpui).
5. Позиция hit-зоны: оригинал `right: calc(-1*8px)` от сайдбара (целиком в gap справа); у нас нулевой элемент ПОСЛЕ сайдбара с `left:-4` — центр совпадает, но зона наполовину накрывает край сайдбара, а не gap целиком.
6. `z-index` не задаётся (порядок отрисовки по дереву).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — грип 4px без содержимого и паддингов (`crates/shell/src/ui/splitter.rs`, `fn v_handle`); зазор вокруг даёт `.body gap`

### Вердикты

## Цикл 3: MATCH

Actions-блок (padding 4/8/8, строка 6×8, gap 10, иконка 20 по центру) 1:1.

## Цикл 4: MATCH

Ручка ресайза: hit 8 в зазоре, бар 3px, градиент 30/40/30 на accent 25% (= `--tint-primary-strong`), тултип «Drag to resize».

## Цикл 8: MATCH

Ручка ресайза 1:1 (хит 8 на left −4, бар 3px, сегменты 30/40/30, accent 25%, тултип).

---

## 22. sidebar-body-resolver — **MATCH** (цикл 9)

*История: ц3:MATCH, ц4:DIVERGES, ц8:DIVERGES, ц9:MATCH*

![оригинал](22-sidebar-body-resolver/original.png)
![наш](22-sidebar-body-resolver/ours.png)

### Оригинал

# 22 sidebar-body-resolver — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\Sidebar.tsx` (81-85), без собственного CSS

## JSX-структура (кратко, вложенность)
```
function SidebarBody():
  id = getPanelSignal("sidebar").value.active
  if (!id) → <ActivityPlaceholder icon="circle-large" label="No tool selected"/>
  else    → <ActivityBody id={id} slot="sidebar"/>
```
Чисто логический компонент — выбирает тело по активной активности слота `sidebar`. Визуальные метрики принадлежат `ActivityPlaceholder` (элемент 69) и телам активностей.

## Метрики (ИЗ CSS, точные значения)
- Собственных стилей нет (нет css-модуля, нет классов, нет обёрточного DOM — рендерит ребёнка напрямую).

## Состояния (классы-варианты с метриками)
- `active == null` → `ActivityPlaceholder` с `icon="circle-large"`, `label="No tool selected"`.
- `active == id` → `<ActivityBody id slot="sidebar">` (сегодня реальная реализация только у `projects` → `SessionsMode`).

## Дополнение атрибутов (цикл 10)

- цвета: N/A: цвета — `SidebarBody()` чистый резолвер, собственного DOM и CSS-модуля нет, возвращает либо `<ActivityPlaceholder>`, либо `<ActivityBody>` (`sidebar/Sidebar.tsx:81-85`). Цвета фолбэк-ветки: `.placeholder { color: var(--text-muted) }` #838aa0, `.glyph { color: var(--text-disabled) }` #60667b, `.label { color: var(--text-primary) }` #cfd4e2, `.hint { color: var(--text-muted) }` #838aa0 (`panel-placeholder/ActivityPlaceholder.module.css`, блоки `.placeholder`/`.glyph`/`.label`/`.hint`)
- отступы: N/A: отступы — у резолвера своего бокса нет (`Sidebar.tsx:81-85`). Отступы фолбэк-ветки: `.placeholder` padding var(--space-5) = 20 + gap var(--space-2) = 8; `.glyph` margin-bottom var(--space-1) = 4; `.label`/`.hint` margin 0 (`ActivityPlaceholder.module.css`)

### Наша реализация

# 22 sidebar-body-resolver — наша реализация
Файлы: `crates\shell\src\root.rs:5253-5276` (выбор тела), `crates\shell\src\root.rs:133-135,365,729-733` (sidebar_activity)

## Структура (gpui-дерево кратко)
```
gap_wrap(
  if customize_open { customize_nav(...) }   ← элементы 35-37
  else              { sessions_sidebar(...) } ← элементы 23-32
)
```
`sidebar_activity: &'static str` хранится в RootView (дефолт `"projects"`, меняется `ShellEvent::ActivityClicked`), но **телом сайдбара не управляет** — используется только для подсветки плитки в activity-bar (root.rs:5222).

## Метрики (из кода, точные)
- Собственных стилей нет (как и в оригинале — чисто логический выбор).

## Отличия от original.md той же папки
1. **Резолвер по активной активности НЕ РЕАЛИЗОВАН**: оригинал — `getPanelSignal("sidebar").active` → `<ActivityBody id slot="sidebar">`; у нас тело сайдбара всегда `sessions_sidebar` (либо customize_nav), какая бы плитка ни была кликнута.
2. **Фоллбек `ActivityPlaceholder("No tool selected", icon circle-large)` НЕ РЕАЛИЗОВАН** — состояния «активность не выбрана» в сайдбаре нет.
3. Для не-`projects` активностей (tree/terminal и др. в слоте sidebar) тел нет — оригинал резолвит их через общий `ActivityBody`.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — резолвер выбирает ТЕЛО (sessions/tool_body/placeholder) и своего бокса не рисует (`crates/shell/src/root.rs`, ветка `sidebar_mode`); паддинги принадлежат выбранному телу
- гэпы: N/A: гэпы — у резолвера один ребёнок
- цвета: N/A: цвета — своей поверхности нет, фон и текст берёт выбранное тело (карта bg-mantle #262533 / bg-sidebar #1d1d28)

### Вердикты

## Цикл 3: MATCH

Ховер строк actions (bg-surface 60% + text-primary, иконка красится) 1:1.

## Цикл 4: DIVERGES

`Sidebar.tsx:81-85` диспатчит ТЕЛО по активному тулу слота (иначе placeholder «No tool selected»); у нас всегда `sessions_sidebar`, а `sidebar_activity` влияет только на подсветку плитки. Волна 8.

## Цикл 8: DIVERGES

Диспатч тела по активному тулу — **закрыто**. **Волной 15 закрыты и два остатка**: единый источник истины (активный тул берётся из восстановленной модели, а не хардкодом) и ветка `projects` в `tool_body` (Projects теперь работает в любом слоте).

## Цикл 9: MATCH

Оба остатка закрыты: ветка `"projects"` в `tool_body` (`root.rs:3636`), `sidebar_activity: restored_sidebar_tool` из модели слоя (`root.rs:437`), не хардкод.

---

## 23. sessions-mode-root — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:MATCH*

![оригинал](23-sessions-mode-root/original.png)
![наш](23-sessions-mode-root/ours.png)

### Оригинал

# 23 sessions-mode-root — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionsMode.tsx` (9-29), `SessionsMode.module.css`

## JSX-структура (кратко, вложенность)
```
<div .root>
  <div .actions>
    <button .action data-tooltip="Start without a folder">
      <i .fas.fa-circle-plus aria-hidden/> No folder session
    <button .action data-tooltip="Pick a folder, then start a session">
      <i .fas.fa-circle-plus aria-hidden/> New session
  <div .header><span>PROJECTS</span></div>
  <div .list>
    groups.length === 0 ? <p .empty>No projects yet. Open a folder or start a session.</p>
                        : groups.map(<ProjectGroup/>)
</div>
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: `display: flex; flex-direction: column; flex: 1; min-height: 0; padding-top: var(--space-2)`
- `.actions`: `display: flex; flex-direction: column; padding: 4px 8px 8px` (top 4 / lr 8 / bottom 8)
- `.action`:
  - `display: flex; align-items: center; gap: 10px; width: 100%`
  - `padding: 6px 8px`
  - `background: transparent; border: none; border-radius: var(--radius-sm)`
  - `color: var(--text-secondary)`
  - `font: inherit; font-size: var(--fs-md); text-align: left; white-space: nowrap`
  - `cursor: pointer`
  - `transition: background var(--transition-fast), color var(--transition-fast)`
- `.action > i`: `width: 20px; text-align: center; font-size: var(--fs-lg); color: var(--text-muted)`
- `.header`:
  - `padding: 8px 8px 8px 12px` (left 12 — инсет как у FileTreeHeader)
  - `font-size: var(--fs-xs); font-weight: 500; letter-spacing: 0.08em`
  - `text-transform: uppercase; font-feature-settings: "ss01"`
  - `color: var(--text-muted); flex-shrink: 0`
- `.list`: `flex: 1; min-height: 0; overflow: auto; padding: 0 var(--space-1) var(--space-2)` (top 0 / lr space-1 / bottom space-2)
- `.empty`: `margin: 0; padding: var(--space-3) var(--space-3) var(--space-3) 12px; color: var(--text-muted); font-size: var(--fs-sm)`

## Состояния (классы-варианты с метриками)
- `.action:hover`: `background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)`
- `.action:hover > i`: `color: var(--text-primary)`
- Пустой список групп → `.empty` абзац вместо `ProjectGroup`-ов.

### Наша реализация

# 23 sessions-mode-root — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:699-861` (sessions_sidebar), `:484-509` (action_row), `crates\metrics\src\lib.rs:42-56`

## Структура (gpui-дерево кратко)
```
div#sidebar .relative .size_full .flex_col .pt(SPACE_2=8) .text_size(FS_SM=12)
├─ div .flex_col .px(8) .pb(8) .pt(4)                  ← actions
│  ├─ action_row("No folder session")
│  └─ action_row("New session")
├─ div .pl(12) .pr(8) .py(8) .text(FS_XS=11, MEDIUM, text_muted) "PROJECTS"
├─ (нет снапшота) → div .px(12) .py(12) text_muted "Loading sessions…"
└─ list: div .flex_1 .min_h(0) .flex_col .pl(SPACE_1=4) .pr(15) .pb(8) .overflow_y_scrollbar
   ├─ группы проектов (элементы 24-27)
   └─ (пусто) → div .px(12) .py(12) text_muted "No projects yet. Open a folder or start a session."
```
action_row: `.gap(10) .w_full .px(8) .py(6) .rounded(RADIUS_SM=8) .text_size(FS_MD=13) .text_color(text_secondary)` + иконка `fa-circle-plus 16px, w 20, text_muted`.

## Метрики (из кода, точные)
- root: padding-top 8; actions: 4/8/8 (top/lr/bottom) — 1:1
- action: gap 10, padding 6×8, radius 8, fs 13, hover `bg tint(bg_surface,0.6)` + `text_primary` — 1:1 (60% bg-surface)
- header: pl 12 / pr 8 / py 8, fs 11, weight 500, text_muted (dark #838aa0)
- list: pl 4 / **pr 15** / pb 8; empty: px 12 / py 12, text_muted, fs 12

## Отличия от original.md той же папки
1. **«New session» вызывает `new_no_folder_session` — тот же хендлер, что «No folder session»** (оригинал: пикер папки, затем сессия). Функциональная заглушка.
2. list `padding-right: 15px` vs оригинальные 4px (`--space-1`) — намеренно, чтобы скроллбар не перекрывал count-badge; строки справа на 11px короче.
3. Header «PROJECTS»: нет `letter-spacing: 0.08em` и `font-feature-settings: "ss01"` (текст уже uppercase-литерал, как в оригинале).
4. `.action:hover > i { color: text-primary }` не реализовано — иконка остаётся text_muted при ховере строки.
5. Тултипы кнопок («Start without a folder» / «Pick a folder, then start a session») отсутствуют.
6. Доп. состояние «Loading sessions…» (пока нет снапшота) — в оригинале его нет.
7. empty: оригинал `padding: 12 12 12 12` (space-3 + left 12) — у нас `px 12 / py 12`, совпадает.

### Вердикты

## Цикл 3: DIVERGES

Заголовок PROJECTS: (а) letter-spacing 0.08em — в gpui НЕТ такого свойства → отмеченное отклонение (ink 50.4 вместо 56.8 CSS-px); (б) ss01 ВКЛЮЧЁН волной 6 (ui::typo::ss01); (в) list padding-right 15 → 4 (волна 6).

## Цикл 4: DIVERGES

Иконка «+» в action-строках не светлела вместе со строкой (`.action:hover > i { color: text-primary }`) — **исправлено волной 7** через `group_hover`. Остальное 1:1 (padding 4/8/8, строка 6×8, gap 10, глиф 16 в боксе 20, header pl12/pr8/py8 + ss01, list `0 4 8`). `letter-spacing 0.08em` у PROJECTS — ограничение gpui (замер: ink 51.5 против 58 лог. px).

## Цикл 8: MATCH

Замер стартов строк 60.0/88.0/128.0 против 60.0/88.0/129.6; padding 4/8/8, py6/px8, gap10, глиф в боксе 20 центрирован, header + ss01, list `0 4 8`.

---

## 24. project-group-header — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:MATCH, ц8:MATCH*

![оригинал](24-project-group-header/original.png)
![наш](24-project-group-header/ours.png)

### Оригинал

# 24 project-group-header — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (43-52), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
<div .group>                        ← flex column, вся группа
  <div .header ref={headerRef} onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
    <button .headerMain onClick={toggle collapsed}>
      <i .codicon.codicon-chevron-{right|down} .chevron aria-hidden/>
      <TreeIcon .icon name={name} type="dir" expanded={!collapsed}/>
      <span .name data-tooltip={project.folderPath ?? "Sessions without a folder"}>{name}</span>
      <span .count>{total}</span>     ← active.length + inactive.length
```
Chevron: `codicon-chevron-right` при collapsed, `codicon-chevron-down` при раскрытом.

## Метрики (ИЗ CSS, точные значения)
- `.group`: `display: flex; flex-direction: column`
- `.header`: `display: flex; align-items: center; height: 26px`
- `.headerMain`:
  - `display: flex; align-items: center; gap: 6px; flex: 1; min-width: 0; height: 100%`
  - `padding: 0 4px 0 6px` (right 4 / left 6)
  - `background: transparent; border: none`
  - `color: var(--text-secondary)`
  - `text-align: left; font: inherit; font-size: var(--fs-sm); font-weight: 500`
  - `cursor: pointer; white-space: nowrap; overflow: hidden`
- `.chevron`: `flex-shrink: 0; font-size: 13px; width: 16px; text-align: center; color: var(--text-muted)`
- `.icon` (TreeIcon): `flex-shrink: 0; width: 16px; height: 16px`
- `.name`: `flex: 1; overflow: hidden; text-overflow: ellipsis`
- `.count` (бейдж-счётчик):
  - `flex-shrink: 0; min-width: 16px; height: 16px; padding: 0 5px`
  - `display: inline-flex; align-items: center; justify-content: center`
  - `border-radius: 9px`
  - `background: var(--bg-surface); color: var(--text-muted); font-size: var(--fs-xs)`

## Состояния (классы-варианты с метриками)
- `.headerMain:hover`: `color: var(--text-primary)` (только цвет текста, фон не меняется)
- collapsed → chevron `codicon-chevron-right`, `TreeIcon expanded=false`, `.sessions` не рендерится
- hover по `.header` → показывает портал-попап `actionsPop` (элемент 25)

### Наша реализация

# 24 project-group-header — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:548-655` (project_header), `:770-812` (сборка группы)

## Структура (gpui-дерево кратко)
```
div#grp-{pid} .flex .items_center .gap(6) .w_full .h(26) .pl(6) .pr(4)
  .text_size(FS_SM=12) .font_weight(MEDIUM=500) .text_color(text_secondary)
  .cursor_pointer .overflow_hidden
  .hover(text_primary) .on_hover(HoverPill grp:{pid})
  .on_mouse_down(L: ToggleProjectCollapse) .on_mouse_down(R: Delete-project modal)
├─ codicon chevron-{right|down} 13px, text_muted, flex_shrink_0
├─ icon_theme::folder_img(name, expanded) 16×16       ← Catppuccin по имени папки
├─ div .flex_1 .min_w(0) .text_ellipsis {name}
├─ count-badge: .min_w(16) .h(16) .px(5) .rounded(9) .bg(bg_surface) .text(FS_XS=11, text_muted)
└─ .when(hovered) anchor_probe()                      ← якорь для overlay-пилюли (эл. 25)
```

## Метрики (из кода, точные)
- Высота 26, gap 6, padding 0 4 0 6 — 1:1
- fs 12, weight 500, `text_secondary` #adb3c7, hover → `text_primary` #cfd4e2 (только цвет, без фона) — 1:1
- chevron: codicon 13px, `text_muted` #838aa0; folder-icon 16×16
- count-badge: min-w 16, h 16, px 5, radius 9, `bg_surface` #3d3f51, fs 11, `text_muted` — 1:1

## Отличия от original.md той же папки
1. Chevron: у оригинала фикс `width: 16px; text-align: center` — у нас глиф без фиксированной ширины 16 (интринсик ~13px); текст группы стартует на пару px левее.
2. Тултип имени (`data-tooltip = folderPath ?? "Sessions without a folder"`) не реализован.
3. Доп. поведение: right-click по хедеру открывает модал «Delete project» — в оригинале RMB на группе ничего не делает (удаление только из hover-попапа).
4. Header — один div (клик по всей строке), у оригинала `.header` + вложенная кнопка `.headerMain`; визуально эквивалентно.
5. Группа без единой сессии не рендерится вовсе (`continue`), см. эл. 26.

### Вердикты

## Цикл 3: DIVERGES

Chevron группы был без бокса (13px глиф вместо width 16 + text-align center) → сдвиг ~2 CSS-px у всей строки. Исправлено волной 6.

## Цикл 4: MATCH

Заголовок группы: chevron 13 в боксе 16 по центру, h26, gap 6, pl6/pr4, badge min-w16/h16/px5/r9/bg-surface/fs-xs — совпало в пределах 1 лог. px. «…» у обрезанного имени нет — ограничение `truncate()`.

## Цикл 8: MATCH

Заголовок группы: полоса 152.0..168.0 против 152.8..168.8; h26, chevron 13 в боксе 16, badge min-w16/h16/px5/r9.

---

## 25. project-actions-popover — **MATCH** (цикл 11)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц11:MATCH*

![оригинал](25-project-actions-popover/original.png)
![наш](25-project-actions-popover/ours.png)

### Оригинал

# 25 project-actions-popover — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (53-84), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<div .actionsPop role="toolbar" aria-label="Project actions" tabIndex={-1}
     style={left/top из clampToViewport(anchor=header, side:"right", offset:4); visibility:hidden до измерения}
     onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
  <button .popAction.add aria-label="New session in this project" data-tooltip="New session here">
    <i .codicon.codicon-add/>
  <button .popAction.delete aria-label="Delete project" data-tooltip="Delete project + its sessions">
    <i .codicon.codicon-trash/>
```
Появление: hover по header группы; позиционируется СПРАВА от header, `POPOVER_OFFSET_PX = 4`. Закрытие: mouseleave, если relatedTarget не header/попап (без таймера — мост через `::before`).

## Метрики (ИЗ CSS, точные значения)
- `.actionsPop`:
  - `position: fixed; z-index: var(--z-dropdown, 1000)`
  - `display: flex; align-items: center; gap: 2px; padding: 3px`
  - `background: var(--bg-surface)`
  - `border: 1px solid var(--divider-soft); border-radius: var(--radius-md)`
  - `box-shadow: var(--shadow-md, 0 4px 16px rgb(0 0 0 / 35%))`
- `.actionsPop::before` (невидимый hover-мост через gap):
  - `content: ""; position: absolute; top: 0; bottom: 0; left: -10px; width: 10px`
- `.popAction`:
  - `display: inline-flex; align-items: center; justify-content: center`
  - `width: 24px; height: 24px; flex-shrink: 0; padding: 0`
  - `background: transparent; border: none; border-radius: var(--radius-xs)`
  - `cursor: pointer; color: var(--text-secondary)`
  - `transition: background var(--transition-fast), color var(--transition-fast)`
- `.popAction .codicon` (`:global`): `font-size: 14px`

## Состояния (классы-варианты с метриками)
- `.popAction:hover`: `background: color-mix(in srgb, var(--text-primary) 12%, transparent); color: var(--text-primary)`
- `.add:hover`: `color: var(--accent-primary)`
- `.delete:hover`: `background: color-mix(in srgb, var(--accent-red) 15%, transparent); color: var(--accent-red)`
- До измерения (`pos == null`): `visibility: hidden`

### Наша реализация

# 25 project-actions-popover — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:382-417` (pill_wrap), `:344-376` (pill_btn), `:657-697` (project_actions_pill), `:863-872` (overlay_pill), `crates\shell\src\overlay.rs:747-781` (рендер в overlay-окне), `crates\shell\src\root.rs:1889-1903` (grace-закрытие)

## Структура (gpui-дерево кратко)
```
overlay-окно (поверх вебвью):
div .absolute .left(anchor.right + 4) .top(anchor.y − 2)
├─ hit_area()                          ← регион ввода overlay-окна
└─ pill_wrap#pill-p-{pid} .occlude .flex .items_center .gap(2) .p(3)
     .rounded(RADIUS_MD=12) .bg(bg_surface) .border_1(tint(text_primary,0.06))
     .shadow(0 4 16 rgba(0,0,0,0.35)) .on_hover(HoverPill grp:{pid})
   ├─ pill_btn codicon-add "New session here"
   └─ pill_btn codicon-trash "Delete project + its sessions" (danger)
```
pill_btn: 24×24, radius XS=4, codicon 13px, base `text_muted`, hover `bg tint(text_primary,0.12)` + `text_primary`; danger hover `bg tint(accent_red,0.16)` + `accent_red`. Показ/скрытие — state `hover_pill` (grace через generation-счётчик + отложенный сброс, а не CSS-мост).

## Метрики (из кода, точные)
- Обёртка: gap 2, padding 3, radius 12, `bg_surface` #3d3f51, border `text_primary @ 6%`, shadow 0 4 16 rgba(0,0,0,.35) — 1:1 c shadow-md
- Кнопка: 24×24, radius 4; offset от строки: +4 по x — 1:1 (POPOVER_OFFSET_PX=4)

## Отличия от original.md той же папки
1. **Иконки 13px vs оригинальные 14px** (`.popAction .codicon { font-size: 14px }` у ProjectGroup-версии; наш pill_btn общий с session-версией, где 13px).
2. Базовый цвет кнопок `text_muted` vs оригинальный `text-secondary`.
3. `.add:hover { color: accent-primary }` не реализован — add при ховере белеет (`text_primary`), а не синеет.
4. `.delete:hover` bg: у нас 16% accent_red, у оригинала 15%.
5. Вертикаль: `top = anchor.y − 2` (пилюля h=30 центрируется относительно строки 26) — оригинал позиционирует через clampToViewport(side:"right"); кламп к вьюпорту у нас ОТСУТСТВУЕТ (у правого края экрана пилюля может уехать за край).
6. Hover-мост `::before` (невидимые 10px слева) заменён event-driven механикой (HoverPill + generation grace) — поведенчески эквивалентно, но зазор 4px не является hit-зоной.
7. transition на кнопках нет (мгновенный hover).
8. Рендер в отдельном overlay-окне (пилюля живёт поверх вебвью) вместо `createPortal(document.body)` + z-dropdown.

### Вердикты

## Цикл 3: DIVERGES

Пилюля действий проекта: база пунктов была text-muted вместо text-secondary; add:hover не красил в accent; delete:hover красный фон 15% (было 16%). Исправлено волной 6.

## Цикл 4: DIVERGES

Глиф пилюли ПРОЕКТА должен быть 14 (`ProjectGroup.module.css`), у нас 13 общий с SessionItem. Позиция пилюли — **исправлено волной 7** (центрирование по якорю). Волна 8: параметр размера глифа.

## Цикл 8: DIVERGES

Глиф пилюли ПРОЕКТА должен быть 14 (`.popAction .codicon`), у нас общий 13. Позиция подтверждена по `clamp-popup.ts` — совпадает.

## Цикл 9: DIVERGES

`ProjectGroup.module.css:96` `.popAction .codicon{14px}` против 13 у сессии (`SessionItem.module.css:168`); у нас один `codicon(glyph, 13.0)` на обе пилюли (`sessions_list.rs:435`). Нужен параметр размера в `pill_btn`.

## Цикл 11: MATCH

Кегли глифов пилюль разведены параметром `glyph_px`: сессия 13 (`SessionItem.module.css:168`), проект 14 (`ProjectGroup.module.css:96`). Остальные метрики (бокс, скругление, ховер, цвета) сверены ранее и совпадают. Открытых претензий нет.

---

## 26. project-sessions-list — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:MATCH, ц8:MATCH*

![оригинал](26-project-sessions-list/original.png)
![наш](26-project-sessions-list/ours.png)

### Оригинал

# 26 project-sessions-list — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (85-103), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
{!collapsed && (
  <div .sessions>
    active.map(<SessionItem/>)                     ← активные сессии
    {total === 0 && <p .empty>No sessions yet.</p>}
    {inactive.length > 0 && (
      <button .inactiveToggle …/>                  ← элемент 27
      {showInactive && inactive.map(<SessionItem/>)}
    )}
  </div>
)}
```

## Метрики (ИЗ CSS, точные значения)
- `.sessions`: `display: flex; flex-direction: column; gap: 2px`
- `.empty`:
  - `margin: 0; padding: 2px 0 2px 18px` (top/bottom 2 / left 18)
  - `font-size: var(--fs-xs); color: var(--text-muted)`

## Состояния (классы-варианты с метриками)
- `collapsed` (по клику на header группы) → весь `.sessions` не рендерится.
- `total === 0` → абзац `.empty` «No sessions yet.»
- Инактивные сессии видны только при `showInactive` (см. элемент 27).

### Наша реализация

# 26 project-sessions-list — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:770-850` (сборка списков внутри sessions_sidebar)

## Структура (gpui-дерево кратко)
```
group: div .flex_col
├─ project_header(...)                  ← элемент 24
└─ .when(!collapsed)
   └─ sessions: div .flex_col .gap(2)
      ├─ active.map(session_row)        ← сортировка ПО АЛФАВИТУ (lowercase)
      ├─ .when(inact>0) inactive_toggle ← элемент 27
      └─ .when(show) inact.map(session_row)  ← сортировка по last_opened desc
```

## Метрики (из кода, точные)
- `.sessions`: flex-col, gap 2 — 1:1
- Прочих собственных стилей нет

## Отличия от original.md той же папки
1. **Empty-состояние «No sessions yet.» НЕ РЕАЛИЗОВАНО**: проект без сессий вообще пропускается (`if act.is_empty() && inact.is_empty() { continue; }`) — оригинал показывает группу с абзацем `.empty` (padding 2 0 2 18, fs-xs, text-muted).
2. Доп. поведение (в оригинале не описано): активные сортируются по алфавиту (стабильная позиция при клике), неактивные — свежие сверху.
3. Свёрнутость (`collapsed`) — 1:1: весь блок сессий не рендерится.

## Дополнение атрибутов (цикл 10)

- цвета: контейнер `.sessions` ни background, ни text_color не задаёт (`crates/shell/src/ui/sessions_list.rs:901` — только flex/flex_col/gap 2) — прозрачный, наследует text_size FS_SM = 12 и цвет от корня сайдбара (`sessions_list.rs:803`); единственный собственный цвет внутри контейнера — плашка «No sessions yet.» text_muted #838aa0 (`sessions_list.rs:910`); цвета строк задаёт `session_row`: покой text_secondary #adb3c7 (`sessions_list.rs:123`), hover bg = bg_surface #3d3f51 при альфе 0.55 + text_primary #cfd4e2 (`sessions_list.rs:106,169`), active — градиент tab_color 0.26 → 0.14 + border tab_color 0.45 (`sessions_list.rs:134-139`); свёрнутый inactive-хвост — text_disabled #60667b (`sessions_list.rs:607`)

### Вердикты

## Цикл 3: DIVERGES

«No sessions yet.»: padding был 26/4 вместо 2px 0 2px 18px. Исправлено волной 6.

## Цикл 4: MATCH

Список сессий группы: gap 2; «No sessions yet.» pl18/py2/fs-xs/text-muted; условие пустоты эквивалентно `total === 0`.

## Цикл 8: MATCH

Список сессий группы 1:1.

---

## 27. project-inactive-toggle — **MATCH** (цикл 8)

*История: ц3:MATCH, ц4:MATCH, ц8:MATCH*

![оригинал](27-project-inactive-toggle/original.png)
![наш](27-project-inactive-toggle/ours.png)

### Оригинал

# 27 project-inactive-toggle — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\ProjectGroup.tsx` (91-98), `ProjectGroup.module.css`

## JSX-структура (кратко, вложенность)
```
<button .inactiveToggle [+ .inactiveOpen при открытом] onClick={toggle showInactive}>
  <i .codicon.codicon-chevron-{down|right} aria-hidden/>
  {inactive.length} inactive session{s}          ← «1 inactive session» / «N inactive sessions»
</button>
```
Chevron: `codicon-chevron-down` при открытом, `codicon-chevron-right` при закрытом.

## Метрики (ИЗ CSS, точные значения)
- `.inactiveToggle`:
  - `display: flex; align-items: center; gap: 6px; width: 100%`
  - `padding: 3px 8px 3px 18px` (top/bottom 3 / right 8 / left 18)
  - `background: transparent; border: none`
  - `color: var(--text-disabled)`
  - `font: inherit; font-size: var(--fs-sm); text-align: left`
  - `cursor: pointer`
- `.inactiveToggle .codicon` (`:global`): `font-size: 12px`
- `.inactiveOpen`: класс вешается, но в CSS-модуле отдельных правил для него НЕТ (визуальное различие — только chevron-иконка из JSX).

## Состояния (классы-варианты с метриками)
- `.inactiveToggle:hover`: `color: var(--text-secondary)` (только цвет, фона нет)
- Открыт (`showInactive`) → chevron down + ниже рендерятся `SessionItem` инактивных.

### Наша реализация

# 27 project-inactive-toggle — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:511-546` (inactive_toggle), `crates\shell\src\root.rs:799-803` (ToggleInactive)

## Структура (gpui-дерево кратко)
```
div#inact-{pid} .flex .items_center .gap(6) .w_full
  .pl(18) .pr(8) .py(3)
  .text_size(FS_SM=12) .text_color(text_disabled)
  .cursor_pointer .hover(text_secondary)
  .on_mouse_down(L: ToggleInactive)
├─ codicon chevron-{down|open ? : right} 12px
└─ "{count} inactive session{s}"
```

## Метрики (из кода, точные)
- gap 6, padding 3 8 3 18 (top/bottom 3, right 8, left 18) — 1:1
- fs 12 (`FS_SM`), цвет `text_disabled` #60667b, hover → `text_secondary` #adb3c7 — 1:1
- chevron codicon 12px — 1:1; down при открытом / right при закрытом — 1:1
- Плюрализация «1 inactive session» / «N inactive sessions» — 1:1

## Отличия от original.md той же папки
Расхождений по метрикам нет. Единственное: у оригинала это `<button>` с `font: inherit`, у нас div (в gpui разницы нет); класса-модификатора `.inactiveOpen` нет и у оригинала он пустой.

## Дополнение атрибутов (цикл 10)

- шрифты: text_size FS_SM = 12 (`crates/shell/src/ui/sessions_list.rs:606`); font-weight не задан (нормальный); chevron `codicon(..., 12.0)` (`sessions_list.rs:613-616`). Текст — `"{count} inactive session{s}"` (`sessions_list.rs:617-620`)

### Вердикты

## Цикл 3: MATCH

Строка сессии: h24, padding 0/8/0/16, dot 4, резерв бордера, градиент active 26/14 + бордер 45% — совпало.

## Цикл 4: MATCH

Toggle «N inactive sessions»: pl18/pr8/py3, gap 6, fs-sm, text-disabled → secondary по ховеру, глиф 12 (замер вертикали совпал: 51 против 50 dev).

## Цикл 8: MATCH

Toggle «N inactive sessions» 1:1.

---

## 28. session-item-row — **DIVERGES** (цикл 9)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](28-session-item-row/original.png)
![наш](28-session-item-row/ours.png)

### Оригинал

# 28 session-item-row — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (88-119), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
<div .row [.active][.tinted][.inactive] ref={rowRef}
     style="--tab-color: resolveSessionColor(session.color ?? var(--accent-primary))"
     role="button" tabIndex={0}
     onClick={activateSession} onDblClick={beginRename}
     onContextMenu={openSessionMenu(x,y)} onKeyDown={F2→beginRename}
     onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
  <span .dot data-bridge={status} data-tooltip={statusTip}/>   ← элемент 29
  <span .label>{session.name}</span>
  <span .time data-tooltip={absoluteTime}>{relativeTime(session.lastOpened)}</span>
  <button .action.pin[.pinned]>…</button>                      ← элемент 30
  {showActions && createPortal(<div .actionsPop …/>)}          ← элемент 32
</div>
```
`tinted = !!session.color`; `.inactive` при `!session.open`. В режиме rename рендерится вариант `.row.editing` (элемент 31).

## Метрики (ИЗ CSS, точные значения)
- `.row`:
  - `--tab-color: var(--accent-primary)` (дефолт; переопределяется inline)
  - `display: flex; align-items: center; gap: var(--space-2); width: 100%`
  - `height: 24px; box-sizing: border-box`
  - `padding: 0 8px 0 16px` (right 8 / left 16)
  - `border: 1px solid transparent; border-radius: var(--radius-xs)`
  - `color: var(--text-secondary); font-size: var(--fs-sm); text-align: left`
  - `cursor: pointer; white-space: nowrap; overflow: hidden`
- `.label`: `flex: 1; overflow: hidden; text-overflow: ellipsis`
- `.time`:
  - `flex-shrink: 0; margin-left: auto`
  - `font-size: var(--fs-xs); font-weight: 600; color: var(--text-muted); opacity: 0.7; white-space: nowrap`
- transition на самой строке нет.

## Состояния (классы-варианты с метриками)
- `.row:hover` (не-selected): `background: color-mix(in srgb, var(--bg-surface) 55%, transparent); color: var(--text-primary)`
- `.tinted` (цветная, не активная): `background: linear-gradient(90deg, color-mix(in srgb, var(--tab-color) 24%, transparent), color-mix(in srgb, var(--tab-color) 13%, transparent))`
- `.tinted:hover`: то же с 30% / 17%
- `.active, .active:hover`:
  - `background: linear-gradient(90deg, color-mix(in srgb, var(--tab-color) 26%, transparent), color-mix(in srgb, var(--tab-color) 14%, transparent))`
  - `border-color: color-mix(in srgb, var(--tab-color) 45%, transparent)`
  - `color: var(--text-primary)`
- Light theme (`[data-theme="light"]`):
  - `.tinted`: 26% / 16%; `.tinted:hover`: 34% / 22%
  - `.active`: gradient 42% / 26%, `border-color` 60%
- `.inactive`: `opacity: 0.6`; `.inactive:hover`: `opacity: 1`; light theme: `opacity: 0.8`
- `.row:hover .action`: `display: inline-flex; opacity: 0.7` (кнопки-экшены появляются на hover)

### Наша реализация

# 28 session-item-row — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:90-269` (session_row), `:39-58` (relative_time), `crates\shell\src\root.rs:804-822` (ActivateSession)

## Структура (gpui-дерево кратко)
```
div#{sid} .group(srow-{sid}) .flex .items_center .gap(SPACE_2=8) .w_full
  .h(24) .pl(16) .pr(8) .border_1(transparent) .rounded(RADIUS_XS=4)
  .text_size(FS_SM=12) .text_color(text_secondary) .cursor_pointer .overflow_hidden
  [active]  → bg linear-gradient(90°, tab_color@26% → tab_color@14%), border tab_color@45%, text_primary
  [tinted]  → bg linear-gradient(90°, tab_color@24% → tab_color@13%)
  [else]    → hover: bg tint(bg_surface,0.55) + text_primary
  [!open]   → opacity 0.6
  .on_mouse_down(L: ActivateSession) (R: OpenSessionMenu x,y) .on_hover(HoverPill)
├─ dot (элемент 29)
├─ div .flex_1 .min_w(0) .text_ellipsis {name}
├─ time: .text(FS_XS=11, SEMIBOLD=600, text_muted) .opacity(0.7) relative_time()
├─ pin_btn (элемент 30)
└─ .when(hovered) anchor_probe()      ← якорь overlay-пилюли (эл. 32)
```
relative_time 1:1 c relative-time.ts: now / Nm / Nh / Nd.

## Метрики (из кода, точные)
- h 24, gap 8, padding 0 8 0 16, border 1 transparent, radius 4, fs 12 — 1:1
- tab_color = `session.color` hex, дефолт `accent_primary` #89b4fa — 1:1
- active: 26%→14% + border 45% + text_primary; tinted: 24%→13%; hover (plain): bg_surface@55% + text_primary — 1:1
- time: fs 11, weight 600, text_muted #838aa0, opacity 0.7 — 1:1
- inactive: opacity 0.6 — 1:1

## Отличия от original.md той же папки
1. **`.tinted:hover` (30%/17%) НЕ РЕАЛИЗОВАН** — цветные неактивные строки не реагируют на ховер фоном/цветом.
2. **`.inactive:hover { opacity: 1 }` НЕ РЕАЛИЗОВАН** — неактивная строка остаётся 0.6 при ховере.
3. **Light-варианты НЕ РЕАЛИЗОВАНЫ** (tinted 26/16, hover 34/22, active 42/26 + border 60%, inactive 0.8) — одни dark-проценты в обеих темах.
4. dblclick → rename и F2 → rename НЕ РЕАЛИЗОВАНЫ (rename только из пилюли/контекст-меню).
5. Тултип абсолютного времени на `.time` отсутствует (сознательно: строка уже несёт ховер-механику пилюли).
6. `role="button"`/`tabIndex=0`/keyboard-активация — нет.

### Вердикты

## Цикл 3: DIVERGES

Ховеры строки: у tinted-строки ховера НЕ БЫЛО (нужен 30/17), inactive не поднимался к opacity 1. Исправлено волной 6. Остаётся: dblclick/F2 → rename.

## Цикл 4: DIVERGES

★ КРАШ: два `.hover()` на одном элементе — вендоренный gpui ставит `debug_assert!(hover_style.is_none())`, в dev раскрытие «N inactive sessions» валило приложение, в release терялся ховер неактивной строки. **Исправлено волной 7** (один собранный ховер; проверено живьём — 37 неактивных раскрылись). Остаётся: dblclick/F2 по строке под курсором (F2 сейчас переименовывает АКТИВНУЮ сессию), светлая палитра `.tinted/.active/.inactive`. Волна 8.

## Цикл 8: DIVERGES

Краш двух `.hover()` закрыт. Осталось: dblclick/F2 по СТРОКЕ (F2 переименовывает активную), ховер цветной строки не поднимает цвет текста до primary (`.row:hover` красит и `.tinted`), светлая палитра.

## Цикл 9: DIVERGES

Геометрия и цвета покоя совпали ПИКСЕЛЬНО: dot 20.8-24.8 у обеих, лейбл 32.8/33.6, шаг строк 26.4/26.4, шаг action-строк 28.0/28.0, «PROJECTS» x=12.8/12.8; градиент active численно (79,68,99.5) против формулы 26% #cba6f7 = (78,67.6,99). ОСТАЛОСЬ: (1) `.row:hover` красит и `.tinted` в text-primary, у нас ховер цветной строки ставит только bg (`sessions_list.rs:156-163`); (2) нет dblclick/F2 НА СТРОКЕ (`SessionItem.tsx:96,98`) — F2 только глобальный; (3) светлая палитра строк (26/16, 34/22, 42/26, border 60%, inactive .8) против жёстких 24/13, 30/17, 26/14, 45%, 0.6 — рецепт уже есть в `session_tabs.rs:43,69-73`.

---

## 29. session-status-dot — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:MATCH, ц8:MATCH*

![оригинал](29-session-status-dot/original.png)
![наш](29-session-status-dot/ours.png)

### Оригинал

# 29 session-status-dot — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (29-38, 102-107), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
<span .dot data-bridge={bridgeStatus} data-tooltip={statusTip} aria-label={statusTip}/>
```
Источник: `session.metadata.bridgeStatus` / `bridgeWorking` (пишет Claude Bridge VSIX). `bridgeWorking === true` → статус `"working"` (приоритет над bridgeStatus). Тултипы: working→«Working…», connected→«Online», connecting→«Connecting…», error→«Error», disconnected→«Offline», иначе — без тултипа.

## Метрики (ИЗ CSS, точные значения)
- `.dot` (база):
  - `flex-shrink: 0; width: 4px; height: 4px; border-radius: 50%`
  - `background: var(--text-muted)` (серый — сессия без статуса/инактивная)
- `.active .dot`: `background: var(--tab-color)` (цветной только у selected-строки)

## Состояния (классы-варианты с метриками)
Селекторы `.row .dot[data-bridge=…]` (префикс `.row`, чтобы победить `.active .dot`):
- `[data-bridge="connected"]`: `background: var(--accent-green, #3fb950)`
- `[data-bridge="connecting"]`: `background: var(--accent-yellow, #d29922)`
- `[data-bridge="error"]`: `background: var(--accent-red, #f85149)`
- `[data-bridge="disconnected"]`: `background: var(--text-muted)`
- `[data-bridge="working"]`:
  - `width: 6px; height: 6px`
  - `background: var(--accent-blue, #58a6ff)`
  - `animation: bridgeWorkingPulse 1.1s ease-in-out infinite`
- `@keyframes bridgeWorkingPulse`: `0%,100% { opacity: 0.5; transform: scale(1) }` / `50% { opacity: 1; transform: scale(1.5) }`

## Дополнение атрибутов (цикл 10)

- отступы: у `.dot` padding/margin НЕТ (`sidebar/SessionItem.module.css:53-59`) — только `flex-shrink: 0`, бокс 4×4 (`:55-56`), в состоянии `working` 6×6 (`:69-70`); внешние отступы задаёт строка-родитель: `.row { gap: var(--space-2) }` = 8 и `.row { padding: 0 8px 0 16px }` (`SessionItem.module.css:7,11`), то есть точка стоит на 16px от левого края строки и в 8px от лейбла

### Наша реализация

# 29 session-status-dot — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:150-189` (внутри session_row)

## Структура (gpui-дерево кратко)
```
div#sdot-{sid} .flex_shrink_0 .w(size) .h(size) .rounded_full .bg(color)
  .when_some(status_tip, tooltip)
```
Источник — `session.metadata.bridgeStatus` / `bridgeWorking` (bool), `bridgeWorking` приоритетнее — 1:1.

## Метрики (из кода, точные)
- База 4×4px, radius full — 1:1
- working: **6×6px**, `accent_blue` #89b4fa, tooltip «Working…» — размер/цвет 1:1
- connected: `accent_green` #a6e3a1, «Online»
- connecting: `accent_yellow` #f9e2af, «Connecting…»
- error: `accent_red` #f38ba8, «Error»
- disconnected: `text_muted` #838aa0, «Offline»
- без статуса: active-строка → tab_color, иначе `text_muted` (без тултипа) — 1:1

## Отличия от original.md той же папки
1. **Анимация `bridgeWorkingPulse` (1.1s, opacity 0.5↔1, scale 1↔1.5) НЕ РЕАЛИЗОВАНА** — working-точка статична 6px.
2. Цвета из палитры Catppuccin (#a6e3a1 и т.д.) vs CSS-фоллбеки оригинала (#3fb950/#d29922/#f85149/#58a6ff) — фоллбеки в оригинале срабатывают только без темы, фактические переменные совпадают с нашей палитрой.
3. `aria-label` нет.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — точка 6×6 без содержимого и паддингов (`crates/shell/src/ui/sessions_list.rs`)
- гэпы: N/A: гэпы — детей нет; расстояние до имени задаёт `gap` строки сессии

### Вердикты

## Цикл 3: DIVERGES

Пульс working-точки: был sin 1s с opacity 1→0.4 без масштаба; оригинал 1.1s, opacity 0.5↔1, scale 1→1.5. Исправлено волной 6 (внутренний абсолютный кружок — transform в gpui нет).

## Цикл 4: MATCH

Точка статуса: 1.1 s, opacity 0.5↔1, «scale» 1→1.5 абсолютным внутренним кружком в боксе 6px; приоритет working > bridgeStatus > active-tab-color 1:1.

## Цикл 8: MATCH

Точка статуса 1:1 (1.1s, 0.5↔1, «scale» внутренним кружком, приоритет working > bridgeStatus > tab-color).

---

## 30. session-pin-button — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:MATCH*

![оригинал](30-session-pin-button/original.png)
![наш](30-session-pin-button/ours.png)

### Оригинал

# 30 session-pin-button — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (111-119), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
<button .action.pin[.pinned при session.pinned]
        aria-label="Pin session"|"Unpin session"
        data-tooltip="Pin to top bar"|"Unpin from top bar"
        onClick={stopPropagation; toggleSessionPinned}>
  <i .fas.fa-thumbtack aria-hidden/>
</button>
```
Кнопка «всегда на месте» в строке (последняя перед порталом), но по CSS скрыта до hover, если сессия не запинена.

## Метрики (ИЗ CSS, точные значения)
- `.action` (база):
  - `display: none` (скрыта без hover — нулевая layout-стоимость)
  - `align-items: center; justify-content: center`
  - `width: 20px; height: 20px; flex-shrink: 0; padding: 0`
  - `background: transparent; border: none; border-radius: var(--radius-xs)`
  - `cursor: pointer; color: var(--text-muted)`
- `.action > i`: `font-size: 13px`
- `.pin > i`: `font-size: 10px` (fa-thumbtack чанковее codicons — уменьшен)

## Состояния (классы-варианты с метриками)
- `.row:hover .action`: `display: inline-flex; opacity: 0.7`
- `.action:hover`: `opacity: 1 !important`
- `.pin.pinned`: `display: inline-flex; opacity: 1; color: var(--tab-color)` — запиненная видима всегда, без hover
- `.pin:hover`: `color: var(--tab-color)`

### Наша реализация

# 30 session-pin-button — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:293-341` (pin_btn)

## Структура (gpui-дерево кратко)
```
div#pin-{sid} .flex_shrink_0 .w(20) .h(20) .items_center .justify_center
  .rounded(RADIUS_XS=4) .cursor_pointer
  .text_color(pinned ? tab_color : text_muted)
  .hover(bg tint(text_primary,0.12) + text_primary)
  .tooltip("Pin to top bar" | "Unpin from top bar")
  .on_mouse_down(L: kamin:sessions:setPinned !pinned)
└─ fa-thumbtack 10px (контейнер 14×14)
[!pinned] → .invisible().group_hover(srow-{sid}, visible)   ← виден только при ховере строки
```

## Метрики (из кода, точные)
- 20×20, radius 4, fa-thumbtack 10px — 1:1
- pinned: виден всегда, цвет tab_color — 1:1
- unpinned: скрыт до ховера строки (invisible + group_hover) — механика 1:1 (у оригинала display:none → inline-flex)

## Отличия от original.md той же папки
1. **Hover-стиль другой**: оригинал — `opacity: .7` при появлении, `opacity: 1` + `color: var(--tab-color)` на own-hover, БЕЗ фона; у нас — полная непрозрачность сразу, own-hover даёт `bg text_primary@12%` + `text_primary` (белеет, а не красится в tab_color).
2. Промежуточного состояния «opacity 0.7 на row-hover» нет.
3. `aria-label` нет.

## Дополнение атрибутов (цикл 10)

- отступы: padding НЕТ (`crates/shell/src/ui/sessions_list.rs:350-390`); бокс 20×20 (`sessions_list.rs:353-354`), глиф центрируется flex+items_center+justify_center (`:355-357`); собственных margin нет — зазор до соседей даёт `.row` gap SPACE_2 = 8 (`sessions_list.rs:114`); внутренний бокс глифа 14×14 (`sessions_list.rs:388-389`)
- цвета: покой — pinned → tab_color (цвет сессии, дефолт accent_primary #89b4fa, `sessions_list.rs:100-104`), иначе text_muted #838aa0 (`sessions_list.rs:360-364`); hover → text_color tab_color, БЕЗ фона (`sessions_list.rs:366`); фон в любом состоянии не задаётся; непинованная кнопка невидима, по group-ховеру строки проявляется с opacity 0.7 (`sessions_list.rs:391-396`)
- шрифты: `fa(FA_THUMBTACK, 10.0)` — кегль 10, семейство FA_FAMILY (`sessions_list.rs:386-387`); своего text_size у кнопки нет — наследует FS_SM = 12 строки (`sessions_list.rs:122`). Оригинал: `.pin > i { font-size: 10px }` (`sidebar/SessionItem.module.css:98`) — совпадает

### Вердикты

## Цикл 3: DIVERGES

Пин: ховер строки должен давать opacity 0.7, свой ховер — цвет tab-color БЕЗ фона; у нас фон text-primary 12%. Волна 7.

## Цикл 4: DIVERGES

Пин рисовал прямоугольный фон text-primary 12% и появлялся сразу с opacity 1 — **исправлено волной 7**: `.pin:hover { color: tab-color }` без фона, показ с opacity 0.7 → 1 по своему ховеру.

## Цикл 8: MATCH

Пин 1:1: без фона, `invisible → visible + 0.7` по ховеру строки, свой ховер 1.0, tab-color, fa 10 в боксе 14.

---

## 31. session-rename-input — **DIVERGES** (цикл 9)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](31-session-rename-input/original.png)
![наш](31-session-rename-input/ours.png)

### Оригинал

# 31 session-rename-input — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (73-86), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
// при renamingSessionId === session.id вместо обычной строки:
<div .row[.active][.tinted][.inactive].editing style="--tab-color:…">
  <span .dot aria-hidden/>
  <input .input ref={inputRef} defaultValue={session.name}
         onKeyDown={Enter→commit; Escape→endRename}
         onBlur={commit}/>
</div>
```
`commit()` = `renameSession(id, input.value)` + `endRename()`. На входе в режим — `focus()` + `select()` (useEffect). Триггеры входа: dblclick по строке, F2, «Rename» в попапе/меню.

## Метрики (ИЗ CSS, точные значения)
- `.editing` (модификатор к `.row`): `background: var(--bg-surface)`
- `.input`:
  - `flex: 1; min-width: 0`
  - `background: var(--bg-base)`
  - `border: 1px solid var(--accent-primary); border-radius: var(--radius-xs)`
  - `color: var(--text-primary)`
  - `font: inherit; font-size: var(--fs-sm)`
  - `padding: 1px 4px`
  - `outline: none`
- Габариты контейнера — как у `.row` (height 24px, padding 0 8px 0 16px, gap var(--space-2)).

## Состояния (классы-варианты с метриками)
- Enter — commit; Escape — cancel (`endRename` без записи); blur — commit.
- hover/focus-стилей у `.input` сверх постоянной accent-рамки нет.

### Наша реализация

# 31 session-rename-input — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:191-214` (ветка rename в session_row), `crates\shell\src\root.rs:1503-1528` (Begin/Commit/CancelRename), `:3844-3863` (ленивое создание InputState + focus), `:4899-4903` (Esc)

## Структура (gpui-дерево кратко)
```
session_row (те же h 24 / pl 16 / pr 8 / gap 8 / radius 4 + active/tinted стили)
├─ dot
└─ div .flex_1 .min_w(0)
     .on_key_down(enter → CommitRename; escape → CancelRename)
   └─ Input::new(input_state).appearance(false)     ← gpui_component, «голый» инпут
```
InputState создаётся лениво в render (seed = имя сессии), сразу `window.focus()`. Commit → `kamin:sessions:rename`.

## Метрики (из кода, точные)
- Габариты контейнера — как у строки (h 24, padding 0 8 0 16, gap 8) — 1:1
- Инпут: `appearance(false)` — без собственного фона/рамки/паддинга; fs наследуется от строки (12)

## Отличия от original.md той же папки
1. **Стили инпута НЕ ПЕРЕНЕСЕНЫ**: оригинал — `bg var(--bg-base)` #313240, `border 1px solid accent-primary`, radius 4, padding 1×4, text_primary; у нас инпут прозрачный без рамки (визуально режим редактирования почти неотличим от обычной строки).
2. **`.editing { background: var(--bg-surface) }` на строке НЕ РЕАЛИЗОВАН**.
3. blur → commit НЕ РЕАЛИЗОВАН (только Enter=commit, Esc=cancel; Esc также глобально через root).
4. `select()` всего текста при входе — не делается (только фокус, seed-значение).
5. time/pin в editing-строке не скрываются... (оригинал рендерит только dot+input; у нас ветка rename тоже возвращает row с dot+input без time/pin — 1:1).

## Дополнение атрибутов (цикл 10)

- шрифты: НЕ НАЙДЕНО: явного кегля/веса у инпута нет — рендерится `Input::new(input).appearance(false)` без text_size/font_weight (`crates/shell/src/ui/sessions_list.rs:265`), кегль отдан дефолту `gpui_component::input`; строка-родитель задаёт FS_SM = 12 (`sessions_list.rs:122`), но наследует ли его Input — в нашем коде не выражено. Оригинал явно ставит `font: inherit; font-size: var(--fs-sm)` = 12 (`sidebar/SessionItem.module.css:183-184`)
- ховер: собственного ховера у инпута нет; ветка rename возвращает тот же `row`, поэтому действует ховер строки — bg = bg_surface #3d3f51 при альфе 0.55 + text_primary #cfd4e2 (`sessions_list.rs:106,168-171`, ветка rename `:246-267`). Отклонение: у оригинала при редактировании фон строки фиксируется непрозрачным `.editing { background: var(--bg-surface) }` #3d3f51 (`SessionItem.module.css:174`) — у нас такого состояния нет, вместо него остаётся полупрозрачный ховер

### Вердикты

## Цикл 3: DIVERGES

Rename-инпут: нужен bg-base + бордер accent + radius 4 + padding 1/4 и коммит по blur. Волна 7.

## Цикл 4: DIVERGES

Rename-инпут без фона/рамки/паддинга (`appearance(false)`), строка без `bg-surface`, коммита по blur и select-all при входе нет. Оригинал: bg-base + 1px accent-primary + radius-xs + padding 1/4. Волна 8.

## Цикл 8: DIVERGES

Rename-инпут без рецепта: нужен bg-base + 1px accent + radius-xs + padding 1/4, `.editing { background: bg-surface }`, коммит по blur, select-all.

## Цикл 9: DIVERGES

Не тронуто: `.editing{background:bg-surface}` + инпут bg-base, 1px accent-primary, radius-xs, padding 1/4 (`SessionItem.module.css:174-187`), select-all и `onBlur=commit` (`SessionItem.tsx:41,82`) против `Input::new(input).appearance(false)` без фона/рамки/паддинга (`sessions_list.rs:246-266`) и создания без select/blur (`root.rs:4193-4199`). Чинить: обёртка + `cx.subscribe` на `InputEvent::Blur` + `dispatch_action(SelectAll)`.

---

## 32. session-actions-popover — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:MATCH*

![оригинал](32-session-actions-popover/original.png)
![наш](32-session-actions-popover/ours.png)

### Оригинал

# 32 session-actions-popover — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (120-162), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<div .actionsPop role="toolbar" aria-label="Session actions" tabIndex={-1}
     style={left/top из clampToViewport(anchor=row, side:"right", offset:4); visibility:hidden до измерения}
     onMouseEnter={openActions} onMouseLeave={closeUnlessBridging}>
  <button .popAction.rename data-tooltip="Rename"><i .codicon.codicon-edit/></button>
  {session.open && <button .popAction.disconnect data-tooltip="Disconnect (free from memory)">
      <i .codicon.codicon-debug-disconnect/></button>}
  <button .popAction.delete data-tooltip="Delete session"><i .codicon.codicon-trash/></button>
```
Появление: hover строки; `POPOVER_OFFSET_PX = 4`, clampToViewport избегает нативного browser-вебвью. Закрытие немедленное (без таймера), если relatedTarget не row/попап — hover-мост `::before` делает handoff одним mouseleave.

## Метрики (ИЗ CSS, точные значения)
- `.actionsPop`:
  - `position: fixed; z-index: var(--z-dropdown, 1000)`
  - `display: flex; align-items: center; gap: 2px; padding: 3px`
  - `background: var(--bg-surface)`
  - `border: 1px solid var(--divider-soft); border-radius: var(--radius-md)`
  - `box-shadow: var(--shadow-md, 0 4px 16px rgb(0 0 0 / 35%))`
- `.actionsPop::before` (прозрачный hover-мост через gap слева):
  - `content: ""; position: absolute; top: 0; bottom: 0; left: -10px; width: 10px`
- `.popAction`:
  - `display: inline-flex; align-items: center; justify-content: center`
  - `width: 24px; height: 24px; flex-shrink: 0; padding: 0`
  - `background: transparent; border: none; border-radius: var(--radius-xs)`
  - `cursor: pointer; color: var(--text-secondary)`
- `.popAction > i`: `font-size: 13px`
- transition нет (в отличие от ProjectGroup-версии, где есть).

## Состояния (классы-варианты с метриками)
- `.popAction:hover`: `background: color-mix(in srgb, var(--text-primary) 12%, transparent); color: var(--text-primary)`
- `.popAction.rename:hover`: `color: var(--accent-primary)`
- `.popAction.disconnect:hover`: `color: var(--accent-blue)`
- `.popAction.delete:hover`: `color: var(--accent-red)`
- disconnect-кнопка рендерится только при `session.open`.
- До измерения (`pos == null`): `visibility: hidden`.

### Наша реализация

# 32 session-actions-popover — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:343-417` (pill_btn/pill_wrap), `:419-481` (session_actions_pill), `:863-872` (overlay_pill), `crates\shell\src\overlay.rs:747-781`, `crates\shell\src\root.rs:1889-1903` (grace)

## Структура (gpui-дерево кратко)
```
overlay-окно: div .absolute .left(row.right + 4) .top(row.y − 2)
├─ hit_area()
└─ pill_wrap#pill-s-{sid} .occlude .flex .gap(2) .p(3) .rounded(RADIUS_MD=12)
     .bg(bg_surface) .border_1(tint(text_primary,0.06)) .shadow(0 4 16 @35%)
     .on_hover(HoverPill {sid})
   ├─ pill_btn codicon-edit "Rename"                        → BeginRename
   ├─ .when(open) pill_btn codicon-debug-disconnect
   │    "Disconnect (free from memory)"                     → deactivate
   └─ pill_btn codicon-trash "Delete session" (danger)      → ConfirmModal
```
pill_btn: 24×24, radius 4, codicon 13px, base `text_muted`; hover `bg tint(text_primary,0.12)` + `text_primary`; danger hover `bg tint(accent_red,0.16)` + `accent_red`.

## Метрики (из кода, точные)
- Обёртка: gap 2, padding 3, `bg_surface`, border text_primary@6% (≈divider-soft), radius 12, shadow 0 4 16 rgba(0,0,0,.35) — 1:1
- Кнопки 24×24, codicon 13px — 1:1 (session-версия оригинала тоже 13)
- Появление по ховеру строки, offset 4px вправо — 1:1
- disconnect только при `session.open` — 1:1

## Отличия от original.md той же папки
1. Базовый цвет кнопок `text_muted` #838aa0 vs оригинальный `text-secondary` #adb3c7.
2. **Цветные hover-акценты пунктов НЕ ПЕРЕНЕСЕНЫ**: оригинал — rename:hover `accent-primary`, disconnect:hover `accent-blue`; у нас оба просто белеют (`text_primary`).
3. delete:hover: у нас свой `bg accent_red@16%`; оригинал session-версии — bg остаётся text-primary@12%, меняется только цвет иконки на accent-red.
4. clampToViewport НЕ РЕАЛИЗОВАН (пилюля может выйти за края; у оригинала ещё и обход нативного browser-вебвью).
5. Hover-мост `::before` (10px слева) заменён event-механикой HoverPill + generation-grace; сам зазор 4px не hit-зона.
6. Вертикаль: top = row.y − 2 (центрирование 30px-пилюли на 24px-строке чуть иное, чем у clampToViewport).
7. Рендер в overlay-окне (поверх вебвью) вместо портала в body.
8. `role="toolbar"`/aria нет.

## Дополнение атрибутов (цикл 10)

- шрифты: N/A: шрифты — в пилюле нет текстовых узлов, только иконочные кнопки; кегль глифов `codicon(glyph, glyph_px)` = 13.0 для всех трёх кнопок сессии (`crates/shell/src/ui/sessions_list.rs:439`, вызовы `:493`, `:509`, `:532`); у пилюли проекта тот же `pill_btn` вызывается с 14.0 (`sessions_list.rs:408-410`). Оригинал: `.popAction > i { font-size: 13px }` (`sidebar/SessionItem.module.css:168`) — совпадает

### Вердикты

## Цикл 3: DIVERGES

Пилюля сессии: rename→accent-primary, disconnect→accent-blue, delete→красный ТЕКСТ на нейтральном фоне. Исправлено волной 6.

## Цикл 4: DIVERGES

Пилюля сессии 1:1 по составу и цветам; вертикаль — **исправлено волной 7** (центрирование по якорю вместо `-2`).

## Цикл 8: MATCH

Пилюля действий сессии 1:1 + центрирование по якорю. Нит: нет клампа top в вьюпорт.

---

## 33. session-context-menu — **MATCH** (цикл 8)

*История: ц3:MATCH, ц4:MATCH, ц8:MATCH*

![оригинал](33-session-context-menu/original.png)
![наш](33-session-context-menu/ours.png)

### Оригинал

# 33 session-context-menu — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionContextMenu.tsx` (41-66, 88-92), `SessionContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
// Одно на приложение, монтируется в App.tsx; driven by signal sessionMenu.
<div .menu role="menu" ref
     style={left/top = клик, клампится к viewport с margin 8px (MENU_MARGIN_PX); visibility:hidden до измерения}>
  <button .item role="menuitem"><i .codicon.codicon-edit/> Rename</button>
  {s.open && <button .item><i .codicon.codicon-sparkle/> Auto-rename from chat</button>}   ← exec "claude-bridge.regenerateTitle"
  <button .item><i .codicon.codicon-{pinned-dirty|pin}/> {Unpin from top bar | Pin to top bar}</button>
  {s.open && <button .item><i .codicon.codicon-circle-slash/> Deactivate (free memory)</button>}
  <div .swatches>…</div>                                    ← элемент 34
  <div .divider/>
  <button .item.danger role="menuitem"><i .codicon.codicon-trash/> Delete</button>
</div>
```
Закрытие: mousedown вне (capture) / Escape.

## Метрики (ИЗ CSS, точные значения)
- `.menu`:
  - `position: fixed; z-index: var(--z-titlebar-popover, 10001)` (должно перекрывать титлбар — `--z-dropdown` оставлял меню за таб-стрипом)
  - `min-width: 200px; padding: var(--space-1)`
  - `border-radius: var(--radius-md)`
  - `background: var(--bg-surface); border: 1px solid var(--divider-soft)`
  - `box-shadow: var(--shadow-dropdown, 0 6px 24px rgb(0 0 0 / 30%))`
- `.item`:
  - `display: flex; align-items: center; gap: 8px; width: 100%`
  - `padding: 6px 8px`
  - `border: none; border-radius: var(--radius-sm); background: transparent`
  - `color: var(--text-secondary); font: inherit; font-size: var(--fs-sm); text-align: left`
  - `cursor: pointer`
- `.item .codicon` (`:global`): `font-size: 14px`
- `.divider`: `height: 1px; margin: var(--space-1) 4px; background: var(--divider-soft)`

## Состояния (классы-варианты с метриками)
- `.item:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent); color: var(--text-primary)`
- `.danger`: `color: var(--accent-red)`
- `.danger:hover`: `background: color-mix(in srgb, var(--accent-red) 16%, transparent); color: var(--accent-red)`
- Пункты «Auto-rename from chat» и «Deactivate» — только при `session.open`.
- Pin-иконка: `codicon-pinned-dirty` при pinned, иначе `codicon-pin`.
- До измерения (`pos == null`): `visibility: hidden`.

### Наша реализация

# 33 session-context-menu — наша реализация
Файлы: `crates\shell\src\ui\context_menu.rs:28-36,66-107,145-328` (session_menu/menu_item), `crates\shell\src\overlay.rs:929` (рендер), `:174-190` (dropdown_shadow), `crates\shell\src\root.rs:823-827,4890-4891,5054-5073` (open/close/скрим)

## Структура (gpui-дерево кратко)
```
overlay-окно:
div .absolute .left(x) .top(y) .w(MENU_W=208) .flex_col .p(SPACE_1=4)
  .rounded(RADIUS_MD=12) .bg(bg_surface) .border_1(tint(text_primary,0.06))
  .shadow(dropdown: 0 8 24 rgba(0,0,0,0.45))
  + hit_area() + stop_propagation на клик внутри
├─ menu_item codicon-edit "Rename"                       → BeginRename
├─ .when(open) menu_item sparkle "Auto-rename from chat" → claude-bridge.regenerateTitle
├─ menu_item codicon-{pinned-dirty|pin} "{Unpin|Pin} … top bar"
├─ .when(open) menu_item codicon-circle-slash "Deactivate (free memory)"
├─ swatches (элемент 34)
├─ divider .h(1) .mx(4) .my(4) .bg(tint(text_primary,0.06))
└─ menu_item codicon-trash "Delete" (danger)             → ConfirmModal
```
menu_item: gap 8, px 8, py 6, radius SM=8, fs 12, codicon 14px; base `text_secondary`, hover `bg text_primary@10%` + `text_primary`; danger — base/hover `accent_red`, hover bg `accent_red@16%`. Кламп: x/y в вьюпорт с margin 8 (est высота 260). Закрытие: скрим/клик-мимо/Esc в root.

## Метрики (из кода, точные)
- padding 4, radius 12, bg_surface #3d3f51, border text_primary@6%, divider 1px mx4 my4 — 1:1
- item: 6×8, gap 8, radius 8, fs 12, codicon 14 — 1:1
- hover 10% / danger 16% — 1:1; состав и условия пунктов (open-only) — 1:1
- MENU_MARGIN 8 — 1:1 (MENU_MARGIN_PX)

## Отличия от original.md той же папки
1. **Ширина: фикс `MENU_W = 208` vs оригинальный `min-width: 200` (авто-рост под контент)**.
2. Shadow: 0 8 24 @45% (общий overlay dropdown_shadow) vs `--shadow-dropdown` 0 6 24 @30% — темнее и ниже.
3. Кламп по высоте — по оценке est_h=260, а не по фактическому измерению (`visibility:hidden` до измерения у оригинала); при куцем меню у нижнего края позиция чуть выше идеала.
4. z-index-механика не нужна: меню в отдельном overlay-окне (эквивалент `--z-titlebar-popover`).
5. `role="menu"`/`menuitem` нет.

## Дополнение атрибутов (цикл 10)

- шрифты: пункт меню text_size FS_SM = 12 (`crates/shell/src/ui/context_menu.rs:97`); font-weight не задан; глиф пункта `codicon(glyph, 14.0)` (`context_menu.rs:105`); глиф «Clear colour» `codicon(CIRCLE_SLASH, 13.0)` (`context_menu.rs:299`). Заголовков/секций с иным кеглем в меню нет

### Вердикты

## Цикл 3: MATCH

Toggle «N inactive sessions» (pl 18, py 3, gap 6, text-disabled → secondary по ховеру, глиф 12) 1:1.

## Цикл 4: MATCH

Контекст-меню сессии 1:1 (min-w 200, p4, radius-md, bg-surface, border 6%, dropdown-shadow, пункты gap8/px8/py6/fs-sm, danger red + 16%, глиф 14, divider h1 mx4 my4, порядок и гейты `open`). Нит: высота для клампа константой 260.

## Цикл 8: MATCH

Контекст-меню сессии 1:1.

---

## 34. session-color-swatches — **DIVERGES** (цикл 9)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](34-session-color-swatches/original.png)
![наш](34-session-color-swatches/ours.png)

### Оригинал

# 34 session-color-swatches — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionContextMenu.tsx` (67-87), `SessionContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
<div .swatches>                                  ← внутри .menu (элемент 33)
  SESSION_COLORS.map(c =>
    <button .swatch [.swatchActive при s.color === c.value]
            style={background: resolveSessionColor(c.value)}
            aria-label="Set colour {c.value}"
            onClick={setSessionColor(s.id, c.value); close}/>)
  <button .swatchClear aria-label="Clear colour" data-tooltip="Clear colour"
          onClick={setSessionColor(s.id, null); close}>
    <i .codicon.codicon-circle-slash/>
  </button>
</div>
```
Палитра — `SESSION_COLORS` из `signals/sessions.js`; фон свотча — inline через `resolveSessionColor`.

## Метрики (ИЗ CSS, точные значения)
- `.swatches`:
  - `display: flex; align-items: center; gap: 4px; flex-wrap: wrap`
  - `padding: 6px 8px`
- `.swatch`:
  - `width: 16px; height: 16px; border-radius: 50%`
  - `border: 2px solid transparent; padding: 0`
  - `cursor: pointer`
  - background — inline (цвет сессии)
- `.swatchClear`:
  - `width: 18px; height: 18px`
  - `display: grid; place-items: center`
  - `background: transparent; border: none; border-radius: 50%`
  - `color: var(--text-muted); cursor: pointer`
- `.swatchClear .codicon` (`:global`): `font-size: 13px`

## Состояния (классы-варианты с метриками)
- `.swatch:hover`: `transform: scale(1.15)` (transition не задан)
- `.swatchActive` (текущий цвет сессии): `border-color: var(--text-primary)`
- `.swatchClear:hover`: `color: var(--text-primary)`

### Наша реализация

# 34 session-color-swatches — наша реализация
Файлы: `crates\shell\src\ui\context_menu.rs:23-26` (SESSION_COLORS), `:109-143` (swatch), `:253-298` (ряд + clear)

## Структура (gpui-дерево кратко)
```
div .flex .items_center .flex_wrap .gap(4) .px(SPACE_2=8) .py(6)
├─ SESSION_COLORS.map(swatch):
│    div#sw-{i} .w(16) .h(16) .rounded_full .border_2
│      .border_color(active ? text_primary : transparent) .bg(hex)
│      .hover(opacity 0.85) .on_mouse_down(setColor)
└─ clear: div#sw-clear .w(18) .h(18) .rounded_full .items_center .justify_center
     .text_color(text_muted) .hover(text_primary) .tooltip("Clear colour")
   └─ codicon-circle-slash 13px            .on_mouse_down(setColor null)
```
SESSION_COLORS (8): `#89b4fa #a6e3a1 #f9e2af #fab387 #f38ba8 #cba6f7 #94e2d5 #f5c2e7`.

## Метрики (из кода, точные)
- Ряд: gap 4, wrap, padding 6×8 — 1:1
- Свотч: 16×16, border 2 (transparent / text_primary при active), круглый — 1:1
- Clear: 18×18, codicon 13, text_muted → hover text_primary, tooltip — 1:1

## Отличия от original.md той же папки
1. **Hover свотча: `opacity 0.85` vs оригинальный `transform: scale(1.15)`** (в gpui div-hover не умеет transform) — эффект «увеличения» заменён затуханием.
2. Цвета — только dark-варианты `SESSION_COLORS`; `resolveSessionColor` (light-подмена) НЕ РЕАЛИЗОВАН — в светлой теме свотчи остаются пастельно-тёмными.
3. `aria-label` («Set colour …», «Clear colour») нет.

## Дополнение атрибутов (цикл 10)

- скругления: свотч `rounded_full` (полный круг) при боксе 16×16 и border_2 (`crates/shell/src/ui/context_menu.rs:127-132`); кнопка сброса цвета тоже `rounded_full`, бокс 18×18 (`context_menu.rs:277-282`); у контейнера-ряда скругления нет (`context_menu.rs:257-263`)
- шрифты: N/A: шрифты — свотчи чисто цветовые, текста и глифов не содержат (`context_menu.rs:125-144`); единственный глиф в ряду — «Clear colour» `codicon(CIRCLE_SLASH, 13.0)` (`context_menu.rs:299`)

### Вердикты

## Цикл 3: DIVERGES

Свотчи цвета: ховер-scale в gpui недоступен (нет transform) — отклонение; светлая палитра resolveSessionColor не подключена. Волна 7.

## Цикл 4: DIVERGES

У свотчей появился НЕсуществующий в оригинале `hover(opacity 0.85)` (в оригинале `transform: scale(1.15)`, которого в gpui нет) + не перенесён светлый `resolveSessionColor`. Волна 8.

## Цикл 8: DIVERGES

Выдуманный ховер свотчей **убран волной 15** (в оригинале `transform: scale(1.15)`, в gpui недоступен). Остаётся светлый `resolveSessionColor`.

## Цикл 9: DIVERGES

Выдуманный ховер убран ✓ (`context_menu.rs:133-135`). ОСТАЛОСЬ: светлые варианты цветов (`sessions.ts:21-37` `SESSION_COLORS[].light` + `resolveSessionColor`) — у нас только dark (`context_menu.rs:24-27`), `grep 1e66f5|40a02b|8839ef` по crates/ = 0; затрагивает `sessions_list.rs:100-104` и `session_tabs.rs:35-39`.

---

## 35. customize-mode-nav — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:MATCH, ц8:MATCH*

![оригинал](35-customize-mode-nav/original.png)
![наш](35-customize-mode-nav/ours.png)

### Оригинал

# 35 customize-mode-nav — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\CustomizeMode.tsx` (79-97), `CustomizeMode.module.css`

## JSX-структура (кратко, вложенность)
```
<div .root>
  <header .header>
    <span .title>CUSTOMIZE</span>
  </header>
  <ul .list>
    PANELS.map(<NavItem/>)                 ← 5 встроенных: Settings(settings-gear),
                                             Design(symbol-color), Extensions(extensions),
                                             Logs(output), System(pulse)
    containers.map(<ContributedTree/>)     ← контрибьютнутые customize-контейнеры (элемент 37)
  </ul>
</div>
```
Контейнеры: `registry.viewContainers.filter(location === "customize")`.

## Метрики (ИЗ CSS, точные значения)
- `.root`: `display: flex; flex-direction: column; padding: var(--space-3) 0; gap: var(--space-2)`
- `.header`: `padding: 8px 12px; display: flex; align-items: center`
- `.title`:
  - `font-size: var(--fs-xs); font-weight: 500; letter-spacing: 0.08em`
  - `color: var(--text-muted); font-feature-settings: "ss01"`
  - (текст «CUSTOMIZE» — uppercase литералом, `text-transform` не задан; рецепт хедера совпадает с PROJECTS из SessionsMode)
- `.list`:
  - `list-style: none; margin: 0; padding: 0 var(--space-2)`
  - `display: flex; flex-direction: column; gap: 2px`

## Состояния (классы-варианты с метриками)
- Собственных состояний у контейнера нет; строки — элементы 36/37.

### Наша реализация

# 35 customize-mode-nav — наша реализация
Файлы: `crates\shell\src\ui\customize.rs:18-25` (PANELS), `:32-160` (customize_nav), `crates\shell\src\root.rs:5253-5263` (монтаж: плоский сайдбар без карточки)

## Структура (gpui-дерево кратко)
```
div .flex_col .size_full .gap(2) .px(SPACE_2=8) .py(SPACE_3=12)
├─ header: div .px(8) .pb(8) .text(FS_XS=11, text_muted) "CUSTOMIZE"
├─ PANELS.map(nav-item)                 ← элемент 36; 5 пунктов:
│    Settings(settings-gear) / Design(symbol-color) / Extensions(extensions)
│    / Logs(output) / System(pulse)     — набор и иконки 1:1
└─ contributed-узел + страницы          ← элемент 37
```

## Метрики (из кода, точные)
- Колонка: gap 2, px 8, py 12
- Header: px 8 (итог слева 8+8=16), pb 8, fs 11, `text_muted` #838aa0

## Отличия от original.md той же папки
1. Header-инсет: у нас 16px слева (px колонки 8 + px хедера 8) vs оригинальные 12 (`padding: 8px 12px` при list-инсете 8).
2. **`font-weight: 500`, `letter-spacing: 0.08em`, `font-feature-settings: "ss01"` у титула НЕ ПЕРЕНЕСЕНЫ** — обычный regular без разрядки.
3. Вертикальный ритм: оригинал `.root { padding: 12px 0; gap: 8px }` + `.header { padding: 8px 12px }`; у нас py 12 + pb 8 у хедера — близко, но `gap: 8` между хедером и списком заменён паддингом, а строки идут с общим gap 2 колонки.
4. Список — не `<ul>/<li>` (в gpui нет семантики), стили эквивалентны (`padding: 0 8; gap: 2` → px 8 / gap 2 на колонке).

### Вердикты

## Цикл 3: DIVERGES

Customize-нав: fs был 12 вместо 13 (fs-md), padding 8/8 вместо 8/12, иконка 15 с жёстким text-muted вместо 14 с наследованием цвета, ховер text-primary 8% вместо bg-surface 50%. Исправлено волной 6.

## Цикл 4: MATCH

Нав Customize: `.root` py space-3 + gap space-2, header px12/py8 + ss01 + fs-xs/500, `.list` px space-2 + gap 2; питч строки 34 лог. против 33.2 расчётных (после фикса line-height). letter-spacing — ограничение gpui.

## Цикл 8: MATCH

Нав Customize 1:1.

---

## 36. customize-nav-item — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:MATCH, ц8:MATCH*

![оригинал](36-customize-nav-item/original.png)
![наш](36-customize-nav-item/ours.png)

### Оригинал

# 36 customize-nav-item — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\CustomizeMode.tsx` (18-37), `CustomizeMode.module.css`

## JSX-структура (кратко, вложенность)
```
<li>
  <button .item [.child][.active] aria-pressed={active}
          onClick={activeCustomizePanel.value = id}>
    <NavIcon/>          ← isImageIcon ? <img width=16 height=16 alt=""> : <span .codicon.codicon-{icon}>
    <span>{label}</span>
  </button>
</li>
```

## Метрики (ИЗ CSS, точные значения)
- `.item`:
  - `display: flex; align-items: center; gap: var(--space-2); width: 100%`
  - `padding: var(--space-2) var(--space-3)`
  - `border-radius: var(--radius-sm); background: transparent`
  - `color: var(--text-secondary)`
  - `font-size: var(--fs-md); text-align: left`
  - (border/cursor в модуле не заданы)
- `.item .codicon` (`:global`): `font-size: 14px !important`
- `<img>`-иконка: атрибуты `width=16 height=16`

## Состояния (классы-варианты с метриками)
- `.item:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.active, .active:hover`: `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` (faint accent tint, без fill; одинаково в light и dark)
- `.child` (вложенная страница контрибьютнутого контейнера): `padding-left: calc(var(--space-3) + 18px)`

### Наша реализация

# 36 customize-nav-item — наша реализация
Файлы: `crates\shell\src\ui\customize.rs:56-86` (builtin-строки), `:123-158` (child-строки contributed)

## Структура (gpui-дерево кратко)
```
div#cz-{id} .flex .items_center .gap(SPACE_2=8) .px(8) .py(6)
  .rounded(RADIUS_SM=8) .text_size(FS_SM=12) .text_color(text_secondary)
  .cursor_pointer
  .hover(bg tint(text_primary,0.08) + text_primary)
  [active] → bg tint(accent_primary,0.16) + text_primary
  .on_mouse_down(L: SetCustomizePanel)
├─ codicon(codicon_by_name(icon), 15px) .text_color(text_muted)
└─ {label}
child-вариант (contributed-страница): .pl(40) .pr(8), остальное то же
```

## Метрики (из кода, точные)
- padding 6×8, gap 8, radius 8, fs 12
- Иконка codicon 15px, `text_muted`
- hover: `text_primary@8%` + text_primary; active: `accent_primary@16%` + text_primary
- child: padding-left 40

## Отличия от original.md той же папки
1. **font-size 12 (`FS_SM`) vs оригинальный `var(--fs-md)` = 13**.
2. Паддинг 6×8 vs оригинальный `8×12` (`space-2 space-3`) — строки ниже и уже.
3. Иконка 15px vs `14px !important`; цвет у нас всегда `text_muted` (оригинал наследует цвет строки: text-secondary → text-primary на hover/active).
4. hover-фон: `text_primary@8%` vs `color-mix(bg-surface 50%, transparent)` — другой рецепт (у оригинала полупрозрачный серый поверхностный, у нас белёсый).
5. active: `accent_primary@16%` + text_primary — 1:1.
6. child-инсет 40 vs `calc(space-3 + 18px)` = 30.
7. `<img width=16 height=16>`-иконки (contributed image icon) НЕ ПОДДЕРЖАНЫ — только codicon по имени с фоллбеком `\u{eb51}`.
8. `aria-pressed` нет.

## Дополнение атрибутов (цикл 10)

- цвета: покой — фона нет, text_secondary #adb3c7 (`crates/shell/src/ui/customize.rs:100`); hover — bg = bg_surface #3d3f51 при альфе 0.5 + text_primary #cfd4e2 (`customize.rs:89,102`); active — bg = accent_primary #89b4fa при альфе 0.16 + text_primary #cfd4e2 (`customize.rs:112-113`); иконка своего цвета не имеет, наследует цвет строки (`customize.rs:106-108`)

### Вердикты

## Цикл 3: DIVERGES

Заголовок CUSTOMIZE: padding 8/12 + ss01 — исправлено волной 6; letter-spacing 0.08em недоступен в gpui (отклонение).

## Цикл 4: MATCH

Пункт нава: px12/py8, gap 8, radius-sm, fs-md 13, text-secondary, глиф 14 без жёсткого цвета, ховер bg-surface 50% + primary, active accent 16% + primary, `.child` pl 30. Остаток 0.8px — фиксированный бокс кодикона 16 против line-box 15.2.

## Цикл 8: MATCH

Пункт нава Customize 1:1.

---

## 37. customize-contributed-tree — **DIVERGES** (цикл 7)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц7:DIVERGES*

![оригинал](37-customize-contributed-tree/original.png)
![наш](37-customize-contributed-tree/ours.png)

### Оригинал

# 37 customize-contributed-tree — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\CustomizeMode.tsx` (42-72), `CustomizeMode.module.css`

## JSX-структура (кратко, вложенность)
```
<>
  <li>
    <button .item [.active при childActive] aria-expanded={open}
            onClick={toggle open; если ни одна страница не открыта → открыть views[0]}>
      <span .codicon.codicon-chevron-right .chevron [.chevronOpen при open]/>
      <NavIcon icon={container.icon}/>
      <span>{container.title}</span>
    </button>
  </li>
  {open && views.map(<NavItem … icon={v.icon ?? "circle-small"} child/>)}   ← элемент 36 с .child
</>
```
`childActive = views.some(v.id === active)` — родитель подсвечен `.active`, когда открыта любая его страница. Дефолт `open = true`.

## Метрики (ИЗ CSS, точные значения)
- Родительская строка — те же `.item`/`.active`, что у элемента 36:
  - `.item`: `display: flex; align-items: center; gap: var(--space-2); width: 100%; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: transparent; color: var(--text-secondary); font-size: var(--fs-md); text-align: left`
  - `.item .codicon`: `font-size: 14px !important`
- `.chevron` (ведёт перед иконкой):
  - `flex: 0 0 auto`
  - `font-size: 12px !important`
  - `color: var(--text-muted)`
  - `transition: transform 120ms ease`
- Дочерние строки: `.child` → `padding-left: calc(var(--space-3) + 18px)`

## Состояния (классы-варианты с метриками)
- `.chevronOpen`: `transform: rotate(90deg)` (иконка всегда `codicon-chevron-right`, поворот через CSS)
- `.item:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.active, .active:hover` (childActive): `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)`
- Свёрнут (`!open`) — дочерние `NavItem` не рендерятся.

### Наша реализация

# 37 customize-contributed-tree — наша реализация
Файлы: crates/shell/src/ui/customize.rs (`customize_nav`, `nav_icon`), host_link.rs (`CzContainer`/`CzPage`, `customize_pages_from`, `customize_pages_from_manifests`)

## Структура (gpui-дерево кратко)
```
для КАЖДОГО контейнера location=customize:
├─ строка-родитель (id "cz-contrib-<id>")
│   ├─ chevron-right|down 12, text-muted
│   ├─ nav_icon(container.icon)
│   └─ container.title
└─ (раскрыт) строки-дети: nav_icon(view.icon ?? circle-small) + view.name
```
Клик по родителю: тоггл группы И, если ни одна его страница не открыта, — открыть `views[0]` (как в оригинале). Родитель подсвечен, когда активна его дочерняя страница (childActive).

## Метрики (из кода, точные)
- Строка: gap SPACE_2 8, px SPACE_3 12, py SPACE_2 8, radius RADIUS_SM 8, fs FS_MD 13, цвет text-secondary; hover bg-surface 50% + text-primary; активная — accent-primary 16% + text-primary.
- Дочерняя строка: `padding-left = SPACE_3 + 18` = 30 (`.child` оригинала), pr 12.
- `nav_icon`: путь/URL (`data:`,`http:`,`https:`,`file:`,`/`) → `img` 16×16; иначе codicon 14 (fallback-глиф gear).
- Фолбэк иконки страницы — `circle-small`, как в оригинале.

## Отличия от original.md той же папки
Титул/иконка контейнера больше не захардкожены — берутся из реестра (`viewContainers[].title/.icon`), причём и из манифестов на диске до прихода снапшота.

## Дополнение атрибутов (цикл 10)

- цвета: строка-родитель контейнера — text_secondary #adb3c7 (`crates/shell/src/ui/customize.rs:144`), hover bg = bg_surface #3d3f51 при альфе 0.5 + text_primary #cfd4e2 (`customize.rs:121,146`), childActive — bg accent_primary #89b4fa при альфе 0.16 + text_primary (`customize.rs:165-166`); chevron text_muted #838aa0 (`customize.rs:159`); дочерние строки — text_secondary #adb3c7, тот же hover, active accent_primary@0.16 + text_primary (`customize.rs:188,190,198-199`); собственного фона в покое нет
- шрифты: строки (и родитель, и дети) text_size FS_MD = 13 (`customize.rs:142,187`); font-weight не задан; chevron `codicon(..., 14.0)` (`customize.rs:157`); `nav_icon` — `codicon(g, 14.0)` либо картинка 16×16 для path/URL-иконок (`customize.rs:41-42,49`); заголовок «CUSTOMIZE» над списком — FS_XS = 11, weight MEDIUM (`customize.rs:77-78`)

### Вердикты

## Цикл 3: DIVERGES

Contributed-узел: отступ детей был 40 вместо 30 (space-3+18) — исправлено; титул и иконка контейнера по-прежнему захардкожены («Claude Bridge» / comment-discussion) вместо данных реестра. Волна 7.

## Цикл 4: DIVERGES

Лишний `mt(space-2)` перед узлом, титул был text-primary вместо secondary, не было подсветки родителя при активной дочерней странице — **всё исправлено волной 7**. Остаётся: титул и иконка контейнера из реестра вместо захардкоженных «Claude Bridge»/comment-discussion. Волна 8.

## Цикл 8: DIVERGES

Титул и иконка contributed-узла захардкожены («Claude Bridge» / comment-discussion), хотя реестр несёт настоящие значения.

## Цикл 9: DIVERGES

`CustomizeMode.tsx:63-64` берёт `container.icon`/`container.title`, у нас захардкожено `comment-discussion` / «Claude Bridge» (`customize.rs:125,128`); корень — `host_link.rs:597-606` берёт из `viewContainers` только id. Совпадает СЛУЧАЙНО (в манифесте контейнер и есть «Claude Bridge»). Живьём нав верен: дети на лог. 39.2 = 8 + 30 ✓, родитель+ребёнок подсвечены вместе ✓. Ещё: один узел на ВСЕ customize-контейнеры (оригинал `containers.map`), фолбэк иконки `gear` против `circle-small`, нет `<img>`-ветки для image-иконок, клик по родителю не выбирает `views[0]`.


## Цикл 7: DIVERGES

Переписано на реестровые контейнеры: узел на КАЖДЫЙ customize-контейнер, реальные
`title`/`icon`, ветка image-иконки, фолбэк страницы `circle-small`, клик по родителю
открывает `views[0]`. Ревью цикла подтвердило метрики строк, отступ `.child` = 30,
childActive-подсветку и совпадение манифестных данных со снапшотом.

Исправлено по итогам ревью: chevron 14 (специфичность `.item :global(.codicon)`
перебивает `.chevron{12px}`), `flex-shrink: 0` у chevron и иконок, контейнер без вью
больше не пропадает, раскрытие группы сбрасывается при входе в Customize (оригинал
размонтирует `CustomizeMode` с `useState(true)`), неизвестное имя иконки даёт пустой
бокс вместо шестерёнки.

Осталось: нет анимации поворота chevron 120 мс (в gpui нет `transform` — ограничение
порта); ранний путь из манифестов работает только в dev-репозитории (`dev_repo()`),
в упакованной сборке навигация появится после снапшота реестра.

---

# Зона 38-51 — Activity-бар, рейлы, пикеры, стрипы

## 38. activity-bar-nav — **DIVERGES** (цикл 11)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц11:DIVERGES*

![оригинал](38-activity-bar-nav/original.png)
![наш](38-activity-bar-nav/ours.png)

### Оригинал

# 38 activity-bar-nav — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:117-128`, `ActivityBar.module.css` (`.bar`, `.barReverse`)

## JSX-структура (кратко, вложенность)
```
<nav class="bar [barReverse]"
     aria-label="{slot} activities"
     data-activity-strip="1"
     data-activity-slot={slot}            // sidebar | rightTop | rightBottom …
     data-activity-orientation="vertical"
     data-activity-drop="blocked|over|undefined">   // от useActivityDropTarget
  // align="top" (default):    {fixedHead}{buttons}{picker}
  // align="bottom" (reverse): {picker}{buttons}{fixedHead}
  fixedHead = <CustomizeTile/>            // только slot === "sidebar" (элемент 40)
  buttons   = <ul class="list"> {tiles} </ul>   // элемент 39
  picker    = <ActivityPicker slot popDirection={reverse ? "down" : "up"}/>  // элемент 42
</nav>
```
- `slot === "sidebar" && !sidebarVisible` → возвращает `null` (бар исчезает целиком).
- `customizeOwnsBar` (sidebar + sidebarMode === "customize") → ни одна плитка не подсвечена активной.
- Сам `<nav>` — drop target (`useActivityDropTarget(slot)`), позиционная вставка между иконками.

## Метрики (ИЗ CSS, точные значения)
`.bar`:
- `display: flex; flex-direction: column; align-items: center`
- `gap: var(--space-2)`
- `padding: var(--space-3) 0` (top/bottom var(--space-3), left/right 0)
- `width: var(--layout-activity-bar-width, 44px)`
- `flex-shrink: 0`
- фон: НЕТ собственного (прозрачная колонка, просвечивает app-backdrop градиент)
- border/border-radius: нет; шрифт: не задаёт (иконки)

`.list` (внутренний `<ul>`):
- `list-style: none; margin: 0; padding: 0`
- `display: flex; flex-direction: column; gap: 2px`
- `width: 100%; align-items: center`

## Состояния (классы-варианты с метриками)
- `.barReverse` (align="bottom"): добавляет `justify-content: flex-end`; DOM-порядок в JSX перевёрнут на {picker, list, fixedHead} — пара «пикер+иконки» прижата к низу, пикер прямо НАД верхней иконкой.
- `data-activity-drop="over" | "blocked"` — атрибут ставится, стилей в этом css-модуле для него нет (подсветку рисует карточка-приёмник).

## Дополнение атрибутов (цикл 10)

- цвета: `.bar` ни background, ни color НЕ задаёт (`activity-bar/ActivityBar.module.css:5-13`) — прозрачная колонка, под ней видно градиент приложения (комментарий `:1-4`). Hex — у детей: `.btn`/`.picker` color var(--text-muted) #838aa0 (`ActivityBar.module.css:62`), hover bg color-mix(var(--bg-surface) #3d3f51 50%, transparent) + color var(--text-primary) #cfd4e2 (`:87-88`), active bg color-mix(var(--accent-primary) #89b4fa 16%, transparent) + color #cfd4e2 (`:95-96`), `.dropPlaceholder` border accent-primary 70% + bg accent-primary 14% (`:27-28`)

### Наша реализация

# 38 activity-bar-nav — наша реализация
Файлы: `crates/shell/src/ui/activity_bar.rs:128-198` (`activity_bar()` — сайдбарный бар), `crates/shell/src/ui/right_column.rs:125-246` (`rail()` — вертикальные рейлы правых карт, аналог align top/bottom), вызов: `crates/shell/src/root.rs:5200-5244`; константы `crates/metrics/src/lib.rs` (`ACTIVITY_BAR_WIDTH`, `SPACE_3`).

## Структура (gpui-дерево кратко)
```
div#activity-bar .relative .flex_shrink_0
  w = ACTIVITY_BAR_WIDTH (44) , h_full
  flex column, items_center, gap 2px, py SPACE_3 (12)
  ├ probe_area("activity-bar")          // замер под probe/оверлей
  ├ tile("customize","gear")            // фиксированный gear СВЕРХУ (элемент 40)
  ├ tile(id, icon) × entries            // pinned слота Sidebar (элемент 39)
  └ div#activity-dots 32×32 «…»         // пикер (элемент 42)
```
- Плитки = `activity.state(Sidebar).pinned` через `lookup()` (только builtin — contributed в сайдбарный бар не попадают: фильтр `filter_map(lookup)` в root.rs:5209-5215).
- `customize_open` → `active=None`: плитки тулов не подсвечены, горит только gear (аналог `customizeOwnsBar`).
- Правые карты: отдельная функция `rail()` (right_column.rs) — та же геометрия (44px, тайлы 32, gap 2, py 12), `bottom=true` → `justify_end` + DOM-порядок {«…», плитки} (зеркало `barReverse`).

## Метрики (из кода, точные)
- Ширина: `m::ACTIVITY_BAR_WIDTH` = **44px**; `flex_shrink_0`, `h_full`.
- Паддинг: `py(SPACE_3)` = **12px** верх/низ, 0 гориз.
- Gap: **2px** — единый между ВСЕМИ детьми (gear/плитки/«…»).
- Фон: нет (прозрачная колонка, просвечивает радиальный градиент).
- Цвета: иконки `p.text_muted` #838aa0 → hover `p.text_primary` #cfd4e2 (детали в 39).

## Отличия от original.md той же папки
1. **Gap между секциями**: оригинал — `.bar` gap `--space-2` (8px) между fixedHead/`.list`/picker, внутри `.list` 2px; у нас единый gap **2px** на всё — gear и «…» стоят ближе к плиткам, чем в оригинале.
2. **Бар не исчезает при скрытом сайдбаре**: оригинал возвращает `null` при `slot==="sidebar" && !sidebarVisible`; у нас `activity_bar` — безусловный child body (root.rs:5200), `.when(self.sidebar_visible)` накрывает только сайдбар-колонку.
3. **Не drop-target**: оригинал — `useActivityDropTarget(slot)` на `<nav>` c позиционной вставкой; у нас hit-test дроп-зон по probe-bounds в глобальном mouse-move (root.rs:4954-4972), вставочного плейсхолдера в вертикальном баре нет (см. 41).
4. Нет семантики: `aria-label`, `data-activity-*` атрибутов нет (нативный рендер, вместо них probe_area).
5. Contributed-тулы в сайдбарном баре не отображаются (filter_map по builtin-lookup); в рейлах правых карт (`lookup_any`) — отображаются.
6. `align=bottom` реализован не в `activity_bar()`, а отдельным `rail()` (правые карты); сайдбарный бар — всегда top.

## Дополнение атрибутов (цикл 10)

- скругления: N/A: скругления — у контейнера бара rounded не задан (`crates/shell/src/ui/activity_bar.rs:148-165`), как и в оригинале; скругления только у детей: плитка RADIUS_SM = 8 (`activity_bar.rs:108`), «…»-пикер rounded 8 (`activity_bar.rs:202`); то же в рейлах правых карт: плитка и `rail-dots` RADIUS_SM = 8 (`crates/shell/src/ui/right_column.rs:180`)

### Вердикты

## Цикл 3: DIVERGES

Гэпы рейла были едиными: 8 у right_column (на ВСЕХ детей) и 2 у activity_bar (на всех). Оригинал: `.bar` gap 8 только между группами (gear / list / picker), `.list` = 2. Исправлено волной 6 — тайлы завёрнуты во внутренний список.

## Цикл 4: DIVERGES

Метрики рейла подтверждены пиксельно (48px, py 12, гэпы 8/2/8, reverse-порядок; центры плиток ours 89.5/139/181.5/232 против orig 87/137.5/179/229.5). Остаётся: бар рендерится безусловно (оригинал `ActivityBar.tsx:50` возвращает `null` при скрытом сайдбаре), нет позиционного drop-target, contributed-тулы отфильтрованы (`lookup` вместо `lookup_any`). Волна 8.

## Цикл 8: DIVERGES

Метрики совпали ПИКСЕЛЬНО после поправки на рамку захвата: центр gear (28.0, 71.2) против (28.0, 70.0), питч 34.0 у обеих, центр «…» 185.2 против 184.0 — приём `w(+4).pr(4)` верен. Осталось: бар рисуется при скрытом сайдбаре (оригинал `return null`), нет позиционного drop-таргета, `lookup` вместо `lookup_any` → contributed-контейнер не даёт плитку.

## Цикл 9: DIVERGES

Метрики совпали: gear (28.00,28.00) у обеих, плитки 68.00/102.00 против 68.40/101.60, питч 34.00, «…» 142.40 против 142.00. Бар гаснет с сайдбаром (root.rs:5746). ОСТАЛОСЬ: activity::lookup builtin-only выбрасывает contributed-плитки; нет позиционного drop (индекс по clientY).

## Цикл 11: DIVERGES

Закрыто: плитки бара стали и источником, и целью drag'а — `ToolPress` шлётся с press, `ToolDragOverTab(Sidebar, i)` с зажатой ЛКМ над плиткой. До этого `over_index` приходил ТОЛЬКО из горизонтальных стрипов, поэтому плитку сайдбара нельзя было ни утащить, ни показать место вставки.

Осталось: `activity::lookup` builtin-only — contributed-плитки выпадают из бара; индекс вставки берётся по плитке под курсором, а не по позиционному расчёту `clientY`.

---

## 39. activity-tile — **DIVERGES** (цикл 9)

*История: ц3:MATCH, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](39-activity-tile/original.png)
![наш](39-activity-tile/ours.png)

### Оригинал

# 39 activity-tile — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:82-99`, `ActivityBar.module.css` (`.list`, `.btn`, `.btnActive`, `.btnImage`, `.tileDragging`)

## JSX-структура (кратко, вложенность)
```
<li key={id} data-tile="1" class={dragging ? "tileDragging" : undefined}>
  <button type="button"
          class="btn [btnActive]"
          aria-pressed={isActive}
          aria-label={item.label}
          data-tooltip={item.label}
          onPointerDown={beginActivityDrag(e, slot, id)}   // pointer-drag; pointerup = клик-активация
          onKeyDown={Enter|Space → activateActivity}
          onContextMenu={openActivityContextMenu(slot, id, clientX, clientY)}>
    <ToolIcon icon={item.icon} imageClassName="btnImage"/>   // элемент 51
  </button>
</li>
```
- HTML5 drag не используется (Tauri `dragDropEnabled` его глушит) — pointer-based drag.
- `isActive = !customizeOwnsBar && id === state.active`.

## Метрики (ИЗ CSS, точные значения)
`.btn` (общий селектор `.btn, .picker`):
- `width: 32px; height: 32px`
- `display: grid; place-items: center`
- `background: transparent; border: none`
- `border-radius: var(--radius-sm)`
- `color: var(--text-muted)`
- `font: inherit; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`

Иконка внутри:
- `.btn :global(.codicon)` — `font-size: 18px; line-height: 1`
- `.btn img`, `.btnImage` — `width: 18px; height: 18px; object-fit: contain` (VSIX SVG/PNG; asset как есть, без filter-перекраски)

Контейнер `.list`: `gap: 2px` между плитками (см. элемент 38).

## Состояния (классы-варианты с метриками)
- `.btn:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.btnActive`, `.btnActive:hover`: `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` — иконка остаётся PRIMARY (не акцентная), без кольца/ring.
- `.tileDragging > .btn`: `opacity: 0.3` (тайл-«призрак» на исходной позиции во время drag).
- focus: отдельных стилей в модуле нет.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.btn` НЕТ (`activity-bar/ActivityBar.module.css:53-66`) — бокс ровно 32×32 (`:55-56`), глиф центрируется `display: grid; place-items: center` (`:57-58`); внешние отступы задаёт список: `.list { margin: 0; padding: 0; gap: 2px }` (`ActivityBar.module.css:40-45`), а вертикальный воздух колонки — `.bar { padding: var(--space-3) 0 }` = 12px сверху/снизу (`:10`) и `.bar { gap: var(--space-2) }` = 8 между группами (`:9`)

### Наша реализация

# 39 activity-tile — наша реализация
Файлы: `crates/shell/src/ui/activity_bar.rs:58-119` (`tile()` — сайдбарный бар), `crates/shell/src/ui/right_column.rs:34-122` (`rail_tile()` — рейлы правых карт); палитра `crates/theme/src/palette.rs`.

## Структура (gpui-дерево кратко)
```
div#<id> 32×32 flex center rounded RADIUS_SM cursor_pointer
  ├ tooltip(activity_label(id))            // если label известен
  └ иконка:
      phosphor_path(icon) → svg 18×18 .text_color(icon_color)
      иначе → div font "codicon" 18px, глиф codicon_glyph(icon)
```
- `tile()` (сайдбар): `on_mouse_down(Left)` → `on_click` → `ShellEvent::ActivityClicked(id)` → `sidebar_activity = id` + выход из Customize (root.rs:729-733).
- `rail_tile()` (правые карты): `on_mouse_down(Left)` → `ShellEvent::ToolPress` (клик-без-движения = активация, движение ≥4px = dnd; root.rs:4946-4972, 5015-5038); RMB → `OpenToolTabMenu` (меню Hide / Move to).

## Метрики (из кода, точные)
- Бокс: **32×32**, `rounded(m::RADIUS_SM)` = **8px**, flex items_center/justify_center.
- Иконка: svg **18×18**; codicon-фолбэк font-size **18px** в `tile()`, **16px** в `rail_tile()` (right_column.rs:74).
- Цвета (dark): базовый `p.text_muted` **#838aa0**; active — `p.text_primary` **#cfd4e2**.
- Hover: bg `p.bg_surface` **#3d3f51** @ alpha **0.5** + text_color → `p.text_primary`.
- Active: bg `p.accent_primary` **#89b4fa** @ alpha **0.16**, иконка text_primary.

## Отличия от original.md той же папки
1. **Сайдбарные плитки не перетаскиваются и без контекст-меню**: оригинал — onPointerDown drag + onContextMenu на каждой плитке; у нас drag (`ToolPress`) и RMB-меню есть только у `rail_tile()` правых карт и стрип-табов, `tile()` — чистый клик.
2. **Нет `.tileDragging`** (opacity 0.3 у исходной плитки во время drag) — исходная плитка визуально не меняется.
3. **Hover-перекраска не работает для Phosphor-svg**: `.hover(|s| s.text_color(primary))` ставит цвет на кнопку, но svg-ветка имеет собственный фиксированный `.text_color(icon_color)` — на hover перекрашивается только codicon-ветка. В `rail_tile()` hover вообще не меняет цвет иконки (только bg).
4. `rail_tile` codicon-фолбэк 16px vs 18px оригинала.
5. Нет img-ветки для VSIX-иконок (URL/data:) — только svg-ассет или codicon-глиф (см. 51).
6. Нет `aria-pressed`/`aria-label`/keyboard (Enter/Space); тултип есть.
7. Оригинальная обёртка `<li data-tile>` отсутствует — кнопка прямо в колонке (эквивалентно, т.к. `.list` в CSS без своих метрик кроме gap).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — плитка 32×32 своих паддингов не имеет, глиф центрируется flex (`crates/shell/src/ui/activity_bar.rs`, `fn tile`); внешние зазоры даёт `.list` gap 2

### Вердикты

## Цикл 3: MATCH

Плитка 32×32, radius sm, глиф 18, active accent 16% + text-primary.

## Цикл 4: DIVERGES

Ховер по АКТИВНОЙ плитке гасил accent (`.btnActive:hover` держит 16%) — **исправлено волной 7** в `activity_bar.rs` и `right_column.rs`. Остаётся: RMB/drag у сайдбарных плиток, `.tileDragging` opacity 0.3; перекраска svg по ховеру — ограничение gpui.

## Цикл 8: DIVERGES

Ховер над активной плиткой закрыт. Осталось: у сайдбарных плиток нет RMB и drag, нет `.tileDragging` 0.3, и ховер РЕЙЛА не поднимает цвет codicon (в баре поднимает) — расхождение между двумя копиями логики.

## Цикл 9: DIVERGES

Только Left (activity_bar.rs:122) против onPointerDown+onContextMenu; нет .tileDragging{opacity:.3}. Претензия ц.8 про ховер codicon ЛОЖНА: tool_glyph фиксирует text_color, hover родителя до глифа не доходит ни в баре, ни в рейле.

---

## 40. activity-customize-tile — **MATCH** (цикл 8)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:MATCH*

![оригинал](40-activity-customize-tile/original.png)
![наш](40-activity-customize-tile/ours.png)

### Оригинал

# 40 activity-customize-tile — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:131-148`, `ActivityBar.module.css` (`.btn`, `.btnActive`)

## JSX-структура (кратко, вложенность)
```
<button type="button"
        class="btn [btnActive]"
        aria-pressed={isActive}          // isActive = sidebarMode === "customize"
        aria-label="Customize"
        data-tooltip="Customize"
        onClick={isActive ? leaveCustomize() : openCustomize("settings")}>
  <ToolIcon icon="gear"/>                // встроенный Phosphor-токен "gear"
</button>
```
- Рендерится ТОЛЬКО в sidebar-баре (`fixedHead`), первым элементом при `align="top"`, последним при reverse.
- Не в `pinned[]`: нельзя перетащить, скрыть, переместить. Нет onPointerDown/onContextMenu — обычный onClick.
- Без обёртки `<li>` — кнопка прямо в `<nav>` (вне `.list`).

## Метрики (ИЗ CSS, точные значения)
Тот же `.btn`, что у элемента 39:
- `width: 32px; height: 32px; display: grid; place-items: center`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-muted); font: inherit; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`
- SVG-иконка (ToolIcon default): 18×18, `fill="currentColor"`

## Состояния (классы-варианты с метриками)
- hover: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.btnActive` (customize открыт): `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)`
- Пока customize активен, остальные плитки бара НЕ подсвечиваются (`customizeOwnsBar`).

## Дополнение атрибутов (цикл 10)

- отступы: та же `.btn`-правило — padding/margin нет, бокс 32×32, центровка grid/place-items (`activity-bar/ActivityBar.module.css:53-58`). Отличие от обычной плитки: gear стоит ПРЯМЫМ ребёнком `.bar` (`ActivityBar.tsx:131-148`), а не внутри `.list`, поэтому зазор до списка задаёт `.bar { gap: var(--space-2) }` = 8 (`ActivityBar.module.css:9`), а не 2px `.list`-гэп; верхний отступ до края колонки — `.bar { padding: var(--space-3) 0 }` = 12 (`:10`)

### Наша реализация

# 40 activity-customize-tile — наша реализация
Файлы: `crates/shell/src/ui/activity_bar.rs:149-155` (внутри `activity_bar()`, тот же `tile()` 58-119); обработчик `crates/shell/src/root.rs:1418-1420` (`ToggleCustomize`), передача `customize_active` — root.rs:5219-5224.

## Структура (gpui-дерево кратко)
```
activity_bar():
  └ tile("customize", "gear", customize_active, on_gear)   // ПЕРВЫЙ child после probe_area
      div#customize 32×32 rounded 8 → svg icons/gear.svg 18×18
```
- Клик → `ShellEvent::ToggleCustomize` (тумблер: открыт → закрыть, закрыт → открыть; ленивая подгрузка prefs хоста).
- Пока `customize_open`: в бар передаётся `active=None` — горит только gear, плитки тулов гаснут (root.rs:5217-5223).
- Не входит в `pinned[]`, не перетаскивается, без RMB — фиксированная системная плитка. Только в сайдбарном баре (в `rail()` правых карт её нет).

## Метрики (из кода, точные)
Идентичны элементу 39 (`tile()`):
- **32×32**, rounded **8px** (`RADIUS_SM`), иконка svg **18×18** (`icons/gear.svg`, vendored Phosphor).
- Базовый цвет `p.text_muted` #838aa0; hover: bg `p.bg_surface` #3d3f51 @0.5 + `p.text_primary` #cfd4e2.
- Active (`customize_open`): bg `p.accent_primary` #89b4fa @0.16, иконка #cfd4e2.
- Тултип «Customize» (`activity_label("customize")`).

## Отличия от original.md той же папки
1. Оригинал: `onClick = isActive ? leaveCustomize() : openCustomize("settings")`; у нас единый toggle `ToggleCustomize` — семантика совпадает (открытие всегда на последней выбранной подпанели `customize_panel`, не форс «settings»).
2. Hover-перекраска иконки на svg-ветке не работает (тот же дефект, что в 39, п.3) — gear на hover получает bg, но остаётся muted.
3. Нет `aria-pressed`/`aria-label`.
4. Позиция совпадает (первый элемент сверху); reverse-варианта у сайдбарного бара нет — не нужен (gear только в sidebar, sidebar всегда align=top).

## Дополнение атрибутов (цикл 10)

- отступы: своих паддингов у плитки нет — размер задан жёстко 32×32, содержимое центрировано flex (`crates/shell/src/ui/activity_bar.rs`, `fn tile`); внешние отступы даёт `.list` (gap 2) и `py SPACE_3` бара
- гэпы: N/A: гэпы — внутри плитки один ребёнок (глиф), gap нечему разделять

### Вердикты

## Цикл 3: DIVERGES

Ховер плитки должен перекрашивать и svg-иконку (у нас цвет svg фиксирован — ограничение gpui: `svg().text_color` не наследует hover-состояние). Отклонение + идея: рисовать иконку через group_hover-вариант. Волна 7.

## Цикл 4: DIVERGES

То же по gear-плитке Customize (ховер над активной) — **исправлено волной 7**; перекраска svg — ограничение gpui.

## Цикл 8: MATCH

Gear-плитка Customize 1:1.

---

## 41. activity-drop-placeholder — **DIVERGES** (цикл 11)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц10:DIVERGES, ц11:DIVERGES*

![оригинал](41-activity-drop-placeholder/original.png)
![наш](41-activity-drop-placeholder/ours.png)

### Оригинал

# 41 activity-drop-placeholder — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityBar.tsx:150-152`, `ActivityBar.module.css` (`.dropPlaceholder`)

## JSX-структура (кратко, вложенность)
```
<li class="dropPlaceholder" aria-hidden="true"/>
```
- Вставляется в `<ul class="list">` на позицию `overIndex` (перед плиткой i, либо в конец при `overIndex === pinned.length`), когда `dragState.overSlot === slot`.
- Пустой элемент, без содержимого.

## Метрики (ИЗ CSS, точные значения)
`.dropPlaceholder`:
- `width: 32px; height: 32px` (повторяет форму живой плитки)
- `border-radius: var(--radius-sm)`
- `border: 1px dashed color-mix(in srgb, var(--accent-primary) 70%, transparent)`
- `background: color-mix(in srgb, var(--accent-primary) 14%, transparent)`
- transition/анимации: нет; позиционирование: обычный flex-item в `.list` (gap 2px)

## Состояния (классы-варианты с метриками)
Одно состояние; появляется/исчезает только через вставку/удаление из DOM во время drag.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.dropPlaceholder` НЕТ (`activity-bar/ActivityBar.module.css:23-29`) — бокс ровно 32×32 (`:24-25`) в размер живой плитки, рамка 1px dashed (`:27`); внешние зазоры даёт `.list { gap: 2px }` (`:45`); вставляется в `<ul class="list">` на позицию `overIndex` (`ActivityBar.tsx:150-152`)

### Наша реализация

# 41 activity-drop-placeholder — наша реализация
Файлы: `crates/shell/src/ui/activity_bar.rs` (`fn drop_placeholder`, вставка в `list`), `crates/shell/src/root.rs` (`tool_drag_over_index(PanelSlot::Sidebar)` — индекс вставки), hit-тест дроп-зон по probe-bounds там же.

## Структура (gpui-дерево кратко)
```
list (flex-col, items-center, gap 2)
├─ [drop_index == i] drop_placeholder      ← пустой бокс 32×32
├─ tile(...)
└─ [drop_index == entries.len()] drop_placeholder   ← вставка в конец
```
Пустой элемент без содержимого, как `<li class="dropPlaceholder">` оригинала.

## Метрики (из кода, точные)
- 32×32, `flex-shrink: 0` — повторяет форму живой плитки.
- radius RADIUS_SM 8.
- Рамка 1px **dashed**, цвет accent-primary #89b4fa при alpha 0.7.
- Фон accent-primary при alpha 0.14.
- Позиционирование — обычный flex-item в `.list` (gap 2), собственных отступов нет.

## Отличия от original.md той же папки
1. Индекс вставки берётся из общей drag-модели, а не из позиционного расчёта по `clientY` (см. элемент 38) — для вертикального бара это значит, что место вставки может отличаться от того, куда реально наведён курсор.
2. Кадра состояния drag в досье пока нет — вердикт по коду.

### Вердикты

## Цикл 3: DIVERGES

Drag-ghost 28×28 квадрат и dashed-плейсхолдеры (32×32 вертикальный, 36×24 горизонтальный) не реализованы. Волна 7.

## Цикл 4: DIVERGES

Drop-плейсхолдер 32×32 (dashed accent 70% + фон accent 14%) не реализован. Волна 8.

## Цикл 8: DIVERGES

Drop-плейсхолдер плитки не реализован: 32×32, r-sm, 1px dashed accent 70%, фон accent 14%.

## Цикл 9: DIVERGES

Не реализован (grep dashed = 0). Оригинал: 32x32, radius-sm, 1px dashed accent 70%, фон accent 14%.

## Цикл 10: DIVERGES

Реализован: `activity_bar::drop_placeholder` — 32×32, radius-sm 8, рамка 1px dashed
accent-primary 70%, фон accent-primary 14%, обычный flex-item в `.list` (gap 2),
вставляется по `overIndex` и в конец списка при `overIndex == entries.len()`.
Индекс приходит из `tool_drag_over_index(PanelSlot::Sidebar)`.

Осталось: позиционный расчёт `overIndex` по clientY (элемент 38) — сейчас индекс даёт
общая drag-модель; кадра со состоянием drag нет.

## Цикл 11: DIVERGES

Плейсхолдер стал ДОСТИЖИМ: плитки рейла теперь шлют `ToolDragOverTab`, и `tool_drag_over_index(Sidebar)` возвращает индекс (ревью ц.11 показало, что вставка не рисовалась ни при каком драге).

Осталось: позиционный `overIndex` по clientY; кадр состояния drag.

---

## 42. activity-picker-dots-trigger — **MATCH** (цикл 9)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:MATCH*

![оригинал](42-activity-picker-dots-trigger/original.png)
![наш](42-activity-picker-dots-trigger/ours.png)

### Оригинал

# 42 activity-picker-dots-trigger — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityPicker.tsx:126-138` (+ обёртка 176-181), `ActivityBar.module.css` (`.pickerAnchor`, `.pickerAnchorInline`, `.picker`)

## JSX-структура (кратко, вложенность)
```
<div class="pickerAnchor | pickerAnchorInline" ref={anchorRef}>   // anchor для clamp-позиционирования
  <button type="button"
          class="picker"
          data-tooltip="Add or remove items"
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-label="Add or remove items"
          onClick={e.stopPropagation(); toggle open}>
    <i class="codicon codicon-more" aria-hidden="true"/>
  </button>
  {menu}   // портал-listbox, элемент 44
</div>
```
- variant="dots" (default). Место в DOM: после `.list` при align="top", перед — при align="bottom" (тогда popDirection="down").

## Метрики (ИЗ CSS, точные значения)
`.pickerAnchor`:
- `position: relative; display: flex; justify-content: center; width: 100%`

`.pickerAnchorInline` (inline-вариант для PanelPlaceholder):
- `position: relative; display: inline-flex` (без width:100%)

`.picker` (общий селектор с `.btn`):
- `width: 32px; height: 32px; display: grid; place-items: center`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-muted); font: inherit; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`
- `.picker :global(.codicon)` — `font-size: 18px; line-height: 1`
- `.picker img` — `width: 18px; height: 18px; object-fit: contain`

## Состояния (классы-варианты с метриками)
- `.picker:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- Открытое меню НЕ подсвечивает триггер (нет active-класса), только `aria-expanded="true"`.
- Popup offset от триггера: `POPUP_OFFSET_PX = 6` (TSX-константа, для clampToViewport).

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.picker` НЕТ — кнопка делит правило с `.btn`: 32×32, `display: grid; place-items: center` (`activity-bar/ActivityBar.module.css:53-66`); обёртка `.pickerAnchor { position: relative; display: flex; justify-content: center; width: 100% }` тоже без padding/margin (`:104-109`), инлайн-вариант `.pickerAnchorInline` — только `position: relative; display: inline-flex` (`:114-117`, «No margin-top» в комментарии `:111-113`); вертикальный зазор от списка даёт `.bar { gap: var(--space-2) }` = 8 (`:9`)

### Наша реализация

# 42 activity-picker-dots-trigger — наша реализация
Файлы: `crates/shell/src/ui/activity_bar.rs:165-196` (сайдбарный бар, `div#activity-dots`), `crates/shell/src/ui/slot_panel.rs:118-147` (`dots()` — горизонтальные стрипы), `crates/shell/src/ui/right_column.rs:146-243` (`rail-dots-*` — рейлы правых карт); обработчик `root.rs:1413-1416` (`OpenToolPicker`).

## Структура (gpui-дерево кратко)
```
div#activity-dots (или #dots-<slot>, #rail-dots-<slot>)
  N×N flex center rounded, tooltip "Add or remove items"
  └ div font "codicon" → "\u{ea7c}" (codicon-more «…»)
on_mouse_down(Left): stop_propagation → ShellEvent::OpenToolPicker(slot, x, y, up)
```
- `x,y` — координаты курсора события; меню (элемент 44) открывается от этой точки, `up` — раскрытие вверх (стрипы нижних панелей, bottom-рейл).
- В bottom-рейле (`rail(bottom=true)`) «…» стоит НАД плитками (DOM {picker, list}) — зеркало оригинального reverse.

## Метрики (из кода, точные)
Три варианта:
- Сайдбарный бар: бокс **32×32**, rounded **8px** (литерал), codicon **18px**.
- Стрип слота (`dots()`): бокс **24×24**, rounded `RADIUS_SM` 8px, codicon **15px** (через `icon::codicon` — внутр. бокс 16×16).
- Рейл правой карты: бокс **32×32**, rounded 8px, codicon **15px**.
- Цвета везде: `p.text_muted` #838aa0; hover: bg `p.bg_surface` #3d3f51 @0.5 + `p.text_primary` #cfd4e2.
- Тултип «Add or remove items» — у всех трёх.

## Отличия от original.md той же папки
1. **Размер иконки**: оригинал везде 18px в кнопке 32×32; у нас 18px только в сайдбарном баре, в стрипах 15px/24×24, в рейлах 15px/32×32.
2. **Нет anchor-обёртки** (`.pickerAnchor` width:100%/`.pickerAnchorInline`): меню позиционируется от координат клика с клампом, а не от rect триггера с flip/re-measure.
3. Offset поповера: оригинал `POPUP_OFFSET_PX = 6` от rect триггера; у нас `±6` от точки курсора (tool_picker.rs:69-73) — визуально меню может встать в другом месте.
4. Нет `aria-haspopup`/`aria-expanded`; открытый пикер не помечает триггер (в оригинале тоже только aria).
5. Три независимые копии кнопки (activity_bar / slot_panel / right_column) вместо одного компонента `ActivityPicker variant="dots"`.

## Дополнение атрибутов (цикл 10)

- отступы: своих паддингов нет — бокс 32×32 с центрированием глифа (`crates/shell/src/ui/activity_bar.rs`, блок `activity-dots`)
- гэпы: N/A: гэпы — один ребёнок (глиф «…»)
- шрифты: глиф codicon-more `\u{ea7c}` кеглем 18 через `font_family("codicon")` (`crates/shell/src/ui/activity_bar.rs:216-218`); в горизонтальном стрипе — `codicon(MORE, 18.0)` (`crates/shell/src/ui/slot_panel.rs:168`); в рейлах правых карт — `font_family("codicon").text_size(px(18.0))` (`crates/shell/src/ui/right_column.rs:197`); font-weight нигде не задан; текста в кнопке нет. Оригинал: `.picker :global(.codicon) { font-size: 18px; line-height: 1 }` (`ActivityBar.module.css:68-69`) — совпадает

### Вердикты

## Цикл 3: DIVERGES

«…» пикер: в стрипе был text-primary 8% ховер и text-muted база — приведено к BottomTabBar (bg-surface 50%). В рейле 32×32/18 — совпадает.

## Цикл 4: DIVERGES

«…» в стрипе был 24×24 с глифом 15 вместо `.picker` 32×32/18 — **исправлено волной 7** (именно из-за этого стрип был на 8 лог. px ниже, см. 48). В баре и рейле 32×32/18 уже совпадали.

## Цикл 8: DIVERGES

Бар и рейл 32×32/глиф 18 — закрыто. **Глиф «…» в стрипе поднят с 15 до 18 волной 15.**

## Цикл 9: MATCH

Пиксельно: чернила «…» 12.00x2.40, центр (28.00,142.40) против 11.20x3.20 и (27.60,142.00) — разница на уровне растеризации; кегль 18 у обеих.

---

## 43. activity-picker-open-tool-pill — **MATCH** (цикл 8)

*История: ц3:MATCH, ц4:MATCH, ц8:MATCH*

![оригинал](43-activity-picker-open-tool-pill/original.png)
![наш](43-activity-picker-open-tool-pill/ours.png)

### Оригинал

# 43 activity-picker-open-tool-pill — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityPicker.tsx:115-125`, `kamin-ide/src/renderer/components/panel-placeholder/PanelPlaceholder.module.css` (`.trigger`)

## JSX-структура (кратко, вложенность)
```
<div class="pickerAnchorInline" ref={anchorRef}>       // ActivityBar.module.css
  <button type="button"
          class="trigger"                              // PanelPlaceholder.module.css
          aria-haspopup="listbox"
          aria-expanded={open}
          onClick={stopPropagation; toggle open}>
    <span>Open Tool</span>
    <i class="fas fa-chevron-down" aria-hidden="true"/>   // FontAwesome
  </button>
  {menu}   // тот же портал-listbox (элемент 44)
</div>
```
- variant="openTool"; используется в PanelPlaceholder (пустая панель). Обёртка inline (`pickerAnchorInline`), чтобы родитель управлял вертикальным размещением.

## Метрики (ИЗ CSS, точные значения)
`.trigger`:
- `display: inline-flex; align-items: center; gap: var(--space-2)`
- `padding: var(--space-1) var(--space-3)` (вертикаль var(--space-1), горизонталь var(--space-3))
- `background: color-mix(in srgb, var(--accent-primary) 16%, transparent)`
- `color: var(--text-primary)` (текст PRIMARY, не accent — accent-on-transparent читался блёкло)
- `border: none; border-radius: var(--radius-sm)`
- `font-size: var(--fs-sm)`
- `margin-top: var(--space-1)`
- `transition: background var(--transition-fast)`
- `.trigger > i { font-size: 10px }` (шеврон)

## Состояния (классы-варианты с метриками)
- `.trigger:hover`: `background: color-mix(in srgb, var(--accent-primary) 26%, transparent)`
- Открытое меню: только `aria-expanded="true"`, визуального класса нет.

### Наша реализация

# 43 activity-picker-open-tool-pill — наша реализация
Файлы: `crates/shell/src/ui/slot_panel.rs:150-183` (`open_tool_btn()`), передаётся как `extra` в `crates/shell/src/ui/panel_placeholder.rs:84-119` (`panel_placeholder_ex`).

## Структура (gpui-дерево кратко)
```
panel_placeholder_ex(label, hint, SlotIcon, extra = open_tool_btn):
  колонка по центру: slot_glyph → label → hint → пилюля

open_tool_btn:
  div#opentool-<slot> inline-строка
    ├ "Open Tool"
    └ icon::fa("\u{f078}" fa-chevron-down, 10px)   // FontAwesome solid, бокс 16×16
on_mouse_down(Left) → ShellEvent::OpenToolPicker(slot, cursor_x, cursor_y, up)
```
- Открывает тот же пикер (элемент 44), что и «…»; используется во всех пустых слотах (`slot_panel` без body).

## Метрики (из кода, точные)
- Паддинг: `px(SPACE_3)` **12px** гориз., `py(SPACE_1)` **4px** верт.; `mt(SPACE_1)` **4px**.
- `gap(SPACE_2)` **8px** между текстом и шевроном; rounded `RADIUS_SM` **8px**.
- Фон: `p.accent_primary` #89b4fa @ alpha **0.16**; hover @ **0.26**.
- Текст: `p.text_primary` #cfd4e2, `FS_SM` **12px**.
- Шеврон: FontAwesome solid (weight 900) **10px** в боксе 16×16.

## Отличия от original.md той же папки
1. Метрики совпадают 1:1 (padding 4/12, gap 8, mt 4, radius 8, fs-sm, accent 16%→26%, текст primary, шеврон 10px).
2. Открытие: оригинал — тот же `ActivityPicker` с anchor-обёрткой `.pickerAnchorInline`, clamp от rect кнопки; у нас меню позиционируется от координат клика (см. 42 п.2-3).
3. Нет `aria-haspopup`/`aria-expanded`.
4. Шеврон у нас в flex-боксе 16×16 (`icon::fa`) — в оригинале голый `<i>` 10px; на геометрию строки не влияет (высота задаётся паддингом).

### Вердикты

## Цикл 3: MATCH

Tooltip плиток совпал по тексту и задержке.

## Цикл 4: MATCH

Пилюля «Open Tool» плейсхолдера 1:1 (padding 4/12, gap 8, mt 4, radius 8, fs-sm, accent .16→.26, шеврон fa 10).

## Цикл 8: MATCH

Пилюля «Open Tool» 1:1.

---

## 44. activity-picker-menu — **DIVERGES** (цикл 11)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц10:DIVERGES, ц11:DIVERGES*

![оригинал](44-activity-picker-menu/original.png)
![наш](44-activity-picker-menu/ours.png)

### Оригинал

# 44 activity-picker-menu — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityPicker.tsx:140-174`, `ActivityBar.module.css` (`.menu`, `.menuPortal`, `.menuLabel`, `.menuItem`, `.menuItemImage`, `.menuLabelText`)

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<ul ref={menuRef} class="menu menuPortal" role="listbox"
    style="left:{pos.left}px; top:{pos.top}px; visibility: visible|hidden">
  <li class="menuLabel">Tools</li>
  {activityRegistry.map(it =>
    <li key={it.id}>
      <button type="button" class="menuItem"
              onClick={isPinned ? unpinFromPanel : pinToPanel; close}>
        <ToolIcon icon={it.icon} imageClassName="menuItemImage"/>
        <span class="menuLabelText">{it.label}</span>
        {isPinned && <i class="codicon codicon-check" aria-hidden="true"/>}
      </button>
    </li>)}
</ul>
```
- Позиционирование: измерение trigger rect + menu rect → `clampToViewport({side: popDirection==="up" ? "top" : "bottom", offset: 6})` — flip+shift, чтобы не вылезло за окно. Стартует `visibility:hidden`, показывается после первого замера (useLayoutEffect). Re-measure на window resize и scroll (capture).
- Закрытие: outside mousedown (capture, с проверкой menuRef — портал не потомок anchor) + Escape.

## Метрики (ИЗ CSS, точные значения)
`.menu`:
- `min-width: 220px`
- `background: var(--bg-surface)`
- `border: 1px solid var(--divider-soft)`
- `border-radius: var(--radius-md)`
- `box-shadow: var(--shadow-dropdown)`
- `list-style: none; margin: 0; padding: var(--space-1)`
- `z-index: var(--z-dropdown)`
- `display: flex; flex-direction: column; gap: 1px`

`.menuPortal`:
- `position: fixed`
- `max-height: calc(100vh - 16px); max-width: calc(100vw - 16px)`
- `overflow-y: auto`

`.menuLabel` (заголовок «Tools»):
- `padding: var(--space-1) var(--space-3)`
- `font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em`
- `color: var(--text-muted)`

`.menuItem`:
- `display: flex; align-items: center; gap: var(--space-2); width: 100%`
- `padding: var(--space-2) var(--space-3)`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-primary); font: inherit; font-size: var(--fs-sm)`
- `text-align: left; cursor: pointer`

`.menuItemImage` (img-ветка ToolIcon): `width: 18px; height: 18px; object-fit: contain`
`.menuLabelText`: `flex: 1`

## Состояния (классы-варианты с метриками)
- `.menuItem:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent)`
- Запиненный пункт: галка `codicon-check` в конце строки (спец-стилей нет, наследует цвет пункта).
- transition/анимаций появления нет.

### Наша реализация

# 44 activity-picker-menu — наша реализация
Файлы: `crates/shell/src/ui/tool_picker.rs:55-150` (`tool_picker()`); рендер в overlay-окне `crates/shell/src/overlay.rs:998-999`; закрытие/состояние `root.rs:1403-1417` (`PinTool`/`UnpinTool`/`OpenToolPicker`/`CloseToolPicker`); модель `crates/shell/src/activity.rs` (`BUILTIN_ACTIVITIES`, `dyn_tools_list`).

## Структура (gpui-дерево кратко)
```
div#tool-picker .occlude .absolute (x,y кламп в вьюпорт; up → раскрытие вверх)
  w 220, flex col, p SPACE_1, rounded RADIUS_MD, bg-surface,
  border 1px text_primary@0.06, shadow dropdown_shadow()
  ├ hit_area()                                  // hit-регион overlay-окна
  ├ div "Tools"  (px SPACE_2, py 4, FS_XS, text_muted)
  └ строка × (BUILTIN_ACTIVITIES + dyn_tools_list):
      div#tp-<id> flex gap SPACE_2, px SPACE_2, py 6, rounded RADIUS_SM
        ├ tool_icon(icon)                       // svg 15×15 или codicon 15px
        ├ div flex_1 label
        └ [pinned] codicon check 13px accent_primary
```
- Клик по строке: pinned → `UnpinTool`, иначе `PinTool` (+активация в модели); оба хендлера ставят `tool_picker = None` — меню закрывается.
- Позиционирование: `est_h = 40 + 34·N`; x кламп `[8, vw-220-8]`; y: `up ? y-est_h-6 : y+6`, кламп по вьюпорту. Скрим-закрытие (клик-мимо, per-pixel hit-test overlay) — в main-окне.

## Метрики (из кода, точные)
- Ширина: фикс **220px** (`PICKER_W`); margin от краёв **8px**.
- Контейнер: `p(SPACE_1)` 4px, rounded `RADIUS_MD` **12px**, bg `p.bg_surface` **#3d3f51**, border 1px `p.text_primary`@**0.06**, shadow `0 8px 24px rgba(0,0,0,0.45)` (alpha-mode).
- Заголовок «Tools»: px **8** / py **4**, `FS_XS` **11px**, `p.text_muted` #838aa0.
- Строка: gap **8**, px **8**, py **6**, rounded **8px**, `FS_SM` **12px**, цвет `p.text_secondary` **#adb3c7**; hover: bg `p.text_primary`@**0.08** + text `p.text_primary` #cfd4e2.
- Иконка **15×15** (svg) / 15px codicon; галка codicon-check **13px** цвета `p.accent_primary` #89b4fa.

## Отличия от original.md той же папки
1. **Паддинги строк**: оригинал `space-2 space-3` (8/12); у нас 6/8 — меню плотнее и уже по контенту.
2. **Иконка 15px** vs 18px оригинала; **галка 13px accent** vs наследуемый цвет пункта.
3. **Цвет строки**: `text_secondary` vs `--text-primary` оригинала (hover совпадает: primary + 8~10% подложка; у нас 8%, оригинал 10%).
4. **Заголовок**: без `text-transform: uppercase` и `letter-spacing: 0.04em`; px 8 vs 12.
5. **w фикс 220** vs `min-width: 220` (длинные label обрезаются/переносятся, не расширяют меню); нет `max-height + overflow-y` (только кламп позиции — при огромном списке меню вылезет).
6. Нет `gap: 1px` между строками (строки вплотную; визуально скрыто паддингами).
7. Позиционирование от точки клика с клампом, без двухпроходного замера/`visibility:hidden`/re-measure на resize и без flip стороны.
8. Hover-перекраска svg-иконки не работает (фикс. `.text_color` на svg — тот же дефект, что 39 п.3).
9. Закрытие по Escape не реализовано для пикера (только клик-мимо через скрим).
10. Роли `listbox`/`aria` отсутствуют.

## Дополнение атрибутов (цикл 10)

- шрифты: заголовок «TOOLS» text_size FS_XS = 11 (`crates/shell/src/ui/tool_picker.rs:102`); строки тулов text_size FS_SM = 12 (`tool_picker.rs:132`); font-weight нигде не задан; иконка строки — svg 18×18 либо `codicon(..., 16.0)` (`tool_picker.rs:40-41,47`); галка pinned — `codicon(CHECK, 13.0)` (`tool_picker.rs:149`). Оригинал: `.menuLabel { font-size: var(--fs-xs) }` 11 + uppercase, `.menuItem { font-size: var(--fs-sm) }` 12 (`activity-bar/ActivityBar.module.css:150-156,158-172`) — кегли совпадают; uppercase у нас применён (`tool_picker.rs:104` — литерал «TOOLS» уже заглавными)

### Вердикты

## Цикл 3: DIVERGES

tool_picker: фиксированная ширина 220 вместо min-width, нет gap 1 и max-height, метки не uppercase, пункты padding 8/8 вместо 8/12, база text-secondary вместо text-primary, ховер 8% вместо 10%, иконки 15 вместо 16/18. Исправлено волной 6.

## Цикл 4: DIVERGES

Меню пикера — волна 6 закрыла min-width/gap/max-height/метку/паддинги/иконки. Остаётся: `overflow_hidden` вместо своего скролла, иконка пункта text-secondary вместо наследуемой primary, галка 13px accent вместо ~16 primary, кламп X по константе 220 при `min_w`, нет Escape. Волна 8.

## Цикл 8: DIVERGES

Меню пикера: `overflow_hidden` вместо `overflow-y: auto`, иконка пункта text-secondary вместо наследуемой primary, галка 13 + accent вместо 16 с наследованием, позиция по `est_h` вместо измеренной, нет Escape.

## Цикл 9: DIVERGES

Совпало: ширина 220.00, шаг 35.20, py8/px12, хедер (131,138,160). ОСТАЛОСЬ: иконка secondary вместо primary; галка 13 против 16; overflow_hidden против auto; est_h считает 34/строку при факте 35.2 (при 9 тулах ~11px); нет Escape.

## Цикл 10: DIVERGES

Закрыто: иконка пункта и галка наследуют цвет строки (text-primary вместо
text-secondary/accent), кегль кодикона 16 (модуль его не переопределяет → база
`.codicon{16px}`), `overflow-y: auto` вместо обрезки, шаг строки в оценке высоты
35.2 вместо 34 (промах ~11px на девяти тулах), Escape закрывает пикер.

Осталось: подтвердить кадром (emit toolPicker без координат рисует пикер вне
видимой области — нужен клик по «…» с реальными bounds).

## Цикл 11: DIVERGES

Закрыто дополнительно: `max-width: calc(100vw − 16px)` (длинное имя contributed-тула уводило меню за правый край).

Осталось: кадр состояния; `SVG_TOKENS` пикера разошёлся с `phosphor_path` (нет «projects», есть «gear») — латентно, пока встроенные тулы шлют «folders».

---

## 45. activity-context-menu — **DIVERGES** (цикл 11)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц10:DIVERGES, ц11:DIVERGES*

![оригинал](45-activity-context-menu/original.png)
![наш](45-activity-context-menu/ours.png)

### Оригинал

# 45 activity-context-menu — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityContextMenu.tsx:132-169`, `ActivityContextMenu.module.css` (`.menu`, `.item`, `.itemLabel`, `.itemMoveTo`, `.chevron`)

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<div ref={rootRef} class="menu" role="menu"
     style="left:{pos.left}px; top:{pos.top}px; visibility: visible|hidden">
  <button type="button" role="menuitem" class="item"
          onMouseEnter={closeSubmenu}
          onClick={unpinFromPanel; close}>
    <i class="codicon codicon-eye-closed"/>
    <span class="itemLabel">Hide</span>
  </button>
  <button type="button" role="menuitem" class="item itemMoveTo"
          aria-haspopup="menu" aria-expanded={submenuOpen}
          onMouseEnter={openSubmenu} onClick={toggleSubmenu}>
    <i class="codicon codicon-arrow-right"/>
    <span class="itemLabel">Move to</span>
    <i class="codicon codicon-chevron-right chevron"/>
  </button>
  {submenuOpen && createPortal(<ul class="submenu">…</ul>, body)}   // элемент 46
</div>
```
- Открывается у курсора (anchor = точка x/y нулевого размера), `clampToViewport(side:"bottom", offset: MENU_OFFSET_PX = 0)`; `visibility:hidden` до первого замера.
- Закрытие: outside mousedown (capture), Escape, любой scroll (capture), window blur.

## Метрики (ИЗ CSS, точные значения)
`.menu` (общий селектор `.menu, .submenu`):
- `position: fixed; z-index: var(--z-dropdown)`
- `min-width: 180px`
- `background: var(--bg-surface)`
- `border: 1px solid var(--divider-soft)`
- `border-radius: var(--radius-md)`
- `box-shadow: var(--shadow-dropdown)`
- `list-style: none; margin: 0; padding: var(--space-1)`
- `display: flex; flex-direction: column; gap: 1px`
- `max-height: calc(100vh - 16px); max-width: calc(100vw - 16px); overflow-y: auto`

`.item`:
- `display: flex; align-items: center; gap: var(--space-2); width: 100%`
- `padding: var(--space-2) var(--space-3)`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-primary); font: inherit; font-size: var(--fs-sm)`
- `text-align: left; cursor: pointer`

`.itemLabel`: `flex: 1`
`.chevron`: `font-size: 12px; color: var(--text-muted)`

## Состояния (классы-варианты с метриками)
- `.item:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.itemMoveTo[aria-expanded="true"]` (сабменю открыто): `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` — «хлебная крошка» пока открыто сабменю.
- transition/анимаций нет.

### Наша реализация

# 45 activity-context-menu — наша реализация
Файлы: `crates/shell/src/overlay.rs:1121-1238` (`tool_tab_menu()` — корневое меню), рендер в overlay-окне (overlay.rs:974 передаёт `tool_menu_sub`); события `root.rs:1185-1198` (`OpenToolTabMenu`/`CloseToolTabMenu`/`ToolMenuSub`); триггеры: RMB по табу стрипа `slot_panel.rs:72-84`, RMB по плитке рейла `right_column.rs:105-116`.

## Структура (gpui-дерево кратко)
```
div#tool-tab-menu .occlude .absolute (x,y кламп; est_h 92)
  min_w 180, flex col, gap 1, p SPACE_1, rounded RADIUS_MD,
  bg-surface, border 1px text_primary@0.06, hit_area()
  ├ div#ttm-hide:   codicon eye-closed 14px muted + "Hide"
  │     клик → UnpinTool(slot,id) + CloseToolTabMenu
  └ div#ttm-moveto: codicon arrow-right 14px muted + "Move to" + chevron-right 12px
        on_hover(true) → ToolMenuSub(true); открытое → bg accent@0.16
```
- Закрытие: скрим main-окна (клик-мимо через per-pixel hit-test overlay) + `close_popovers_except("ttab")` при открытии других поповеров.

## Метрики (из кода, точные)
- `MENU_W` (min-w) **180px**, кламп-маржин **8px**, оценка высоты **92px**.
- Контейнер: `p(SPACE_1)` 4, `gap 1px`, rounded `RADIUS_MD` **12px**, bg `p.bg_surface` #3d3f51, border 1px `p.text_primary`@0.06.
- Пункт: gap `SPACE_2` **8**, px `SPACE_3` **12**, py `SPACE_2` **8**, rounded `RADIUS_SM` **8px**, `FS_SM` **12px**, текст `p.text_primary` #cfd4e2.
- Hover: bg `p.text_primary`@**0.10**.
- Иконки пунктов: codicon **14px** `p.text_muted` #838aa0; шеврон **12px** muted.
- «Move to» при открытом сабменю: bg `p.accent_primary` #89b4fa @**0.16**.

## Отличия от original.md той же папки
1. **Нет box-shadow**: `box_style` не вызывает `.shadow()` (`--shadow-dropdown` оригинала отсутствует) — меню без тени.
2. **Иконки muted**, в оригинале `<i>` наследует цвет пункта (`--text-primary`).
3. «Move to» открывается ТОЛЬКО hover'ом; клик не тогглит (оригинал: onClick=toggle + onMouseEnter). Ховер «Hide» НЕ закрывает сабменю (оригинал закрывает).
4. Закрытие: нет Escape, нет закрытия по scroll (capture) и window blur — только клик-мимо.
5. Триггеры покрывают стрип-табы и рейлы правых карт, но НЕ плитки сайдбарного бара (у `tile()` нет RMB — см. 39 п.1); оригинал вешает меню на все плитки.
6. Нет `max-height/max-width + overflow-y`; est_h 92 — только для клампа позиции.
7. Нет ролей `menu`/`menuitem`, `aria-haspopup`/`aria-expanded`.
8. Паддинги/gap/радиусы/hover 10% — совпадают с оригиналом 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: пункты «Hide» и «Move to» — text_size FS_SM = 12 (`crates/shell/src/overlay.rs:1199,1228`); font-weight не задан; глифы пунктов — codicon кеглем 14 (`overlay.rs:1210-1211`, `:1239-1240`); chevron «▸» — codicon кеглем 12 (`overlay.rs:1247-1248`)

### Вердикты

## Цикл 3: DIVERGES

Подменю «Move to ▸» — тень и якорь не по dropdown-рецепту. Волна 7.

## Цикл 4: DIVERGES

У меню Hide / Move to не было тени — **исправлено волной 7** (`dropdown_shadow()` в `box_style`). Остаётся: иконки пунктов muted вместо primary, нет max-height/Escape/закрытия по скроллу, ховер «Hide» не закрывает сабменю, у сайдбарных плиток нет RMB. Волна 8.

## Цикл 8: DIVERGES

Тень закрыта. Осталось: иконки пунктов 14 + muted вместо 16 с наследованием primary, нет max-height/overflow/Escape, ховер «Hide» не закрывает сабменю.

## Цикл 9: DIVERGES

overlay.rs:1202,1232 кегль 14 + text_muted против 16 + primary; шеврон 12 против фактических 16; нет max_h/overflow-y:auto; нет Escape; у ttm-hide нет on_hover -> сабменю не закрывается.

## Цикл 10: DIVERGES

Закрыто: глифы пунктов 16 и цвет наследуется от строки (было 14 + text-muted),
шеврон 12 (единственное место, где модуль кегль задаёт), `max-height: 100vh − 16` +
`overflow-y: auto`, Escape закрывает меню вместе с сабменю, ховер по «Hide» гасит
сабменю (раньше открытое сабменю висело).

Осталось: якорь сабменю (элемент 46) и кадр состояния.

## Цикл 11: DIVERGES

Закрыто дополнительно: `max-width: calc(100vw − 16px)`; `tool_menu_sub` сбрасывается при КАЖДОМ открытии меню (клик-мимо гасил только само меню, и следующий RMB показывал сабменю у старого якоря).

Осталось: якорь сабменю (элемент 46); кадр состояния.

---

## 46. activity-context-submenu — **DIVERGES** (цикл 9)

*История: ц3:MATCH, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](46-activity-context-submenu/original.png)
![наш](46-activity-context-submenu/ours.png)

### Оригинал

# 46 activity-context-submenu — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityContextMenu.tsx:171-204`, `ActivityContextMenu.module.css` (`.submenu`, `.subItem`, `.subItemIcon`, `.subItemLabel`)

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
<ul ref={submenuRef} class="submenu" role="menu"
    style="left:{subPos.left}px; top:{subPos.top}px; visibility: visible|hidden">
  {targets.map(e =>            // SLOT_ENTRIES минус текущий slot
    <li key={e.slot}>
      <button type="button" role="menuitem" class="subItem"
              onClick={moveActivity(state.slot, id, e.slot, MAX_SAFE_INTEGER); close}>
        <span class="subItemIcon"><PanelIcon slot={e.icon}/></span>
        <span class="subItemLabel">{e.label}</span>
      </button>
    </li>)}
</ul>
```
- SLOT_ENTRIES (порядок и подписи): sidebar→"Sidebar" (icon left), main→"Left" (main), mainBottom→"Left Bottom" (main-bottom), centralBottom→"Center Bottom" (center-bottom), rightTop→"Right" (right-top), rightBottom→"Right Bottom" (right-bottom). `centralTop` исключён намеренно.
- Позиционирование: anchor = rect строки `.itemMoveTo`, `clampToViewport(side:"right", offset: 4)`.
- Move = append в конец целевой панели (тот же путь, что DnD-drop на пустой бар).

## Метрики (ИЗ CSS, точные значения)
`.submenu` — идентично `.menu` элемента 45 (общий селектор):
- `position: fixed; z-index: var(--z-dropdown); min-width: 180px`
- `background: var(--bg-surface); border: 1px solid var(--divider-soft)`
- `border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)`
- `list-style: none; margin: 0; padding: var(--space-1)`
- `display: flex; flex-direction: column; gap: 1px`
- `max-height: calc(100vh - 16px); max-width: calc(100vw - 16px); overflow-y: auto`

`.subItem` — идентично `.item`:
- `display: flex; align-items: center; gap: var(--space-2); width: 100%`
- `padding: var(--space-2) var(--space-3)`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer`

`.subItemIcon`: `display: inline-flex; align-items: center; justify-content: center; color: var(--text-muted)` (внутри — `PanelIcon`, титлбарный размер 14×12)
`.subItemLabel`: `flex: 1`

## Состояния (классы-варианты с метриками)
- `.subItem:hover`: `background: color-mix(in srgb, var(--text-primary) 10%, transparent)`
- transition/анимаций нет.

### Наша реализация

# 46 activity-context-submenu — наша реализация
Файлы: `crates/shell/src/overlay.rs:1240-1318` (сабменю внутри `tool_tab_menu()`); иконки слотов `crates/shell/src/ui/panel_placeholder.rs:34-80` (`slot_glyph_small`, аналог PanelIcon); обработчик `root.rs:1194-1198` (`MoveToolTo` → `move_activity(..., usize::MAX)`).

## Структура (gpui-дерево кратко)
```
[sub_open] div#tool-tab-submenu — тот же box_style, что корень (45):
  left = x + 180 + 2; top = (y + 40).min(vh-240).max(8)
  └ строка × (SLOT_ENTRIES минус текущий slot):
      div#ttm-<slot> flex gap SPACE_2 px SPACE_3 py SPACE_2 rounded SM
        ├ slot_glyph_small(SlotIcon)     // нативная рамка 14×12 (scale 1.0)
        └ div flex_1 label
      клик → MoveToolTo(src, id, dst) + CloseToolTabMenu
```
- Порядок и подписи: Sidebar / Left / Left Bottom / Center Bottom / Right / Right Bottom (`centralTop` исключён) — как оригинал.
- Move = append в конец целевого слота (`usize::MAX`) — как оригинал.

## Метрики (из кода, точные)
- Контейнер: идентичен 45 — min-w **180**, p 4, gap 1, rounded **12**, bg #3d3f51, border text_primary@0.06, **без тени**.
- Строка: gap **8**, px **12**, py **8**, rounded **8**, FS_SM **12px**, текст `p.text_primary` #cfd4e2; hover bg `p.text_primary`@0.10.
- Иконка слота: `slot_glyph_small` — рамка **14×12**, border 1px `p.text_muted` #838aa0, подсвеченный слот `p.text_muted`@0.85, скругления 1.5/1.0.

## Отличия от original.md той же папки
1. **Иконка «Sidebar» = иконке «Left»**: обе строки используют `SlotIcon::Main` (полный левый столбец) — у оригинала различаются варианты `left` и `main`; у нашего `SlotIcon` варианта для sidebar нет.
2. **Позиционирование фиксированное**: `x+MENU_W+2`, `y+40` (кламп) — оригинал якорит к rect строки `.itemMoveTo` c `clampToViewport(side:"right", offset:4)`; вертикальное смещение у нас всегда +40 от верха меню.
3. Нет тени (как 45 п.1); нет ролей `menu`/`menuitem`.
4. Сабменю не закрывается при уходе ховера на «Hide» (см. 45 п.3).
5. Иконка слота muted (рамка text_muted) — совпадает с оригинальным `.subItemIcon { color: text-muted }`.
6. Строки/hover/фильтр текущего слота/append-семантика — 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: строки сабменю — text_size FS_SM = 12 (`crates/shell/src/overlay.rs:1322`); font-weight не задан; иконка слота — не шрифт, а нативный div-глиф `slot_glyph_small` масштаба 1.0 (14×12) (`overlay.rs:1331`, реализация `crates/shell/src/ui/panel_placeholder.rs:97-99`), поэтому кегля у неё нет

### Вердикты

## Цикл 3: MATCH

Контекст-меню плитки (Hide / Move to) по составу и метрикам совпало.

## Цикл 4: DIVERGES

Сабменю «Move to ▸»: тень — исправлена волной 7; якорь по-прежнему фиксированный `y+40` вместо rect строки + clamp side «right» offset 4; иконка «Sidebar» совпадает с «Left». Волна 8.

## Цикл 8: DIVERGES

Тень закрыта. Якорь сабменю всё ещё фиксированный (`y + 40`) вместо rect строки + side «right» offset 4. **Претензия «иконка Sidebar = Left» СНЯТА**: в `PanelIcon.tsx` они одинаковы, наш вариант верен.

## Цикл 9: DIVERGES

overlay.rs:1293-1294: sub_x = x+180+2, sub_y = y+40 против якоря по rect строки Move to, side right, offset 4.

---

## 47. activity-drag-ghost — **DIVERGES** (цикл 9)

*История: ц3:MATCH, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](47-activity-drag-ghost/original.png)
![наш](47-activity-drag-ghost/ours.png)

### Оригинал

# 47 activity-drag-ghost — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityDragGhost.tsx:13-21`, `ActivityDragGhost.module.css` (`.ghost`)

## JSX-структура (кратко, вложенность)
```
// null, если dragGhost.value пуст. Монтируется один раз в App.tsx.
<div class="ghost"
     style="left:{g.x}px; top:{g.y}px"
     aria-hidden="true">
  <ToolIcon icon={g.icon}/>    // default size 18
</div>
```

## Метрики (ИЗ CSS, точные значения)
`.ghost`:
- `position: fixed; z-index: 9999` (hex-литерал числа, не var)
- `transform: translate(-50%, -50%)` — центр на курсоре
- `pointer-events: none`
- `width: 28px; height: 28px`
- `display: grid; place-items: center`
- `border-radius: var(--radius-sm)`
- `background: color-mix(in srgb, var(--accent-primary) 22%, var(--bg-surface))`
- `border: 1px solid color-mix(in srgb, var(--accent-primary) 50%, transparent)`
- `color: var(--accent-primary)` (иконка акцентная — единственное место, где ToolIcon красится в accent)
- `box-shadow: 0 4px 14px rgb(0 0 0 / 35%)` (hex/rgb-литерал)
- `opacity: 0.92`
- transition/анимаций нет; позиция обновляется инлайн-стилем от сигнала

## Состояния (классы-варианты с метриками)
Одно состояние; существует только во время pointer-drag плитки.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.ghost` НЕТ (`activity-bar/ActivityDragGhost.module.css:1-16`); бокс 28×28 (`:6-7`), глиф центрируется `display: grid; place-items: center` (`:8-9`), рамка 1px (`:12`). «Отступ» относительно курсора задаётся не padding-ом, а `position: fixed` + `transform: translate(-50%, -50%)` — центр ghost строго на курсоре (`:2,4`), позиция подставляется инлайном `left/top` (`ActivityDragGhost.tsx:13-21`)

### Наша реализация

# 47 activity-drag-ghost — наша реализация
Файлы: `crates/shell/src/root.rs:5411-5441` (рендер в конце корневого дерева main-окна), состояние `ToolDrag` root.rs:1780-1804 (ToolPress/ToolDragOverTab), обновление позиции root.rs:4946-4972 (глобальный mouse-move, порог 4px).

## Структура (gpui-дерево кратко)
```
.when_some(tool_drag.filter(started)):
  gpui::deferred(
    div .absolute
      left = cursor_x + 10, top = cursor_y + 8
      px SPACE_3, py 4, rounded RADIUS_SM
      bg accent_primary @ (over.is_some() ? 0.85 : 0.45)
      text FS_SM, цвет accent_action_fg
      child = label тула (lookup по id, фолбэк "Tool")
  )
```
- Текстовая пилюля с именем тула, а не иконка. Появляется после порога движения 4px; исчезает на mouse-up.

## Метрики (из кода, точные)
- Смещение от курсора: **+10px / +8px** (вправо-вниз от точки, НЕ по центру).
- Паддинг: px `SPACE_3` **12**, py **4**; rounded `RADIUS_SM` **8px**.
- Фон: `p.accent_primary` **#89b4fa**, alpha **0.85** при наличии дроп-зоны, **0.45** без цели.
- Текст: `FS_SM` **12px**, цвет `p.accent_action_fg` **#313240** (dark) / #ffffff (light).
- Тени, рамки, opacity-обёртки — нет.

## Отличия от original.md той же папки
1. **Полностью другой визуал**: оригинал — квадрат 28×28 с ИКОНКОЙ тула (`ToolIcon` 18, цвет accent), bg `accent 22% mix bg-surface`, border 1px accent@50%, `box-shadow 0 4px 14px rgba(0,0,0,.35)`, opacity 0.92; у нас — текстовая accent-пилюля с label.
2. Позиция: оригинал `translate(-50%,-50%)` (центр на курсоре); у нас смещение +10/+8 от курсора.
3. У нас alpha кодирует наличие дроп-цели (0.45/0.85) — в оригинале ghost статичен.
4. Contributed-тулы: `lookup()` не знает dyn-тулов → для них label «Tool» (не реальное имя; `lookup_any` не используется).
5. Рендер в конце дерева main-окна через `gpui::deferred` (аналог fixed z-9999 + pointer-events:none — deferred рисуется поверх, событий не ловит).

### Вердикты

## Цикл 3: MATCH

Порядок рейла bottom-карты {picker, list} с justify-end — совпал.

## Цикл 4: DIVERGES

Drag-ghost — текстовая пилюля вместо квадрата 28×28 с иконкой (accent 22% на bg-surface, border accent 50%, shadow 0 4 14 .35, opacity .92, центр на курсоре). Волна 8.

## Цикл 8: DIVERGES

Drag-ghost: нужен квадрат 28×28 r-sm (accent 22% на bg-surface, бордер accent 50%, глиф accent-primary, shadow 0 4 14 /35%, opacity .92, центр НА курсоре) вместо текстовой пилюли со смещением.

### Правка волны 16 (вердикт не выставлен — ждёт цикла сверки)

Ghost переписан по `ActivityDragGhost.module.css`: 28×28, radius-sm, фон = НЕПРОЗРАЧНЫЙ микс accent 22% + bg-surface, рамка accent 50%, глиф тула 18px цветом accent-primary, shadow 0 4 14 /35%, opacity .92, левый/верхний край = курсор − 14 (эквивалент `translate(-50%,-50%)`). Была текстовая пилюля с лейблом и смещением (+10,+8), причём её фон менялся от наличия цели — выдумка, в оригинале один класс без вариантов. `root.rs:5907-5969`; глиф вынесен в `activity_bar::tool_glyph()` (= `ToolIcon.tsx`, дефолт 18).

## Цикл 9: DIVERGES

Все СЕМЬ утверждений волны 16 подтверждены по коду (root.rs:6018-6069): 28x28, radius-sm, непрозрачный микс accent 22%+bg-surface, рамка accent 50%, shadow 0 4 14 /35%, opacity .92, центр на курсоре. ОСТАЛОСЬ: кегль глифа 18 безусловно (у оригинала codicon-тулы 16 по каскаду); lookup builtin-only -> contributed даёт circle-large, которого нет в codicon_glyph; ghost в ГЛАВНОМ окне, не в overlay, вебвью на время драга не гасятся.

---

## 48. bottom-tab-bar-strip — **DIVERGES** (цикл 11)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц10:DIVERGES, ц11:DIVERGES*

![оригинал](48-bottom-tab-bar-strip/original.png)
![наш](48-bottom-tab-bar-strip/ours.png)

### Оригинал

# 48 bottom-tab-bar-strip — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/BottomTabBar.tsx:70-84`, `BottomTabBar.module.css` (`.strip`, `.tabs`, `.pickerSlot`)

## JSX-структура (кратко, вложенность)
```
<div class="strip"
     data-activity-strip="1"
     data-activity-slot={slot}                 // main | mainBottom | centralBottom
     data-activity-orientation="horizontal">
  <div class="tabs" role="tablist" aria-label="{slot} tabs">
    {tabs}                                     // элемент 49 + плейсхолдеры (50)
  </div>
  <div class="pickerSlot">
    <ActivityPicker slot={slot} popDirection="up"/>   // «…» dots, элемент 42
  </div>
</div>
```
- Те же данные, что у вертикального ActivityBar; drop владеет карточка-приёмник (`useActivityDropTarget`), стрип — только drag-start и контекст-меню.
- Рецепт портирован из Bridge `FileViewerTabs.tsx`.

## Метрики (ИЗ CSS, точные значения)
`.strip`:
- `display: flex; align-items: center; gap: var(--space-1)`
- `flex-shrink: 0`
- `padding: 4px var(--space-2)` (вертикаль 4px, горизонталь var(--space-2))
- `border-radius: var(--radius-sm)`
- фон: не задан (прозрачный)

`.tabs`:
- `display: flex; align-items: center; gap: var(--space-1)`
- `flex: 1; min-width: 0`
- `overflow-x: auto; scrollbar-width: none` (скрытый скроллбар)

`.pickerSlot`:
- `flex-shrink: 0; display: flex; align-items: center; margin-left: auto` (пикер прижат к правому краю)

## Состояния (классы-варианты с метриками)
Вариантов у самого стрипа нет; состояния несут табы (49) и плейсхолдер (50).

## Дополнение атрибутов (цикл 10)

- цвета: `.strip` ни background, ни color НЕ задаёт (`activity-bar/BottomTabBar.module.css:5-12`) — прозрачная полоса поверх фона карты. Hex — у детей: `.tab` color var(--text-secondary) #adb3c7 (`:33`), hover bg color-mix(var(--bg-surface) #3d3f51 50%, transparent) + color var(--text-primary) #cfd4e2 (`:43-44`), `.tabActive` bg color-mix(var(--accent-primary) #89b4fa 16%, transparent) + color #cfd4e2 (`:65-66`), `.dropPlaceholder` border 1px dashed accent-primary #89b4fa 70% + bg accent-primary 14% (`:78-79`), `.tabDragging { opacity: 0.3 }` (`:69`)

### Наша реализация

# 48 bottom-tab-bar-strip — наша реализация
Файлы: `crates/shell/src/ui/slot_panel.rs:185-237` (`slot_panel()` — стрип строится при `pinned.len() > 0`); вызовы: `root.rs:3998` (Main), `root.rs:4089` (MainBottom), `root.rs:4583` (CentralBottom).

## Структура (gpui-дерево кратко)
```
slot_panel(slot, state, label, icon, picker_up, drag_over, body):
  div col size_full min_h 0
    ├ [pinned non-empty] bar:
    │    div flex items_center gap 2, flex_shrink_0, px SPACE_2, pt SPACE_1
    │      ├ tab(...) × pinned          // элемент 49
    │      ├ div flex_1                 // спейсер
    │      └ dots(slot, picker_up)      // «…» 24×24, элемент 42
    └ div flex_1 min_h 0 → body | panel_placeholder_ex(label, hint, icon, open_tool_btn)
```
- Слоты: Main («Claude Bridge»-таб), MainBottom, CentralBottom. `picker_up` — раскрытие пикера вверх для нижних панелей.
- Drop-цель определяется probe-hit-тестом карточки (root.rs:4957-4966), стрип отдаёт только `ToolDragOverTab` (reorder-индекс) с зажатой ЛКМ.

## Метрики (из кода, точные)
- Строка стрипа: `gap` **2px**, `px(SPACE_2)` **8px**, `pt(SPACE_1)` **4px** (снизу паддинга НЕТ), `flex_shrink_0`.
- Фон/радиус: нет (прозрачный, внутри glint-карты).
- «…»: 24×24, codicon-more 15px, прижат вправо спейсером `flex_1` (аналог `margin-left: auto`).

## Отличия от original.md той же папки
1. **Gap 2px** vs `--space-1` (4px) оригинала — табы плотнее.
2. **Паддинг**: `4px 8px` только сверху (`pt`), оригинал — `padding: 4px space-2` симметрично (низ 4px у нас отсутствует; тело начинается сразу).
3. **Нет скролла**: оригинал `.tabs { overflow-x: auto; scrollbar-width: none; flex:1; min-width:0 }`; у нас табы прямо в строке без scroll-контейнера — при переполнении табы сжимают/выталкивают «…».
4. Нет обёртки `.tabs`/`.pickerSlot` и `role=tablist`/`data-activity-*` атрибутов.
5. Нет `border-radius: radius-sm` на стрипе (у оригинала есть, но невидим без фона — расхождение формальное).
6. Пустой слот: оригинал рендерит стрип всегда (данные те же), плейсхолдер отдельно; у нас при `pinned.is_empty()` стрип отсутствует целиком, «…» доступен только пилюлей «Open Tool ▾».

## Дополнение атрибутов (цикл 10)

- цвета: стрип своего фона НЕ задаёт — просвечивает карта слота bg-mantle #262533 (`crates/theme/src/palette.rs`); цвет текста наследуется от карты (text-secondary #adb3c7), активный таб — accent-primary 16% поверх (`crates/shell/src/ui/slot_panel.rs`, `fn tab`)
- ховер: N/A: ховер — у самого стрипа состояния нет; ховер несут табы (bg-surface 55%) и «…» (bg-surface 50%)

### Вердикты

## Цикл 3: DIVERGES

Стрип нижней панели: gap 2 вместо space-1, padding только сверху вместо 4px по вертикали. Исправлено волной 6.

## Цикл 4: DIVERGES

Стрип был 32 лог. вместо 40 (высоту задаёт пикер 32×32) — **исправлено волной 7** вместе с 42. Остаётся: нет `.tabs { overflow-x: auto }`, при пустом `pinned` стрип отсутствует целиком.

## Цикл 8: DIVERGES

Стрип всегда виден и высота 40 — закрыто. Осталось: нет контейнера `.tabs { flex 1; min-width 0; overflow-x: auto }` — переполнение режется вместо скролла.

## Цикл 9: DIVERGES

Замер подтвердил: таб 720.00..744.00 (h 24.00) отцентрован в стрипе 40, стрип виден без табов. ОСТАЛОСЬ: нет контейнера .tabs{flex:1;min-width:0;overflow-x:auto} — табы лежат прямо в flex стрипа, переполнение режет.

## Цикл 10: DIVERGES

Закрыто: появился контейнер `.tabs` — flex 1, min-width 0, `overflow-x: auto`, gap
space-1; переполнение теперь скроллится, а не режется.

Осталось: кадр состояния переполнения.

## Цикл 11: DIVERGES

Закрыто дополнительно: `.strip { border-radius: var(--radius-sm) }`.

Осталось: кадр переполнения.

---

## 49. bottom-tab — **DIVERGES** (цикл 10)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц10:DIVERGES*

![оригинал](49-bottom-tab/original.png)
![наш](49-bottom-tab/ours.png)

### Оригинал

# 49 bottom-tab — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/BottomTabBar.tsx:48-64`, `BottomTabBar.module.css` (`.tab`, `.tabActive`, `.tabDragging`, `.tabImage`, `.tabLabel`)

## JSX-структура (кратко, вложенность)
```
<button type="button" data-tab="1"
        class="tab [tabActive] [tabDragging]"
        aria-pressed={isActive}
        aria-label={item.label}
        data-tooltip={item.label}
        onPointerDown={beginActivityDrag(e, slot, id)}
        onKeyDown={Enter|Space → activateActivity}
        onContextMenu={openActivityContextMenu(slot, id, x, y)}>
  <ToolIcon icon={item.icon} size={TAB_ICON_SIZE_PX} imageClassName="tabImage"/>  // TAB_ICON_SIZE_PX = 13
  <span class="tabLabel">{item.label}</span>
</button>
```

## Метрики (ИЗ CSS, точные значения)
`.tab`:
- `display: inline-flex; align-items: center; gap: 6px`
- `padding: 4px 10px; height: 24px`
- `background: transparent; border: none`
- `border-radius: var(--radius-sm)`
- `color: var(--text-secondary)`
- шрифт: `font-size: 11px; font-weight: 500; letter-spacing: 0.02em` (family/line-height не заданы — наследуются)
- `white-space: nowrap; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`

Иконка:
- `.tab :global(.codicon)` — `font-size: 13px; line-height: 1`
- `.tabImage` — `width: 13px; height: 13px; object-fit: contain`
- SVG-ветка ToolIcon получает `size=13` пропом (TAB_ICON_SIZE_PX, экспортируется для Design-panel sample)

`.tabLabel`: `overflow: hidden; text-overflow: ellipsis; min-width: 0`

## Состояния (классы-варианты с метриками)
- `.tab:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- `.tabActive`, `.tabActive:hover`: `background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)` — без кольца.
- `.tabDragging`: `opacity: 0.3`
- focus: отдельных стилей нет.

### Наша реализация

# 49 bottom-tab — наша реализация
Файлы: `crates/shell/src/ui/slot_panel.rs:27-115` (`tab()`); модель `crates/shell/src/activity.rs:308-313` (`lookup_any` — builtin + contributed); dnd/клик `root.rs:1780-1804, 5015-5038`.

## Структура (gpui-дерево кратко)
```
div#st-<slot>-<id> flex items_center gap SPACE_1, h 24, px SPACE_3, rounded SM
  ├ иконка: phosphor svg 13×13 | codicon 13px (codicon_by_name, фолбэк \u{eb51})
  └ label (lookup_any; фолбэк "Tool")
события:
  LMB down → ToolPress(slot,id,x,y)      // клик без движения = активация, ≥4px = drag
  mouse_move c зажатой ЛКМ → ToolDragOverTab(slot, index)   // цель reorder
  RMB → OpenToolTabMenu (Hide / Move to ▸, элемент 45)
```

## Метрики (из кода, точные)
- Высота **24px**, px `SPACE_3` **12**, gap `SPACE_1` **4**, rounded `RADIUS_SM` **8px**.
- Шрифт: `FS_SM` **12px**, без font-weight/letter-spacing.
- Цвета: базовый `p.text_muted` **#838aa0**; hover: bg `p.text_primary`@**0.08** + text `p.text_primary` #cfd4e2.
- Active: bg `p.accent_primary` #89b4fa @**0.16** + text #cfd4e2.
- Drag-over (цель вставки): `border_l_2` цвета `p.accent_primary` (2px слева).
- Иконка **13×13** (= TAB_ICON_SIZE_PX оригинала); цвет primary/active — иначе muted.

## Отличия от original.md той же папки
1. **Шрифт**: 12px без weight 500 и letter-spacing 0.02em (оригинал 11px/500/0.02em) — табы у нас крупнее и «легче».
2. **Hover-фон**: `text_primary @ 8%` vs оригинального `bg-surface 50% mix` — другой рецепт (заметно в light-теме).
3. **Базовый цвет**: `text_muted` vs `--text-secondary` оригинала (текст таба темнее).
4. **Паддинг/gap**: px 12 vs 10; gap 4 vs 6.
5. **Нет `.tabDragging`** (opacity 0.3 на перетаскиваемом табе) — вместо него drag-over `border_l_2` на целевом табе (в оригинале цель показывает отдельный dropPlaceholder, см. 50).
6. Нет `white-space: nowrap`/ellipsis у label и нет tooltip'а/aria/keyboard (Enter/Space).
7. Иконка hover не перекрашивается (цвет посчитан от `active` заранее — тот же дефект svg, что 39 п.3).
8. Иконка/размер 13px, активация кликом, RMB-меню — совпадают.

### Вердикты

## Цикл 3: DIVERGES

Таб стрипа: fs-sm/без веса/text-muted/px 12/gap 4 вместо 11px/500/text-secondary/px 10/gap 6. Исправлено волной 6 (letter-spacing 0.02em недоступен).

## Цикл 4: DIVERGES

Типографика/цвета/паддинги таба совпали (замеры: неактивный 173,179,199 против 175,179,198; активный 207,212,226 против 208,212,225; фон пилюли 54,60,83 против 56,60,83). Ховер по активному табу — **исправлен волной 7**. Остаётся: нет nowrap/ellipsis у лейбла, нет `.tabDragging` 0.3; `letter-spacing 0.02em` (−4px ширины) — ограничение gpui.

## Цикл 8: DIVERGES

Типографика и ховер активного таба закрыты. Осталось: нет ellipsis у лейбла (готовый `text_fit::fit` не задействован), нет `.tabDragging` 0.3.

## Цикл 9: DIVERGES

Замер: h 24.00, px 10, чернила иконки 12.00 в боксе 13, gap 6, active accent-tint — совпало. ОСТАЛОСЬ: нет эллипсиса лейбла; нет .tabDragging{opacity:.3}.

## Цикл 10: DIVERGES

Закрыто: лейбл таба усечается (`overflow: hidden; text-overflow: ellipsis;
white-space: nowrap`).

Осталось: `.tabDragging { opacity: .3 }` — состояние перетаскиваемого таба.

---

## 50. bottom-tab-drop-placeholder — **DIVERGES** (цикл 10)

*История: ц3:MATCH, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES, ц10:DIVERGES*

![оригинал](50-bottom-tab-drop-placeholder/original.png)
![наш](50-bottom-tab-drop-placeholder/ours.png)

### Оригинал

# 50 bottom-tab-drop-placeholder — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/BottomTabBar.tsx:87-89`, `BottomTabBar.module.css` (`.dropPlaceholder`)

## JSX-структура (кратко, вложенность)
```
<span class="dropPlaceholder" aria-hidden="true"/>
```
- Вставляется в `.tabs` на `overIndex` (или в конец при `overIndex === pinned.length`), когда `dragState.overSlot === slot`.

## Метрики (ИЗ CSS, точные значения)
`.dropPlaceholder`:
- `display: inline-block`
- `width: 36px; height: 24px` (высота = высоте таба)
- `border-radius: var(--radius-sm)`
- `border: 1px dashed color-mix(in srgb, var(--accent-primary) 70%, transparent)`
- `background: color-mix(in srgb, var(--accent-primary) 14%, transparent)`
- transition/анимаций нет; flex-item в `.tabs` (gap var(--space-1))

## Состояния (классы-варианты с метриками)
Одно состояние; появляется/исчезает вставкой в DOM во время drag.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.dropPlaceholder` НЕТ (`activity-bar/BottomTabBar.module.css:73-80`) — бокс 36×24 (`:75-76`), рамка 1px dashed (`:78`); внешние зазоры даёт контейнер `.tabs { gap: var(--space-1) }` = 4 (`:17`); сама полоса `.strip` добавляет по краям padding 4px var(--space-2) = 4/8 (`:10`)

### Наша реализация

# 50 bottom-tab-drop-placeholder — наша реализация
Файлы: `crates/shell/src/ui/slot_panel.rs` (`fn drop_placeholder`, вставка внутрь `.tabs`), `crates/shell/src/root.rs` (`tool_drag_over_index(slot)`, событие `ToolDragOverTab`)

## Структура (gpui-дерево кратко)
```
tabs (flex, items-center, gap 4, flex-1, min-w 0, overflow-x auto)
├─ [drag_over == i] drop_placeholder       ← пустой бокс 36×24
├─ tab(...)
└─ [drag_over == pinned.len()] drop_placeholder   ← вставка в конец
```
Прежняя индикация (левая accent-рамка 2px на целевом табе) убрана.

## Метрики (из кода, точные)
- 36×24 (высота = высоте таба), `flex-shrink: 0`.
- radius RADIUS_SM 8.
- Рамка 1px **dashed**, accent-primary #89b4fa при alpha 0.7.
- Фон accent-primary при alpha 0.14.
- Flex-item в `.tabs` с gap SPACE_1 4; собственных паддингов нет.

## Отличия от original.md той же папки
Кадра состояния drag в досье нет — вердикт по коду.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — плейсхолдер пустой (36×24 без содержимого), собственных паддингов нет; зазоры вокруг даёт `.tabs` gap SPACE_1 = 4 (`crates/shell/src/ui/slot_panel.rs`)

### Вердикты

## Цикл 3: MATCH

Active-таб accent 16% + text-primary.

## Цикл 4: DIVERGES

Drop-плейсхолдер таба (36×24 dashed) заменён на `border_l_2` у целевого таба; вставки в конец нет. Волна 8.

## Цикл 8: DIVERGES

Drop-стаб таба 36×24 dashed не реализован (у нас `border_l_2` на целевом табе), вставки в конец нет.

## Цикл 9: DIVERGES

36x24 dashed не реализован; вместо него border_l_2 на целевом табе (slot_panel.rs:116); вставки в конец нет.

## Цикл 10: DIVERGES

Реализован: `slot_panel::drop_placeholder` — 36×24, radius-sm 8, рамка 1px dashed
accent-primary 70%, фон accent-primary 14%; вставляется по `overIndex` внутрь `.tabs`
и в конец при `overIndex == pinned.len()`. Прежняя индикация (`border-left` на
соседнем табе) убрана.

Осталось: кадр состояния drag.

---

## 51. tool-icon — **DIVERGES** (цикл 9)

*История: ц3:DIVERGES, ц4:DIVERGES, ц8:DIVERGES, ц9:DIVERGES*

![оригинал](51-tool-icon/original.png)
![наш](51-tool-icon/ours.png)

### Оригинал

# 51 tool-icon — оригинал
Файлы: `kamin-ide/src/renderer/components/tool-icon/ToolIcon.tsx:26-48`, `tool-icon/tool-icon-paths.ts`; css-модуля нет (размеры img задают классы вызывающих: `.btnImage`/`.menuItemImage` 18×18, `.tabImage` 13×13)

## JSX-структура (кратко, вложенность)
Три ветки по приоритету:
```
1. isImageIcon(icon)  // /^(?:data:|https?:|file:|\/)/  (signals/activity.ts:89-91)
   <img src={icon} alt="" class={imageClassName} aria-hidden="true"/>

2. TOOL_ICON_PATHS[icon]  // встроенный токен: folders, search, warning, terminal, gear
   <svg class={className} width={size} height={size}
        viewBox="0 0 256 256"            // TOOL_ICON_VIEWBOX = 256
        fill="currentColor" aria-hidden="true">
     <path d={path}/>
   </svg>

3. иначе (codicon-имя)
   <i class="codicon codicon-{icon}[ className]" aria-hidden="true"/>
```

## Метрики (ИЗ CSS, точные значения)
- Собственного CSS нет. Props: `size` (px) — по умолчанию `DEFAULT_SIZE_PX = 18` — идёт в width/height атрибуты SVG; BottomTabBar передаёт 13.
- SVG: `fill="currentColor"` — наследует цвет кнопки (muted → hover primary → active primary; в drag-ghost accent).
- `<img>`-ветка размеров сама не имеет — их дают классы вызывающего:
  - `.btn img`, `.picker img`, `.btnImage`, `.menuItemImage` (ActivityBar.module.css): `width: 18px; height: 18px; object-fit: contain`
  - `.tabImage` (BottomTabBar.module.css): `width: 13px; height: 13px; object-fit: contain`
- codicon-ветка: размер задаёт вызывающий (`.btn/.picker :global(.codicon)` 18px, `.tab :global(.codicon)` 13px, `line-height: 1`).

## Состояния (классы-варианты с метриками)
Состояний нет — чистый рендер. Vendored-иконки: Phosphor regular (одиночный `path`, viewBox 256), ключи-токены: `folders`, `search`, `warning`, `terminal`, `gear`. Неизвестный токен → фоллбек в codicon-шрифт (VSIX-имена работают без изменений).

## Дополнение атрибутов (цикл 10)

- цвета: собственных hex у компонента нет. Ветка Phosphor-SVG: `fill="currentColor"` (`tool-icon/ToolIcon.tsx:39`) — цвет полностью от родителя. Ветка codicon: цвет не задаётся, наследуется (`ToolIcon.tsx:47`). Ветка `<img>`: currentColor НЕ применяется, цвет — свойство ассета, расширения обязаны поставлять монохромные SVG (`ToolIcon.tsx:28`, обоснование в комментарии `activity-bar/ActivityBar.module.css:71-75`). Фактические цвета от вызывающих: `.btn`/`.picker` var(--text-muted) #838aa0, hover var(--text-primary) #cfd4e2, active #cfd4e2 (`ActivityBar.module.css:62,88,96`); `.menuItem` var(--text-primary) #cfd4e2 (`:167`); `.tab` var(--text-secondary) #adb3c7, active #cfd4e2 (`activity-bar/BottomTabBar.module.css:33,66`); `.glyph` плейсхолдера var(--text-disabled) #60667b (`panel-placeholder/ActivityPlaceholder.module.css`, блок `.glyph`)
- отступы: у самого `ToolIcon` ни padding, ни margin — CSS-модуля нет (`ToolIcon.tsx:1-48`). «Отступы» сводятся к размерному боксу: prop `size` по умолчанию 18 (`ToolIcon.tsx:24`, применяется к `width`/`height` SVG `:35-36`); `<img>`-ветка размер берёт из класса вызывающего — `.btnImage`/`.menuItemImage` 18×18 (`ActivityBar.module.css:76-83`), `.tabImage` 13×13 (`BottomTabBar.module.css:50-54`); внешние зазоры до подписи дают контейнеры: `.menuItem { gap: var(--space-2) }` = 8 (`ActivityBar.module.css:161`), `.tab { gap: 6px }` (`BottomTabBar.module.css:27`)

### Наша реализация

# 51 tool-icon — наша реализация
Единого компонента НЕТ — рендер иконки тула продублирован в 4 местах:
Файлы: `crates/shell/src/ui/activity_bar.rs:16-38` (`phosphor_path()` + `codicon_glyph()`), `crates/shell/src/ui/tool_picker.rs:27-52` (`tool_icon()`), `crates/shell/src/ui/slot_panel.rs:85-104` (иконка таба), `crates/shell/src/ui/right_column.rs:65-78` (иконка рейл-плитки); общие помощники `crates/shell/src/ui/icon.rs` (`codicon()`, бокс 16×16) и `crates/shell/src/ui/codicon_map.rs` (`codicon_by_name`).

## Структура (две ветки vs три у оригинала)
```
1. phosphor_path(icon) → Some("icons/<token>.svg")
   gpui::svg().path(...).w/h(size).text_color(...)      // vendored Phosphor, currentColor-аналог
   токены: folders(projects), tree-view(tree), search, warning(problems),
           terminal, gear(customize)
2. иначе codicon-шрифт:
   div .font_family("codicon") .text_size(size) .child(глиф)
   глиф: codicon_map::codicon_by_name(icon) (пикер/табы/рейлы)
         либо локальный codicon_glyph() (activity_bar: extensions,
         claudeBridgePlan/Todos/Agents + фолбэк \u{ea7b} file)
```
- Ветки `<img src=URL/data:>` НЕТ — image-иконки VSIX не поддержаны.

## Метрики (из кода, точные)
- Размер по вызывающему: бар/рейлы **18×18** (svg; codicon 18px в баре, **16px** в рейле), пикер **15×15** / 15px, стрип-табы **13×13** / 13px.
- Цвет: задаётся вызывающим (`text_muted` #838aa0 / active `text_primary` #cfd4e2 / пикер `text_secondary` #adb3c7); svg красится `.text_color()` = аналог `fill="currentColor"`, но значение фиксируется при построении (hover не перекрашивает — дефект).
- `icon::codicon()` — глиф в flex-боксе **16×16**, размер шрифта параметром.

## Отличия от original.md той же папки
1. **Нет img-ветки** (`isImageIcon`: data:/https?:/file:/`/`) — VSIX-расширения с растровыми/URL-иконками получат codicon-фолбэк `\u{ea7b}`/`\u{eb51}` вместо своей иконки.
2. Phosphor не как инлайн `<path viewBox 256>` из `tool-icon-paths.ts`, а vendored svg-ассеты `icons/*.svg` (assets.rs) — визуально те же глифы.
3. Набор токенов шире: + `tree-view` (в оригинальном TOOL_ICON_PATHS его нет — tree там contributed?); фолбэк-глифы неизвестных различаются между копиями (`\u{ea7b}` в баре vs `\u{eb51}` в табах/рейлах).
4. Рейл-плитка: codicon 16px в боксе 32 (оригинал 18px).
5. Дублирование логики в 4 местах вместо одного `<ToolIcon>` — размеры/фолбэки уже разъехались (18/16/15/13).
6. Hover/active-перекраска svg не работает (цвет вычислен заранее), codicon-ветка в `tile()` перекрашивается — поведение веток различается, у оригинала обе наследуют currentColor.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — иконка рисуется как svg/глиф без собственных паддингов; отступы задают контейнеры (плитка 32×32, таб px 10)
- гэпы: N/A: гэпы — у иконки нет детей; расстояние до лейбла даёт `gap 6` строки таба (`crates/shell/src/ui/slot_panel.rs`)
- шрифты: кегль codicon-ветки различается по месту вызова: рейл 18 (`activity_bar.rs`, `fn tool_glyph`), таб стрипа 13 (`slot_panel.rs`), пикер 16 (`tool_picker.rs`), правый рейл 18 (`right_column.rs`); font-family везде «codicon», weight не задаётся

### Вердикты

## Цикл 3: DIVERGES

ToolIcon: ветка img (VSIX-иконки) не реализована, размеры глифа не унифицированы между рейлом (18) / стрипом (13) / меню (16). Частично приведено волной 6; img-ветка — волна 7.

## Цикл 4: DIVERGES

Размеры ToolIcon по вызывающим совпадают с оригиналом (бар/рейл 18, пикер 18/16, таб 13). Остаётся: нет ветки `<img>` для VSIX-иконок, фолбэк-глиф различается между 4 копиями логики, цвет svg фиксируется при построении (ограничение gpui). Волна 8: вынести ToolIcon в один модуль.

## Цикл 8: DIVERGES

Четыре копии логики иконки тула с ТРЕМЯ разными фолбэками; у оригинала фолбэк один плюс ветка `<img>` для VSIX-иконок. Размеры по вызывающим совпадают.

## Цикл 9: DIVERGES

Четыре копии рендера с тремя разными фолбэками (activity_bar.rs:30-38, right_column.rs:78, slot_panel.rs:105, tool_picker.rs:29-50) против одного ToolIcon с фолбэком codicon-${icon}; ветки <img> для URL-иконок в порте нет вовсе.

---

# Зона 52-91 — Панели, карты, экраны Customize, терминал

## 52. app-shell — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](52-app-shell/original.png)
![наш](52-app-shell/ours.png)

### Оригинал

# 52 app-shell — оригинал
Файлы: kamin-ide/src/renderer/components/layout/AppLayout.tsx (строки 55-79), kamin-ide/src/renderer/components/layout/AppLayout.module.css

## JSX-структура (кратко, вложенность)
```
div.appWrapper
├─ <Titlebar />
├─ div.body (или .body + .bodyNoSidebar когда сайдбар скрыт)
│  ├─ <ActivityBar slot="sidebar" align="top" />
│  ├─ {sidebar}                                  (проп)
│  ├─ div.mainColumn [data-centre-column]        (только если inCustomize || mainVisible)
│  │  │  style={ minWidth: MAIN_MIN_WIDTH_PX }   (= 100px, config/constants.ts:50)
│  │  ├─ {main}                                  (проп; если inCustomize || mainVisible)
│  │  └─ <MainBottomPanel />                     (если !inCustomize && !noSessions)
│  ├─ <FilePanel fill={fileFills} />             (если !inCustomize && !noSessions)
│  └─ <RightPanel fill={rightFills} />           (если !inCustomize && !noSessions)
├─ <StatusBar />
├─ <Toasts />
└─ <CommandPalette />
```
Логика: `noSessions = !inCustomize && openSessions.length === 0`; `mainColumnPresent = inCustomize || mainVisible`; `fileFills = !mainColumnPresent && filePanelVisible`; `rightFills = !mainColumnPresent && !filePanelVisible`.

## Метрики (ИЗ CSS, точные значения)
### .appWrapper
- display: flex; flex-direction: column; height: 100vh; width: 100vw; overflow: hidden
- background (брендовый фон, 3 слоя):
  - `radial-gradient(ellipse 1200px 600px at 20% 10%, color-mix(in srgb, var(--accent-purple) 8%, transparent), transparent 60%)`
  - `radial-gradient(ellipse 800px 500px at 90% 90%, color-mix(in srgb, var(--accent-primary) 6%, transparent), transparent 60%)`
  - `var(--bg-sidebar)`
- color: var(--text-primary)

### .body
- flex: 1; display: flex; flex-direction: row; min-height: 0; overflow: hidden
- gap: var(--space-2) — единственный источник межпанельных отступов (дети без собственных горизонтальных margin)
- padding: 0 var(--space-1) — симметричный горизонтальный гуттер (половина межпанельного gap)

### .bodyNoSidebar
- пустое правило `{}` — специального padding нет, симметричный гуттер уже на .body

### .mainColumn
- flex: 1; display: flex; flex-direction: column; min-height: 0
- min-width — инлайн из компонента: `MAIN_MIN_WIDTH_PX` = 100px
- вертикального gap НЕТ намеренно (MainBottomPanel несёт свой 10px resize handle сверху)

## Состояния (классы-варианты с метриками)
- `.body` vs `.body .bodyNoSidebar` — без визуальной разницы (bodyNoSidebar пуст)
- Customize-режим: рендерятся только Titlebar + ActivityBar + sidebar + mainColumn (FilePanel/RightPanel/MainBottomPanel опущены)
- noSessions: mainColumn показывает welcome; FilePanel/RightPanel/MainBottomPanel опущены (сигналы видимости не трогаются)
- hover/active/transition: нет

### Наша реализация

# 52 app-shell — наша реализация
Файлы: crates/shell/src/root.rs:3387 (render), 5091-5130 (корневой фон+radial), 5131-5189 (titlebar), 5190-5369 (body), 5370-5380 (status_bar), 2565-2587 (gap_wrap); crates/shell/src/ui/radial_bg.rs:49-125; crates/metrics/src/lib.rs:14,20,32-33

## Структура (gpui-дерево кратко)
```
div (root, .relative, track_focus, key_context "Root")
├─ canvas: корневой фон bg_sidebar, paint_quad по hole_segments_multi
│    (дыры под composition-вебвью вместо сплошного .bg())
├─ radial_bg.layers(viewport): 2 absolute img (baked PNG)
├─ titlebar(...)                                   (высота TITLEBAR_HEIGHT=42)
├─ div#body .relative .flex_1 .min_h(0) .flex .pl(4)
│  ├─ activity_bar(...)
│  ├─ when(sidebar_visible): div w(sidebar_w) + gap_wrap + v_handle("sidebar-handle")
│  ├─ when(customize_open): gap_wrap(glint(customize_panel)) на всю ширину
│  ├─ when(!customize && has_active):
│  │    when(main_visible)  → main_wrap (flex_1, min_w 100)
│  │    when(file_visible)  → main_file_handle + file_wrap (w=file_w, shrink 0)
│  │    when(right_visible) → file_right_handle + right_wrap (w=right_w+44, shrink 0)
│  └─ when(!customize && !has_active): welcome_full (flex_1)
├─ status_bar(...)                                 (высота STATUS_BAR_HEIGHT=24)
└─ скрим/оверлеи (palette/quickopen/fif/modal)
```
Межпанельный зазор — НЕ flex-gap: каждая колонка оборачивается `gap_wrap` (px 4 + условные pt/pb 4), смежные 4+4 = 8px.

## Метрики (из кода, точные)
- Корневой фон: `p.bg_sidebar` — dark #1d1d28, light #f4f1ea (canvas-заливка сегментами вокруг вебвью-дыр)
- Radial-слои (radial_bg.rs): бейк в PNG, alpha = A·(1 − d/0.6):
  - purple: эллипс 1200×600, центр 20%/10% вьюпорта, accent_purple (#cba6f7 dark / #8a5fc8 light), peak α 0.08
  - primary: эллипс 800×500, центр 90%/90%, accent_primary (#89b4fa dark / #da8343 light), peak α 0.06
- body: `pl(px(BODY_GUTTER_X=4))` — гаттер ТОЛЬКО слева; flex-row; min_h 0
- gap_wrap: px 4, pt/pb 4 (условно) — эквивалент body gap 8 + гуттер 4
- Titlebar 42 / StatusBar 24; текст-цвет задают дети (нет общего color на корне)

## Отличия от original.md той же папки
1. Гуттер: оригинал `padding: 0 var(--space-1)` (4px с ОБЕИХ сторон body); у нас только `pl(4)` — справа роль гуттера играет rail правой колонки (44px). При скрытой правой панели правый край без 4px-гуттера.
2. Механизм зазоров: оригинал — flex `gap: 8` на .body; у нас — паддинги gap_wrap каждой колонки. Визуально то же 8px, но зазор принадлежит колонке (hit-зоны сплиттеров живут в нём).
3. Радиальный градиент: CSS radial-gradient → бейк PNG-спрайтов (линейный спад до 0.6 — математически совпадает с `transparent 60%`), но без color-mix — прямая альфа поверх bg_sidebar; при resize спрайты не перегенерируются (фикс-размер эллипса — как в CSS).
4. `color: var(--text-primary)` на appWrapper не переносился — цвет задаётся точечно в каждом компоненте.
5. Welcome: у нас заменяет все три колонки (welcome_full flex_1); оригинал: welcome внутри mainColumn + FilePanel/RightPanel опущены — итоговая площадь совпадает.
6. Customize: у нас одна glint-карта на всю область (колонки не рендерятся); оригинал держит mainColumn (CustomizePanel внутри него) — визуально эквивалентно.
7. fill-режимы (fileFills/rightFills — колонка растягивается при скрытом main) НЕ реализованы: file_wrap/right_wrap всегда фикс-ширины.
8. Дыры под composition-вебвью в корневом фоне — наша специфика (в оригинале нет; там DOM-слои).

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — у корня (`root.rs` render, `div().relative().track_focus()`) hover-стилей нет, как и у `.appWrapper`/`.body` оригинала; ховер живёт только у ручек (55/57/62) и детей.

### Вердикты

# 52 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: фон+эллипсы, левый гуттер 4, mainColumn min-w 100, welcome в main-карте.
Расхождения: нет pr(4) у body (rail вплотную к краю окна); гэп activity-bar/
sidebar 4 vs 8; noSessions не скрывает File/Right/MainBottom; fileFills/
rightFills (flex 1 1 0) не реализованы.

## Цикл 5: DIVERGES

`.body gap 8` между activity-bar и сайдбаром даёт только 4 (бар — прямой ребёнок body без обёртки, зазор несёт лишь `pl 4` сайдбара) → нужен `pr(4)` у бара. `noSessions` не гасит File/Right/MainBottom (оригинал `AppLayout.tsx:55-79` опускает их при нуле сессий). fill-режимы (`fileFills`/`rightFills` = flex 1 1 0) не реализованы. `pl/pr 4` и welcome-в-карте — подтверждены.

## Цикл 6: DIVERGES

Зазор 8 закрыт (`pr 4` у бара), но border-box съел ширину — плитки уехали на 2px влево (замер ц.6). **Исправлено волной 10**: `w(ACTIVITY_BAR_WIDTH + 4).pr(4)`. Осталось: `noSessions` не гасит File/Right/MainBottom, fill-режимы.

---

## 53. main-content — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](53-main-content/original.png)
![наш](53-main-content/ours.png)

### Оригинал

# 53 main-content — оригинал
Файлы: kamin-ide/src/renderer/components/main/MainContent.tsx (строки 35-58), kamin-ide/src/renderer/components/main/MainContent.module.css

## JSX-структура (кратко, вложенность)
```
main.main [aria-label="Left"] [data-activity-slot="main"]
  style={ height: `${heightPct}%` }
  data-activity-drop = "blocked" | "over" | undefined  (drop-target хук)
  onDragOver / onDragLeave / onDrop
├─ (customize)  → <CustomizePanel />
├─ (noSessions) → <WelcomePlaceholder />
└─ (иначе)
   ├─ <BottomTabBar slot="main" />
   └─ activeId ? <ActivityBody id={activeId} slot="main" />
              : <PanelPlaceholder label="Left" slot="main" />
```
Высота: `customize || noSessions ? 100% : (mainBottomVisible ? mainSplit*100 : 100)%`, `toFixed(2)`.

## Метрики (ИЗ CSS, точные значения)
### .main
- `composes: glint-surface from global` (theme/global.css:96):
  - border: 1px solid transparent
  - background: `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- flex-shrink: 0; display: flex; flex-direction: column
- min-width: 0; min-height: 0; overflow: hidden
- margin: 0 (межпанельные отступы — от gap родителя)
- border-radius: var(--radius-lg)
- position: relative
- height — инлайн-стиль (проценты от mainSplit)

### Drop-индикация (глобально, theme/global.css:53-67)
- `[data-activity-drop="over"]`: background-color `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; outline `1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent)`; outline-offset: -2px; transition: background-color var(--transition-fast), outline-color var(--transition-fast)
- `[data-activity-drop="blocked"]`: background-color `color-mix(in srgb, var(--accent-red) 12%, transparent)`; box-shadow `inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent)`; transition: background-color var(--transition-fast), box-shadow var(--transition-fast)

## Состояния (классы-варианты с метриками)
- customize: тело = CustomizePanel, height 100%, без табов
- noSessions: тело = WelcomePlaceholder, height 100%
- нормальный: BottomTabBar + ActivityBody/PanelPlaceholder; height = mainSplit*100% при видимом Left Bottom, иначе 100%
- data-activity-drop="over"/"blocked" — метрики выше
- hover/focus: нет собственных

### Наша реализация

# 53 main-content — наша реализация
Файлы: crates/shell/src/root.rs:3988-4012 (chat_content), 4046-4110 (main_column), 4709-4714 (main_wrap); crates/shell/src/ui/glint.rs:122-233 (glint_surface_wv_holed); crates/shell/src/ui/slot_panel.rs:187-237

## Структура (gpui-дерево кратко)
```
main_wrap: div .flex_1 .min_w(PANEL_MIN_SIZE=100) .h_full
└─ main_column (при main_bottom_visible — flex-col из 53+55+54; иначе gap_wrap(chat_content))
   └─ div h=relative(main_split) .min_h(100) → gap_wrap_v(pt4, pb0)
      └─ chat_content = glint_surface_wv_holed(
           div#main-slot .relative .size_full
           └─ slot_panel(Main, state, "Left", SlotIcon::Main, picker_up=false, drag_over, body))
```
slot_panel: если pinned>0 — стрип табов (h24-пилюли + «…») сверху, затем тело активного тула (tool_body) либо panel_placeholder("Left").

## Метрики (из кода, точные)
- Карточка (glint): внешний радиус RADIUS_LG=16, кромка 1px из двух 2-стоповых 135°-градиентов glint_edge (dark #ffffff α.18 / light #3c2814 α.18) 0→22% и 78%→100%, mid = glint_mid (dark #262533=bg_mantle / light #e6e1d4=bg_surface), внутренняя заливка inset 1px радиус 15 = bg_mantle (dark #262533 / light #fbf7f4)
- Высота: `relative(main_split)`, main_split кламп [MAIN_SPLIT_MIN 0.2, MAIN_SPLIT_MAX 0.85], дефолт 0.7; без нижнего ящика — 100% (gap_wrap с pt и pb 4)
- min-width 100 (PANEL_MIN_SIZE), min_h 100 у верхней секции
- Стрип: px 8, pt 4, gap 2; таб h 24, px 12, rounded 8, fs 12; active bg accent_primary α.16
- Дыры под composition-вебвью: glint рисуется paint_quad-сегментами вокруг зон + угловые маски R=12

## Отличия от original.md той же папки
1. Drop-индикация `data-activity-drop="over"/"blocked"` (accent-tint 10% + dashed outline / red-tint 12% + inset shadow) НЕ реализована — drag тулов подсвечивает только позицию вставки в стрипе (border_l 2 accent).
2. Оригинал: BottomTabBar + ActivityBody как отдельные дети `.main`; у нас единый slot_panel (стрип+тело) — структура эквивалентна, но таб-метрики свои (см. 65).
3. Высота: оригинал — инлайн `height: N%` (toFixed(2)); у нас `relative(main_split)` — та же доля, без строкового округления.
4. Customize/Welcome не внутри main-content: обрабатываются на уровне body (см. 52); оригинал рендерит CustomizePanel/WelcomePlaceholder внутри `.main`.
5. glint-кромка: CSS 4-стоповый linear-gradient → два наложенных 2-стоповых слоя (лимит gpui 0.2.2); пиксельно эквивалентно.
6. aria (`main[aria-label="Left"]`, data-activity-slot) отсутствует — в gpui нет DOM.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — карта `chat_content` (`root.rs:4845-4891`, glint + `slot_panel`) hover-стилей не имеет, у `.main` оригинала их тоже нет; drag-подсветка `data-activity-drop` не портирована (уже отмечено в «Отличиях»), а ховер табов стрипа — элемент 49/65.

### Вердикты

# 53 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: glint r16, height mainSplit, welcome-ветка. Расхождения: нет
data-activity-drop (over accent10%+dashed / blocked red12%+inset); BottomTabBar
скрыт при пустом pinned (оригинал всегда); лишний min_h(100); нет flex-shrink 0.

## Цикл 5: DIVERGES

Нет drop-состояний `[data-activity-drop=over|blocked]` (`global.css:53-67`: accent 10% + 1px dashed accent 60% offset −2 / red 12% + inset 2px red 60%). Стрип скрыт при пустом `pinned` — оригинал всегда рисует `.strip` с пикером. Лишняя пилюля «Open Tool ▾»: `MainContent.tsx:55` даёт placeholder БЕЗ `activitySlot`, у нас `slot_panel` всегда передаёт кнопку.

## Цикл 6: DIVERGES

Лишняя пилюля у центральной карты убрана ✓. Осталось: drop-состояния `over`/`blocked`, стрип скрыт при пустом `pinned` (оригинал рисует его всегда).

---

## 54. main-bottom-panel — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](54-main-bottom-panel/original.png)
![наш](54-main-bottom-panel/ours.png)

### Оригинал

# 54 main-bottom-panel — оригинал
Файлы: kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.tsx (строки 57-86), kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.module.css

## JSX-структура (кратко, вложенность)
```
section.panel [aria-label="Left Bottom"]  style={ height: `${heightPct}%` }
├─ div.resizeHandle [role=separator aria-orientation=horizontal] [data-tooltip="Drag to resize"]  (элемент 55)
│  └─ span.resizeHandleBar [aria-hidden]
└─ div.card [data-activity-slot="mainBottom"] [data-activity-drop=over|blocked|undefined]
   ├─ <BottomTabBar slot="mainBottom" />
   └─ <Body/>: activeId ? <ActivityBody id slot="mainBottom" />
                        : <PanelPlaceholder label="Left Bottom" slot="main-bottom" activitySlot="mainBottom" />
```
Рендер null при `!mainBottomVisible`. Высота: `mainVisible ? (1 - mainSplit)*100 : 100`%, `toFixed(2)`. mainSplit клампится в [MAIN_SPLIT_LOWER=0.2, MAIN_SPLIT_UPPER=0.85] (config/constants.ts:75-76).

## Метрики (ИЗ CSS, точные значения)
### .panel
- flex-shrink: 0; display: flex; flex-direction: column; position: relative
- фона нет (гейт-фон просвечивает); height — инлайн-процент

### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### Drop-индикация — глобальные `[data-activity-drop="over"/"blocked"]` (см. 53, theme/global.css:53-67)

## Состояния (классы-варианты с метриками)
- mainBottomVisible=false → null
- mainVisible=false → height 100% колонки
- data-activity-drop="over"/"blocked" на .card — глобальные метрики
- hover/transition собственных нет (у card)

### Наша реализация

# 54 main-bottom-panel — наша реализация
Файлы: crates/shell/src/root.rs:4046-4110 (ветка main_bottom_visible; сам ящик 4078-4106); crates/shell/src/ui/glint.rs:122-233; crates/shell/src/ui/slot_panel.rs:187-237

## Структура (gpui-дерево кратко)
```
main_column (flex-col, при layout.main_bottom_visible)
├─ [верх: chat_content, h=relative(main_split)]        — элемент 53
├─ h_handle("main-bottom-handle")                      — элемент 55
└─ div .flex_1 .min_h(0) .min_w(0)
   └─ gap_wrap_v(pt=false, pb=true)  (px 4, pb 4)
      └─ glint_surface_wv_holed(
           div#main-bottom .relative .size_full + probe_area("main-bottom")
           └─ slot_panel(MainBottom, state, "Left Bottom",
                SlotIcon::MainBottom, picker_up=true, drag_over, body))
```
`main_bottom_visible=false` → ветка else: только gap_wrap(chat_content).

## Метрики (из кода, точные)
- Высота: `flex_1` — остаток колонки после верха relative(main_split); эквивалент (1 − mainSplit)·100%
- Карточка: glint radius 16 / inner 15, кромка edge α.18 (dark #ffffff / light #3c2814), заливка bg_mantle (#262533 / #fbf7f4)
- gap_wrap_v: px 4, pb 4, pt 0 (смежный с ручкой паддинг убран — вертикальный зазор = 8px ручки)
- Пустое состояние: panel_placeholder «Left Bottom» + пилюля «Open Tool ▾» (accent α.16, hover α.26), пикер открывается ВВЕРХ (picker_up=true)
- Стрип табов как у 53 (h24, px12, rounded 8, fs 12)

## Отличия от original.md той же папки
1. Drop-индикация `data-activity-drop="over"/"blocked"` на карточке НЕ реализована.
2. Оригинал: ручка (элемент 55) — ребёнок section.panel; у нас ручка — сиблинг между верхом и ящиком в main_column. Итоговая геометрия та же.
3. Высота: оригинал инлайн-процент `(1-mainSplit)*100%` на section.panel; у нас flex_1 при фиксированном верхе — та же доля.
4. `mainVisible=false → height 100%` не поддержано: у нас при скрытом main скрыт весь main_wrap (ящик исчезает вместе с колонкой).
5. section/aria-label «Left Bottom» → нет DOM; label живёт в плейсхолдере.

### Вердикты

# 54 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: карта glint+r16, гейт, ручка, плейсхолдер+Open Tool.
Расхождения: нет drop-индикации; стрип скрыт при пустом pinned.

## Цикл 5: DIVERGES

Как 53: нет drop-индикации, стрип скрыт при пустом `pinned`. Карта, гейт, ручка, плейсхолдер с пилюлей — 1:1.

## Цикл 6: DIVERGES

Как 53 — drop-индикация и всегда видимый стрип.

---

## 55. main-bottom-resize-handle — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](55-main-bottom-resize-handle/original.png)
![наш](55-main-bottom-resize-handle/ours.png)

### Оригинал

# 55 main-bottom-resize-handle — оригинал
Файлы: kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.tsx (строки 64-73), kamin-ide/src/renderer/components/main-bottom-panel/MainBottomPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.resizeHandle [role="separator"] [aria-orientation="horizontal"]
  [aria-label="Resize Left Bottom"] [data-tooltip="Drag to resize"]
  onMouseDown={onResizeDown}
└─ span.resizeHandleBar [aria-hidden="true"]
```
Drag: cursor "row-resize" через useDragHandler; delta по Y / высоту колонки прибавляется к mainSplit, кламп [0.2, 0.85]. Если `!mainVisible` — drag не начинается.

## Метрики (ИЗ CSS, точные значения)
### .resizeHandle
- flex-shrink: 0; height: 10px; width: 100%
- cursor: row-resize
- display: flex; align-items: center; justify-content: center
- position: relative; user-select: none
- background: transparent; border: none; padding: 0; color: inherit; font: inherit
- `:focus { outline: none; }`

### .resizeHandleBar (грип)
- display: block; width: 32px; height: 3px
- background: var(--bg-overlay)
- border-radius: var(--radius-xs)
- opacity: 0.7
- transition: opacity 0.15s, background 0.15s
- pointer-events: none

## Состояния (классы-варианты с метриками)
- `.resizeHandle:hover .resizeHandleBar`: opacity 1; background var(--accent-primary)
- focus: outline: none
- active-класса нет (drag через JS)

### Наша реализация

# 55 main-bottom-resize-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:88-136 (h_handle); crates/shell/src/root.rs:4059-4077 (вызов), 2983-2988 (DragKind::MainBottom), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
h_handle("main-bottom-handle", show, pr=0)
= div .flex_shrink_0 .h(SPACE_2=8) .min_w(0) .pr(0)
    .flex .items_center .justify_center .cursor_row_resize
    .tooltip("Drag to resize") .on_mouse_down(Left) .on_hover
  └─ div .relative (probe_area("main-bottom-handle") + грип)
     └─ div 32×3 .rounded(RADIUS_XS=4)
```
show = hovered_handle == id || dragging(MainBottom) — hover state-driven через RootView.hovered_handle (не CSS :hover).

## Метрики (из кода, точные)
- Hit-зона: высота 8px (SPACE_2), ширина — stretch колонки
- Грип: 32×3px, radius 4 (RADIUS_XS)
  - idle: bg_overlay (dark #515567 / light #d6d0c0), opacity 0.7
  - hover/drag: accent_primary (dark #89b4fa / light #da8343), opacity 1
- Drag: `main_split = init + dy/body_h`, кламп [MAIN_SPLIT_MIN 0.2, MAIN_SPLIT_MAX 0.85]; персист `mainSplit` одним патчем на mouse-up (end_drag)
- Курсор row-resize; tooltip «Drag to resize»

## Отличия от original.md той же папки
1. Высота hit-зоны 8px против 10px оригинала (`.resizeHandle { height: 10px }`).
2. Нет transition 0.15s (opacity/background) — переключение мгновенное.
3. Hover реализован состоянием (hovered_handle) — визуально то же, но подсветка не сработает во время чужого драга.
4. Гард оригинала «`!mainVisible` → drag не начинается» не нужен: при скрытом main ручка не рендерится вовсе.
5. role="separator"/aria — нет DOM; tooltip совпадает.

## Дополнение атрибутов (цикл 10)

- отступы: собственных паддингов нет — `h_handle(..., pr = 0.0)` в вызове для main-bottom (`root.rs:4906-4910`), т.е. `.pr(px(0))` (`splitter.rs:126`); грип 32×3 центрируется `justify_center` по всей ширине колонки. Для сравнения: у правой колонки та же функция вызывается с `pr = ACTIVITY_BAR_WIDTH 48` (`root.rs:5532`), у file-bottom тоже 0 (`root.rs:5420`).

### Вердикты

# 55 — verdict (review cycle 1)
VERDICT: MATCH
h10, грип 32×3 r4 overlay .7 / hover accent 1.0 — 1:1 (transition .15s — допуск).

## Цикл 5: MATCH

Ручка main-bottom: h 10, грип 32×3 r4, bg-overlay .7 → accent 1.0. `transition .15s` — ограничение gpui.

## Цикл 6: MATCH

Ручка main-bottom 1:1.

---

## 56. right-panel-column — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](56-right-panel-column/original.png)
![наш](56-right-panel-column/ours.png)

### Оригинал

# 56 right-panel-column — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 102-110), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.column [aria-label="Right activity column"]  ref=columnRef
  style = fill ? { flex: "1 1 0", minWidth: RIGHT_PANEL_MIN_WIDTH_PX }
               : { width: rightPanelWidth px, minWidth: RIGHT_PANEL_MIN_WIDTH_PX }
├─ div.resizeHandle (только !fill; элемент 57)
├─ div.cardWithBar (topPct)      — элемент 58
├─ div.splitHandle (bottomShown) — элемент 59
└─ div.cardWithBar (bottomPct, bottomShown) — элемент 60
```
Рендер null при `!rightPanelVisible`. RIGHT_PANEL_MIN_WIDTH_PX = 100 (config/constants.ts:51). topPct = bottomShown ? split*100% : "100%"; bottomPct = (1-split)*100% (toFixed(2)); split клампится [0.15, 0.85].

## Метрики (ИЗ CSS, точные значения)
### .column
- display: flex; flex-direction: column
- flex-shrink: 1 (сжимается до min-width при тесноте)
- min-height: 0; position: relative
- фона нет — гейт-фон просвечивает между двумя карточками
- width / min-width / flex — инлайн (см. выше)

## Состояния (классы-варианты с метриками)
- fill=true: `flex: 1 1 0` вместо фикс-ширины; width-handle не рендерится
- rightPanelVisible=false → null
- bottomShown=false: только верхняя карточка, height 100%
- hover/transition собственных нет

## Дополнение атрибутов (цикл 10)

- цвета: собственного фона у `.column` нет (`RightPanel.module.css:4-12` — только flex/min-height/position), сквозь зазор между картами просвечивает подложка `.appWrapper`: `var(--bg-sidebar)` = #1d1d28 dark (`dark-theme.css:13`) / #f4f1ea light (`light-theme.css:26`) плюс два radial-слоя (accent-purple 8% / accent-primary 6%, `AppLayout.module.css:12-14`).
- отступы: у `.column` и `.cardWithBar` padding/margin нет (`RightPanel.module.css:4-22`); всё межпанельное расстояние даёт родитель `.body { gap: var(--space-2) 8px; padding: 0 var(--space-1) 4px }` (`AppLayout.module.css:31,37`); ширину/минимум колонка получает инлайн (width = rightPanelWidth, min-width 100).

### Наша реализация

# 56 right-panel-column — наша реализация
Файлы: crates/shell/src/root.rs:4605-4705 (right_column_el), 4759-4764 (right_wrap), 3976 (right_w); crates/shell/src/ui/right_column.rs:249-266 (card_with_rail), 125-246 (rail)

## Структура (gpui-дерево кратко)
```
right_wrap: div .w(right_w + ACTIVITY_BAR_WIDTH=44) .flex_shrink_0 .h_full
└─ right_column_el: div .flex .flex_col .size_full .min_w(0)
   ├─ div h=relative(right_split) .min_h(100) → card_with_rail(top)   — элемент 58
   ├─ h_handle("right-split-handle", pr=44)                            — элемент 59
   └─ div .flex_1 .min_h(0) → card_with_rail(bottom, rail_bottom=true) — элемент 60
```
Показ гейтится `layout.right_panel_visible` (when(rv) в body). Ручка ширины (между file и right) — сиблинг в body: `file_right_handle` (элемент 57).

## Метрики (из кода, точные)
- Ширина: right_w = layout.right_panel_width_px.round() (дефолт RIGHT_PANEL_DEFAULT=280) + 44 rail; min при драге PANEL_MIN_SIZE=100
- right_split: поле RootView, дефолт RIGHT_SPLIT_DEFAULT=0.55, кламп [0.15, 0.85]
- Колонка без фона (просвечивает bg_sidebar+radial), rail width 44 (ACTIVITY_BAR_WIDTH)
- flex_shrink_0 на right_wrap; min_w(0) внутри

## Отличия от original.md той же папки
1. fill-режим (`flex: 1 1 0` при скрытом центре) НЕ реализован — всегда фикс-ширина.
2. `flex-shrink: 1` + min-width 100 оригинала → у нас flex_shrink_0: при тесноте колонка не сжимается (сжимается main).
3. bottomShown-гейт отсутствует: нижняя карточка и split-handle рендерятся всегда (rightPanelBottomVisible нет в нашей layout-модели).
4. right_split НЕ персистится (end_drag сохраняет только sidebar/file/right width + mainSplit + fileBottom) — оригинал сохраняет rightPanelSplit.
5. Ширина колонки включает rail (+44): оригинальная rightPanelWidth задаёт всю колонку вместе с ActivityBar; у нас right_w — контентная часть, rail добавляется сверху. При равных сохранённых числах наша колонка на 44px шире.
6. width-handle не absolute внутри колонки, а нулевой сиблинг в body (см. 57).

## Дополнение атрибутов (цикл 10)

- цвета: колонка фона не имеет (`root.rs:5470-5590`) — просвечивает корневая заливка bg_sidebar #1d1d28 dark / #f4f1ea light (`root.rs:6060`, `palette.rs:56,94`) плюс два запечённых radial-спрайта: accent_purple #cba6f7 / #8a5fc8 при peak α 0.08 и accent_primary #89b4fa / #da8343 при peak α 0.06 (`radial_bg.rs:96-97`, `palette.rs:76,83,114,121`).
- отступы: у `right_wrap`/`right_column_el` padding нет (`root.rs:5644-5650`, `:5470`); горизонтальный полузазор даёт `gap_wrap` карты — `pl 4`, справа 0, карта вплотную к рейлу (`right_column.rs:21-31`); рейл — `py 12` (SPACE_3, `right_column.rs:145`).
- гэпы: flex-`gap` у колонки нет; 8px до соседа собираются из `pl 4` двух смежных `gap_wrap`; вертикальный зазор между картами = хит-зона `h_handle` 10px (`splitter.rs:121`, вызов `root.rs:5528-5532` с `pr = 48`); внутри рейла gap 8 между группами (`right_column.rs:144`) и gap 2 между плитками (`right_column.rs:155`).
- ховер: N/A: ховер — `right_column_el` (`root.rs:5470-5590`) и `card_with_rail` (`right_column.rs:210-227`) hover-стилей не задают; ховер есть только у плиток рейла (`right_column.rs:96` — `bg_surface` α .5) и у split-ручки (элемент 59).

### Вердикты

# 56 — verdict (review cycle 1)
VERDICT: DIVERGES
Ширина = persisted + 44 (оригинал: persisted ВКЛЮЧАЕТ rail); flex_shrink_0 vs
shrink 1 + min-w 100; fill-режим не реализован.

## Цикл 5: DIVERGES

`.column { flex-shrink: 1 }` + min-width 100 → у нас `flex_shrink_0`: при тесноте колонка не сжимается и давит центр. fill-режима нет. Ширина без лишнего +rail и гейт `bottomShown` — исправлены.

## Цикл 6: DIVERGES

`flex_shrink_0` вместо shrink 1 + `min_w 100`; fill-режима нет.

---

## 57. right-panel-width-handle — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](57-right-panel-width-handle/original.png)
![наш](57-right-panel-width-handle/ours.png)

### Оригинал

# 57 right-panel-width-handle — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 113-124), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.resizeHandle (+ .resizeHandleActive при drag)  [data-tooltip="Drag to resize"]
  [role="separator"] [aria-orientation="vertical"] [aria-label="Resize right panel"]
  onMouseDown={onWidthDown}
└─ span.resizeHandleBar [aria-hidden="true"]
```
Drag: cursor "col-resize"; drag влево растит правую панель. Если File-панель видима — торг между Right и File (кламп RIGHT_PANEL_MIN_WIDTH_PX=100 / FILE_PANEL_MIN_WIDTH_PX=100, вызывает `layoutActiveEditorNow()`); иначе рост против центра через `clampGrowth(..., MAIN_MIN_WIDTH_PX=100)`. Не рендерится при fill.

## Метрики (ИЗ CSS, точные значения)
### .resizeHandle
- position: absolute; top: 0; left: calc(-1 * var(--space-2)) — целиком в ЛЕВОМ зазоре
- width: var(--space-2); height: 100%
- cursor: col-resize
- z-index: var(--z-resize-handle)
- user-select: none; display: flex; align-items: stretch; justify-content: center

### .resizeHandleBar
- display: block; width: 2px; height: 100%
- opacity: 0
- background: `linear-gradient(to bottom, transparent 0%, var(--bg-overlay) 30%, var(--bg-overlay) 70%, transparent 100%)`
- transition: opacity 0.15s, background 0.15s, width 0.15s
- pointer-events: none

## Состояния (классы-варианты с метриками)
- `.resizeHandle:hover .resizeHandleBar` и `.resizeHandleActive .resizeHandleBar` (drag):
  - opacity: 1; width: 3px
  - background: `linear-gradient(to bottom, transparent 0%, var(--tint-primary-strong) 30%, var(--tint-primary-strong) 70%, transparent 100%)`

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin нет; вся геометрия — инсет `position: absolute; top: 0; left: calc(-1 * var(--space-2))` = −8px при `width: var(--space-2)` 8px, `height: 100%` (`RightPanel.module.css:31-43`), т.е. хит-зона целиком лежит в 8px-зазоре слева от колонки.

### Наша реализация

# 57 right-panel-width-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:22-86 (v_handle, v_bar); crates/shell/src/root.rs:4740-4758 (file_right_handle), 2959-2975 (DragKind::FileRight), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
v_handle("file-right-handle") — сиблинг между file_wrap и right_wrap в body
= div .relative .w(0) .h_full .flex_shrink_0
  └─ div .absolute .left(-4) .top_0 .w(SPACE_2=8) .h_full
       .flex .items_center .justify_center .cursor_col_resize
       .tooltip("Drag to resize") .on_mouse_down(Left) .on_hover
     └─ when(show): v_bar(3px)  — 3 сегмента: fade-in 30% / solid 40% / fade-out 30%
```
show = hovered_handle == "file-right-handle" || dragging(FileRight).

## Метрики (из кода, точные)
- Hit-зона: ширина 8px (SPACE_2), центрирована на стыке (absolute left −4 от нулевого элемента)
- Полоса: ширина 3px, высота 100%; цвет tint(accent_primary, 0.25) — «tint-primary-strong»; концы растворяются 2-стоповыми градиентами 180° (0–30% fade-in, 30–70% solid, 70–100% fade-out)
- idle: полоса не рендерится вовсе
- Drag (FileRight): трейд file↔right: nf=init.0+d, nr=init.1−d, взаимный кламп PANEL_MIN_SIZE=100; персист filePanelWidthRatio + rightPanelWidthPx на mouse-up

## Отличия от original.md той же папки
1. Позиция hit-зоны: оригинал — absolute `left: -8px, width 8px` ЦЕЛИКОМ в левом зазоре; у нас −4..+4 — центр на стыке, 4px заходят на панель.
2. Направление трейда: оригинал «drag влево растит правую панель», торг right↔file; у нас торг file↔right той же ручкой (эквивалент), но fallback-ветки «file скрыт → рост против центра через clampGrowth(MAIN_MIN=100)» нет — при скрытой file-панели ручка всё равно двигает скрытую file-ширину (баг-расхождение; в body ручка привязана к when(rv), а не fv).
3. idle-состояние: оригинал держит полосу 2px bg_overlay с opacity 0 + transition 0.15s; у нас элемент отсутствует и появляется мгновенно (без анимации). Видимое hover/drag-состояние совпадает (3px, tint-primary-strong, растворение 30/70%).
4. `layoutActiveEditorNow()` не нужен — gpui-редактор релэйаутится в том же кадре.
5. z-index var(--z-resize-handle) не нужен: hit-зона живёт в зазоре gap_wrap, перекрытий нет.
6. role/aria-label «Resize right panel» — нет DOM.

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding нет; геометрия — нулевой сиблинг `w(0)` с хит-зоной `absolute; left −4; top 0; w SPACE_2 8; h 100%` (`splitter.rs:63-79`), т.е. инсет −4 против −8 у оригинала: хит центрирован на стыке и 4px заходит на кромку панели.
- цвета: полоса `tint(accent_primary, 0.25)` (`splitter.rs:62`) = #89b4fa α .25 dark / #da8343 α .25 light (`palette.rs:83,121`) — эквивалент `--tint-primary-strong` (`variables.css:110,128`); idle-полосы нет вовсе (элемент не рендерится), у оригинала idle = `--bg-overlay` #515567/#d6d0c0 при opacity 0.

### Вердикты

# 57 — verdict (review cycle 1)
VERDICT: DIVERGES
Рецепт полосы/торга — 1:1. Позиция: для нашей эмуляции зазора верно left(-4)
(стык = центр зазора); left(-8) залезал на кромку карты — ИСПРАВЛЕНО после ревью.

## Цикл 5: MATCH

Ручка ширины правой колонки: hit 8, полоса 3px `tint-primary-strong` (accent 25% в обеих темах), растворение 30/70%. Idle-полоса 2px@0 и transition — ограничение gpui.

## Цикл 6: MATCH

Ручка ширины правой колонки 1:1.

---

## 58. right-panel-top-card — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](58-right-panel-top-card/original.png)
![наш](58-right-panel-top-card/ours.png)

### Оригинал

# 58 right-panel-top-card — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 133-151), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.cardWithBar  style={ height: topPct }  onDragOver/onDragLeave/onDrop
├─ aside.card [aria-label="Right"] [data-activity-slot="rightTop"]
│    [data-activity-drop=over|blocked|undefined]
│  └─ topActive ? <ActiveTool slot="rightTop" /> (→ ActivityBody)
│              : <PanelPlaceholder label="Right" slot="right-top" activitySlot="rightTop" />
└─ <ActivityBar slot="rightTop" align="top" />
```
topPct = bottomShown ? (rightPanelSplit*100).toFixed(2)% : "100%".

## Метрики (ИЗ CSS, точные значения)
### .cardWithBar (обёртка card + activity bar)
- display: flex; flex-direction: row; min-height: 0
- height — инлайн-процент
- `.cardWithBar > aside.card { flex: 1; min-width: 0; }`

### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### Прочие классы модуля (используются телами карточек)
- .cardHeader: padding 8px 12px; text-transform uppercase; font-size var(--fs-xs); font-weight 500; letter-spacing 0.08em; color var(--text-muted)
- .empty: flex 1; flex-direction column; align-items center; justify-content center; gap var(--space-1); padding var(--space-4); color var(--text-muted); text-align center
- .empty > i: font-size 24px; opacity 0.4; margin-bottom var(--space-1)
- .empty > p: margin 0; font-size var(--fs-sm)

## Состояния (классы-варианты с метриками)
- data-activity-drop="over": background `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; outline `1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent)`; outline-offset -2px (theme/global.css:53)
- data-activity-drop="blocked": background `color-mix(in srgb, var(--accent-red) 12%, transparent)`; box-shadow `inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent)` (theme/global.css:63)
- bottomShown=false → height 100%

### Наша реализация

# 58 right-panel-top-card — наша реализация
Файлы: crates/shell/src/root.rs:4612-4650; crates/shell/src/ui/right_column.rs:249-266 (card_with_rail), 34-122 (rail_tile), 125-246 (rail); crates/shell/src/ui/glint.rs:122-233

## Структура (gpui-дерево кратко)
```
div h=relative(right_split) .min_h(100) .w_full
└─ card_with_rail(rail_bottom=false): div .flex .size_full .min_w(0)
   ├─ gap_wrap(card, pt=4, pb=0)  (px 4)
   │  └─ glint_surface_wv_holed(
   │       div#right-top .relative .size_full + probe_area("right-top")
   │       └─ tool_body(RightTop) | panel_placeholder("Right Top", …, SlotIcon::RightTop))
   └─ rail(RightTop): div .w(44) .h_full .flex_col .items_center .gap(2) .py(12)
      ├─ rail_tile ×N (pinned): 32×32, rounded 8, иконка 18px (phosphor svg) / 16px (codicon)
      └─ dots «…» 32×32 (codicon ea7c 15px, tooltip "Add or remove items")
```
Карта БЕЗ таб-стрипа (тулы — в рейле), тело = чистое тело активного тула.

## Метрики (из кода, точные)
- Карточка: glint radius 16 / inner 15, кромка edge α.18, заливка bg_mantle (#262533 dark / #fbf7f4 light)
- Высота: relative(right_split), min_h 100
- Rail: ширина 44 (ACTIVITY_BAR_WIDTH), py 12 (SPACE_3), gap 2; плитка 32×32 rounded 8; active bg = accent_primary α.16; hover bg = bg_surface α.5 (#3d3f51/50%); иконка active text_primary (#cfd4e2), иначе text_muted (#838aa0)
- Placeholder: label «Right Top» fs 16 semibold, hint fs 12, глиф SlotIcon::RightTop (scale 2.8)

## Отличия от original.md той же папки
1. Label плейсхолдера «Right Top» — оригинал «Right» (aria-label карточки "Right").
2. Drop-индикация `data-activity-drop="over"/"blocked"` НЕ реализована (нет accent-tint 10% + dashed outline / red 12% + inset shadow).
3. Rail = наша реализация ActivityBar слота: ширина 44 (token), у оригинального `.splitHandle` fallback var = 48px — если фактический CSS-var 44, совпадает; сами метрики плиток (32×32/gap 2/py 12) — наши, сверка с элементом 38 отдельно.
4. `.cardWithBar > aside.card { flex: 1 }` → у нас gap_wrap с size_full + min_w(0), rail flex_shrink_0 — эквивалент.
5. Классы .cardHeader/.empty оригинального модуля не портированы (тела карточек используют свои компоненты).
6. bottomShown=false → height 100% — не поддержано (низ всегда виден, см. 56).

## Дополнение атрибутов (цикл 10)

- шрифты: собственного текста у карты нет (`root.rs:5491-5518` — glint-обёртка + тело тула), `.cardHeader` оригинала (fs-xs 11 / weight 500 / ls .08em) в `RightPanel.tsx:140-149` не используется и у нас не портирован. Кегли приходят от содержимого: плейсхолдер «Right» — заголовок fs-lg 16 + weight 600 и подсказка fs-sm 12 при line-height 1.3 (`panel_placeholder.rs:123-135`), пилюля «Open Tool ▾» fs-sm 12 + глиф FontAwesome 10 (`slot_panel.rs:192,209`), глифы рейла — codicon/phosphor 18 (`right_column.rs:69-78`).

### Вердикты

# 58 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: cardWithBar, glint r16, split%, label Right, rail-структура.
Расхождения: нет пилюли Open Tool (activitySlot=rightTop); зазор card/rail 4 vs 0;
нет drop-индикации; rail 44 vs 48, gap 2 vs 8, codicon 16/15 vs 18; лишний min_h 100.

## Цикл 5: DIVERGES

`RightPanel.tsx:148` передаёт `activitySlot="rightTop"` → у оригинала в пустом состоянии ЕСТЬ пилюля «Open Tool»; у нас placeholder без `extra`. Плюс нет drop-индикации. Рейл 48, гэпы 8+2, py 12, иконка 18, label «Right» — 1:1.

## Цикл 6: MATCH

Пилюля «Open Tool» добавлена (рецепт 1:1 с `.trigger`). Остаток зоны — общая drop-индикация (см. 53).

---

## 59. right-panel-split-handle — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](59-right-panel-split-handle/original.png)
![наш](59-right-panel-split-handle/ours.png)

### Оригинал

# 59 right-panel-split-handle — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 155-164), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.splitHandle [role="separator"] [aria-orientation="horizontal"]
  [aria-label="Resize right-panel split"] [data-tooltip="Drag to resize"]
  onMouseDown={onSplitDown}
└─ span.splitGrip [aria-hidden="true"]
```
Рендерится только при bottomShown. Drag: cursor "row-resize"; delta по Y / высоту колонки прибавляется к rightPanelSplit, кламп [RIGHT_PANEL_SPLIT_LOWER=0.15, RIGHT_PANEL_SPLIT_UPPER=0.85] (config/constants.ts:89-90).

## Метрики (ИЗ CSS, точные значения)
### .splitHandle
- flex-shrink: 0; height: 10px
- cursor: row-resize
- position: relative; display: flex; align-items: center; justify-content: center
- background: transparent (гейт-фон просвечивает между карточками)
- padding-right: var(--layout-activity-bar-width, 48px) — грип центрируется по карточке, не по колонке (activity bar справа не рассекается)

### .splitGrip
- display: block; width: 32px; height: 3px
- background: var(--bg-overlay)
- border-radius: var(--radius-xs)
- opacity: 0.7
- transition: opacity 0.15s, background 0.15s

## Состояния (классы-варианты с метриками)
- `.splitHandle:hover .splitGrip`: opacity 1; background var(--accent-primary)
- active-класса нет (drag через JS)

### Наша реализация

# 59 right-panel-split-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:88-136 (h_handle); crates/shell/src/root.rs:4652-4670 (вызов, pr=ACTIVITY_BAR_WIDTH), 2989-2993 (DragKind::RightSplit)

## Структура (gpui-дерево кратко)
```
h_handle("right-split-handle", pr=44)
= div .flex_shrink_0 .h(8) .min_w(0) .pr(44)
    .flex .items_center .justify_center .cursor_row_resize
    .tooltip("Drag to resize")
  └─ div .relative (probe_area + грип 32×3 rounded 4)
```
Сиблинг между верхней и нижней card_with_rail в right_column_el.

## Метрики (из кода, точные)
- Hit: высота 8px (SPACE_2); pr 44 (ACTIVITY_BAR_WIDTH) — грип центрируется по карточке, не по колонке (rail справа не рассекается)
- Грип: 32×3, radius 4; idle bg_overlay (#515567/#d6d0c0) opacity 0.7; hover/drag accent_primary (#89b4fa/#da8343) opacity 1
- Drag: right_split = init + dy/body_h, кламп [RIGHT_SPLIT_MIN 0.15, RIGHT_SPLIT_MAX 0.85]

## Отличия от original.md той же папки
1. Высота hit-зоны 8px против 10px оригинала.
2. pr = 44 против `padding-right: var(--layout-activity-bar-width, 48px)` — при токене 44 совпадает, fallback 48 не воспроизводим.
3. Нет transition 0.15s.
4. Рендерится всегда (bottomShown-гейта нет — низ правой колонки не отключаем).
5. right_split после драга НЕ персистится (в end_drag патче отсутствует) — оригинал сохраняет rightPanelSplit.
6. role/aria — нет DOM.

### Вердикты

# 59 — verdict (review cycle 1)
VERDICT: DIVERGES
Рецепт — 1:1. pr ручки 44 vs 48 (корень: ACTIVITY_BAR_WIDTH=44 против токена 48).

## Цикл 5: DIVERGES

`rightPanelSplit` не персистится: в `end_drag` патч без этого поля. Рецепт h10 + `pr 48` — 1:1.

## Цикл 6: DIVERGES

`rightPanelSplit` по-прежнему не персистится.

---

## 60. right-panel-bottom-card — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](60-right-panel-bottom-card/original.png)
![наш](60-right-panel-bottom-card/ours.png)

### Оригинал

# 60 right-panel-bottom-card — оригинал
Файлы: kamin-ide/src/renderer/components/right-panel/RightPanel.tsx (строки 166-184), kamin-ide/src/renderer/components/right-panel/RightPanel.module.css

## JSX-структура (кратко, вложенность)
```
div.cardWithBar  style={ height: bottomPct }  onDragOver/onDragLeave/onDrop
├─ aside.card [aria-label="Right Bottom"] [data-activity-slot="rightBottom"]
│    [data-activity-drop=over|blocked|undefined]
│  └─ bottomActive ? <ActiveTool slot="rightBottom" /> (→ ActivityBody)
│                 : <PanelPlaceholder label="Right Bottom" slot="right-bottom" activitySlot="rightBottom" />
└─ <ActivityBar slot="rightBottom" align="bottom" />   (зеркальный: пикер сверху)
```
Рендерится только при `rightPanelBottomVisible`. bottomPct = ((1 - rightPanelSplit)*100).toFixed(2)%.

## Метрики (ИЗ CSS, точные значения)
Идентичны верхней карточке (те же классы .cardWithBar / .card):
### .cardWithBar
- display: flex; flex-direction: row; min-height: 0; height — инлайн-процент
- `.cardWithBar > aside.card { flex: 1; min-width: 0; }`

### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

## Состояния (классы-варианты с метриками)
- data-activity-drop="over"/"blocked" — глобальные (theme/global.css:53-67): accent-tint 10% + dashed outline 60% / red-tint 12% + inset box-shadow 2px red 60%
- Отличие от top-card: ActivityBar с `align="bottom"` (зеркальная раскладка бара), aria-label "Right Bottom"

### Наша реализация

# 60 right-panel-bottom-card — наша реализация
Файлы: crates/shell/src/root.rs:4671-4703; crates/shell/src/ui/right_column.rs:249-266 (card_with_rail rail_bottom=true), 141-194 (bottom-rail порядок)

## Структура (gpui-дерево кратко)
```
div .flex_1 .min_h(0) .min_w(0)
└─ card_with_rail(rail_bottom=true)
   ├─ gap_wrap(card, pt=0, pb=4)
   │  └─ glint_surface_wv_holed(
   │       div#right-bottom .relative .size_full + probe_area("right-bottom")
   │       └─ tool_body(RightBottom) | panel_placeholder("Right Bottom", …, SlotIcon::RightBottom))
   └─ rail(RightBottom, bottom=true): .justify_end; порядок детей: «…»-пикер НАД плитками
```
Пикер «…» открывается вверх (up=true). Тело rightBottom-тула = как правило вебвью плана (webview_body_dyn).

## Метрики (из кода, точные)
- Высота: flex_1 (остаток после верха relative(right_split)) — эквивалент (1−split)·100%
- Карточка: glint radius 16 / inner 15, заливка bg_mantle; те же цвета, что 58
- Rail: 44px, justify_end, gap 2, py 12; dots 32×32 сверху, затем плитки 32×32 (зеркало align=bottom оригинала: пикер над плитками)
- gap_wrap: pt 0 (смежный с ручкой), pb 4

## Отличия от original.md той же папки
1. `rightPanelBottomVisible`-гейт НЕ реализован — нижняя карточка всегда рендерится.
2. Drop-индикация `data-activity-drop` НЕ реализована.
3. Label «Right Bottom» совпадает с оригиналом (в отличие от top-card, где у нас «Right Top» vs «Right»).
4. Зеркальный ActivityBar align="bottom" → наш rail(bottom=true): justify_end + пикер над плитками — DOM-порядок оригинала {picker, list} воспроизведён.
5. Высота: оригинал инлайн `(1-split)*100%`; у нас flex_1 — та же доля без округления toFixed(2).

## Дополнение атрибутов (цикл 10)

- цвета: карта — `glint_surface_wv_holed` (`root.rs:5554-5580`, `glint.rs:28-40`): заливка glint_mid #262533 dark / #e6e1d4 light (`palette.rs:87,125`), внутренний rect bg_mantle #262533 / #fbf7f4 (`palette.rs:55,93`), кромка glint_edge #ffffff α .18 / #3c2814 α .18 (`palette.rs:86,124`). Рейл снизу: плитка idle прозрачная, hover bg_surface α .5 = #3d3f51 / #e6e1d4 (`right_column.rs:52-56`), active accent_primary α .16 = #89b4fa / #da8343 (`right_column.rs:57-61`), иконка активной text_primary #cfd4e2 / #322e28, неактивной text_muted #838aa0 / #6e685d (`right_column.rs:62-66`).

### Вердикты

# 60 — verdict (review cycle 1)
VERDICT: DIVERGES
Как 58 (низ): нет Open Tool; зазор 4 vs 0; нет drop-индикации; rail 44/gap2/codicon.

## Цикл 5: DIVERGES

Как 58: нет пилюли для `activitySlot="rightBottom"`, нет drop-индикации.

## Цикл 6: MATCH

Пилюля добавлена; drop-индикация — общий пункт 53.

---

## 61. file-panel-column — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](61-file-panel-column/original.png)
![наш](61-file-panel-column/ours.png)

### Оригинал

# 61 file-panel-column — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 91-98), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
aside.filePanel [aria-label="File column"]
  style = fill ? { flex: "1 1 0", minWidth: FILE_PANEL_MIN_WIDTH_PX }
               : { width: filePanelWidth px, minWidth: FILE_PANEL_MIN_WIDTH_PX }
├─ div.resizeHandle (только !fill; элемент 62)
├─ div.card.topCard — элемент 63
├─ div.splitHandle (filePanelBottomVisible) — элемент 64
└─ div.card.bottomCardWithTabs (filePanelBottomVisible) — элемент 65
```
Рендер null при `!filePanelVisible`. FILE_PANEL_MIN_WIDTH_PX = 100 (config/constants.ts:46).

## Метрики (ИЗ CSS, точные значения)
### .filePanel
- display: flex; flex-direction: column
- flex-shrink: 1 (сжимается до min-width, не выталкивая правую панель)
- min-height: 0; position: relative
- фона нет — гейт-фон просвечивает в зазоре между карточками
- width / min-width / flex — инлайн (см. выше)

## Состояния (классы-варианты с метриками)
- fill=true: `flex: 1 1 0`, width-handle не рендерится
- filePanelVisible=false → null
- filePanelBottomVisible=false: только верхняя карточка (flex 1)
- hover/transition собственных нет

## Дополнение атрибутов (цикл 10)

- цвета: `.filePanel` фона не имеет (`FilePanel.module.css:4-12`) — между картами и по краям просвечивает `.appWrapper`: `var(--bg-sidebar)` #1d1d28 / #f4f1ea (`dark-theme.css:13`, `light-theme.css:26`) + radial accent-purple 8% / accent-primary 6% (`AppLayout.module.css:12-14`). Карты внутри — `.card` c `--glint-border` (rgba(255,255,255,.18) на углах) и заливкой `var(--bg-mantle)` #262533 / #fbf7f4 (`FilePanel.module.css:62-70`, `dark-theme.css:12,31`).
- отступы: у колонки padding/margin нет; горизонтальный ритм задаёт родитель `.body { gap: 8px; padding: 0 4px }` (`AppLayout.module.css:31,37`), вертикальный шов между картами — `.splitHandle { height: 10px }` (`FilePanel.module.css:97-106`); собственный padding есть только у `.modeHeader` — `6px 8px 0` (`FilePanel.module.css:83`).

### Наша реализация

# 61 file-panel-column — наша реализация
Файлы: crates/shell/src/root.rs:4113-4602 (file_column), 4734-4739 (file_wrap), 3970-3975 (file_w из ratio); crates/metrics/src/lib.rs:61,70-73

## Структура (gpui-дерево кратко)
```
file_wrap: div .w(file_w) .flex_shrink_0 .h_full
└─ file_column: div .flex .flex_col .size_full .min_w(0)
   ├─ div h=relative(1 − bottom_ratio) .min_h(100) .w_full
   │  └─ gap_wrap_v_top( glint(top_card) )          — элемент 63
   ├─ h_handle("file-bottom-handle", pr=0)          — элемент 64
   └─ div .flex_1 .min_h(0) .min_w(0)
      └─ gap_wrap_v(pb=4)( glint(slot_panel CentralBottom) ) — элемент 65
```
Показ гейтится `layout.file_panel_visible`; ширинная ручка (между main и file) — сиблинг `main_file_handle` в body (элемент 62).

## Метрики (из кода, точные)
- Ширина: file_w = width_from_ratio(file_panel_width_ratio, PANEL_MIN_SIZE=100, viewport_w).round(); дефолт FILE_PANEL_DEFAULT=360 (px до первой конвертации)
- bottom_ratio: кламп [BOTTOM_RATIO_MIN 0.1, BOTTOM_RATIO_MAX 0.8]; дефолт из FILE_BOTTOM_DEFAULT=180px→ratio
- Колонка без фона; flex_shrink_0 на wrap; min_h(100) у верхней секции

## Отличия от original.md той же папки
1. Ширина хранится как ratio от вьюпорта (filePanelWidthRatio) — оригинал хранит px (filePanelWidth); при resize окна наша колонка масштабируется, оригинальная остаётся фикс-px.
2. `flex-shrink: 1` (сжатие до min-width при тесноте) → у нас flex_shrink_0 — не сжимается.
3. fill-режим (`flex: 1 1 0` при скрытом main) НЕ реализован.
4. `filePanelBottomVisible`-гейт отсутствует: split-handle и нижняя карточка всегда рендерятся.
5. Раскладка высот инвертирована: оригинал — низ фикс-px (flexShrink 0), верх flex 1; у нас верх = relative(1−ratio), низ = flex_1. Итоговые доли совпадают, но семантика ресайза иная (см. 64).
6. aside/aria-label «File column» — нет DOM.

## Дополнение атрибутов (цикл 10)

- цвета: колонка без фона (`root.rs:5399-5413`) — просвечивает bg_sidebar #1d1d28 dark / #f4f1ea light (`root.rs:6060`, `palette.rs:56,94`) + radial-спрайты accent_purple α .08 / accent_primary α .06 (`radial_bg.rs:96-97`); карты внутри — glint: заливка bg_mantle #262533 / #fbf7f4 (`palette.rs:55,93`), mid #262533 / #e6e1d4 (`palette.rs:87,125`), кромка glint_edge #ffffff α .18 / #3c2814 α .18 (`palette.rs:86,124`, `glint.rs:28-40`).
- гэпы: flex-`gap` у колонки нет — `file_col` это `div().flex().flex_col()` без gap (`root.rs:5399`); межпанельные 8px собираются из `gap_wrap` каждой карты (`px(4)` слева и справа, `root.rs:3023-3031`) → 4+4; вертикальный «gap» между верхней и нижней картой = хит-зона `h_handle` 10px (`splitter.rs:121`, вызов `root.rs:5416-5420`).
- ховер: N/A: ховер — сама колонка (`root.rs:5399-5413`) hover-стилей не имеет, как и `.filePanel`; ховеры внутри принадлежат mode-табам (66), split-ручке (64) и строкам дерева (94/95).

### Вердикты

# 61 — verdict (review cycle 1)
VERDICT: DIVERGES
Ширина ratio×viewport vs px-персист; flex_shrink_0 vs shrink1+min-w100; нет fill.

## Цикл 5: DIVERGES

Ширина хранится как `filePanelWidthRatio` от вьюпорта, у оригинала `filePanelWidth` в px; `flex_shrink_0` вместо shrink 1 + min-w 100; fill-режима нет.

## Цикл 6: DIVERGES

Ширина файловой панели всё ещё ratio от вьюпорта вместо px; `flex_shrink_0`; fill нет.

---

## 62. file-panel-width-handle — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](62-file-panel-width-handle/original.png)
![наш](62-file-panel-width-handle/ours.png)

### Оригинал

# 62 file-panel-width-handle — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 101-112), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.resizeHandle (+ .resizeHandleActive при drag)  [data-tooltip="Drag to resize"]
  [role="separator"] [aria-orientation="vertical"] [aria-label="Resize file panel"]
  onMouseDown={onWidthDown}
└─ span.resizeHandleBar [aria-hidden="true"]
```
Drag: cursor "col-resize"; `desired = max(FILE_PANEL_MIN_WIDTH_PX=100, startWidth - deltaX)`, затем `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX=100)`; на каждое изменение — синхронный `layoutActiveEditorNow()` (убивает мерцание minimap Monaco). Не рендерится при fill.

## Метрики (ИЗ CSS, точные значения)
### .resizeHandle
- position: absolute; top: 0; left: calc(-1 * var(--space-2)) — целиком в ЛЕВОМ зазоре (между main и file)
- width: var(--space-2); height: 100%
- cursor: col-resize
- z-index: var(--z-resize-handle)
- user-select: none; display: flex; align-items: stretch; justify-content: center

### .resizeHandleBar
- display: block; width: 2px; height: 100%
- opacity: 0
- background: `linear-gradient(to bottom, transparent 0%, var(--bg-overlay) 30%, var(--bg-overlay) 70%, transparent 100%)`
- transition: opacity 0.15s, background 0.15s, width 0.15s
- pointer-events: none

## Состояния (классы-варианты с метриками)
- `.resizeHandle:hover .resizeHandleBar` и `.resizeHandleActive .resizeHandleBar` (drag):
  - opacity: 1; width: 3px
  - background: `linear-gradient(to bottom, transparent 0%, var(--tint-primary-strong) 30%, var(--tint-primary-strong) 70%, transparent 100%)`

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin нет; геометрия — инсет `position: absolute; top: 0; left: calc(-1 * var(--space-2))` = −8px, `width: var(--space-2)` 8px, `height: 100%` (`FilePanel.module.css:17-29`) — хит-зона целиком в левом 8px-зазоре, ровно как у правой панели (`RightPanel.module.css:31-43`).

### Наша реализация

# 62 file-panel-width-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:22-86 (v_handle); crates/shell/src/root.rs:4715-4733 (main_file_handle), 2952-2958 (DragKind::MainFile), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
v_handle("main-file-handle") — сиблинг между main_wrap и file_wrap в body
= div .relative .w(0) .h_full .flex_shrink_0
  └─ div .absolute .left(-4) .w(8) .h_full .cursor_col_resize
       .tooltip("Drag to resize") .on_mouse_down .on_hover
     └─ when(show): v_bar 3px (fade 30% / solid 40% / fade 30%), tint(accent_primary, 0.25)
```
show = hovered_handle == id || dragging(MainFile).

## Метрики (из кода, точные)
- Hit: 8px (SPACE_2), центр на стыке (−4..+4)
- Полоса hover/drag: 3px × 100%, tint-primary-strong (accent_primary α 0.25: dark #89b4fa, light #da8343), вертикальное растворение концов 0–30 / 70–100%
- idle: пусто (нет элемента)
- Drag (MainFile): вправо → main шире; `nf = (init − d).max(PANEL_MIN_SIZE=100)` → file_panel_width_ratio = ratio_from_width(nf, viewport_w); персист на mouse-up
- Центр защищён flex: main_wrap min_w 100 flex_1

## Отличия от original.md той же папки
1. Позиция: оригинал absolute `left: -8px` целиком в зазоре; у нас −4..+4 симметрично стыку.
2. `clampGrowth(desired, prev, MAIN_MIN_WIDTH_PX=100)` не воспроизведён формулой: у нас только `max(100)`, а невозможность задавить центр обеспечивает flex-раскладка (main_wrap min_w 100). Поведение на границе близко, но file может «упереться» иначе при узком окне.
3. Результат хранится ratio (не px) — см. 61.
4. idle-полоса с opacity 0 + transition 0.15s → у нас мгновенное появление, полосы в idle нет.
5. `layoutActiveEditorNow()` (анти-мерцание Monaco minimap) не нужен — редактор наш, релэйаут в кадре.
6. role/aria «Resize file panel» — нет DOM.

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding нет — `v_handle("main-file-handle")` (`root.rs:5600-5618`) это `w(0)` сиблинг с хит-зоной `absolute; left −4; w SPACE_2 8; h 100%` (`splitter.rs:63-79`); против инсета `left: −8px` у оригинала — наша зона центрирована на стыке.

### Вердикты

# 62 — verdict (review cycle 1)
VERDICT: DIVERGES
Рецепт+кламп 100/clampGrowth — 1:1. Позиция left(-8) → left(-4) — ИСПРАВЛЕНО после ревью.

## Цикл 5: MATCH

Ручка ширины файловой панели 1:1.

## Цикл 6: MATCH

Ручка ширины 1:1.

---

## 63. file-panel-top-card — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](63-file-panel-top-card/original.png)
![наш](63-file-panel-top-card/ours.png)

### Оригинал

# 63 file-panel-top-card — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 114-129), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.card.topCard [aria-label="File card"]
├─ div.modeHeader
│  └─ <FilePanelModeTabs />   (элемент 66)
└─ (filePanelMode === "web") → <BrowserPane />          (элемент 67)
   (selectedFile)            → <FileViewer />
   (иначе)                   → <PanelPlaceholder label="File" slot="center"
                                 hint="Click a file in any panel, or drag-and-drop one from outside" />
```
Drop-target нет (верхняя карточка — editor-поверхность, без пикера и без drops).

## Метрики (ИЗ CSS, точные значения)
### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### .topCard
- flex: 1; min-height: 0 (занимает остаток высоты при открытой нижней карточке)

### .modeHeader
- display: flex; justify-content: flex-end; align-items: center
- padding: 6px 8px 0
- flex-shrink: 0

## Состояния (классы-варианты с метриками)
- Тело: web-режим / файл выбран / placeholder — переключается контентом, без классов-вариантов
- hover/active/transition собственных нет

### Наша реализация

# 63 file-panel-top-card — наша реализация
Файлы: crates/shell/src/root.rs:4496-4549 (top_card + обёртка), 4118-4314 (web-ветка), 4315-4487 (редактор), 4488-4495 (плейсхолдер); crates/shell/src/ui/glint.rs:122-233

## Структура (gpui-дерево кратко)
```
div h=relative(1 − bottom_ratio) .min_h(100)
└─ gap_wrap_v_top (px 4, pt 4, pb 0)
   └─ glint_surface_wv_holed(top_card)
      top_card: div .flex_col .size_full .min_h(0)
        .on_drop(ExternalPaths → OpenFile) .on_drop(DraggedFile → OpenFile)
      ├─ modeHeader: div .flex .justify_end .items_center .flex_shrink_0
      │    .pt(6) .px(8) → file_panel_mode_tabs (элемент 66)
      └─ top_content:
         web-режим  → browser_pane / visual_frame (элемент 67)
         есть табы  → редактор: полоса editor_tabs_bar + Save-кнопка + рамка
                      editor_bg radius 12 (breadcrumb h24 + Input + minimap + sticky)
         иначе      → panel_placeholder("File",
                      "Click a file in any panel, or drag-and-drop one from outside", SlotIcon::Center)
```

## Метрики (из кода, точные)
- Карточка: glint radius 16 / inner 15, заливка bg_mantle (#262533 / #fbf7f4)
- modeHeader: pt 6, px 8 (SPACE_2), pb 0, justify-end — точно `padding: 6px 8px 0`
- Редакторная рамка: mx 4, mt 4, mb 4, rounded 12 (RADIUS_MD), bg editor_bg (#1d1c25 / #fcfaf6); breadcrumb h 24, px 12, fs 11, JetBrains Mono, text_muted
- Save-кнопка (dirty): px 12, py 3, rounded 8, bg accent_action, fs 11 semibold, text accent_action_fg, hover opacity 0.9
- Плейсхолдер: label «File» fs 16 semibold + hint fs 12 (текст совпадает с оригиналом)

## Отличия от original.md той же папки
1. Drop-target ЕСТЬ (внешние файлы из Explorer + drag из дерева → открыть в редакторе) — оригинал явно «без drops». Расширение поведения, не потеря.
2. Вместо `<FileViewer />` (Monaco + свои табы) — собственный стек: editor_tabs_bar + gpui-component Input(code_editor) + breadcrumb + sticky-scroll + minimap; Save-кнопка в полосе табов (в оригинале грязность — dirty-точка на табе, сохранение Ctrl+S).
3. `.topCard { flex: 1 }` → у нас верх задан долей relative(1−bottom_ratio) (инверсия схемы высот, см. 61/64).
4. aria-label «File card» — нет DOM.
5. Сама карточка и modeHeader — 1:1 (glint 16, header 6/8/0 justify-end).

### Вердикты

# 63 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: glint r16, modeHeader 6/8/0, web-ветка, плейсхолдер File без пилюли.
Расхождения: верх flex1+низ px (оригинал) vs ratio-механика; лишние on_drop (не видимо).

## Цикл 5: MATCH

Верхняя карта файловой панели: glint r16, modeHeader `pt6 px8 pb0 justify-end`, hint — 1:1.

## Цикл 6: MATCH

Верхняя карта 1:1.

---

## 64. file-panel-split-handle — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](64-file-panel-split-handle/original.png)
![наш](64-file-panel-split-handle/ours.png)

### Оригинал

# 64 file-panel-split-handle — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 133-142), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.splitHandle [role="separator"] [aria-orientation="horizontal"]
  [aria-label="Resize bottom pane"] [data-tooltip="Drag to resize"]
  onMouseDown={onSplitDown}
└─ span.splitGrip [aria-hidden="true"]
```
Рендерится только при `filePanelBottomVisible`. Drag: cursor "row-resize"; `next = max(BOTTOM_PANE_MIN_HEIGHT_PX=100, startHeight - deltaY)` → filePanelBottomHeight (пиксели, не ratio) + `layoutActiveEditorNow()`.

## Метрики (ИЗ CSS, точные значения)
### .splitHandle
- flex-shrink: 0; height: 10px
- cursor: row-resize
- position: relative; display: flex; align-items: center; justify-content: center
- background: transparent (гейт-фон просвечивает)
- padding-right НЕТ (в отличие от RightPanel.splitHandle — тут нет activity bar сбоку)

### .splitGrip
- display: block; width: 32px; height: 3px
- background: var(--bg-overlay)
- border-radius: var(--radius-xs)
- opacity: 0.7
- transition: opacity 0.15s, background 0.15s

## Состояния (классы-варианты с метриками)
- `.splitHandle:hover .splitGrip`: opacity 1; background var(--accent-primary)
- active-класса нет (drag через JS)

### Наша реализация

# 64 file-panel-split-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:88-136 (h_handle); crates/shell/src/root.rs:4551-4569 (вызов), 2976-2982 (DragKind::FileBottom), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
h_handle("file-bottom-handle", pr=0)
= div .flex_shrink_0 .h(8) .min_w(0) .pr(0)
    .flex .items_center .justify_center .cursor_row_resize
    .tooltip("Drag to resize")
  └─ div .relative (probe_area + грип 32×3 rounded 4)
```
Сиблинг между верхней картой и слотом centralBottom в file_column.

## Метрики (из кода, точные)
- Hit: высота 8px; pr 0 (rail сбоку нет — грип по центру колонки, как в оригинале)
- Грип: 32×3, radius 4; idle bg_overlay opacity 0.7; hover/drag accent_primary opacity 1
- Drag: `ratio = init − dy/body_h` (вниз → низ меньше), кламп [BOTTOM_RATIO_MIN 0.1, BOTTOM_RATIO_MAX 0.8]; персист `filePanelBottomHeightRatio` на mouse-up

## Отличия от original.md той же папки
1. Высота hit-зоны 8px против 10px.
2. Модель ресайза: оригинал двигает ПИКСЕЛЬНУЮ высоту низа (`max(100, startHeight − deltaY)`), низ фиксирован в px; у нас ratio колонки с клампом [0.1, 0.8] — при resize окна низ масштабируется, у оригинала нет; жёсткого min 100px у низа нет (0.1 доли может быть <100px на низких окнах).
3. Нет transition 0.15s.
4. Рендерится всегда (filePanelBottomVisible-гейта нет).
5. `layoutActiveEditorNow()` не нужен.
6. role/aria «Resize bottom pane» — нет DOM; tooltip совпадает.

## Дополнение атрибутов (цикл 10)

- цвета: грип idle — bg_overlay #515567 dark / #d6d0c0 light при opacity 0.7 (`splitter.rs:113-114`, `palette.rs:58,96`); hover/drag — accent_primary #89b4fa / #da8343 при opacity 1 (`splitter.rs:106`, `palette.rs:83,121`); сама хит-зона (h 10px) без фона — просвечивает подложка bg_sidebar #1d1d28 / #f4f1ea (`palette.rs:56,94`). Совпадает с `.splitGrip` / `.splitHandle:hover .splitGrip` (`FilePanel.module.css:108-121`).

### Вердикты

# 64 — verdict (review cycle 1)
VERDICT: MATCH
h10 без pr, грип-рецепт, гейт — 1:1 (ratio-механика — см. 65).

## Цикл 5: MATCH

Сплит файловой панели: h10 без `pr`, грип 1:1.

## Цикл 6: MATCH

Сплит 1:1.

---

## 65. file-panel-bottom-card — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](65-file-panel-bottom-card/original.png)
![наш](65-file-panel-bottom-card/ours.png)

### Оригинал

# 65 file-panel-bottom-card — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanel.tsx (строки 143-155), kamin-ide/src/renderer/components/file-panel/FilePanel.module.css

## JSX-структура (кратко, вложенность)
```
div.card.bottomCardWithTabs [aria-label="Bottom card"]
  style={ height: `${filePanelBottomHeight}px`, flexShrink: 0 }  — ФИКС-высота в px
  [data-activity-slot="centralBottom"] [data-activity-drop=over|blocked|undefined]
  onDragOver/onDragLeave/onDrop
├─ <BottomTabBar slot="centralBottom" />
└─ <BottomCardBody/>: activeId ? <ActivityBody id slot="centralBottom" />
     : <PanelPlaceholder label="Central Bottom" slot="center-bottom" activitySlot="centralBottom" />
```
Рендерится только при `filePanelBottomVisible`. Высота — пиксели из filePanelBottomHeight (мин 100, BOTTOM_PANE_MIN_HEIGHT_PX).

## Метрики (ИЗ CSS, точные значения)
### .card
- `composes: glint-surface from global`: border 1px solid transparent; background `linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box, var(--glint-border) border-box`
- display: flex; flex-direction: column; min-height: 0; overflow: hidden; position: relative
- border-radius: var(--radius-lg)

### .bottomCardWithTabs
- display: flex; flex-direction: column (BottomTabBar фикс-высоты сверху, тело flex)

## Состояния (классы-варианты с метриками)
- data-activity-drop="over": background `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; outline `1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent)`; outline-offset -2px
- data-activity-drop="blocked": background `color-mix(in srgb, var(--accent-red) 12%, transparent)`; box-shadow `inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent)`

### Наша реализация

# 65 file-panel-bottom-card — наша реализация
Файлы: crates/shell/src/root.rs:4570-4600; crates/shell/src/ui/slot_panel.rs:27-115 (tab), 187-237 (slot_panel); crates/shell/src/ui/glint.rs:122-233

## Структура (gpui-дерево кратко)
```
div .flex_1 .min_h(0) .min_w(0)
└─ gap_wrap_v(pt=0, pb=4)
   └─ glint_surface_wv_holed(
        div#central-bottom .relative .size_full + probe_area("central-bottom")
        └─ slot_panel(CentralBottom, state, "Central Bottom",
             SlotIcon::CenterBottom, picker_up=true, drag_over, body))
```
slot_panel: стрип (pinned>0) + тело активного тула (напр. консоль/терминал) либо panel_placeholder «Central Bottom» с «Open Tool ▾» (пикер вверх).

## Метрики (из кода, точные)
- Высота: flex_1 = доля bottom_ratio колонки (верх занял 1−ratio); кламп ratio [0.1, 0.8]
- Карточка: glint radius 16 / inner 15, заливка bg_mantle
- Стрип: px 8 (SPACE_2), pt 4 (SPACE_1), gap 2; таб: h 24, px 12 (SPACE_3), gap 4 (SPACE_1), rounded 8 (RADIUS_SM), fs 12, иконка 13px (phosphor/codicon); idle text_muted, hover bg text_primary α.08 + text_primary; active bg accent_primary α.16 + text_primary; drag-over: border_l 2 accent_primary
- «…» dots 24×24 rounded 8, codicon ea7c 15px, справа (flex_1-спейсер перед ним)

## Отличия от original.md той же папки
1. Высота: оригинал — ФИКС-px (`height: filePanelBottomHeight px, flexShrink 0`, мин 100); у нас доля колонки (см. 64) — при resize окна поведение расходится.
2. Drop-индикация `data-activity-drop="over"/"blocked"` НЕ реализована; вместо неё — только индикатор вставки в стрипе.
3. BottomTabBar оригинала (элемент 48/49: TAB_ICON_SIZE 13, свои паддинги) заменён нашим стрипом slot_panel — иконка 13px совпадает, остальные метрики (h24/px12/rounded 8) требуют сверки с 48-bottom-tab-bar-strip отдельно.
4. aria-label «Bottom card», data-activity-slot — нет DOM.
5. Рендерится всегда (filePanelBottomVisible-гейта нет).

## Дополнение атрибутов (цикл 10)

- цвета: карта «Central Bottom» — `glint_surface_wv_holed` (`root.rs:5440-5459`): заливка bg_mantle #262533 dark / #fbf7f4 light (`palette.rs:55,93`), mid glint #262533 / #e6e1d4 (`palette.rs:87,125`), кромка glint_edge #ffffff α .18 / #3c2814 α .18 (`palette.rs:86,124`). Стрип-табы: текст text_secondary #adb3c7 / #524c43 (`slot_panel.rs:50`), hover bg_surface α .5 #3d3f51 / #e6e1d4 + text_primary #cfd4e2 / #322e28 (`slot_panel.rs:36,54`), active accent_primary α .16 (`slot_panel.rs:119`), drop-плейсхолдер — бордер accent α .7 + фон accent α .14 (`slot_panel.rs:134-135`), «…»-пикер text_muted #838aa0 / #6e685d (`slot_panel.rs:152`).

### Вердикты

# 65 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: glint, гейт, плейсхолдер+Open Tool. Расхождения: высота ratio vs фикс-px
(min 100, no-shrink); нет drop-индикации; стрип скрыт при пустом pinned.

## Цикл 5: DIVERGES

Высота нижней карты = доля колонки (кламп 0.1-0.8), у оригинала `FilePanel.tsx:143` фиксированные px + `flexShrink 0` + минимум 100. Нет drop-индикации; стрип скрыт при пустом `pinned`.

## Цикл 6: DIVERGES

Высота нижней карты — доля колонки вместо px + min 100.

---

## 66. file-panel-mode-tabs — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](66-file-panel-mode-tabs/original.png)
![наш](66-file-panel-mode-tabs/ours.png)

### Оригинал

# 66 file-panel-mode-tabs — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/FilePanelModeTabs.tsx (строки 10-29), kamin-ide/src/renderer/components/file-panel/FilePanelModeTabs.module.css

## JSX-структура (кратко, вложенность)
```
div.switcher [role="tablist"] [aria-label="File panel mode"]
├─ button.tab.left(.active при mode="files") [role="tab"] [aria-selected]
│  ├─ i.codicon.codicon-files [aria-hidden]
│  └─ span "Files"
└─ button.tab.right(.active при mode="web") [role="tab"] [aria-selected]
   ├─ i.codicon.codicon-globe [aria-hidden]
   └─ span "Web"
```
Клик переключает `filePanelMode` ("files" | "web", персистится).

## Метрики (ИЗ CSS, точные значения)
### .switcher
- display: inline-flex; flex-shrink: 0

### .tab
- display: inline-flex; align-items: center; gap: 5px
- height: 24px; padding: 0 10px
- border: 1px solid var(--divider-soft)
- background: var(--bg-surface)
- color: var(--text-secondary)
- font: inherit; font-size: var(--fs-sm)
- cursor: pointer

### .left (склейка в центре)
- border-radius: var(--radius-md) 0 0 var(--radius-md)
- border-right: none (шов без двойного бордера)

### .right
- border-radius: 0 var(--radius-md) var(--radius-md) 0

## Состояния (классы-варианты с метриками)
- `.tab:hover`: color var(--text-primary)
- `.active, .active:hover` (рецепт выбранной строки file/tree):
  - background: `linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent))`
  - border-color: `color-mix(in srgb, var(--accent-primary) 45%, transparent)`
  - color: var(--text-primary)
- transition не объявлен

### Наша реализация

# 66 file-panel-mode-tabs — наша реализация
Файлы: crates/shell/src/ui/file_panel_tabs.rs:25-103; вызов root.rs:4520-4531 (modeHeader)

## Структура (gpui-дерево кратко)
```
file_panel_mode_tabs(mode): div .flex .flex_shrink_0
├─ tab("fpm-files", codicon eaf0, "Files", left=true)
│    div .flex .items_center .gap(5) .h(24) .px(10)
│      .border_1 .rounded_l(12) .border_r_0
└─ tab("fpm-web", codicon eb01, "Web", left=false)
     … .rounded_r(12)
```
Клик → ShellEvent::SetFileMode("files"|"web") → layout.file_panel_mode (persist).

## Метрики (из кода, точные)
- Таб: h 24, px 10, gap 5, fs 12 (FS_SM), иконка codicon 14px
- border 1px --divider-soft = tint(text_primary, 0.06); bg bg_surface (#3d3f51 dark / #e6e1d4 light); текст text_secondary (#adb3c7 / #524c43)
- Склейка: левый rounded_l 12 (RADIUS_MD) + border_r_0; правый rounded_r 12 — шов без двойного бордера
- active: bg linear-gradient 90° tint(accent_primary,0.26) → tint(accent_primary,0.14); border tint(accent_primary,0.45); текст text_primary
- hover (неактивный): текст → text_primary (фон не меняется)

## Отличия от original.md той же папки
1. Метрики 1:1: h24/px10/gap5, divider-soft, bg-surface, radius-md по внешним краям, активный градиент 26→14% и бордер 45%, hover-цвет — всё совпадает.
2. `transition` в оригинале не объявлен — у нас его тоже нет. Совпадение.
3. role="tablist"/"tab", aria-selected — нет DOM.
4. Иконки: codicon files U+EAF0 / globe U+EB01 14px — оригинал те же классы codicon (размер иконки в css оригинала не переопределён, наследует fs — возможное расхождение 12 vs 14px, в original.md размер не зафиксирован).

## Дополнение атрибутов (цикл 10)

- шрифты: текст вкладки fs-sm 12 (`file_panel_tabs.rs:46`, `metrics/lib.rs:43`), начертание обычное (`font-weight` не задаётся — у `.tab` оригинала тоже `font: inherit`, `FilePanelModeTabs.module.css:15-16`); глиф codicon 16 (`file_panel_tabs.rs:72`) против «наследуемых» 12 у оригинала (`.tab` не переопределяет размер иконки) — расхождение кегля иконки.

### Вердикты

# 66 — verdict (review cycle 1)
VERDICT: DIVERGES
Всё 1:1 кроме: codicon 14 vs базовые 16.

## Цикл 5: MATCH

Табы режима (Files/Web): h24/px10/gap5, divider-soft, bg-surface, r12 по внешним краям, шов `border_r_0`, active градиент 26→14% + бордер 45%, codicon 16.

## Цикл 6: MATCH

Табы Files/Web 1:1.

---

## 67. browser-pane — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](67-browser-pane/original.png)
![наш](67-browser-pane/ours.png)

### Оригинал

# 67 browser-pane — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/BrowserPane.tsx (строки 77-104), kamin-ide/src/renderer/components/file-panel/BrowserPane.module.css

## JSX-структура (кратко, вложенность)
```
div.pane
├─ div.navbar
│  ├─ button.navBtn [data-tooltip="Back"]    → i.codicon.codicon-arrow-left
│  ├─ button.navBtn [data-tooltip="Forward"] → i.codicon.codicon-arrow-right
│  ├─ button.navBtn [data-tooltip="Reload"]  → i.codicon.codicon-refresh
│  └─ form.addrForm (onSubmit → browser.navigate(draft))
│     └─ input.addr [type=text] [spellcheck=false] [placeholder="Search or enter address"]
│        value = editing ? draft : url; onFocus → select() + editing
└─ div.viewport [data-browser-viewport]  ref=viewportRef
```
Поведение: нативный child-webview позиционируется по rect вьюпорта (`browser.setBounds` × devicePixelRatio, ResizeObserver + window resize). Скрывается (`browser.hide()`) когда перекрыт поповером: MutationObserver по body, rAF-coalesce; POPUP_SELECTOR = `[role='menu'], [role='dialog'], [role='listbox'], [data-tooltip-popup]`, проверка пересечения rect'ов.

## Метрики (ИЗ CSS, точные значения)
### .pane
- display: flex; flex-direction: column; flex: 1; min-height: 0

### .navbar
- display: flex; align-items: center; gap: 4px
- padding: 4px 6px; flex-shrink: 0

### .navBtn
- display: inline-flex; align-items: center; justify-content: center
- width: 26px; height: 26px
- border: none; border-radius: var(--radius-sm)
- background: transparent; color: var(--text-secondary); cursor: pointer

### .addrForm
- flex: 1; display: flex

### .addr
- flex: 1; height: 26px; padding: 0 10px
- border: 1px solid var(--divider-soft); border-radius: var(--radius-sm)
- background: var(--bg-base); color: var(--text-primary)
- font: inherit; font-size: var(--fs-sm)

### .viewport
- flex: 1; min-height: 0
- margin: 0 6px 6px (боковой/нижний инсет 6px — рамка под нативный webview внутри скруглённой карточки)
- border-radius: var(--radius-md)
- фон не задан (прозрачен: несинхронный кадр показывает поверхность панели, не дыру)

## Состояния (классы-варианты с метриками)
- `.navBtn:hover`: background var(--bg-surface-hover); color var(--text-primary)
- `.addr:focus`: outline none; border-color var(--accent-primary)
- transition не объявлены

### Наша реализация

# 67 browser-pane — наша реализация
Файлы: crates/shell/src/ui/browser_pane.rs:66-145 (wry-вариант), 151-218 (visual_frame, windows), 26-38 (normalize_url), 40-63 (nav_btn); root.rs:4118-4308 (web-ветка, форвардинг ввода)

## Структура (gpui-дерево кратко)
```
div#browser-pane .flex_1 .min_h(0) .flex_col
├─ навбар: div .flex .items_center .gap(4) .flex_shrink_0 .h(32) .px(8)
│    .border_b_1 .border_color(tint(text_primary, 0.06))
│  ├─ nav_btn "br-back"   codicon ea9b (Back)
│  ├─ nav_btn "br-fwd"    codicon ea9c (Forward)
│  ├─ nav_btn "br-reload" codicon eb37 (Reload)
│  └─ адрес: div .flex_1 .min_w(0) .ml(4) .px(8) .h(24) .rounded(8)
│       .bg(tint(bg_surface,0.5)) .border_1(tint(bg_overlay,0.4))
│       .on_key_down(Enter → normalize_url → load_url/navigate)
│     └─ Input(address).appearance(false)
└─ вьюпорт: div#browser-viewport .relative .flex_1 .min_h(0) .px(8) .pb(8)
   └─ webview (wry) | composition-визуал (дыра в кадре, canvas sync_zone)
```
normalize_url: схема как есть; «домен.tld» → https://; иначе Google-поиск. Visual-режим: мышь/скролл форвардятся SendMouseInput, курсор страницы мапится на gpui CursorStyle, back/forward/reload через wv_visual.

## Метрики (из кода, точные)
- Навбар: h 32, px 8, gap 4 (SPACE_1), border-bottom 1px tint(text_primary,0.06)
- nav_btn: 26×26, rounded 8 (RADIUS_SM), цвет text_secondary; hover bg tint(text_primary,0.1) + text_primary
- Адрес: h 24, px 8, rounded 8, bg bg_surface α.5, border 1px bg_overlay α.4
- Вьюпорт: px 8, pb 8 («воздух» вокруг вебвью); скругление зоны — угловые маски R=12 (RADIUS_MD) в glint-канвасе

## Отличия от original.md той же папки
1. Навбар: оригинал `padding: 4px 6px`, БЕЗ border-bottom и без фикс-высоты; у нас h 32, px 8 + разделительная линия снизу.
2. Адресная строка: оригинал h 26, px 10, bg --bg-base, border --divider-soft, focus → border accent-primary; у нас h 24, px 8, bg bg_surface/50%, border bg_overlay/40%, focus-подсветки НЕТ.
3. nav_btn hover: оригинал bg --bg-surface-hover (#3b3b52); у нас tint(text_primary, 0.1).
4. Вьюпорт-инсет: оригинал margin 0 6px 6px + border-radius 12; у нас px 8 / pb 8 (8 против 6), радиус 12 совпадает (маски).
5. Скрытие webview при перекрытии поповерами (MutationObserver + POPUP_SELECTOR) не нужно: наши поповеры живут в overlay-окне НАД вебвью (feedback_all_popovers_overlay).
6. form/onSubmit → on_key_down(Enter); placeholder «Search or enter address» задаётся InputState вне этого файла (не проверено здесь).
7. Forward: оригинал browser.navigate-API; у нас history.forward()/evaluate_script (wry) или wv_visual::forward().

## Дополнение атрибутов (цикл 10)

- шрифты: адресная строка fs-sm 12 (`browser_pane.rs:128`, дублируется в visual-варианте `:208`) = `.addr { font-size: var(--fs-sm) }` (`BrowserPane.module.css:41`); глифы nav-кнопок codicon 16 (`browser_pane.rs:63`) — у оригинала `.navBtn` кегля не задаёт, наследует 16px `.codicon` (`skeleton.css:2-4`), т.е. совпадает; собственных font-weight/семейства панель не ставит.

### Вердикты

# 67 — verdict (review cycle 1)
VERDICT: DIVERGES
Все фиксы подтверждены (навбар 4/6, addr h26 bg-base focus-accent, hover
surface-hover, инсет 6). Остаток: codicon navBtn 14 vs 16; viewport без r-md (невидимо).

## Цикл 5: MATCH

Браузер-пейн: навбар `py4 px6 gap4`, navBtn 26 + hover bg-surface-hover, адресная строка h26 px10 bg-base + focus accent, вьюпорт инсет 6. Мелочь: у div-вьюпорта нет `radius-md` (скругление даёт dcomp-клип).

## Цикл 6: MATCH

Браузер-пейн 1:1 (радиус вьюпорта даёт dcomp-клип).

---

## 68. panel-placeholder — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](68-panel-placeholder/original.png)
![наш](68-panel-placeholder/ours.png)

### Оригинал

# 68 panel-placeholder — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/PanelPlaceholder.tsx (строки 31-42), kamin-ide/src/renderer/components/panel-placeholder/PanelPlaceholder.module.css

## JSX-структура (кратко, вложенность)
```
div.placeholder
├─ span.glyph [aria-hidden="true"]
│  └─ <PanelIcon slot={slot} />        (SVG из титлбарного семейства LayoutToggles)
├─ h2.label  {label}
├─ p.hint    {hint ?? "Open new tool or drag-n-drop tool from other panels"}
└─ <ActivityPicker slot popDirection="up" variant="openTool" />   (только если activitySlot задан)
```
Пилюля «Open Tool» = ActivityPicker с variant openTool; открывает тот же пикер, что «...» activity bar'а — выбор пинит и активирует активность.

## Метрики (ИЗ CSS, точные значения)
### .placeholder
- flex: 1; display: flex; flex-direction: column
- align-items: center; justify-content: center; text-align: center (мёртвый центр карточки)
- gap: var(--space-2)
- padding: var(--space-5) var(--space-5)
- color: var(--text-muted)

### .glyph
- color: var(--text-muted); margin-bottom: var(--space-1); font-size: 0
- `.glyph svg { width: 28px; height: 24px; }` (PanelIcon штатно 14×12 — тут увеличен ×2)

### .label
- margin: 0; font-size: var(--fs-lg); font-weight: 600; color: var(--text-primary)

### .hint
- margin: 0; font-size: var(--fs-sm); color: var(--text-muted); line-height: var(--lh-snug)

### .trigger (пилюля «Open Tool»)
- display: inline-flex; align-items: center; gap: var(--space-2)
- padding: var(--space-1) var(--space-3)
- background: `color-mix(in srgb, var(--accent-primary) 16%, transparent)`
- color: var(--text-primary); border: none
- border-radius: var(--radius-sm)
- font-size: var(--fs-sm); margin-top: var(--space-1)
- transition: background var(--transition-fast)
- `.trigger > i { font-size: 10px; }`

## Состояния (классы-варианты с метриками)
- `.trigger:hover`: background `color-mix(in srgb, var(--accent-primary) 26%, transparent)`
- без activitySlot пикер (пилюля) не рендерится вовсе

### Наша реализация

# 68 panel-placeholder — наша реализация
Файлы: crates/shell/src/ui/panel_placeholder.rs:84-124 (panel_placeholder_ex / panel_placeholder), 34-80 (glyph — нативная PanelIcon); crates/shell/src/ui/slot_panel.rs:150-183 (open_tool_btn «Open Tool ▾»)

## Структура (gpui-дерево кратко)
```
panel_placeholder_ex(label, hint, slot, extra):
div .size_full .flex_col .items_center .justify_center
    .gap(8) .p(20) .overflow_hidden .text_color(text_muted)
├─ div .mb(4) → slot_glyph(slot)  (glyph scale 2.8: рамка 14×12 → 39.2×33.6)
├─ label: fs 16 (FS_LG) semibold text_primary
├─ hint:  fs 12 (FS_SM) text_muted, max_w 240, text_center
└─ when_some(extra): open_tool_btn — пилюля «Open Tool ▾»
```
Глиф — нативные div вместо SVG: рамка border 1px text_muted, rounded 1.5·s, внутри бар подсвеченного слота (text_muted α.85, rounded 1·s); 7 вариантов SlotIcon (Main/MainBottom/Center/CenterBottom/Right/RightTop/RightBottom), геометрия PanelIcon.tsx (SLOT_INSET 1.5, ширины 4.5, RIGHT_X 8 и т.д.).

## Метрики (из кода, точные)
- Контейнер: gap 8 (SPACE_2), padding 20 (SPACE_5), цвет text_muted (#838aa0 / #6e685d)
- Глиф: масштаб 2.8 → 39.2×33.6px; mb 4 (SPACE_1)
- label: 16px semibold text_primary (#cfd4e2 / #322e28)
- hint: 12px text_muted, max-width 240, по центру
- Пилюля (slot_panel::open_tool_btn): px 12 (SPACE_3), py 4 (SPACE_1), mt 4, rounded 8 (RADIUS_SM), gap 8, fs 12, текст text_primary; bg tint(accent_primary, 0.16), hover 0.26; «Open Tool» + fa chevron-down 10px

## Отличия от original.md той же папки
1. Размер глифа: у нас 39.2×33.6 (scale 2.8) против 28×24 оригинала (scale 2 от 14×12) — наш заметно крупнее.
2. hint: max-width 240 добавлен (у оригинального PanelPlaceholder.hint ограничения ширины нет — только line-height lh-snug; 240 — это метрика ActivityPlaceholder).
3. Пилюля: метрики 1:1 (py 4 ≈ padding space-1, px 12 = space-3, bg 16%/hover 26%, radius sm, mt space-1, иконка 10px); transition var(--transition-fast) не воспроизведён (мгновенный hover).
4. Глиф нативными div (currentColor→text_muted α.85 у слота) — оригинал SVG PanelIcon с opacity 0.85; визуально эквивалентно.
5. Пилюля рендерится только там, где caller передал extra (слоты со стрипом: Left/Left Bottom/Central Bottom); правые карты и центр «File» — без неё, что соответствует «без activitySlot пикер не рендерится».
6. h2/p/aria — нет DOM.

## Дополнение атрибутов (цикл 10)

- шрифты: заголовок fs-lg 16 + weight 600 SEMIBOLD (`panel_placeholder.rs:123-124`, `metrics/lib.rs:45`), подсказка fs-sm 12 с line-height 1.3 = 15.6px (`panel_placeholder.rs:131-133`), пилюля «Open Tool ▾» fs-sm 12 + шеврон FontAwesome 10 (`slot_panel.rs:192,209`). Оригинал: `.label { font-size: var(--fs-lg); font-weight: 600 }`, `.hint { var(--fs-sm); line-height: var(--lh-snug) 1.3 }`, `.trigger { var(--fs-sm) }`, `.trigger > i { 10px }` (`PanelPlaceholder.module.css:30-64`) — 1:1.

### Вердикты

# 68 — verdict (review cycle 1)
VERDICT: DIVERGES
MATCH: плейсхолдер-рецепт, габарит 28×24, слоты ×2, label/hint/trigger.
Расхождения: рамка глифа на весь бокс 28×24 border1 vs rect-инсет (24×20, stroke
2.4); hint lh 1.4 + max_w 360 — ИСПРАВЛЕНО после ревью (1.3, без max-w);
пилюля не передаётся правым картам.

## Цикл 5: MATCH

PanelIcon сверен поштучно (1.5/4.5/8/4.75/6/7/3.5/9), глиф 28×24, label 16/600, hint 12 lh1.3, пилюля px12 py4 mt4 r8 bg 16%/hover 26% + иконка 10. Отклонение: `bar()` клампит палочки во внутренность рамки → они на 2px короче SVG с каждого конца.

## Цикл 6: MATCH

PanelIcon 1:1; кламп палочек — отмеченное отклонение.

---

## 69. activity-placeholder — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

*кадр «оригинал» отсутствует*
*кадр «наш» отсутствует*

### Оригинал

# 69 activity-placeholder — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/ActivityPlaceholder.tsx (строки 21-27), kamin-ide/src/renderer/components/panel-placeholder/ActivityPlaceholder.module.css

## JSX-структура (кратко, вложенность)
```
div.placeholder
├─ <ToolIcon icon={icon} size={36} className=glyph />   (GLYPH_SIZE_PX = 36)
├─ h2.label  {label}
└─ p.hint    "Nothing to show here yet."
```
Отличие от PanelPlaceholder: пустое тело УЖЕ выбранной активности — пикера «Open Tool» намеренно нет.

## Метрики (ИЗ CSS, точные значения)
### .placeholder
- flex: 1; display: flex; flex-direction: column
- align-items: center; justify-content: center; text-align: center
- gap: var(--space-2)
- padding: var(--space-5)
- color: var(--text-muted)

### .glyph
- font-size: 36px
- color: var(--text-disabled)
- margin-bottom: var(--space-1)

### .label
- margin: 0; font-size: var(--fs-md); font-weight: 600; color: var(--text-primary)
  (у PanelPlaceholder — fs-lg; здесь на ступень меньше)

### .hint
- margin: 0; font-size: var(--fs-xs); color: var(--text-muted)
- line-height: var(--lh-snug); max-width: 240px

## Состояния (классы-варианты с метриками)
- вариантов/hover/transition нет — статичный empty-state

### Наша реализация

# 69 activity-placeholder — наша реализация
Файлы: `crates/shell/src/ui/panel_placeholder.rs` (`fn activity_placeholder`)

## Структура/содержание
Пустое состояние УЖЕ выбранной активности, у которой ещё нет тела:
```
div (size-full, flex-col, items-center, justify-center)
├─ обёртка глифа (mb 4) → Phosphor-svg 36 или codicon 36
├─ заголовок активности
└─ «Nothing to show here yet.»
```
Путь Phosphor-иконки берётся ИЗ мапы `activity_bar::phosphor_path` (алиасы вроде «problems» дают `icons/warning.svg`), неизвестное имя уходит в codicon-фолбэк.

## Метрики (из кода, точные)
- Контейнер: gap SPACE_2 8, padding SPACE_5 20, центровка по обеим осям, цвет text-muted #838aa0.
- Глиф: 36×36, цвет text-disabled #60667b, отбивка снизу SPACE_1 4.
- Заголовок: fs FS_MD 13, weight 600, цвет text-primary #cfd4e2.
- Подпись: fs FS_XS 11, line-height 1.3, max-width 240, цвет наследуется (text-muted).
- Скругления: N/A: скругления — плейсхолдер рисуется на поверхности карты, своего фона и рамки не имеет.
- Ховер: N/A: ховер — состояние неинтерактивное.

## Отличия от original.md той же папки
1. `ToolIcon` оригинала умеет ветку `<img>` для URL-иконок расширений — у нас только Phosphor-ассет или codicon.
2. Кадра пары в досье нет: элемент виден лишь у активности без нативного тела.

### Вердикты

# 69 — verdict (review cycle 1)
VERDICT: MATCH
36/text-disabled, fs-md 600, fs-xs max-w 240, без пикера — 1:1 (lh дефолт — допуск).

## Цикл 5: MATCH

Activity-плейсхолдер реализован: глиф 36 text-disabled, label fs-md/600 primary, hint fs-xs max-w 240, без пикера. Нет `lh-snug` у хинта. (`ours.md` устарел — там значилось «не реализовано».)

## Цикл 6: MATCH

Activity-плейсхолдер 1:1; у хинта нет `lh-snug` (наследует глобальные 1.169).

---

## 70. webview-loading-skeleton — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

*кадр «оригинал» отсутствует*
*кадр «наш» отсутствует*

### Оригинал

# 70 webview-loading-skeleton — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.tsx (строки 38-60), kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.module.css

## JSX-структура (кратко, вложенность)
```
div.wrap [role="status"] [aria-label="Loading panel…"]
├─ div.bar [aria-hidden]                (тулбар-скелет)
│  ├─ span.sk.pill
│  └─ span.sk.search
├─ div.rows [aria-hidden]               (6 строк, SKELETON_ROWS = 6)
│  └─ ×6 div.row
│     ├─ span.sk.icon
│     └─ div.lines
│        ├─ span.sk.line
│        └─ span.sk.lineDim
├─ div.waitNote  (только seconds >= 3, EXPLAIN_AFTER_S)
│  текст: `Waiting for the extension host to open this panel · {N}s` + ` · attempt {N}` при attempts > 1
└─ span.srOnly "Loading…"
```
Секундомер: setInterval 1000ms.

## Метрики (ИЗ CSS, точные значения)
### .wrap
- position: absolute; inset: 0; overflow: hidden
- display: flex; flex-direction: column; gap: 14px
- padding: 16px 18px
- background: `var(--bg-surface, var(--editor-bg, #22222e))`

### .bar
- display: flex; align-items: center; gap: 10px; flex-shrink: 0

### .rows
- display: flex; flex-direction: column; gap: 14px; min-height: 0

### .row
- display: flex; align-items: center; gap: 12px

### .lines
- display: flex; flex-direction: column; gap: 7px; flex: 1; min-width: 0

### .sk (шиммер-примитив)
- position: relative; overflow: hidden; border-radius: 6px
- background: `color-mix(in srgb, var(--text-primary, #cdd6f4) 8%, transparent)`
- `::after`: inset 0; transform translateX(-100%); background `linear-gradient(90deg, transparent, color-mix(in srgb, var(--text-primary, #cdd6f4) 9%, transparent), transparent)`
- animation: `kaminSkShimmer 1.25s ease-in-out infinite`; keyframes: `100% { transform: translateX(100%); }`

### Размеры скелет-блоков
- .pill: 84×22px, border-radius 8px
- .search: flex 1, height 22px, border-radius 8px
- .icon: 30×30px, border-radius 8px, flex-shrink 0
- .line: height 11px, width var(--sk-row)
- .lineDim: height 9px, opacity 0.6, width calc(var(--sk-row) * 0.62)
- Ширины строк (nth-child 6n+1..6n+6): 90% / 70% / 80% / 60% / 75% / 50%

### .waitNote
- margin-top: var(--space-3, 12px); text-align: center
- font-size: 11px; color: var(--text-disabled)
- font-variant-numeric: tabular-nums

### .srOnly
- position absolute; width/height 1px; overflow hidden; clip rect(0 0 0 0); white-space nowrap

## Состояния (классы-варианты с метриками)
- seconds < 3: без waitNote; >= 3: waitNote появляется
- attempts > 1: добавляется ` · attempt N`
- анимация: только shimmer (1.25s ease-in-out infinite); hover/focus нет

### Наша реализация

# 70 webview-loading-skeleton — наша реализация
Файлы: НЕ РЕАЛИЗОВАНО (шиммер-скелета нет). Вместо него: crates/shell/src/root.rs:2799-2815 (webview_body «Loading…»), 2891-2906 (webview_body_dyn), 5325-5332 (czShared «Loading…»)

## Структура (gpui-дерево кратко)
Шиммер-скелет (тулбар-пилюля 84×22 + поиск + 6 строк, анимация kaminSkShimmer 1.25s, «Waiting for the extension host… Ns · attempt N» после 3с) НЕ портирован. Пока HTML вью загружен, но скрипт ещё не подал признак жизни (`has_html && !alive`), рисуется простая заглушка:
```
div .size_full .flex_col .items_center .justify_center .gap(8)
├─ codicon U+EB19 (loading) 22px, цвет accent_primary
└─ "Loading…": fs 13 (FS_MD), text_secondary
```
czShared (contributed Customize-страница) — просто текст «Loading…» text_muted по центру.

## Метрики (из кода, точные)
- Иконка 22px accent_primary (#89b4fa / #da8343); текст 13px text_secondary; gap 8
- Анимации нет (иконка статична — codicon-modifier-spin не воспроизведён)
- Таймера/waitNote/attempt-счётчика нет; ретрай resolve — молча каждые 5с (root.rs:3024-3033, view_resolve_at)

## Отличия от original.md той же папки
1. Весь скелет (шиммер-примитивы .sk, размеры pill 84×22 / search / icon 30×30 / строки 90-50%, анимация 1.25s) — отсутствует.
2. waitNote «Waiting for the extension host to open this panel · Ns · attempt N» — отсутствует; пользователь не видит ни секунд, ни попыток.
3. Фон: оригинал bg-surface поверх якоря (absolute inset 0); у нас заглушка в потоке тела карты, фон карты (bg_mantle) просвечивает.
4. role="status"/srOnly — нет DOM.

## Дополнение атрибутов (цикл 10)

- отступы: у статической load-обложки паддингов нет — контент центрируется `items_center/justify_center` без padding (`root.rs:3244-3260`); у динамической contributed-ветки вокруг вебвью инсет px 8 / pb 8 (`root.rs:3311-3312`, тот же инсет в `visual_wv_body`, `root.rs:3103-3104`). Оригинал скелета: `.wrap { padding: 16px 18px }`, `.errWrap { padding: 24px }` (`WebviewLoadingSkeleton.module.css:7,105`) — шиммер-строк с их отступами у нас нет вообще.
- скругления: N/A: скругления — шиммер-примитивов (`.sk` radius 6, `.pill/.search/.icon` radius 8, `WebviewLoadingSkeleton.module.css:44,66-69`) нет; единственное скругление рядом — подложка динамического вебвью radius-md 12 (`root.rs:3319`, `metrics/lib.rs:38`), она не часть обложки.
- шрифты: единственный текст обложки — «Loading…» fs-md 13 (`root.rs:3256`, `metrics/lib.rs:44`) цветом text_secondary; спиннер codicon 22 (`root.rs:3253`). У оригинала на этом месте нет текста вовсе (только `.srOnly` + `.waitNote { font-size: 11px }`, `WebviewLoadingSkeleton.module.css:150-156`).
- ховер: N/A: ховер — обложка некликабельна, hover-правил нет (`root.rs:3244-3260`); у оригинала hover есть только у кнопки Retry `.retry:hover` (`WebviewLoadingSkeleton.module.css:144-146`), а она относится к элементу 71 и не портирована.

### Вердикты

# 70 — verdict (review cycle 1)
VERDICT: DIVERGES
Скелет не реализован: нет .wrap (inset0, p 16/18, bg-surface), pill 84×22 + search
h22, 6 строк icon 30×30 + линии 11/9 op.6 (90/70/80/60/75/50%), шиммер 1.25s,
waitNote c секундами и attempts, sr-only. Сейчас — центрированное «Loading…».

## Цикл 5: DIVERGES

Скелет загрузки вебвью не реализован (grep `shimmer|Waiting for the extension host` = 0): нет `.wrap` inset0/p16-18/bg-surface, pill 84×22, search h22, шести строк icon 30×30 + линии 11/9 op .6 (90/70/80/60/75/50%), анимации 1.25s, waitNote с секундами и попытками, sr-only. Вместо — центрированное «Loading…». Кадра нет.

## Цикл 6: DIVERGES

Скелет загрузки вебвью не реализован.

---

## 71. webview-load-error — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

*кадр «оригинал» отсутствует*
*кадр «наш» отсутствует*

### Оригинал

# 71 webview-load-error — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.tsx (строки 65-75), kamin-ide/src/renderer/components/panel-placeholder/WebviewLoadingSkeleton.module.css

## JSX-структура (кратко, вложенность)
```
div.errWrap [role="alert"]
├─ i.fas.fa-triangle-exclamation.errIcon [aria-hidden]
├─ div.errTitle  "This panel didn't load"
├─ div.errHint   "The extension host may still be starting up."
└─ button.retry  onClick={onRetry}
   ├─ i.fas.fa-rotate [aria-hidden]
   └─ " Retry"
```
Терминальное состояние после исчерпания retry-бюджета resolve.

## Метрики (ИЗ CSS, точные значения)
### .errWrap
- position: absolute; inset: 0
- display: flex; flex-direction: column; align-items: center; justify-content: center
- gap: 8px; padding: 24px; text-align: center
- background: `var(--bg-surface, var(--editor-bg, #22222e))`

### .errIcon
- font-size: 22px
- color: `var(--accent-yellow, #f9e2af)`; opacity: 0.85
- margin-bottom: 4px

### .errTitle
- font-size: var(--fs-md, 13px); font-weight: 600
- color: `var(--text-primary, #cdd6f4)`

### .errHint
- font-size: var(--fs-sm, 12px); color: `var(--text-muted, #9399b2)`
- max-width: 280px; line-height: 1.4

### .retry
- display: inline-flex; align-items: center; gap: 6px
- padding: 6px 16px
- border-radius: var(--radius-sm, 8px)
- border: `1px solid var(--divider-soft, color-mix(in srgb, var(--text-primary, #cdd6f4) 14%, transparent))`
- background: `color-mix(in srgb, var(--text-primary, #cdd6f4) 6%, transparent)`
- color: `var(--text-primary, #cdd6f4)`
- font-size: var(--fs-sm, 12px); cursor: pointer
- transition: background 0.15s ease

## Состояния (классы-варианты с метриками)
- `.retry:hover`: background `color-mix(in srgb, var(--text-primary, #cdd6f4) 12%, transparent)`
- других вариантов нет

### Наша реализация

# 71 webview-load-error — наша реализация
Файлы: НЕ РЕАЛИЗОВАНО. Retry-логика без UI: crates/shell/src/root.rs:3023-3033 (view_resolve_at, повтор resolve_webview каждые 5с)

## Структура (gpui-дерево кратко)
Терминального состояния «This panel didn't load» (fa-triangle-exclamation 22px accent_yellow + title + hint + кнопка Retry) нет. Retry-бюджет не исчерпывается: пока вью не alive, host_link::resolve_webview перезапрашивается бесконечно с интервалом ≥5с, на экране остаётся «Loading…» (см. 70) либо panel_placeholder (если HTML ещё не пришёл).

## Метрики (из кода, точные)
— (элемент отсутствует; ошибочное состояние визуально неотличимо от загрузки)

## Отличия от original.md той же папки
1. Нет всего элемента: errWrap (bg-surface, absolute inset 0), errIcon 22px accent-yellow op .85, errTitle fs-md 600, errHint fs-sm max-w 280, кнопка .retry (padding 6/16, border divider-soft, bg text-primary 6% / hover 12%).
2. Нет ручного Retry — только автоповтор; при мёртвом extension host пользователь навсегда видит «Loading…» без объяснения.
3. role="alert" — нет DOM.

## Дополнение атрибутов (цикл 10)

- гэпы: gap 8 (`SPACE_2`) между спиннером и «Loading…» в единственной заменяющей обложке (`root.rs:3251`, `metrics/lib.rs:51`); у оригинального `.errWrap` gap 8 (`WebviewLoadingSkeleton.module.css:104`) — совпало бы, но состав детей другой (нет иконки-предупреждения, заголовка, подсказки и Retry).
- цвета: спиннер accent_primary #89b4fa dark / #da8343 light (`root.rs:3253`, `palette.rs:83,121`), текст text_secondary #adb3c7 / #524c43 (`root.rs:3257`, `palette.rs:64,102`). Оригинал ошибки: фон `--bg-surface` #3d3f51 / #e6e1d4, иконка `--accent-yellow` #f9e2af / #c89a3f при opacity .85, заголовок `--text-primary` #cfd4e2 / #322e28, подсказка `--text-muted` #838aa0 / #6e685d, Retry — фон text-primary 6% + бордер `--divider-soft` (`WebviewLoadingSkeleton.module.css:107-138`). Жёлтого/красного состояния у нас НЕТ ни одного.
- скругления: N/A: скругления — кнопки Retry (`border-radius: var(--radius-sm) 8px`, `WebviewLoadingSkeleton.module.css:135`) нет; повтор идёт автоматически без UI (backoff 350ms×1.5 до 3s, 45 попыток, `root.rs:3640-3656`), у обложки скруглённых элементов нет.

### Вердикты

# 71 — verdict (review cycle 1)
VERDICT: DIVERGES
Не реализован: .errWrap + errIcon 22 accent-yellow + «This panel didn't load»
(13/600) + hint (12 muted max-w 280 lh1.4) + Retry (6/16 r8 border divider-soft
bg 6%/hover 12%). При исчерпании ретраев порт остаётся на Loading…

## Цикл 5: DIVERGES

Экран ошибки загрузки вебвью не реализован (grep `didn't load|Retry` = 0): нет errWrap, errIcon 22 accent-yellow op .85, title 13/600, hint 12 max-w 280 lh1.4, кнопки Retry (6/16, r8, divider-soft, bg 6%/hover 12%). Ретрай бесконечный раз в 5 с, терминального состояния нет. Кадра нет.

## Цикл 6: DIVERGES

Экран ошибки загрузки вебвью не реализован.

---

## 72. chat-switch-skeleton — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](72-chat-switch-skeleton/original.png)
![наш](72-chat-switch-skeleton/ours.png)

### Оригинал

# 72 chat-switch-skeleton — оригинал
Файлы: `src/renderer/components/panel-placeholder/ChatSwitchSkeleton.tsx` (10-21), `src/renderer/components/panel-placeholder/ChatSwitchSkeleton.module.css`

## JSX-структура (кратко, вложенность)
```
div.wrap [role=status, aria-label="Loading conversation…"]
├─ div.brand
│  ├─ span.glow (aria-hidden)
│  └─ img.logo (kaminoid.svg, draggable=false, aria-hidden)
├─ span.caption — «Loading conversation…»
└─ span.bar (aria-hidden)
   └─ span.barFill
```

## Метрики (ИЗ CSS, точные значения)
- `.wrap`: `position:absolute; inset:0`; flex column, `align-items:center; justify-content:center`; `gap:18px`; `padding:24px`; `overflow:hidden`; background `var(--editor-bg, var(--bg-base, #1e1e28))`
- `.brand`: `position:relative`; `display:grid; place-items:center`; 96×96px
- `.glow`: `position:absolute`; 150×150px; `border-radius:50%`; background `radial-gradient(circle, color-mix(in srgb, var(--accent-primary, #89b4fa) 28%, transparent) 0%, transparent 66%)`; `filter:blur(8px)`
- `.logo`: `position:relative; z-index:1`; 64×64px; `user-select:none; -webkit-user-drag:none`; `filter:drop-shadow(0 6px 18px rgba(0,0,0,0.35))`
- `.caption`: font-size `var(--fs-sm, 12px)`; `letter-spacing:0.01em`; color `var(--text-muted, #9399b2)`
- `.bar`: `position:relative`; 180×3px; `border-radius:999px`; `overflow:hidden`; background `color-mix(in srgb, var(--text-primary, #cdd6f4) 8%, transparent)`
- `.barFill`: `position:absolute; inset:0`; `border-radius:inherit`; background `linear-gradient(90deg, transparent, var(--accent-primary, #89b4fa), transparent)`; стартовый `transform:translateX(-100%)`
- Анимации:
  - `.glow` — `kaminSwitchBreathe 2.4s ease-in-out infinite`: 0%/100% `opacity:0.5; scale(0.94)` → 50% `opacity:1; scale(1.06)`
  - `.logo` — `kaminSwitchFloat 2.4s ease-in-out infinite`: 0%/100% `translateY(0)` → 50% `translateY(-4px)`
  - `.barFill` — `kaminSwitchSweep 1.15s ease-in-out infinite`: до `translateX(100%)`
- `@media (prefers-reduced-motion: reduce)`: все три `animation:none`; `.barFill` — `transform:none; opacity:0.6`

## Состояния (классы-варианты с метриками)
- Вариантных классов нет; монтируется только пока `covering` (см. 76-persistent-webview-layer), поверх чат-iframe при переключении сессии.

### Наша реализация

# 72 chat-switch-skeleton — наша реализация

НЕ РЕАЛИЗОВАНО (брендовой «шторки» с логотипом/glow/indeterminate-полосой нет).

Замена (эквивалент цепочки «не показать белую вспышку/старый чат»):
Файлы: `crates/shell/src/root.rs:225,420,624-628,814` (switching_to), `root.rs:2758-2824` (webview_body: load-cover), `root.rs:3702-3742` (chat-cover: wv2 скрыт до первого ipc), `crates/shell/src/ui/chat_webview.rs:119-125` (WebviewAlive по первому ipc)

## Структура (gpui-дерево кратко)
- `webviews_alive: HashSet<String>` — wv2-child показывается ТОЛЬКО после первого ipc-сообщения скрипта вью (`ShellEvent::WebviewAlive`); до этого wv2 `set_visible(false)` и панель рисует gpui-плейсхолдер.
- Load-cover (root.rs:2799-2815): `div` flex-col центр, gap `SPACE_2`(8) → `codicon \u{eb19}` 22px accent_primary (#89b4fa) → текст «Loading…» FS_MD(13) text_secondary (#adb3c7).
- Переключение сессии: `switching_to: Option<String>` → спиннер на чипе сессионного таба (session_tabs), гасится по подъёму сессии (root.rs:624-628). Поверх самого вебвью НИЧЕГО не рисуется — WebView2 перерисовывает контент in-place.

## Метрики (из кода, точные)
- Cover: gap 8, иконка 22px `#89b4fa`, текст 13px `#adb3c7`. Ни логотипа, ни полосы, ни анимаций.

## Отличия от original.md той же папки
1. Нет брендового скелета вообще: логотип 64px + glow 150px + breathe/float/sweep-анимации, caption «Loading conversation…», полоса 180×3 — всё отсутствует.
2. Нет непрозрачного фона `--editor-bg` поверх iframe при переключении сессии; вместо шторки — спиннер на чипе таба + нативная перерисовка вебвью.
3. Нет transition opacity 140ms и состояния `covering`.
4. Cover у нас применяется к ЛЮБОМУ вебвью до первого ipc (не только чату), т.е. это скорее аналог 70-webview-loading-skeleton, чем 72.

## Дополнение атрибутов (цикл 10)

- отступы: паддингов нет — load-обложка чата это тот же центрированный блок без padding (`root.rs:3244-3260`), показ гейтится `webviews_alive`/`switching_to` (`root.rs:242,4436-4439`). Оригинал: `.wrap { padding: 24px }` (`ChatSwitchSkeleton.module.css:13`).
- скругления: N/A: скругления — брендовой обложки с glow-кругом (`border-radius: 50%`) и полосой-свипом (`border-radius: 999px`, `ChatSwitchSkeleton.module.css:31,62`) нет; у нашей обложки скруглённых элементов нет вовсе.
- шрифты: «Loading…» fs-md 13 (`root.rs:3256`) против `.caption { font-size: var(--fs-sm) 12px }` оригинала (`ChatSwitchSkeleton.module.css:53`) — кегль на шаг крупнее; лого/анимаций (96/64px, breathe/float/sweep) нет.

### Вердикты

# 72 — verdict (review cycle 1)
VERDICT: DIVERGES
Не реализовано: брендовый скелет (лого 64 + glow 150 28% + «Loading conversation…»
+ полоса 180×3 + breathe/float/sweep, шторка 140ms). Замена — generic load-cover.

## Цикл 5: DIVERGES

Скелет переключения чата не реализован (grep `Loading conversation|kaminSwitch` = 0): лого 64 + glow 150 (accent 28%, blur 8) + caption + полоса 180×3 + breathe/float/sweep + шторка 140 мс. Кадра нет (транзиент).

## Цикл 6: DIVERGES

Шторка переключения чата не реализована.

---

## 73. contributed-container-body — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](73-contributed-container-body/original.png)
![наш](73-contributed-container-body/ours.png)

### Оригинал

# 73 contributed-container-body — оригинал
Файлы: `src/renderer/components/activity-bodies/ContributedContainerBody.tsx` (30-38 — `ContributedContainerBody`, 43-48 — `ContributedViewBody`), `src/renderer/components/activity-bodies/ContributedContainerBody.module.css`

## JSX-структура (кратко, вложенность)
```
ContributedContainerBody({containerId})
├─ views = registry.views.filter(containerId)
├─ если views.length === 0 → <ActivityPlaceholder icon="circle-large" label="No views" />
└─ div.root
   └─ views.map → <ViewSection viewId name type />   (см. 74)

ContributedViewBody({viewId, flush})  — одиночный view без хедера (Customize)
├─ chat без сессии (viewId===CHAT_VIEW_ID && openSessions.length===0) → <WelcomePlaceholder />
├─ view не найден → <ActivityPlaceholder icon="circle-large" label="No view" />
└─ type==="webview" ? <WebviewViewBody viewId flush /> : <TreeViewBody viewId />
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: `display:flex; flex-direction:column; height:100%; min-height:0`
- Отступов/padding/margin/border-radius у `.root` нет; шрифт/цвет не задаются (наследуются)
- hover/active/focus — нет; transition/анимаций — нет; позиционирование — обычный поток

## Состояния (классы-варианты с метриками)
- Пустой контейнер (0 views) → рендерится `ActivityPlaceholder` (элемент 69), не `.root`
- Chat view без открытых сессий → полный `WelcomePlaceholder` (элемент 77) вместо тела

## Дополнение атрибутов (цикл 10)

- цвета: собственных фонов у `.root`/`.view`/`.frame` нет (`ContributedContainerBody.module.css:1-14,52-61`) — просвечивает карта `--bg-mantle` #262533 dark / #fbf7f4 light (`dark-theme.css:12`, `light-theme.css:25`); заголовок вью `.title { color: var(--text-muted) }` #838aa0 / #6e685d (`:23`; `dark-theme.css:37`, `light-theme.css:47`); `.viewBadge { background: var(--accent-primary); color: var(--bg-base) }` = #89b4fa на #313240 dark / #da8343 на #fbf8f1 light (`:41-42`; `light-theme.css:24,109`).

### Наша реализация

# 73 contributed-container-body — наша реализация

НЕ РЕАЛИЗОВАНО как контейнер-стек нескольких views.

Замена: каждый contributed view = отдельный «dyn tool» активити-реестра; тело одного тула рендерит `tool_body` (dyn-ветка).
Файлы: `crates/shell/src/root.rs:3012-3058` (tool_body → dyn_tool branch), `root.rs:2844-2914` (webview_body_dyn), `root.rs:3982-4012,5366-5368` (welcome вместо панелей без сессии)

## Структура (gpui-дерево кратко)
```
div (flex-col, size_full, min_h 0)
├─ div — секц-титул: d.view_name.to_uppercase()   (см. 74)
└─ div (flex_1, min_h 0) → webview_body_dyn(view_id, wv, alive, placeholder, p)
```
- Нет группировки по containerId: контейнер расширения разворачивается в плоский список тулов, каждый живёт в своём слоте самостоятельно.
- resolve-ретрай: `view_resolve_at` — повторный `resolve_webview` не чаще раза в 5с, пока вью не alive (root.rs:3024-3033).
- Welcome-замещение: при отсутствии активной сессии welcome заменяет ВСЮ панельную область (все колонки), а не только chat-view (root.rs:5366).

## Метрики (из кода, точные)
- Обёртка: `flex-col`, `size_full`, `min_h 0` — совпадает с `.root` оригинала (без отступов/фона).
- Титул: px 12, pt 4 (SPACE_1), pb 2, FS_XS(11), weight Medium, text_muted `#838aa0`, uppercase.

## Отличия от original.md той же папки
1. Нет multi-view стека: оригинал рендерит `views.map → ViewSection` внутри одного `.root`; у нас один view = один тул = одна панель.
2. Нет `ActivityPlaceholder "No views"` для пустого контейнера — пустой контейнер просто не порождает тулов.
3. Welcome-фоллбек шире оригинала: заменяет всю область панелей, а не тело chat-view.
4. `ContributedViewBody` (flush-вариант для Customize) у нас — отдельная ветка root.rs:5304-5333 (czShared-вебвью, один переиспользуемый wv2 на все contributed Customize-страницы; в оригинале — свой iframe на view).

## Дополнение атрибутов (цикл 10)

- скругления: подложка динамического вебвью radius-md 12 (`root.rs:3319`, `metrics/lib.rs:38`) против `.frame { border-radius: var(--radius-lg) 16px }` оригинала (`ContributedContainerBody.module.css:57`) — на шаг мельче; бейдж хедера radius 9 (`root.rs:3547`) = `.viewBadge { border-radius: 9px }` (`:39`).
- ховер: N/A: ховер — ни контейнер (`root.rs:3661-3674`), ни хедер вью (`root.rs:3523-3557`) hover-стилей не имеют, как и `.root`/`.view`/`.title`/`.frame` оригинала; у бейджа только tooltip (`root.rs:3555`).

### Вердикты

# 73 — verdict (review cycle 1)
VERDICT: DIVERGES
Архитектурная замена (dyn-tool вместо стека ViewSection); нет ActivityPlaceholder
«No views»; welcome-фоллбек шире оригинала (ИСПРАВЛЕНО в wave1: welcome теперь
только в main-карте — перепроверить в цикле 2).

## Цикл 5: DIVERGES

Архитектурно: один contributed view = один dyn-тул; стека `views.map → ViewSection` внутри `.root` нет, нет `ActivityPlaceholder "No views"`.

## Цикл 6: DIVERGES

Contributed-контейнер: стека вью нет (архитектурно).

---

## 74. contributed-view-section — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](74-contributed-view-section/original.png)
![наш](74-contributed-view-section/ours.png)

### Оригинал

# 74 contributed-view-section — оригинал
Файлы: `src/renderer/components/activity-bodies/ContributedContainerBody.tsx` (62-77 — `ViewSection`), `src/renderer/components/activity-bodies/ContributedContainerBody.module.css`

## JSX-структура (кратко, вложенность)
```
section.view
├─ header.title
│  ├─ span — meta?.title ?? name (createTreeView title переопределяет contributed name)
│  ├─ span.viewDescription — meta.description (опционально)
│  └─ span.viewBadge [data-tooltip=badge.tooltip] — badge.value (опционально)
└─ type==="webview" ? <WebviewViewBody viewId /> : <TreeViewBody viewId />
```
Chat view без сессии → вместо всей секции `<WelcomePlaceholder />` (без хедера).

## Метрики (ИЗ CSS, точные значения)
- `.view`: `display:flex; flex-direction:column; flex:1; min-height:0` (виды стекаются; одиночный webview заполняет тело)
- `.title` (хедер): flex, `align-items:center`; padding `var(--space-1) var(--space-3)`; font-size `var(--fs-xs)`; `text-transform:uppercase`; `letter-spacing:0.04em`; color `var(--text-muted)`; `flex-shrink:0`
- `.viewDescription`: `margin-left:var(--space-2)`; `font-weight:400`; `opacity:0.55`
- `.viewBadge`: `margin-left:auto`; `min-width:18px`; padding `0 5px`; `border-radius:9px` (половина min-height — пилюля); background `var(--accent-primary)`; color `var(--bg-base, #fff)`; `font-size:0.75em`; `line-height:16px`; `text-align:center`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- description и badge — условные (только при `treeMeta` от `createTreeView`)
- badge несёт tooltip через `data-tooltip`

### Наша реализация

# 74 contributed-view-section — наша реализация
Файлы: crates/shell/src/root.rs (`contrib_view_header`, ветка `dyn_tool` в `tool_body`), ui/contributed_tree.rs (мета вью)

## Структура (gpui-дерево кратко)
```
div .flex_col .size_full .min_h 0                     ← .view
├─ contrib_view_header                                ← .title
│   ├─ титул: meta.title ?? contributed name, uppercase
│   ├─ (meta.description) .viewDescription
│   └─ (meta.badge) .viewBadge (ml auto)
└─ тело: вебвью (type=webview) либо tree_view_body (TreeDataProvider)
```
Мета берётся из `kamin:tree:getMeta` и broadcast'а `kamin:tree:meta` (createTreeView).

## Метрики (из кода, точные)
- `.title`: flex, items-center, padding SPACE_1 4 / SPACE_3 12 (симметрично, как в оригинале), fs FS_XS 11, text-muted, flex-shrink 0.
- `.viewDescription`: margin-left SPACE_2 8, font-weight 400, opacity 0.55.
- `.viewBadge`: margin-left auto, min-w 18, px 5, radius 9, bg `--accent-primary`, цвет `--bg-base`, fs 0.75em (11×0.75), line-height 16, по центру; tooltip = `badge.tooltip`.

## Отличия от original.md той же папки
1. `letter-spacing .04em` в gpui недоступен; uppercase делается в Rust (`to_uppercase`).
2. Несколько `.view` в одном контейнере (стек с flex:1) не поддержано — панель показывает ПЕРВОЕ вью контейнера (см. 73).

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — секция (`contrib_view_header`, `root.rs:3514-3559` + тело) не интерактивна и hover-правил не задаёт; у `ViewSection` оригинала (`ContributedContainerBody.tsx:62-77`, стили `.view`/`.title`) их тоже нет — сворачивания по клику в этой версии нет.

### Вердикты

# 74 — verdict (review cycle 1)
VERDICT: DIVERGES
Хедер = только uppercase-имя. Нет .viewDescription (ml8, op .55) и .viewBadge
(min-w18 px5 r9 accent bg-base fs.75em); паддинг 12/4/2 vs 4/12; нет ls .04em;
титул не предпочитает meta.title.

## Цикл 5: DIVERGES

Хедер секции contributed-вью `px12 pt4 pb2`, у оригинала `padding: space-1 space-3` = 4/12 симметрично. Нет `.viewDescription` (ml 8, weight 400, opacity .55) и `.viewBadge` (min-w18, px5, r9, bg accent-primary, color bg-base, fs .75em, lh16, tooltip). Титул берётся из `name`, а не `meta.title`.

## Цикл 6: DIVERGES

Хедер вью: паддинги 4/12, `.viewDescription`, `.viewBadge`, титул из `meta.title`.

## Цикл 7: DIVERGES

Хедер переписан: padding 4/12, fs-xs, text-muted, flex-shrink 0, титул
`meta.title ?? name`, `.viewDescription` (ml 8, weight 400, op .55), `.viewBadge`
(ml auto, min-w 18, px 5, r 9, accent-primary, bg-base, .75em, lh 16, tooltip).
Исправлено по ревью: `text-transform: uppercase` наследуется — description и badge
тоже в верхнем регистре.

Осталось: стек нескольких `.view` в одном контейнере (панель показывает первое вью);
`letter-spacing .04em` (нет в gpui).

---

## 75. webview-view-anchor — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](75-webview-view-anchor/original.png)
![наш](75-webview-view-anchor/ours.png)

### Оригинал

# 75 webview-view-anchor — оригинал
Файлы: `src/renderer/components/activity-bodies/ContributedContainerBody.tsx` (79-143 — `WebviewViewBody`, JSX 136-142), `src/renderer/components/activity-bodies/ContributedContainerBody.module.css`

## JSX-структура (кратко, вложенность)
```
div [data-webview-anchor=viewId] .frame | .frameFlush (flush ? frameFlush : frame)
└─ пока !hasHtml:
   ├─ exhausted → <WebviewLoadError onRetry />          (элемент 71)
   └─ иначе   → <WebviewLoadingSkeleton attempts />     (элемент 70)
```
Iframe здесь НЕ рендерится — только якорь; `PersistentWebviewLayer` копирует rect + border-radius якоря и позиционирует над ним живой iframe.

Логика resolve-retry: `RESOLVE_MAX_ATTEMPTS=45`, base 350ms, backoff ×1.5, max 3000ms (~2 мин); рестарт по `kamin:exthost:respawned`; `retryNonce` — ручной Retry.

## Метрики (ИЗ CSS, точные значения)
- `.frame`: `flex:1; min-height:0`; margin `0 var(--space-2) var(--space-2)` (top 0, стороны/низ space-2); `overflow:hidden`; `border-radius:var(--radius-lg)`; `position:relative` (якорь для absolute-скелета). Карточка со скруглениями БЕЗ glint-бордера
- `.frameFlush`: `flex:1; min-height:0; overflow:hidden; position:relative` — без inset и radius (view уже внутри host-карточки, Customize)
- Цвета не задаются — вебвью красит свою поверхность сам, radius клипует
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- `.frame` — карточный вариант (по умолчанию, `flush=false`)
- `.frameFlush` — flush-вариант (Customize-страницы, `flush=true`)
- Пока html не пришёл: внутри скелет (attempts показывается) или Retry-карточка после исчерпания бюджета

## Дополнение атрибутов (цикл 10)

- цвета: якорь прозрачен — `.frame`/`.frameFlush` фона не задают (`ContributedContainerBody.module.css:52-71`), под ним видна карта `--bg-mantle` #262533 / #fbf7f4, а сам вебвью красит свою поверхность `--editor-bg` #1d1c25 dark (`dark-theme.css:21`) / #fcfaf6 light (`light-theme.css:32`); комментарий CSS прямо фиксирует «webview paints its own surface».

### Наша реализация

# 75 webview-view-anchor — наша реализация
Файлы: `crates/shell/src/root.rs:2774-2824` (webview_body, статические вью), `root.rs:2844-2914` (webview_body_dyn), `root.rs:2635-2756` (visual_wv_body: canvas-prepaint = синк зоны), `crates/shell/src/probe_registry.rs` (probe_area — реестр bounds по id)

## Структура (gpui-дерево кратко)
Роль «якоря» играют два механизма:
1. wry-режим: `div#id.relative.size_full` → `probe_area(id)` (записывает bounds кадра в реестр) → `wv.clone()` (wry-чайлд сам позиционируется по bounds элемента).
2. visual hosting (`KAMIN_VISUAL_WV=1`): `visual_wv_body(id)` — `div` px 8 / pb 8 → probe_area + `gpui::canvas` prepaint, который тем же кадром зовёт `wv_visual::sync_zone_view` (позиция+размер dcomp-визуала) и `set_zone_view` («дыра» в фоне карты).

Динамический вариант (webview_body_dyn, contributed-тулы): вокруг вебвью px 8 / pb 8 (top 0), под ним подложка `rounded(RADIUS_MD)` bg editor_bg `#1d1c25` (закрывает разрыв ресайза).

Состояния до готовности:
- нет HTML (`!has_html`) → `panel_placeholder(label, "Open new tool or drag-n-drop tool from other panels", slot)` (см. 68);
- HTML есть, скрипт не жив (`!alive`) → load-cover: codicon `\u{eb19}` 22px accent_primary + «Loading…» FS_MD text_secondary.

## Метрики (из кода, точные)
- Инсет вокруг вебвью: left/right 8, bottom 8, top 0 (= margin `0 var(--space-2) var(--space-2)` оригинала).
- Радиус подложки дин-вью: RADIUS_MD = 12; статические вью в visual-режиме — клип углов делает dcomp-clip / `overlay::round_webview_children` (root.rs:3415-3440), радиус по зонам.
- Подложка: p.editor_bg `#1d1c25` (диаг-режим KAMIN_VWV_PAINTDBG=1 — оранжевый).
- Ретрай resolve: не чаще 1 раза в 5с (root.rs:3024-3033), без лимита попыток.

## Отличия от original.md той же папки
1. Радиус: оригинал `.frame` — `radius-lg` 16; у нас дин-подложка 12 (RADIUS_MD); статические вью клипуются по зоне отдельным механизмом.
2. Вместо WebviewLoadingSkeleton (шиммер, attempts) и WebviewLoadError (Retry) — единый load-cover «Loading…» без счётчика попыток и без ручного Retry.
3. Ретрай-политика: оригинал 45 попыток, backoff 350ms×1.5 до 3000, exhausted-состояние; у нас фикс-интервал 5с без исчерпания.
4. `.frameFlush` (Customize) — эквивалент есть: czShared-вебвью рендерится без инсетов внутри glint-карты (root.rs:5304-5333).
5. Якорь-механика иная по сути: не DOM rect + слой поверх, а probe-реестр bounds + канвас-prepaint (visual) либо позиционирование wry-чайлда.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — зона-якорь (`visual_wv_body`, `root.rs:3099-3140`) hover-подсветки не имеет: мышь форвардится в composition-вебвью (`send_mouse_view`), меняется только курсор (`wv_cursor`, `root.rs:3105`); у `.frame` оригинала hover-правил тоже нет.

### Вердикты

# 75 — verdict (review cycle 1)
VERDICT: DIVERGES
Инсеты 0/8/8/8 ок; радиус карточки 12 vs 16 (.frame radius-lg). Вместо
WebviewLoadingSkeleton/WebviewLoadError(Retry) — единый load-cover; ретрай фикс-5с
vs 45×350ms×1.5→3000 + exhausted.

## Цикл 5: DIVERGES

Подложка дин-вью `rounded(12)`, у оригинала `.frame { border-radius: var(--radius-lg) }` = 16. Инсеты 0/8/8/8 верны. Ретрай фиксированный 5 с без `RESOLVE_MAX_ATTEMPTS=45` и backoff 350×1.5→3000; вместо скелета и Retry — общий load-cover.

## Цикл 6: DIVERGES

Радиус подложки 12 вместо 16; ретрай без backoff/лимита.

---

## 76. persistent-webview-layer — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](76-persistent-webview-layer/original.png)
![наш](76-persistent-webview-layer/ours.png)

### Оригинал

# 76 persistent-webview-layer — оригинал
Файлы: `src/renderer/components/activity-bodies/PersistentWebviewLayer.tsx` (45-54 — слой, 56-218 — `PersistentItem`; inline-стили, css-модуля нет)

## JSX-структура (кратко, вложенность)
```
div [aria-hidden] style={position:fixed; left:0; top:0; width:0; height:0; zIndex:5}
└─ для каждого id из webviewViewHtml:
   div (ref) style={position:fixed; display:none; overflow:hidden; pointerEvents:auto}  ← rect синхронизируется JS
   ├─ <WebviewPanelView id html localResourceRoots visible={shown} />
   └─ только для CHAT_VIEW_ID ("claudeBridgeChat"):
      div [aria-hidden] style={position:absolute; inset:0; opacity:covering?1:0;
           pointerEvents:covering?"auto":"none"; transition:"opacity 140ms ease-out"; zIndex:2}
      └─ covering && <ChatSwitchSkeleton />   (элемент 72)
```

## Метрики (inline, точные значения)
- `OVERLAY_Z = 5` — ниже модалок/тултипов/дропдаунов, выше панельной поверхности
- Контейнер item: `position:fixed`, `display:none` → при видимом якоре `display:block` + `left/top/width/height` = `getBoundingClientRect()` якоря `[data-webview-anchor=id]`; `borderRadius` копируется из `getComputedStyle(anchor).borderRadius` один раз при показе (кэш)
- Видимость: якорь есть и `r.width>1 && r.height>1`; иначе `display:none` (iframe остаётся смонтирован)
- Шторка чата: `transition: opacity 140ms ease-out`; `zIndex:2`; pointerEvents следуют opacity
- Синк-механика: rAF-schedule; per-frame loop при `body.kamin-dragging` (сплиттер) и window-resize (стоп через `RESIZE_SETTLE_MS=200`); burst `BURST_FRAMES=12` кадров на смену сессии и на каждый layout-сигнал (panelStates.*, sidebarVisible, sidebarMode, filePanelVisible, filePanelBottomVisible, filePanelMode, activeCustomizePanel); ResizeObserver на body; scroll capture; ленивый интервал `SYNC_INTERVAL_MS=500`

## Состояния
- hidden (`display:none`, iframe жив) / shown (rect якоря)
- covering (только chat view, `shown && chatSwitchCovered`): непрозрачная шторка со скелетом; скелет монтируется только пока covering (анимация не крутится вечно)

## Дополнение атрибутов (цикл 10)

- цвета: слой полностью прозрачен — инлайн-стили содержат только геометрию и `zIndex: OVERLAY_Z = 5` (`PersistentWebviewLayer.tsx:33,50`), у элемента-контейнера `position/display/overflow/pointerEvents` (`:199`); цвет даёт сам вебвью (`--editor-bg` #1d1c25 dark / #fcfaf6 light, `dark-theme.css:21`, `light-theme.css:32`). Единственная «краска» слоя — накрывашка чата: полупрозрачный слой `opacity 0→1` c `transition 140ms ease-out` над `ChatSwitchSkeleton`, чей фон = `var(--editor-bg, var(--bg-base, #1e1e28))` (`PersistentWebviewLayer.tsx:204-207`, `ChatSwitchSkeleton.module.css:14`).
- отступы: собственных padding/margin нет ни у слоя, ни у item — геометрия копируется 1:1 из rect якоря (`left/top/width/height` = `getBoundingClientRect()`, `PersistentWebviewLayer.tsx:80-83`), корень слоя `left:0, top:0, width:0, height:0` (`:50`), накрывашка `inset: 0` (`:205`); скругление тоже копируется (`getComputedStyle(anchor).borderRadius`, `:88-89`) — инсет вокруг вебвью целиком принадлежит якорю `.frame { margin: 0 var(--space-2) var(--space-2) }` = 0/8/8 (`ContributedContainerBody.module.css:55`).

### Наша реализация

# 76 persistent-webview-layer — наша реализация

НЕ РЕАЛИЗОВАНО как DOM-слой `position:fixed` с iframe'ами. Замена: composition visual hosting + персистентные нативные WebView2-чайлды.

Файлы: `crates/shell/src/wv_visual.rs` (весь файл — CoreWebView2CompositionController в dcomp-underlay визуал), `crates/shell/src/root.rs:3470-3560` (все вебвью создаются на ПЕРВОМ кадре и живут весь ран; дисковый кэш HTML), `root.rs:3386-3404` (clear_zones/hide на кадр), `root.rs:3702-3742` (show/hide wv2-чайлдов по видимости тула), `root.rs:3415-3440` (`overlay::round_webview_children` — скругление углов чайлдов), `crates/shell/src/ui/glint.rs:61-115` (hole_segments — «дыры» в фоне карт под зоны)

## Структура (gpui-дерево кратко)
- Персистентность: `RootView.webviews: HashMap<viewId, Entity<WebView>>` — wry/wv2-чайлды создаются один раз (первый кадр; create_controller пампит event loop → вне первого кадра RefCell-паника) и не уничтожаются; HTML едет `load_url` в любом кадре.
- Позиционирование: не rAF-loop по DOM-якорю, а prepaint-канвас каждого кадра (`sync_zone_view`) — позиция/размер dcomp-визуала обновляются тем же кадром, что и layout (синхронно со сплиттером, без дребезга).
- Видимость: `wv.show()/hide()` по правилу «тул активен в каком-либо слоте ∧ панели видимы ∧ не Customize ∧ alive» (root.rs:3716-3742); скрытый чайлд остаётся смонтирован (буфер/стейт живут) — аналог `display:none`.
- «Шторка» чата: отсутствует (см. 72); z-порядок решается нативным HWND/dcomp, не zIndex 5.
- czShared: ОДИН переиспользуемый вебвью на все contributed Customize-страницы (root.rs:3475-3476).
- Backdrop: одноцветный dcomp-визуал editor_bg ПОД вебвью прикрывает щели, пока Chromium догоняет relayout (wv_visual.rs Host.backdrop).

## Метрики (из кода, точные)
- Скругление зон: dcomp `IDCompositionRectangleClip` (антиалиас) в visual-режиме; в wry-режиме — оконный регион `round_webview_children(zones, scale)`.
- Гистерезис ресайза поверхности: SetBounds только когда размер замер ≥120мс; доводчик 160мс (wv_visual.rs Host.want_since/settle_pending).
- Загрузочный HTML-кэш: `cache/webview-html/{id}.html` — UI рисуется сразу, extension активируется ~8с фоном.

## Отличия от original.md той же папки
1. Нет DOM-слоя (OVERLAY_Z=5), нет копирования rect/borderRadius якоря через getBoundingClientRect/getComputedStyle — синк идёт из layout-движка gpui тем же кадром (жёстче, чем rAF-burst 12 кадров + интервал 500мс оригинала).
2. Нет chatSwitchCovered/шторки с transition 140ms (см. 72).
3. Нет per-frame loop на `body.kamin-dragging`/resize-settle 200мс — не нужен: prepaint синхронен.
4. Contributed Customize-страницы делят один вебвью (czShared) вместо N персистентных iframe.
5. Дополнительно к оригиналу: «дыры» в фонах карт (hole_segments) и backdrop-визуал — артефакты composition-хостинга, у DOM-оригинала не требовались.

## Дополнение атрибутов (цикл 10)

- цвета: слой как таковой прозрачен — composition-вебвью живут в dcomp-underlay ПОД кадром gpui, а в фонах карт под их зоны вырезаются «дыры» (`glint.rs:64,78` — `hole_segments_multi`/`hole_segments`, `glint.rs:122` — `glint_surface_wv_holed`); подложка ровно под вебвью (закрывает разрыв догоняющего ресайза) — editor_bg #1d1c25 dark / #fcfaf6 light (`root.rs:3332`, `palette.rs:59,97`); вокруг дыр канвас заливает bg_sidebar #1d1d28 / #f4f1ea (`root.rs:6060`, `palette.rs:56,94`).
- отступы: инсет вокруг вебвью 8px по бокам и снизу, сверху 0 (`root.rs:3311-3312`; тот же инсет в `visual_wv_body` — `root.rs:3103-3104`); у Customize-обёртки вместо этого pt/pb 8 (`root.rs:6339-6340`); скругление углов чайлдов задаётся не CSS-радиусом, а регионом по зонам (`root.rs:4099` → `overlay.rs:1460`). Оригинал padding/margin не имеет вовсе — геометрия копируется из rect якоря, а инсет принадлежит `.frame`.

### Вердикты

# 76 — verdict (review cycle 1)
VERDICT: DIVERGES (архитектурно, осознанно)
Composition visual hosting вместо DOM-слоя z5 + rect-копирования; нет шторки
140ms; czShared — один вебвью на Customize-страницы.

## Цикл 5: DIVERGES

Осознанно архитектурно: composition hosting + постоянные WebView2-чайлды вместо DOM-слоя z5 с копированием rect/borderRadius. Шторки чата (opacity 140 мс) нет; `czShared` — один вебвью на все Customize-страницы.

## Цикл 6: DIVERGES

Слой вебвью — composition hosting (осознанно), шторки чата нет.

---

## 77. welcome-placeholder — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](77-welcome-placeholder/original.png)
![наш](77-welcome-placeholder/ours.png)

### Оригинал

# 77 welcome-placeholder — оригинал
Файлы: `src/renderer/components/main/WelcomePlaceholder.tsx` (10-37), `src/renderer/components/main/WelcomePlaceholder.module.css`

## JSX-структура (кратко, вложенность)
```
section.welcome [aria-label="Welcome to KaminIDE"]
├─ div.logoWrap (::before — радиальный glow)
│  └─ img.logo (kaminoid.svg, draggable=false)
├─ h1.title — «KaminIDE»
├─ span.version — «v{appVersion}» (условно)
├─ p.tagline — «An AI-native workspace — …»
├─ div.actions
│  ├─ button.primary — fas fa-folder-open + «New session in folder…»
│  └─ button.secondary — fas fa-plus + «Empty session»
└─ div.features
   ├─ span.feature — fas fa-comments + «Claude chat + tools»
   ├─ span.feature — fas fa-folder-tree + «Your files & editor»
   └─ span.feature — fas fa-terminal + «Integrated terminal»
```

## Метрики (ИЗ CSS, точные значения)
- `.welcome`: `flex:1; min-height:0`; flex column, центрирование обеих осей; `text-align:center`; `gap:var(--space-4)`; `padding:var(--space-6)`; `overflow:auto`
- `.logoWrap`: `position:relative; display:grid; place-items:center; margin-bottom:var(--space-1)`
- `.logoWrap::before`: `position:absolute`; 220×220px; `border-radius:50%`; background `radial-gradient(circle, color-mix(in srgb, var(--accent-primary) 26%, transparent) 0%, transparent 68%)`; `filter:blur(6px)`; `z-index:0`
- `.logo`: `position:relative; z-index:1`; 112×112px; `user-select:none; -webkit-user-drag:none`; `filter:drop-shadow(0 6px 18px rgba(0,0,0,0.35))`
- `.title`: `margin:0`; font-family `var(--font-display, inherit)`; `font-size:2.4rem`; `font-weight:700`; `letter-spacing:-0.02em`; `line-height:1.05`; color `var(--text-primary)`; `z-index:1`
- `.version`: `inline-block`; padding `2px 10px`; `border-radius:var(--radius-pill, 999px)`; background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--text-primary)`; font-size `var(--fs-xs)`; `font-variant-numeric:tabular-nums`
- `.tagline`: `margin:0; max-width:30rem`; font-size `var(--fs-md)`; line-height `var(--lh-snug)`; color `var(--text-muted)`
- `.actions`: flex, `flex-wrap:wrap`; `gap:var(--space-3)`; `justify-content:center`; `margin-top:var(--space-2)`
- `.primary`/`.secondary` (общее): `inline-flex; align-items:center`; `gap:var(--space-2)`; padding `var(--space-2) var(--space-4)`; `border-radius:var(--radius-sm)`; font-size `var(--fs-sm)`; `font-weight:600`; cursor pointer; `transition: background var(--transition-fast), transform var(--transition-fast)`
- `.primary`: background `var(--accent-primary)`; color `var(--accent-on-primary, #fff)`; border none
  - hover: background `color-mix(in srgb, var(--accent-primary) 86%, #000)`; `transform:translateY(-1px)`
- `.secondary`: background `color-mix(in srgb, var(--text-primary) 6%, transparent)`; color `var(--text-primary)`; border `1px solid var(--divider-soft, color-mix(in srgb, var(--text-primary) 14%, transparent))`
  - hover: background `color-mix(in srgb, var(--text-primary) 12%, transparent)`; `transform:translateY(-1px)`
- `.features`: flex, wrap; gap `var(--space-2) var(--space-5)` (row col); `justify-content:center`; `margin-top:var(--space-3)`; `max-width:34rem`
- `.feature`: `inline-flex; align-items:center; gap:var(--space-2)`; font-size `var(--fs-sm)`; color `var(--text-muted)`
- `.feature > i`: color `var(--accent-primary)`; `font-size:13px`

## Состояния (классы-варианты с метриками)
- `.version` — только при `appVersion.value`
- hover primary/secondary — см. выше (затемнение/подсветка + подъём на 1px)

### Наша реализация

# 77 welcome-placeholder — наша реализация
Файлы: `crates/shell/src/ui/welcome.rs:42-185` (welcome), `welcome.rs:28-38` (feature-чип), `crates/shell/src/root.rs:4013-4043` (вызов + нативный folder-пикер), `root.rs:290-291,454` (запечённый glow-спрайт), `crates/shell/src/ui/radial_bg.rs` (bake_glow)

## Структура (gpui-дерево кратко)
```
div#welcome (size_full, flex-col, items_center, justify_center, gap 16, p 24, overflow_hidden)
├─ div.relative 112×112 (mb 4)
│  ├─ img glow-спрайт 240×240 absolute (left/top −64) — запечённый radial (в gpui radial-градиента нет)
│  └─ img icons/kaminoid.svg 112×112 relative
├─ div «KaminIDE» — 38px, Bold, text_primary #cfd4e2
├─ div версия-пилюля — px 10, py 2, rounded 999, bg accent_primary 14% (#89b4fa @0.14), FS_XS(11), text_primary
├─ div tagline — max_w 480, FS_MD(13), line-height 13×1.4=18.2, text_muted #838aa0
├─ div actions (flex, wrap, gap 12, justify_center, mt 8)
│  ├─ #welcome-folder: fa folder-open(f07c) 13 + «New session in folder…» — px 16, py 8, rounded 8,
│  │    bg accent_primary #89b4fa, text accent_action_fg #313240, FS_SM(12), Semibold; hover opacity 0.9
│  └─ #welcome-empty: fa plus 13 + «Empty session» — px 16, py 8, rounded 8,
│       bg text_primary 6%, border 1px text_primary 14%, FS_SM, Semibold, text_primary; hover opacity 0.85
└─ div features (flex, wrap, gap_x 20, gap_y 8, justify_center, mt 12, max_w 544)
   └─ 3 × feature: fa-иконка 13 accent_primary + label — FS_SM, text_muted
      (comments f086 «Claude chat + tools», folder-tree f802 «Your files & editor», terminal f120 «Integrated terminal»)
```
Действия: folder → нативный `prompt_for_paths` → `kamin:sessions:newSessionInFolder`; empty → `kamin:sessions:newNoFolderSession`.

## Метрики (из кода, точные)
- gap 16 (SPACE_4), p 24 (SPACE_6) ✓ оригинал; лого 112 ✓; версия `v{CARGO_PKG_VERSION}`.
- Заголовок 38px Bold (оригинал 2.4rem = 38.4px, weight 700) — совпадение по факту.
- tagline max_w 480 = 30rem ✓; features max_w 544 = 34rem ✓; gap 20/8 = space-5/space-2 ✓.
- Цвета: text_primary #cfd4e2, text_muted #838aa0, accent_primary #89b4fa, кнопка-текст #313240 (dark).

## Отличия от original.md той же папки
1. Glow: запечённый спрайт 240×240 (bake_glow, alpha 0.5) вместо CSS `::before` 220×220 radial 26% + blur 6 — размер и профиль градиента приблизительные.
2. Hover кнопок: opacity 0.9/0.85 вместо `color-mix 86% black` / `12% заливки` + `translateY(-1px)` — подъёма нет.
3. Нет `drop-shadow(0 6px 18px rgba(0,0,0,.35))` на лого.
4. Нет `letter-spacing:-0.02em` и `line-height:1.05` у заголовка.
5. Primary-кнопка text = accent_action_fg #313240 (оригинал `--accent-on-primary, #fff` — в dark у оригинала белый, у нас тёмный!).
6. tagline line-height 1.4 (у оригинала `--lh-snug`).
7. Поведенчески: welcome заменяет ВСЮ панельную область (root.rs:5366), в оригинале — только main-колонку.
8. `overflow_hidden` вместо `overflow:auto` (низкие окна клипуют, не скроллят).

## Дополнение атрибутов (цикл 10)

- скругления: версия-пилюля radius 999 (`welcome.rs:100`) = `border-radius: var(--radius-pill, 999px)` оригинала, где токен `--radius-pill` в темах НЕ объявлен (grep пуст) и работает именно фолбэк 999px (`WelcomePlaceholder.module.css:61`); обе кнопки radius-sm 8 (`welcome.rs:134,161`, `metrics/lib.rs:37`) = `border-radius: var(--radius-sm)` (`:90`); glow — запечённый спрайт 220×220 вместо `border-radius: 50%` круга (`welcome.rs:73-78` против `:25-34`), т.е. круглая маска не CSS-радиусом; у feature-чипов скруглений нет ни там, ни там.

### Вердикты

# 77 — verdict (review cycle 1)
VERDICT: DIVERGES (мелочи; ядро подтверждено)
Подтверждено: 38.4 титул, primary #fff + hover ×.86, secondary hover 12%, pill,
gap/паддинги, features, tagline, logo 112.
Расхождения: (1) secondary border 14% → divider-soft 6%; (2) glow 240@.5 →
220@accent26% fade68%; (3) нет text-center у .welcome; (4) overflow hidden vs auto;
(5) lh 1.4 vs 1.3; (6) нет drop-shadow лого.

## Цикл 5: DIVERGES

Welcome: подтверждены фиксы 38.4/700, primary #fff + hover ×0.86, secondary bg 6% + border 6% + hover 12%, центрирование, glow 220, tagline lh 1.3, max-w 480/544. Остаток: нет `drop-shadow(0 6px 18px rgba(0,0,0,.35))` у лого, нет `letter-spacing -0.02em` + `line-height 1.05` у заголовка, `overflow_hidden` вместо `overflow:auto` (низкое окно клипует), `translateY(-1px)` на ховере — ограничение gpui.

## Цикл 6: DIVERGES

Welcome: нет drop-shadow лого, `overflow_hidden` вместо auto.


## Цикл 7: DIVERGES

`overflow: auto` закрыт (`welcome.rs`: `overflow_y_scroll` вместо `overflow_hidden`).
Осталось: `filter: drop-shadow(0 6px 18px rgba(0,0,0,.35))` у лого — в gpui нет
фильтров, а box-shadow дал бы прямоугольную тень под непрямоугольным SVG.

---

## 78. customize-content-panel — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](78-customize-content-panel/original.png)
![наш](78-customize-content-panel/ours.png)

### Оригинал

# 78 customize-content-panel — оригинал
Файлы: `src/renderer/components/main/CustomizePanel.tsx` (31-48 — панель, 81-88 — `ComingSoon`), `src/renderer/components/main/CustomizePanel.module.css`

## JSX-структура (кратко, вложенность)
```
section.panel
├─ header.header
│  ├─ h1.title — contributed?.name | titleFor(panel): Extensions/Logs/System/Design/Settings
│  └─ p.subtitle — «Contributed by an extension.» | subtitleFor(panel)
└─ div.{bodyFlush|body}   (contributed → bodyFlush)
   └─ contributed → <ContributedViewBody viewId flush />
      | "extensions" → <ExtensionsPanel /> | "logs" → <LogsPanel /> | "system" → <SystemLogPanel />
      | "design" → <DesignPanel /> | "settings" → <SettingsPanel />
      | иначе → <ComingSoon> = div.placeholder (i.fas.fa-screwdriver-wrench + span «Phase B»)
```

## Метрики (ИЗ CSS, точные значения)
- `.panel`: `flex:1`; flex column; `overflow:hidden`
- `.header`: padding `var(--space-5) var(--space-6) var(--space-3)` (top right/left bottom); `border-bottom:1px solid color-mix(in srgb, var(--bg-overlay) 30%, transparent)`
- `.title`: `margin:0`; font-size `var(--fs-xl)`; `font-weight:600`; color `var(--text-primary)`
- `.subtitle`: margin `var(--space-1) 0 0`; font-size `var(--fs-md)`; color `var(--text-muted)`
- `.body`: `flex:1; overflow-y:auto`; padding `var(--space-4) var(--space-6)`
- `.bodyFlush`: `flex:1; display:flex; flex-direction:column; min-height:0; overflow:hidden` — БЕЗ padding (webview edge-to-edge, без card-in-a-card)
- `.placeholder`: flex column, центрирование; `gap:var(--space-2)`; `padding:var(--space-7)`; color `var(--text-muted)`
- `.placeholder i`: `font-size:32px; opacity:0.5`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- `.body` (встроенные страницы, с padding) ↔ `.bodyFlush` (contributed webview-страница, flush)
- `ComingSoon` — фоллбек для неизвестной страницы

### Наша реализация

# 78 customize-content-panel — наша реализация
Файлы: `crates/shell/src/ui/customize.rs:163-172` (title_for), `customize.rs:261-340` (customize_panel), `crates/shell/src/root.rs:5297-5352` (обёртка glint-картой на всю область + contrib-ветка czShared)

## Структура (gpui-дерево кратко)
```
glint_surface_wv_holed (root.rs:5302) — карта на ВСЮ панельную область
└─ customize_panel:
   div (size_full, flex-col, min_h 0, p 20)
   ├─ div title — 20px, Semibold, text_primary #cfd4e2
   ├─ div subtitle — mt 2, mb 12, FS_SM(12), text_muted #838aa0
   └─ div (flex_1, min_h 0) → body:
      settings → pref-строки | design → design_panel | extensions → extensions_panel
      | logs → logs_panel | system → system_panel
      | _ → центр «Coming soon» (text_muted)
   contrib-страница (root.rs:5304-5333): вместо customize_panel — czShared-вебвью
      (div#cz-contrib.relative.size_full + probe_area + wv) либо центр «Loading…»
```

## Метрики (из кода, точные)
- Общий паддинг панели: p 20 (SPACE_5) — единый, и для хедера, и для тела.
- title 20px Semibold; subtitle FS_SM(12), mt 2, mb 12 (SPACE_3).
- ComingSoon: только текст «Coming soon» text_muted #838aa0, по центру.
- Тексты title/subtitle: Settings/«KaminIDE app preferences.», Design/«Themes, icons and visual tuning.», Extensions/«Installed extensions.», Logs/«Host and extension logs.», System/«System log and diagnostics.».

## Отличия от original.md той же папки
1. title 20px вместо `--fs-xl` 22; subtitle FS_SM 12 вместо `--fs-md` 13.
2. Нет `border-bottom` под хедером (`1px color-mix(bg-overlay 30%)`).
3. Паддинги: оригинал header `20 24 12` + body `16 24`; у нас всё p 20 — правый/левый уже (20 vs 24).
4. Тело не скроллится централизованно (`overflow-y:auto` в `.body` оригинала); скролл — внутри конкретных страниц (design/extensions — `overflow_y_scrollbar`).
5. ComingSoon без иконки `fa-screwdriver-wrench` 32px и текста «Phase B» — просто «Coming soon».
6. contrib-страница: тот же flush-принцип (без паддинга), но через переиспользуемый czShared-вебвью; subtitle «Contributed by an extension.» не выводится (хедера вообще нет — вебвью на всю карту).
7. Тексты subtitle отличаются от оригинала (у оригинала свои subtitleFor-строки).
8. Панель обёрнута в glint-карту на всю панельную область (в оригинале CustomizePanel живёт внутри main-карты).

## Дополнение атрибутов (цикл 10)

- гэпы: flex-`gap` у шелла нет — `customize_panel` это `flex_col` из хедера и тела без gap (`customize.rs:499-544`), вертикальный ритм задают паддинги: хедер pt 20 / px 24 / pb 12 (`customize.rs:513-516`), тело py 16 / px 24 (`customize.rs:538-539`). Совпадает с оригиналом (`.panel`/`.header`/`.body` без gap, `CustomizePanel.module.css:1-30`). Отличие: `.placeholder { gap: var(--space-2) 8 }` (`:49`) у нас не воспроизведён — fallback «Coming soon» это один центрированный текст без gap и без иконки (`customize.rs:496-505`).
- ховер: N/A: ховер — у шелла панели hover-правил нет (`customize.rs:499-544`), как и у `.panel`/`.header`/`.body` оригинала; ховеры принадлежат навигации Customize (элемент 36) и содержимому конкретной страницы (79/80/84/86).

### Вердикты

# 78 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет header-блока (p 20/24/12 + border-b bg-overlay 30%); титул 20 vs fs-xl 22;
сабтайтл 12 vs 13, mt2 vs space-1; нет .body padding 16/24; ComingSoon без
fa-screwdriver-wrench 32/op.5 и p space-7. (customize.rs:317-338)

## Цикл 5: DIVERGES

Customize-хедер: нет блока `.header { padding: 20 24 12; border-bottom: 1px color-mix(bg-overlay 30%) }` и `.body { padding: 16 24; overflow-y:auto }` — у нас единый `p(20)` без линии. Сабтайтл должен быть fs-md 13 + mt 4 (у нас fs-sm 12 + mt 2). **Все пять текстов сабтайтлов другие** (см. `CustomizePanel.tsx:73-79`). ComingSoon без `fa-screwdriver-wrench 32/op .5`, без «Phase B», без `padding: space-7`. Титул 22 верен.

## Цикл 6: DIVERGES

**Закрыто волной 9**: хедер 20/24/12 + нижняя линия, тело 16/24, сабтайтл 13 + mt 4, все пять текстов дословно из оригинала. Осталось: у тела нет `overflow-y: auto`; ComingSoon без глифа 32/op .5, «Phase B» и `padding: space-7`.

---

## 79. design-panel-shell — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](79-design-panel-shell/original.png)
![наш](79-design-panel-shell/ours.png)

### Оригинал

# 79 design-panel-shell — оригинал
Файлы: `src/renderer/components/main/DesignPanel.tsx` (18-41 — панель, 43-55 — `Section`), `src/renderer/components/main/DesignPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
└─ 6 × section.section (Colors / Typography / Spacing / Radius / Shadows / Components)
   ├─ header.sectionHeader
   │  ├─ h2.sectionTitle — title
   │  └─ p.sectionSubtitle — subtitle
   └─ div.sectionBody — {ColorTokens|TypographyTokens|SpacingTokens|RadiusTokens|ShadowTokens|ComponentSamples}
```
Тексты сабтайтлов: «Theme tokens — resolve from the active dark/light palette.», «Font families + the 5-step size scale.», «space-1..7 — every gap/padding in the codebase resolves to one of these.», «4-step concentric scale anchored at 16px outer.», «Elevation tokens. Lower index = more grounded.», «Live samples — values track the palette above.»

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `gap:var(--space-6)`; `padding-bottom:var(--space-6)`
- `.section`: flex column; `gap:var(--space-3)`
- `.sectionHeader`: flex column; `gap:2px`
- `.sectionTitle`: `margin:0`; font-size `var(--fs-lg)`; `font-weight:600`; color `var(--text-primary)`
- `.sectionSubtitle`: `margin:0`; font-size `var(--fs-sm)`; color `var(--text-muted)`; line-height `var(--lh-snug)`
- `.sectionBody`: border `1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)`; `border-radius:var(--radius-md)`; background `var(--bg-mantle)`; `padding:var(--space-4)`
- hover/active/focus — нет; transition — нет; позиционирование — поток

## Состояния (классы-варианты с метриками)
- Вариантов нет; read-only контейнер, значения токенов резолвятся из активной темы в рендер-тайме.

### Наша реализация

# 79 design-panel-shell — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs:18-44` (section), `design_panel.rs:269-313` (design_panel — сборка 6 секций)

## Структура (gpui-дерево кратко)
```
div#design-panel (flex-col, size_full, min_h 0, overflow_y_scrollbar)
└─ 6 × section(title, subtitle, body):
   div (flex-col, mb 20)
   ├─ div title — FS_MD(13), Semibold, text_primary #cfd4e2
   ├─ div subtitle — mb 8, FS_XS(11), text_muted #838aa0
   └─ body (Colors / Typography / Spacing / Radius / Shadows / Components)
```
Тексты сабтайтлов 1:1 с оригиналом («Theme tokens — resolve from the active dark/light palette.» и т.д.).

## Метрики (из кода, точные)
- Разрядка секций: mb 20 (SPACE_5) на секцию (не gap контейнера).
- title 13px Semibold; subtitle 11px, mb 8 (SPACE_2).
- Скролл: собственный `overflow_y_scrollbar` (gpui-component).
- Тела секций — БЕЗ рамки/фона/паддинга (голый поток).

## Отличия от original.md той же папки
1. `sectionTitle`: у нас FS_MD 13 против `--fs-lg` 16 оригинала; `sectionSubtitle` FS_XS 11 против `--fs-sm` 12.
2. `.sectionBody`-карточки НЕТ: оригинал — border `1px color-mix(bg-surface 60%)` + radius-md 12 + bg `--bg-mantle` + padding 16; у нас тело секции без обрамления.
3. Межсекционный ритм: mb 20 у секции против `gap:var(--space-6)` 24 у `.root`; нет `padding-bottom:24`.
4. Нет `gap:12` внутри секции и `gap:2px` в хедере — свои отступы (subtitle mb 8).
5. Скролл живёт на самой панели (в оригинале скроллит `.body` CustomizePanel).

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — оболочка design-панели статична: `section()` (`design_panel.rs:26-60`) и сборка секций (`design_panel.rs:496+`) hover-правил не задают, у `.root`/`.section`/`.sectionBody` оригинала их тоже нет (`DesignPanel.module.css:1-39`); hover есть только внутри сэмплов (элементы 135–152).

### Вердикты

# 79 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет .sectionBody (border bg-surface60% + bg-mantle + r12 + p16); sectionTitle 13
vs fs-lg 16; subtitle 11 vs fs-sm 12 (без lh-snug); секции mb20 vs gap24 + pb24.
(Витрины токенов ПЕРЕПИСАНЫ в wave2 — shell-стили карточек ещё нет.)

## Цикл 5: DIVERGES

Design-панель: нет `.sectionBody` (border 1px bg-surface 60% + radius-md 12 + bg-mantle + padding 16) — тело секции голое; `sectionTitle` 13 вместо fs-lg 16, `sectionSubtitle` 11 вместо fs-sm 12 + lh-snug; ритм `mb 20` вместо `.root { gap 24; padding-bottom 24 }`, нет `gap 12` в секции и `gap 2` в хедере.

## Цикл 6: MATCH

**Закрыто волной 9**: карточка `.sectionBody`, заголовок fs-lg/600, сабтайтл fs-sm + lh-snug, `gap 12`/`gap 2`, ритм 24.

---

## 80. logs-panel — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](80-logs-panel/original.png)
![наш](80-logs-panel/ours.png)

### Оригинал

# 80 logs-panel — оригинал
Файлы: `src/renderer/components/main/LogsPanel.tsx` (73-137), `src/renderer/components/main/LogsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
channels.length===0 →
div.empty — i.fas.fa-inbox + span «No output channels yet. Extensions register them via <code>vscode.window.createOutputChannel(name)</code>.»

иначе:
div.layout
├─ nav.list [aria-label="Output channels"]
│  └─ button.item(.active) [data-tooltip="{extensionId} · {name}"]
│     ├─ span.itemName — c.name
│     └─ span.itemExt — c.extensionId
└─ div.right
   ├─ header.toolbar
   │  ├─ input[type=search].search [placeholder="Filter…"]
   │  ├─ button.toolBtn [data-tooltip="Copy entire buffer"] — codicon-copy (disabled при пустом буфере)
   │  └─ button.toolBtn [data-tooltip="Clear channel"] — codicon-clear-all (disabled при пустом буфере)
   └─ pre.body (ref, auto-scroll) — visibleBuffer
```
Поведение: фильтр debounce 150ms, сбрасывается при смене канала; stick-to-bottom с зазором 6px.

## Метрики (ИЗ CSS, точные значения)
- `.layout`: `display:grid; grid-template-columns:220px 1fr`; `gap:var(--space-3)`; `height:100%; min-height:0`
- `.list`: flex column; `gap:2px`; `overflow:auto`; `padding-right:var(--space-2)`
- `.item`: flex column, `align-items:flex-start`; `gap:2px`; padding `var(--space-2) var(--space-3)`; background transparent; border `1px solid transparent`; `border-radius:var(--radius-sm)`; color `var(--text-secondary)`; `font:inherit`; `text-align:left`; `width:100%`; `transition:background var(--transition-fast)`
  - hover: background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `var(--text-primary)`
  - `.item.active`: background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--accent-primary)`; border-color `color-mix(in srgb, var(--accent-primary) 35%, transparent)`
- `.itemName`: font-size `var(--fs-sm)`; `font-weight:500`
- `.itemExt`: font-size `var(--fs-xs)`; color `var(--text-muted)`; font-family `var(--font-mono)`
- `.right`: `display:grid; grid-template-rows:auto 1fr`; `gap:var(--space-2)`; `min-height:0`
- `.toolbar`: flex, `align-items:center`; `gap:var(--space-2)`
- `.search`: `flex:1`; padding `4px 8px`; background `var(--bg-base)`; color `var(--text-primary)`; border `1px solid var(--bg-surface)`; `border-radius:var(--radius-sm)`; font-size `var(--fs-sm)`; `outline:none`
  - focus: `border-color:var(--accent-primary)`
- `.toolBtn`: 26×26px; `display:grid; place-items:center`; background transparent; color `var(--text-secondary)`; border none; `border-radius:var(--radius-sm)`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover (не disabled): background `var(--bg-surface)`; color `var(--text-primary)`
  - `[disabled]`: `opacity:0.4; cursor:not-allowed`
  - `.toolBtn .codicon`: `font-size:14px`
- `.body`: background `var(--bg-base)`; border `1px solid var(--bg-surface)`; `border-radius:var(--radius-sm)`; `padding:var(--space-3)`; font-family `var(--font-mono)`; font-size `var(--fs-xs)`; color `var(--text-primary)`; `overflow:auto`; `white-space:pre-wrap; word-break:break-word`; line-height `var(--lh-snug)`; `margin:0`
- `.empty`: flex column, центрирование; `gap:var(--space-2)`; `height:100%`; color `var(--text-muted)`; `text-align:center`; `padding:var(--space-5)`
  - `.empty i`: `font-size:32px; opacity:0.6`; `.empty code`: `var(--font-mono)`, `var(--fs-xs)`

## Состояния (классы-варианты с метриками)
- `.item.active` — активный канал (акцентная заливка 14% + бордер 35%)
- `.toolBtn[disabled]` — при отсутствии канала/пустом буфере
- empty-state — при 0 каналов (вся панель заменяется `.empty`)

### Наша реализация

# 80 logs-panel — наша реализация
Файлы: `crates/shell/src/ui/logs_panel.rs:47-199` (logs_panel), `logs_panel.rs:24-44` (filter_input, matches), `crates/shell/src/output_log.rs` (OutputChannels — буферы каналов)

## Структура (gpui-дерево кратко)
```
div (flex, size_full, min_h 0)
├─ list: div (flex-col, w 220, flex_shrink_0, gap 2, pr 8)
│  ├─ 0 каналов → div «No output channels yet» FS_XS text_muted
│  └─ строка канала #och-{key}: div flex-col, px 8, py 4, rounded 8
│     ├─ name — FS_SM(12), text_primary, ellipsis
│     └─ extension_id — FS_XS(11), text_muted, ellipsis
│     hover bg text_primary 8%·wait→6%; active bg accent_primary 16%
└─ right: div (flex-col, flex_1, min_w 0, min_h 0)
   ├─ toolbar (flex, justify_end, gap 8, pb 4)
   │  ├─ filter_input: w 240, px 8, py 2, rounded 8, bg bg_primary 60% (#313240@0.6),
   │  │    border 1px text_primary 8%, codicon-filter(ea6d) 12 + Input (gpui-component)
   │  └─ #log-clear: codicon-clear-all(ea76) 12 + «Clear» — px 8, py 3, rounded 8, FS_XS,
   │       text_muted; hover bg text_primary 8% + text_primary
   └─ #log-buffer: div flex-col, flex_1, overflow_y_scrollbar, p 8, rounded 8,
        bg bg_primary 60%, font «JetBrains Mono» 11px, text_secondary #adb3c7
        — последние 400 отфильтрованных строк построчными div
        пусто → «Buffer is empty» | «No lines match the filter»
   нет активного канала → центр «Select a channel»
```

## Метрики (из кода, точные)
- Колонка каналов 220 ✓; hover строки 6% text_primary; active bg #89b4fa@0.16.
- Буфер: моно 11px (= fs-xs ✓), паддинг 8, радиус 8, кап рендера 400 строк.
- Фильтр: case-insensitive substring, без debounce (посимвольно через InputState).

## Отличия от original.md той же папки
1. Тулбар прижат вправо (justify_end); фильтр фикс 240px — у оригинала `search flex:1` на всю ширину.
2. Кнопка Clear — иконка+текст «Clear»; у оригинала два icon-only `.toolBtn` 26×26 (copy + clear) с disabled-состояниями. **Copy отсутствует.**
3. Активный канал: только заливка accent 16%; у оригинала 14% + `border accent 35%` + текст красится в accent_primary.
4. `itemExt` не моноширинный (оригинал `--font-mono`).
5. Буфер: фон `bg_primary@0.6` без бордера (оригинал bg-base + border 1px bg-surface, padding 12); построчные div вместо `pre` (нет white-space:pre-wrap переносов — длинные строки клипуются render-движком).
6. Нет auto-scroll/stick-to-bottom — показывается хвост 400 строк без скролл-позиционирования.
7. Empty-state панели: голый текст, без `fa-inbox` 32px и `<code>vscode.window.createOutputChannel</code>`.
8. Раскладка flex вместо grid `220px 1fr` c gap 12 — у нас pr 8 (межколоночный зазор уже).
9. Фильтр не сбрасывается при смене канала и без debounce 150ms.

## Дополнение атрибутов (цикл 10)

- скругления: инпут фильтра radius-sm 8 (`logs_panel.rs:33`, `metrics/lib.rs:37`), кнопки тулбара 26×26 radius-sm 8 (`logs_panel.rs:63`), строка канала radius-sm 8 (`logs_panel.rs:160`), буфер-тело radius-sm 8 (`logs_panel.rs:245`). Оригинал: `.search`, `.toolBtn`, `.item`, `.body` — все `border-radius: var(--radius-sm)` (`LogsPanel.module.css:77,92,25,103`) — 1:1; empty-состояние скруглений не имеет ни там, ни там.

### Вердикты

# 80 — verdict (review cycle 1)
VERDICT: DIVERGES (мелочи; ядро подтверждено)
Подтверждено: layout gap12+колонка 220, item-рецепт+active, search flex1, toolBtn
26 copy/clear, body bg-base mono, empty-state.
Расхождения: .list без скролла; item hover без text-primary; lh 1.4 vs 1.3;
inbox = codicon vs fa + <code> не mono; свои заглушки пустого буфера; нет debounce
150ms и stick-to-bottom(6px).

## Цикл 5: DIVERGES

Logs: подтверждены колонка 220 + gap 12 + pr 8, item 8/12 + border-резерв + hover 50% + active accent 14%/35%, mono-расширение, поиск 4/8, toolBtn 26 codicon 14 с disabled 0.4, тело bg-base + p12 + mono fs-xs, empty inbox 32 op .6. Остаток: у `.list` нет `overflow:auto`; empty-текст 12 против унаследованных 16 и `<code>` не моно; тело `line_height(FS_XS*1.4)` вместо lh-snug 1.3; ховер строки не поднимает цвет до primary; нет debounce 150 мс, сброса фильтра при смене канала и stick-to-bottom (6px); буфер срезан до 400 строк.

## Цикл 6: DIVERGES

Не тронуто: у списка каналов нет `overflow`, тело `lh` 1.4 вместо 1.3, empty-текст мельче оригинала и `<code>` не моно, глиф codicon вместо fa-inbox.


## Цикл 7: DIVERGES

Закрыто: `.list { overflow: auto }` (скролл списка каналов), `.body` line-height
`--lh-snug` 1.3 (было 1.4), empty-state — FontAwesome fa-inbox 32 op .6 вместо codicon,
текст 16px (кегль наследуется от документа, `.empty` его не задаёт) и `<code>` моно fs-xs.

Осталось: подтвердить живьём кадром (панель Logs при нуле каналов) — вердикт по коду.

---

## 81. system-log-panel — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](81-system-log-panel/original.png)
![наш](81-system-log-panel/ours.png)

### Оригинал

# 81 system-log-panel — оригинал
Файлы: `src/renderer/components/main/SystemLogPanel.tsx` (27-72), `src/renderer/components/main/SystemLogPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.layout
├─ div.toolbar
│  ├─ input[type=search].search [placeholder="Filter logs…"]
│  ├─ div.levels [role=group, aria-label="Filter by level"]
│  │  └─ 4 × button.levelBtn(.levelActive) [aria-pressed] — all / error / warning / info
│  └─ button.clear [data-tooltip="Clear logs"] — codicon-clear-all
└─ visible.length===0 →
   div.empty — i.fas.fa-inbox + span («No system logs yet — …» | «No logs match the filter.»)
   иначе:
   ul.list
   └─ li.row(.error|.warning|.info)
      ├─ i.codicon.codicon-{error|warning|info}.icon
      ├─ span.source — e.source
      ├─ span.message — e.message
      └─ span.time [data-tooltip=absoluteTime] — relativeTime
```
Порядок: newest-first (reverse).

## Метрики (ИЗ CSS, точные значения)
- `.layout`: flex column; `height:100%; min-height:0`
- `.toolbar`: flex, `align-items:center`; `gap:var(--space-2)`; padding `0 0 var(--space-2)`; `flex-shrink:0`
- `.search`: `flex:1; min-width:0`; `height:28px`; padding `0 10px`; background `var(--bg-base)`; border `1px solid var(--divider-soft)`; `border-radius:var(--radius-sm)`; color `var(--text-primary)`; `font:inherit`; font-size `var(--fs-sm)`; `outline:none`
  - focus: `border-color:var(--accent-primary)`
- `.levels`: flex; `gap:2px`
- `.levelBtn`: padding `4px 10px`; background transparent; border `1px solid transparent`; `border-radius:var(--radius-sm)`; color `var(--text-muted)`; `font:inherit`; font-size `var(--fs-xs)`; `text-transform:capitalize`
  - hover: color `var(--text-primary)`; background `color-mix(in srgb, var(--text-primary) 8%, transparent)`
  - `.levelActive`: color `var(--text-primary)`; background `color-mix(in srgb, var(--accent-primary) 22%, transparent)`
- `.clear`: `display:grid; place-items:center`; 28×28px; `flex-shrink:0`; background transparent; border none; `border-radius:var(--radius-sm)`; color `var(--text-muted)`
  - hover: color `var(--text-primary)`; background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.list`: `flex:1; min-height:0; overflow-y:auto`; `margin:0; padding:0; list-style:none`; font-family `var(--font-mono, ui-monospace, monospace)`; font-size `var(--fs-xs)`
- `.row`: `display:grid; grid-template-columns:16px max-content 1fr max-content`; `align-items:baseline`; `gap:var(--space-2)`; padding `3px var(--space-2)`; `border-bottom:1px solid color-mix(in srgb, var(--divider-soft) 50%, transparent)`
  - hover: background `color-mix(in srgb, var(--text-primary) 5%, transparent)`
- `.icon`: `align-self:center`; `font-size:13px`
  - `.error .icon`: color `var(--accent-red)`; `.warning .icon`: `var(--accent-yellow, #d8a657)`; `.info .icon`: `var(--accent-blue)`
- `.source`: color `var(--text-muted)`; `white-space:nowrap`
- `.message`: color `var(--text-primary)`; `white-space:pre-wrap; word-break:break-word; overflow-wrap:anywhere`
  - `.error .message`: color `var(--accent-red)`
- `.time`: color `var(--text-muted)`; `white-space:nowrap`; font-size `var(--fs-xs)`
- `.empty`: `flex:1`; flex column, центрирование; `gap:var(--space-2)`; color `var(--text-muted)`; `text-align:center`; `padding:var(--space-4)`
  - `.empty > i`: `font-size:24px; opacity:0.5`
- transition — нет

## Состояния (классы-варианты с метриками)
- `.levelActive` — активный сегмент фильтра (акцент 22%)
- `.row.error|.warning|.info` — цвет иконки; error красит и message в `--accent-red`
- empty-state: разные тексты для «пусто вообще» и «ничего не подошло под фильтр»

### Наша реализация

# 81 system-log-panel — наша реализация
Файлы: `crates/shell/src/ui/logs_panel.rs:202-300` (system_panel), `logs_panel.rs:24-44` (filter_input, matches), `crates/shell/src/output_log.rs` (SysEntry)

## Структура (gpui-дерево кратко)
```
div (flex-col, size_full, min_h 0)
├─ toolbar (flex, justify_end, gap 8, pb 4)
│  ├─ filter_input (общий с Logs): w 240, px 8, py 2, rounded 8, bg bg_primary 60%, codicon-filter 12
│  └─ #syslog-clear «Clear» — px 8, py 3, rounded 8, FS_XS, text_muted; hover bg 8% + text_primary
└─ #syslog-body: div flex-col, flex_1, min_h 0, overflow_y_scrollbar, gap 2
   ├─ пусто → центр «No diagnostics yet» | «No entries match the filter»
   └─ строка (newest-first, iter().rev()): div flex, items_center, gap 8, px 8, py 4, rounded 8
      ├─ codicon уровня 13px: error ea87 accent_red #f38ba8 / warning ea6c accent_primary #89b4fa / info ea74 accent_blue #89b4fa
      ├─ source — w 70 fixed, FS_XS(11), text_muted #838aa0
      └─ message — flex_1, min_w 0, FS_SM(12), text_secondary #adb3c7
```
Фильтр матчит по `"{level} {source} {message}"` case-insensitive.

## Метрики (из кода, точные)
- Строка: px 8, py 4, gap 8, rounded 8; иконка 13px; source-колонка 70px.
- newest-first ✓ (reverse).

## Отличия от original.md той же папки
1. **Нет сегментированного фильтра уровней** (all/error/warning/info, aria-pressed, accent 22%) — только текстовый фильтр.
2. **Warning-иконка накрашена accent_primary #89b4fa вместо `--accent-yellow`** (#f9e2af) — грубое цветовое расхождение.
3. Нет колонки времени (relative-time + tooltip absolute).
4. `.error .message` у оригинала красится в accent-red; у нас message всегда text_secondary.
5. Строки не моноширинные (оригинал `.list` font-mono fs-xs); у нас message FS_SM обычным UI-шрифтом.
6. Раскладка flex (gap 8) вместо grid `16px max-content 1fr max-content` baseline; нет `border-bottom` между строками; нет hover 5%.
7. Search: фикс 240px справа вместо `flex:1` height 28 слева; свой стиль (bg_primary 60% vs bg-base + divider-soft).
8. Clear — текстовая кнопка без иконки (оригинал 28×28 icon-only codicon-clear-all + tooltip).
9. Empty-state без `fa-inbox` 24px.

## Дополнение атрибутов (цикл 10)

- скругления: поле поиска radius-sm 8 (`logs_panel.rs:320`), кнопки уровней radius-sm 8 (`logs_panel.rs:343`), кнопка Clear 28×28 radius-sm 8 (`logs_panel.rs:372`); у строк списка скруглений нет (`logs_panel.rs:441-451`). Оригинал: `.search`/`.levelBtn`/`.clear` — `var(--radius-sm)` (`SystemLogPanel.module.css:23,35,54`), `.row` без радиуса (`:71-78`) — 1:1.

### Вердикты

# 81 — verdict (review cycle 1)
VERDICT: DIVERGES (крупно)
Warning=accent-yellow подтверждён. Нет сегмент-фильтра .levels (levelBtn 4×10
r-sm, levelActive accent 22%); clear текстовый vs 28×28 codicon-clear-all; search
не h28/0-10/divider-soft; нет grid-строки 16px/max/1fr/max и колонки .time
(+tooltip); row px8 py4 rounded vs 3/8 border-b divider-soft50%; нет hover 5%;
message secondary fs-sm vs primary fs-xs mono (+error=red); список не mono;
empty без fa-inbox 24 и с другим текстом.

## Цикл 5: DIVERGES

System log — крупно: нет сегментов all/error/warning/info (`padding 4px 10px`, r-sm, active accent 22% + primary); Clear должен быть 28×28 grid + codicon-clear-all + тултип, а не текст; поле фильтра ~40px вместо `height 28 + padding 0 10 + border divider-soft`, плейсхолдер «Filter…» вместо «Filter logs…»; строка — flex gap8 px8 py4 r8 вместо `grid 16px max-content 1fr max-content` + baseline + `padding 3px space-2` + `border-bottom 1px divider-soft 50%` + hover 5%; нет колонки времени (relative + tooltip); message fs-sm/text-secondary вместо fs-xs/mono/primary (+ `.error .message` = accent-red); `.source` фикс w70 вместо max-content nowrap; empty без `fa-inbox 24 op .5`. Warning = accent_yellow — исправлено.

## Цикл 6: DIVERGES

**Переписан волной 9**: сегменты уровней, `.row` grid/baseline/3px 8/border-bottom/hover 5%, колонка времени, моно fs-xs + красное сообщение ошибки, `.source` max-content, empty глиф 24 op .5, поле h28/px10/bg-base/border + «Filter logs…». Замеры ц.6: пилюля 20.8 против 21.6, поле 28.8 против 28 — совпало. Остаток: **глиф Clear был `word-wrap` вместо `clear-all` (исправлено волной 10)**; `Input` не наследует размер/паддинг обёртки (текст ~15 вместо 12, инсет 24 вместо 11); нет тултипа абсолютного времени; тексты empty не оригинальные.

---

## 82. settings-panel — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](82-settings-panel/original.png)
![наш](82-settings-panel/ours.png)

### Оригинал

# 82 settings-panel — оригинал
Файлы: `src/renderer/components/settings/SettingsPanel.tsx` (28-74), `src/renderer/components/settings/SettingsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ <LegacyBridgeCard />   (элемент 83, условный)
├─ section.section — «Notifications»
│  ├─ h3.sectionTitle
│  └─ label.row [for=pref-background-toasts]
│     ├─ input[type=checkbox] (disabled пока !loaded)
│     └─ span.rowText — «Show background notifications when KaminIDE is not focused»
│        └─ span.rowDesc — «Raises a native, always-on-top toast when …»
└─ section.section — «Terminal»
   └─ label.row [for=pref-use-conpty]
      ├─ input[type=checkbox]
      └─ span.rowText — «Use the system ConPTY DLL (Windows-signed)»
         └─ span.rowDesc — «Off (default) uses node-pty's bundled ConPTY — …»
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `gap:var(--space-4)`
- `.section`: flex column; `gap:var(--space-2)`
- `.sectionTitle`: `margin:0`; `font-size:11px`; `font-weight:600`; `letter-spacing:0.06em`; `text-transform:uppercase`; color `var(--text-muted)`
- `.row`: flex, `align-items:flex-start`; `gap:10px`; padding `4px 0`; `cursor:pointer`; `font-size:13px`; color `var(--text-primary)`
- `.row input`: `margin-top:2px`
- `.rowText`: `display:block`
- `.rowDesc`: `display:block`; `margin-top:2px`; `font-size:11px`; `line-height:1.5`; color `var(--text-muted)`
- `.placeholder` (в css, в текущем JSX не используется): flex column, центр; `gap:var(--space-2)`; padding `var(--space-6) 0`; color `var(--text-muted)`; `.placeholder i` — `font-size:32px; opacity:0.5`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- Чекбоксы `disabled` пока prefs не загрузились (`!loaded`)
- `LegacyBridgeCard` рендерится только при найденном legacy-Bridge

### Наша реализация

# 82 settings-panel — наша реализация
Файлы: `crates/shell/src/ui/customize.rs:273-301` (ветка "settings" в customize_panel), `customize.rs:174-245` (pref_row), `customize.rs:247-256` (section)

## Структура (gpui-дерево кратко)
```
div (flex-col)
├─ section «Notifications» — mt 16, mb 4, FS_SM(12), Semibold, text_secondary #adb3c7
├─ pref_row #pref-toasts (backgroundToasts)
├─ section «Terminal»
└─ pref_row #pref-conpty (useConptyDll)

pref_row: div (flex, items_start, gap 12, p 8, rounded 8, cursor_pointer, hover bg text_primary 4%)
├─ кастомный чекбокс 16×16: mt 2, rounded 4, border 1px text_muted #838aa0;
│    checked → bg+border accent_primary #89b4fa, галка codicon eab2 12px цветом accent_action_fg #313240
└─ div flex-col gap 2
   ├─ label — FS_SM(12), text_primary #cfd4e2
   └─ desc — FS_XS(11), text_muted #838aa0, max_w 560
```
Клик по всей строке тумблерит `kamin:prefs:set`; до загрузки prefs строка `opacity 0.5`.
Тексты label/desc 1:1 с оригиналом (toasts + ConPTY DLL).

## Метрики (из кода, точные)
- Чекбокс 16×16, radius 4, галка 12px; строка p 8, gap 12; desc max_w 560.
- Заголовок секции: mt 16 (SPACE_4), mb 4 (SPACE_1), 12px Semibold #adb3c7.

## Отличия от original.md той же папки
1. `sectionTitle`: у оригинала uppercase 11px weight 600 letter-spacing .06em text_muted; у нас 12px Semibold text_secondary без uppercase/letter-spacing.
2. Чекбокс кастомный 16×16 (оригинал — нативный `input[type=checkbox]` с margin-top 2).
3. Строка: gap 12 + p 8 + rounded 8 + hover 4% (оригинал gap 10, padding `4px 0`, без hover и радиуса).
4. Loading-состояние: opacity 0.5 всей строки вместо `disabled` на чекбоксе (клик при этом не блокируется — SetPref уйдёт).
5. desc: FS_XS 11 ✓, но без `line-height:1.5`; ограничение max_w 560 — своё.
6. **LegacyBridgeCard отсутствует** (см. 83).
7. Контейнер: без `gap:16` между секциями — ритм задают mt/mb заголовков.

### Вердикты

# 82 — verdict (review cycle 1)
VERDICT: DIVERGES
sectionTitle 12/secondary/mixed vs 11/600/uppercase/ls.06/muted; row gap12 p8
+лишние rounded/hover vs gap10 p 4/0 без hover; кастомный чекбокс vs нативный;
rowDesc без lh1.5; !loaded глушит всю строку; нет LegacyBridgeCard.

## Цикл 5: DIVERGES

Settings: `sectionTitle` должен быть 11/600/uppercase/text-muted (у нас 12/Semibold/text-secondary mixed-case — на кадре «Notifications» вместо «NOTIFICATIONS»); `.row` — `gap 10; padding 4px 0` без фона, радиуса и ховера (у нас gap 12 + p8 + r8 + hover 4%); label 13 вместо 12; `rowDesc` без `line-height 1.5` и с лишним `max_w 560` (описание переносится в две строки там, где оригинал влезает в одну); чекбокс кастомный вместо нативного; при `!loaded` строка гаснет, но клик проходит — оригинал ставит `disabled`; нет `.root { gap: 16 }`.

## Цикл 6: DIVERGES

**Закрыто волной 9**: sectionTitle 11/600/uppercase/muted, `.row` gap 10 + 4px 0 без фона/ховера, label 13, `rowDesc` lh 1.5, клик до загрузки не уходит в `SetPref`. **Регрессия волны 9 (снял `max_w`, не сделал колонку сжимаемой) — описание обрезалось; исправлено волной 10** (`flex_1 + min_w 0`). Осталось: `.section gap 8` (у нас 4), лишние 16 сверху у первой секции, `opacity .5` на всей строке вместо `disabled` у чекбокса, кастомный чекбокс 16 вместо нативного 13.

---

## 83. legacy-bridge-card — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](83-legacy-bridge-card/original.png)
![наш](83-legacy-bridge-card/ours.png)

### Оригинал

# 83 legacy-bridge-card — оригинал
Файлы: `src/renderer/components/settings/LegacyBridgeCard.tsx` (82-101), `src/renderer/components/settings/LegacyBridgeCard.module.css`

## JSX-структура (кратко, вложенность)
```
null при !fp?.found, иначе:
div.card
├─ div.icon — i.fas.fa-box-archive
├─ div.body
│  ├─ h2.title — «Legacy Electron Bridge detected»
│  └─ p.desc — «Found: {installed app | folder "Open with" menu entry | saved config}. KaminIDE has already imported …»
└─ button.remove (disabled при busy) — «Remove old Bridge» | «Removing…»
```
Клик Remove → ConfirmModal (danger) → reimportSessions → `uninstall_electron_bridge` → toast → re-detect (карточка исчезает).

## Метрики (ИЗ CSS, точные значения)
- `.card`: flex, `align-items:flex-start`; `gap:var(--space-3)`; `padding:var(--space-3)`; background `var(--bg-surface)`; border `1px solid var(--divider-soft)`; `border-radius:var(--radius-md)`
- `.icon`: `flex:none`; `display:grid; place-items:center`; 32×32px; `border-radius:var(--radius-sm)`; color `var(--accent-primary)`; `font-size:16px`
- `.body`: `flex:1; min-width:0`
- `.title`: `margin:0`; `font-size:13px`; `font-weight:600`; color `var(--text-primary)`
- `.desc`: margin `var(--space-1) 0 0`; `font-size:12px`; `line-height:1.5`; color `var(--text-muted)`
- `.remove`: `flex:none; align-self:center`; padding `var(--space-1) var(--space-3)`; border `1px solid var(--accent-red)`; `border-radius:var(--radius-sm)`; background transparent; color `var(--accent-red)`; `font-size:12px`; `font-weight:600`; `transition: background 0.12s ease, color 0.12s ease`
  - hover (не disabled): background `var(--accent-red)`; color `#fff`
  - disabled: `opacity:0.6; cursor:default`

## Состояния (классы-варианты с метриками)
- Не найден footprint → компонент возвращает `null`
- `busy` → кнопка disabled, текст «Removing…»
- hover `.remove` — инверсия (красная заливка, белый текст)

### Наша реализация

# 83 legacy-bridge-card — наша реализация
Файлы: `crates/shell/src/ui/customize.rs:238-343` (`legacy_bridge_card`), `crates/shell/src/legacy_bridge.rs` (детект footprint + `uninstall_electron_bridge`), `root.rs` (`ModalAction::RemoveLegacyBridge`)

## Структура (gpui-дерево кратко)
```
None при !fp.found, иначе:
div .card  (flex, items_start, gap 12, p 12, r 12, bg-surface, border divider-soft)
├─ div .icon 32×32, r 8, accent-primary, fa-box-archive 16
├─ div .body (flex_1, min_w 0)
│   ├─ «Legacy Electron Bridge detected» — fs 13 / 600 / text-primary
│   └─ desc — mt 4, fs 12, lh 1.5, text-muted; перечисление найденного
└─ обёртка h_full items_center → кнопка «Remove old Bridge»
```
Клик → ConfirmModal (danger, «Remove it») → `ModalAction::RemoveLegacyBridge` → реимпорт сессий → `uninstall_electron_bridge` → re-detect.

## Метрики (из кода, точные)
- `.card`: gap SPACE_3 12, padding 12, radius RADIUS_MD 12, bg `--bg-surface`, рамка 1px divider-soft (text-primary 6%).
- `.icon`: 32×32, radius RADIUS_SM 8, цвет accent-primary, глиф FontAwesome 16.
- `.title`: fs FS_MD 13, weight 600, text-primary. `.desc`: mt SPACE_1 4, fs FS_SM 12, line-height 1.5, text-muted.
- `.remove`: px SPACE_3 12 / py SPACE_1 4, radius 8, рамка 1px accent-red, текст accent-red fs 12 / 600; hover — заливка accent-red + белый текст. `align-self: center` сделан обёрткой (у `Stateful` нет `self_center()`).

## Отличия от original.md той же папки
1. Нет состояния `busy`: кнопка не блокируется и не меняет текст на «Removing…» (opacity .6) на время удаления — удаление уходит в поток без флага в состоянии.
2. Нет CSS-перехода 0.12s (в gpui нет transition).

### Вердикты

# 83 — verdict (review cycle 1)
VERDICT: DIVERGES
Не реализовано: LegacyBridgeCard целиком (детект + .card bg-surface/divider-soft
/r12, icon 32 accent, remove-кнопка red c hover-инверсией, ConfirmModal(danger)
→ reimport → uninstall → re-detect).

## Цикл 5: DIVERGES

Карточка «Legacy Electron Bridge detected» не реализована целиком (grep `legacy|uninstall_electron|box-archive` = 0). Оригинал: `.card` bg-surface + divider-soft + r12 + p12, `.icon` 32×32 accent 16, title 13/600, desc 12/1.5, кнопка `.remove` (4/12, border accent-red, ховер — красная заливка + #fff), busy → «Removing…», ConfirmModal(danger) → reimport → uninstall → re-detect.

## Цикл 6: DIVERGES

Карточка Legacy Bridge не реализована (у оригинала — первый блок Settings).


## Цикл 7: DIVERGES

Вердикт «не реализовано» устарел: карточка есть (`customize.rs:238-343`) и метрики
совпадают — card 12/12/r12/bg-surface/divider-soft, icon 32×32 r8 accent + fa-box-archive 16,
title 13/600, desc mt4 12/1.5 muted, кнопка 4×12 r8 рамка accent-red 12/600 с инверсией по
ховеру, подтверждение danger-модалкой перед удалением.

Осталось: состояние `busy` (кнопка disabled, «Removing…», opacity .6); перехода 0.12s нет
(ограничение gpui).

---

## 84. extensions-panel — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](84-extensions-panel/original.png)
![наш](84-extensions-panel/ours.png)

### Оригинал

# 84 extensions-panel — оригинал
Файлы: `src/renderer/components/extensions/ExtensionsPanel.tsx` (79-110 — `ExtensionsPanel`), `src/renderer/components/extensions/ExtensionsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ header.header
│  ├─ span — «Extensions»
│  └─ button.installBtn [data-tooltip="Install from a .vsix archive"] — codicon-cloud-download + «Install»
└─ div.list
   ├─ list.length===0 → p.empty — «No extensions installed.»
   ├─ sideloaded>0 → div.groupHeader «Installed — {N}» + Row×N
   └─ builtin>0   → div.groupHeader «Built-in — {N}» + Row×N   (Row — элемент 85)
```
Сортировка по displayName; иконки — кэш localStorage + host fetch.

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `height:100%; min-height:0`
- `.header`: flex, `align-items:center; justify-content:space-between`; `gap:var(--space-2)`; padding `var(--space-1) var(--space-2) var(--space-1) var(--space-3)`; font-size `var(--fs-xs)`; `text-transform:uppercase`; `letter-spacing:0.04em`; color `var(--text-muted)`; `flex-shrink:0`
- `.installBtn`: `inline-flex; align-items:center; gap:4px`; padding `3px 8px`; font-size `var(--fs-xs)`; `text-transform:none; letter-spacing:0`; `border-radius:var(--radius-sm)`; border `1px solid color-mix(in srgb, var(--accent-primary) 40%, transparent)`; background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--text-primary)`
  - hover: background `color-mix(in srgb, var(--accent-primary) 26%, transparent)`
  - `.installBtn .codicon`: `font-size:12px`
- `.list`: `flex:1; min-height:0; overflow:auto`; padding `0 var(--space-2) var(--space-2)`
- `.empty`: `padding:var(--space-3)`; color `var(--text-muted)`; font-size `var(--fs-sm)`
- `.groupHeader`: padding `var(--space-2) var(--space-2) 4px`; font-size `var(--fs-xs)`; `font-weight:600`; `text-transform:uppercase`; `letter-spacing:0.04em`; color `var(--text-muted)`
- transition — нет (кроме элементов строки, см. 85)

## Состояния (классы-варианты с метриками)
- empty (0 расширений), группы условные (sideloaded/builtin)
- hover `.installBtn` — усиление акцентной заливки 14%→26%

### Наша реализация

# 84 extensions-panel — наша реализация
Файлы: `crates/shell/src/ui/extensions_panel.rs:197-260` (extensions_panel), `extensions_panel.rs:168-194` (group), `extensions_panel.rs:17-62` (ExtDesc: разбор kamin:extensions:list)

## Структура (gpui-дерево кратко)
```
exts == None → центр «Loading…» text_muted
иначе:
div#extensions-panel (flex-col, size_full, min_h 0, overflow_y_scrollbar)
├─ install-ряд (flex, justify_end, pb 6)
│  └─ #ext-install-vsix: codicon-add(ea60) 12 + «Install from VSIX…»
│     px 10, py 3, rounded 6, gap 6, bg accent_primary 16% (#89b4fa@0.16), 12px, text_primary;
│     hover bg accent 26% → ShellEvent::InstallVsixPrompt
├─ group «Installed» (sideloaded: !builtin)
└─ group «Built-in»

group: div flex-col, mb 16
├─ заголовок — mb 4, FS_SM(12), Semibold, text_secondary #adb3c7
├─ пусто → «None» FS_XS text_muted
└─ ext_row × N (см. 85)
```

## Метрики (из кода, точные)
- Install-кнопка: px 10 / py 3 / rounded 6 / gap 6 / 12px; bg #89b4fa@0.16 → hover @0.26.
- groupHeader: 12px Semibold #adb3c7, mb 4; группа mb 16.

## Отличия от original.md той же папки
1. Нет хедера панели «Extensions» (uppercase FS_XS) — титул даёт CustomizePanel-обёртка; Install-кнопка вынесена в отдельный правый ряд.
2. Кнопка: «Install from VSIX…» + codicon-add; у оригинала «Install» + codicon-cloud-download, есть `border 1px accent 40%`, radius-sm 8 (у нас 6, без бордера).
3. groupHeader без счётчика «— N», без uppercase/letter-spacing; 12px Semibold vs 11px/600 uppercase.
4. Empty: per-group «None» вместо единого «No extensions installed.»; добавлено состояние «Loading…» (в оригинале нет).
5. Нет сортировки по displayName и кэша иконок (иконок нет вообще, см. 85).
6. Паддинги списка свои (нет `0 8 8` у `.list`), скролл на всей панели.

## Дополнение атрибутов (цикл 10)

- шрифты: хедер «EXTENSIONS» fs-xs 11 (`extensions_panel.rs:236`, `metrics/lib.rs:42`); кнопка Install наследует те же 11 + глиф codicon 12 (`extensions_panel.rs:262`); заголовок группы fs-xs 11 + weight 600 SEMIBOLD (`extensions_panel.rs:191-192`); пустой список fs-sm 12 (`extensions_panel.rs:280`); статус загрузки fs-sm 12 (`extensions_panel.rs:214`). Оригинал: `.header`/`.installBtn`/`.groupHeader` — `var(--fs-xs)`, `.groupHeader { font-weight: 600 }`, `.empty { var(--fs-sm) }`, `.installBtn .codicon { 12px }` (`ExtensionsPanel.module.css:14,26,53-54,48,36`) — 1:1; uppercase у нас делается строкой (`to_uppercase()`), letter-spacing .04em в gpui недоступен.

### Вердикты

# 84 — verdict (review cycle 1)
VERDICT: MATCH
Хедер/installBtn/list/empty/groupHeader/сортировка — 1:1 (extensions_panel.rs).
Прим.: ls .04em нет в gpui; реальные data-URL иконки — фаза расширений.

## Цикл 5: MATCH

Extensions-панель 1:1: хедер 4/8/4/12 fs-xs uppercase muted, installBtn 3/8 gap4 border accent 40% bg 14%/hover 26% codicon 12, `.list` 0/8/8 + скролл, `.empty` p12 fs-sm, groupHeader 8/8/4 «TITLE — N», сортировка по displayName.

## Цикл 6: MATCH

Extensions-панель 1:1.

---

## 85. extension-row — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](85-extension-row/original.png)
![наш](85-extension-row/ours.png)

### Оригинал

# 85 extension-row — оригинал
Файлы: `src/renderer/components/extensions/ExtensionsPanel.tsx` (56-77 — `Row`), `src/renderer/components/extensions/ExtensionsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.row(.disabled при !e.enabled)
├─ icon ? img.icon (data-URL) : i.codicon.codicon-extensions.iconFallback
├─ div.meta
│  ├─ span.name [data-tooltip=e.id] — displayName
│  └─ span.sub — «{version} · {active|idle|disabled|activation error}»
└─ div.rowActions
   ├─ button.toggle — «Disable» | «Enable»
   └─ !builtin → button.uninstall [data-tooltip="Uninstall", aria-label] — codicon-trash
```

## Метрики (ИЗ CSS, точные значения)
- `.row`: flex, `align-items:center`; `gap:var(--space-2)`; `padding:var(--space-2)`; `border-radius:var(--radius-sm)`
  - hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- `.disabled`: `opacity:0.55`
- `.icon`: 26×26px; `flex-shrink:0`; `border-radius:var(--radius-xs)`; `object-fit:contain`
- `.iconFallback`: 26×26px; `flex-shrink:0`; `display:grid; place-items:center`; `font-size:16px`; color `var(--text-muted)`
- `.meta`: `flex:1; min-width:0`; flex column
- `.name`: font-size `var(--fs-sm)`; color `var(--text-primary)`; `overflow:hidden; text-overflow:ellipsis; white-space:nowrap`
- `.sub`: font-size `var(--fs-xs)`; color `var(--text-muted)`
- `.rowActions`: flex, `align-items:center`; `gap:4px`; `flex-shrink:0`
- `.toggle`: `flex-shrink:0`; padding `2px 10px`; font-size `var(--fs-xs)`; `border-radius:var(--radius-sm)`; border `1px solid color-mix(in srgb, var(--text-muted) 30%, transparent)`; background `var(--bg-surface)`; color `var(--text-primary)`
  - hover: background `var(--bg-overlay)`
- `.uninstall`: `display:grid; place-items:center`; 24×22px; border none; `border-radius:var(--radius-sm)`; background transparent; color `var(--text-muted)`
  - hover: background `color-mix(in srgb, var(--accent-red) 16%, transparent)`; color `var(--accent-red)`
- transition — нет

## Состояния (классы-варианты с метриками)
- `.disabled` (opacity 0.55) — выключенное расширение
- Статус-текст в `.sub`: disabled / activation error / active / idle
- uninstall-кнопка только у sideloaded (`!builtin`)
- hover-эффекты: строка (surface 60%), toggle (bg-overlay), uninstall (red 16% + red-текст)

### Наша реализация

# 85 extension-row — наша реализация
Файлы: `crates/shell/src/ui/extensions_panel.rs:110-166` (ext_row), `extensions_panel.rs:76-108` (toggle-пилюля), `extensions_panel.rs:51-61` (status)

## Структура (gpui-дерево кратко)
```
div (flex, items_center, gap 12, p 8, rounded 8; hover bg text_primary 4%)
├─ div.meta (flex-col, flex_1, min_w 0)
│  ├─ ряд baseline gap 8: displayName FS_SM(12) text_primary + «v{version}» FS_XS(11) text_muted
│  └─ id — FS_XS, text_muted
├─ статус-текст FS_XS: active→accent_blue #89b4fa / idle,disabled→text_muted #838aa0 / activation error→accent_red #f38ba8
└─ toggle #extt-{id}: пилюля 34×18, rounded 9, px 2;
   on → bg accent_primary #89b4fa, бегунок справа; off → bg bg_overlay #515567, слева;
   бегунок 14×14 rounded_full white → ShellEvent::ToggleExtension
```

## Метрики (из кода, точные)
- Строка: gap 12, p 8, rounded 8, hover 4% text_primary.
- Toggle: 34×18 / трек-радиус 9 / бегунок 14×14 белый.
- Статусы: disabled / activation error / active / idle (логика 1:1).

## Отличия от original.md той же папки
1. **Нет иконки расширения** (img 26×26 / codicon-extensions fallback) — строка начинается с текста.
2. **Нет кнопки uninstall** (codicon-trash, red-hover) — удаление sideloaded недоступно из UI.
3. Enable/Disable — switch-пилюля 34×18 вместо текстовой кнопки «Enable/Disable» (padding 2 10, border, bg-surface).
4. Статус — отдельный цветной текст справа; у оригинала он в `.sub` второй строкой «{version} · {status}» без цвета. Версия у нас в первой строке рядом с именем; вторая строка — id (у оригинала id только в tooltip имени).
5. Выключенное расширение не приглушается (`opacity:0.55` у оригинала нет).
6. hover строки 4% text_primary вместо `bg-surface 60%`.
7. Нет ellipsis на имени (переполнение не защищено), нет data-tooltip.

## Дополнение атрибутов (цикл 10)

- скругления: строка radius-sm 8 (`extensions_panel.rs:104`), кнопка Enable/Disable radius-sm 8 (`extensions_panel.rs:82`), кнопка uninstall 24×22 radius-sm 8 (`extensions_panel.rs:161`); у иконки-фоллбэка скругления нет (`extensions_panel.rs:108-116`). Оригинал: `.row`/`.toggle`/`.uninstall` — `var(--radius-sm)` (`ExtensionsPanel.module.css:65,107,124`), а `.icon` (реальная картинка) — `var(--radius-xs)` 4 (`:68`); у нас реальных иконок расширений пока нет, поэтому radius-xs не задействован.
- шрифты: имя fs-sm 12 (`extensions_panel.rs:127`), подпись «version · status» fs-xs 11 (`extensions_panel.rs:137`), тумблер fs-xs 11 (`extensions_panel.rs:86`), глиф-фоллбэк codicon 16 (`extensions_panel.rs:116`), корзина codicon 12 (`extensions_panel.rs:174`). Оригинал: `.name { var(--fs-sm) }`, `.sub`/`.toggle { var(--fs-xs) }`, `.iconFallback { font-size: 16px }` (`ExtensionsPanel.module.css:91,99,106,75`) — 1:1.

### Вердикты

# 85 — verdict (review cycle 1)
VERDICT: MATCH
row/disabled .55/iconFallback 26/meta/name+tooltip/sub/toggle/uninstall — 1:1.
Прим.: trash 12px vs наследуемые 13 — субпиксель.

## Цикл 5: MATCH

Строка расширения 1:1 (row gap8 p8 r8 hover 60%, disabled 0.55, iconFallback 26 codicon 16, name fs-sm ellipsis + tooltip id, sub `{version} · {status}`, toggle 2/10, uninstall 24×22 hover red 16%). Остаток: реальные иконки расширений из хоста не тянутся (всегда fallback), сортировка байтовая вместо `localeCompare`.

## Цикл 6: MATCH

Строка расширения 1:1 (иконки хоста и localeCompare — остаток).

---

## 86. problems-panel — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](86-problems-panel/original.png)
![наш](86-problems-panel/ours.png)

### Оригинал

# 86 problems-panel — оригинал
Файлы: `src/renderer/components/problems/ProblemsPanel.tsx` (44-102), `src/renderer/components/problems/ProblemsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ header.header
│  ├─ span — «Problems»
│  └─ span.counts
│     ├─ button.countBtn(.countActive) [data-tooltip="Filter errors", disabled при 0]
│     │  └─ i.codicon.codicon-error(.errIcon при >0) + {counts.errors}
│     └─ button.countBtn(.countActive) [data-tooltip="Filter warnings", disabled при 0]
│        └─ i.codicon.codicon-warning(.warnIcon при >0) + {counts.warnings}
└─ div.list
   ├─ 0 файлов → p.empty — «No problems have been detected in the workspace.»
   ├─ файлы (cap 100, step 200) → div.group
   │  ├─ button.fileRow (toggle collapse)
   │  │  ├─ i.codicon.codicon-chevron-{right|down}.chevron
   │  │  ├─ <TreeIcon.fileIcon name type=file>
   │  │  ├─ span.fileName — basename
   │  │  ├─ span.fileDir [data-tooltip=uri] — dirname
   │  │  └─ span.fileCount — diagnostics.length
   │  ├─ !collapsed → ProblemRow×N (cap 200/файл; элемент 87)
   │  └─ >200 → div.fileDir style={padding:"2px 0 2px 28px"} — «… N more problems in this file»
   └─ hiddenFiles>0 → button.showMore — codicon-ellipsis + «Show N more files (M hidden)»
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `height:100%; min-height:0`
- `.header`: flex, `align-items:center; justify-content:space-between`; padding `8px 8px 8px 12px`; font-size `var(--fs-xs)`; `font-weight:500`; `text-transform:uppercase`; `letter-spacing:0.08em`; `font-feature-settings:"ss01"`; color `var(--text-muted)`; `flex-shrink:0` (совпадает с FileTreeHeader)
- `.counts`: `inline-flex; gap:4px`; `text-transform:none; letter-spacing:0`
- `.countBtn`: `inline-flex; align-items:center; gap:3px`; padding `1px 6px`; border `1px solid transparent`; `border-radius:9px`; background transparent; color `var(--text-muted)`; `font:inherit`; font-size `var(--fs-xs)`
  - hover (не disabled): background `color-mix(in srgb, var(--bg-surface) 70%, transparent)`
  - disabled: `cursor:default; opacity:0.8`
  - `.countActive`: background `color-mix(in srgb, var(--accent-primary) 18%, transparent)`; border-color `color-mix(in srgb, var(--accent-primary) 40%, transparent)`; color `var(--text-primary)`
  - `.countBtn .codicon`: `font-size:12px`
- `.errIcon`: color `var(--accent-red)`; `.warnIcon`: color `var(--accent-yellow)` (окрашены только при count>0)
- `.list`: `flex:1; min-height:0; overflow:auto`; padding `0 0 var(--space-2)`; font-size `var(--fs-sm)`
- `.empty`: `height:100%`; flex column, центрирование; `text-align:center`; `padding:var(--space-5)`; `margin:0`; color `var(--text-muted)`; font-size `var(--fs-sm)`
- `.group`: flex column
- `.fileRow`: flex, `align-items:center`; `gap:6px`; `width:100%; height:24px`; padding `0 var(--space-2)`; background transparent; border none; color `var(--text-secondary)`; `text-align:left`; `white-space:nowrap; overflow:hidden`; `font:inherit`; font-size `var(--fs-sm)`
  - hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- `.chevron`: `flex-shrink:0`; `font-size:13px`; `width:16px`; `text-align:center`; color `var(--text-muted)`
- `.fileIcon`: `flex-shrink:0`; 16×16px
- `.fileName`: color `var(--text-primary)`; `flex-shrink:0`
- `.fileDir`: `flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- `.fileCount`: `flex-shrink:0`; `min-width:16px; height:16px`; padding `0 5px`; `inline-flex`, центр; `border-radius:9px`; background `var(--bg-surface)`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- `.showMore`: flex, `align-items:center; gap:6px`; `width:100%`; border none; background none; `font:inherit`; font-size `var(--fs-xs)`; color `var(--text-muted)`; padding `6px 10px`; `text-align:left`
  - hover: color `var(--text-primary)`; background `color-mix(in srgb, var(--bg-surface) 55%, transparent)`
- transition — нет

## Состояния (классы-варианты с метриками)
- `.countActive` — активный severity-фильтр (accent 18% + бордер 40%)
- `.countBtn:disabled` — при нулевом счётчике (opacity 0.8)
- collapse per file (chevron right/down), caps: 100 файлов (+200 по showMore), 200 строк/файл

### Наша реализация

# 86 problems-panel — наша реализация
Файлы: `crates/shell/src/ui/problems.rs:62-179` (problems_panel), `problems.rs:19-46` (Diag из kamin:diag:*), `crates/shell/src/root.rs:3346` (подключение как тело тула "problems")

## Структура (gpui-дерево кратко)
```
diags пусто → центр: codicon-check(eba4) 15 accent_green #a6e3a1 + «No problems detected», gap 8, text_muted
иначе:
div#problems-body (flex-col, size_full, min_h 0, overflow_y_scrollbar, p 8, gap 2)
└─ на каждый uri (слияние владельцев по uri, сортировка ключей):
   ├─ заголовок файла: flex gap 8, px 8, pt 4
   │  ├─ base_name(uri) — FS_SM(12), Semibold, text_primary #cfd4e2
   │  └─ счётчик — FS_XS(11), text_muted (голый текст)
   └─ строки диагностик (sort по (severity, line)) — см. 87
```

## Метрики (из кода, точные)
- Контейнер: p 8, gap 2; хедер файла px 8 / pt 4.
- Сортировка: файлы по uri (unstable), внутри — severity, затем line.

## Отличия от original.md той же папки
1. **Нет хедера «Problems» со счётчиками-фильтрами** errors/warnings (countBtn, countActive, disabled) — фильтрации по severity нет вообще.
2. **Нет collapse по файлу** (chevron right/down) — группы всегда развёрнуты.
3. Хедер файла: нет TreeIcon 16×16, нет dirname (`.fileDir` ellipsis), счётчик — голый текст вместо пилюли (bg-surface, radius 9, min-width 16).
4. Нет капов: рендерятся все файлы и все строки (оригинал: 100 файлов + step 200, 200 строк/файл, кнопка «Show N more files»).
5. Empty-state: добавлена зелёная галка-иконка; текст короче («No problems detected» vs «No problems have been detected in the workspace.»).
6. Высота хедера файла не фиксирована (оригинал `.fileRow` height 24, hover bg-surface 60%; у нас хедер некликабелен и без hover).
7. Файл-хедер Semibold text_primary (оригинал: fileName text_primary обычный, строка целиком text_secondary).

## Дополнение атрибутов (цикл 10)

- шрифты: хедер «PROBLEMS» fs-xs 11 + weight 500 MEDIUM и `font-feature-settings: ss01` (`problems.rs:184-185`, `ui/typo.rs` `ss01()`); счётчики fs-xs 11 с глифом codicon 12 (`problems.rs:115,117`); список fs-sm 12 (`problems.rs:241`); пустое состояние fs-sm 12 (`problems.rs:251`); пилюля-счётчик файла fs-xs 11 (`problems.rs:334`). Оригинал: `.header { var(--fs-xs); font-weight: 500; font-feature-settings: "ss01" }`, `.countBtn { var(--fs-xs) }`, `.countBtn .codicon { 12px }`, `.list`/`.empty { var(--fs-sm) }`, `.fileCount { var(--fs-xs) }` (`ProblemsPanel.module.css:14-18,41,49,59,72,130`) — 1:1; letter-spacing .08em не переносится (нет в gpui).

### Вердикты

# 86 — verdict (review cycle 1)
VERDICT: DIVERGES (крупно)
Warning=accent-yellow подтверждён. Нет хедера Problems + .counts/.countBtn
(countActive accent18%+border40%); нет collapse по файлу (chevron 13/w16); нет
TreeIcon 16 и .fileDir; fileRow не h24/gap6/hover surface60%, имя SEMIBOLD vs
normal; счётчик не пилюля min-w16 h16 r9; нет .showMore и капов 100/200;
empty-state другой; лишний p8 у списка.

## Цикл 5: MATCH

Problems-панель 1:1 (хедер 8/8/8/12, countBtn 1/6 r9 + border-резерв, active accent 18%/40%, `.list` pb8 + скролл, fileRow h24 gap6 px8 + chevron 13 в боксе 16 + TreeIcon 16 + dirname ellipsis + пилюля min-w16 h16 px5 r9, капы 100/200 + «Show N more files (M hidden)»). Остаток: у хедера не применён `ss01` (хелпер есть); мы пересортировываем файлы по uri и диагностики по (severity, line), оригинал сохраняет порядок хоста.

## Цикл 6: MATCH

Problems: `ss01` применён; пересортировка по uri/(severity,line) — остаток.

---

## 87. problem-row — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](87-problem-row/original.png)
![наш](87-problem-row/ours.png)

### Оригинал

# 87 problem-row — оригинал
Файлы: `src/renderer/components/problems/ProblemRow.tsx` (26-41), `src/renderer/components/problems/ProblemsPanel.module.css` (секция «Diagnostic row», строки 133-177)

## JSX-структура (кратко, вложенность)
```
button.row (onClick → openFileAt(uri, diag.range))
├─ i.codicon.codicon-{error|warning|info|lightbulb}.sevIcon.{sevError|sevWarning|sevInfo|sevHint}
├─ span.message [data-tooltip=diag.message] — diag.message
├─ origin && span.origin — «source(code)» | «source» | «code»
└─ span.location — «[Ln {startLine+1}, Col {startChar+1}]»
```
Severity map: 0→error/sevError, 1→warning/sevWarning, 2→info/sevInfo, 3→lightbulb/sevHint; неизвестное → error.

## Метрики (ИЗ CSS, точные значения)
- `.row`: flex, `align-items:center`; `gap:6px`; `width:100%; min-height:22px`; padding `0 var(--space-2) 0 26px` (левый отступ 26px — индент под иконку файла); background transparent; border none; color `var(--text-secondary)`; `text-align:left`; `white-space:nowrap; overflow:hidden`; `font:inherit`; font-size `var(--fs-sm)`; `cursor:pointer`
  - hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`; color `var(--text-primary)`
- `.sevIcon`: `flex-shrink:0`; `font-size:14px`
- `.sevError`: color `var(--accent-red)`; `.sevWarning`: `var(--accent-yellow)`; `.sevInfo`: `var(--accent-blue)`; `.sevHint`: `var(--text-muted)`
- `.message`: `flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis`
- `.origin`: `flex-shrink:0`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- `.location`: `flex-shrink:0`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- transition — нет

## Состояния (классы-варианты с метриками)
- 4 severity-класса иконки (цвета выше)
- `.origin` — условный (только при source/code)
- hover — подсветка строки + осветление текста

### Наша реализация

# 87 problem-row — наша реализация
Файлы: `crates/shell/src/ui/problems.rs:123-176` (строка диагностики внутри problems_panel)

## Структура (gpui-дерево кратко)
```
div#prob-{uri}-{i} (flex, items_center, gap 8, pl 16, pr 8, py 3, rounded 8, cursor_pointer)
├─ codicon severity 13px:
│    0 error ea87 accent_red #f38ba8 · 1 warning ea6c accent_primary #89b4fa
│    · 2 info ea74 accent_blue #89b4fa · 3 hint ea76 text_muted #838aa0
├─ message — flex_1, min_w 0, ellipsis, nowrap, FS_SM(12), text_secondary #adb3c7
├─ source (если непустой) — FS_XS(11), text_muted
└─ «:{line+1}» — FS_XS, text_muted
hover: bg text_primary 6%
click → ShellEvent::OpenFileAt(uri, line+1)
```

## Метрики (из кода, точные)
- pl 16 / pr 8 / py 3 / gap 8 / rounded 8; иконка 13px; message 12px; source/location 11px.

## Отличия от original.md той же папки
1. **Warning-иконка = accent_primary #89b4fa вместо `--accent-yellow`** #f9e2af — цветовое расхождение.
2. Hint-глиф: `ea76` (у оригинала codicon-lightbulb); в коде помечен как «circle».
3. Локация: «:{line}» — без колонки и без формата «[Ln x, Col y]».
4. origin — только `source`; вариант «source(code)» / «code» не поддержан (code не парсится из DTO).
5. Левый индент 16 (оригинал 26px — под иконку файла); иконка severity 13px (оригинал 14).
6. hover: только заливка 6% (оригинал bg-surface 60% + осветление текста до text_primary).
7. rounded 8 у строки (оригинал без радиуса); нет min-height 22 (py 3 ≈ сопоставимо); нет data-tooltip на message.

## Дополнение атрибутов (цикл 10)

- шрифты: строка диагностики кегля не задаёт — наследует fs-sm 12 от списка (`problems.rs:241`, строка `:356-403`); иконка severity codicon 14 (`problems.rs:377`); origin и «[Ln, Col]» fs-xs 11 (`problems.rs:392,400`); строка «… N more problems in this file» fs-xs 11 (`problems.rs:411`); «Show N more files» fs-xs 11 + глиф codicon 12 (`problems.rs:431,442`). Оригинал: `.row { font: inherit; font-size: var(--fs-sm) }`, `.sevIcon { 14px }`, `.origin`/`.location { var(--fs-xs) }`, `.showMore { var(--fs-xs) }` (`ProblemsPanel.module.css:148-154,170,176,188`) — 1:1.

### Вердикты

# 87 — verdict (review cycle 1)
VERDICT: DIVERGES
pl16 vs 26; py3+rounded vs min-h22 без radius; hover 6% tint vs bg-surface60%
+text-primary; gap8 vs 6; sevIcon 13 vs 14; hint-глиф не lightbulb; «:N» vs
«[Ln N, Col M]» (character не парсится); origin без code; нет tooltip message.

## Цикл 5: MATCH

Строка проблемы 1:1: min-h22, pl26/pr8, gap6, sevIcon 14 (red/yellow/blue/lightbulb-muted), message flex1 ellipsis + tooltip, origin `source(code)`, `[Ln N, Col M]` fs-xs muted, hover 60% + primary.

## Цикл 6: MATCH

Строка проблемы 1:1.

---

## 88. terminal-view — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](88-terminal-view/original.png)
![наш](88-terminal-view/ours.png)

### Оригинал

# 88 terminal-view — оригинал
Файлы: `src/renderer/components/terminal/TerminalView.tsx` (54-77), `src/renderer/components/terminal/TerminalView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ <TerminalToolbar shells defaultShellId sessions onOpen onSetDefault onClose onSelect />  (элемент 89)
└─ div.body
   ├─ sessions.map → <TerminalSession ptyId active />   (элемент 91; все смонтированы, неактивные display:none)
   └─ sessions.length===0 → div.empty
      ├─ i.codicon.codicon-terminal
      └─ p — «No terminal yet — pick a shell from the “+” menu.»
```
Состояние per-slot (`getTerminalSessions(slot)`/`getActiveTerminal(slot)`); авто-открытие первой сессии один раз на слот (guard hasAutoOpenedRef + openInFlight).

## Метрики (ИЗ CSS, точные значения)
- `.root`: `flex:1`; flex column; `min-height:0`; margin `0 6px 6px` (top 0 — тулбар прижат к activity tab strip; лево/право/низ 6px — гуттеры карточки); background `var(--bg-mantle)`; `border-radius:var(--radius-md)`; `overflow:hidden`
- `.body`: `flex:1; position:relative; min-height:0`; background `var(--editor-bg)` (та же поверхность, что редактор; xterm красит то же значение); `border-radius:var(--radius-md)`; `overflow:hidden`
- `.empty`: `position:absolute; inset:0`; flex column, центрирование; `gap:var(--space-2)`; color `var(--text-muted)`
  - `.empty .codicon`: `font-size:28px; opacity:0.6`
  - `.empty p`: `margin:0`; font-size `var(--fs-sm)`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- empty-state при 0 сессий (absolute поверх `.body`)
- Тулбар на mantle-фоне; активный таб + body образуют единую editor-bg поверхность (см. 89)

### Наша реализация

# 88 terminal-view — наша реализация
Файлы: `crates/shell/src/root.rs:3106-3344` (ветка "terminal" в tool_body), `root.rs:117-118` (TERM_CELL_W=7.2, TERM_CELL_H=16.0), `crates/shell/src/term.rs` (TermSession — alacritty_terminal поверх PTY)

## Структура (gpui-дерево кратко)
```
div#terminal-body (track_focus, key_context "Terminal", flex-col, size_full, min_h 0,
                   overflow_hidden, p 8, font «JetBrains Mono» 12px, line-height 16,
                   text_secondary #adb3c7)
├─ term_toolbar(...)   (см. 89)
└─ #terminal-grid (relative, flex-col, flex_1, min_h 0, overflow_hidden)
   ├─ probe_area("terminal") — bounds для resize (cols = w/7.2, rows = h/16)
   ├─ строки грида: div flex h 16, раны с fg-цветом; выделение bg accent_primary 30%
   └─ block-курсор absolute: ячейка 7.2×16, bg accent_primary #89b4fa, символ цветом bg_primary #313240
терминалов нет → центр «Starting shell…» text_muted
```
Ввод: keystroke→PTY, Ctrl+C с выделением = copy (не SIGINT), Ctrl+V = paste (CRLF→CR); scroll-wheel = скроллбэк ×3 строки; drop файла/узла дерева → путь (с кавычками при пробеле) в PTY.

## Метрики (из кода, точные)
- Ячейка 7.2×16; шрифт моно 12px; паддинг тела 8 (SPACE_2).
- Selection: #89b4fa@0.3; курсор — инверсия accent/bg.

## Отличия от original.md той же папки
1. Рендер — собственный грид на alacritty_terminal, не xterm.js: нет `.root` (margin 0 6 6, bg mantle, radius 12) и нет `.body` с фоном `--editor-bg` — терминал рисуется прямо на фоне карты слота (bg_mantle #262533), отдельной editor-bg (#1d1c25) поверхности НЕТ.
2. Шрифт 12px (оригинал xterm fontSize 13).
3. Empty-state: «Starting shell…» без иконки; оригинал — codicon-terminal 28px + «No terminal yet — pick a shell from the “+” menu.» (и сессия у нас автосоздаётся, empty почти не виден).
4. Per-slot состояния нет: один список terminals на приложение (оригинал — сессии на слот).
5. Неактивные сессии не рендерятся вовсе (см. 91) — у оригинала все смонтированы с display:none.
6. Курсор — block accent_primary (оригинал xterm cursor `--editor-cursor` #a0a0d0, blink).
7. text_secondary #adb3c7 как базовый fg (оригинал `--editor-fg` #dcdce4).

## Дополнение атрибутов (цикл 10)

- гэпы: flex-`gap` у карты терминала нет — тулбар (min-h 30) и тело идут встык (`root.rs:3948-3980`), как `.root` оригинала (`TerminalView.module.css:11-23`, gap не объявлен). Внутренние gap принадлежат тулбару (элемент 89: `.bar` gap 4, `.tabs` gap 2 — `term_toolbar.rs:359,130`). Пустое состояние `.empty { gap: var(--space-2) 8 }` (`TerminalView.module.css:63`) не портировано: пустого состояния нет — при показе панели сессия спавнится автоматически (`root.rs:4503`).
- ховер: N/A: ховер — у карты терминала и у сетки ячеек hover-правил нет (`root.rs:3948-3980`, рендер рун `:3834-3860`), как и у `.root`/`.body`/`.session` оригинала; ховеры принадлежат табам и кнопкам тулбара (89/90).

### Вердикты

# 88 — verdict (review cycle 1)
VERDICT: DIVERGES
Ядро подтверждено (mantle-карта mx6 mb6, body editor-bg, инсеты 8/22/10/14,
mono13/lh17, cell 7.8×17, cursor editor-cursor).
Расхождения: ранний return «Starting shell…» БЕЗ карты и тулбара (оригинал:
.root+toolbar всегда, .empty absolute); нет .empty (terminal 28 op.6 + «No
terminal yet — pick a shell from the “+” menu.»); scrollback 10000 vs 5000.

## Цикл 5: DIVERGES

Терминал: `.root` (mx6/mb6, bg-mantle, r12) и `.body` (editor-bg, r12, инсеты 8/22/10/14), mono 13, ячейка 7.8×17, курсор `editor_cursor` — подтверждены. Расхождение: при нуле сессий ветка выходит РАНО → голое «Starting shell…» без карты и тулбара; оригинал всегда рисует `.root` + тулбар, а `.empty` (absolute inset 0, gap 8, codicon-terminal 28 op .6 + «No terminal yet — pick a shell from the “+” menu.») лежит ВНУТРИ `.body`. Сессии не per-slot.

## Цикл 6: DIVERGES

При нуле сессий ранний выход без карты и тулбара.

---

## 89. terminal-toolbar — **MATCH** (цикл 6)

*История: ц5:MATCH, ц6:MATCH*

![оригинал](89-terminal-toolbar/original.png)
![наш](89-terminal-toolbar/ours.png)

### Оригинал

# 89 terminal-toolbar — оригинал
Файлы: `src/renderer/components/terminal/TerminalToolbar.tsx` (151-216 — хедер; логика overflow 40-80), `src/renderer/components/terminal/TerminalToolbar.module.css`

## JSX-структура (кратко, вложенность)
```
header.bar
├─ overflow.enabled → button.scrollBtn [aria-label="Scroll tabs left", disabled=!canLeft] — codicon-chevron-left
├─ div.tabs (ref, скрытый скроллбар)
│  └─ button.tab(.tabActive)
│     ├─ i.codicon.codicon-terminal
│     ├─ span.tabLabel — s.label
│     └─ span.close [role=button, tabIndex=0, data-tooltip="Close"] — codicon-close
├─ overflow.enabled → button.scrollBtn [aria-label="Scroll tabs right", disabled=!canRight] — codicon-chevron-right
└─ div.anchor
   ├─ button.addBtn [aria-haspopup=menu, aria-expanded, data-tooltip="New terminal"] — codicon-add
   └─ portal-меню (элемент 90)
```
Скролл: page-step `max(32px, floor(clientWidth*0.8))`, `scrollTo({behavior:"smooth"})`; чевроны появляются только при переполнении (ResizeObserver + scroll).

## Метрики (ИЗ CSS, точные значения)
- `.bar`: flex, `align-items:flex-end`; `gap:var(--space-1)`; padding `0 25px`; `flex-shrink:0`; `min-height:30px`
- `.tabs`: flex, `align-items:flex-end`; `gap:2px`; `flex:1; min-width:0`; `overflow-x:auto`; `scrollbar-width:none`; `::-webkit-scrollbar{display:none}`
- `.scrollBtn`: 22×30px; `display:grid; place-items:center`; background transparent; border none; `border-radius:var(--radius-xs)`; color `var(--text-secondary)`; `flex-shrink:0`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover (не disabled): background `var(--bg-surface)`; color `var(--text-primary)`
  - disabled: `opacity:0.35; cursor:default`
  - codicon: `font-size:12px`
- `.tab`: `inline-flex; align-items:center; gap:6px`; padding `0 10px`; `height:30px`; background transparent; border none; `border-radius:8px 8px 0 0`; color `var(--text-secondary)`; `font-size:11px; font-weight:500; letter-spacing:0.02em`; `white-space:nowrap`; `flex:0 1 auto`; `min-width:80px; max-width:220px`; `position:relative`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - `.tab .codicon`: `font-size:12px; line-height:1`
  - hover: background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `var(--text-primary)`
- `.tabActive`, `.tabActive:hover`: background `var(--editor-bg)`; color `var(--text-primary)` — таб сливается с поверхностью консоли
  - `.tabActive::before/::after`: вогнутые уголки 6×6px, `position:absolute; bottom:0`; before: `left:-6px; background:radial-gradient(circle at 0 0, transparent 6px, var(--editor-bg) 6.5px)`; after: `right:-6px; radial-gradient(circle at 100% 0, …)`; `pointer-events:none`
- `.tabLabel`: `overflow:hidden; text-overflow:ellipsis; max-width:160px`
- `.close`: 16×16px; `inline-flex`, центр; `border-radius:var(--radius-xs)`; `color:inherit`; `opacity:0`
  - codicon: `font-size:11px`
  - `.tab:hover .close`, `.tabActive .close`: `opacity:0.7`
  - `.close:hover`: `opacity:1`; background `color-mix(in srgb, var(--bg-overlay) 60%, transparent)`
- `.anchor`: `position:relative; flex-shrink:0`
- `.addBtn`: 28×28px; `align-self:center`; `padding:0; margin:0`; `inline-flex`, центр; `line-height:1`; background transparent; border none; `border-radius:50%`; color `var(--text-secondary)`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover: background `var(--bg-surface)`; color `var(--text-primary)`
  - `[aria-expanded="true"]`: background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--accent-primary)`
  - codicon: `font-size:15px; width:14px; height:14px; line-height:1.1; display:block`

## Состояния (классы-варианты с метриками)
- `.tabActive` — editor-bg заливка + вогнутые уголки (inverted-radius, как Chrome/JetBrains)
- `.close` — скрыт (opacity 0), виден при hover таба/на активном (0.7), hover самой кнопки (1 + заливка)
- `.scrollBtn` — только при переполнении; disabled по краям
- `.addBtn[aria-expanded=true]` — акцентная подсветка открытого меню

### Наша реализация

# 89 terminal-toolbar — наша реализация
Файлы: `crates/shell/src/ui/term_toolbar.rs:64-289` (term_toolbar), `term_toolbar.rs:24` (TAB_W=112), `term_toolbar.rs:27-61` (scroll_btn)

## Структура (gpui-дерево кратко)
```
bar: div (flex, items_center, gap 4, flex_shrink_0, pb 4)
├─ overflow → scroll_btn left (chevron-left eab5) 18×22, rounded 3, 12px
├─ tabs: div (flex, gap 4, flex_1, overflow_hidden) — окно [first..last], visible = ⌊(panel_w−70)/112⌋
│  └─ таб #term-tab-{i}: div flex, gap 4, h 24, pl 8, pr 4, rounded 8, FS_SM(12), UI_FONT
│     ├─ codicon-terminal(ea85) 12
│     ├─ «{title} {i+1}»
│     └─ close #term-tabx-{i}: 16×16, rounded 3, codicon-close(ea76) 11, tooltip «Close»;
│          hover bg text_primary 12%
│     active: bg accent_primary 16% (#89b4fa@0.16) + text_primary; иначе hover bg 6%
├─ overflow → scroll_btn right (chevron-right eab6)
└─ add-якорь #term-add (relative)
   ├─ #term-add-btn: 22×22, rounded 8, codicon-add(ea60) 13, tooltip «New terminal»; hover bg 8%
   └─ menu_open → deferred(menu).priority(60)   (см. 90)
```

## Метрики (из кода, точные)
- Таб: h 24, pl 8/pr 4, rounded 8, gap 4; пилюля расчётной ширины TAB_W=112 (для пагинации).
- Шевроны 18×22 (disabled → opacity 0.3); «+» 22×22; bar pb 4, gap 4.
- Active-таб: #89b4fa@0.16.

## Отличия от original.md той же папки
1. **Активный таб — accent-заливка 16%, а не слив с editor-bg**; вогнутых уголков (radial-gradient ::before/::after 6×6) нет; радиус 8 со всех сторон вместо `8 8 0 0`.
2. Высота таба 24 (оригинал 30); нет min-width 80 / max-width 220 / ellipsis label (ширина по контенту).
3. Лейбл «{title} {N}» с порядковым номером (оригинал только s.label).
4. Close-кнопка видима всегда (оригинал opacity 0 → 0.7 на hover/active → 1).
5. «+» 22×22 rounded 8 (оригинал 28×28 круглая, aria-expanded → акцентная подсветка открытого меню — у нас нет).
6. Скролл табов: постраничное окно по индексу (шаг 1 таб, TAB_W-эвристика) вместо пиксельного smooth-scroll 80% ширины; шевроны 18×22 (оригинал 22×30).
7. Паддинг бара `pb 4` без `0 25px` боковых; выравнивание items_center (оригинал flex-end, табы «стоят» на нижней кромке).
8. Шрифт таба 12px (оригинал 11px/500/letter-spacing 0.02em).

## Дополнение атрибутов (цикл 10)

- скругления: таб скруглён только сверху — `rounded_tl(8)` + `rounded_tr(8)` (`term_toolbar.rs:150-151`) = `border-radius: 8px 8px 0 0` (`TerminalToolbar.module.css:50`); крестик закрытия radius-xs 4 (`term_toolbar.rs:193`) = `var(--radius-xs)` (`:112`); кнопка «+» `rounded_full()` (`term_toolbar.rs:222`) = `border-radius: 50%` (`:144`); шеврон прокрутки radius-xs 4 (`term_toolbar.rs:88`) = `var(--radius-xs)` (`:32`); вогнутые уголки активного таба — радиус 6 рисуется путём (`term_toolbar.rs:27`) вместо `::before/::after` c `radial-gradient` 6px (`:95-102`).
- шрифты: таб fs 11 + weight 500 MEDIUM, семейство UI (`term_toolbar.rs:152-154`) = `.tab { font-size: 11px; font-weight: 500; letter-spacing: .02em }` (`TerminalToolbar.module.css:52-54`; ls не переносится); глиф таба codicon 12 (`term_toolbar.rs:165`) = `.tab .codicon { 12px }` (`:69`); крестик codicon 11 (`term_toolbar.rs:203`) = `.close .codicon { 11px }` (`:117`); «+» codicon 15 (`term_toolbar.rs:233`) = `.addBtn .codicon { 15px }` (`:155`); шеврон codicon 12 (`term_toolbar.rs:90`) = `.scrollBtn .codicon { 12px }` (`:40`).

### Вердикты

# 89 — verdict (review cycle 1)
VERDICT: MATCH
bar/tabs/scrollBtn/tab/tabActive+вогнутые уголки/close-гейт/addBtn/label — 1:1.
Прим.: ls .02em нет; лейбл «{title} {i+1}» vs s.label; оконный overflow вместо
scroll+page-step.

## Цикл 5: MATCH

Тулбар терминала 1:1 (bar items-end gap4 px25 min-h30, tabs gap2 flex1, scrollBtn 22×30 disabled 0.35 codicon 12, tab h30 px10 gap6 r8-8-0-0 fs11/500 min-w80 max-w220, label max-w160 ellipsis, hover 50%, active = editor-bg + вогнутые уголки 6×6, close 16 opacity 0→0.7→1 + overlay 60%, addBtn 28 round + accent 14% при открытом меню). Остаток: лейбл `{title} {i+1}` вместо `shellLabel`; overflow оконный по индексу, а не пиксельный smooth-scroll 80%; `letter-spacing .02em` — ограничение.

## Цикл 6: MATCH

Тулбар терминала 1:1 (лейбл `{title} {i+1}` — остаток).

---

## 90. terminal-shell-menu — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

*кадр «оригинал» отсутствует*
![наш](90-terminal-shell-menu/ours.png)

### Оригинал

# 90 terminal-shell-menu — оригинал
Файлы: `src/renderer/components/terminal/TerminalToolbar.tsx` (112-149 — portal-меню; позиционирование 99-110), `src/renderer/components/terminal/TerminalToolbar.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
ul.menu [role=menu] style={left,top из clampToViewport(side:"bottom", offset:6px); visibility}
├─ shells.length===0 → li.menuEmpty — «No shells discovered»
└─ shells.map → li.menuRow
   ├─ button.menuItem [role=menuitem] (click → close + onOpen(id))
   │  ├─ i.codicon.codicon-{s.icon ?? "terminal"}.itemIcon
   │  ├─ span.itemLabel — s.label
   │  └─ isDefault → span.defaultTag — «default»
   └─ button.starBtn(.starOn) [aria-pressed, data-tooltip="Default shell"|"Set as default"]
      └─ i.codicon.codicon-star-{full|empty}
```
Закрытие: клик вне (mousedown capture) или Escape; POPUP_OFFSET_PX=6.

## Метрики (ИЗ CSS, точные значения)
- `.menu`: `position:fixed`; `z-index:var(--z-dropdown)`; `min-width:200px`; background `var(--bg-surface)`; border `1px solid var(--divider-soft)`; `border-radius:var(--radius-md)`; `box-shadow:var(--shadow-dropdown)`; `list-style:none; margin:0`; `padding:var(--space-1)`; flex column; `gap:1px`; `max-height:calc(100vh - 16px)`; `overflow-y:auto`
- `.menuEmpty`: padding `var(--space-2) var(--space-3)`; font-size `var(--fs-sm)`; color `var(--text-muted)`
- `.menuRow`: flex, `align-items:center`; `gap:2px`; `.menuRow .menuItem { flex:1 }`
- `.menuItem`: flex, `align-items:center`; `gap:var(--space-2)`; `width:100%`; padding `var(--space-2) var(--space-3)`; background transparent; border none; `border-radius:var(--radius-sm)`; color `var(--text-primary)`; `font:inherit`; font-size `var(--fs-sm)`; `text-align:left`
  - hover: background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.itemIcon`: `width:16px; text-align:center`; color `var(--text-muted)`
- `.itemLabel`: `flex:1; white-space:nowrap`
- `.defaultTag`: font-size `var(--fs-xs)`; color `var(--text-muted)`; `text-transform:uppercase`; `letter-spacing:0.04em`
- `.starBtn`: 24×24px; `inline-flex`, центр; `flex-shrink:0`; background transparent; border none; `border-radius:var(--radius-sm)`; color `var(--text-muted)`; `transition:background var(--transition-fast), color var(--transition-fast)`
  - hover: background `color-mix(in srgb, var(--text-primary) 10%, transparent)`; color `var(--text-primary)`
  - codicon: `font-size:12px`
- `.starOn`, `.starOn:hover`: color `var(--accent-primary)`

## Состояния (классы-варианты с метриками)
- `.starOn` — выбранный дефолтный шелл (акцентная звезда star-full; у остальных star-empty)
- `.menuEmpty` — 0 обнаруженных шеллов
- `visibility:hidden` до вычисления позиции (двухпроходное измерение)

### Наша реализация

# 90 terminal-shell-menu — наша реализация
Файлы: `crates/shell/src/ui/term_toolbar.rs:172-258` (дропдаун профилей внутри term_toolbar), `crates/shell/src/term.rs:55-98` (ShellProfile, profiles: PowerShell/cmd/+Git Bash если найден)

## Структура (gpui-дерево кратко)
```
gpui::deferred(menu).with_priority(60), occlude:
div (absolute, top 26, right 0, w 210, flex-col, p 4, rounded 12,
     bg bg_surface #3d3f51, border 1px text_primary 6%, font UI_FONT)
└─ на профиль #term-prof-{id}: div flex, gap 8, px 8, py 4, rounded 8, FS_SM(12), text_secondary;
   hover bg text_primary 8% + text_primary; click → TermNew(id)
   ├─ codicon-terminal(ea85) 12
   ├─ label (flex_1)
   ├─ isDefault → тег «default»: px 4, rounded 3, FS_XS(11), text accent_primary #89b4fa, bg accent 14%
   └─ star #term-star-{id}: 18×18, rounded 3, codicon eb59(star-full)/ea6a 12;
        цвет: default → accent_yellow #f9e2af, иначе text_muted; hover bg 12% + text_primary;
        tooltip «Default shell»/«Set as default»; click → TermSetDefaultShell (persist)
```

## Метрики (из кода, точные)
- Меню: w 210, top 26 от «+», p 4, rounded 12 (RADIUS_MD), bg #3d3f51.
- Пункт: px 8 / py 4 / rounded 8 / gap 8 / 12px; иконка 12; звезда 18×18.

## Отличия от original.md той же папки
1. Позиционирование: absolute под якорем (top 26, right 0) через `deferred`, без portal/clampToViewport/двухпроходного измерения; offset 26 вместо POPUP_OFFSET_PX 6 от низа кнопки.
2. Нет `box-shadow: var(--shadow-dropdown)`; бордер text_primary 6% (оригинал divider-soft = 14%).
3. Ширина 210 фикс (оригинал min-width 200 + рост по контенту); нет max-height/скролла.
4. «default»-тег — акцентная мини-плашка (bg accent 14%, текст accent) вместо muted-uppercase-текста.
5. Звезда: активная — accent_yellow #f9e2af (оригинал `.starOn` accent-primary); размер 18×18 (оригинал 24×24).
6. Пункт: py 4 (оригинал `8px 12px` — выше); hover 8% (оригинал 10%).
7. Иконка пункта всегда codicon-terminal (оригинал `s.icon ?? "terminal"` per-shell).
8. Нет `.menuEmpty` «No shells discovered» (profiles() всегда ≥2) и нет закрытия по Escape (закрытие — toggle кнопкой/occlude-кликом).

## Дополнение атрибутов (цикл 10)

- шрифты: пункт профиля fs-sm 12 (`term_toolbar.rs:281`, `metrics/lib.rs:43`) = `.menuItem { font-size: var(--fs-sm) }` (`TerminalToolbar.module.css:191`); иконка пункта codicon 12 в боксе 16 (`term_toolbar.rs:290-295`) = `.itemIcon { width: 16px }` + наследуемый кегль; тег «DEFAULT» fs-xs 11 (`term_toolbar.rs:302`) = `.defaultTag { var(--fs-xs) }` (`:220`); звезда codicon 12 (`term_toolbar.rs:345`) = `.starBtn .codicon { 12px }` (`:241`); семейство меню — UI-шрифт (`term_toolbar.rs:266`).

### Вердикты

# 90 — verdict (review cycle 1)
VERDICT: DIVERGES (мелочи)
Меню/menuItem/itemIcon/defaultTag/starBtn подтверждены 1:1.
Расхождения: shadow 0/6/24 vs --shadow-dropdown 0/4/16; нет .menuEmpty; absolute
вместо portal+clamp+max-h 100vh-16.

## Цикл 5: DIVERGES

Меню шеллов: min-w200, bg-surface, divider-soft, r12, dropdown-shadow, p4 gap1, пункт 8/12 r-sm fs-sm hover 10%, itemIcon w16 center muted codicon 12, defaultTag fs-xs uppercase, starBtn 24 hover 10%, `starOn` accent — 1:1. Остаток: `top(30)` без `POPUP_OFFSET_PX = 6` (меню вплотную к кнопке); нет `max-height: calc(100vh - 16px)` + скролла и клампа к вьюпорту; нет `.menuEmpty` «No shells discovered»; иконка пункта всегда terminal (оригинал `s.icon ?? "terminal"`); нет закрытия по Escape.

## Цикл 6: DIVERGES

Меню шеллов: `top(30)` без offset 6, нет max-height/скролла, `.menuEmpty`, `s.icon`, Escape.

---

## 91. terminal-session-host — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](91-terminal-session-host/original.png)
![наш](91-terminal-session-host/ours.png)

### Оригинал

# 91 terminal-session-host — оригинал
Файлы: `src/renderer/components/terminal/TerminalSession.tsx` (142-149 — JSX; 43-140 — жизненный цикл), `src/renderer/components/terminal/TerminalView.module.css` (`.session`)

## JSX-структура (кратко, вложенность)
```
div.session (ref=hostRef) [data-pty-id=ptyId] style={display: active ? "flex" : "none"}
└─ (внутрь xterm.open() монтирует свой DOM)
```
Один инстанс на `ptyId`; при неактивности `display:none` — буфер/курсор/скролл выживают. `data-pty-id` используется external-drop.ts для роутинга file-drop paste.

## Метрики (ИЗ CSS, точные значения)
- `.session`: `position:absolute`; `top:8px; right:22px; bottom:10px; left:14px`; flex column
  - right 22px шире прочих — место под scroll thumb xterm; flush-right курсор не налезает на скруглённый угол
  - абсолютные инсеты = гуттеры, чтобы FitAddon (clientWidth/Height) давал корректные cols/rows без клипа нижней строки
- Inline: `display:flex` (active) / `display:none` (неактивен)
- xterm-опции (из TSX): `cursorBlink:true`; fontFamily `var(--font-mono)` (fallback `monospace`); `fontSize:13`; `scrollback:5000`
- Тема xterm из токенов: background `--editor-bg` (fallback `#1d1c25`), foreground `--editor-fg` (`#dcdce4`), cursor `--editor-cursor` (`#a0a0d0`); перечитывается по rAF при смене `resolvedTheme`/`activeContributedThemeId`
- Клавиатура: Ctrl/Cmd+C при выделении — copy+clearSelection (не SIGINT); Ctrl+V не перехватывается (нативный paste)
- hover/active/focus css — нет; transition — нет

## Состояния (классы-варианты с метриками)
- active (`display:flex`) / скрыт (`display:none`, смонтирован)
- exit процесса: в буфер пишется `\r\n\x1b[2m[process exited with code N]\x1b[0m\r\n`
- ResizeObserver на хосте → `fit.fit()` + `pty.resize(cols, rows)`

### Наша реализация

# 91 terminal-session-host — наша реализация

НЕ РЕАЛИЗОВАНО как DOM-хост с `display:none`-персистентностью (xterm-инстансов нет).

Замена: буфер сессии живёт в данных, не в дереве.
Файлы: `crates/shell/src/term.rs:101-...` (TermSession: alacritty_terminal Term + PTY + Proxy), `crates/shell/src/root.rs:187` (`terminals: Vec<TermSession>`), `root.rs:3109-3130` (рендер ТОЛЬКО активной: `terminals.get(term_active)`, resize по probe-bounds), `root.rs:3199-3245` (screen_styled → раны + курсор)

## Структура (gpui-дерево кратко)
- `TermSession` держит alacritty-грид, скроллбэк, выделение и writer PTY; жив, пока таб не закрыт (`TermClose`), независимо от того, отображается ли.
- Каждый кадр рендерится грид только активной сессии (`screen_styled()` → строки-раны); переключение таба = рендер другого буфера, «скрытых DOM-хостов» нет по определению immediate-mode gpui.
- Resize: по bounds probe-области `"terminal"` (cols = w/7.2, rows = h/16) — аналог FitAddon.
- `Proxy` (term.rs:38-53): PtyWrite (DSR/CPR-ответы) → обратно в PTY (без них PSReadLine виснет), прочее → notify UI.

## Метрики (из кода, точные)
- Ячейка: TERM_CELL_W=7.2 × TERM_CELL_H=16 (root.rs:117-118); шрифт «JetBrains Mono» 12px.
- Цвета: fg по ранам из палитры терминала, базовый text_secondary #adb3c7; выделение accent_primary@0.3; курсор block bg #89b4fa / глиф #313240.
- Инсеты хоста: p 8 общего terminal-body (нет асимметричных 8/22/10/14).

## Отличия от original.md той же папки
1. Нет `data-pty-id`-хостов и inline display:flex/none — неактивные сессии не присутствуют в дереве (буфер сохраняется в TermSession, поведенчески эквивалентно).
2. Нет асимметричных инсетов `.session` (top 8 / right 22 / bottom 10 / left 14, зазор под scroll-thumb) — единый p 8; скроллбара-thumb нет вообще (скролл колесом).
3. xterm-опции не применимы: fontSize у нас 12 (оригинал 13); cursorBlink нет (статичный block); scrollback задаётся alacritty-конфигом, не 5000 xterm.
4. Тема: не из `--editor-bg/-fg/-cursor` (#1d1c25/#dcdce4/#a0a0d0), а из палитры UI: фон карты bg_mantle, fg text_secondary #adb3c7, курсор accent_primary #89b4fa — грубое цветовое расхождение.
5. Ctrl+V перехватывается вручную (нативного paste нет); Ctrl+C-c-выделением = copy ✓ как оригинал.
6. Сообщение `[process exited with code N]` в буфер не пишется.
7. ResizeObserver → fit: заменён probe-bounds пересчётом каждый рендер ✓ (эквивалент).

## Дополнение атрибутов (цикл 10)

- скругления: тело сессии radius-md 12 (`root.rs:3971`, `metrics/lib.rs:38`) = `.body { border-radius: var(--radius-md) }` (`TerminalView.module.css:39`); у самой сетки/`.session`-инсета скруглений нет ни там (`:43-54`), ни у нас (инсеты 8/22/10/14 — `root.rs:3973-3976`).
- ховер: N/A: ховер — хост сессии некликабелен в смысле подсветки: обработчики есть (фокус по клику, drop файла, выделение мышью — `root.rs:3944-3947,3910-3943`), но hover-стилей нет; у `.session`/`.body` оригинала hover-правил тоже нет.

### Вердикты

# 91 — verdict (review cycle 1)
VERDICT: DIVERGES (мелочи)
Инсеты/mono13/цвета/Ctrl+C/Ctrl+V/fit/drop подтверждены.
Расхождения: нет cursorBlink; scrollback 10000 vs 5000; смонтирована только
активная сессия (эффект тот же); нет «[process exited with code N]».

## Цикл 5: DIVERGES

Хост сессии терминала: инсеты, mono 13, editor-bg/-fg/-cursor, Ctrl+C с выделением = copy, Ctrl+V, probe-fit, drop путей — подтверждены. Остаток: нет `cursorBlink` (курсор статичный), scrollback не 5000, выделение accent-primary 30% вместо белого 30% (дефолт xterm), нет «[process exited with code N]».

## Цикл 6: DIVERGES

Нет `cursorBlink`, scrollback 5000, цвета выделения xterm, «[process exited…]».

---

# Зона 92-107 — Дерево файлов и его меню

## 92. file-tree-root — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](92-file-tree-root/original.png)
![наш](92-file-tree-root/ours.png)

### Оригинал

# 92 file-tree-root — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:55-74`, `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root (+ optional className из пропа, конкатенация)
├── <FileTreeHeader />
└── div.body [data-file-tree] (onContextMenu: если e.target === e.currentTarget → openFileContextMenu корня {path: root, name: basename(root), type: "dir"})
    └── <FolderNode key={root} path={root} depth={0} initiallyExpanded />
```
- `key={root}` — смена workspace-папки полностью ремаунтит дерево.
- Right-click по пустой области (сами строки делают stopPropagation через собственные обработчики) = контекст-меню корневой папки.

## Метрики (ИЗ CSS, точные значения)
`.root`:
- flex: 1; display: flex; flex-direction: column; min-height: 0
- цвета/шрифт не задаёт (наследует)

`.body`:
- flex: 1; overflow: auto
- padding: 4px 6px 8px (top 4, право/лево 6, низ 8; горизонтальный inset чтобы скруглённый highlight строк не прилипал к краям панели)
- font-size: var(--fs-sm)
- background не задан (прозрачный)

## Состояния (классы-варианты с метриками)
- Вариантных классов нет; состояние «нет папки» — отдельный элемент 93 (`.empty`).

## Дополнение атрибутов (цикл 10)

- цвета: собственных фонов у `.root`/`.body` нет (`FileTreeView.module.css:1-15`) — просвечивает карта-хозяин с `--bg-mantle` #262533 dark (`dark-theme.css:12`) / #fbf7f4 light (`light-theme.css:25`); цвет текста приходит от строк `.row { color: var(--text-secondary) }` #adb3c7 / #524c43 (`:75`; `dark-theme.css:36`, `light-theme.css:46`), у пустого состояния `.empty { color: var(--text-muted) }` #838aa0 / #6e685d (`:26`).

### Наша реализация

# 92 file-tree-root — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:415-641` (`file_tree()`), `crates/shell/src/root.rs:3060-3104` (wiring в tool_body «tree»), `crates/shell/src/root.rs:132,364` (владелец `TreeState`)

## Структура (gpui-дерево кратко)
```
div .size_full .flex .flex_col .min_h(0)
├── header (элемент 98)
└── scroll_body: div #panel_key .relative .flex_1 .min_h(0) .flex .flex_col
      .text_size(FS_SM) .overflow_y_scrollbar_with(tree.scroll) .px(6) .pb(8)
    ├── probe_area(panel_key)
    ├── root_row (кастомная строка корня: chevron 12 + folder_img 16×16 + имя;
    │             pl 8, gap SPACE_1=4, py 2, radius XS, hover text_primary 6%;
    │             LMB toggle, RMB → меню корня)
    └── rows(root, depth=1) при root_expanded (элементы 94-96);
        если cache пуст → "Loading…" (pl 20, py 2, text_muted)
```
Смена workspace сбрасывает `TreeState` целиком (root.rs:638) — аналог ремаунта по `key={root}`.

## Метрики (из кода, точные)
- Тело: `px(6)`, `pb(8)`, **top-padding нет**; font `FS_SM` = 12px; фон прозрачный; скролл — `ScrollHandle` (программный, для Locate).
- root_row: `pl(8)`, `pr(SPACE_2=8)`, `py(2)`, `gap(SPACE_1=4)`, `rounded(RADIUS_XS=4)`, hover `text_primary` a=0.06 (#cfd4e2 @6%).
- Chevron корня: codicon 12px в боксе 16×16, цвет `text_muted` #838aa0.

## Отличия от original.md той же папки
1. **RMB по пустой области тела не открывает меню корня** — обработчик RMB только на строках; в оригинале `onContextMenu` на `.body` (e.target===currentTarget → меню корня). Меню корня доступно лишь через RMB по root_row.
2. **padding тела**: у нас `6px горизонталь + 8 низ`, верхних 4px нет (оригинал `padding: 4px 6px 8px`).
3. **Корень — кастомная строка**, не обычный FolderNode depth 0: gap 4 вместо 6, hover `text_primary 6%` вместо `bg-surface 55%`, нет TreeIcon-«isRoot», нет selected-состояния у корня.
4. `[data-file-tree]` нет — вместо него id `panel_key` + probe_area.

## Дополнение атрибутов (цикл 10)

- шрифты: скролл-тело задаёт базовый кегль fs-sm 12 (`file_list.rs:663`, `metrics/lib.rs:43`) = `.body { font-size: var(--fs-sm) }` (`FileTreeView.module.css:14`); заголовок хедера fs-xs 11 + weight 500 MEDIUM + ss01 (`file_list.rs:528-529`); бейдж «Indexing…» fs-xs 11 + глиф codicon 12 (`file_list.rs:541,544`); кнопки тулбара — глиф codicon 14 (`file_list.rs:453`).

### Вердикты

## Цикл 4: DIVERGES

Корневая строка: цвет был `text_primary` (замер ink 207,212,226 против
175,179,198 у оригинала) — `.row { color: text-secondary }` + hover красит в
primary. **Исправлено волной 7** (`file_list.rs` корневая строка). Остаётся:
RMB по пустой области тела не открывает меню корня (оригинал
`FileTreeView.tsx:55-74` вешает `onContextMenu` на `.body`), селект корня.

## Цикл 8: DIVERGES

Цвет корневой строки закрыт. Осталось: RMB по пустой области тела и селект корневой строки.

## Цикл 10: DIVERGES

RMB по пустой области тела не открывает меню корня (у scroll_body нет обработчика правой кнопки); корневая строка не проходит через applyClickSelection — нет ни стиля выделения, ни вызова on_select. НОВОЕ: корень не красит лейбл декорацией и не рисует бейдж.

---

## 93. file-tree-empty-state — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](93-file-tree-empty-state/original.png)
![наш](93-file-tree-empty-state/ours.png)

### Оригинал

# 93 file-tree-empty-state — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:40-53`, `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root (+ className)
├── <FileTreeHeader />
└── div.empty
    ├── i.codicon.codicon-folder.emptyIcon (aria-hidden)
    ├── p.emptyHint "No active session with a folder."
    └── p.emptyHint "Pick a session in Projects, or start one with a folder."
```
Рендерится, когда `workspaceFolder.value` = null.

## Метрики (ИЗ CSS, точные значения)
`.empty`:
- flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center
- gap: var(--space-2); padding: var(--space-5)
- text-align: center; color: var(--text-muted)

`.emptyIcon`:
- font-size: 32px; color: var(--text-disabled)

`.emptyHint`:
- margin: 0; font-size: var(--fs-sm)

В CSS есть также `.openBtn` (кнопка «Open Folder»: margin-top var(--space-2); padding 6px 14px; background var(--accent-primary); color var(--accent-action-fg); border 1px solid var(--accent-primary); border-radius var(--radius-sm); font-size var(--fs-sm); font-weight 600; transition background var(--transition-fast); hover: background/border-color var(--accent-action-hover)) — в текущем JSX empty-состояния кнопка НЕ рендерится (класс не используется в .tsx).

## Состояния (классы-варианты с метриками)
- Нет hover/active-вариантов; статичный блок.

### Наша реализация

# 93 file-tree-empty-state — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:429-448` (ранний return в `file_tree()` при `workspace=None`)

## Структура (gpui-дерево кратко)
```
div #panel_key .size_full .flex .flex_col .items_center .justify_center
    .gap(SPACE_2=8) .text_color(text_muted)
├── codicon "\u{ea83}" (folder) 32px в боксе 16×16
├── div .text_size(FS_SM) "No active session with a folder."
└── probe_area(panel_key)
```
Хедер (элемент 98) при этом НЕ рендерится (ранний return до его сборки).

## Метрики (из кода, точные)
- gap `SPACE_2` = 8px; цвет контейнера `text_muted` #838aa0; глиф 32px; текст `FS_SM` = 12px.
- padding не задаётся (центрирование flex-ом).

## Отличия от original.md той же папки
1. **Вторая подсказка отсутствует**: только «No active session with a folder.», нет «Pick a session in Projects, or start one with a folder.»
2. **Цвет иконки**: наследует `text_muted` #838aa0; в оригинале `.emptyIcon` = `--text-disabled` (#60667b) — иконка у нас светлее.
3. `padding: var(--space-5)` (20px) не воспроизведён — при узкой панели текст ляжет ближе к краям.
4. **Хедер дерева не рендерится** в empty-состоянии; в оригинале `<FileTreeHeader />` есть и тут (title «PROJECT», disabled-кнопки).
5. Бокс глифа 16×16 при font 32px — глиф вылезает за бокс (центрирован, визуально ок, но геометрия не 1:1).

## Дополнение атрибутов (цикл 10)

- скругления: N/A: скругления — кнопка `.openBtn { border-radius: var(--radius-sm) 8 }` (`FileTreeView.module.css:39-50`) не портирована: наше пустое состояние — только глиф и строка текста, скруглённых элементов нет (`file_list.rs:486-502`).
- шрифты: подсказка fs-sm 12 (`file_list.rs:498`) = `.emptyHint { font-size: var(--fs-sm) }` (`FileTreeView.module.css:34-37`); глиф codicon 32 (`file_list.rs:495`) = `.emptyIcon { font-size: 32px }` (`:29-32`); текст кнопки (fs-sm 12 / weight 600 у оригинала, `:46-47`) отсутствует вместе с кнопкой.
- ховер: N/A: ховер — hover есть только у `.openBtn:hover` оригинала (фон `--accent-action-hover` #74c7ec dark / #b16527 light, `FileTreeView.module.css:52-55`); кнопки у нас нет, у остальной разметки пустого состояния hover-правил нет.

### Вердикты

## Цикл 4: DIVERGES

Пустое состояние: нет второй подсказки «Pick a session in Projects, or start
one with a folder.», `.emptyIcon` должен быть `--text-disabled` (у нас
наследуется text-muted), нет `padding: var(--space-5)`, и в empty-состоянии
оригинал ВСЁ РАВНО рисует `FileTreeHeader` — у нас ранний return без него.

## Цикл 8: DIVERGES

Пустое состояние: нет заголовка дерева, второй подсказки, `padding: space-5`, иконка наследует muted вместо text-disabled.

## Цикл 10: DIVERGES

Все четыре пункта живы: в пустой ветке не рисуется хедер; нет второй подсказки «Pick a session in Projects…»; нет padding 20; цвет глифа text_muted #838aa0 вместо text-disabled #60667b.

---

## 94. file-tree-folder-row — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](94-file-tree-folder-row/original.png)
![наш](94-file-tree-folder-row/ours.png)

### Оригинал

# 94 file-tree-folder-row — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:171-195` (FolderNode), `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`, `file-tree-helpers.tsx` (indentPx)

## JSX-структура (кратко, вложенность)
```
div.node
└── button.row.rowDir[.rowSelected][.dropTarget]
    (style: paddingLeft = indentPx(depth) = depth*12 + 8 px;
     aria-expanded; data-tree-id={path}; draggable={depth > 0};
     onDragStart → beginNativeDrag; onClick → setActiveTreeNode + (Ctrl/Shift-select через applyClickSelection, иначе toggle expand);
     onContextMenu → openFileContextMenu; onKeyDown → onRowKey (Delete/F2/Ctrl+X/C/V))
    ├── i.codicon.chevron — loading ? "codicon-loading codicon-modifier-spin" : expanded ? "codicon-chevron-down" : "codicon-chevron-right" (aria-hidden)
    ├── <TreeIcon className={icon} name type="dir" expanded isRoot={depth===0} />
    ├── span.label (style.color = decorationColor(deco.color) при decoration; data-tooltip = deco.tooltip ?? path) {name}
    └── <RowBadge deco /> (элемент 97)
```

## Метрики (ИЗ CSS, точные значения)
`.node`: display: contents (не создаёт бокс).

`.row`:
- display: flex; align-items: center; gap: 6px
- width: 100%; height: 22px; padding-right: 8px; box-sizing: border-box
- padding-left — инлайн: `depth*12 + 8`px (INDENT_PX=12, BASE_INDENT_PX=8, file-tree-helpers.tsx:14-17)
- background: transparent
- border: 1px solid transparent (зарезервирован, чтобы accent-бордер selected не сдвигал layout)
- border-radius: var(--radius-xs)
- color: var(--text-secondary)
- text-align: left; cursor: pointer; white-space: nowrap; overflow: hidden
- font: inherit; font-size: var(--fs-sm)

`.chevron`:
- flex-shrink: 0; font-size: 13px; width: 16px; text-align: center; color: var(--text-muted)

`.icon` (бокс TreeIcon):
- flex-shrink: 0; width: 16px; height: 16px

`.label`:
- flex: 1; overflow: hidden; text-overflow: ellipsis

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--bg-surface) 55%, transparent); color: var(--text-primary)
- `.rowSelected`, `.rowSelected:hover`:
  - background: linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent))
  - border-color: color-mix(in srgb, var(--accent-primary) 45%, transparent)
  - color: var(--text-primary)
- `.rowSelected .chevron`: color: inherit
- `.dropTarget`, `.dropTarget:hover` (drag файла/папки над строкой папки):
  - background: color-mix(in srgb, var(--accent-primary) 22%, transparent)
  - outline: 1px solid var(--accent-primary); outline-offset: -1px
- Спиннер загрузки: chevron заменяется на `codicon-loading codicon-modifier-spin` (метрики те же, что `.chevron`)
- `.rowDir` — селектора в CSS-модуле нет (класс-маркер без правил)
- `.flash` (при locate): animation: treeFlash 0.9s ease-out 1; @keyframes treeFlash: 0% background color-mix(in srgb, var(--accent-primary) 40%, transparent) → 100% transparent

### Наша реализация

# 94 file-tree-folder-row — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:171-327` (`rows()`, ветка `is_dir`), `crates/shell/src/ui/icon.rs:45-55` (codicon), `crates/shell/src/icon_theme.rs:129-138` (folder_img)

## Структура (gpui-дерево кратко)
```
div #"{panel_key}:{path}" .flex .items_center .gap(6) .pl(depth*12+8) .pr(8) .py(2)
    .rounded(RADIUS_XS) .cursor_pointer .hover(bg_surface 55%)
    [selected → bg linear_gradient(90°, accent 26% → 14%) + border_1 accent 45%]
    on_mouse_down(Left): Ctrl → select-toggle, иначе select + toggle expand
    on_mouse_down(Right): stop_propagation + OpenFileMenu(path, true, x, y)
├── codicon(loading ? "\u{eb19}" : expanded ? CHEVRON_DOWN : CHEVRON_RIGHT, 12px)
│   .text_color(text_muted)
├── icon_theme::folder_img(name, expanded) 16×16 .flex_shrink_0
├── label: div .flex_1 .min_w(0) .overflow_hidden .text_ellipsis .whitespace_nowrap
│   [deco.color → text_color(deco_color(id))]
└── badge (элемент 97)
```
Дети рекурсивно append'ятся плоско в общий Vec (обёртки `.node`/`.children` нет — display:contents эквивалент).

## Метрики (из кода, точные)
- gap 6; `pl = depth*12 + 8`; `pr = SPACE_2` = 8; `py 2` (высота контентная); `rounded RADIUS_XS` = 4.
- hover: `bg_surface` #3d3f51 a=0.55.
- selected: градиент 90° `accent_primary` #89b4fa 26%→14% + `border_1` accent 45%; hover поверх selected тот же (hover-bg перекрывает? — hover задан всегда, при selected градиент в bg, hover заменит его на bg_surface 55% — см. отличия).
- chevron: глиф 12px в боксе 16×16, `text_muted` #838aa0; spinner-глиф `\u{eb19}` (codicon-loading).
- иконка 16×16; label — цвет deco или наследуемый.

## Отличия от original.md той же папки
1. **Высота строки**: `py(2)` (~20px контентно) вместо фиксированной `height: 22px`.
2. **Нет резервного `border: 1px solid transparent`** — бордер появляется только у selected → контент selected-строки сдвигается на 1px.
3. **Цвет текста не задан** (наследуется), оригинал: `--text-secondary` → hover `--text-primary`. Hover у нас цвет НЕ меняет.
4. **`.rowSelected:hover`**: у нас hover(bg_surface 55%) объявлен на всех строках — при наведении на selected градиент подменяется обычным hover-фоном (оригинал сохраняет градиент).
5. **chevron 12px** vs 13px оригинала; `.rowSelected .chevron: color inherit` не воспроизведён (остаётся muted).
6. **Спиннер не вращается** — статичный глиф codicon-loading без `codicon-modifier-spin` анимации.
7. **Папки не draggable** (draggable={depth>0} оригинала; у нас on_drag только у файлов) и **нет `.dropTarget`** (accent 22% + outline) — drop на папку не реализован.
8. **Нет Shift-select** (только Ctrl-toggle) и **нет клавиатуры** (Delete/F2/Ctrl+X/C/V, aria-expanded).
9. **Нет data-tooltip** (deco.tooltip ?? path) и нет `.flash`-анимации locate.
10. Клик срабатывает на mouse_down, не на click.

## Дополнение атрибутов (цикл 10)

- шрифты: строка своего кегля не задаёт — наследует fs-sm 12 от скролл-тела (`file_list.rs:663`, строка `:207-227`), ровно как `.row { font: inherit; font-size: var(--fs-sm) }` (`FileTreeView.module.css:80-81`); chevron codicon 13 в боксе 16 (`file_list.rs:307-316`) = `.chevron { font-size: 13px; width: 16px }` (`:120-127`); бейдж декорации fs-xs 11 + weight 600 SEMIBOLD (`file_list.rs:365-366`) = `.badge { var(--fs-xs); font-weight: 600 }` (`:147-153`); имя папки — тот же наследуемый кегль без weight.

### Вердикты

## Цикл 4: DIVERGES

Метрики строки папки приведены (h 22 + резерв бордера, chevron 13 в боксе 16,
иконка 16, gap 6) — **волна 6**; ховер по ВЫДЕЛЕННОЙ строке гасил
accent-градиент — **исправлено волной 7** (`.when(!is_selected)`).
Остаётся: спиннер не вращается (нет анимации `codicon-modifier-spin`),
`.dropTarget` (accent 22% + outline −1) нет, папки не draggable,
нет Shift-select/клавиатуры/`.flash`.

## Цикл 8: DIVERGES

Метрики и ховер над выделенной закрыты, шаг строк 22.0 у обеих сторон. Осталось: спиннер не вращается, `.dropTarget` нет, папки не draggable, нет Shift-select/клавиатуры/`.flash`.

## Цикл 10: DIVERGES

Метрики совпали живьём: шаг строк 22.08 (оригинал 22), индент 12.04, gap 6, бокс chevron 16, иконка 16. ОСТАЛОСЬ: спиннер без вращения (codicon-modifier-spin); .dropTarget не реализован (grep = 0); папки не draggable; нет shift-select, клавиатуры и .flash.

---

## 95. file-tree-file-row — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](95-file-tree-file-row/original.png)
![наш](95-file-tree-file-row/ours.png)

### Оригинал

# 95 file-tree-file-row — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:228-253` (FileLeaf), `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`, `file-tree-helpers.tsx` (indentPx)

## JSX-структура (кратко, вложенность)
```
button.row.rowFile[.rowSelected]
  (style: paddingLeft = depth*12 + 8 px; data-tree-id={path}; draggable (всегда);
   onDragStart → beginNativeDrag; onClick → setActiveTreeNode + (Ctrl/Shift-select, иначе openFile);
   onContextMenu → openFileContextMenu; onKeyDown → onRowKey)
├── span.chevronSpacer (aria-hidden) — выравнивающий спейсер вместо chevron
├── <TreeIcon className={icon} name type="file" />
├── span.label (style.color = decorationColor(deco.color) при decoration; data-tooltip = deco.tooltip ?? path) {name}
└── <RowBadge deco />
```
Selected — только из explorer-selection (синхронизирована с активным файлом редактора).

## Метрики (ИЗ CSS, точные значения)
`.row` — как у 94:
- display: flex; align-items: center; gap: 6px; width: 100%; height: 22px
- padding-right: 8px; padding-left инлайн `depth*12 + 8`px; box-sizing: border-box
- background: transparent; border: 1px solid transparent; border-radius: var(--radius-xs)
- color: var(--text-secondary); text-align: left; cursor: pointer; white-space: nowrap; overflow: hidden
- font: inherit; font-size: var(--fs-sm)

`.chevronSpacer` (общее правило с `.chevron`):
- flex-shrink: 0; font-size: 13px; width: 16px; text-align: center; color: var(--text-muted)

`.icon`: flex-shrink: 0; width: 16px; height: 16px

`.label`: flex: 1; overflow: hidden; text-overflow: ellipsis

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--bg-surface) 55%, transparent); color: var(--text-primary)
- `.rowSelected`, `.rowSelected:hover`: background: linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent)); border-color: color-mix(in srgb, var(--accent-primary) 45%, transparent); color: var(--text-primary)
- `.rowFile` — класс-маркер без CSS-правил
- `.flash` (locate): animation: treeFlash 0.9s ease-out 1 (40% accent → transparent)

### Наша реализация

# 95 file-tree-file-row — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:171-327` (`rows()`, ветка `!is_dir`, drag 240-245), `crates/shell/src/ui/file_list.rs:71-98` (`DraggedFile`, `FileDragGhost`), `crates/shell/src/icon_theme.rs:119-126` (file_img)

## Структура (gpui-дерево кратко)
```
div #"{panel_key}:{path}"  (тот же контейнер, что 94)
    on_mouse_down(Left): Ctrl → select-toggle, иначе select + OpenFile(path)
    on_mouse_down(Right): OpenFileMenu(path, false, x, y)
    on_drag(DraggedFile{path}) → ghost: FileDragGhost (пилюля с именем:
        px SPACE_2, py 2, radius SM, bg_surface, border text_primary 15%, FS_XS)
├── chevron-спейсер: div 16×16 (пустой)
├── icon_theme::file_img(name) 16×16 .flex_shrink_0
├── label (flex_1, ellipsis, deco-цвет)
└── badge (элемент 97)
```

## Метрики (из кода, точные)
- Все метрики строки идентичны 94: gap 6, `pl depth*12+8`, pr 8, py 2, radius 4, hover `bg_surface` 55%, selected градиент accent 26%→14% + бордер 45%.
- Спейсер 16×16 (оригинал: width 16, font-size 13 — визуально эквивалент).
- Drag-ghost: `SPACE_2`/2px паддинги, `RADIUS_SM` 8, `bg_surface` #3d3f51, бордер `text_primary` 15%, текст `FS_XS` 11 `text_primary`.

## Отличия от original.md той же папки
1. Все пункты 1-4, 8-10 из 94 (высота py2 vs 22px, нет резервного бордера, нет text-secondary→primary, hover перекрывает selected, нет Shift-select/клавиатуры/тултипа/flash, mouse_down).
2. **Drag** — внутренний gpui `on_drag` (drop: редактор → открыть, терминал → путь) вместо нативного `beginNativeDrag`; в ОС файл унести нельзя. Ghost-пилюля — наша добавка (в оригинале нативный drag-image).
3. **Selected не синхронизирован с активным файлом редактора** — только клик/Ctrl-клик по дереву (оригинал: explorer-selection ← активный таб).

### Вердикты

## Цикл 4: DIVERGES

Строка файла: то же, что 94 (ховер над выделенной — исправлено).
Остаётся: выделение не следует за активным табом редактора (`tree.selected`
пишется только кликом и Locate, в `OpenFile`/`FileOpened` не выставляется),
нативного OS-drag нет.

## Цикл 8: DIVERGES

Выделение не следует за активным табом редактора; нативного OS-drag нет.

## Цикл 10: DIVERGES

Нет синхронизации выделения с активным файлом (installActiveFileSelectionSync): выделение двигают только клик по узлу и Locate, открытие файла и смена таба — нет. Драг внутренний вместо beginNativeDrag.

---

## 96. file-tree-children-states — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](96-file-tree-children-states/original.png)
![наш](96-file-tree-children-states/ours.png)

### Оригинал

# 96 file-tree-children-states — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:196-222`, `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
{expanded && (
  div.children
  ├── {entries === null} → div.loading (style: paddingLeft = indentPx(depth+1)) "Loading…"
  ├── {entries.length === 0} → div.emptyChild (paddingLeft = indentPx(depth+1)) "(empty)"
  ├── entries.slice(0, childCap).map → <FolderNode> | <FileLeaf> (key = path, depth+1)
  └── {entries.length > childCap} → button.showMore (paddingLeft = indentPx(depth+1))
      ├── i.codicon.codicon-ellipsis (aria-hidden)
      └── "Show {min(rest, 200)} more ({rest} hidden)"
)}
```
- Кап: TREE_CHILD_CAP = 100, шаг TREE_CHILD_STEP = 200 (клик по showMore: childCap += 200).
- «Loading…» только при первом листинге (entries === null); при fsRev-рефреше старые entries остаются, спиннер — в chevron строки.
- indentPx(d) = d*12 + 8 px.

## Метрики (ИЗ CSS, точные значения)
`.children`: display: contents.

`.loading`, `.emptyChild` (общее правило):
- font-size: var(--fs-xs); color: var(--text-muted); padding: 2px 0
- padding-left — инлайн (см. выше)

`.showMore`:
- display: flex; align-items: center; gap: 6px; width: 100%
- border: none; background: none; font: inherit; font-size: var(--fs-xs)
- color: var(--text-muted); cursor: pointer; padding: 3px 0 (плюс инлайн padding-left); text-align: left

## Состояния (классы-варианты с метриками)
- `.showMore:hover`: color: var(--text-primary); background: color-mix(in srgb, var(--bg-surface) 55%, transparent)
- `.loading` / `.emptyChild` — статичны, без hover.

### Наша реализация

# 96 file-tree-children-states — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:67-68` (`DIR_RENDER_CAP`), `:161-170` (усечение), `:344-372` («Show N more…»), `:619-627` (корневой «Loading…»); `crates/shell/src/root.rs:1143` (`ShowMoreDir` → `show_all`), `:711-713` (loading-set)

## Структура (gpui-дерево кратко)
```
rows(dir): дети рекурсивно плоским списком (без обёртки .children)
├── [entries.len() > 200 && !show_all] → рендер первых 200
└── capped → div #"{panel_key}:more:{dir}" .flex .items_center
      .pl((depth+1)*12+8) .py(2) .rounded(RADIUS_XS) .text_size(FS_XS)
      .text_color(text_muted) .hover(bg text_primary 6%)
      on_mouse_down(Left) → ShowMoreDir(dir)  → "Show {N} more…"

Корень (file_tree): root_expanded && cache пуст →
    div .pl(20) .py(2) .text_color(text_muted) "Loading…"
```
Для раскрытой ПОДдиректории без листинга детей нет вовсе — индикатор только spinner-глиф в chevron строки (tree.loading).

## Метрики (из кода, точные)
- Кап `DIR_RENDER_CAP` = **200**; клик по «Show more» ставит dir в `show_all` → показываются ВСЕ.
- showMore: `pl (depth+1)*12+8`, `py 2`, radius 4, `FS_XS` 11, `text_muted` #838aa0, hover `text_primary` a=0.06.
- Loading (корень): `pl 20`, `py 2`, `text_muted`, шрифт наследуемый FS_SM.

## Отличия от original.md той же папки
1. **Кап 200 без шага**: оригинал TREE_CHILD_CAP=100 + шаг 200 («догрузка» порциями); у нас 200 и клик раскрывает всё сразу.
2. **Лейбл**: «Show {N} more…» без «({rest} hidden)» и **без иконки codicon-ellipsis**.
3. **«(empty)» не рендерится** — пустая раскрытая папка выглядит как закрытая (ничего под ней).
4. **«Loading…» только на корневом уровне** (и с pl 20, не indentPx(1)=20 — тут совпало); в поддиректориях текстового Loading нет, только chevron-spinner.
5. hover showMore: `text_primary 6%` фон без смены цвета текста; оригинал — `bg-surface 55%` + `color: text-primary`.
6. padding showMore `py 2` vs `3px 0`; у нас есть radius/фон-хайлайт, у оригинала кнопка без скругления фона (background: none, hover-фон без radius-указания — фактически тот же класс, расхождение минимально).

## Дополнение атрибутов (цикл 10)

- гэпы: у строк «Loading…» и «(empty)» flex-gap нет — это одиночные текстовые блоки с `pl = depth*12 + 8` и `py 2` (`file_list.rs:168-176,181-189`), как `.loading`/`.emptyChild { padding: 2px 0 }` (`FileTreeView.module.css:155-160`); у строки «Show N more» gap 6 между глифом и текстом (`file_list.rs:408`) = `.showMore { gap: 6px }` (`:164-168`).
- шрифты: все три состояния fs-xs 11 (`file_list.rs:172,185,415`, `metrics/lib.rs:42`) = `.loading`/`.emptyChild`/`.showMore { font-size: var(--fs-xs) }` (`FileTreeView.module.css:157,172`); глиф «…» codicon 13 (`file_list.rs:423`); корневой «Loading…» кегля не задаёт — наследует fs-sm 12 тела (`file_list.rs:688-692`).

### Вердикты

## Цикл 4: DIVERGES

Состояния детей — **исправлено волной 7**: кап 100 + шаг 200
(`DIR_RENDER_CAP`/`DIR_RENDER_STEP`, было 200 и «раскрыть всё»), лейбл
«Show N more (M hidden)» + глиф ellipsis, `.emptyChild` «(empty)» и
per-dir «Loading…». Проверить живьём на большой папке (>100 файлов).

## Цикл 8: DIVERGES

Кап 100/шаг 200, «(N hidden)», «(empty)», per-dir «Loading…» — закрыто и проверено живьём. **Индент «Show N more» исправлен волной 15** (был на 12px глубже: лишний `+1` к глубине). Нит: у `.showMore` не должно быть скругления фона ховера.

## Цикл 10: DIVERGES

Кап 100/шаг 200, «Show N more (N hidden)», «(empty)», per-dir Loading — на месте; индент подтверждён закрытым численно. Остался нит: у showMore лишнее скругление RADIUS_XS, в оригинале скругления нет.

---

## 97. file-tree-row-badge — **MATCH** (цикл 10)

*История: ц4:MATCH, ц8:MATCH, ц10:MATCH*

![оригинал](97-file-tree-row-badge/original.png)
![наш](97-file-tree-row-badge/ours.png)

### Оригинал

# 97 file-tree-row-badge — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/file-tree-helpers.tsx:62-65` (RowBadge), `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
{deco?.badge ? (
  span.badge
    (style.color = decorationColor(deco.color);  // ThemeColor → css-цвет
     data-tooltip = deco.tooltip)
  {deco.badge}   // короткая строка-статус: git "M"/"U" и т.п.
) : null}
```
Данные — из FileDecorationProvider через `useFileDecoration(path)` (hostRpc.fileDecorations.get; ре-запрос по path-scoped tick или глобальной версии).

## Метрики (ИЗ CSS, точные значения)
`.badge`:
- flex-shrink: 0
- margin-left: auto (прижат к правому краю строки)
- padding-left: 6px
- font-size: var(--fs-xs); font-weight: 600
- color — инлайн из `decorationColor(deco.color)` (ThemeColor decoration'а); background/border нет

## Состояния (классы-варианты с метриками)
- Вариантов нет; цвет полностью определяется decoration. При отсутствии `deco.badge` элемент не рендерится.

## Дополнение атрибутов (цикл 10)

- цвета: `.badge` собственного цвета не задаёт (`FileTreeView.module.css:147-153`) — красится из `decorationColor(id)` (`signals/file-decorations.ts:41-60`): modified → `--accent-orange` #fab387 dark (`dark-theme.css:49`) / #da8343 light (`light-theme.css:65`); untracked/added/stageModified → `--accent-green` #a6e3a1 / #5e9855 (`:45`, `:61`); deleted/conflicting → `--accent-red` #f38ba8 / #ca3939 (`:43`, `:59`); ignored → `--text-disabled` #60667b / #938e82 (`:38`, `:48`); submodule и фоллбэк неизвестного id → `--accent-blue` #89b4fa / #3b6fc4 (`:41`, `:57`); list.warning/problemsWarning → `--accent-yellow` #f9e2af / #c89a3f (`:46`, `:62`). Без ThemeColor цвет наследуется от строки — `--text-secondary` #adb3c7 / #524c43 (`FileTreeView.module.css:75`).

### Наша реализация

# 97 file-tree-row-badge — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:308-326` (рендер в rows()), `:22-45` (`Deco`, `deco_color`), `crates/shell/src/root.rs:1147-1169` (кэш decorations, инвалидация)

## Структура (gpui-дерево кратко)
```
{deco.badge} → div .flex_shrink_0 .text_size(FS_XS)
    .text_color(deco.color → deco_color(id) | text_muted)
    .child(badge)          // "M"/"U"/…
{нет badge} → пустой div
```
Данные: `kamin:decorations` хоста, кэш `tree.deco: path → Option<Deco>` (None = «запрошено, пусто»); свежие пути запрашиваются при листинге (root.rs:701-703).

## Метрики (из кода, точные)
- `FS_XS` = 11px; прижат вправо за счёт `flex_1` у label (аналог margin-left:auto).
- Цвета `deco_color` (COLOR_MAP 1:1): modified→`accent_orange` #fab387, untracked/added/stageModified→`accent_green` #a6e3a1, deleted/conflicting→`accent_red` #f38ba8, ignored→`text_disabled` #60667b, submodule→`accent_blue` #89b4fa, list.error→red, list.warning→`accent_yellow` #f9e2af, fallback→`accent_blue`.
- Без background/border ✓.

## Отличия от original.md той же папки
1. **Нет `font-weight: 600`** — бейдж обычным весом.
2. **Фолбэк-цвет**: badge без deco.color у нас `text_muted`; в оригинале инлайн-color не ставится → наследует цвет строки (text-secondary/при hover primary).
3. **Нет `data-tooltip`** (deco.tooltip) — тултип декорации не показывается (осознанно, см. комментарий file_list.rs:22).
4. padding-left 6px оригинала компенсирован row-gap 6 — эквивалент; при отсутствии badge у нас рендерится пустой div (в оригинале null) — на layout не влияет.

### Вердикты

## Цикл 4: MATCH

Badge декорации: `ml auto + pl 6 + weight 600 + fs-xs`, цвет из ThemeColor.
Нит цикла 4 (фолбэк `text_muted` вместо наследования цвета строки) —
**исправлен волной 7** (`when_some(color)`).

## Цикл 8: MATCH

Badge декорации 1:1.

## Цикл 10: MATCH

Бейдж строки 1:1: flex-shrink 0 / ml auto / pl 6 / fs 11 / weight 600 = css:147-153. Живьём правый край бейджа на 7-8 px левее края кнопки Refresh — ровно разница инсетов 15 против 8.

---

## 98. file-tree-header-toolbar — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](98-file-tree-header-toolbar/original.png)
![наш](98-file-tree-header-toolbar/ours.png)

### Оригинал

# 98 file-tree-header-toolbar — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeHeader.tsx:26-77`, `kamin-ide/src/renderer/components/file-tree/FileTreeHeader.module.css`, `FileTreeView.module.css` (`.flash` — flashRow)

## JSX-структура (кратко, вложенность)
```
header.header
├── span.title (data-tooltip = root) {folderName ?? "PROJECT"}   // последняя часть пути
├── {indexing.value} → span.indexing (data-tooltip="Building the search index (Ctrl+P)…")
│   ├── i.codicon.codicon-loading.codicon-modifier-spin (aria-hidden)
│   └── "Indexing…"
└── div.actions
    ├── button.btn [aria-label/data-tooltip "Locate selected file"; disabled = !root || !selectedFile]
    │   └── i.codicon.codicon-target
    ├── button.btn [collapsed ? "Expand all folders" : "Collapse all folders"; disabled = !root; onClick toggleCollapseAll]
    │   └── i.codicon.{codicon-expand-all | codicon-collapse-all}
    └── button.btn [aria-label/data-tooltip "Refresh"; disabled = !root; onClick: workspaceFolder → null → queueMicrotask восстановить (полный ремаунт)]
        └── i.codicon.codicon-refresh
```
Locate: revealTarget.value = path → каскадное раскрытие предков; поллинг `[data-tree-id]` каждые 50мс до 60 попыток; найдя — scrollIntoView({block:"center", behavior:"smooth"}) + класс `.flash` на 900мс (SCROLL_FLASH_MS).

## Метрики (ИЗ CSS, точные значения)
`.header`:
- display: flex; align-items: center; gap: var(--space-1)
- padding: 8px 8px 8px 12px; flex-shrink: 0

`.title`:
- flex: 1; font-size: var(--fs-xs); font-weight: 500; letter-spacing: 0.08em
- color: var(--text-muted); font-feature-settings: "ss01"
- overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-transform: uppercase

`.indexing`:
- display: inline-flex; align-items: center; gap: 4px; flex-shrink: 0
- font-size: var(--fs-xs); color: var(--text-muted); opacity: 0.85
- `.indexing :global(.codicon)`: font-size: 12px

`.actions`: display: inline-flex; align-items: center; gap: 2px

`.btn`:
- width: 22px; height: 22px; display: grid; place-items: center
- background: transparent; border: none; color: var(--text-muted)
- border-radius: var(--radius-xs); cursor: pointer
- transition: background var(--transition-fast), color var(--transition-fast)
- `.btn :global(.codicon)`: font-size: 14px

`.flash` (в FileTreeView.module.css): animation: treeFlash 0.9s ease-out 1; keyframes: 0% background color-mix(in srgb, var(--accent-primary) 40%, transparent) → 100% transparent

## Состояния (классы-варианты с метриками)
- `.btn:hover:not([disabled])`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.btn[disabled]`: opacity: 0.4; cursor: not-allowed
- Collapse/Expand — одна кнопка, иконка и подписи переключаются по `treeAllCollapsed`.

### Наша реализация

# 98 file-tree-header-toolbar — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:376-409` (`tool_btn`), `:458-520` (header), `:112-146` (`flat_row_index`); `crates/shell/src/root.rs:550-591` (`LocateSelectedFile`), `:665-673` (Refresh/Collapse), `:1171-1173` (IndexStatus)

## Структура (gpui-дерево кратко)
```
header: div .flex .items_center .gap(SPACE_1=4) .pl(12) .pr(8) .py(8) .flex_shrink_0
├── title: div .flex_1 .min_w(0) .ellipsis .nowrap .text_size(FS_XS)
│     .font_weight(MEDIUM) .text_color(text_muted) .child(имя_папки.to_uppercase())
├── [indexing] div .px(8) .text_size(FS_XS) .text_color(text_muted) .opacity(0.85)
│     "Indexing…"                       // без спиннера
└── actions: div .flex .items_center .gap(2)
    ├── tool_btn "tree-locate"   "\u{ebf8}" "Locate selected file"
    ├── tool_btn "tree-collapse" "\u{eac5}" "Collapse all folders"   // collapse-all
    └── tool_btn "tree-refresh"  "\u{eb37}" "Refresh"

tool_btn: div 22×22 .flex center .rounded(RADIUS_XS) .text_color(text_muted)
    codicon(glyph, 14) ; hover: bg_surface 60% + text_primary; disabled → opacity 0.4
```
Locate (root.rs): цель = selected | активный таб редактора → раскрыть предков до корня (+дозапрос листингов) → select → скролл `set_offset(idx*24 − 140)`.
Refresh: пере-листинг всех expanded директорий (без ремаунта). Collapse: `expanded = {root}`.

## Метрики (из кода, точные)
- header: gap 4, `padding: 8px 8px 8px 12px` ✓, flex_shrink_0 ✓.
- title: `FS_XS` 11, weight 500, `text_muted` #838aa0, uppercase, ellipsis.
- indexing: FS_XS, text_muted, opacity 0.85.
- btn: 22×22, глиф 14px, radius 4, hover `bg_surface` #3d3f51 a=0.6 + `text_primary` #cfd4e2.

## Отличия от original.md той же папки
1. **Title без `letter-spacing: 0.08em` и `font-feature-settings: "ss01"`**.
2. **Indexing без спиннера** codicon-loading (spin, 12px) и gap 4 — только текст; тултип «Building the search index…» отсутствует.
3. **Collapse/Expand не тумблер**: всегда collapse-all (\u{eac5}) с одной подписью; состояния `treeAllCollapsed` → codicon-expand-all/«Expand all folders» нет.
4. **Disabled-логика отсутствует**: все 3 кнопки всегда активны (`disabled=false`); в оригинале locate гаснет без root/selectedFile, остальные без root.
5. **Locate без флеша и smooth-скролла**: мгновенный `set_offset` по расчётной высоте строки 24px (реальная ~20-22 → накапливается неточность на длинных списках); `.flash` (treeFlash 0.9s accent 40%→transparent) не реализован нигде в gpui-порте; поллинга data-tree-id нет (расчёт синхронный — листинги предков могут ещё не прийти → скролл до подгрузки промахивается).
6. **Refresh** — пере-листинг expanded-директорий, а не полный ремаунт (null→восстановить); кэш deco не сбрасывается.
7. Глиф locate `\u{ebf8}` — сверить с оригинальным `codicon-target`.

### Вердикты

## Цикл 4: DIVERGES

Тулбар заголовка: кнопки 22×22/глиф 14/ховер bg-surface 60%/disabled 0.4 и
глифы (EBF8/EAC5/EB37) — MATCH. Locate считал 24px на строку при фактических
22 — **исправлено волной 7** (`root.rs` scroll-to-row 22.0).
Остаётся: Collapse не тумблер, `disabled` не вычисляется, нет `.flash`
(treeFlash 0.9s), Refresh — пере-листинг вместо ремаунта, нет тултипа
«Building the search index…». `letter-spacing 0.08em` — ограничение gpui.

## Цикл 8: DIVERGES

Шаг скролла Locate 22 закрыт. Осталось: Collapse не тумблер, `disabled` не вычисляется, нет `.flash`, Refresh — пере-листинг вместо ремаунта, нет тултипов Indexing и полного пути у титула.

## Цикл 10: DIVERGES

Титул pl 12 и питч кнопок 24.4 совпали. ОСТАЛОСЬ: Collapse не тумблер (всегда collapse-all, оригинал меняет глиф и лейбл по treeAllCollapsed); disabled не вычисляется у трёх кнопок; нет .flash; Refresh = пере-листинг вместо полного ремаунта; нет тултипов у титула (полный путь) и бейджа Indexing.

---

## 99. tree-icon-img — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](99-tree-icon-img/original.png)
![наш](99-tree-icon-img/ours.png)

### Оригинал

# 99 tree-icon-img — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/TreeIcon.tsx:39-40` (весь компонент 22-41), `kamin-ide/src/renderer/components/file-tree/TreeIcon.module.css`; данные: `file-tree/file-icons.ts`, `vendor/fileIcons.ts`, `vendor/folderIcons.ts`, signals/icon-theme

## JSX-структура (кратко, вложенность)
```
img.img (+ optional className, конкатенация "img className") src={src} alt="" aria-hidden
```
Логика src:
- Синхронно: builtin Catppuccin — `type === "dir" ? folderIconUrl(name, expanded) : fileIconUrl(name)` (строки никогда не мигают пустыми).
- Асинхронный апгрейд: при активной contributed icon-theme (`activeIconDoc`) — `themeIconUrl(name, type, expanded, isRoot)`; если вернул url → setSrc(url), null → остаётся Catppuccin.
- useEffect ресетит на builtin при любой смене входов (name/type/expanded/isRoot/doc/themeKind); guard `live` против гонки.
- Пропсы: `expanded` (папки: open/closed глиф), `isRoot` (rootFolder* карты темы).

## Метрики (ИЗ CSS, точные значения)
`.img`:
- display: block
- размеры в модуле НЕ заданы — бокс задаёт вызывающая сторона (в дереве `.icon` из FileTreeView.module.css: width 16px, height 16px, flex-shrink 0)

Light-тема:
- `:global([data-theme="light"]) .img`: filter: saturate(3.2) brightness(0.7)
  (Catppuccin-пастель на светлых панелях выцветает — насыщение ×3.2, затемнение до 0.7)

## Состояния (классы-варианты с метриками)
- Вариантных классов нет. Два визуальных режима: dark (без фильтра) / light (filter выше). Контент src: builtin Catppuccin ↔ contributed-theme icon.

## Дополнение атрибутов (цикл 10)

- цвета: CSS цвета иконке не задаёт — `.img { display: block }` (`TreeIcon.module.css:5`), краски лежат внутри Catppuccin-SVG, и currentColor строки (`--text-secondary` #adb3c7 dark / #524c43 light, `FileTreeView.module.css:75`; `dark-theme.css:36`, `light-theme.css:46`) на неё НЕ действует — в отличие от codicon-chevron рядом. Единственная цветовая правка — светлая тема: `filter: saturate(3.2) brightness(0.7)` для `[data-theme="light"] .img` (`TreeIcon.module.css:6`), компенсация пастелей на панели `--bg-mantle` #fbf7f4 (`light-theme.css:25`).
- отступы: собственных padding/margin у `.img` нет; горизонтальный ритм даёт строка — `.row { gap: 6px; padding-right: 8px }` (`FileTreeView.module.css:62-68`), а бокс иконки фиксирован `.icon { width: 16px; height: 16px; flex-shrink: 0 }` (`:131-135`); отступ уровня — `indentPx(depth) = depth*12 + 8` (`file-tree-helpers.tsx`).

### Наша реализация

# 99 tree-icon-img — наша реализация
Файлы: `crates/shell/src/icon_theme.rs:119-138` (`file_img`/`folder_img`), `:37-109` (резолв contributed icon-темы), `crates/shell/src/cat_icons.rs:2489+, 4759+` (Catppuccin-маппинг ext/name/folder, сгенерирован), `crates/shell/src/ui/file_list.rs:281-289` (бокс 16×16)

## Структура (gpui-дерево кратко)
```
gpui::img(src) .flex_shrink_0 .w(16) .h(16)     // бокс задаёт вызывающая сторона
src:
  ACTIVE contributed icon-theme (kamin:iconTheme:load, iconPath абсолютные)
    → resolve_file: fileNames → цепочка суффиксов после каждой точки → file-дефолт
    → resolve_folder: folderNames(Expanded) → folder/folderExpanded (взаимный фолбэк)
  иначе → cat_icons::file_icon(name) | folder_icon(name, open)  // embedded SVG-ассеты
```
Тема — глобальный `static ACTIVE: Mutex<Option<IconTheme>>`; SVG contributed-темы читаются gpui напрямую с диска.

## Метрики (из кода, точные)
- 16×16, flex_shrink_0 (у вызывающего) ✓; сами SVG несут цвета Catppuccin.
- fontCharacter-дефиниции тем не поддержаны → фолбэк на Catppuccin (гэп совпадает с оригиналом, plan/25).

## Отличия от original.md той же папки
1. **Порядок резолва инвертирован без визуального дефекта**: у нас contributed-тема резолвится СИНХРОННО первой (SVG с диска), фолбэк Catppuccin; в оригинале синхронно Catppuccin + асинхронный апгрейд до темы. «Мигания» нет в обоих, но у нас нет промежуточного кадра Catppuccin.
2. **Light-фильтр НЕ реализован**: `[data-theme="light"] .img { filter: saturate(3.2) brightness(0.7) }` — в light-теме Catppuccin-пастель останется блеклой на светлых панелях.
3. **`isRoot` не поддержан** — карты `rootFolder*` contributed-темы игнорируются (корень получает обычную folder-иконку).
4. Резолв-порядок расширений: у нас цепочка суффиксов слева направо после каждой точки (длинный суффикс первым — как VS Code) — совпадает с оригиналом; регистронезависимость ✓.

## Дополнение атрибутов (цикл 10)

- цвета: `file_img`/`folder_img` отдают `gpui::img` без тонирования (`icon_theme.rs:119-138`) — цвет внутри Catppuccin-SVG (`cat_icons.rs`), currentColor строки (text_secondary #adb3c7 dark / #524c43 light, `file_list.rs:221`, `palette.rs:64,102`) на картинку не влияет; светлотемного фильтра `saturate(3.2) brightness(0.7)` у нас НЕТ (grep по `crates/shell/src` пуст) — в light-теме иконки бледнее оригинала на панели bg_mantle #fbf7f4 (`palette.rs:93`).
- отступы: у иконки padding/margin нет — только фикс-бокс 16×16 (`file_list.rs:332-334`); зазор до имени даёт строка `gap 6` (`file_list.rs:211`), правый край `pr SPACE_2 8` (`:214`), отступ уровня `pl = depth*12 + 8` (`:213`).

### Вердикты

## Цикл 4: DIVERGES

TreeIcon: бокс 16×16 — MATCH. Нет светлого фильтра
(`saturate(3.2) brightness(0.7)`) и карт `rootFolder*`/`isRoot`.

## Цикл 8: DIVERGES

Бокс 16×16 верен. Нет светлого фильтра `saturate(3.2) brightness(0.7)` и карт `rootFolder*` для корня.

## Цикл 10: DIVERGES

Бокс 16×16 верен. Нет светлого фильтра saturate(3.2) brightness(0.7) (grep = 0). Нет rootFolder-иконок: в IconTheme нет полей rootFolder/rootFolderNames, флаг isRoot в folder_img не передаётся.

---

## 100. file-context-menu — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](100-file-context-menu/original.png)
![наш](100-file-context-menu/ours.png)

### Оригинал

# 100 file-context-menu — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileContextMenu.tsx:133-146` (компонент 21-161), `kamin-ide/src/renderer/components/file-tree/FileContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
createPortal(document.body):
div.menu [role=menu] (style: left/top px из clampToViewport(anchor=курсор, side:"bottom", offset:0); visibility hidden→visible после измерения)
└── items.flatMap:  // порядок: state.extra (tab-actions) → builtinActions → explorerContextItems
    ├── {смена a.group} → div.separator [role=separator]
    └── row(a):
        ├── без children → button.item[.danger] [role=menuitem]
        │   ├── i.fas.{a.icon}.itemIcon (aria-hidden; фикс. слот и без иконки)
        │   └── span.label {a.label}
        └── с children → button.item.hasSub [role=menuitem, aria-haspopup=menu]
            ├── i.fas.itemIcon
            ├── span.label
            └── i.codicon.codicon-chevron-right.chevron
```
Поведение: закрытие по outside-mousedown (capture) / Esc / scroll(capture); ре-открытие на новой позиции ресетит submenu; hover leaf-строки root-меню → scheduleSubClose (grace 250мс), hover `.hasSub` → открыть submenu (элемент 101). Иконки — FontAwesome (`fas`), chevron — codicon.

## Метрики (ИЗ CSS, точные значения)
`.menu`:
- position: fixed; z-index: var(--z-dropdown)
- min-width: 180px; max-height: calc(100vh - 16px); max-width: calc(100vw - 16px); overflow-y: auto
- background: var(--bg-surface); border: 1px solid var(--divider-soft)
- border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)
- margin: 0; padding: var(--space-1)
- display: flex; flex-direction: column; gap: 1px

`.item`:
- display: flex; align-items: center; gap: var(--space-2); width: 100%
- padding: var(--space-2) var(--space-3)
- background: transparent; border: none; border-radius: var(--radius-sm)
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer

`.itemIcon`:
- width: 16px; font-size: 12px; text-align: center; flex-shrink: 0; color: var(--text-muted)

`.label`: flex: 1; white-space: nowrap

`.hasSub`: position: relative

`.chevron`: font-size: 12px; color: var(--text-muted); margin-left: var(--space-2)

`.separator`: height: 1px; margin: var(--space-1) var(--space-2); background: var(--divider-soft)

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--text-primary) 10%, transparent)
- `.danger`: color: var(--accent-danger, #e5484d); `.danger .itemIcon`: color: inherit
- `.danger:hover`: background: color-mix(in srgb, var(--accent-danger, #e5484d) 16%, transparent)
- visibility: hidden до измерения bounding rect (двухпроходное позиционирование), затем visible.

### Наша реализация

# 100 file-context-menu — наша реализация
Файлы: `crates/shell/src/ui/file_menu.rs:114-547` (`file_menu()`, `item`/`icon_slot`/`divider`), `crates/shell/src/root.rs:828-847` (открытие/multi), `:4892,5055-5066` (закрытие Esc/click-away), `crates/shell/src/overlay.rs:174+` (`dropdown_shadow`). Рендер в overlay-окне (единый слой), `hit_area` в корне.

## Структура (gpui-дерево кратко)
```
layer: div .absolute .top_0 .left_0 .size_full
└── col: div #file-menu .occlude .absolute .left(x) .top(y) .min_w(200)
      .flex .flex_col .gap(1) .p(SPACE_1) .rounded(RADIUS_MD)
      .bg(bg_surface) .border_1(text_primary 6%) .shadow(dropdown)
    ├── hit_area()
    ├── «Open In ▸» (hover → каскад, элемент 101) + divider
    ├── [dir] New File… / New Folder… + divider
    ├── Cut / Copy / Paste + divider          // multi>1 → операция над выбором
    ├── Rename… / Delete («Delete N items» при multi) + divider
    ├── Copy Path / Copy Relative Path
    └── contributed explorer/context (when-движок, сортировка групп
        navigation-first, divider на смене группы, без иконок;
        клик → kamin:command:execute с Uri {$mid:1})
item: .flex .items_center .gap(SPACE_2) .px(SPACE_3) .py(SPACE_2) .rounded(RADIUS_SM)
      .text_size(FS_SM); icon_slot 16px (FA-глиф 12px, muted | red)
```
Позиция: `x = clamp(cursor, MARGIN..viewport−200−8)`, `y = clamp(cursor, ..viewport−est_h−8)`, est_h = 380 (dir) / 330 (file).

## Метрики (из кода, точные)
- Меню: min-width **200**, padding `SPACE_1` 4, gap 1, radius `RADIUS_MD` 12, bg `bg_surface` #3d3f51, бордер 1px `text_primary` a=0.06, тень 0/8/24 rgba(0,0,0,.45).
- Item: gap 8, px 12, py 8, radius 8, `FS_SM` 12, `text_primary` #cfd4e2; hover `text_primary` 10%.
- Danger: `accent_red` #f38ba8 (текст+иконка), hover red 16%.
- Иконки: FontAwesome solid (weight 900), слот 16px, глиф 12px, `text_muted` #838aa0.
- Divider: h 1, mx `SPACE_2` 8, my `SPACE_1` 4, `text_primary` 6%.

## Отличия от original.md той же папки
1. **min-width 200 vs 180** (SUB тоже шире — см. 101).
2. **Позиционирование эвристикой** est_h (380/330) вместо двухпроходного измерения (visibility hidden→visible); при contributed-пунктах est_h занижен → меню у нижнего края может вылезти. **Нет max-height/overflow-y** (`calc(100vh-16px)`).
3. **danger-цвет**: `accent_red` #f38ba8 vs `var(--accent-danger, #e5484d)` оригинала.
4. **Бордер/сепаратор** из `text_primary 6%` vs `var(--divider-soft)` — сверить фактическое значение токена.
5. **Порядок**: «Open In» вынесен первым фикс-пунктом; в оригинале порядок = state.extra (tab-actions) → builtinActions → contributed. **tab-actions (extra) не поддержаны.**
6. **Закрытие**: Esc + click-away (root.rs) есть; закрытия по scroll(capture) нет; `role=menu/menuitem`-семантики нет.
7. Contributed-пункты без иконок (слот пустой) — как в оригинале иконка тоже отсутствует? В оригинале i.fas рендерится всегда фикс-слотом — совпадает.
8. Наша добавка: «Delete N items» при мультиселекте (в original.md не описано).

### Вердикты

## Цикл 4: DIVERGES

Контекст-меню файла — **волна 7**: `min_w` 180 (было 200), `max-height
100vh−16`, danger `#e5484d` (у оригинала токен `--accent-danger` не объявлен,
работает CSS-фолбэк; НЕ accent-red #f38ba8). Остаётся: позиция по эвристике
`est_h` вместо двухпроходного измерения, «Open In» принудительно первым,
tab-actions (`state.extra`) нет, нет закрытия по скроллу и Escape,
`overflow_hidden` вместо своего скролла.

## Цикл 8: DIVERGES

min-w 180, max-height, danger #e5484d закрыты. Осталось: позиция по `est_h`, «Open In» принудительно первым, нет tab-actions, Escape и закрытия по скроллу.

## Цикл 10: DIVERGES

Закрыто: min-w 180, max-h 100vh−16, danger #e5484d. ★ ДВЕ ЛОЖНЫЕ претензии: «Open In принудительно первым» — в оригинале обе группы действий тоже безусловно начинаются с Open In; «нет Escape» — биндинг есть и меню гасится. Осталось: позиция по захардкоженному est_h вместо реального rect; нет закрытия по скроллу. Претензия про tab-actions не по адресу: функционал есть, отличается архитектура.

---

## 101. file-context-submenu — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](101-file-context-submenu/original.png)
![наш](101-file-context-submenu/ours.png)

### Оригинал

# 101 file-context-submenu — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileContextMenu.tsx:147-157`, `kamin-ide/src/renderer/components/file-tree/FileContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
{sub && createPortal(document.body):
  div.menu [role=menu, tabIndex=-1]
    (style: left/top из clampToViewport(anchor = rect родительской .hasSub строки, side: "right", offset: 2); visibility hidden→visible;
     onMouseEnter → cancelSubClose; onMouseLeave → scheduleSubClose)
  └── sub.action.children.map → leaf(a, inSub=true):
      button.item[.danger] [role=menuitem] (onMouseEnter → cancelSubClose)
      ├── i.fas.{a.icon}.itemIcon
      └── span.label {a.label}
}
```
Каскад «Open In ▸»: открывается hover'ом по `.hasSub` в root-меню; закрытие с grace-задержкой SUB_CLOSE_DELAY_MS = 250мс (диагональный проход курсора через соседние строки не убивает submenu); вход в submenu или возврат на родителя отменяет таймер. Клик по leaf: a.run() + закрытие всего меню.

## Метрики (ИЗ CSS, точные значения)
Использует те же классы, что 100 (тот же модуль):
- `.menu`: position fixed; z-index var(--z-dropdown); min-width 180px; background var(--bg-surface); border 1px solid var(--divider-soft); border-radius var(--radius-md); box-shadow var(--shadow-dropdown); padding var(--space-1); flex column; gap 1px; max-height calc(100vh - 16px); max-width calc(100vw - 16px); overflow-y auto
- `.item`: flex; gap var(--space-2); padding var(--space-2) var(--space-3); border-radius var(--radius-sm); color var(--text-primary); font-size var(--fs-sm)
- `.itemIcon`: width 16px; font-size 12px; color var(--text-muted)
- `.label`: flex 1; white-space nowrap
- Позиционирование: справа от anchor-строки, offset 2px, кламп во viewport.

## Состояния (классы-варианты с метриками)
- `.item:hover`: background color-mix(in srgb, var(--text-primary) 10%, transparent)
- `.danger` / `.danger:hover` — как в 100
- visibility: hidden до измерения, потом visible; таймер закрытия 250мс.

### Наша реализация

# 101 file-context-submenu — наша реализация
Файлы: `crates/shell/src/ui/file_menu.rs:272-303` (строка «Open In» + hover-открытие), `:549-641` (каскад), `:114-116` (константы), `crates/shell/src/root.rs:592-596` (`FileMenuOpenIn`)

## Структура (gpui-дерево кратко)
```
[menu.open_in] → sub: div #file-menu-sub .occlude .absolute .left(sub_x) .top(sub_y)
    .min_w(260) .flex .flex_col .gap(1) .p(SPACE_1) .rounded(RADIUS_MD)
    .bg(bg_surface) .border_1(text_primary 6%) .shadow(dropdown)
  ├── hit_area()
  ├── «Reveal in File Explorer» (explorer.exe [/select,path])
  └── dir → «Open in Terminal» | file → «Open in Associated Application»
sub_x = x + 200 + 2 (влево при нехватке: x − 260 − 2); sub_y = clamp(y, ..vh−120)
Открытие: on_hover строки «Open In ▸» → ShellEvent::FileMenuOpenIn(true);
активная строка подсвечена bg text_primary 10%.
```

## Метрики (из кода, точные)
- SUB_W min-width **260**; offset от родительского меню 2px ✓; бокс/item-метрики идентичны 100 (p 4, gap 1, radius 12, bg_surface, бордер 6%, item px12/py8/radius8/FS_SM, hover 10%).
- Строка-родитель: chevron-right codicon `\u{eab6}` 12px `text_muted` справа.

## Отличия от original.md той же папки
1. **Grace-закрытие 250мс НЕ реализовано, и хуже — каскад вообще не закрывается при ховере других пунктов root-меню**: замыкание `close_sub` создано, но мёртвое (`let _ = &close_sub;`, file_menu.rs:307-313) — sub живёт до закрытия всего меню.
2. **min-width 260 vs 180**.
3. **Привязка по вертикали**: `sub_y = y` (top root-меню), не rect строки `.hasSub`; совпадает только пока «Open In» — первая строка; кламп по низу `vh−120` эвристикой, без измерения.
4. Пункты каскада захардкожены (Reveal / Terminal / Associated App) — contributed-детей нет; в оригинале children строятся динамически.
5. Нет `role=menu`/`tabIndex=-1`, нет visibility-двухпроходности.

## Дополнение атрибутов (цикл 10)

- цвета: фон каскада bg_surface #3d3f51 dark / #e6e1d4 light (`file_menu.rs:613`, `palette.rs:57,95`), бордер `text_primary 6%` = #cfd4e2 α .06 / #322e28 α .06 (`file_menu.rs:615`, `--divider-soft`, `variables.css:151`), тень — `overlay::dropdown_shadow()` (`file_menu.rs:616`); пункт: текст text_primary #cfd4e2 / #322e28 (`file_menu.rs:547`), hover-фон text_primary α .10 (`file_menu.rs:189`), у danger-пункта hover #e5484d α .16 (`file_menu.rs:186-188`), иконка слота text_muted #838aa0 / #6e685d (`file_menu.rs:151`), danger-цвет — фолбэк-хекс #e5484d (`file_menu.rs:142`; токен `--accent-danger` в темах не объявлен, проверено grep).
- шрифты: пункт fs-sm 12 (`file_menu.rs:546`, `metrics/lib.rs:43`), глиф FontAwesome 12 в боксе 16 (`file_menu.rs:159`), шеврон «Open In ▸» codicon 12 (`file_menu.rs:306`); собственного font-weight у пунктов нет.

### Вердикты

## Цикл 4: DIVERGES

Каскад «Open In» — **волна 7**: `min_w` 180 (было 260), якорь по строке
`.hasSub` (`y + space-1`) вместо верха меню, живое закрытие каскада
(ховер по остальной части меню; раньше `close_sub` был мёртвой привязкой
`let _ = &close_sub;`). Остаётся: 250 мс грация вместо мгновенного закрытия
(нужен таймер в состоянии root).

## Цикл 8: DIVERGES

min-w 180, якорь по строке, живое закрытие закрыты. Осталось: грация 250 мс (нужен таймер в состоянии).

## Цикл 10: DIVERGES

Закрыто: min-w 180 и offset 2. Осталось: грация 250 мс при уходе курсора (сейчас каскад гаснет мгновенно); якорь подменю берётся от первого пункта, а не от rect строки «Move to».

---

## 102. generic-tree — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](102-generic-tree/original.png)
![наш](102-generic-tree/ours.png)

### Оригинал

# 102 generic-tree — оригинал
Файлы: `kamin-ide/src/renderer/components/tree/Tree.tsx:38-53`, `kamin-ide/src/renderer/components/tree/Tree.module.css`

## JSX-структура (кратко, вложенность)
```
ul.tree [role=tree]
└── nodes.map → <TreeRow key={n.id} node depth={0} expanded selectedId onToggle onSelect />
    (TreeRow — элемент 103; открытые dir рендерят ul.subtree [role=group] с детьми depth+1)
```
Полностью контролируемое: caller владеет `expanded: ReadonlySet<string>` и `selectedId: string | null`; клик по любому узлу → `onSelect(node)`, по dir дополнительно `onToggle(id)`. TreeNode: {id, label, type: "dir"|"file", meta?, icon?, children?}.

## Метрики (ИЗ CSS, точные значения)
`.tree`, `.subtree` (общее правило):
- list-style: none; margin: 0; padding: 0
- Отступ вложенности НЕ через ul — через paddingLeft строки (depth * 14px, инлайн; см. 103).

## Состояния (классы-варианты с метриками)
- У контейнера вариантов нет; всё состояние на строках (103).

## Дополнение атрибутов (цикл 10)

- цвета: N/A: цвета — `.tree`/`.subtree` содержат только `list-style: none; margin: 0; padding: 0` (`Tree.module.css:1-5`); весь цвет принадлежит строке (элемент 103): `.row { color: var(--text-primary) }` #cfd4e2 dark / #322e28 light (`:18`; `dark-theme.css:34`, `light-theme.css:44`).

### Наша реализация

# 102 generic-tree — наша реализация

**НЕ РЕАЛИЗОВАНО.**

Переиспользуемого контролируемого дерева (аналог `tree/Tree.tsx`: `role=tree`, caller-owned `expanded: Set` + `selectedId`, произвольные TreeNode {id, label, type, meta, icon, children}) в gpui-порте нет. Grep по `crates/shell/src/` — единственное дерево это `ui/file_list.rs`, специализированное под файловый воркспейс (свой TreeState, host-листинги).

Потребители оригинального generic Tree:
- `sample-tree` дизайн-панели (элемент 139) — в `ui/design_panel.rs` тоже отсутствует;
- прочих потребителей в host-renderer нет.

## Отличия от original.md той же папки
Полное отсутствие компонента. При портировании учесть: `.tree`/`.subtree` без собственных отступов (indent на строке, 14px/уровень — отличается от file-tree 12px+8), полностью контролируемое состояние.

## Дополнение атрибутов (цикл 10)

- цвета: компонента нет; у единственного дерева-аналога контейнер тоже без фона — просвечивает карта bg_mantle #262533 dark / #fbf7f4 light (`palette.rs:55,93`), а цвет текста задаёт строка: text_secondary #adb3c7 / #524c43 (`file_list.rs:221`, `palette.rs:64,102`). При портировании учесть: у generic-строки оригинала базовый цвет — `--text-primary` #cfd4e2 / #322e28, не secondary.

### Вердикты

## Цикл 4: DIVERGES

Generic TreeView не реализован (только файловое дерево).

## Цикл 8: DIVERGES

Generic TreeView не реализован.

## Цикл 10: DIVERGES

Generic-дерево не реализовано. ★ НОВОЕ: это тот же компонент, что элемент 139 (sample-tree) — в продуктовом UI не используется вовсе, только в семплах Design; закрытие 139 закрывает и 102/103.

---

## 103. generic-tree-row — **DIVERGES** (цикл 10)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES*

![оригинал](103-generic-tree-row/original.png)
![наш](103-generic-tree-row/ours.png)

### Оригинал

# 103 generic-tree-row — оригинал
Файлы: `kamin-ide/src/renderer/components/tree/Tree.tsx:63-106` (TreeRow), `kamin-ide/src/renderer/components/tree/Tree.module.css`

## JSX-структура (кратко, вложенность)
```
li [role=treeitem, aria-expanded (только dir), aria-selected]
├── button.row[.selected] (style: paddingLeft = depth*14 px; onClick: dir → onToggle(id), всегда onSelect(node))
│   ├── span.chevron[.chevronHidden если нет детей] (aria-hidden)
│   │   └── i.codicon.codicon-chevron-{down|right}
│   ├── i.codicon.codicon-{node.icon ?? (dir ? "folder" : "file")}.{iconDir|iconFile} (aria-hidden)
│   ├── span.label {node.label}
│   └── {node.meta} → span.meta
└── {isOpen && children} → ul.subtree [role=group] → рекурсивные TreeRow (depth+1)
```
INDENT_PX = 14 (Tree.tsx:56).

## Метрики (ИЗ CSS, точные значения)
`.row`:
- display: inline-flex; align-items: center; gap: var(--space-2); width: 100%
- padding: 4px var(--space-2) (+ инлайн paddingLeft = depth*14 px); box-sizing: border-box
- background: transparent; border: 1px solid transparent (резерв под selected-бордер); border-radius: var(--radius-xs)
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer
- transition: background var(--transition-fast)

`.chevron`:
- width: 14px; display: grid; place-items: center; font-size: 10px; color: var(--text-muted); flex-shrink: 0

`.iconDir`: color: var(--accent-yellow); flex-shrink: 0; font-size: var(--fs-sm)
`.iconFile`: color: var(--text-muted); flex-shrink: 0; font-size: var(--fs-sm)

`.label`: flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap

`.meta`: font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-muted); flex-shrink: 0

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--bg-surface) 55%, transparent)
- `.row.selected`, `.row.selected:hover`:
  - background: linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent))
  - border-color: color-mix(in srgb, var(--accent-primary) 45%, transparent)
  - color: var(--text-primary)
- `.chevronHidden`: visibility: hidden (лист/пустой dir — место сохраняется)
- chevron: codicon-chevron-down (открыт) ↔ codicon-chevron-right (закрыт)

### Наша реализация

# 103 generic-tree-row — наша реализация

**НЕ РЕАЛИЗОВАНО** (вместе с контейнером, см. 102-generic-tree/ours.md).

Строки generic-дерева (TreeRow: indent 14px/уровень, chevron 14px grid-center глиф 10px со скрываемым `.chevronHidden`, codicon-иконки dir=`accent_yellow`/file=`text_muted`, правый `.meta` моноширинный FS_XS, padding 4px/SPACE_2, selected — тот же accent-градиент 26→14% + бордер 45%) в gpui-порте отсутствуют.

Ближайший родственник — строки `ui/file_list.rs` (элементы 94/95), но у них другой indent (12px+8), img-иконки вместо codicon, нет meta-слота и chevronHidden.

## Отличия от original.md той же папки
Полное отсутствие компонента.

## Дополнение атрибутов (цикл 10)

- гэпы: компонента нет; ближайший аналог — строки файлового дерева `file_list.rs:211` с `gap 6` (у generic-строки оригинала `gap: var(--space-2)` 8, `Tree.module.css:10`), в contributed-дереве тоже 6 (`contributed_tree.rs:301`).
- цвета: компонента нет; в аналоге (`file_list.rs:200-243`) hover-фон bg_surface α .55 = #3d3f51 dark / #e6e1d4 light (`palette.rs:57,95`) + текст text_primary #cfd4e2 / #322e28, выделение — градиент 90° accent_primary α .26 → α .14 (#89b4fa / #da8343, `palette.rs:83,121`) с бордером accent α .45; базовый цвет строки у нас text_secondary #adb3c7 / #524c43 (`file_list.rs:221`) против `--text-primary` у generic-строки оригинала (`Tree.module.css:18`) — расхождение при портировании учесть.
- скругления: компонента нет; в аналоге radius-xs 4 (`file_list.rs:220`, `metrics/lib.rs:36`) — совпадает с `.row { border-radius: var(--radius-xs) }` оригинала (`Tree.module.css:17`).
- ховер: компонента нет; в аналоге hover = фон bg_surface α .55 + text_primary, и только у НЕвыделенной строки (`file_list.rs:225-227`), что соответствует паре `.row:hover` / `.row.selected:hover` оригинала (`Tree.module.css:26-37`); transition `background var(--transition-fast) 150ms` (`:23`) в gpui не воспроизводится.

### Вердикты

## Цикл 4: DIVERGES

Строка generic TreeView не реализована.

## Цикл 8: DIVERGES

Строка generic TreeView не реализована.

## Цикл 10: DIVERGES

Не реализовано; рецепт ОТЛИЧАЕТСЯ от file_list и переиспользовать его нельзя: INDENT_PX 14 против 12, padding 4/8 против h22, chevron 14/10 против 16/13, iconDir accent-yellow, колонка meta моно fs-xs.

---

## 104. contributed-tree-view-body — **DIVERGES** (цикл 7)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES, ц7:DIVERGES*

![оригинал](104-contributed-tree-view-body/original.png)
![наш](104-contributed-tree-view-body/ours.png)

### Оригинал

# 104 contributed-tree-view-body — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:42-49` (TreeViewBody 27-50, TreeLevel 54-79), CSS переиспользуется из `file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root (FileTreeView.module.css)
├── {message} → div (инлайн-стиль: padding "4px 8px"; fontSize var(--fs-sm); opacity 0.75) {message}   // TreeView.message
└── div.body
    └── <TreeLevel viewId parent=undefined depth=0 version />
        ├── nodes === null → div.loading (paddingLeft = indentPx(depth)) "Loading…"
        ├── nodes.length === 0 && depth === 0 → div.emptyChild "(empty)"  (глубже — <></>)
        ├── nodes.slice(0, 100).map → <TreeNode key={n.handle}> (элемент 105)
        └── nodes.length > 100 → div.emptyChild "… {N-100} more"
```
- Дети лениво с хоста: hostRpc.trees.getChildren(viewId, parent); рефреш по treeChangeVersion[viewId].
- Кап TREE_CHILD_CAP = 100 (без кнопки догрузки — только счётчик остатка).
- indentPx(d) = d*12 + 8 px (INDENT_PX=12, BASE_INDENT_PX=8 — локальные копии в TreeViewBody.tsx:14-15).
- meta/DnD подтягиваются на mount (getMeta, hasDnd).

## Метрики (ИЗ CSS, точные значения)
Из FileTreeView.module.css:
- `.root`: flex: 1; display: flex; flex-direction: column; min-height: 0
- `.body`: flex: 1; overflow: auto; padding: 4px 6px 8px; font-size: var(--fs-sm)
- `.loading`, `.emptyChild`: font-size: var(--fs-xs); color: var(--text-muted); padding: 2px 0 (+ инлайн paddingLeft)
- message-баннер (инлайн): padding: 4px 8px; font-size: var(--fs-sm); opacity: 0.75

## Состояния (классы-варианты с метриками)
- Loading / empty / overflow («… N more») — см. структуру; вариантных классов у контейнера нет.

### Наша реализация

# 104 contributed-tree-view-body — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`tree_view_body`, `level`), root.rs (`contributed_tree_section`, состояние `trees`), host_link.rs (`request_tree_children`, `request_tree_meta`, канал `kamin:tree:changed`)

## Структура/содержание
```
div .flex_1 .flex_col .min_h 0                  ← .root
├─ (meta.message) div px 8 / py 4, fs-sm, opacity .75
└─ div .flex_1 .overflow_y_scroll .pt 4 .px 6 .pb 8 .text_size 12   ← .body
    └─ уровни: level("", depth 0) → строки узлов + рекурсия раскрытых
```
Дети тянутся лениво: корень — при первом показе панели, уровень узла — при первом раскрытии (`kamin:tree:getChildren`). `kamin:tree:changed` помечает все известные уровни как «грузится» и перезапрашивает их (аналог перемонтирования по `version`). Состояния: «Loading…» (уровень ещё не пришёл), «(empty)» только на depth 0, «… N more» при > 100 узлов.

## Метрики (из кода, точные)
- `.body`: padding 4 / 6 / 8, fs FS_SM 12, `overflow_y_scroll`, min-h 0.
- `.loading`/`.emptyChild`: fs FS_XS 11, text-muted, py 2 + `paddingLeft = depth*12 + 8`.
- message-баннер: px SPACE_2 8, py SPACE_1 4, fs FS_SM 12, opacity 0.75.
- Кап уровня TREE_CHILD_CAP = 100, без кнопки догрузки — только счётчик остатка.

## Отличия от original.md той же папки
1. Вью выбирается по `contributes.views[].type != "webview"` (`DynTool.webview`); tree-вью больше не регистрируются как вебвью и не ждут html.
2. Customize-страницы contributed-контейнеров по-прежнему рендерятся только вебвью (Bridge объявляет их `type: webview`); tree-страница в Customize не поддержана.
3. DnD (`TreeDragAndDropController`) не портирован — в gpui нет HTML5-DnD.

## Дополнение атрибутов (цикл 10)

- цвета: собственного фона у тела нет (`contributed_tree.rs:439-450`) — просвечивает карта bg_mantle #262533 dark / #fbf7f4 light (`palette.rs:55,93`); строки «Loading…» / «(empty)» / «… N more» — text_muted #838aa0 / #6e685d (`contributed_tree.rs:160`, `palette.rs:65,103`); message-баннер цвета не задаёт, наследует текст карты при opacity 0.75 (`contributed_tree.rs:435`). Совпадает с оригиналом (`.body` без фона, `.loading`/`.emptyChild { color: var(--text-muted) }`, `FileTreeView.module.css:8-15,155-160`).

### Вердикты

## Цикл 4: DIVERGES

Contributed TreeDataProvider не реализован: grep по `crates/shell/src` даёт
0 совпадений на `TreeDataProvider|kamin:trees|getChildren`.

## Цикл 8: DIVERGES

Contributed TreeDataProvider не реализован (grep по `TreeDataProvider|getChildren|kamin:trees|checkboxState` = 0).

## Цикл 10: DIVERGES

Contributed-дерево не реализовано: grep TreeDataProvider|treeGetChildren|checkboxState по crates/shell/src = 0; ветка tool_body рисует только вебвью.

## Цикл 7: DIVERGES

Тело портировано; метрики `.body` (4/6/8 + fs-sm), `.loading`/`.emptyChild`
(fs-xs, text-muted, py 2, отступ уровня), message-баннер (4/8, fs-sm, op .75), кап 100 +
«… N more», «(empty)» только на depth 0 — сверены и совпали.

Исправлено по ревью: узел, пришедший УЖЕ раскрытым (`collapsibleState == Expanded`),
теперь добирает свой уровень (раньше висело вечное «Loading…»); `kamin:tree:changed`
больше не затирает уровни в `None` — старое содержимое стоит до прихода нового.

Осталось: DnD не портирован; tree-страницы Customize по-прежнему только вебвью; живой
проверки нет — единственное tree-вью фикстур (`helloTree`) второе в контейнере, а
панель показывает первое (ограничение элемента 73).

---

## 105. contributed-tree-node-row — **DIVERGES** (цикл 7)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES, ц7:DIVERGES*

![оригинал](105-contributed-tree-node-row/original.png)
![наш](105-contributed-tree-node-row/ours.png)

### Оригинал

# 105 contributed-tree-node-row — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:144-178` (TreeNode 81-186), CSS из `file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
div.node
├── button.row.{rowDir|rowFile}[.rowSelected] (ref=rowRef)
│   (style: paddingLeft = depth*12 + 8 px; aria-expanded только если expandable;
│    data-tooltip = node.tooltip ?? node.label; draggable = dndEnabled;
│    onDragStart/onDragOver/onDrop → hostRpc.trees.handleDrag/handleDrop;
│    onClick: toggle expand + reportExpansion, выставить selection + reportSelection, выполнить node.command)
│   ├── expandable ? i.codicon.codicon-chevron-{down|right}.chevron : span.chevronSpacer
│   ├── {node.checkboxState !== undefined} → span.treeCheckbox (элемент 106)
│   ├── <NodeIcon node expanded /> (элемент 107)
│   ├── span.label {node.label}
│   └── {node.description} → span (инлайн: opacity 0.55; marginLeft "6px"; fontSize "0.85em")
└── {expandable && expanded} → div.children → <TreeLevel parent={node.handle} depth={depth+1} />
```
- expandable = collapsibleState !== 0 (NONE); стартовое expanded = сигнал ?? (collapsibleState === 2 EXPANDED).
- rowDir/rowFile по expandable (не по типу файла).
- reveal-action: scrollIntoView({block:"nearest"}) + focus + expand, затем consume.

## Метрики (ИЗ CSS, точные значения)
Общие с file-tree строками (FileTreeView.module.css):
- `.node`, `.children`: display: contents
- `.row`: display flex; align-items center; gap 6px; width 100%; height 22px; padding-right 8px; box-sizing border-box; background transparent; border 1px solid transparent; border-radius var(--radius-xs); color var(--text-secondary); text-align left; cursor pointer; white-space nowrap; overflow hidden; font inherit; font-size var(--fs-sm)
- padding-left инлайн: depth*12 + 8 px
- `.chevron`/`.chevronSpacer`: flex-shrink 0; font-size 13px; width 16px; text-align center; color var(--text-muted)
- `.label`: flex 1; overflow hidden; text-overflow ellipsis
- description (инлайн): opacity 0.55; margin-left 6px; font-size 0.85em

## Состояния (классы-варианты с метриками)
- `.row:hover`: background color-mix(in srgb, var(--bg-surface) 55%, transparent); color var(--text-primary)
- `.rowSelected`(+ :hover): background linear-gradient(90deg, color-mix(in srgb, var(--accent-primary) 26%, transparent), color-mix(in srgb, var(--accent-primary) 14%, transparent)); border-color color-mix(in srgb, var(--accent-primary) 45%, transparent); color var(--text-primary); `.rowSelected .chevron`: color inherit
- `.rowDir`/`.rowFile` — маркеры без CSS-правил
- draggable только при зарегистрированном TreeDragAndDropController (treeDnd[viewId]).

### Наша реализация

# 105 contributed-tree-node-row — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`level`), root.rs (`ShellEvent::TreeClick`)

## Структура/содержание
```
div .row  (id "tv:<view>:<handle>")
├─ expandable ? chevron-down|right в боксе 16 : спейсер 16
├─ (checkboxState) .treeCheckbox (элемент 106)
├─ node_icon (элемент 107)
├─ label .flex_1 .overflow_hidden .text_ellipsis .whitespace_nowrap
└─ (description) ml 6, opacity .55, fs 0.85em
```
Клик — `TreeClick{expandable, expanded, command}`: тоггл раскрытия + `kamin:tree:reportExpansion`, выделение + `kamin:tree:reportSelection`, затем `node.command` через `kamin:commands:execute` (и на листьях, и на родителях). Tooltip = `node.tooltip ?? node.label`.

## Метрики (из кода, точные)
- `.row`: flex, items-center, gap 6, w-full, `paddingLeft = depth*12 + 8`, pr SPACE_2 8, h 22, border 1px transparent (резерв), radius RADIUS_XS 4, fs FS_SM 12, цвет text-secondary.
- hover невыделенной: bg-surface 55% + text-primary.
- `.rowSelected`: линейный градиент 90° accent-primary 26% → 14%, рамка accent-primary 45%, текст text-primary; chevron наследует цвет.
- `.chevron`/спейсер: бокс 16, глиф 13, text-muted.

## Отличия от original.md той же папки
1. DnD не портирован (нет `draggable`, handleDrag/handleDrop).
2. Reveal-действие (`scrollIntoView` + focus + expand по `kamin:tree:reveal`) не портировано.
3. `.rowDir`/`.rowFile` — маркеры без правил, в порте не нужны.

## Дополнение атрибутов (цикл 10)

- цвета: базовый текст text_secondary #adb3c7 dark / #524c43 light (`contributed_tree.rs:312`, `palette.rs:64,102`); hover — фон bg_surface α .55 #3d3f51 / #e6e1d4 + текст text_primary #cfd4e2 / #322e28 (`contributed_tree.rs:291-296,314-316`); выделение — линейный градиент 90° accent_primary α .26 → α .14 (#89b4fa / #da8343) с бордером accent α .45 и текстом text_primary (`contributed_tree.rs:320-334`); chevron text_muted #838aa0 / #6e685d, у выделенной строки цвет наследуется (`contributed_tree.rs:365-367`); бордер по умолчанию прозрачный — резерв под выделение (`:308-309`). 1:1 с `.row`/`.row:hover`/`.rowSelected` (`FileTreeView.module.css:62-94`).
- шрифты: строка fs-sm 12 (`contributed_tree.rs:311`, `metrics/lib.rs:43`); chevron codicon 13 (`contributed_tree.rs:361-363`); иконка-глиф codicon кеглем fs-sm 12 в боксе 16×16 (`contributed_tree.rs:173,200-209`); description — 0.85em от fs-sm = 10.2px при opacity .55 (`contributed_tree.rs:398`, инлайн-стиль оригинала `TreeViewBody.tsx`); галка чекбокса codicon fs-xs 11 (`contributed_tree.rs:249`). Собственного font-weight у строки нет.

### Вердикты

## Цикл 4: DIVERGES

Строка узла contributed-дерева (label/description/tooltip) не реализована.

## Цикл 8: DIVERGES

Строка узла contributed-дерева не реализована.

## Цикл 10: DIVERGES

Не реализовано (см. 104).

## Цикл 7: DIVERGES

CSS-паритет строки полный: gap 6, h 22, pr 8, отступ `depth*12+8`, border 1px
transparent, r-xs 4, fs-sm, text-secondary; hover bg-surface 55% только у невыделенной;
selected — градиент 90° accent 26→14% + рамка 45%; chevron бокс 16 / глиф 13 / muted и
наследование цвета у выделенной; порядок детей; description ml 6 / op .55 / .85em;
tooltip `tooltip ?? label`.

Исправлено по ревью: команда узла уходила несуществующим методом
`kamin:commands:execute` — теперь `kamin:command:execute` (клик по узлу с командой
работал вхолостую); строка получила `overflow: hidden` + `white-space: nowrap`.

Осталось: DnD и reveal не портированы.

---

## 106. contributed-tree-checkbox — **DIVERGES** (цикл 7)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES, ц7:DIVERGES*

![оригинал](106-contributed-tree-checkbox/original.png)
![наш](106-contributed-tree-checkbox/ours.png)

### Оригинал

# 106 contributed-tree-checkbox — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:162-174`, CSS: `file-tree/FileTreeView.module.css` (`.treeCheckbox`)

## JSX-структура (кратко, вложенность)
```
{node.checkboxState !== undefined && (
  span.treeCheckbox [role=checkbox, aria-checked = (checkboxState === 1), tabIndex=0]
    (data-tooltip = node.checkboxTooltip — только если задан;
     onClick → toggleCheckbox (stopPropagation, reportCheckbox с инвертированным состоянием);
     onKeyDown: " " или "Enter" → preventDefault + toggle)
  └── {checked} → i.codicon.codicon-check (aria-hidden)   // unchecked — пустой бокс
)}
```
TreeItemCheckboxState: CHECKED=1, UNCHECKED=0. Тоггл независим от клика по строке; провайдер обновляет модель и re-fetch возвращает перевёрнутое состояние.

## Метрики (ИЗ CSS, точные значения)
`.treeCheckbox`:
- display: inline-flex; align-items: center; justify-content: center
- width: 14px; height: 14px; margin-right: 4px; flex-shrink: 0
- border: 1px solid var(--border-strong, currentColor)
- border-radius: 3px
- font-size: 11px (размер codicon-галки)
- cursor: pointer
- background не задан (прозрачный)

## Состояния (классы-варианты с метриками)
- checked: внутри рендерится codicon-check (11px); unchecked: пусто. CSS-вариантов (hover/checked-классов) нет — различие только контентом и aria-checked.
- Фокусируем (tabIndex=0), клавиатурный toggle Space/Enter.

## Дополнение атрибутов (цикл 10)

- цвета: заливки нет; рамка `border: 1px solid var(--border-strong, currentColor)` (`FileTreeView.module.css:114`) — токен `--border-strong` в темах НЕ объявлен (grep по `variables.css`/`dark-theme.css`/`light-theme.css` пуст), поэтому реально работает `currentColor` = цвет строки: `--text-secondary` #adb3c7 dark / #524c43 light в покое (`:75`; `dark-theme.css:36`, `light-theme.css:46`), `--text-primary` #cfd4e2 / #322e28 на hover и у выделенной строки (`:87,93`); галка-codicon тоже currentColor.

### Наша реализация

# 106 contributed-tree-checkbox — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`checkbox`), root.rs (`ShellEvent::TreeCheckbox`)

## Структура/содержание
Рендерится, когда у узла задан `checkboxState`. Клик — `cx.stop_propagation()` + `kamin:tree:reportCheckbox` с ИНВЕРТИРОВАННЫМ состоянием; провайдер обновляет модель, `onDidChangeTreeData` возвращает перевёрнутый узел.

## Метрики (из кода, точные)
14×14, margin-right 4, flex-shrink 0, центровка, radius 3, рамка 1px currentColor (белый 35%), cursor pointer; при CHECKED — codicon-check размером FS_XS 11. `checkboxTooltip` вешается тултипом, если задан.

## Отличия от original.md той же папки
Клавиатурного тоггла (Space/Enter, tabIndex=0) нет — в порте нет фокус-навигации по строкам дерева.

## Дополнение атрибутов (цикл 10)

- цвета: заливки нет; цвет рамки передаётся параметром `border` = цвет строки (эквивалент currentColor): text_secondary #adb3c7 dark / #524c43 light у обычной строки и text_primary #cfd4e2 / #322e28 у выделенной (`contributed_tree.rs:372-379`, `palette.rs:62,64,100,102`); галка тоже наследует цвет строки (`contributed_tree.rs:249`).
- шрифты: галка — codicon кеглем fs-xs 11 (`contributed_tree.rs:249`, `metrics/lib.rs:42`) = `.treeCheckbox { font-size: 11px }` (`FileTreeView.module.css:117`); собственного семейства/веса у бокса нет.
- ховер: N/A: ховер — у `.treeCheckbox` оригинала правила `:hover` нет (`FileTreeView.module.css:106-118`), у нашего бокса тоже (`contributed_tree.rs:223-247`: только `cursor_pointer` + tooltip); визуально меняется лишь унаследованный currentColor, когда подсвечивается строка (105).

### Вердикты

## Цикл 4: DIVERGES

Чекбокс узла (14×14, radius 3, глиф 11, `checkboxState`) не реализован.

## Цикл 8: DIVERGES

Чекбокс узла (14×14, r3, глиф 11) не реализован.

## Цикл 10: DIVERGES

Не реализовано (см. 104).

## Цикл 7: DIVERGES

14×14, mr 4, flex-shrink 0, r3, галка 11, `stop_propagation`, инверсия состояния,
`checkboxTooltip` — совпали. Исправлено по ревью: рамка больше не жёстко-белая 35%, а
currentColor строки (`--border-strong` в оригинале не определён → фоллбек currentColor);
на светлой теме рамка теперь видна.

Осталось: клавиатурный тоггл (Space/Enter, tabIndex=0) — в порте нет фокус-навигации
по строкам дерева.

---

## 107. contributed-tree-node-icon — **DIVERGES** (цикл 7)

*История: ц4:DIVERGES, ц8:DIVERGES, ц10:DIVERGES, ц7:DIVERGES*

![оригинал](107-contributed-tree-node-icon/original.png)
![наш](107-contributed-tree-node-icon/ours.png)

### Оригинал

# 107 contributed-tree-node-icon — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bodies/TreeViewBody.tsx:189-197` (NodeIcon), CSS: `file-tree/FileTreeView.module.css` (`.icon`); при resourceUri — `file-tree/TreeIcon.tsx` + `TreeIcon.module.css`

## JSX-структура (кратко, вложенность)
Три взаимоисключающие ветки (приоритет сверху вниз):
```
1. node.codicon (ThemeIcon)   → i.codicon.codicon-{node.codicon}.icon (aria-hidden)
2. node.resourceUri           → <TreeIcon className={icon} name={basename(resourceUri)}
                                  type={collapsibleState === 0 ? "file" : "dir"} expanded />
                                  // = img.img.icon (см. элемент 99)
3. иначе (generic)            → i.codicon.{collapsibleState === 0 ? "codicon-circle-outline" : "codicon-folder"}.icon
```
basename: `resourceUri.split(/[\\/]/).pop() ?? ""`.

## Метрики (ИЗ CSS, точные значения)
`.icon` (FileTreeView.module.css):
- flex-shrink: 0; width: 16px; height: 16px
- цвета для codicon-веток не переопределяются классом `.icon` (наследование от строки: обычно var(--text-secondary), hover/selected var(--text-primary))

Для ветки TreeIcon дополнительно `.img` (TreeIcon.module.css):
- display: block; light-тема: filter: saturate(3.2) brightness(0.7)

## Состояния (классы-варианты с метриками)
- Вариантных классов нет; иконка меняется по данным узла (codicon / resourceUri / generic) и по expanded (open/closed глиф папки в ветке TreeIcon).

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding/margin у `.icon` нет — только фиксированный бокс `width: 16px; height: 16px; flex-shrink: 0` (`FileTreeView.module.css:131-135`); зазор до лейбла даёт строка `.row { gap: 6px }` (`:65`), правый край — `.row { padding-right: 8px }` (`:68`), отступ уровня — `indentPx(depth) = depth*12 + 8`; чекбокс перед иконкой добавляет `margin-right: 4px` (`:111`).

### Наша реализация

# 107 contributed-tree-node-icon — наша реализация
Файлы: crates/shell/src/ui/contributed_tree.rs (`node_icon`)

## Структура/содержание
Три взаимоисключающие ветки в порядке оригинала:
1. `node.codicon` (ThemeIcon) → `codicon_by_name(...)`, бокс 16;
2. `node.resourceUri` → basename → `icon_theme::file_img` (collapsibleState == NONE) либо `folder_img(name, expanded)`, 16×16;
3. иначе — codicon-circle-outline (лист) / codicon-folder (узел), 16.

## Метрики (из кода, точные)
Бокс 16×16, flex-shrink 0; цвет codicon-веток наследуется от строки (text-secondary, у выделенной — text-primary).

## Отличия от original.md той же папки
Light-фильтр `saturate(3.2) brightness(0.7)` для `<img>`-иконок (TreeIcon.module.css) не применяется — общий пробел порта (см. элемент 99).

## Дополнение атрибутов (цикл 10)

- отступы: у бокса иконки padding/margin нет — фикс 16×16 (`contributed_tree.rs:200-209`), img-вариант тоже 16×16 (`:184-188`); отступ уровня строки `pl = depth*12 + 8` (`:303`, `indent()` `:150-152`), правый край `pr SPACE_2 8` (`:304`); чекбокс перед иконкой добавляет `mr 4` (`:227`) — как `.treeCheckbox { margin-right: 4px }`.
- гэпы: собственного gap у иконки нет (`icon_box` — flex-center без gap, `contributed_tree.rs:200-209`); расстояние до лейбла даёт строка `gap 6` (`contributed_tree.rs:301`) = `.row { gap: 6px }`.
- цвета: codicon-глиф своего цвета не имеет — наследует цвет строки: text_secondary #adb3c7 dark / #524c43 light, на hover/выделении text_primary #cfd4e2 / #322e28 (`contributed_tree.rs:167-196` + строка `:312,315,334`); при `resourceUri` рисуется img-иконка Catppuccin/contributed-темы со СВОИМИ цветами внутри SVG (`contributed_tree.rs:177-189`, `icon_theme.rs:119-138`); светлотемный фильтр оригинала `saturate(3.2) brightness(0.7)` (`TreeIcon.module.css:6`) НЕ портирован — grep по `crates/shell/src` не даёт ни `saturate`, ни `brightness`.
- ховер: N/A: ховер — у иконки собственных hover-правил нет (`contributed_tree.rs:167-210`), как и у `.icon` оригинала; меняется только унаследованный цвет при ховере строки (105).

### Вердикты

## Цикл 4: DIVERGES

Иконка узла contributed-дерева не реализована; светлый фильтр иконок
(как в 99) тоже отсутствует.

## Цикл 8: DIVERGES

Иконка узла contributed-дерева не реализована.

## Цикл 10: DIVERGES

Не реализовано (см. 104).

## Цикл 7: DIVERGES

Порядок веток и данные верны (codicon → resourceUri basename → generic
circle-outline/folder; `file_img`/`folder_img(name, expanded)`).

Исправлено по ревью: `.icon` задаёт только БОКС 16×16, кегль глифа наследуется от
строки — было `codicon(glyph, 16.0)` (бокс = кегль), стало бокс 16 / шрифт fs-sm 12;
неизвестный ThemeIcon даёт пустой бокс, а не circle-outline.

Осталось: light-фильтр `saturate(3.2) brightness(0.7)` для `<img>`-иконок (см. 99).

---

# Зона 108-129 — Редактор, оверлеи, статус-бар, модалки

## 108. file-viewer-wrapper — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](108-file-viewer-wrapper/original.png)
![наш](108-file-viewer-wrapper/ours.png)

### Оригинал

# 108 file-viewer-wrapper — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewer.tsx` (22-79), `FileViewer.module.css`

## JSX-структура (кратко, вложенность)
```
div.viewer [data-drop-zone="editor"]
├─ <FileViewerTabs />                                (таб-стрип, №110)
└─ div.body (или .body.bodyFlush при webview)
   ├─ активный слот: WebviewPanelView | MonacoEditor | Empty
   │    (retained-панель активна → слот пустой (null))
   └─ для каждой retainContextWhenHidden-панели:
      div.retainLayer style="display: flex|none"     (по p.id === активный wvId)
         └─ <WebviewPanelView visible={...} />
```
- `openFiles` лимит 12 (`OPEN_FILES_LIMIT`); переполнение выкидывает старейший un-pinned неактивный.
- Позиции табов стабильны, новые добавляются в конец.
- webview-таб = путь `webview://<id>`.

## Метрики (ИЗ CSS, точные значения)
`.viewer`:
- flex: 1; display: flex; flex-direction: column; min-height: 0
- margin: 0 6px 6px (верх 0, бока 6px, низ 6px)
- background: var(--bg-mantle)
- border-radius: var(--radius-md); overflow: hidden

`.body`:
- flex: 1; min-height: 0; display: flex; flex-direction: column
- background: var(--editor-bg)
- border-radius: var(--radius-md); overflow: hidden
- padding: 8px 0 10px (верх 8px, бока 0, низ 10px — гуттеры редактора)

`.bodyFlush` (webview активен): padding: 0

`.retainLayer`: flex: 1; min-height: 0; flex-direction: column (display управляется inline: `flex`/`none`)

## Состояния (классы-варианты с метриками)
- `.body` → `.body.bodyFlush`: только padding 8px 0 10px → 0 (когда показывается webview-панель).
- `.retainLayer[display:none]`: скрытая retained-панель (iframe остаётся в DOM).
- hover/active/focus/transition: отсутствуют на обвязке.

### Наша реализация

# 108 file-viewer-wrapper — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:4315-4495 (ветка редактора), 4498-4549 (top_card + glint-обёртка), crates\metrics\src\lib.rs, crates\theme\src\palette.rs

## Структура (gpui-дерево кратко)
```
gap_wrap_v_top(glint_surface_wv_holed(          — карта File-панели (glint-рамка)
  top_card: div.flex_col.size_full
  ├─ on_drop(ExternalPaths) + on_drop(DraggedFile)   — drop-zone редактора
  ├─ div (mode-header: justify_end, pt 6, px 8) → file_panel_mode_tabs
  └─ top_content =
     ├─ editor_tabs непусты: div.flex_col.size_full
     │  ├─ ряд: editor_tabs_bar (№110) + (dirty) кнопка «Save  Ctrl+S»
     │  └─ рамка редактора: div.flex_col.flex_1.mx(4).mt(4).mb(4)
     │     .rounded(12).overflow_hidden().bg(editor_bg)
     │     ├─ breadcrumb-строка 24px (путь ~-сокращённый, mono)
     │     └─ ряд: [Input редактора + sticky-overlay] + minimap
     └─ пусто: panel_placeholder (№109)
))
```
Лимит табов `MAX_EDITOR_TABS = 12` (root.rs:112) — эвикт старейшего un-pinned неактивного (root.rs:3648-3661), pinned-first сортировка (root.rs:2074-2081).

## Метрики (из кода, точные)
- Рамка редактора: `mx 4` (SPACE_1), `mt 4`, `mb 4`, `rounded 12` (RADIUS_MD), `bg p.editor_bg` #1d1c25, overflow hidden
- Breadcrumb: h 24, px 12 (SPACE_3), fs 11 (FS_XS), font «JetBrains Mono», цвет p.text_muted #838aa0, ellipsis
- Mode-header: pt 6, px 8 (SPACE_2), justify_end
- Кнопка Save: mx 8, px 12, py 3, rounded 8, fs 11 semibold, bg p.accent_action #89b4fa, fg p.accent_action_fg #313240, hover opacity .9
- Внешняя карта — glint-рамка (не bg-mantle-контейнер)

## Отличия от original.md той же папки
1. Нет контейнера `.viewer` (margin 0 6px 6px, bg-mantle, radius-md) — вместо него общая glint-карта File-панели; редакторная рамка получает mx 4 / mt 4 / mb 4 вместо паддингов `.body` 8px 0 10px.
2. Нет `.bodyFlush` и retained-слоя `retainLayer`: webview-панели у нас НЕ открываются как редакторские табы (см. №114/№115), путей `webview://<id>` нет.
3. Добавлен breadcrumb-заголовок с путём внутри рамки (в оригинале FileViewer его нет).
4. Добавлена кнопка «Save  Ctrl+S» при dirty (в оригинале нет).
5. Mode-header (file/web-переключатель) — часть этой обвязки; в оригинале это отдельный элемент file-panel-top-card (№63).
6. Drop-zone есть (ExternalPaths + внутренний drag из дерева), атрибут-семантики `data-drop-zone` нет.

### Вердикты

# 108 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет .viewer (m 0/6/6, bg-mantle, r12) — общая glint-карта; .body 8/0/10+editor-bg
vs mx4/mt4/mb4; нет bodyFlush/retainLayer; сверх оригинала: breadcrumb h24 mono,
кнопка Save. Лимит 12 табов + эвикт — 1:1.

## Цикл 2: DIVERGES
Нет .viewer/.body (m 0/6/6 mantle r12; 8/0/10 editor-bg); +breadcrumb/Save сверх.

## Цикл 5: DIVERGES

Нет `.viewer` (m 0/6/6, bg-mantle, r12) и `.body` (8/0/10, editor-bg): у нас glint-карта с рамкой mx4/mt4/mb4. Сверх оригинала — breadcrumb h24 mono и кнопка «Save Ctrl+S». Нет bodyFlush/retainLayer.

## Цикл 6: DIVERGES

`.viewer`/`.body` не приведены; breadcrumb и «Save Ctrl+S» — сверх оригинала.

## Цикл 7: DIVERGES

Инсет карты: оригинал margin 0 6 6 + padding тела 8/0/10 против наших mx/mt/mb 4 без паддинга (замер: зазор 4.8 против 6). Сверх оригинала — breadcrumb 24 и кнопка Save.

---

## 109. file-viewer-empty — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](109-file-viewer-empty/original.png)
![наш](109-file-viewer-empty/ours.png)

### Оригинал

# 109 file-viewer-empty — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewer.tsx` (81-88), `FileViewer.module.css` (44-70)

## JSX-структура (кратко, вложенность)
```
div.empty
├─ i.codicon.codicon-file [aria-hidden]
└─ p: "Pick a file from the tree, or press <kbd>Ctrl+P</kbd> to open one by name."
```

## Метрики (ИЗ CSS, точные значения)
`.empty`:
- flex: 1; display: flex; flex-direction: column
- align-items: center; justify-content: center
- gap: var(--space-2)
- padding: var(--space-5)
- color: var(--text-muted); text-align: center

`.empty .codicon` (глиф файла):
- font-size: 36px
- color: var(--text-disabled)

`.empty kbd`:
- display: inline-block
- padding: 2px 6px
- background: var(--bg-surface); color: var(--text-primary)
- border-radius: var(--radius-xs)
- font-family: var(--font-mono); font-size: var(--fs-xs)
- border: 1px solid color-mix(in srgb, var(--text-muted) 30%, transparent)

## Состояния (классы-варианты с метриками)
Одно статическое состояние; hover/active/transition отсутствуют.

### Наша реализация

# 109 file-viewer-empty — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:5352-5357 (вызов), crates\shell\src\ui\panel_placeholder.rs:37-89 (glyph), 92-94 (slot_glyph), 103-144 (шаблон)

## Структура/содержание
```
panel_placeholder("File",
  "Click a file in any panel, or drag-and-drop one from outside",
  SlotIcon::Center, p)
└─ div.size_full.flex_col.items_center.justify_center.gap(8).p(20).overflow_hidden
   ├─ div.mb(4) └─ slot_glyph(Center) = glyph(scale 2.0), канва 28×24
   │     ├─ frame_rect: absolute left 2 top 2, 24×20, border_2, rounded 3
   │     └─ bar (Center): absolute left 9.5 top 3, 9×18, rounded 2
   ├─ label «File»
   └─ hint (text_center; max-width НЕТ)
```
Глиф — не SVG, а нативные div: канва 14×12 в исходных координатах PanelIcon.tsx, умноженная на scale 2.0 (`slot_glyph`, panel_placeholder.rs:92-94). Бар кламплется во внутреннюю область рамки (SLOT_INSET 1.5). При s ≥ 2.0 рамка рисуется `border_2` (2px), при s = 1.0 — `border_1`.

## Метрики (из кода, точные)
- отступы: контейнер p 20 (SPACE_5); обёртка глифа mb 4 (SPACE_1); у label и hint собственных padding/margin нет
- гэпы: контейнер gap 8 (SPACE_2)
- цвета: базовый цвет контейнера p.text_muted #838aa0; label p.text_primary #cfd4e2; hint p.text_muted #838aa0; рамка глифа p.text_muted #838aa0 (α 1.0); слот-бар p.text_muted α 0.85
- скругления: рамка глифа 3.0 (1.5 × scale 2.0); слот-бар 2.0 (1.0 × 2.0); у контейнера и текстов скруглений нет
- шрифты: label font-size 16 (FS_LG), font-weight 600 SEMIBOLD; hint font-size 12 (FS_SM), line-height 15.6 (12 × 1.3), font-weight 400; семейство — UI «Bricolage Grotesque» (наследуется от окна); моно-шрифта нет
- ховер: N/A: ховер — одно статичное состояние, ни одного `.hover(...)` в panel_placeholder.rs (совпадает с оригиналом)

## Отличия от original.md той же папки
1. Другой глиф: слот-рамка PanelIcon (Center) 28×24 вместо `codicon-file` 36px цветом `--text-disabled`.
2. Другой текст: «Click a file in any panel, or drag-and-drop one from outside» вместо «Pick a file from the tree, or press Ctrl+P to open one by name.»; подсказки про Ctrl+P и `<kbd>`-чипа (padding 2×6, bg-surface, text-primary, radius-xs, font-mono fs-xs, border text-muted 30%) нет вовсе.
3. Добавлен заголовок «File» fs 16 semibold — в оригинале только глиф + `<p>`.
4. Совпадают: gap 8 (space-2), padding 20 (space-5), flex-column + центрирование по обеим осям, text-align center, базовый цвет text-muted.
5. max-width у hint нет ни у нас, ни в CSS оригинала — совпадает (240 есть только в соседнем `activity_placeholder`, panel_placeholder.rs:189).
6. Добавлен `overflow_hidden` у контейнера (в CSS оригинала его нет).
7. `flex: 1` у `.empty` → у нас `size_full` внутри уже растянутой карты.

### Вердикты

# 109 — verdict (review cycle 1)
VERDICT: DIVERGES
Глиф slot-рамка vs codicon-file 36 text-disabled; текст другой; нет kbd-чипа Ctrl+P;
лишний заголовок File.

## Цикл 2: DIVERGES
panel_placeholder вместо .empty (codicon-file 36 + kbd Ctrl+P).

## Цикл 5: DIVERGES

Пустое состояние вьюера: у нас слотовый `panel_placeholder` (глиф 39×34 + «File» 16), у оригинала `.empty` = codicon-file 36 text-disabled + текст с `<kbd>Ctrl+P</kbd>`.

## Цикл 6: DIVERGES

Пустое состояние вьюера — слотовый placeholder вместо `.empty` с `<kbd>Ctrl+P</kbd>`.

## Цикл 7: DIVERGES

Пустое состояние: глиф 36 text-disabled, gap 8, pad 20, текст с kbd Ctrl+P → у нас общий panel_placeholder.

---

## 110. file-viewer-tabs-strip — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](110-file-viewer-tabs-strip/original.png)
![наш](110-file-viewer-tabs-strip/ours.png)

### Оригинал

# 110 file-viewer-tabs-strip — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewerTabs.tsx` (155-165, 196-199), `FileViewerTabs.module.css` (7-25, 156-169)

## JSX-структура (кратко, вложенность)
```
div.bar                                     (стрип + условная overflow-кнопка, №112)
├─ div.strip [ref] role=tablist aria-label="Open files" tabIndex=-1
│  │  onPointerMove / onPointerUp (pointer-reorder, НЕ HTML5 drag)
│  ├─ button.tab × N (№111, сортировка pinned-first, стабильный порядок)
│  └─ span.dropIndicator style="left: <x>px" [aria-hidden]   (только во время drag, over >= 0)
└─ (overflow && …) div.overflow (№112)
```
- Порог драга: `DRAG_THRESHOLD_PX = 4`; полусдвиг индикатора `GAP_HALF_PX = 2`.
- Overflow детектится по `scrollWidth > clientWidth + 1` (каждый рендер + ResizeObserver).
- `tabs.length === 0` → компонент возвращает null (стрипа нет).

## Метрики (ИЗ CSS, точные значения)
`.bar`: display: flex; align-items: center; flex-shrink: 0

`.strip`:
- position: relative; display: flex; align-items: center
- gap: var(--space-1)
- padding: 4px var(--space-2) (симметрично по вертикали)
- flex: 1; min-width: 0
- overflow: hidden; scrollbar-width: none; `::-webkit-scrollbar { display: none }`

`.dropIndicator`:
- position: absolute; top: 5px; bottom: 5px
- width: 2px; border-radius: 1px
- background: var(--accent-primary)
- pointer-events: none
- left задаётся inline в px

## Состояния (классы-варианты с метриками)
- Drag активен: перетаскиваемый таб получает `.tabDragging` (opacity: 0.3, см. №111), индикатор вставки показан.
- Overflow: появляется `.overflow`-блок с кнопкой ▾ (№112).
- hover/transition на самом стрипе отсутствуют.

### Наша реализация

# 110 file-viewer-tabs-strip — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\editor_tabs.rs:37-313 (editor_tabs_bar), crates\shell\src\root.rs:4330-4355 (вызов; ширина из probe-реестра «file-tabs», −16)

## Структура (gpui-дерево кратко)
```
div.bar: flex.items_center.gap(4).flex_shrink_0.px(8).pt(4).overflow_hidden
├─ tab × visible (№111)
└─ (hidden непусты) кнопка «N ▾» + deferred-меню (№112)
```
Раскладка: оценка ширины таба `tab_width_est = chars×6.5 + 50` (+4 gap); не влезшие в `available_w − 40` уходят в hidden; активный всегда видим (подмена последнего видимого). Drag-reorder: mouse-down → `TabPress(i,x,y)` (порог 4px разруливает root), зажатая ЛКМ над табом → `TabDragOver(i)`; цель вставки = `border_l_2 accent_primary` на самом табе.

## Метрики (из кода, точные)
- Полоса: gap 4 (SPACE_1 — совпадает), px 8 (SPACE_2), pt 4 (SPACE_1), pb НЕТ, overflow_hidden
- Индикатор вставки: левый бордер 2px p.accent_primary #89b4fa на целевом табе
- Порог драга 4px (root.rs, TabDrag.started)

## Отличия от original.md той же папки
1. Индикатор вставки — `border_l_2` на табе (сдвигает контент на 2px) вместо absolute-полосы 2×(h−10) rounded-1, позиционируемой в px.
2. Padding: у оригинала 4px сверху И снизу (`padding: 4px var(--space-2)`), у нас только pt 4.
3. Overflow-детект: оценка ширины по числу символов (6.5px/симв) vs реальный `scrollWidth > clientWidth + 1` + ResizeObserver — при пропорциональном шрифте оценка неточна.
4. Активный таб принудительно остаётся видимым подменой последнего (в оригинале стрип скроллится, scrollIntoView).
5. `.tabDragging` (opacity 0.3 у перетаскиваемого) не реализован — визуально драг показывает только индикатор цели.
6. Нет role=tablist / aria-label / tabIndex.
7. При 0 табов полоса не рендерится (ветка редактора не активна) — поведение совпадает.

## Дополнение атрибутов (цикл 10)

- скругления: таб border-radius 8 (RADIUS_SM) (editor_tabs.rs:102); close-кнопка border-radius 4 (RADIUS_XS) (editor_tabs.rs:202); dirty-точка rounded_full 6×6 (editor_tabs.rs:186); сама полоса `.bar` без скругления
- ховер: неактивный таб — bg p.bg_surface #3d3f51 α .5 + text p.text_primary #cfd4e2 (editor_tabs.rs:90,171); close-крестик: opacity 0 → 0.7 по group_hover таба (editor_tabs.rs:222), у активного 0.7 постоянно (editor_tabs.rs:220), собственный hover крестика — bg p.bg_overlay #515567 α .6 + opacity 1.0 (editor_tabs.rs:203-205); у активного таба hover нет (bg accent_primary α .16 фиксирован, editor_tabs.rs:167-169)

### Вердикты

# 110 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет pb4 (только pt); dropIndicator = border_l_2 vs absolute 2px top5/bottom5 r1;
overflow-детект эвристикой ширины.

## Цикл 2: DIVERGES
dropIndicator border_l vs absolute 2px; overflow-эвристика ширины.

## Цикл 5: DIVERGES

Стрип табов: `py 4` симметрично — исправлено. Остаток: индикатор вставки = `border_l_2` на табе вместо absolute 2×(h−10) r1 с inline-left; overflow определяется эвристикой `chars×6.5+50` вместо `scrollWidth > clientWidth+1`.

## Цикл 6: DIVERGES

Индикатор вставки `border_l_2` вместо absolute-полосы; overflow-детект эвристикой.

## Цикл 7: DIVERGES

dropIndicator (absolute top/bottom 5, w2, r1) против border_l_2; вставка в конец невыразима; overflow-детект по scrollWidth против оценки len*6.5+50.

---

## 111. file-viewer-tab — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](111-file-viewer-tab/original.png)
![наш](111-file-viewer-tab/ours.png)

### Оригинал

# 111 file-viewer-tab — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewerTabs.tsx` (166-195), `FileViewerTabs.module.css` (83-173)

## JSX-структура (кратко, вложенность)
```
button.tab [.tabActive] [.tabDragging] role=tab data-tab-path aria-selected data-tooltip={полный путь}
├─ (pinned) i.codicon.codicon-pinned.pinIcon
├─ <TabIcon>.tabIcon      (webview → иконка расширения-владельца / файл → TreeIcon)
├─ span.label             (basename / live-title webview-панели)
├─ (dirty) span.dirty "●" aria-label="Unsaved changes"
└─ button.close aria-label="Close" data-tooltip="Close"|"Discard & close"
   └─ i.codicon.codicon-close
```
- pointerdown (левая) → press-bookkeeping; выбор — на pointerup стрипа (клик без сдвига ≥4px = select).
- middle-click (`onAuxClick` button===1) закрывает; right-click — контекст-меню (Close / Close Others / Close to the Right / Close All + файловое меню, для webview `builtin:false`).

## Метрики (ИЗ CSS, точные значения)
`.tab`:
- display: inline-flex; align-items: center; gap: 6px
- padding: 4px 6px 4px 10px; height: 24px
- background: transparent; border: none
- border-radius: var(--radius-sm)
- color: var(--text-secondary)
- font-size: 11px; font-weight: 500; letter-spacing: 0.02em
- white-space: nowrap; cursor: pointer; flex-shrink: 0
- transition: background var(--transition-fast), color var(--transition-fast)

`.tabIcon`: flex-shrink: 0; width: 14px; height: 14px
`.label`: white-space: nowrap (без усечения)
`.dirty`: color: var(--accent-orange); font-size: 10px; line-height: 1
`.pinIcon`: font-size: 11px; opacity: 0.7

`.close`:
- width: 16px; height: 16px; inline-flex центр; padding: 0
- background: transparent; border: none; border-radius: var(--radius-xs)
- color: inherit; opacity: 0
- transition: opacity var(--transition-fast), background var(--transition-fast)
- `.close .codicon`: font-size: 11px

## Состояния (классы-варианты с метриками)
- `.tab:hover`: background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)
- `.tabActive`, `.tabActive:hover`: background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary) — без рамки/кольца
- `.tabDragging`: opacity: 0.3
- `.tab:hover .close`, `.tabActive .close`: opacity: 0.7
- `.close:hover`: opacity: 1; background: color-mix(in srgb, var(--bg-overlay) 60%, transparent)

### Наша реализация

# 111 file-viewer-tab — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\editor_tabs.rs:85-209 (таб), 315-440 (RMB-меню editor_tab_menu)

## Структура (gpui-дерево кратко)
```
div#ftab-{i}: flex.items_center.gap(6).h(24).pl(8).pr(4).rounded(8)
├─ file_img (Catppuccin-иконка 14×14)
├─ (pinned) fa "\u{f08d}" (thumbtack) 11px text_muted
├─ name (basename)
└─ dirty ? div 14×14 c кругом 6×6 accent_orange
        : div#ftabx-{i} 16×16 rounded(3) codicon close "\u{ea76}" 11px
tooltip = полный путь; middle-click → CloseEditorTab; RMB → editor_tab_menu
```
RMB-меню (в overlay, w 220): Pin/Unpin Tab, Close, Close Others, Close to the Right, Close All, разделитель, «File actions…» (переход в меню дерева по path).

## Метрики (из кода, точные)
- Таб: h 24, gap 6, pl 8 (SPACE_2), pr 4, rounded 8 (RADIUS_SM), fs 11, weight 500 (MEDIUM), цвет p.text_secondary #adb3c7
- Hover: bg p.bg_surface #3d3f51 a=.5 + text p.text_primary #cfd4e2
- Активный: bg p.accent_primary #89b4fa a=.16 + text p.text_primary (hover не меняет)
- Иконка файла: 14×14; pin: FA f08d 11px p.text_muted #838aa0
- Dirty: круг 6×6 p.accent_orange #fab387 в боксе 14×14
- Close: 16×16, rounded 3, цвет p.text_muted; hover bg p.text_primary a=.12 + text_primary; глиф 11px; tooltip «Close»
- Меню таба: w 220, rounded 12, bg p.bg_surface, border p.text_primary a=.06, p 4; item px 12 py 4 rounded 8 fs 12, hover text_primary a=.08

## Отличия от original.md той же папки
1. Dirty — нарисованный круг 6px accent_orange вместо текстового «●» 10px (визуально близко).
2. Close всегда видим (в оригинале opacity 0 → 0.7 на hover таба → 1 на hover кнопки); hover-фон close: text_primary 12% vs bg-overlay 60%; rounded 3 vs radius-xs 4.
3. Pin-иконка — FontAwesome thumbtack (f08d) text_muted вместо `codicon-pinned` c opacity .7.
4. Padding: pl 8 / pr 4 vs оригинал 4px 6px 4px 10px (слева 10, справа 6); letter-spacing 0.02em отсутствует; transition отсутствуют.
5. RMB-меню содержит Pin/Unpin и «File actions…» (в оригинале pin в самом меню файла; сравнить состав), рендерится в overlay-окне.
6. Меток webview-панелей нет (webview — не таб), TabIcon-ветки для расширений нет.
7. Select — на mouse-up без сдвига (порог 4px в root) — совпадает с оригиналом; middle-click close — совпадает.

### Вердикты

# 111 — verdict (review cycle 1)
VERDICT: DIVERGES
pl8/pr4 vs 10/6; нет ls .02em; порядок pin/icon/label; dirty XOR close (оригинал ОБА);
close всегда видим (нет 0-.7-1 + bg-overlay60%), r3 vs 4; нет tabDragging .3.
h24/gap6/fs11/hover/active — 1:1.

## Цикл 2: DIVERGES
Нет ls .02em; нет tabDragging .3; тултип close не «Discard & close» при dirty.

## Цикл 5: DIVERGES

Таб вьюера: pl10/pr6, pin `codicon-pinned` op .7, dirty И close вместе, close 0→.7→1 + overlay 60%, r-xs — исправлено. Остаток: нет `.tabDragging` opacity .3; тултип close всегда «Close» (у оригинала «Discard & close» при dirty); letter-spacing .02em — ограничение gpui.

## Цикл 6: DIVERGES

Нет `.tabDragging`; тултип close без варианта «Discard & close» при dirty.

## Цикл 7: DIVERGES

Нет tabDragging opacity .3; тултип close всегда «Close» вместо «Discard & close»; dirty квад 6x6 вместо глифа fs10.

---

## 112. file-viewer-tabs-overflow — **DIVERGES** (цикл 11)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES, ц11:DIVERGES*

![оригинал](112-file-viewer-tabs-overflow/original.png)
![наш](112-file-viewer-tabs-overflow/ours.png)

### Оригинал

# 112 file-viewer-tabs-overflow — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewerTabs.tsx` (200-232), `FileViewerTabs.module.css` (28-81)

## JSX-структура (кратко, вложенность)
```
div.overflow [ref]                          (рендерится только когда scrollWidth > clientWidth + 1)
├─ button.overflowBtn aria-label="Open files menu" data-tooltip="More open files" aria-expanded
│  └─ i.codicon.codicon-chevron-down
└─ (menuOpen) div.overflowMenu role=menu
   └─ button.overflowItem [.overflowItemActive] role=menuitem title={путь} × N
      ├─ (pinned) i.codicon.codicon-pinned.pinIcon
      ├─ <TabIcon>.tabIcon
      ├─ span.overflowLabel
      └─ (dirty) span.dirty "●"
```
- Клик по item → выбрать таб + scrollIntoView в стрипе; закрытие по mousedown вне / Escape.

## Метрики (ИЗ CSS, точные значения)
`.overflow`: position: relative; flex-shrink: 0; padding-right: var(--space-1)

`.overflowBtn`:
- inline-flex центр; width: 24px; height: 24px
- border: none; border-radius: var(--radius-sm)
- background: transparent; color: var(--text-secondary); cursor: pointer

`.overflowMenu`:
- position: absolute; top: calc(100% + 2px); right: 0; z-index: 30
- min-width: 200px; max-width: 360px; max-height: 60vh; overflow-y: auto
- padding: var(--space-1)
- border-radius: var(--radius-md)
- background: var(--bg-surface); border: 1px solid var(--divider-soft)
- box-shadow: 0 6px 24px rgb(0 0 0 / 30%)

`.overflowItem`:
- display: flex; align-items: center; gap: 6px; width: 100%
- padding: 5px 8px; border: none; border-radius: var(--radius-sm)
- background: transparent; color: var(--text-secondary)
- font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer

`.overflowLabel`: flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis

## Состояния (классы-варианты с метриками)
- `.overflowBtn:hover`: background: var(--bg-surface-hover); color: var(--text-primary)
- `.overflowItem:hover`: background: var(--bg-surface-hover); color: var(--text-primary)
- `.overflowItemActive`, `.overflowItemActive:hover`: background: color-mix(in srgb, var(--accent-primary) 16%, transparent); color: var(--text-primary)

### Наша реализация

# 112 file-viewer-tabs-overflow — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\editor_tabs.rs:229-343; закрытие по Escape — crates\shell\src\root.rs:5857-5858, закрытие кликом-мимо — root.rs:6020-6042; состояние — root.rs:224, 1988-1989

## Структура/содержание
```
div#ftabs-overflow (relative, flex_shrink_0, h 24): «{N}» + codicon chevron-down 12
└─ (overflow_open) deferred(menu, priority 60)
   div.occlude.absolute.top(28).right(0).min_w(200).max_w(360).max_h(400).overflow_hidden
   └─ div#ftov-{i} × скрытые табы:
      ├─ file_img 14×14 (Catppuccin-иконка)
      ├─ label: flex_1.min_w(0).overflow_hidden.text_ellipsis.whitespace_nowrap
      └─ (dirty) круг 6×6 rounded_full accent_orange
```
Клик по кнопке → `ToggleFileTabsOverflow`; клик по пункту → `SelectEditorTab(i)` + `ToggleFileTabsOverflow`. Escape (`CloseOverlay`, root.rs:5857) и mouse-down мимо меню (root.rs:6027, 6038) тоже закрывают.

## Метрики (из кода, точные)
- отступы: кнопка px 6 при h 24 (py нет); меню p 4 (SPACE_1); пункт px 8 (SPACE_2) / py 5
- гэпы: кнопка gap 2; меню gap 1 между пунктами; пункт gap 6
- цвета: кнопка p.text_secondary #adb3c7; меню bg p.bg_surface #3d3f51, border 1px p.text_primary #cfd4e2 α 0.06, shadow 0 6 24 rgba(0,0,0,0.30); пункт p.text_secondary #adb3c7; dirty-круг p.accent_orange #fab387
- скругления: кнопка 8 (RADIUS_SM); меню 12 (RADIUS_MD); пункт 8 (RADIUS_SM); dirty — rounded_full
- шрифты: кнопка font-size 12 (FS_SM), weight 400; chevron — codicon 12; пункт font-size 12 (FS_SM), weight 400; собственных шрифтовых правил у меню нет
- фоны по ховеру: кнопка — p.text_primary α 0.08 + текст p.text_primary #cfd4e2; пункт — p.bg_surface_hover #3b3b52 (сплошной), цвет текста НЕ меняется

## Отличия от original.md той же папки
1. Кнопка показывает СЧЁТЧИК скрытых («N ▾») и имеет размер по содержимому (h 24, px 6); у оригинала — квадрат 24×24 только с chevron.
2. Кнопка: hover-фон text-primary 8% вместо `--bg-surface-hover`, и это единственное расхождение в самой кнопке; тултипа «More open files» и aria-label нет.
3. Меню: min-w 200 / max-w 360 / p 4 / radius-md / bg-surface / shadow 0 6 24 30% — совпадают с оригиналом 1:1. Бордер `text_primary α .06` = `--divider-soft` (color-mix text-primary 6%) — тоже совпадает.
4. max-height 400px фикс вместо `60vh`, и `overflow-y: auto` не портирован: у нас `overflow_hidden` — лишние пункты обрезаются, прокрутки в меню НЕТ.
5. `top: 28` фикс вместо `calc(100% + 2px)`; z-index 30 → `deferred(priority 60)`.
6. Пункт: gap 6, px 8, py 5, radius-sm, fs 12, text-secondary — совпадают 1:1; hover bg-surface-hover совпадает, но цвет текста на hover НЕ поднимается до `--text-primary`.
7. Активный пункт не подсвечен: `.overflowItemActive` (accent-primary 16% + text-primary) не портирован.
8. Pin-иконка в пункте отсутствует (оригинал показывает `codicon-pinned`).
9. `title={путь}` у пункта нет (у нас тултипа на пунктах меню нет вовсе).
10. Dirty-точка — accent-orange, как у оригинала и как у нашего таба №111 (прежнее расхождение с accent-primary устранено).
11. `.overflow { padding-right: var(--space-1) }` у контейнера кнопки не портирован.
12. Escape и mousedown-вне закрывают меню — совпадает с оригиналом; scrollIntoView выбранного таба не нужен (активный таб принудительно видим, №110).

### Вердикты

# 112 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет shadow 0/6/24/30%; w240/max-h400 vs min200-max360/60vh; item 12/4 gap8 primary
vs 8/5 gap6 secondary; hover tint vs bg-surface-hover; нет overflowItemActive и pin;
кнопка «N v» vs 24×24 chevron. dirty-orange/Esc/клик-вне/бордер — подтверждены.

## Цикл 2: DIVERGES
max-h 400 vs 60vh; нет overflowItemActive и pin в item; триггер «N ▾» vs 24×24 chevron+тултип; border text-primary6% vs divider-soft (то же значение — ок фактически).

## Цикл 5: DIVERGES

Overflow-меню табов: min-w200/max-w360, shadow 0/6/24/.3, item 5/8 gap6, dirty-точка — исправлено. Остаток: триггер должен быть «N ▾» (h24 px6 + счётчик), у нас 24×24 chevron; нет `.overflowItemActive` (accent 16%) и pin-иконки в пункте; `max_h 400` вместо 60vh; `top 28` вместо `calc(100% + 2px)`.

## Цикл 6: DIVERGES

`max_h 400` вместо 60vh, `top 28` вместо `calc(100%+2px)`, нет active-подсветки и pin-иконки. Уточнение ц.6: триггер у оригинала — 24×24 chevron с тултипом «More open files», у нас «N ▾» без тултипа (цикл 5 сформулировал направление наоборот).

## Цикл 7: DIVERGES

max-height 60vh против 400; top 26 против 28; нет overflowItemActive и pinIcon; триггер 24x24 chevron + тултип против «N» без тултипа.

## Цикл 11: DIVERGES

Закрыто: `max-height` считается от вьюпорта (60vh) вместо фиксированных 400 и у меню появилась прокрутка — раньше длинный список просто обрезался.

Осталось: сверить кадром; `ours.md` переписан по факту кода (min-w 200 / max-w 360, item 8/5, dirty accent-orange, Escape и клик-мимо закрывают).

---

## 113. monaco-editor-host — **DIVERGES** (цикл 8)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES, ц8:DIVERGES*

![оригинал](113-monaco-editor-host/original.png)
![наш](113-monaco-editor-host/ours.png)

### Оригинал

# 113 monaco-editor-host — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/MonacoEditor.tsx` (168-349, рендер 345-348), `MonacoEditor.module.css`; опции создания редактора — `MonacoEditor.tsx:183-208` (+ `monaco-loader.ts`)

## JSX-структура (кратко, вложенность)
```
error ? div.error "Failed to open: {error}"
      : div.host [ref]        (Monaco монтируется в него императивно)
```

## Метрики (ИЗ CSS, точные значения)
`.host`: flex: 1; min-height: 0

`.host .monaco-editor .scrollbar .slider` (:global):
- border-radius: var(--radius-xs) (только геометрия; цвета — через темы scrollbarSlider.*)

`.error`:
- flex: 1; display: flex; align-items: center; justify-content: center
- padding: var(--space-5)
- color: var(--accent-red)
- font-family: var(--font-mono); font-size: var(--fs-sm)

## Опции Monaco (из TSX — определяют вид редактора)
- automaticLayout: true; scrollBeyondLastLine: false; smoothScrolling: true
- stickyScroll: { enabled: true }; minimap: { enabled: true }
- scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 }
- fixedOverflowWidgets: true (hover/suggest в body-слое, не клипаются карточкой)
- largeFileOptimizations: true; maxTokenizationLineLength: 20000
- fontFamily: getComputedStyle(documentElement) `--font-mono` || "monospace"
- fontSize: 13

## Состояния (классы-варианты с метриками)
- `.error`: показывается вместо `.host` при неудачном чтении файла.
- hover/transition в CSS-модуле отсутствуют (всё внутри Monaco).

### Наша реализация

# 113 monaco-editor-host — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:4316-4354 (создание `InputState::code_editor` + LSP + EOL + зеркало минимапы), root.rs:5267-5348 (рендер рамки, breadcrumb, редактора, минимапы, скроллбара), root.rs:2915 (`editor_lang`); crates\shell\src\ui\editor_minimap.rs:22-36, 86-276 (минимапа), 281-... (скроллбар); crates\shell\src\ui\sticky_scroll.rs:17-19; crates\metrics\src\lib.rs:47; редактор — vendored gpui-component Input: vendor\gpui-component\src\input\ (input.rs:256 `LINE_HEIGHT`, element.rs — рендер, mode.rs — tree-sitter highlighter)

## Структура/содержание
Monaco НЕТ. Вместо него — gpui-component `Input` в режиме `code_editor(language)` (vendored, с нашими патчами).
```
div.flex_col.flex_1.mx(4).mt(4).mb(4).rounded(12).overflow_hidden.bg(editor_bg)
├─ breadcrumb: div.h(24).px(12).text_size(11).font_family("JetBrains Mono").text_muted
│                └─ путь с ~-сокращением домашней папки, ellipsis + nowrap
└─ div.flex.flex_1
   ├─ div.relative.flex_1.min_w(0).h_full.font_family("JetBrains Mono").text_size(13)
   │  ├─ Input::new(input).h_full().appearance(false).hide_scrollbar()
   │  └─ children(sticky) — наш sticky-scroll оверлей
   ├─ editor_minimap::minimap(input, minimap_input, p)   — 67px
   └─ editor_minimap::scrollbar(input, p)                — 12px
```
- Язык по расширению (`editor_lang`, root.rs:2915), подсветка tree-sitter, номера строк встроены в `code_editor`.
- LSP: `HostLsp::new(path, lang)` → `st.lsp.hover_provider` / `definition_provider` (root.rs:4321-4329).
- EOL детект при открытии: `text.contains("\r\n")` → CRLF (root.rs:4318), уходит в статус-бар (№119).
- Минимапа — ЗЕРКАЛЬНЫЙ `InputState` (`.line_number(false).minimap()`, root.rs:4341-4354) поверх канваса с thumb; порядок колонок как в Zed: текст → минимапа → скроллбар.
- LRU-лимит табов: `MAX_EDITOR_TABS`, вытесняется самый давний чистый (root.rs:4372-4386).

## Метрики (из кода, точные)
- отступы: рамка редактора mx 4 (SPACE_1) / mt 4 / mb 4 (SPACE_1); breadcrumb h 24 + px 12 (SPACE_3); у самого `Input` padding нет (flex_1 + h_full); минимапа и скроллбар — соседние колонки без отступов
- гэпы: N/A: гэпы — три колонки (текст / минимапа 67 / скроллбар 12) идут подряд, `gap` не задан
- цвета: фон рамки p.editor_bg #1d1c25; breadcrumb p.text_muted #838aa0; thumb минимапы p.text_primary #cfd4e2 α 0.08 + левая полоска 1px p.text_primary α 0.16; скроллбар — трек-бордер p.text_primary α 0.06, thumb p.bg_overlay #515567 α 0.55
- скругления: рамка редактора 12 (RADIUS_MD); у `Input`, минимапы, скроллбара и thumb скруглений нет
- шрифты: обёртка редактора — семейство «JetBrains Mono» + `text_size(px(m::EDITOR_FONT_SIZE))` = 13 (root.rs:5323-5326, ПРИМЕНЯЕТСЯ); высота строки 20 (`LINE_HEIGHT = Rems(1.25)` при rem 16, input.rs:256; `sticky_scroll::EDITOR_LINE_H = 20.0`); breadcrumb «JetBrains Mono» 11 (FS_XS); минимапа text_size 2.0 (`MM_FONT`) с line-height 3.08 (`MM_LINE_H = MM_FONT × ED_LINE_H / ED_FONT = 2 × 20/13`)
- ховер: N/A: ховер — ни у обёртки редактора, ни у минимапы, ни у скроллбара нет `.hover(...)`; реакции только на wheel / mouse-down / drag
- прочие константы: `MM_WIDTH` 67, `MIN_THUMB` 25, скроллбар `SB_W` 12, `MAX_STICKY` 5

## Отличия от original.md той же папки
1. Полная замена движка: gpui-component `Input` (rope + tree-sitter) вместо Monaco — опции `automaticLayout`, `smoothScrolling`, `fixedOverflowWidgets`, `largeFileOptimizations`, `maxTokenizationLineLength`, `scrollBeyondLastLine` неприменимы.
2. `.error` («Failed to open: {error}», accent-red, font-mono fs-sm, padding space-5) НЕ РЕАЛИЗОВАН — сбой чтения файла отдельного вью не даёт.
3. `fontSize: 13` совпадает: `m::EDITOR_FONT_SIZE` = 13 применяется к обёртке редактора (root.rs:5326).
4. `fontFamily` у оригинала берётся из `getComputedStyle --font-mono`; у нас семейство «JetBrains Mono» проставлено строкой в двух местах (root.rs:5302, 5323) — фоллбеков нет.
5. stickyScroll: у оригинала семантический Monaco; у нас — свой indentation-оверлей, максимум 5 строк (`MAX_STICKY`).
6. minimap: у оригинала Monaco-минимапа; у нас — зеркальный `Input` шириной 67px + канвас-thumb с клик-центрированием и драг-слежением (порт Zed).
7. Скроллбар: свой, 12px, трек с бордером слева, thumb min 25px — вместо Monaco `verticalScrollbarSize: 8` / `horizontalScrollbarSize: 8` со slider `border-radius: var(--radius-xs)`; горизонтального скроллбара нет (`soft_wrap(false)`, прокрутка по X — колесом).
8. Hover/suggest-виджеты — LSP-поповеры vendored input (input/popovers), а не Monaco-виджеты в body-слое.
9. Добавлен breadcrumb-ряд внутри рамки редактора (h 24, mono 11, text-muted) — в оригинальном `MonacoEditor` его нет.
10. `line-height` в original.md не указан (задаётся Monaco по fontSize) — сравнить нечем; у нас жёстко 20px.

### Вердикты

# 113 — verdict (review cycle 1)
VERDICT: DIVERGES
Monaco vs gpui Input (опции n/a); нет .error-вью; fontSize 13 не применён к Input.

## Цикл 2: DIVERGES
gpui Input vs Monaco; нет .error-вью; fontSize 13 не задан.

## Цикл 5: DIVERGES

Хост редактора: gpui `Input` вместо Monaco (опции неприменимы); нет состояния `.error` «Failed to open: …»; `EDITOR_FONT_SIZE` не используется нигде в shell — размер текста наследуется от окна вместо 13.

## Цикл 6: DIVERGES

`EDITOR_FONT_SIZE` объявлен, но не используется; нет `.error`-вью.

## Цикл 7: DIVERGES

Кегль редактора 14 против 13; константа EDITOR_FONT_SIZE=13 объявлена и не используется. Нет error-вью.

## Цикл 8: DIVERGES

Закрыто: кегль редактора 13 — обёртка ставит `text_size(m::EDITOR_FONT_SIZE)`, и `Input`
наследует его вместо оконных 14; константа перестала быть мёртвой. Минимапа
пересчитана от новой базы (`ED_FONT = EDITOR_FONT_SIZE`, `mm_line = 2 × 20/13 ≈ 3.08`).

Осталось: движок — gpui-component `Input` вместо Monaco (осознанная замена); нет
error-вью «Failed to open: …».

---

## 114. webview-panel-view — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](114-webview-panel-view/original.png)
![наш](114-webview-panel-view/ours.png)

### Оригинал

# 114 webview-panel-view — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/WebviewPanelView.tsx` (рендер 369-387), `WebviewPanelView.module.css`; ретрай-карточка — `panel-placeholder/WebviewLoadingSkeleton.tsx:65-76` (`WebviewLoadError`, стили из её module.css)

## JSX-структура (кратко, вложенность)
```
div.container
├─ iframe.frame [ref] sandbox="allow-scripts allow-forms" src={kaminwebview://…} title="Extension webview" onLoad
└─ stalled && !loaded && !painted
   ? <WebviewLoadError onRetry>                          (карточка с retry)
   :  div.loader [.loaderHidden при painted] aria-hidden
      └─ div.spinner
```
- HTML сервится с `http://kaminwebview.localhost` (Windows/WebView2); НЕ srcdoc.
- Cover держится до `__kaminReady` постмессаджа, fallback `READY_FALLBACK_MS = 1200` мс.
- Load-watchdog `LOAD_WATCHDOG_MS = 20000` мс → stalled → ретрай-карточка.
- Crash-watchdog: ping каждые `CRASH_PING_INTERVAL_MS = 4000` мс, `CRASH_MISS_LIMIT = 8` подряд (~32с) → reload; `BUSY_GRACE_MS = 180000`.
- WebviewLoadError: `div.errWrap[role=alert] > i.fas.fa-triangle-exclamation.errIcon + div.errTitle "This panel didn't load" + div.errHint + button.retry (fa-rotate + "Retry")`.

## Метрики (ИЗ CSS, точные значения)
`.container`: position: relative; width: 100%; height: 100%

`.frame`:
- width: 100%; height: 100%; border: none; display: block
- background: transparent

`.loader`:
- position: absolute; inset: 0; z-index: 2
- display: flex; align-items: center; justify-content: center
- background: var(--bg-surface, var(--editor-bg, #22222e))
- opacity: 1; transition: opacity 180ms ease
- pointer-events: none

`.loaderHidden`: opacity: 0

`.spinner`:
- width: 22px; height: 22px; border-radius: 50%
- border: 2.5px solid color-mix(in srgb, var(--text-primary, #cdd6f4) 16%, transparent)
- border-top-color: var(--accent-action, var(--accent-primary, #d77757))
- animation: kaminWvSpin 0.7s linear infinite (`to { transform: rotate(360deg) }`)

## Состояния (классы-варианты с метриками)
- loading: `.loader` opacity 1 поверх iframe (z-index 2).
- painted: `.loader.loaderHidden` opacity 0 (fade 180ms).
- stalled (watchdog 20s, не loaded/painted): вместо loader — `WebviewLoadError` ретрай-карточка.
- hidden retained-панель: управляется родителем (№108, display: none) — компонент остаётся смонтирован.

## Дополнение атрибутов (цикл 10)

- отступы: `.container`/`.frame` padding и margin не задают (WebviewPanelView.module.css:1-16), работает глобальный сброс `* { margin: 0; padding: 0 }` (global.css:12); `.loader` — inset 0, padding нет (WebviewPanelView.module.css:24-25); ретрай-карточка `.errWrap` — inset 0 + padding 24px (WebviewLoadingSkeleton.module.css:99,105), `.errIcon` margin-bottom 4px (:114), `.retry` padding 6px 16px (:134)

### Наша реализация

# 114 webview-panel-view — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:2762-2824 (webview_panel/webview_body), 2844-2911 (webview_body_dyn), 3703-3779 (показ/скрытие живых вебвью), crates\shell\src\ui\chat_webview.rs (build_webview/stage_html/has_html)

## Структура (gpui-дерево кратко)
Вебвью у нас — ПАНЕЛЬНЫЕ (слоты layout), не редакторские табы. Два хостинга:
```
1) visual hosting (wv_visual, Windows): composition-визуал WebView2 + канвас-зона
   → visual_wv_body(id) (дыры в фонах gpui, скругление угловыми масками)
2) обычный: Entity<gpui_component::webview::WebView> (WebView2-чайлд-окно)
div (match):
├─ has_html && alive → div#id.relative.size_full [probe_area] + wv
├─ has_html && !alive → центр: codicon "\u{eb19}" 22px accent_primary + «Loading…»
└─ иначе → panel_placeholder («Open new tool or drag-n-drop tool from other panels»)
```
`webview_body_dyn` (contributed-тулы): + px 8 / pb 8 воздух, подложка rounded 12 bg editor_bg под вебвью.
HTML: `stage_html` пишет в cache-файл (`webview-html/{id}.html`) и грузит file-URL; `alive` — по `WebviewAlive`-пингу из вебвью.

## Метрики (из кода, точные)
- Loading-состояние: gap 8, глиф 22px p.accent_primary #89b4fa, текст fs 13 (FS_MD) p.text_secondary #adb3c7
- Дин-вебвью: px 8, pb 8, подложка rounded 12 (RADIUS_MD) bg p.editor_bg #1d1c25
- Карта — glint-рамка (glint_surface_wv_holed)

## Отличия от original.md той же папки
1. Не iframe: WebView2 (чайлд-окно или composition-визуал) — sandbox="allow-scripts allow-forms" и `kaminwebview://`/`http://kaminwebview.localhost` схемы отсутствуют; HTML грузится из staged-файла.
2. Fade-cover со спиннером (opacity-transition 180ms, spinner 22px 0.7s) не реализован — вместо него статичное «Loading…» с глифом (без анимации вращения).
3. Load-watchdog 20s, crash-ping 4s×8, BUSY_GRACE и ретрай-карточка WebviewLoadError («This panel didn't load» + Retry) — НЕ РЕАЛИЗОВАНЫ.
4. `__kaminReady`/READY_FALLBACK_MS-логики нет; готовность = `WebviewAlive`-пинг.
5. Retained-панели: вебвью живут в HashMap постоянно; скрытие — прятание чайлд-окон/визуалов (root.rs:3703-3760), а не display:none у слоя.
6. Скругление и «дыра» в фонах — angular-маски glint-канваса (visual hosting), у оригинала — обычный CSS.

### Вердикты

# 114 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет loader/spinner (22×22 border 2.5 accent-action, spin .7s) + fade 180ms; нет
WebviewLoadError+Retry; нет watchdogs 20s / 4s×8 / 180s / 1200ms (__kaminReady).

## Цикл 2: DIVERGES
loader/spinner/Retry/watchdogs отсутствуют.

## Цикл 5: DIVERGES

Вебвью-панель: нет loader-cover (spinner 22×22, border 2.5, top-color accent-action, 0.7s) и fade 180 мс; нет `WebviewLoadError` + Retry; нет вотчдогов 20s / 4s×8 / 180s / `__kaminReady` 1200 мс. Вместо всего — статичное «Loading…».

## Цикл 6: DIVERGES

Нет loader-cover, fade, Retry и вотчдогов — статичное «Loading…».

## Цикл 7: DIVERGES

Loader-cover со спиннером, fade 180ms, Retry и два вотчдога против статического глифа 22 + Loading.

---

## 115. webview-tab-icon — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](115-webview-tab-icon/original.png)
![наш](115-webview-tab-icon/ours.png)

### Оригинал

# 115 webview-tab-icon — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/WebviewTabIcon.tsx` (29-38); css-модуля нет — размеры даёт класс потребителя (`FileViewerTabs.module.css` `.tabIcon`: width/height 14px)

## JSX-структура (кратко, вложенность)
```
extId неизвестен | иконки нет | ещё грузится:
  i.codicon.codicon-browser {className} [aria-hidden]      (fallback)
иконка загружена (data URL):
  img {className} src={dataUrl} alt=""
```
- `extId` = `webviewPanels.find(p.id === id)?.ownerExtId`.
- Резолв через `hostRpc.extensions.icon(extId)` (тот же эндпоинт, что Extensions panel); data URL кешируется module-wide (signal `iconCache`), повторный fetch не делается; ошибка → удаление из `requested` для ретрая на следующем рендере.

## Метрики (ИЗ CSS, точные значения)
Собственных стилей нет. В контексте таба (№111) получает `.tabIcon`:
- flex-shrink: 0; width: 14px; height: 14px

## Состояния (классы-варианты с метриками)
- fallback (codicon-browser) ↔ img — переключение по факту загрузки data URL; визуальных hover/active нет.

## Дополнение атрибутов (цикл 10)

- цвета: собственных нет — `<img>` рисует data-URL иконки расширения как есть; codicon-fallback наследует `color` таба = `var(--text-secondary)` #adb3c7 (FileViewerTabs.module.css:92), на hover и у активного таба — `var(--text-primary)` #cfd4e2 (:102-109)
- отступы: N/A: отступы — у компонента нет своего CSS-модуля; в табе расстояние даёт `.tab { gap: 6px }` (FileViewerTabs.module.css:86), а `.tabIcon` задаёт только flex-shrink 0 и width/height 14px (:116-120)

### Наша реализация

# 115 webview-tab-icon — наша реализация
Файлы: —

НЕ РЕАЛИЗОВАНО.

Webview-панели у нас не открываются как редакторские табы (нет путей `webview://<id>` в editor_tabs), поэтому иконка таба вебвью-панели отсутствует как элемент. Резолв иконки расширения-владельца (`hostRpc.extensions.icon`) нигде не вызывается для табов; редакторские табы используют только Catppuccin-иконки файлов (`crate::icon_theme::file_img`, editor_tabs.rs:148). Fallback `codicon-browser` тоже не используется.

## Отличия от original.md той же папки
Элемент целиком отсутствует: ни img с data-URL иконки расширения, ни codicon-browser fallback, ни кеша иконок.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — элемент отсутствует целиком (вебвью-панели не открываются как редакторские табы), ховерить нечего
- отступы: N/A: отступы
- гэпы: N/A: гэпы
- цвета: N/A: цвета

### Вердикты

# 115 — verdict (review cycle 1)
VERDICT: DIVERGES
Элемент отсутствует: webview:// в editor_tabs нет, иконки расширения не резолвятся.

## Цикл 2: DIVERGES
webview://-табы не реализованы.

## Цикл 5: DIVERGES

Иконка таба вебвью отсутствует: `grep "webview://"` по shell пуст, иконка расширения не резолвится, fallback codicon-browser нет.

## Цикл 6: DIVERGES

Иконка таба вебвью не реализована.

## Цикл 7: DIVERGES

WebviewTabIcon не портирован; вебвью-табов нет вовсе, всегда иконка файла из темы.

---

## 116. status-bar-root — **MATCH** (цикл 6)

*История: ц2:MATCH, ц5:MATCH, ц6:MATCH*

![оригинал](116-status-bar-root/original.png)
![наш](116-status-bar-root/ours.png)

### Оригинал

# 116 status-bar-root — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (29-48), `StatusBar.module.css` (1-21)

## JSX-структура (кратко, вложенность)
```
footer.statusBar
├─ div.left
│  ├─ Item "N active"  (icon circle-filled, tone ok)
│  ├─ (failed > 0)   Item "N failed" (icon warning, tone warn)
│  ├─ (disabled > 0) Item "N off"    (icon circle-slash)
│  ├─ Item "N cmds"  (icon symbol-keyword)
│  └─ ContributedItem × N (alignment Left=1, sort priority desc)
└─ div.right
   ├─ ContributedItem × N (alignment Right, sort priority asc)
   ├─ <EditorEncodingItems /> (№119)
   └─ <VersionUpdateItem />   (№120)
```

## Метрики (ИЗ CSS, точные значения)
`.statusBar`:
- height: var(--layout-status-bar-height)
- background: transparent (без бордера; градиент appWrapper просвечивает)
- display: flex; align-items: stretch
- font-size: var(--fs-xs); color: var(--text-muted)
- padding: 0 var(--space-2); gap: var(--space-1)

`.left`, `.right`:
- display: flex; align-items: stretch
- gap: 2px (умышленно плотнее space-1)

`.right`: margin-left: auto

## Состояния (классы-варианты с метриками)
Контейнер статичен; состояния — у item'ов (№117-120).

### Наша реализация

# 116 status-bar-root — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:160-280 (status_bar), crates\shell\src\root.rs:5370-5380 (вызов), crates\metrics\src\lib.rs:20

## Структура (gpui-дерево кратко)
```
div#status-bar: relative.flex_shrink_0.h(24).w_full.flex.items_center.px(8).gap(4)
├─ probe_area("status-bar")
├─ left: flex.items_center.gap(2)
│  ├─ item «N active» (circle-filled, green)
│  ├─ (failed>0) «N failed» (warning, yellow)
│  ├─ (disabled>0) «N off» (circle-slash, muted)
│  ├─ «N cmds» (symbol-keyword, muted)
│  └─ contrib × N (alignment=1, priority desc)
└─ right: flex.items_center.gap(2).ml_auto
   ├─ contrib × N (alignment=2)
   ├─ (update) пилюля «Update {ver}» (№120)
   ├─ (eol) «UTF-8» + «LF|CRLF» (№119)
   └─ бренд «KaminIDE {version}» (№120)
```

## Метрики (из кода, точные)
- Высота 24 (STATUS_BAR_HEIGHT), px 8 (SPACE_2), gap контейнера 4 (SPACE_1), fs 11 (FS_XS), базовый цвет p.text_muted #838aa0, фон прозрачный (градиент окна просвечивает)
- Группы left/right: gap 2, right = ml_auto

## Отличия от original.md той же папки
1. `items_center` вместо `align-items: stretch` (item'ы не тянутся на всю высоту бара; при нашем py у item'ов hit-зона ниже).
2. Правые contributed отсортированы по УБЫВАНИЮ priority — общая сортировка перед разбором; в оригинале правые по возрастанию (asc).
3. Порядок правой группы: contributed → update-пилюля → encoding/EOL → бренд; в оригинале contributed → EncodingItems → VersionUpdateItem (версия/апдейт — ПОСЛЕДНИЙ, единый item).
4. Тег `<footer>` и aria отсутствуют (div + probe_area).
5. gap 2 у групп и gap 4 контейнера — совпадают с оригиналом.

## Дополнение атрибутов (цикл 10)

- шрифты: контейнер font-size 11 (FS_XS) (status_bar.rs:253), font-weight 400, семейство — UI-шрифт окна «Bricolage Grotesque» (собственного font_family бар не задаёт); группы left/right своих шрифтовых правил не задают — наследуют 11

### Вердикты

# 116 — verdict (review cycle 1)
VERDICT: DIVERGES
items_center vs stretch (пилюли ниже 24); порядок правой группы (update перед
encoding vs после); правые contributed desc vs asc. h24/fs-xs/px8/gap — 1:1.

## Цикл 2: MATCH

## Цикл 5: MATCH

Статус-бар: h24/px8/gap4/fs-xs/text-muted/прозрачный фон, растяжка через снятый `items_center`, группы gap 2 + `ml_auto`, порядок contributed → encoding/EOL → update → бренд. Замер по кадру: полоса ровно 24 логических.

## Цикл 6: MATCH

Статус-бар 1:1.

---

## 117. status-item-builtin — **MATCH** (цикл 6)

*История: ц2:MATCH, ц5:MATCH, ц6:MATCH*

![оригинал](117-status-item-builtin/original.png)
![наш](117-status-item-builtin/ours.png)

### Оригинал

# 117 status-item-builtin — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (147-158), `StatusBar.module.css` (23-49)

## JSX-структура (кратко, вложенность)
```
button.item [.ok|.warn|.brand] type=button tabIndex=-1 [data-tooltip={title} aria-label={title}]
├─ (icon) span.codicon.codicon-{icon} [aria-hidden]
└─ span {label}
```
- Чисто информационный: нет onClick; `tabIndex=-1` держит вне tab-order, но hover-тултип работает (в отличие от `disabled`).
- Варианты: "N active" (ok), "N failed" (warn), "N off", "N cmds".

## Метрики (ИЗ CSS, точные значения)
`.item`:
- display: flex; align-items: center; gap: 4px
- padding: 0 var(--space-2)
- color: var(--text-muted)
- border-radius: var(--radius-xs)
- font-size: var(--fs-xs)

`.item .codicon`: font-size: 12px !important

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.ok`: color: var(--accent-green)
- `.warn`: color: var(--accent-yellow)
- `.brand`: color: var(--accent-primary); font-weight: 500
- transition отсутствует.

### Наша реализация

# 117 status-item-builtin — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:132-156 (item), 168-199 (варианты)

## Структура (gpui-дерево кратко)
```
div#{tip}: flex.items_center.gap(4).px(8).rounded(4)
├─ (glyph) codicon 12px
└─ div {label}
tooltip(tip)
```
Варианты: «N active» (circle-filled \u{ea71}, accent_green), «N failed» (warning \u{ea6c}, accent_yellow), «N off» (circle-slash \u{eabd}, text_muted), «N cmds» (symbol-keyword \u{eb62}, text_muted).

## Метрики (из кода, точные)
- gap 4, px 8 (SPACE_2), rounded 4 (RADIUS_XS), fs 11 (FS_XS), codicon 12px
- Tone-цвета: ok p.accent_green #a6e3a1, warn p.accent_yellow #f9e2af, muted p.text_muted #838aa0
- Hover: bg p.bg_surface #3d3f51 a=.6 + text p.text_primary #cfd4e2

## Отличия от original.md той же папки
1. Метрики и hover совпадают (gap 4 / px space-2 / radius-xs / fs-xs / codicon 12 / bg-surface 60%).
2. Не `<button tabIndex=-1>` — обычный div; aria-label нет; тултип — наш gpui-тултип (overlay), не data-tooltip.
3. `.brand`-тона в item() нет — бренд-элемент собран отдельно (№120).
4. Вертикальный padding отсутствует у обоих (высота от контента у нас vs stretch в оригинале — см. №116 п.1).

### Вердикты

# 117 — verdict (review cycle 1)
VERDICT: MATCH
item-рецепт/тона/тултипы/глифы — 1:1 (высота пилюли — корень в 116).

## Цикл 2: MATCH

## Цикл 5: MATCH

Встроенный элемент статуса: gap4/px8/r-xs/fs11/codicon 12/hover bg-surface 60% + text-primary; тона ok/warn/muted 1:1.

## Цикл 6: MATCH

Встроенный элемент статуса 1:1.

---

## 118. status-item-contributed — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](118-status-item-contributed/original.png)
![наш](118-status-item-contributed/ours.png)

### Оригинал

# 118 status-item-contributed — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (69-83), `StatusBar.module.css` (23-45)

## JSX-структура (кратко, вложенность)
```
button.item [.clickable] type=button disabled={!command}
  style={item.color ? { color } : undefined}
  [data-tooltip={tooltip} aria-label={tooltip}]
  onClick → hostRpc.commands.execute(command)
└─ renderCodiconText(item.text)     ($(icon) → codicon-спаны + текст; общий парсер с QuickPick)
```
- clickable = есть `item.command`; иначе `disabled`.
- `item.color` — произвольный цвет расширения, inline.

## Метрики (ИЗ CSS, точные значения)
`.item` (общее с №117):
- display: flex; align-items: center; gap: 4px
- padding: 0 var(--space-2)
- color: var(--text-muted); border-radius: var(--radius-xs); font-size: var(--fs-xs)
- `.item .codicon`: font-size: 12px !important

`.clickable`: cursor: pointer

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.item:disabled`: cursor: default
- `.item:disabled:hover`: background: transparent; color: var(--text-muted) (не реагирует на hover)

### Наша реализация

# 118 status-item-contributed — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:31-63 (`ContribItem`), 66-89 (`rich_text`), 92-131 (`contrib`), 209-214 и 263-269 (размещение слева/справа по alignment+priority)

## Структура/содержание
```
div#sbi-{id}: flex.items_center.gap(4).px(8).rounded(4)
└─ children(rich_text(text))   — «$(icon)» → codicon 12px + текст-куски
(tooltip)  .tooltip(KaminTooltip)
(command)  .cursor_pointer.hover(...).on_mouse_down → поток → host RPC
           «kamin:command:execute» с аргументом command
```
`ContribItem` — `StatusBarItemState` 1:1 (id / alignment / priority / text / tooltip / command / color / visible), парсится из JSON (`from_value`, status_bar.rs:46-62). Цвет `#hex` → `parse_hex`, иначе `p.text_muted`. Alignment 1 → левая группа (сортировка priority DESC), 2 → правая (priority ASC).

## Метрики (из кода, точные)
- отступы: px 8 (SPACE_2), py НЕТ (как `.item { padding: 0 var(--space-2) }`); высота 24 — растяжка по бару (`align-items: stretch`, у ряда нет `items_center`)
- гэпы: внутри пилюли gap 4 (глиф ↔ текст); между пилюлями gap 2 у левой и правой групп, gap 4 (SPACE_1) у корня бара
- цвета: fg = `item.color` (`#hex`) либо p.text_muted #838aa0; глифы красятся тем же fg; текст на hover — p.text_primary #cfd4e2
- скругления: rounded 4 (RADIUS_XS)
- шрифты: собственного размера пилюля не задаёт → наследует 11 (FS_XS) от корня бара (status_bar.rs:253); font-weight 400; глифы `$(icon)` — codicon font-size 12 (status_bar.rs:77)
- фоны по ховеру: p.bg_surface #3d3f51 α 0.6 — ТОЛЬКО у пилюль с `command`

## Отличия от original.md той же папки
1. Совпадают 1:1: gap 4, padding 0×8, radius-xs, fs-xs 11, codicon 12, hover `bg-surface 60%` + `text-primary`.
2. Hover только у пилюль с `command` — поведенчески совпадает с оригиналом (`disabled={!command}`, `.item:disabled:hover` прозрачный + text-muted), но у нас это не `<button disabled>`: нет `cursor: default`-семантики и aria-состояния.
3. `item.color` применяется только если строка начинается с `#`; идентификаторы ThemeColor (`statusBarItem.warningBackground`, `charts.red`) молча падают в text-muted.
4. Парсер `$(name)`: имя ищется в нашей `codicon_map`; нераспознанное имя молча ВЫПАДАЕТ из вывода (оригинальный `renderCodiconText` — общий с QuickPick — оставляет спан класса codicon).
5. Тултип — наш `KaminTooltip` (№129) вместо `data-tooltip` + `aria-label`.
6. Инлайновый `style={{color}}` оригинала у нас применяется к контейнеру и к глифам одинаково — совпадает.

### Вердикты

# 118 — verdict (review cycle 1)
VERDICT: DIVERGES
r-sm vs r-xs; лишний py1; сорт desc vs asc; цвет только #hex. Парсер/hover/клик — 1:1.

## Цикл 2: DIVERGES
item.color только #hex.

## Цикл 5: DIVERGES

Contributed-элемент: r-xs и снятый py исправлены, ховер только у кликабельных (= `:disabled:hover` оригинала). Остаток: `item.color` парсится только как `#hex` → ThemeColor-идентификаторы игнорируются, элемент красится text-muted.

## Цикл 6: DIVERGES

`item.color` только `#hex` → ThemeColor-идентификаторы уходят в muted.

## Цикл 7: DIVERGES

ThemeColor резолвится браузером, у нас фильтр по '#' отправляет их в muted. НОВОЕ: ховер contributed-элемента 8% против bg-surface 60% — две копии логики разошлись.

---

## 119. status-editor-encoding-eol — **MATCH** (цикл 6)

*История: ц2:MATCH, ц5:MATCH, ц6:MATCH*

![оригинал](119-status-editor-encoding-eol/original.png)
![наш](119-status-editor-encoding-eol/ours.png)

### Оригинал

# 119 status-editor-encoding-eol — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (55-64), `StatusBar.module.css` (23-45)

## JSX-структура (кратко, вложенность)
```
selectedFile отсутствует → null (не рендерится)
иначе:
├─ Item label="UTF-8" title="Encoding"
└─ (eol) Item label="LF"|"CRLF" title="End of line"
```
- Item = встроенный item №117 (button.item, tabIndex=-1, data-tooltip).
- Кодировка всегда "UTF-8" (host читает/пишет UTF-8); EOL — реактивно из активной Monaco-модели (`activeEditorEol`), обновляется при смене файла, null → строка EOL скрыта.

## Метрики (ИЗ CSS, точные значения)
Использует `.item` без tone-классов:
- display: flex; align-items: center; gap: 4px
- padding: 0 var(--space-2)
- color: var(--text-muted); border-radius: var(--radius-xs); font-size: var(--fs-xs)

## Состояния (классы-варианты с метриками)
- `.item:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- Отображается ТОЛЬКО при активном текстовом редакторе (VS Code-парити).

### Наша реализация

# 119 status-editor-encoding-eol — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:273-276 (when_some(eol)), crates\shell\src\root.rs:103 (EditorTab.eol), 3617 (детект), 5374-5377 (передача)

## Структура (gpui-дерево кратко)
```
when_some(eol):
├─ item(None, «UTF-8», text_muted, tooltip «File encoding»)
└─ item(None, eol («LF»|«CRLF»), text_muted, tooltip «End of line»)
```
`eol` = Some только когда есть активный editor tab; определяется ОДИН раз при открытии файла: `text.contains("\r\n") → "CRLF" else "LF"` (root.rs:3617).

## Метрики (из кода, точные)
Как №117 без глифа: gap 4, px 8, rounded 4 (RADIUS_XS), fs 11, p.text_muted #838aa0; hover bg p.bg_surface a=.6 + p.text_primary.

## Отличия от original.md той же папки
1. EOL статичен с момента открытия файла — не реактивен к смене EOL в буфере (оригинал следит за activeEditorEol Monaco-модели).
2. Тултипы «File encoding» / «End of line» vs «Encoding» / «End of line» (первый отличается).
3. Условие показа — непустые editor_tabs (активный таб); режим web скрывает редактор, но табы остаются — возможен показ без видимого редактора (оригинал: только при активном текстовом редакторе).
4. Метрики item — совпадают.

## Дополнение атрибутов (цикл 10)

- шрифты: font-size 11 (FS_XS) (status_bar.rs:154), font-weight 400; глифа у «UTF-8»/«LF»/«CRLF» нет (item вызван с `None`, status_bar.rs:272-273)

### Вердикты

# 119 — verdict (review cycle 1)
VERDICT: DIVERGES
Тултип File encoding vs Encoding; гейт по eol (UTF-8 пропадает) vs по selectedFile.

## Цикл 2: MATCH

## Цикл 5: MATCH

Encoding/EOL: тултипы «Encoding»/«End of line», метрики 1:1. (EOL статичен с момента открытия — поведенческое, не визуальное.)

## Цикл 6: MATCH

Encoding/EOL 1:1.

---

## 120. status-version-update — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](120-status-version-update/original.png)
![наш](120-status-version-update/ours.png)

### Оригинал

# 120 status-version-update — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (90-145), `StatusBar.module.css` (23-76)

## JSX-структура (кратко, вложенность)
Три взаимоисключающих состояния (downloading > update-available > idle):
```
1) downloading:
div.item.brand.update.downloading role=progressbar aria-valuemin=0 aria-valuemax=100 [aria-valuenow={pct}]
  data-tooltip="Downloading the KaminIDE update…"
├─ span.progressFill style="width: {pct}%|100%; opacity: 1|0.5"
└─ span.progressLabel
   ├─ span.codicon.codicon-cloud-download
   └─ span "Updating {pct}%" | "Updating {N.n} MB"      (indeterminate: без Content-Length)

2) update available:
button.item.clickable.brand.update data-tooltip="Update to KaminIDE {v} — you have {cur}" onClick=installUpdate
├─ span.codicon.codicon-cloud-download
└─ span "Update {version}"

3) idle:
button.item.clickable.brand data-tooltip="Check for updates" onClick=checkForUpdate
└─ span "KaminIDE {version|0.0.1}"
```
- Indeterminate fill: opacity `INDETERMINATE_FILL_OPACITY = 0.5`, width 100%.

## Метрики (ИЗ CSS, точные значения)
База `.item`: display: flex; align-items: center; gap: 4px; padding: 0 var(--space-2); border-radius: var(--radius-xs); font-size: var(--fs-xs); `.codicon` 12px !important
`.clickable`: cursor: pointer
`.brand`: color: var(--accent-primary); font-weight: 500

`.update`:
- background: color-mix(in srgb, var(--accent-primary) 22%, transparent)
- color: var(--accent-primary); font-weight: 600

`.downloading`: position: relative; overflow: hidden

`.progressFill`:
- position: absolute; left: 0; top: 0; bottom: 0
- background: color-mix(in srgb, var(--accent-primary) 32%, transparent)
- transition: width 120ms linear
- pointer-events: none

`.progressLabel`: position: relative; display: inline-flex; align-items: center; gap: 6px

## Состояния (классы-варианты с метриками)
- `.item:hover` (idle): background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.update:hover`: background: color-mix(in srgb, var(--accent-primary) 34%, transparent); color: var(--accent-primary)
- downloading: не кнопка (div), прогресс-заливка позади лейбла; ширина трекает байты (120ms linear).

### Наша реализация

# 120 status-version-update — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:216-241 (бренд), 275-307 (update-пилюля), 175 и 308 (взаимоисключение по `has_update`); crates\shell\src\root.rs:1448-1450 (`UpdateAvailable`), 2296-2330 (`CheckForUpdates` → host RPC `kamin:updater:check` → тост), 6414-6422 (передача `env!("CARGO_PKG_VERSION")`)

## Структура/содержание
ДВА взаимоисключающих состояния вместо трёх (нет `downloading`):
```
1) update_available → div#update-pill: flex.items_center.gap(3).px(8).py(1).rounded(4)
   ├─ codicon cloud-download "\u{ea9a}" 12
   └─ «Update {ver}»
   tooltip «Update to KaminIDE {ver} — you have {version}»
   клик → cmd /c start "" {url}  (внешний браузер)
2) иначе → div#status-brand: flex.items_center.px(8).rounded(4)
   └─ «KaminIDE {version}»
   tooltip «Check for updates»
   клик → ShellEvent::CheckForUpdates → kamin:updater:check → тост
          «Update available: KaminIDE {v}» / «You are up to date» / «Update check failed: {e}»
```
Взаимоисключение: `.when(!has_update, |row| row.child(brand))` (status_bar.rs:308). Порядок правой группы: contributed → UTF-8/EOL → update | brand.

## Метрики (из кода, точные)
- отступы: update-пилюля px 8 (SPACE_2) + py 1; бренд px 8 (SPACE_2), py нет; высота обоих 24 (растяжка по бару)
- гэпы: update-пилюля gap 3 (глиф ↔ текст); между элементами правой группы gap 2
- цвета: update — bg p.accent_primary #89b4fa α 0.22, текст p.accent_primary #89b4fa; бренд — текст p.accent_primary #89b4fa
- скругления: обе пилюли rounded 4 (RADIUS_XS)
- шрифты: update — font-size 11 (наследует FS_XS от бара), font-weight 600 SEMIBOLD, глиф codicon 12; бренд — font-size 11 (FS_XS, задан явно), font-weight 500 MEDIUM
- фоны по ховеру: update — p.accent_primary α 0.34; бренд — p.bg_surface #3d3f51 α 0.6

## Отличия от original.md той же папки
1. Состояние `downloading` НЕ РЕАЛИЗОВАНО целиком: `role=progressbar` + `aria-valuenow`, `.progressFill` (accent-primary 32%, absolute, `transition: width 120ms linear`, indeterminate = width 100% / opacity 0.5), `.progressLabel` (gap 6) и тексты «Updating {pct}%» / «Updating {N.n} MB». Скачивание уходит во внешний браузер (`cmd /c start`), прогресса внутри приложения нет.
2. Бренд КЛИКАБЕЛЕН: cursor pointer, hover `bg-surface 60%`, тултип «Check for updates» — совпадает с оригиналом. Результат проверки показывается тостом, а не переходом item'а в состояние «update available» тем же кликом.
3. Бренд и update-пилюля взаимоисключающие — совпадает с оригиналом.
4. Совпадают: fill accent 22%, hover accent 34%, weight 600, radius-xs, глиф cloud-download 12, текст «Update {ver}», тултип «Update to KaminIDE {v} — you have {cur}», `.brand` weight 500 + accent-primary.
5. У update-пилюли добавлен `py 1` (в оригинале `.item { padding: 0 var(--space-2) }`) и gap 3 вместо 4.
6. Версия — `env!("CARGO_PKG_VERSION")` на билд-тайме; фоллбека `version || "0.0.1"` нет.
7. `.update:hover` в оригинале ещё и фиксирует `color: accent-primary` (чтобы generic `.item:hover` не перебил) — у нас generic-hover'а на этом элементе нет, поведение совпадает без явного правила.

### Вердикты

# 120 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет downloading (progressFill 120ms + indeterminate + Updating N%); пилюля и бренд
одновременно vs взаимоисключаемость; idle-бренд не кликабелен (нет Check for updates);
hover op.85 vs accent34%; тексты тултипов. Пилюля-рецепт (22%/600/r-xs) — 1:1.

## Цикл 2: DIVERGES
Нет downloading; пилюля+бренд одновременно; idle-бренд не кликабелен.

## Цикл 5: DIVERGES

Версия/обновление: пилюля 22%/600/r-xs/hover 34% + тултип — 1:1. Остаток: нет состояния downloading (progressFill accent 32%, width 120ms linear, «Updating N%»/МБ, role=progressbar); пилюля и бренд рисуются одновременно вместо трёх взаимоисключающих; idle-бренд не кликабелен («Check for updates»).

## Цикл 6: DIVERGES

Нет состояния загрузки обновления; пилюля и бренд рисуются одновременно; бренд не кликабелен.

## Цикл 7: DIVERGES

Осталось только состояние загрузки. ДВЕ ЛОЖНЫЕ претензии: пилюля и бренд взаимоисключающи; бренд кликабелен с тултипом.

---

## 121. confirm-modal — **DIVERGES** (цикл 7)

*История: ц2:MATCH, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](121-confirm-modal/original.png)
![наш](121-confirm-modal/ours.png)

### Оригинал

# 121 confirm-modal — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/ConfirmModal.tsx` (73-98), `ConfirmModal.module.css`

## JSX-структура (кратко, вложенность)
```
div.overlay role=presentation onClick(target===currentTarget → cancel)
└─ div.dialog role=dialog aria-modal=true aria-label={title}
   ├─ h3.title
   ├─ div.body dangerouslySetInnerHTML={sanitized bodyHtml}    (вырезаны <script>, on*=, javascript:)
   └─ div.actions
      ├─ button.cancelBtn {cancelLabel="Cancel"}
      └─ button.confirmBtn [.danger] [ref автофокус] {confirmLabel="Confirm"}
```
- Esc = cancel; backdrop-клик = cancel; автофокус Confirm (Enter принимает); восстановление фокуса на предыдущий элемент при закрытии.

## Метрики (ИЗ CSS, точные значения)
`.overlay`:
- position: fixed; inset: 0; z-index: var(--z-modal)
- background: var(--overlay-deep)
- display: flex; align-items: center; justify-content: center
- animation: fadeIn 0.12s ease-out (opacity 0→1)

`.dialog`:
- background: var(--bg-primary)
- border: 1px solid var(--bg-surface)
- border-radius: var(--radius-md)
- padding: var(--space-5)
- min-width: 320px; max-width: 480px
- box-shadow: var(--shadow-modal)

`.title`:
- margin: 0 0 var(--space-3)
- font-size: var(--fs-md); font-weight: 600; color: var(--text-primary)

`.body`:
- margin: 0 0 var(--space-4)
- font-size: var(--fs-sm); color: var(--text-secondary); line-height: var(--lh-snug)

`.actions`: display: flex; gap: var(--space-2); justify-content: flex-end

`.cancelBtn`, `.confirmBtn` (общее):
- padding: var(--space-1) var(--space-4)
- border-radius: var(--radius-sm); font-size: var(--fs-sm); cursor: pointer
- transition: background var(--transition-fast)

`.cancelBtn`: border: 1px solid var(--bg-overlay); background: transparent; color: var(--text-primary)
`.confirmBtn`: border: none; background: var(--accent-action); color: var(--accent-action-fg); font-weight: 600

## Состояния (классы-варианты с метриками)
- `.cancelBtn:hover`: background: var(--bg-surface)
- `.confirmBtn:hover`: background: var(--accent-action-hover)
- `.confirmBtn.danger`: background: var(--accent-red)
- `.confirmBtn.danger:hover`: background: var(--accent-maroon)

### Наша реализация

# 121 confirm-modal — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\modal.rs:69-214 (render_modal, dialog_button, dialog_button_bg); рендер — в overlay-окне (crates\shell\src\overlay.rs), скрим-затемнение main-окна — root.rs:5384+

## Структура (gpui-дерево кратко)
```
div (скрим): absolute.size_full.flex.center.bg(rgba(0,0,0,.6)) — клик = cancel
└─ div (диалог): stop_propagation + region_area()
   ├─ title
   ├─ body
   ├─ (prompt) input-блок (№122)
   └─ actions: [Cancel] [Confirm]
```
Esc — обрабатывается снаружи (RootView). Danger-вариант красит Confirm в accent_red.

## Метрики (из кода, точные)
- Скрим: rgba(0,0,0,0.6) (= overlay-deep)
- Диалог: min_w 320, max_w 480, p 20 (SPACE_5), rounded 12 (RADIUS_MD), bg p.bg_primary #313240, border 1 p.bg_surface #3d3f51
- Title: fs 13 (FS_MD), weight 600, p.text_primary #cfd4e2, mb 12 (SPACE_3)
- Body: fs 12 (FS_SM), p.text_secondary #adb3c7, line_height 15.6 (fs×1.3), mb 16 (SPACE_4)
- Actions: gap 8, justify_end
- Cancel: px 16 (SPACE_4), py 4 (SPACE_1), rounded 8, border 1 p.bg_overlay #515567, fs 12, p.text_primary; hover bg p.bg_surface
- Confirm: те же паддинги, bg p.accent_action #89b4fa (danger → p.accent_red #f38ba8), fg p.accent_action_fg #313240, weight 600; hover opacity .9

## Отличия от original.md той же папки
1. box-shadow: var(--shadow-modal) отсутствует (тени у диалога нет).
2. fadeIn-анимация 0.12s отсутствует.
3. Body — плоский текст (sanitized-HTML-рендер не реализован).
4. Confirm hover = opacity .9 вместо bg accent-action-hover #74c7ec; danger hover не accent-maroon.
5. Автофокус Confirm (Enter принимает) и восстановление фокуса — нет.
6. Рендер в отдельном overlay-окне; затемнение фона рисует main-окно (двухоконная схема, у оригинала один DOM).
7. Все размеры/цвета покоя (min/max, p 20, radius 12, бордер, палитра кнопок) — совпадают.

### Вердикты

# 121 — verdict (review cycle 1)
VERDICT: MATCH
Скрим/диалог/кнопки/hover-ы/shadow-modal/Esc — 1:1 (fadeIn — deviation).

## Цикл 2: MATCH

## Цикл 5: DIVERGES

Confirm-модалка: p20, gap 9.6, кнопки 70/67, bg #313240, border #3d3f51, вилка ширины 320-480, shadow-modal, danger-цвета — точно. Расхождения: высота диалога 136 против 125 и кнопки 30 против 24 — **корень был в отсутствии line-height у overlay-слоя, исправлено волной 8**; скрим 0.5 вместо `overlay-deep` .6 — **исправлено**; `body` плоским текстом (у оригинала имя сессии жирным из bodyHtml); Esc не привязан к overlay-окну — нужна живая проверка.

## Цикл 6: DIVERGES

Рецепт модалки точен. Остаток: тело плоским текстом вместо жирного имени сессии; Escape в overlay-окне не привязан (`on_key_down` в `overlay.rs` нет). Высоту перемерить после line-height.

## Цикл 7: DIVERGES

Текст с strong против плоского с кавычками. Высота ЗАКРЫТА (125.6 против 125). ЛОЖНАЯ: Escape привязан.

---

## 122. prompt-modal — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

*кадр «оригинал» отсутствует*
![наш](122-prompt-modal/ours.png)

### Оригинал

# 122 prompt-modal — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/PromptModal.tsx` (71-102), `PromptModal.module.css`

## JSX-структура (кратко, вложенность)
```
div.overlay role=presentation onClick(target===currentTarget → cancel)
└─ div.dialog role=dialog aria-modal=true aria-label={title}
   ├─ h3.title
   ├─ input.input [.invalid] type=text [ref фокус+select] placeholder value
   │    Enter → submit (блокируется при error)
   ├─ (error) div.error {текст валидации}
   └─ div.actions
      ├─ button.cancelBtn "Cancel"
      └─ button.confirmBtn "OK" disabled={!!error}
```
- `validate` бежит на каждый ввод; строка → invalid + inline-ошибка + disabled OK. Esc = cancel. Reset к defaultValue при каждом открытии; восстановление фокуса при закрытии.

## Метрики (ИЗ CSS, точные значения)
`.overlay`: position: fixed; inset: 0; z-index: var(--z-modal); background: var(--overlay-deep); flex центр; animation: fadeIn 0.12s ease-out

`.dialog`:
- background: var(--bg-primary); border: 1px solid var(--bg-surface)
- border-radius: var(--radius-md); padding: var(--space-5)
- min-width: 360px; max-width: 520px
- box-shadow: var(--shadow-modal)

`.title`: margin: 0 0 var(--space-3); font-size: var(--fs-md); font-weight: 600; color: var(--text-primary)

`.input`:
- width: 100%; padding: var(--space-2) var(--space-3)
- border: 1px solid var(--bg-surface); border-radius: var(--radius-sm)
- background: var(--bg-base); color: var(--text-primary)
- font-size: var(--fs-md); font-family: inherit; outline: none
- transition: border-color var(--transition-fast)

`.error`: margin-top: var(--space-2); font-size: var(--fs-xs); color: var(--accent-red)

`.actions`: display: flex; gap: var(--space-2); justify-content: flex-end; margin-top: var(--space-4)

`.cancelBtn`, `.confirmBtn`: padding: var(--space-1) var(--space-4); border-radius: var(--radius-sm); font-size: var(--fs-sm); cursor: pointer; transition: background var(--transition-fast)
`.cancelBtn`: border: 1px solid var(--bg-overlay); background: transparent; color: var(--text-primary)
`.confirmBtn`: border: none; background: var(--accent-action); color: var(--accent-action-fg); font-weight: 600

## Состояния (классы-варианты с метриками)
- `.input:focus`: border-color: var(--accent-primary)
- `.input.invalid`: border-color: var(--accent-red)
- `.cancelBtn:hover`: background: var(--bg-surface)
- `.confirmBtn:hover:not(:disabled)`: background: var(--accent-action-hover)
- `.confirmBtn:disabled`: opacity: 0.5; cursor: not-allowed

### Наша реализация

# 122 prompt-modal — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\modal.rs:14-27 (`Modal` + поле `placeholder`), 75-197 (`render_modal`), 110-116 (ширины prompt-режима), 155-177 (input-блок + Enter), 199-245 (кнопки); crates\shell\src\overlay.rs:1023-1042 (ленивое создание `InputState` в overlay-окне, `placeholder`, фокус), 1055-1070 (`ConfirmModalInput`); Esc — crates\shell\src\root.rs:5809, 5837-5838

## Структура/содержание
Отдельного компонента нет — это та же `ConfirmModal` (№121) с `prompt: Some(seed)`:
```
scrim div.absolute.size_full.flex.items_center.justify_center.bg(rgba(0,0,0,.6))  [клик = cancel]
└─ dialog div.relative.min_w(360).max_w(520).p(20).rounded(12)
          .bg(bg_primary).border_1(bg_surface).shadow(modal)   [клик внутри = stop_propagation]
   ├─ title  div.mb(12)
   ├─ body   div.mb(16)
   ├─ (prompt) input-блок div.mb(16).px(8).py(4).rounded(8).bg(bg_surface α .6).border_1(bg_overlay)
   │            ├─ on_key_down «enter» → stop_propagation + confirm
   │            └─ gpui_component::input::Input::new(input).appearance(false)
   └─ actions div.flex.gap(8).justify_end
      ├─ Cancel  (бордер bg_overlay, без фона)
      └─ Confirm (confirm_label; accent_action, danger → accent_red)
```
- `InputState` создаётся лениво в OVERLAY-окне (`InputState` требует `Window` того окна, где рендерится) и получает фокус (overlay.rs:1029-1037); при закрытии обнуляется — поэтому значение сбрасывается к `seed` при каждом открытии.
- Поле `Modal.placeholder: Option<SharedString>` доходит до `InputState::placeholder` (overlay.rs:1028-1033). Задано ТОЛЬКО в демо Design-панели (design_samples.rs:366, «e.g. my-extension»); боевые вызовы (`CreateEntry`, `RenameFs`, `SaveLayoutPreset`, `RenamePreset`) передают `None`.
- Подтверждение: `ConfirmModalInput(value)` → `run_modal_action`. Esc закрывает через `CloseOverlay` (root.rs:5837-5838).

## Метрики (из кода, точные)
- отступы: диалог p 20 (SPACE_5); input-блок px 8 (SPACE_2) / py 4; кнопки px 16 (SPACE_4) / py 4 (SPACE_1); отбивки — margin-bottom: заголовок 12 (SPACE_3), тело 16 (SPACE_4), input-блок 16 (SPACE_4)
- гэпы: ряд кнопок gap 8 (SPACE_2); вертикальных `gap` у диалога нет (всё на margin-bottom)
- цвета: скрим rgba(0,0,0,0.6) (= `--overlay-deep`); диалог bg p.bg_primary #313240 + border 1px p.bg_surface #3d3f51; заголовок p.text_primary #cfd4e2; тело p.text_secondary #adb3c7; input-блок bg p.bg_surface #3d3f51 α 0.6 + border 1px p.bg_overlay #515567; Cancel — текст p.text_primary #cfd4e2, border p.bg_overlay #515567; Confirm — bg p.accent_action #89b4fa (danger: p.accent_red #f38ba8), текст p.accent_action_fg #313240; placeholder красит vendored Input цветом `muted_foreground` СВОЕЙ темы, не нашей палитры (element.rs:956-959)
- скругления: диалог 12 (RADIUS_MD); input-блок 8 (RADIUS_SM); обе кнопки 8 (RADIUS_SM)
- шрифты: заголовок 13 (FS_MD) / 600 SEMIBOLD; тело 12 (FS_SM), line-height 15.6 (12 × 1.3); кнопки 12 (FS_SM) — Cancel weight 400, Confirm weight 600; сам `Input` размера не задаёт — наследует базовый кегль окна
- фоны по ховеру: Cancel — p.bg_surface #3d3f51 (сплошной); Confirm — p.accent_action_hover #74c7ec, в danger-режиме p.accent_maroon #eba0ac; у input-блока ни hover, ни focus-подсветки нет

## Отличия от original.md той же папки
1. Ширины 360 / 520 — совпадают с оригиналом (prompt шире confirm 320/480).
2. Enter в инпуте = сабмит — реализовано (modal.rs:169-174), совпадает с оригиналом; но у оригинала Enter блокируется при ошибке валидации, у нас сабмитит всегда.
3. Live-валидация НЕ РЕАЛИЗОВАНА: `validate` на каждый ввод, класс `.invalid` (border-color accent-red), inline `.error` (margin-top space-2, fs-xs, accent-red) и `disabled` у OK (opacity 0.5, cursor not-allowed) — ничего этого нет.
4. Инпут: bg `bg-surface 60%` + border `bg-overlay` вместо `bg-base` + border `bg-surface`; focus-подсветки `border-color: accent-primary` нет; `transition: border-color` нет.
5. `placeholder` поддержан в модели и доходит до `InputState`, но боевые prompt-вызовы его не задают — виден только в демо-блоке Design-панели.
6. Select-all значения при фокусе нет (overlay.rs:1037 делает только `window.focus`); сброс к `defaultValue` работает через пересоздание `InputState` при открытии — совпадает по эффекту.
7. Скрим 0.6 (= overlay-deep) совпадает; `animation: fadeIn 0.12s ease-out` отсутствует; `z-index: var(--z-modal)` → порядок детей overlay-слоя.
8. Восстановление фокуса при закрытии отсутствует.
9. Esc = cancel и клик по бэкдропу = cancel — совпадают с оригиналом.
10. Прочие общие расхождения №121 (роль `dialog`/`aria-modal` отсутствует) действуют и здесь.

### Вердикты

# 122 — verdict (review cycle 1)
VERDICT: DIVERGES
input 8/4 vs 8/12, bg surface60% vs bg-base, border overlay vs bg-surface; нет
focus/invalid-бордера, .error, disabled OK; лишний .body-параграф; кнопка не OK.
360/520 + Enter — подтверждены.

## Цикл 2: DIVERGES
Инпут стили (8/12, bg-base, border bg-surface, focus/invalid); нет .error/disabled OK/«OK»; лишний body.

## Цикл 5: DIVERGES

Prompt-модалка: 360/520 и Enter-сабмит — исправлены. Остаток: инпут px8/py4 + `bg_surface .6` + border `bg_overlay` вместо padding 8/12 + bg-base + border bg-surface, нет focus-бордера accent и `.invalid`; нет строки ошибки и disabled OK; confirm-label «Save»/«Rename» вместо «OK»; лишний body-параграф. Кадр не снялся (файл совпал с другим) — по скрину не проверено.

## Цикл 6: DIVERGES

Инпут не по рецепту (px8/py4 + bg-surface .6 вместо 8/12 + bg-base + border bg-surface + focus accent + `.invalid`); нет строки ошибки и disabled OK; лейбл «Save»/«Rename» вместо «OK»; лишний body-параграф.

## Цикл 7: DIVERGES

Инпут 8/12 + bg-base + focus accent + invalid red против px8/py4 + bg-surface .6; нет строки ошибки и disabled OK; лейбл Save вместо OK; лишний body_el.

---

## 123. quick-pick-modal — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

*кадр «оригинал» отсутствует*
![наш](123-quick-pick-modal/ours.png)

### Оригинал

# 123 quick-pick-modal — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/QuickPickModal.tsx` (65-123), `QuickPickModal.module.css`

## JSX-структура (кратко, вложенность)
```
div.overlay role=presentation onClick(backdrop; НЕ закрывает при ignoreFocusOut)
└─ div.panel role=dialog aria-modal=true aria-label={title ?? "Select"}
   ├─ (title) div.title
   ├─ input.input [ref фокус] placeholder={placeHolder ?? "Type to filter…"}
   │    Enter: multi → OK; single → первый selectable
   ├─ (prompt) div.prompt
   ├─ ul.list role=listbox aria-multiselectable={multi}
   │  ├─ (пусто) li.empty "No matching items"
   │  ├─ separator (kind=-1): li.separator role=separator {label}
   │  └─ li > button.item role=option aria-selected
   │     ├─ (multi) i.codicon.codicon-check|codicon-circle-large-outline .check
   │     ├─ span.label   (renderCodiconText: $(icon))
   │     ├─ (description) span.description
   │     └─ (detail) span.detail
   └─ (multi) div.actions
      ├─ button.cancelBtn "Cancel"
      └─ button.okBtn "OK ({checked.size})"
```
- Фильтр по label (+ description/detail при matchOnDescription/matchOnDetail); separators и alwaysShow обходят фильтр. Esc = resolve(null).

## Метрики (ИЗ CSS, точные значения)
`.overlay`:
- position: fixed; inset: 0; z-index: var(--z-modal)
- background: var(--overlay-modal)
- display: flex; justify-content: center; padding-top: var(--layout-palette-top-offset)
- animation: qpFade 0.12s ease-out

`.panel`:
- width: var(--layout-palette-width); max-width: calc(100vw - 32px); max-height: var(--layout-palette-max-height)
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 80%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-modal)
- flex column; overflow: hidden

`.title`: padding: var(--space-2) var(--space-4); font-size: var(--fs-sm); font-weight: 600; color: var(--text-primary); border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)

`.input`:
- margin: var(--space-2) var(--space-3) 0; padding: var(--space-2) var(--space-3)
- background: var(--bg-base); border: 1px solid color-mix(in srgb, var(--bg-surface) 70%, transparent)
- border-radius: var(--radius-sm); outline: none; color: var(--text-primary); font-size: var(--fs-md)

`.prompt`: padding: var(--space-1) var(--space-4) 0; font-size: var(--fs-sm); color: var(--text-secondary)

`.list`: list-style: none; margin: 0; padding: var(--space-1); overflow: auto; flex: 1; flex column; gap: 1px

`.item`:
- display: flex; align-items: baseline; gap: var(--space-2); width: 100%
- padding: var(--space-2) var(--space-3); border: none; background: transparent
- border-radius: var(--radius-sm); cursor: pointer; text-align: left
- font: inherit; font-size: var(--fs-md); color: var(--text-primary)

`.check`: align-self: center; font-size: 13px; color: var(--accent-primary); flex-shrink: 0
`.label`: flex-shrink: 0
`.description`: color: var(--text-muted); font-size: var(--fs-sm)
`.detail`: margin-left: auto; color: var(--text-muted); font-size: var(--fs-xs); font-family: var(--font-mono); nowrap + ellipsis
`.empty`: padding: var(--space-3) var(--space-4); color: var(--text-muted); font-style: italic

`.separator`:
- display: flex; align-items: center; gap: var(--space-2)
- padding: var(--space-1) var(--space-3); margin-top: var(--space-1)
- font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: 0.04em
- color: var(--text-muted); border-top: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- `:first-child`: border-top: none; margin-top: 0

`.actions`: display: flex; justify-content: flex-end; gap: var(--space-2); padding: var(--space-2) var(--space-3); border-top: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)

`.cancelBtn`, `.okBtn`: padding: var(--space-1) var(--space-3); border-radius: var(--radius-sm); border: 1px solid transparent; font-size: var(--fs-sm); cursor: pointer
`.cancelBtn`: background: transparent; color: var(--text-secondary)
`.okBtn`: background: var(--accent-primary); color: var(--accent-action-fg, #fff)

## Состояния (классы-варианты с метриками)
- `.input:focus`: border-color: var(--accent-primary)
- `.item:hover`: background: color-mix(in srgb, var(--accent-primary) 18%, transparent)
- `.cancelBtn:hover`: background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.okBtn:hover`: background: var(--accent-action-hover, var(--accent-primary))
- multi-чекбокс: codicon-check (выбран) / codicon-circle-large-outline (нет)

### Наша реализация

# 123 quick-pick-modal — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\quick_pick.rs:96-245 (quick_pick), 24-88 (QpItem/QuickPickState)

## Структура (gpui-дерево кратко)
```
div#quick-pick: occlude.absolute.top(84).left((vw−640)/2).w(640)
├─ (title) заголовок
├─ (input) ряд: search-codicon 14 + Input(appearance false), border-b
├─ p(4) → список (max_h 420, скролл)
│  ├─ separator-строка (kind=-1): label fs 11 muted
│  └─ row#qp-{i}: [чекбокс multi] + label + description
└─ (canPickMany) футер: кнопка «OK»
```
Single: клик → `QuickPickResolve(req_id, [i])`; multi: клик = toggle, OK резолвит checked. Esc/скрим → resolve(null) (снаружи). Ответ хосту — deferred respond (HostReply::Later).

## Метрики (из кода, точные)
- Бокс: top 84, w 640, rounded 12 (RADIUS_MD), bg p.bg_mantle #262533, border 1 p.bg_surface a=.8
- Title: px 12 (SPACE_3), pt 8, fs 12 (FS_SM), weight 600, p.text_primary
- Input-ряд: px 12, py 4, border-b p.bg_surface a=.6, search "\u{ea6d}" 14px p.text_muted
- Список: max_h 420, gap 1, обёрнут в p 4
- Row: gap 8, px 12, py 4, rounded 8, fs 12, p.text_secondary; hover bg p.text_primary a=.08 + text_primary
- Чекбокс: codicon check \u{eab2} / circle-large \u{eabc} 13px, on = p.accent_primary #89b4fa
- Description: fs 11, p.text_muted, в строку за label
- OK: px 16, py 4, rounded 8, bg p.accent_action #89b4fa, fg #313240, weight 600, hover opacity .9
- Separator: px 12, pt 4, fs 11, p.text_muted

## Отличия от original.md той же папки
1. `detail` парсится, но НЕ РЕНДЕРИТСЯ (у оригинала span.detail mono справа).
2. Prompt-строка (options.prompt) не рендерится.
3. Инпут «в стиле палитры» (transparent, border-b) вместо обрамлённого поля bg-base с focus border accent; иконка search добавлена (в оригинале её нет).
4. Row: fs 12 text_secondary vs fs-md 13 text-primary; hover text_primary 8% vs accent-primary 18%; padding py 4 vs space-2 (8).
5. Separator: без uppercase, letter-spacing и border-top.
6. Multi-футер: только «OK» без счётчика «OK (N)» и без Cancel-кнопки.
7. Фильтр только по label (matchOnDescription/matchOnDetail, alwaysShow — нет); separators фильтр обходят — совпадает.
8. max-h списка 420 фикс vs palette-max-height контейнера; скрима-элемента здесь нет (затемнение рисует main-окно), ignoreFocusOut считан, но на клик скрима не влияет.

### Вердикты

# 123 — verdict (review cycle 1)
VERDICT: DIVERGES
НЕТ скрима overlay-modal + backdrop-отмены; нет shadow-modal; title без border-b;
input без bg-base/border/r-sm (+лишний search-глиф); нет .prompt/.empty/.detail/
Cancel; OK без (N); item center/py4/fs-sm/secondary/hover-tint vs baseline/8/fs-md/
primary/accent18%; unchecked-чек muted vs accent; separator без border/uppercase;
фильтр только label.

## Цикл 2: DIVERGES
Нет скрима/shadow-modal/prompt/empty/detail/Cancel/OK(N); item-рецепт; separator; фильтр.

## Цикл 5: DIVERGES

QuickPick — худший в зоне: не было скрима вовсе (**добавлен волной 8**), нет shadow-modal, нет max-h панели (только у списка) вместо 60vh, title без border-bottom, инпут «палитрой» вместо обрамлённого поля bg-base с focus-accent, нет `.prompt`/`.empty`/`.detail`/Cancel, «OK» без «(N)», строка py4/fs12/secondary/hover 8% вместо py8/fs13/primary/accent 18%, невыбранный чекбокс muted вместо accent, separator без border-top и uppercase, фильтр только по label, нет `on_key_down` (Esc держится только на main).

## Цикл 6: DIVERGES

Скрим добавлен ✓. **Волна 10**: `max_h 60vh` перенесён на ПАНЕЛЬ + `overflow_hidden` + `shadow::modal()`. Осталось: обрамлённый инпут bg-base, рецепт строк (py8/fs13/primary/accent 18%), `.prompt`/`.empty`/`.detail`/Cancel, «OK (N)», border-top у separator, фильтр по description, Escape.

## Цикл 7: DIVERGES

ЛОЖНАЯ в обратную сторону: скрима НЕТ вовсе, клик мимо не закрывает. Осталось: инпут без рамки; строки py4 против 8/12 fs-md accent 18%; нет prompt/empty/detail/Cancel; OK без счётчика; separator без border-top; фильтр только по label.

---

## 124. quick-open — **DIVERGES** (цикл 8)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES, ц8:DIVERGES*

![оригинал](124-quick-open/original.png)
![наш](124-quick-open/ours.png)

### Оригинал

# 124 quick-open — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/QuickOpen.tsx` (84-125), `QuickOpen.module.css`

## JSX-структура (кратко, вложенность)
```
div.backdrop role=presentation onMouseDown → close
└─ div.box role=presentation onMouseDown stopPropagation
   ├─ input.input [ref фокус] placeholder="Type a file name…"
   │    ArrowDown/ArrowUp двигают active, Enter коммитит
   └─ ul.list role=listbox aria-label="Quick Open results"
      ├─ (нет результатов && query) li.empty "No matches"
      └─ li.item [.itemActive] role=option aria-selected × N (mouseenter → active, click → открыть)
         ├─ span.itemName {basename(rel)}
         └─ span.itemPath {dir(rel)}
```
- Открытие: Ctrl/Cmd+P (без Shift), обработчик capture на document; Esc закрывает. Debounce `QO_DEBOUNCE_MS = 80` мс; backend ≤ 50 хитов.

## Метрики (ИЗ CSS, точные значения)
`.backdrop`:
- position: fixed; inset: 0; z-index: var(--z-overlay)
- display: flex; justify-content: center; align-items: flex-start; padding-top: 12vh
- background: rgba(0, 0, 0, 0.35); backdrop-filter: blur(2px)

`.box`:
- width: min(640px, calc(100vw - 32px))
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)
- overflow: hidden; flex column

`.input`:
- width: 100%; padding: 12px 14px
- background: transparent; color: var(--text-primary)
- border: none; border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent)
- font-size: var(--fs-md); outline: none

`.list`: list-style: none; margin: 0; padding: var(--space-1) 0; max-height: min(50vh, 480px); overflow-y: auto

`.item`: display: flex; align-items: baseline; gap: var(--space-2); padding: 6px 14px; cursor: pointer

`.itemName`: font-size: var(--fs-sm); color: var(--text-primary); font-weight: 500
`.itemPath`: flex: 1; font-size: var(--fs-xs); color: var(--text-muted); overflow hidden + ellipsis + nowrap; text-align: right
`.empty`: padding: 12px 14px; color: var(--text-muted); font-size: var(--fs-sm); text-align: center

## Состояния (классы-варианты с метриками)
- `.itemActive`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- Светлая тема `[data-theme="light"] .itemActive`: background: var(--accent-primary); color: var(--accent-action-fg); `.itemName` → var(--accent-action-fg); `.itemPath` → color-mix(in srgb, var(--accent-action-fg) 80%, transparent)
- :hover-класса нет — active управляется mouseenter из TSX.

### Наша реализация

# 124 quick-open — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\quick_open.rs:93-194 (quick_open), 41-90 (hit_row); инпут/подписка — root.rs:3917-3925

## Структура (gpui-дерево кратко)
```
div (backdrop): absolute.size_full.flex.justify_center.items_start.pt(108).bg(rgba(0,0,0,.35))
├─ input_area() + on_key_down (escape → close; enter → открыть ПЕРВЫЙ)
├─ mouse_down → close
└─ бокс: w(640).max_w(vw−32).rounded(12).bg(bg_mantle).border(bg_surface .6).shadow(0 6 24 .4)
   ├─ input-ряд: px(14).py(12).border_b(bg_surface .5) → Input(appearance false)
   └─ список: py(4).max_h(480), row × ≤50
      row: name (medium) + dir-путь (ellipsis)
```
Изменение текста инпута сразу шлёт `kamin:index:findFile` (root-подписка). Первый ряд подсвечен, Enter открывает его.

## Метрики (из кода, точные)
- Backdrop: rgba(0,0,0,0.35); pt = 0.12×900 = 108 (константа, НЕ от вьюпорта)
- Бокс: w 640, rounded 12 (RADIUS_MD), bg p.bg_mantle #262533, border p.bg_surface #3d3f51 a=.6, shadow 0 6 24 rgba(0,0,0,.4)
- Input-ряд: px 14, py 12, border-b bg_surface a=.5
- Список: max_h 480, MAX_ROWS 50
- Row: baseline, gap 8, px 14, py 6; name fs 12 (FS_SM) weight 500 p.text_primary; путь fs 11 p.text_muted ellipsis
- Первый ряд/hover: bg p.accent_primary #89b4fa a=.14
- Empty «No matches»: px 14, py 12, fs 12, p.text_muted, по центру

## Отличия от original.md той же папки
1. pt 108 — фиксированный (0.12×900), а не 12vh реального вьюпорта: на других высотах позиция уезжает.
2. Навигации стрелками (ArrowUp/Down + mouseenter-active) НЕТ — активен всегда первый ряд, Enter открывает только его.
3. Путь не выровнен вправо (`text-align: right` нет) — идёт сразу за именем.
4. Debounce 80ms отсутствует — запрос на каждый ввод.
5. backdrop-filter: blur(2px) нет (у скрима только альфа).
6. «No matches» показывается и при пустом query (оригинал — только при непустом).
7. Light-темы вариант active-строки (bg accent_primary + fg accent-action-fg) не реализован.
8. w/max-w бокса, паддинги инпута и строк (12/14, 6/14), цвет подсветки 14% — совпадают.

### Вердикты

# 124 — verdict (review cycle 1)
VERDICT: DIVERGES
shadow инлайн 0/6/24/.4 vs dropdown 0/4/16/.5; нет text-right у path; max-h без 50vh;
лишний hover; нет стрелок; нет light-active. pt12vh/бокс/строки — 1:1.

## Цикл 2: DIVERGES
path не text-right; max-h без min(50vh,480); нет стрелок; нет light-active.

## Цикл 5: DIVERGES

QuickOpen: pt 12vh, `max_h min(50vh,480)`, path справа, dropdown-тень — подтверждены кадром. Инпут-ряд 55 против 39 и строка 31.2 против 26.4 — **исправлены волной 8** (line-height overlay + фиксированная высота ряда 40; замер после правки: ряд 39.2, шаг строк 26.0 против 26.4 у оригинала). Скрим 0.35 — **исправлен**. Остаток: нет blur(2px) у скрима, нет ArrowUp/Down, нет debounce 80 мс, нет светлого варианта активной строки.

## Цикл 6: DIVERGES

Шаг строк списка закрыт (25.6 против 26.4). Инпут-ряд: замер ц.6 дал 34.4 — **волна 10** добавила `flex_shrink_0` к фиксированной высоте 40, пере-замер после сборки: перепад на 39.2-44 лог. (в пределах шума, требует чистого замера ревьюером). Осталось: blur(2px) скрима, стрелки, debounce 80 мс, светлый active.

## Цикл 7: DIVERGES

Инпут-ряд ЗАКРЫТ (40.0 против 40.2). Осталось: нет debounce 80 мс, нет стрелок, светлый active solid accent.

## Цикл 8: DIVERGES

Закрыто: debounce 80 мс (`QO_DEBOUNCE_MS`, поколение запроса гасит устаревшие),
навигация ↑/↓ по списку (перехват в фазе capture — фокус держит `Input` оверлея),
Enter открывает АКТИВНУЮ строку, светлая тема заливает активную строку сплошным
accent (`accent-action-fg` у имени, 80% у пути).

Осталось: живая проверка клавиатуры probe-ом невозможна (gpui игнорирует и
`PostMessage(WM_KEYDOWN)`, и `SendInput` без реального фокуса) — состояние проверено
через `emit overlayMove`, сама подсветка активной строки подтверждена кадром.

---

## 125. find-in-files — **DIVERGES** (цикл 8)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES, ц8:DIVERGES*

![оригинал](125-find-in-files/original.png)
![наш](125-find-in-files/ours.png)

### Оригинал

# 125 find-in-files — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/FindInFiles.tsx` (89-137), `FindInFiles.module.css`

## JSX-структура (кратко, вложенность)
```
div.backdrop role=presentation onMouseDown → close
└─ div.box role=presentation stopPropagation
   ├─ input.input [ref фокус] placeholder="Search in files…"
   ├─ div.status  "Searching…" | "Type at least 2 chars" | "{N} hits"
   └─ ul.list role=listbox aria-label="Find in Files results"
      └─ li.item [.itemActive] role=option aria-selected × N
         ├─ div.itemHeader
         │  ├─ span.itemRel {rel}
         │  └─ span.itemLine ":{line}"
         └─ div.itemSnippet
            ├─ span {до матча}
            ├─ mark.match {матч}
            └─ span {после}
```
- Открытие: Ctrl/Cmd+Shift+F (document capture); Esc закрывает. Debounce `FIF_DEBOUNCE_MS = 220` мс; минимум 2 символа; backend ≤ 200 хитов.

## Метрики (ИЗ CSS, точные значения)
`.backdrop`:
- position: fixed; inset: 0; z-index: var(--z-overlay)
- flex; justify-content: center; align-items: flex-start; padding-top: 10vh
- background: rgba(0, 0, 0, 0.35); backdrop-filter: blur(2px)

`.box`:
- width: min(720px, calc(100vw - 32px)); max-height: 76vh
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-dropdown)
- overflow: hidden; flex column

`.input`: width: 100%; padding: 12px 14px; background: transparent; color: var(--text-primary); border: none; border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent); font-size: var(--fs-md); outline: none

`.status`: padding: 6px 14px; font-size: var(--fs-xs); color: var(--text-muted)

`.list`: list-style: none; margin: 0; padding: 0 0 var(--space-2); overflow-y: auto

`.item`: padding: 6px 14px; cursor: pointer; flex column; gap: 2px; border-radius: var(--radius-xs)

`.itemHeader`: display: flex; align-items: baseline; gap: 4px; font-size: var(--fs-xs); color: var(--text-muted)
`.itemRel`: overflow hidden + ellipsis + nowrap
`.itemLine`: font-variant-numeric: tabular-nums
`.itemSnippet`: font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-secondary); nowrap + hidden + ellipsis

`.match`:
- background: color-mix(in srgb, var(--accent-orange) 35%, transparent)
- color: var(--text-primary); border-radius: 2px

## Состояния (классы-варианты с метриками)
- `.itemActive`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- active управляется mouseenter/стрелками; :hover-класса нет; transition отсутствует.

### Наша реализация

# 125 find-in-files — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\find_in_files.rs:127-234 (find_in_files), 45-123 (hit_row, split3); инпут/подписка — root.rs:3927-3944

## Структура (gpui-дерево кратко)
```
div (backdrop): absolute.size_full.pt(0.12×vh).bg(rgba(0,0,0,.35)) — клик/Esc close, Enter → первый хит
└─ бокс: w(720).max_w(vw−32).max_h(0.76×vh).rounded(12).bg(bg_mantle).shadow(0 6 24 .4)
   ├─ input-ряд: px(14).py(12).border_b
   ├─ status: «Searching…» | «Type at least 2 chars» | «{N} hits»
   └─ список (скролл), row × ≤200:
      ├─ header: rel (ellipsis) + «:{line}»
      └─ snippet: mono, [до][match: bg accent_orange .35][после]
```
Запрос при len≥2 (root-подписка, busy-флаг); клик/Enter → `OpenFileAt(abs, line)`.

## Метрики (из кода, точные)
- Backdrop: rgba(0,0,0,.35); pt = 12% высоты вьюпорта (min 600)
- Бокс: w 720, max_h 76% vh, rounded 12, bg p.bg_mantle #262533, border p.bg_surface a=.6, shadow 0 6 24 rgba(0,0,0,.4)
- Input: px 14, py 12, border-b bg_surface a=.5
- Status: px 14, py 6, fs 11 (FS_XS), p.text_muted
- Row: px 14, py 6, rounded 4 (RADIUS_XS), gap 2, flex-col; header fs 11 p.text_muted; snippet «JetBrains Mono» fs 11 p.text_secondary
- Match: bg p.accent_orange #fab387 a=.35, rounded 2, text p.text_primary
- Первый ряд/hover: bg p.accent_primary a=.14; MAX_ROWS 200

## Отличия от original.md той же папки
1. pt 12% vs 10vh у оригинала (окно ниже).
2. Debounce 220ms отсутствует — запрос на каждый ввод при len≥2 (порог 2 символа совпадает).
3. Стрелочной навигации нет; активен всегда первый хит.
4. backdrop-filter: blur(2px) нет.
5. `font-variant-numeric: tabular-nums` на «:{line}» нет.
6. box-shadow свой (0 6 24 .4) vs var(--shadow-dropdown).
7. Совпадает: w 720 / max-h 76vh, паддинги 12/14 и 6/14, статус-тексты дословно, подсветка матча accent-orange 35% rounded 2, кап 200.

## Дополнение атрибутов (цикл 10)

- шрифты: header ряда (rel + `:line`) font-size 11 (FS_XS), font-weight 400 (find_in_files.rs:83); snippet — font-family «JetBrains Mono» (моно), font-size 11 (FS_XS) (find_in_files.rs:102-103); status-строка font-size 11 (FS_XS) (find_in_files.rs:221); input-ряд собственного font-size не задаёт — наследует базовый размер окна (find_in_files.rs:206-215)

### Вердикты

# 125 — verdict (review cycle 1)
VERDICT: DIVERGES
pt 12vh vs 10vh; кламп vh.max(600); shadow как 124; нет tabular-nums; нет стрелок;
лишний hover. Остальное — 1:1 (720/76vh/match-подсветка).

## Цикл 2: DIVERGES
Только стрелки ArrowUp/Down.

## Цикл 5: DIVERGES

Find-in-files: pt 10vh (111 против 110 логических), w720, max-h 76vh, статус-тексты, подсветка совпадения accent-orange 35% r2 — 1:1; `tnum` есть. Инпут-ряд и скрим — **исправлены волной 8**. Остаток: нет стрелок и debounce 220 мс.

## Цикл 6: DIVERGES

Осталось: стрелки и debounce 220 мс.

## Цикл 7: DIVERGES

Нет стрелок и debounce 220 мс — запрос на каждое нажатие. Остальное совпало.

## Цикл 8: DIVERGES

Закрыто: debounce 220 мс с гашением устаревших ответов (busy взводится сразу, как
`setBusy(true)` до таймера), ↑/↓ по списку, Enter открывает активную строку.

Осталось: живая проверка клавиатуры probe-ом невозможна (см. 124).

---

## 126. workspace-symbols — **DIVERGES** (цикл 8)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES, ц8:DIVERGES*

![оригинал](126-workspace-symbols/original.png)
![наш](126-workspace-symbols/ours.png)

### Оригинал

# 126 workspace-symbols — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/WorkspaceSymbols.tsx` (79-111); CSS — переиспользование `overlays/QuickOpen.module.css` (метрики идентичны №124)

## JSX-структура (кратко, вложенность)
```
div.backdrop role=presentation onMouseDown → close
└─ div.box role=presentation stopPropagation
   ├─ input.input [ref фокус] placeholder="Go to symbol in workspace…"
   └─ ul.list role=listbox aria-label="Workspace symbols"
      ├─ (нет результатов && query) li.empty "No symbols"
      └─ li.item [.itemActive] role=option aria-selected × N
         ├─ span.codicon.codicon-{SYMBOL_ICON[kind] ?? symbol-misc}
         ├─ span.itemName {name}
         └─ span.itemPath "{containerName · }{basename(uri)}"
```
- Открытие: Ctrl/Cmd+T (без Shift); Esc закрывает. Debounce `WS_DEBOUNCE_MS = 120` мс; минимум 1 символ.
- SymbolKind→codicon карта: 4 class, 5 method, 6 property, 7 field, 8 constructor, 9 enum, 10 interface, 11 function, 12 variable, 13 constant, 22 struct, 1/2 namespace, 23 event; fallback `symbol-misc`.
- Enter/клик → `openFileAt(uri, range)` (открыть + reveal диапазона).

## Метрики (ИЗ CSS, точные значения)
Полностью из QuickOpen.module.css:
- `.backdrop`: fixed inset 0; z-index var(--z-overlay); flex center/flex-start; padding-top: 12vh; background: rgba(0,0,0,0.35); backdrop-filter: blur(2px)
- `.box`: width min(640px, calc(100vw - 32px)); background var(--bg-mantle); border 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent); border-radius var(--radius-md); box-shadow var(--shadow-dropdown); overflow hidden
- `.input`: padding 12px 14px; background transparent; border-bottom 1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent); font-size var(--fs-md)
- `.list`: padding var(--space-1) 0; max-height min(50vh, 480px); overflow-y auto
- `.item`: flex baseline; gap var(--space-2); padding 6px 14px; cursor pointer
- `.itemName`: font-size var(--fs-sm); color var(--text-primary); font-weight 500
- `.itemPath`: flex 1; font-size var(--fs-xs); color var(--text-muted); ellipsis; text-align right
- `.empty`: padding 12px 14px; color var(--text-muted); font-size var(--fs-sm); text-align center
- Codicon-иконка символа: без собственного класса, размер по умолчанию codicon.

## Состояния (классы-варианты с метриками)
- `.itemActive`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- Light-тема: `.itemActive` background var(--accent-primary), текст/путь → var(--accent-action-fg) (см. №124).

### Наша реализация

# 126 workspace-symbols — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\workspace_symbols.rs:118-220 (workspace_symbols), 40-115 (kind_glyph, symbol_row); инпут/подписка — root.rs:3946-3961

## Структура (gpui-дерево кратко)
Тот же бокс, что QuickOpen (№124):
```
backdrop (.35, pt 108) → бокс w(640).bg(bg_mantle).rounded(12).shadow(0 6 24 .4)
├─ input-ряд px(14).py(12).border_b
└─ список py(4).max_h(480), row × ≤100:
   [SymbolKind-codicon 14 accent_blue] [name medium] [«container · basename» ellipsis]
```
Запрос при len≥1 (`kamin:lang:workspaceSymbol`); Enter/клик → `OpenFile(uri)`.

## Метрики (из кода, точные)
- Как №124: backdrop rgba(0,0,0,.35), pt 108 (0.12×900 фикс), бокс w 640, rounded 12, bg #262533, border bg_surface a=.6, shadow 0 6 24 .4; input px 14 py 12; row px 14 py 6 gap 8
- Иконка kind: codicon 14px, p.accent_blue #89b4fa
- name: fs 12 weight 500 p.text_primary; path: fs 11 p.text_muted ellipsis
- Первый ряд/hover: p.accent_primary a=.14; MAX_ROWS 100; empty «No symbols» при query>0
- Kind-карта: 1..3 namespace, 4 class, 5|8|11|12 method/fn, 6 property, 7 field, 9 enum, 10 interface, 13 variable, 14 constant, 22 struct, 23..24 event, иначе symbol-misc

## Отличия от original.md той же папки
1. Открытие БЕЗ reveal диапазона: `OpenFile(uri)` вместо `openFileAt(uri, range)` — курсор не прыгает к символу.
2. Иконка kind окрашена accent_blue (у оригинала цвет по умолчанию codicon = текущий text-цвет).
3. pt фиксированный 108 (не 12vh вьюпорта); blur нет; стрелок нет; light-тема active — нет (наследие №124).
4. Debounce 120ms отсутствует (запрос на каждый ввод, min 1 символ — совпадает).
5. Kind-карта чуть шире оригинала (2..3 → namespace-глиф; 8/11/12 слиты в method/fn) — визуально совместимо, но constructor(8) у оригинала отдельный глиф.

## Дополнение атрибутов (цикл 10)

- скругления: бокс border-radius 12 (RADIUS_MD) (workspace_symbols.rs:209); у строк списка радиуса нет (`symbol_row` без `.rounded`, workspace_symbols.rs:82-89) — совпадает с оригиналом (`.item` без border-radius)

### Вердикты

# 126 — verdict (review cycle 1)
VERDICT: DIVERGES
SymbolKind-карта сдвинута (12=variable, 13=constant, 14=misc, 3=misc, 24=misc по
оригиналу); иконка accent-blue vs наследование; shadow как 124; нет text-right/50vh/
стрелок. reveal OpenFileAt + pt12vh — подтверждены.

## Цикл 2: DIVERGES
path text-right; max-h 50vh; baseline; стрелки.

## Цикл 5: DIVERGES

Workspace symbols: глиф без своего цвета, path справа, max-h, reveal через `OpenFileAt` — исправлены. Инпут-ряд и скрим — **волна 8**. Остаток: kind-карта слепляет 8 (constructor) и 11 (function) в глиф метода; нет стрелок, debounce 120 мс, светлого active.

## Цикл 6: DIVERGES

kind-карта слепляет 8/11 с методом; стрелки, debounce, светлый active.

## Цикл 7: DIVERGES

Глифы ctor/function/method слеплены в один; нет стрелок и debounce 120 мс; светлый active нет.

## Цикл 8: DIVERGES

Закрыто: debounce 120 мс, ↑/↓ по списку, Enter по активной строке, светлая тема —
сплошная accent-заливка активной строки (модуль CSS общий с QuickOpen).

Осталось: глифы ctor/function/method слеплены в один; живая проверка клавиатуры
probe-ом невозможна (см. 124).

---

## 127. command-palette — **DIVERGES** (цикл 7)

*История: ц2:DIVERGES, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](127-command-palette/original.png)
![наш](127-command-palette/ours.png)

### Оригинал

# 127 command-palette — оригинал
Файлы: `kamin-ide/src/renderer/components/command-palette/CommandPalette.tsx` (26-90), `CommandPalette.module.css`

## JSX-структура (кратко, вложенность)
```
button.scrim type=button aria-label="Close command palette" onClick → close
└─ div.palette role=dialog aria-label="Command palette" onClick stopPropagation
   ├─ div.inputRow
   │  ├─ span.codicon.codicon-search
   │  ├─ input.input [ref фокус] placeholder="Type a command name…"
   │  │    Enter → выполнить list[0]
   │  └─ kbd.kbd "Esc"
   ├─ ul.list
   │  ├─ (пусто) li.empty  No commands match "{query}"
   │  └─ li > button.row × N (кап PALETTE_MAX_ROWS)
   │     ├─ span.title  [span.category "{category}: "] {title}
   │     └─ span.id {command id}
   └─ div.footer "{N} command(s) · Enter to run"
```
- Скрим — `<button>` (клавиатурно-достижимая цель закрытия).

## Метрики (ИЗ CSS, точные значения)
`.scrim`:
- position: fixed; inset: 0; z-index: var(--z-modal)
- background: var(--overlay-modal)
- display: flex; justify-content: center; padding: 0; padding-top: var(--layout-palette-top-offset)
- animation: fade 0.12s ease-out; border: none; cursor: default; font: inherit; color: inherit

`.palette`:
- width: var(--layout-palette-width); max-width: calc(100vw - 32px); max-height: var(--layout-palette-max-height)
- background: var(--bg-mantle)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 80%, transparent)
- border-radius: var(--radius-md); box-shadow: var(--shadow-modal)
- flex column; overflow: hidden

`.inputRow`:
- display: flex; align-items: center; gap: var(--space-2)
- padding: var(--space-3) var(--space-4)
- border-bottom: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)
- `.codicon`: font-size: 16px !important; color: var(--text-muted)

`.input`: flex: 1; background: transparent; border: none; outline: none; color: var(--text-primary); font-size: var(--fs-md)

`.kbd`:
- font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-muted)
- background: color-mix(in srgb, var(--bg-overlay) 50%, transparent)
- padding: 2px 6px; border-radius: var(--radius-xs)

`.list`: list-style: none; margin: 0; padding: var(--space-1); overflow: auto; flex: 1; flex column; gap: 1px

`.row`:
- display: flex; align-items: baseline; justify-content: space-between; gap: var(--space-3)
- width: 100%; padding: var(--space-2) var(--space-3)
- border: none; background: transparent; border-radius: var(--radius-sm)
- cursor: pointer; text-align: left; font: inherit; font-size: var(--fs-md); color: inherit

`.title`: color: var(--text-primary); flex: 1
`.category`: color: var(--text-muted); font-weight: 500
`.id`: font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-muted)
`.empty`: padding: var(--space-3) var(--space-4); color: var(--text-muted); font-style: italic
`.footer`: padding: var(--space-2) var(--space-4); border-top: 1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent); font-size: var(--fs-xs); color: var(--text-muted)

## Состояния (классы-варианты с метриками)
- `.row:hover`: background: color-mix(in srgb, var(--accent-primary) 18%, transparent)
- `.list > li:first-child .row` (подсветка первой строки — цель Enter): background: color-mix(in srgb, var(--accent-primary) 12%, transparent)

### Наша реализация

# 127 command-palette — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\command_palette.rs:118-251 (command_palette), 35-115 (filter, command_row); инпут — root.rs:3909-3915; константы crates\metrics\src\lib.rs:23-24

## Структура (gpui-дерево кратко)
```
div (скрим): absolute.size_full.pt(84).bg(rgba(0,0,0,.5)) — клик = close, Esc/Enter на key_down
└─ панель: w(640).max_h((vh−84−48)×0.75).rounded(12).bg(bg_mantle).shadow(0 8 32 .5)
   ├─ input-ряд: search-codicon 16 + Input + kbd «Esc»
   ├─ список p(4).gap(1), row × ≤50:
   │  row: [category: ][title] … [id mono]
   └─ футер «{N} command(s) · Enter to run»
```
Фильтр: substring по title/id/category, внутренние `_`-команды скрыты. Enter запускает первый ряд.

## Метрики (из кода, точные)
- Скрим rgba(0,0,0,.5); top 84 (PALETTE_TOP_OFFSET), w 640 (PALETTE_WIDTH)
- Панель: rounded 12, bg p.bg_mantle #262533, border 1 p.bg_surface a=.8, shadow 0 8 32 rgba(0,0,0,.5)
- Input-ряд: px 16 (SPACE_4), py 6, gap 8, border-b bg_surface a=.6; search 16px p.text_muted
- kbd: «JetBrains Mono» fs 11, bg p.bg_overlay #515567 a=.5, px 6, py 2, rounded 4
- Row: px 12 (SPACE_3), py 8 (SPACE_2), rounded 8, fs 13 (FS_MD), baseline, justify_between, gap 12; category p.text_muted weight 500; title p.text_primary ellipsis; id mono fs 11 p.text_muted
- Первый ряд: bg p.accent_primary a=.12; hover a=.18
- Футер: px 16, py 8, border-t bg_surface a=.6, fs 11, p.text_muted
- Empty: px 16, py 12, italic, p.text_muted

## Отличия от original.md той же папки
1. Input-ряд py 6 вместо space-3 (12) — осознанная компенсация: gpui-Input несёт собственную высоту ~30px (комментарий в коде).
2. max-h = (vh−84−48)×0.75 — аппроксимация 60vh, не точное значение.
3. Empty-текст «No commands match» без кавычек-query (оригинал: `No commands match "{query}"`).
4. Скрим — div, не `<button aria-label>`; клавиатурная навигация стрелками отсутствует (Enter = первый).
5. Первый ряд 12% + hover 18% — совпадают; футер/kbd/цвета — совпадают.
6. MAX_ROWS 50 — кап как PALETTE_MAX_ROWS оригинала.

## Дополнение атрибутов (цикл 10)

- скругления: панель border-radius 12 (RADIUS_MD) (command_palette.rs:191); строка row border-radius 8 (RADIUS_SM) (command_palette.rs:96); kbd «Esc» border-radius 4 (RADIUS_XS) (command_palette.rs:235); у скрима, input-ряда и футера скруглений нет

### Вердикты

# 127 — verdict (review cycle 1)
VERDICT: DIVERGES
max-h эвристика vs 60vh; empty без {query}; inputRow py6 vs 12/16; скрим не
кнопка (a11y). Остальное — 1:1 (shadow-modal, футер, ряды).

## Цикл 2: DIVERGES
60vh; empty без {query}; inputRow 12/16; скрим-кнопка a11y.

## Цикл 5: DIVERGES

Палитра команд: ряды, футер, kbd, подсветка 12% + hover 18%, shadow-modal — 1:1; inputRow py6 замером попадает (~42 против 40). Остаток: `max_h` = (vh−84−48)×0.75 = 729 логических против 60vh = 662 (низ панели уезжает ниже); empty без кавычек вокруг запроса; скрим не `<button aria-label>`; нет стрелок. Оригинальный кадр палитру не показал — оригинал по скрину не проверен.

## Цикл 6: DIVERGES

`max_h` 60vh — закрыто ✓. **Волна 10**: инпут-ряд переведён на фиксированные 44 (был `py 6` под старую метрику). Осталось: кавычки вокруг запроса в empty, стрелки, скрим не focusable.

## Цикл 7: DIVERGES

Empty-текст без кавычек и запроса. ЛОЖНАЯ: стрелок нет и в оригинале. max_h 60vh и ряд 44 закрыты.

---

## 128. toasts-stack — **DIVERGES** (цикл 7)

*История: ц2:MATCH, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

*кадр «оригинал» отсутствует*
![наш](128-toasts-stack/ours.png)

### Оригинал

# 128 toasts-stack — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/Toasts.tsx` (21-63), `Toasts.module.css`

## JSX-структура (кратко, вложенность)
```
div.stack role=region aria-label="Notifications"        (null при 0 тостах)
└─ div.toast .{info|success|warning|error} [.leaving] role={error → alert, иначе status} × N
   ├─ span.codicon.codicon-{info|pass|warning|error}.icon
   ├─ div.content
   │  ├─ (title) div.title
   │  ├─ div.message
   │  └─ (actions) div.actions
   │     └─ button.actionBtn {label} × N   (клик резолвит промис pushToast)
   └─ button.dismiss aria-label="Dismiss notification"
      └─ span.codicon.codicon-close
```
- Иконки: info→info, success→pass, warning→warning, error→error.

## Метрики (ИЗ CSS, точные значения)
`.stack`:
- position: fixed; bottom: 36px; right: var(--space-4)
- display: flex; flex-direction: column; gap: var(--space-2)
- z-index: var(--z-toast); pointer-events: none; max-width: 360px

`.toast`:
- display: flex; align-items: flex-start; gap: var(--space-3)
- padding: var(--space-3) var(--space-4)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 70%, transparent)
- border-radius: var(--radius-md)
- background: color-mix(in srgb, var(--bg-surface) 50%, transparent)
- backdrop-filter: blur(8px)
- box-shadow: var(--shadow-card-popup)
- font-size: var(--fs-sm); color: var(--text-primary)
- pointer-events: auto
- animation: slide 0.18s ease-out (from translateX(8px)/opacity 0 → to translateX(0)/opacity 1)

`.icon`: flex-shrink: 0; margin-top: 2px; font-size: var(--fs-md)
`.content`: flex: 1; min-width: 0
`.title`: font-weight: 600; margin-bottom: 2px
`.message`: color: var(--text-secondary); word-break: break-word

`.actions`: display: flex; gap: var(--space-2); margin-top: var(--space-2); flex-wrap: wrap

`.actionBtn`:
- padding: 2px var(--space-3); border-radius: var(--radius-xs)
- border: 1px solid color-mix(in srgb, var(--accent-primary) 40%, transparent)
- background: transparent; color: var(--accent-primary)
- font-size: var(--fs-xs); cursor: pointer; font-family: inherit
- transition: background var(--transition-fast)

`.dismiss`:
- flex-shrink: 0; padding: 0; width: 16px; height: 16px
- display: grid; place-items: center
- background: none; border: none; color: var(--text-disabled); cursor: pointer; font-size: var(--fs-xs)

## Состояния (классы-варианты с метриками)
- `.toast.leaving`: animation: slideOut 0.18s ease-in forwards (to translateX(12px)/opacity 0); pointer-events: none. Длительность = TOAST_EXIT_MS в state.ts.
- `.actionBtn:hover`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- `.dismiss:hover`: color: var(--text-primary)
- Severity — ТОЛЬКО цвет иконки (без рейла/тинта): `.info .icon` var(--accent-blue); `.success .icon` var(--accent-green); `.warning .icon` var(--accent-yellow); `.error .icon` var(--accent-red)

### Наша реализация

# 128 toasts-stack — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\toasts.rs:57-180 (toast_card, toasts), crates\shell\src\overlay.rs:728-745 (позиция в overlay-окне), root.rs:1547-1556 (авто-скрытие 5s)

## Структура (gpui-дерево кратко)
```
overlay-обёртка: absolute.bottom(36).right(16).w(360) + hit_area()
└─ стек: flex_col.gap(8).w_full
   └─ card × N: items_start.gap(12)
      ├─ severity-codicon (info \u{ea74} | pass \u{eba4} | warning \u{ea6c} | error \u{ea87})
      ├─ content: (title) + message + (action-чипы)
      └─ dismiss 16×16 (codicon close)
```
Action-чип и dismiss тостов `shellreq-N` (showMessage) отвечают хосту (label / null). Не-sticky тосты авто-скрываются через 5s.

## Метрики (из кода, точные)
- Позиция: bottom 36, right 16 (SPACE_4), ширина обёртки 360
- Card: p 12 (SPACE_3) + px 16 (SPACE_4), rounded 12 (RADIUS_MD), bg p.bg_surface #3d3f51 a=.92, border 1 p.bg_surface a=.7, fs 12 (FS_SM)
- Иконка: 13px (FS_MD), mt 2, цвет severity: info p.accent_blue #89b4fa, success p.accent_green #a6e3a1, warning p.accent_yellow #f9e2af, error p.accent_red #f38ba8
- Title: weight 600, mb 2, p.text_primary; message p.text_secondary
- Actions: flex-wrap, gap 8, mt 8; чип px 12 py 2 rounded 4, border p.accent_primary a=.4, текст fs 11 p.accent_primary, hover bg accent a=.14
- Dismiss: 16×16, fs 11, p.text_disabled #60667b, hover text_primary

## Отличия от original.md той же папки
1. Фон карточки — почти непрозрачный bg_surface 92% вместо 50% + backdrop-blur(8px): blur невозможен в overlay-окне без альфы, поэтому тинт добит до непрозрачности.
2. Анимации slide-in 0.18s и `.leaving` slideOut — НЕ РЕАЛИЗОВАНЫ (появление/уход мгновенны).
3. box-shadow (shadow-card-popup) отсутствует.
4. max-width 360 стал ФИКСИРОВАННОЙ шириной обёртки (карточки всегда 360, у оригинала — по контенту до 360).
5. role=region/alert/status и aria-label нет.
6. Позиция bottom 36 / right space-4, gap 8, паддинги, severity-мэппинг иконок и цветов, чипы и dismiss — совпадают.

### Вердикты

# 128 — verdict (review cycle 1)
VERDICT: MATCH
Стек/карточка (surface50%+border70%+card-popup+max-w360)/иконки/акции/dismiss — 1:1
(blur+slide — deviation).

## Цикл 2: MATCH

## Цикл 5: DIVERGES

Тосты: bg surface 50%, border 70%, `shadow_card_popup`, `max_w 360` — исправлены. Высота карточки 45.6 против 38.4 — **корень (line-height overlay) исправлен волной 8**. Остаток: нет slide-in 0.18s и `.leaving` slideOut; правый отступ ~9.6 вместо 16 — похоже на сдвиг origin overlay по X, нужна проверка. Оригинального кадра нет.

## Цикл 6: DIVERGES

Рецепт 1:1. Осталось: slide-in/`.leaving`; правый отступ замерен 8.8 вместо 16 — вместе с альфой детей overlay указывает на сам overlay-слой, нужна живая проверка.

## Цикл 7: DIVERGES

ЛОЖНАЯ: правый отступ ровно 16.0 после калибровки (капчур оверлея смещён +9 физ. по X). Осталось: slide-in и blur(8px).

---

## 129. tooltip — **DIVERGES** (цикл 7)

*История: ц2:MATCH, ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](129-tooltip/original.png)
![наш](129-tooltip/ours.png)

### Оригинал

# 129 tooltip — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/Tooltip.tsx` (123-138), `Tooltip.module.css`

## JSX-структура (кратко, вложенность)
```
div.tooltip [ref] [data-tooltip-popup]
  style={ left: {px}, top: {px}, opacity: visible?1:0, visibility: anchor?visible:hidden }
└─ {text}
```
- Единственный инстанс на документ (монтируется в App.tsx); слушает `pointerenter`/`pointerleave` (capture) по `closest("[data-tooltip]")`.
- Двухпроходное позиционирование: стадия 1 — рендер невидимым (opacity 0), стадия 2 — useLayoutEffect измеряет getBoundingClientRect и `clampToViewport({ side: "top", offset: 8 })` (`OFFSET_PX = 8`) до пейнта.
- Скрытие: pointerleave, mousedown, visibilitychange, window blur, scroll (capture).
- Принимает тултипы из вебвью через сигнал `webviewTooltip` (anchor уже в host-координатах).

## Метрики (ИЗ CSS, точные значения)
`.tooltip`:
- position: fixed; pointer-events: none; z-index: var(--z-tooltip)
- background: var(--bg-surface); color: var(--text-primary)
- padding: 4px 8px
- border-radius: var(--radius-xs)
- font-size: var(--fs-xs); line-height: var(--lh-snug)
- max-width: min(640px, calc(100vw - 16px))
- white-space: nowrap; overflow: hidden; text-overflow: ellipsis
- transition: opacity 0.1s
- box-shadow: var(--shadow-mini)
- left/top задаются inline (px), opacity 0→1 по завершении clamp

## Состояния (классы-варианты с метриками)
- Измерение (стадия 1): visibility: visible, opacity: 0.
- Показан: opacity: 1 (fade 0.1s).
- Нет якоря: visibility: hidden.
- hover/active-классов нет.

### Наша реализация

# 129 tooltip — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\tooltip.rs (весь: `KaminTooltip`, `tooltip_box_at`, `half_width`, `tooltip`); crates\shell\src\overlay.rs:719-722 (`tooltip_live` → `tooltip_box_at`), overlay.rs:191 (`tooltip_region`); crates\shell\src\root.rs:170, 667-673 (состояние `tooltip_live`), 2405-2407 (overlay-окно не прячется, пока тултип жив)

## Структура/содержание
```
элемент.tooltip(tooltip("текст")) → gpui показывает KaminTooltip по ховеру (задержка gpui)
MAIN-окно: KaminTooltip::render НЕ рисует бокс (WebView2-чайлды перекрыли бы) →
  шлёт ShellEvent::TooltipShow(text, mouse.x, mouse.y) с позицией мыши ЭТОГО окна
overlay-окно: tooltip_box_at(text, x, y):
  half = shape_line(text, 11px, UI_FONT «Bricolage Grotesque»).width.min(640) / 2 + 8
  left = (x − half).clamp(4, max(vw − 2·half − 4, 4))     ← кламп по X к вьюпорту
  top  = (y − 14 − box_h).max(4)                          ← НАД курсором
  box_h = 11 × 1.3 + 8 = 22.3
Drop KaminTooltip → TooltipHide (гасит overlay-копию)
```
Палитра — `kamin_theme::current_palette()` (следует активной теме). Отдельная ветка `KaminTooltip::render` внутри самого overlay-окна ставит бокс относительным `absolute` (left −half, top −(box_h + 14)) без клампа — боевой путь main → overlay идёт через `tooltip_box_at`.

## Метрики (из кода, точные)
- отступы: px 8 (SPACE_2), py 4
- гэпы: N/A: гэпы — у бокса ровно один ребёнок (текст), `gap` не задан
- цвета: bg p.bg_surface #3d3f51 (dark) / #e6e1d4 (light); текст p.text_primary #cfd4e2 / #322e28; тень 0 2 8 rgba(0,0,0,0.3) (= `--shadow-mini`)
- скругления: rounded 4 (RADIUS_XS)
- шрифты: font-size 11 (FS_XS), line-height 14.3 (11 × 1.3), font-weight 400 NORMAL, семейство `crate::root::UI_FONT` = «Bricolage Grotesque»
- ховер: N/A: ховер — тултип сам не hoverable; hit-регион ставится только на бокс (`tooltip_region`), состояний ховера нет
- прочее: max-width 640, `whitespace_nowrap` + `overflow_hidden`; смещение от курсора 14px вверх; горизонтальный кламп 4px от краёв

## Отличия от original.md той же папки
1. Якорь — КУРСОР, а не элемент: центр по X от позиции мыши, верх бокса = y − 14 − box_h. Оригинал считает от `getBoundingClientRect` элемента с `{ side: "top", offset: 8 }`.
2. Кламп есть по X (4 … vw − 2·half − 4) и по верхней границе (`.max(4)`); переворота под курсор при нехватке места сверху и клампа по правому/нижнему краю в стиле `clampToViewport` нет.
3. Fade-in 0.1s и двухпроходное «невидимое измерение» (opacity 0 → layout-измерение → opacity 1) отсутствуют: ширина меряется шейпером ДО рендера.
4. `text-overflow: ellipsis` нет — только `overflow_hidden` (обрезка без многоточия); `max-width: 640` есть, но без `min(640px, calc(100vw − 16px))`.
5. Тултипы из вебвью (сигнал `webviewTooltip`) не принимаются.
6. Рисуется в ОТДЕЛЬНОМ overlay-окне поверх WebView2, а не в DOM того же документа; показ — по gpui-ховеру с его задержкой, а не по `pointerenter` на `[data-tooltip]`.
7. Скрытие — только через `Drop` у `KaminTooltip`; явных подписок на `mousedown` / `visibilitychange` / `window blur` / `scroll (capture)` нет.
8. Палитра берётся из `current_palette()` — light-тема поддержана (прежний хардкод DARK устранён).
9. Метрики покоя (bg-surface, padding 4×8, radius-xs 4, fs-xs 11, lh-snug 1.3, shadow-mini, nowrap, max-width 640, pointer-events нет) — совпадают 1:1.

### Вердикты

# 129 — verdict (review cycle 1)
VERDICT: MATCH
Бокс/палитра live/позиция над элементом+clamp — 1:1. Зазоры: нет max-w vw-16-клампа
и text-ellipsis (жёсткая обрезка). (якорь по курсору — deviation gpui).

## Цикл 2: MATCH
(микро: ellipsis, кламп 100vw-16)

## Цикл 5: DIVERGES

Тултип: геометрия 1:1 и подтверждена кадром (бокс 22.4 логических у обеих сторон; px8/py4/r4/fs11/lh1.3/nowrap/max-w640), позиция над якорем + кламп и `current_palette()` — исправлены. Остаток: заливка в кадре замерена как #181920 против #3d3f51 у оригинала (модалка в том же кадре опаковая — на артефакт захвата не спишешь, нужна живая проверка); `tooltip_box_at` не пушит region-rect; тень захардкожена тёмной .3 вместо `shadows::mini()`; нет ellipsis и клампа `min(100vw−16)`.

## Цикл 6: DIVERGES

Геометрия 1:1. Осталось: `tooltip_box_at` не пушит region, тень захардкожена вместо `shadows::mini()`, нет ellipsis и клампа ширины; замеренная альфа заливки — общий вопрос overlay-слоя.

## Цикл 7: DIVERGES

tooltip_box_at не пушит tooltip_region; тень захардкожена под тёмную; нет text_ellipsis; max_w без min(640, 100vw-16).

---

# Зона 130-159 — Токены дизайна, sample-компоненты, глобальные стили

## 130. design-color-tokens — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](130-design-color-tokens/original.png)
![наш](130-design-color-tokens/ours.png)

### Оригинал

# 130 design-color-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:13-42, design-sections.module.css:1-50, theme/variables.css, theme/dark-theme.css

## Содержание/структура
`ColorTokens()` — `.colorGroups` (flex column, gap `--space-4` 16px) из 4 групп `.colorGroup` (flex column, gap `--space-2` 8px):
- `<h3 class=groupLabel>` — заголовок группы (Surface / Text / Accent / Semantic)
- `.swatches` — grid `repeat(auto-fill, minmax(180px, 1fr))`, gap 8px
- каждый `.swatch`: `.swatchChip` (div с `style="background: var(--<token>)"`) + `<code class=swatchName>--<token></code>`

Группы и токены (порядок из COLOR_GROUPS):
- Surface: bg-primary, bg-base, bg-mantle, bg-sidebar, bg-surface, bg-overlay
- Text: text-primary, text-subtext, text-secondary, text-muted, text-disabled
- Accent: accent-blue, accent-sapphire, accent-teal, accent-green, accent-yellow, accent-orange, accent-red, accent-maroon, accent-pink, accent-purple, accent-rosewater
- Semantic: accent-primary, accent-action, accent-action-hover, accent-action-fg

Никакой JS-резолюции значений — браузер резолвит `var(--token)` при пейнте (dark/light переключается само).

## Метрики
CSS:
- `.groupLabel`: margin 0; font-size `--fs-xs` (11px); text-transform uppercase; letter-spacing 0.06em; color `--text-muted`
- `.swatches`: grid, `grid-template-columns: repeat(auto-fill, minmax(180px, 1fr))`, gap 8px
- `.swatch`: flex, align-items center, gap 8px, padding 8px, background `color-mix(in srgb, var(--bg-surface) 30%, transparent)`, border-radius `--radius-xs` (4px)
- `.swatchChip`: 28×28px, border-radius 4px, border `1px solid color-mix(in srgb, var(--text-primary) 12%, transparent)`, flex-shrink 0
- `.swatchName`: font-family `--font-mono`, font-size 11px, color `--text-secondary`, word-break break-all

Полная таблица токен → значение (дефолтная тёмная тема, dark-theme.css / :root в variables.css):

| Токен | Значение (dark) |
|---|---|
| --bg-primary | #313240 |
| --bg-base | #313240 (alias --bg-primary) |
| --bg-mantle | #262533 |
| --bg-sidebar | #1d1d28 |
| --bg-surface | #3d3f51 |
| --bg-overlay | #515567 |
| --text-primary | #cfd4e2 |
| --text-subtext | #afb6ca |
| --text-secondary | #adb3c7 |
| --text-muted | #838aa0 |
| --text-disabled | #60667b |
| --accent-blue | #89b4fa |
| --accent-sapphire | #74c7ec |
| --accent-teal | #94e2d5 |
| --accent-green | #a6e3a1 |
| --accent-yellow | #f9e2af |
| --accent-orange | #fab387 |
| --accent-red | #f38ba8 |
| --accent-maroon | #eba0ac |
| --accent-pink | #f5c2e7 |
| --accent-purple | #cba6f7 |
| --accent-rosewater | #f5e0dc |
| --accent-primary | var(--accent-blue) → #89b4fa |
| --accent-action | var(--accent-blue) → #89b4fa |
| --accent-action-hover | var(--accent-sapphire) → #74c7ec |
| --accent-action-fg | var(--bg-primary) → #313240 |

Сопутствующие цветовые токены темы, не показанные в свотчах (dark-theme.css / variables.css :root, для полноты палитры):

| Токен | Значение (dark) |
|---|---|
| --glint-border | linear-gradient(135deg, rgba(255,255,255,0.18) 0%, var(--bg-mantle) 22%, var(--bg-mantle) 78%, rgba(255,255,255,0.18) 100%) — в :root-фоллбеке mid-стопы var(--bg-base) |
| --editor-bg | #1d1c25 |
| --editor-fg | #dcdce4 |
| --editor-cursor | #a0a0d0 |
| --overlay-modal | rgba(0, 0, 0, 0.5) |
| --overlay-soft | rgba(0, 0, 0, 0.35) |
| --overlay-deep | rgba(0, 0, 0, 0.6) |
| --bg-surface-hover | #3b3b52 |
| --bg-overlay-hover | #3e3e56 |
| --bg-tint-red | #2e1e22 |
| --bg-tint-red-soft | #45283b |
| --bg-tint-green | #1e2e1e |
| --bg-tint-green-soft | #1e2e1e |
| --bg-tint-orange | #2e1e1e |
| --bg-tint-blue | #1a1a27 |
| --accent-blue-soft | #b4d0fb |
| --accent-blue-soft-2 | #b4befe |
| --accent-blue-soft-3 | #c0d3ff |
| --accent-purple-soft | #b48bef |
| --accent-green-soft | #94d899 |
| --accent-red-dark | #e06c8a |
| --accent-red-dark-2 | #e06c88 |
| --accent-red-dark-3 | #e87c99 |
| --accent-orange-dark | #f9b36d |
| --accent-yellow-dark | #8a7a2e |
| --text-muted-2 | #7f849c |
| --text-muted-light | #acb2d2 |
| --divider-soft | color-mix(in srgb, var(--text-primary) 6%, transparent) |

Семейство semantic-primary алиасов (variables.css): --accent-primary-soft → --accent-blue-soft; --accent-primary-soft-2 → --accent-blue-soft-2; --accent-primary-soft-3 → --accent-blue-soft-3; --bg-tint-primary → --bg-tint-blue; --tint-primary-* → --tint-blue-*.

Tint-токены (color-mix, variables.css:103-151): --tint-red-soft 10%, --tint-red-soft-2 8%, --tint-red-medium 18%, --tint-red-border 30%, --tint-red-border-strong 40% (от accent-red); --tint-blue-soft 6%, --tint-blue-medium 12%, --tint-blue-strong 25%, --tint-blue-border 25%, --tint-blue-border-strong 50% (от accent-blue); --tint-yellow-soft 8%/-medium 12%/-strong 18%/-border 30% (accent-yellow); --tint-green-soft 8%/-medium 14%/-strong 18%/-border 40% (accent-green); --tint-purple-soft 8%/-medium 12%/-border 25% (accent-purple); --tint-orange-soft 14% (accent-orange); --tint-muted-soft 8%/-medium 18% (text-muted); --tint-overlay-scrim 70%/-heavy 92% (bg-sidebar); --tint-surface-soft 40%/-medium 55% (bg-surface); --tint-overlay-medium 50%/-strong 80% (bg-overlay).

## Состояния/варианты
Статичная витрина, интерактива нет. Значения свотчей меняются вместе с темой (`[data-theme="dark"]` / `[data-theme="light"]` на `<html>`; `:root`-фоллбек в variables.css зеркалит dark для первого пейнта).

### Наша реализация

# 130 design-color-tokens — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\theme\src\palette.rs:1-144 (`Palette`, const `DARK` / `LIGHT`, тесты инвариантов); crates\shell\src\ui\design_panel.rs:452-481 (`swatch`), 483-493 (`group_label`), 496-560 (4 группы, 26 токенов), 26-72 (`section`), 882-887 (секция «Colors»)

## Структура/содержание
Палитра — Rust-структура `Palette` (const `DARK` / `LIGHT`), без CSS-переменных; contributed-темы позже лягут рантайм-мапой поверх.
Секция «Colors»: колонка групп (gap 16) из 4 групп (gap 8), каждая = `group_label` + flex-wrap ряд свотчей (gap 8).
```
colors div.flex_col.gap(16)
└─ группа × 4: div.flex_col.gap(8)
   ├─ group_label «SURFACE» / «TEXT» / «ACCENT» / «SEMANTIC»
   └─ swatches div.flex.flex_wrap.gap(8)
      └─ swatch × N: div.flex.items_center.gap(8).min_w(180).flex_grow.p(8).rounded(4).bg(bg_surface α .3)
         ├─ чип div 28×28.rounded(4).border_1(text_primary α .12).bg(токен)
         └─ подпись «--{token}» (mono, FS_XS, text_secondary)
```
Группы и токены — тот же порядок, что `COLOR_GROUPS` оригинала: Surface (bg-primary, bg-base, bg-mantle, bg-sidebar, bg-surface, bg-overlay), Text (text-primary, text-subtext, text-secondary, text-muted, text-disabled), Accent (blue, sapphire, teal, green, yellow, orange, red, maroon, pink, purple, rosewater), Semantic (accent-primary, accent-action, accent-action-hover, accent-action-fg) — 26 токенов.

## Метрики (из кода, точные)
- отступы: свотч p 8 (SPACE_2); у групп и контейнера padding нет; внешний отступ даёт `section()` — тело p 16 (SPACE_4), секция mb 24 (SPACE_6)
- гэпы: контейнер групп gap 16 (SPACE_4); группа gap 8 (SPACE_2); ряд свотчей gap 8 (SPACE_2); внутри свотча gap 8 (SPACE_2)
- цвета: подложка свотча p.bg_surface #3d3f51 α 0.3; бордер чипа p.text_primary #cfd4e2 α 0.12; подпись p.text_secondary #adb3c7; `group_label` p.text_muted #838aa0; тело секции bg p.bg_mantle #262533 + border 1px p.bg_surface α 0.6; заголовок секции p.text_primary #cfd4e2, сабтайтл p.text_muted #838aa0
- скругления: свотч 4 (RADIUS_XS); чип 4 (RADIUS_XS); тело секции 12 (RADIUS_MD)
- шрифты: подпись свотча «JetBrains Mono» 11 (FS_XS) weight 400; `group_label` 11 (FS_XS) weight 700 BOLD (UA-дефолт `<h3>`), текст через `to_uppercase()`; заголовок секции 16 (FS_LG) / 600; сабтайтл 12 (FS_SM), line-height 15.6
- ховер: N/A: ховер — витрина статична, в секции Colors нет ни одного `.hover(...)` (совпадает с оригиналом)

Полная таблица наш-токен → значение, DARK (palette.rs:52-88):

| Токен | Значение |
|---|---|
| bg_primary | #313240 |
| bg_base | #313240 |
| bg_mantle | #262533 |
| bg_sidebar | #1d1d28 |
| bg_surface | #3d3f51 |
| bg_overlay | #515567 |
| editor_bg | #1d1c25 |
| editor_fg | #dcdce4 |
| editor_cursor | #a0a0d0 |
| text_primary | #cfd4e2 |
| text_subtext | #afb6ca |
| text_secondary | #adb3c7 |
| text_muted | #838aa0 |
| text_disabled | #60667b |
| text_muted_2 | #7f849c |
| text_muted_light | #acb2d2 |
| accent_blue | #89b4fa |
| accent_sapphire | #74c7ec |
| accent_red | #f38ba8 |
| accent_maroon | #eba0ac |
| accent_green | #a6e3a1 |
| accent_yellow | #f9e2af |
| accent_pink | #f5c2e7 |
| accent_purple | #cba6f7 |
| accent_orange | #fab387 |
| accent_teal | #94e2d5 |
| accent_rosewater | #f5e0dc |
| accent_action | #89b4fa (= blue) |
| accent_action_hover | #74c7ec |
| accent_action_fg | #313240 |
| accent_primary | #89b4fa |
| bg_surface_hover | #3b3b52 |
| bg_overlay_hover | #3e3e56 |
| glint_edge | rgba(255,255,255,0.18) |
| glint_mid | #262533 (= bg_mantle) |

LIGHT (palette.rs:90-126): bg_primary #f6efeb; bg_base #fbf8f1; bg_mantle #fbf7f4; bg_sidebar #f4f1ea; bg_surface #e6e1d4; bg_overlay #d6d0c0; editor_bg #fcfaf6; editor_fg #48433c; editor_cursor #48433c; text_primary #322e28; text_subtext #463f37; text_secondary #524c43; text_muted #6e685d; text_disabled #938e82; text_muted_2 #524c43; text_muted_light #524c43; accent_blue #3b6fc4; sapphire #3a8aa3; red #ca3939; maroon #d35a5a; green #5e9855; yellow #c89a3f; pink #c46598; purple #8a5fc8; orange #da8343; teal #4a9999; rosewater #c08571; accent_action #da8343 (= orange); action_hover #b16527; action_fg #ffffff; accent_primary #da8343; bg_surface_hover #d8d4c4; bg_overlay_hover #c2bcab; glint_edge rgba(60,40,20,0.18); glint_mid #e6e1d4 (= bg_surface).

Инварианты закрыты тестами (palette.rs:132-143): dark action = blue, light action = orange; glint_mid = bg_mantle (dark) / bg_surface (light).

## Отличия от original.md той же папки
Значения всех присутствующих токенов совпадают токен-в-токен (dark): bg-primary/base/mantle/sidebar/surface/overlay, все 5 text-*, все 11 accent-*, accent-primary/action/action-hover/action-fg, editor-bg/fg/cursor, bg-surface-hover #3b3b52, bg-overlay-hover #3e3e56, text-muted-2 #7f849c, text-muted-light #acb2d2, glint (edge rgba(255,255,255,.18), mid = bg-mantle как в dark-theme.css; `:root`-фоллбек с mid = bg-base не воспроизводим — не нужен). Расхождений в ЗНАЧЕНИЯХ НЕТ.

Отсутствуют в палитре (в оригинале есть, у нас нет полей):
- `--overlay-modal` rgba(0,0,0,.5) / `--overlay-soft` .35 / `--overlay-deep` .6 — у нас скрим модалки захардкожен `rgba(0,0,0,0.6)` в `modal.rs:64-71` (это ровно overlay-deep; overlay-modal и overlay-soft не используются нигде);
- вся семья `--bg-tint-*` (red / red-soft / green / green-soft / orange / blue);
- `--accent-blue-soft/-2/-3`, `--accent-purple-soft`, `--accent-green-soft`, `--accent-red-dark/-2/-3`, `--accent-orange-dark`, `--accent-yellow-dark`;
- `--divider-soft` (color-mix text-primary 6%) — у нас собирается ad-hoc `tint(text_primary, 0.06)` по месту (напр. editor_tabs.rs:272);
- все `--tint-*` color-mix токены — каждое место делает `tint()` со своей α;
- семейство `--accent-primary-soft` / `--bg-tint-primary` / `--tint-primary-*` алиасов.

Отличия витрины (секция Colors):
- состав и порядок 26 токенов, 4 группы, `group_label` uppercase — совпадают 1:1 с `COLOR_GROUPS`;
- чип 28×28, border text-primary 12%, свотч p 8 + bg `bg-surface 30%` + radius-xs, подпись mono FS_XS — совпадают;
- layout: CSS-grid `repeat(auto-fill, minmax(180px, 1fr))` заменён на flex-wrap с `min_w 180 + flex_grow` (в gpui нет grid) — перенос и растяжка последнего ряда ведут себя иначе;
- `letter-spacing: 0.06em` у `.groupLabel` в gpui недоступен;
- у подписи нет `word-break: break-all` (длинные имена не рвутся);
- подпись — обычный div с моно-семейством, а не `<code>` (визуально эквивалентно, семантики нет);
- цвет подписи `text_secondary` совпадает с оригиналом.

### Вердикты

# 130 design-color-tokens — verdict (review cycle 1)
VERDICT: DIVERGES
Частично реализовано: плоские 12 свотчей 22×22 (design_panel.rs:73-90, 53-59).
Оригинал: 4 группы (Surface/Text/Accent/Semantic), 27 токенов, чипы 28×28,
grid auto-fill minmax(180px,1fr), .swatch p8 + bg-surface@30% + radius-xs,
mono-имена --<token>. Отсутствуют: bg-base, bg-mantle, text-subtext,
accent-sapphire/teal/green/yellow/orange/maroon/pink/purple/rosewater,
accent-action-hover, accent-action-fg.

## Цикл 5: DIVERGES

Все 26 токенов, 4 группы, чип 28×28, p8/bg-surface 30%/radius-xs и mono-имена — 1:1. Расхождение одно: `.swatches` у оригинала `grid repeat(auto-fill, minmax(180px,1fr))` (ячейка тянется), у нас жёсткая `w(180)` во flex-wrap → «--accent-action-hover» упирается в правый край карточки. Чинить: `min_w(180) + flex_grow`. `letter-spacing 0.06em` у `.groupLabel` — ограничение gpui.

## Цикл 6: DIVERGES

grid→flex: полные ряды совпали (5 ячеек по 206.4), но сирота-ряд растягивается — bg-overlay и accent-rosewater по 1064.0 вместо 206.4; Semantic-ряд 256.8/257.6/268.0/257.6. Плюс .groupLabel это <h3> без font-weight → UA-дефолт 700, у нас 400.

---

## 131. design-typography-tokens — **MATCH** (цикл 5)

*История: ц5:MATCH*

![оригинал](131-design-typography-tokens/original.png)
![наш](131-design-typography-tokens/ours.png)

### Оригинал

# 131 design-typography-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:44-78, design-sections.module.css:52-91, theme/variables.css:172-204

## Содержание/структура
`TypographyTokens()` — `.typoStack` (flex column, gap `--space-3` 12px):
1. `.typoSample` (flex column, gap 2px): `<code class=tokenName>--font-sans</code>` + `<span style="font-family: var(--font-sans); font-size: var(--fs-lg)">Bricolage Grotesque — quick brown fox 0123456789</span>`
2. `.typoSample`: `<code class=tokenName>--font-mono</code>` + `<span style="font-family: var(--font-mono); font-size: var(--fs-md)">JetBrains Mono — quick brown fox 0123456789</span>`
3. `.typoScale` — 5 строк `.typoRow` по FS_SCALE: `<code class=tokenName>--fs-*</code>` + `<span class=tokenValue>NNpx</span>` + `<span style="font-size: var(--fs-*)">The five steps</span>`

## Метрики
CSS:
- `.typoStack`: flex column, gap 12px
- `.typoSample`: flex column, gap 2px
- `.typoScale`: flex column, gap 8px; margin-top 8px; padding-top 12px; border-top `1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent)`
- `.typoRow`: grid `90px 60px 1fr`; align-items baseline; gap 12px
- `.tokenName`: font `--font-mono`, 11px, color `--text-muted`
- `.tokenValue`: font `--font-mono`, 11px, color `--text-disabled`

Таблица токенов (variables.css):

| Токен | Значение |
|---|---|
| --font-sans | 'Bricolage Grotesque Variable', 'Bricolage Grotesque', -apple-system, sans-serif |
| --font-mono | 'JetBrains Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace |
| --fs-xs | 11px |
| --fs-sm | 12px |
| --fs-md | 13px |
| --fs-lg | 16px |
| --fs-xl | 22px |

Легаси-алиасы (в variables.css, в витрине не показаны): --fs-xxs → --fs-xs; --fs-10 → --fs-xs; --fs-base → --fs-sm; --fs-15 → --fs-md; --fs-18 → --fs-lg; --fs-2xl → --fs-lg.

Line-height токены (variables.css:199-204, в витрине не показаны): --lh-none 1; --lh-snug 1.3; --lh-normal 1.4; --lh-base 1.5; --lh-relaxed 1.6.

FS_SCALE в tsx дублирует значения строками: fs-xs 11px, fs-sm 12px, fs-md 13px, fs-lg 16px, fs-xl 22px.

Шрифт Bricolage Grotesque самохостится через `@import "@fontsource-variable/bricolage-grotesque"` (global.css:9); в списке два имени семейства — Variable (реальный бандл) и легаси 'Bricolage Grotesque' для копипасты из Bridge.

## Состояния/варианты
Статичная витрина. Демо-строки: sans-образец на `--fs-lg` (16px), mono-образец на `--fs-md` (13px), шкала — фраза «The five steps» в каждом из 5 размеров.

### Наша реализация

# 131 design-typography-tokens — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:562-616 (font-сэмплы + шкала), 427-438 (`token_name`), 440-450 (`token_value`), 888-893 (секция «Typography»); crates\metrics\src\lib.rs:41-47 (FS_*); crates\shell\src\main.rs:81-97 (шрифты вшиты в бинарь); crates\shell\src\root.rs:66 (`UI_FONT = "Bricolage Grotesque"`), design_panel.rs:19 (`MONO = "JetBrains Mono"`)

## Структура/содержание
```
typo div.flex_col.gap(12)
├─ font_sample("--font-sans", «Bricolage Grotesque», FS_LG 16)
│  └─ div.flex_col.gap(2): token_name «--font-sans» + строка
│     «Bricolage Grotesque — quick brown fox 0123456789»
├─ font_sample("--font-mono", «JetBrains Mono», FS_MD 13)
│  └─ «JetBrains Mono — quick brown fox 0123456789»
└─ typo_scale div.flex_col.gap(8).mt(8).pt(12).border_t_1(bg_surface α .5)
   └─ ряд × 5: div.flex.items_baseline.gap(12)
      ├─ token_name «--fs-*» w 90
      ├─ token_value «11px|12px|13px|16px|22px» w 60
      └─ «The five steps» размером шага
```
Шкала берётся ИЗ metrics: `m::FS_XS` 11, `m::FS_SM` 12, `m::FS_MD` 13, `m::FS_LG` 16, `m::FS_XL` 22 (design_panel.rs:587-593) — хардкодов в витрине нет.
Шрифты вшиты в бинарь (main.rs:81-97): `bricolage-latin.ttf`, `bricolage-latin-ext.ttf`, `JetBrainsMono-Variable.ttf`, `JetBrainsMono-Italic-Variable.ttf`.

## Метрики (из кода, точные)
- отступы: у `typo` и font-сэмплов padding/margin нет; шкала mt 8 (SPACE_2) + pt 12 (SPACE_3); колонка имени w 90 (в рядах шкалы) / без ширины в font-сэмплах; колонка значения w 60; внешний отступ даёт `section()` — тело p 16 (SPACE_4), секция mb 24 (SPACE_6)
- гэпы: `typo` gap 12 (SPACE_3); font-сэмпл gap 2; шкала gap 8 (SPACE_2); ряд шкалы gap 12 (SPACE_3)
- цвета: `token_name` p.text_muted #838aa0; `token_value` p.text_disabled #60667b; демо-строки p.text_primary #cfd4e2; разделитель над шкалой — border-top 1px p.bg_surface #3d3f51 α 0.5
- скругления: N/A: скругления — в секции Typography ни одного `rounded` (12 RADIUS_MD есть только у рамки `section()`)
- шрифты: `token_name` / `token_value` — «JetBrains Mono» 11 (FS_XS) weight 400; font-sans-сэмпл — «Bricolage Grotesque» 16 (FS_LG); font-mono-сэмпл — «JetBrains Mono» 13 (FS_MD); шкала — «The five steps» на 11 / 12 / 13 / 16 / 22 из `m::FS_*`; ряд выровнен по `items_baseline`
- ховер: N/A: ховер — витрина статична, ни одного `.hover(...)` в секции Typography

## Отличия от original.md той же папки
1. Значения шкалы совпадают 1:1: fs-xs 11, fs-sm 12, fs-md 13, fs-lg 16, fs-xl 22 — и в metrics, и в витрине (берутся из `m::FS_*`, а не дублируются строками, как `FS_SCALE` в tsx; строки-значения используются только для колонки `tokenValue`).
2. Оба font-сэмпла присутствуют, с теми же демо-фразами и размерами (`--font-sans` на fs-lg, `--font-mono` на fs-md) — совпадают.
3. Разделитель над шкалой (mt 8 + pt 12 + border-top bg-surface 50%) и gap 8 у шкалы — совпадают.
4. Ряд шкалы: CSS-grid `90px 60px 1fr` заменён на flex с фикс-ширинами 90 и 60 + `flex_1`-текст; align-items baseline и gap 12 совпадают. Растяжка последней колонки в gpui-flex отличается от `1fr`.
5. Демо-фраза «The five steps» и подписи `--fs-*` / «NNpx» — совпадают.
6. Шрифтовые стеки: у нас ровно 2 семейства из бинаря («Bricolage Grotesque», «JetBrains Mono»); CSS-фоллбеки (`Bricolage Grotesque Variable`, -apple-system, Fira Code, Cascadia Code, Consolas, monospace) не нужны и отсутствуют. Имя семейства у нас — легаси-вариант «Bricolage Grotesque» (не «… Variable»), name-таблица шрифта под это починена.
7. Легаси fs-алиасы (`--fs-xxs`, `--fs-10`, `--fs-base`, `--fs-15`, `--fs-18`, `--fs-2xl`) не портированы — в оригинальной витрине они тоже не показаны.
8. `--lh-*` токены (`none` 1, `snug` 1.3, `normal` 1.4, `base` 1.5, `relaxed` 1.6) в metrics отсутствуют вовсе: line-height по месту считается как `FS × 1.3` и т.п. В витрине они не показаны и у оригинала.
9. `<code class=tokenName>` → обычный div с моно-семейством: визуально эквивалентно, семантики нет.

### Вердикты

# 131 design-typography-tokens — verdict (review cycle 1)
VERDICT: DIVERGES
Подтверждено: шкала 11/12/13/16/22 = variables.css (metrics lib.rs:45-46; design_panel.rs:98-99).
Расхождения:
- Нет сэмпл-строк --font-sans (fs-lg, «Bricolage Grotesque — quick brown fox 0123456789»)
  и --font-mono (fs-md, «JetBrains Mono — …») — design_panel.rs:93-120.
- Строка: flex + один 80px-лейбл vs .typoRow grid 90px 60px 1fr
  (.tokenName mono 11 text-muted / .tokenValue mono 11 text-disabled).
- Нет font-mono на лейблах; gap 4 vs 8; нет сепаратора mt8/pt12/border-top bg-surface@50%.
- Демо-текст «Bricolage Grotesque — KaminIDE» vs «The five steps».

## Цикл 5: MATCH

Типографика: оба font-сэмпла, шкала 11/12/13/16/22, сепаратор mt8/pt12/border-top bg-surface 50%, колонки 90/60/1fr. Хардкоды 15/18 из цикла 1 убраны.

---

## 132. design-spacing-tokens — **MATCH** (цикл 5)

*История: ц5:MATCH*

![оригинал](132-design-spacing-tokens/original.png)
![наш](132-design-spacing-tokens/ours.png)

### Оригинал

# 132 design-spacing-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:80-94, design-sections.module.css:93-111, theme/variables.css:153-160

## Содержание/структура
`SpacingTokens()` — `.spaceStack` (flex column, gap 8px), 7 строк `.spaceRow`:
`<code class=tokenName>--space-N</code>` + `<span class=tokenValue>NNpx</span>` + `<span class=spaceBar style="width: var(--space-N)">` (полоска-мерка шириной в значение токена).

## Метрики
CSS:
- `.spaceStack`: flex column, gap 8px
- `.spaceRow`: grid `90px 60px 1fr`; align-items center; gap 12px
- `.spaceBar`: height 16px; background `--accent-primary`; border-radius `--radius-xs` (4px); width = `var(--space-N)`
- `.tokenName`: mono 11px `--text-muted`; `.tokenValue`: mono 11px `--text-disabled`

Полная таблица токенов (variables.css):

| Токен | Значение |
|---|---|
| --space-1 | 4px |
| --space-2 | 8px |
| --space-3 | 12px |
| --space-4 | 16px |
| --space-5 | 20px |
| --space-6 | 24px |
| --space-7 | 28px |

Массив values в tsx: ["4px","8px","12px","16px","20px","24px","28px"].

## Состояния/варианты
Статичная витрина, интерактива нет. Цвет полоски `--accent-primary` = #89b4fa (dark).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — `.spaceStack`, `.spaceRow` и `.spaceBar` не задают ни padding, ни margin (design-sections.module.css:94-111); всё расстояние даётся gap 8 (`--space-2`) и gap 12 (`--space-3`), внешний padding 16 приходит от `.sectionBody` (элемент 79)

### Наша реализация

# 132 design-spacing-tokens — наша реализация
Файлы: crates/shell/src/ui/design_panel.rs:122-156, 288-293 (секция Spacing); crates/metrics/src/lib.rs:49-56 (SPACE_1..7)

## Структура/содержание
Секция «Spacing»: горизонтальный flex items-end (gap SPACE_2 = 8px) из 7 колонок. Колонка = квадрат `s×s` px (rounded 2px, bg = accent_primary α 0.5) + подпись-цифра «1»…«7» (FS_XS 11, text_muted). Значения из metrics: SPACE_1..SPACE_6, седьмой — хардкод 28.0 (SPACE_7 в metrics существует, но в массиве design_panel.rs:131 стоит литерал).

## Метрики (из кода, точные)
Токены (metrics): SPACE_1 4, SPACE_2 8, SPACE_3 12, SPACE_4 16, SPACE_5 20, SPACE_6 24, SPACE_7 28 — значения совпадают с оригиналом полностью.
Витрина: квадрат w=h=значение токена; radius 2; fill accent_primary 50%; подпись — только номер шага.

## Отличия от original.md той же папки
- Значения всех 7 токенов идентичны (4/8/12/16/20/24/28).
- Форма витрины другая: горизонтальные квадраты-ступени вместо вертикального списка строк grid `90px 60px 1fr` с полоской-меркой (height 16px, width=токен).
- Полоска оригинала — сплошной `--accent-primary`, radius 4; у нас тон 50% α, radius 2.
- Подпись «N» вместо `--space-N` (mono 11 text-muted) + «NNpx» (mono 11 text-disabled).
- Мелочь кода: седьмое значение — литерал 28.0 вместо m::SPACE_7 (значение то же, но обходит metrics).

### Вердикты

# 132 design-spacing-tokens — verdict (review cycle 1)
VERDICT: DIVERGES
Значения 4/8/12/16/20/24 верны (metrics lib.rs:50-56), НО design_panel.rs:131
хардкодит 28.0 вместо m::SPACE_7 (нарушение №0).
Структура: у нас горизонтальный ряд квадратов w=h=токен; оригинал — 7 вертикальных
.spaceRow (grid 90px 60px 1fr) с баром h16, width=токен.
Бар accent@0.5 r2 vs solid accent + radius-xs 4. Лейблы «1..7» vs --space-N + NNpx (mono).

## Цикл 5: MATCH

Отступы: 7 рядов, бар h16 шириной токена, radius-xs, accent-primary, лейблы `--space-N` + «Npx»; `SPACE_7` задействован.

---

## 133. design-radius-tokens — **MATCH** (цикл 5)

*История: ц5:MATCH*

![оригинал](133-design-radius-tokens/original.png)
![наш](133-design-radius-tokens/ours.png)

### Оригинал

# 133 design-radius-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:96-115, design-sections.module.css:113-132, theme/variables.css:162-170

## Содержание/структура
`RadiusTokens()` — `.radiusGrid` из 4 `.radiusItem`:
`.radiusBox` (квадрат со `style="borderRadius: var(--radius-*)"`) + `<code class=tokenName>--radius-*</code>` + `<span class=tokenValue>Npx</span>`.

## Метрики
CSS:
- `.radiusGrid`: grid `repeat(auto-fill, minmax(120px, 1fr))`, gap 12px
- `.radiusItem`: flex column, align-items center, gap 4px
- `.radiusBox`: 80×80px; background `--bg-surface` (#3d3f51 dark); border `1px solid color-mix(in srgb, var(--accent-primary) 50%, transparent)`; border-radius = токен

Полная таблица токенов (variables.css):

| Токен | Значение | Назначение (комментарий в css) |
|---|---|---|
| --radius-xs | 4px | chips, badges, inline code, micro buttons |
| --radius-sm | 8px | cards inside cards: code blocks, tables, plugin grid items |
| --radius-md | 12px | level-1 cards: chat bubbles, button groups, capsule buttons |
| --radius-lg | 16px | level-0 panels: mainPanel, terminal panel, asst-merge container |
| --radius-xl | 16px | alias --radius-lg (legacy callers), в витрине не показан |

RADIUS_TOKENS в tsx: radius-xs 4px, radius-sm 8px, radius-md 12px, radius-lg 16px. Правило шкалы (комментарий variables.css): outer = inner + padding — концентрическая 4-ступенчатая шкала с якорем 16px.

## Состояния/варианты
Статичная витрина, интерактива нет.

### Наша реализация

# 133 design-radius-tokens — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:652-679 (секция Radius), 427-450 (`token_name` / `token_value`), 900-905 (вызов `section`); crates\metrics\src\lib.rs:35-39 (RADIUS_*)

## Структура/содержание
```
radius div.flex.flex_wrap.gap(12)
└─ колонка × 4: div.w(120).flex_col.items_center.gap(4)
   ├─ бокс div 80×80.rounded(токен).border_1(accent_primary α .5).bg(bg_surface)
   ├─ token_name «--radius-xs» / «--radius-sm» / «--radius-md» / «--radius-lg»
   └─ token_value «4px» / «8px» / «12px» / «16px»
```
Сабтайтл секции: «4-step concentric scale anchored at 16px outer».

## Метрики (из кода, точные)
- отступы: у колонки и бокса собственных padding/margin нет; внешний даёт `section()` — тело p 16 (SPACE_4), секция mb 24 (SPACE_6)
- гэпы: ряд колонок gap 12 (SPACE_3); внутри колонки gap 4 (SPACE_1)
- цвета: бокс bg p.bg_surface #3d3f51, border 1px p.accent_primary #89b4fa α 0.5; `token_name` p.text_muted #838aa0; `token_value` p.text_disabled #60667b
- скругления: демонстрируемые токены — 4 (RADIUS_XS), 8 (RADIUS_SM), 12 (RADIUS_MD), 16 (RADIUS_LG); у колонок скруглений нет
- шрифты: обе подписи — «JetBrains Mono» 11 (FS_XS) weight 400; `token_value` в фикс-колонке 60px
- ховер: N/A: ховер — витрина статична, ни одного `.hover(...)` в секции Radius

## Отличия от original.md той же папки
1. Значения 4 токенов идентичны (4 / 8 / 12 / 16). Алиас `--radius-xl` (= 16, legacy) не портирован — вызовов нет, в оригинальной витрине он тоже не показан.
2. Бокс 80×80, bg `bg-surface`, border `accent-primary 50%`, колонка gap 4 + items-center, подписи `--radius-*` + «Npx» — совпадают с оригиналом.
3. Layout: CSS-grid `repeat(auto-fill, minmax(120px, 1fr))` заменён на flex-wrap с фикс-колонкой 120px (в gpui нет grid) — колонки не растягиваются по остатку строки.
4. `token_value` у нас в фикс-боксе шириной 60px внутри колонки, выровненной по центру: при коротком «4px» текст остаётся в 60px-боксе, из-за чего центрирование подписи чуть отличается от оригинала (там `<span>` по контенту).
5. Правило шкалы «outer = inner + padding» вынесено в сабтайтл секции — в оригинале это комментарий в CSS.

### Вердикты

# 133 design-radius-tokens — verdict (review cycle 1)
VERDICT: DIVERGES
Токены 4/8/12/16 верны. Бокс 48×48 vs 80×80; fill accent@0.12 vs bg-surface;
border accent 100% vs accent 50%; лейбл «xs 4» vs --radius-* + Npx;
flex gap12 vs grid auto-fill minmax(120px,1fr) gap12. (design_panel.rs:161-183)

## Цикл 5: MATCH

Радиусы: бокс 80×80, bg-surface, border accent 50%, ячейка 120, gap 12/4.

---

## 134. design-shadow-tokens — **MATCH** (цикл 5)

*История: ц5:MATCH*

![оригинал](134-design-shadow-tokens/original.png)
![наш](134-design-shadow-tokens/ours.png)

### Оригинал

# 134 design-shadow-tokens — оригинал
Файлы: kamin-ide/src/renderer/components/main/design-sections.tsx:117-134, design-sections.module.css:134-153, theme/dark-theme.css:104-113 (variables.css:74-82 — зеркальный фоллбек)

## Содержание/структура
`ShadowTokens()` — `.shadowGrid` из 9 `.shadowItem`:
`.shadowBox` (прямоугольник со `style="boxShadow: var(--shadow-*)"`) + `<code class=tokenName>--shadow-*</code>`.
Порядок из SHADOW_TOKENS: shadow-mini, shadow-card, shadow-bar, shadow-tab, shadow-dropdown, shadow-card-popup, shadow-toast, shadow-lg, shadow-modal.

## Метрики
CSS:
- `.shadowGrid`: grid `repeat(auto-fill, minmax(140px, 1fr))`, gap 16px
- `.shadowItem`: flex column, align-items center, gap 8px
- `.shadowBox`: 100×64px; background `--bg-primary` (#313240 dark); border-radius `--radius-sm` (8px)

Полная таблица токенов (дефолтная тёмная тема):

| Токен | Значение |
|---|---|
| --shadow-mini | 0 2px 8px rgba(0, 0, 0, 0.3) |
| --shadow-card | 0 0 6px rgba(0, 0, 0, 0.2) |
| --shadow-bar | 0 -4px 12px rgba(0, 0, 0, 0.4) |
| --shadow-tab | 0 6px 18px rgba(0, 0, 0, 0.45) |
| --shadow-dropdown | 0 4px 16px rgba(0, 0, 0, 0.5) |
| --shadow-card-popup | 0 8px 24px rgba(0, 0, 0, 0.5) |
| --shadow-toast | 0 10px 40px rgba(0, 0, 0, 0.4) |
| --shadow-lg | 0 8px 16px rgba(0, 0, 0, 0.3) |
| --shadow-modal | 0 8px 32px rgba(0, 0, 0, 0.5) |

## Состояния/варианты
Статичная витрина, интерактива нет. 9 тонов elevation от mini до modal.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — `.shadowGrid`, `.shadowItem` и `.shadowBox` не задают padding/margin (design-sections.module.css:135-153); расстояния только gap 16 (`--space-4`) в гриде и gap 8 (`--space-2`) в ячейке

### Наша реализация

# 134 design-shadow-tokens — наша реализация
Файлы: crates/shell/src/ui/design_panel.rs:190-222, 300-305 (секция Shadows)

## Структура/содержание
Секция «Shadows»: flex row (gap SPACE_4 = 16px) из 3 боксов. Бокс 96×56px, rounded RADIUS_MD 12, bg `bg_surface`, BoxShadow из кода, подпись именем внутри бокса (FS_XS 11, text_muted). Токенов теней как таковых нет — тройка захардкожена в design_panel; общего словаря shadow-* в metrics/theme не существует.

## Метрики (из кода, точные)
| Имя | offset | blur | spread | цвет |
|---|---|---|---|---|
| mini | 0 4px | 8 | 0 | rgba(0,0,0,0.3) |
| dropdown | 0 4px | 16 | 0 | rgba(0,0,0,0.35) |
| modal | 0 4px | 32 | 0 | rgba(0,0,0,0.5) |

## Отличия от original.md той же папки
- Показаны 3 тона из 9: НЕТ shadow-card, shadow-bar, shadow-tab, shadow-card-popup, shadow-toast, shadow-lg.
- Значения показанных расходятся:
  - mini: у нас 0 4px 8 α.3, оригинал 0 2px 8px α.3 (offset-y 4 vs 2);
  - dropdown: у нас α 0.35, оригинал 0 4px 16px α 0.5 (альфа занижена);
  - modal: у нас offset-y 4, оригинал 0 8px 32px α.5 (offset-y 4 vs 8).
- Нет системы токенов: живые поповеры/меню (context_menu, layout_popover и др.) задают тени по месту — сверка их с --shadow-dropdown отдельная.
- Бокс 96×56 вместо 100×64; radius 12 вместо 8; bg `bg_surface` (#3d3f51) вместо `--bg-primary` (#313240); имя ВНУТРИ бокса вместо подписи под ним; layout row вместо grid minmax(140px,1fr) gap 16.

## Дополнение атрибутов (цикл 10)

- отступы: у боксов теней собственных паддингов нет — размер 96×56 задан жёстко, подпись центрируется; внешние зазоры даёт ряд `gap SPACE_4` 16 (`crates/shell/src/ui/design_panel.rs`, секция Shadows)

### Вердикты

# 134 design-shadow-tokens — verdict (review cycle 1)
VERDICT: DIVERGES (словарь MATCH, витрина DIVERGES)
shadows.rs: все 9 токенов 1:1 с variables.css:74-82. ✓
Витрина design_panel.rs:191-222: 3 бокса вместо 9, инлайновые НЕВЕРНЫЕ значения
(mini y4 vs 2; dropdown α.35 vs .5; modal y4 vs 8); бокс 96×56 r12 bg-surface
vs 100×64 r8 bg-primary.
СИСТЕМНО: shadows.rs всегда чёрный — light-тема требует rgba(27,26,22,.08-.18)
(light-theme.css:139-147). 9 файлов ещё инлайнят BoxShadow мимо словаря.

## Цикл 5: MATCH

Тени: 9 токенов в порядке SHADOW_TOKENS, бокс 100×64 r8, ячейка 140, gap 16. `shadows.rs` сверен построчно с dark/light-theme.css — все 9×2 значения совпадают, включая ink rgba(27,26,22).

---

## 135. sample-buttons — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](135-sample-buttons/original.png)
![наш](135-sample-buttons/ours.png)

### Оригинал

# 135 sample-buttons — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:58-67, design-sections.module.css:195-235

## Содержание/структура
`ButtonsRow()` в Block «Buttons»: 4 кнопки `<button type=button>` — Primary (`.btnPrimary`), Secondary (`.btnSecondary`), Danger (`.btnDanger`), Ghost (`.btnGhost`).

## Метрики
Общее для всех 4 классов:
- padding `var(--space-1) var(--space-4)` = 4px 16px
- border-radius `--radius-sm` (8px)
- font: inherit; font-size `--fs-sm` (12px)
- cursor pointer; transition `background var(--transition-fast)` (150ms ease)

`.btnPrimary`: background `--accent-action` (#89b4fa dark); color `--accent-action-fg` (#313240); border none; font-weight 600.
`.btnSecondary`: background transparent; color `--text-primary`; border `1px solid var(--bg-overlay)` (#515567).
`.btnDanger`: background `--accent-red` (#f38ba8); color `--bg-primary` (#313240); border none; font-weight 600.
`.btnGhost`: background transparent; color `--text-secondary`; border `1px solid transparent`.

## Состояния/варианты
- `.btnPrimary:hover` → background `--accent-action-hover` (#74c7ec)
- `.btnSecondary:hover` → background `--bg-surface` (#3d3f51)
- `.btnDanger:hover` → background `--accent-maroon` (#eba0ac)
- `.btnGhost:hover` → background `--bg-surface`; color `--text-primary`

### Наша реализация

# 135 sample-buttons — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_samples.rs:71-78 (`DsBtn`), 84-152 (`ds_btn`), 155-165 (`sample_buttons`); crates\shell\src\ui\design_panel.rs:804 (`block("Buttons", …)`), 78-123 (обёртка `block`)

## Структура/содержание
```
block «BUTTONS»
└─ div.flex.flex_wrap.gap(8)
   ├─ div#ds-btn-primary   «Primary»    bg accent_action, fg accent_action_fg, weight 600
   ├─ div#ds-btn-secondary «Secondary»  прозрачный, border bg_overlay, fg text_primary
   ├─ div#ds-btn-danger    «Danger»     bg accent_red, fg bg_primary, weight 600
   └─ div#ds-btn-ghost     «Ghost»      прозрачный, border rgba(0,0,0,0), fg text_secondary
```
Все 4 варианта реализованы (`DsBtn::{Primary, Secondary, Danger, Ghost}`), каждый — stateful div с `cursor_pointer()` и `.hover(...)`. У Ghost бордер прозрачный, а не отсутствующий — иначе кнопка была бы на 2px уже соседей.

## Метрики (из кода, точные)
- отступы: кнопка px 16 (SPACE_4) / py 4 (SPACE_1) — у всех 4 вариантов одинаково
- гэпы: ряд кнопок gap 8 (SPACE_2); блок `block()` — колонка gap 8 (SPACE_2), тело-ряд flex-wrap gap 8
- цвета: Primary — bg p.accent_action #89b4fa, текст p.accent_action_fg #313240; Secondary — фон прозрачный, текст p.text_primary #cfd4e2, border 1px p.bg_overlay #515567; Danger — bg p.accent_red #f38ba8, текст p.bg_primary #313240; Ghost — фон прозрачный, текст p.text_secondary #adb3c7, border 1px rgba(0,0,0,0); подпись блока p.text_muted #838aa0
- скругления: все 4 кнопки rounded 8 (RADIUS_SM)
- шрифты: все кнопки font-size 12 (FS_SM); Primary и Danger — weight 600 SEMIBOLD, Secondary и Ghost — weight 400; подпись блока 11 (FS_XS) weight 700 BOLD uppercase
- фоны по ховеру: Primary — p.accent_action_hover #74c7ec; Secondary — p.bg_surface #3d3f51; Danger — p.accent_maroon #eba0ac; Ghost — p.bg_surface #3d3f51 + текст поднимается до p.text_primary #cfd4e2

## Отличия от original.md той же папки
1. Все 4 варианта, их цвета, паддинги 4×16, radius-sm 8, fs-sm 12, weight 600 у Primary/Danger и все 4 hover-состояния — совпадают с оригиналом 1:1.
2. `transition: background var(--transition-fast)` (150ms ease) отсутствует — в gpui переходов нет, смена фона мгновенная.
3. Кнопки — stateful `div`, а не `<button type=button>`: `cursor: pointer` есть, но нет клавиатурной активации, фокус-кольца и роли button.
4. У Ghost бордер задан явно прозрачным (rgba 0,0,0,0) — то же, что `1px solid transparent` в оригинале.
5. `font: inherit` оригинала → у нас семейство наследуется от окна («Bricolage Grotesque»), размер задан явно 12.

### Вердикты

# 135 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Кнопки-семплы: padding 4/16, r8, fs 12 и все 4 палитры фонов/ховеров верны. Дефект: `.btnGhost` у оригинала `border: 1px solid transparent`, у нас границы нет вовсе → Ghost на 2px ниже и уже соседей. Чинить: `.border_1().border_color(transparent_black())`.

## Цикл 6: MATCH

Кнопки: прозрачный фон Rgba{a:0} (design_panel.rs:771); живой кадр — все четыре одной высоты и базовой линии.

---

## 136. sample-list-item — **DIVERGES** (цикл 11)

*История: ц5:DIVERGES, ц6:DIVERGES, ц11:DIVERGES*

![оригинал](136-sample-list-item/original.png)
![наш](136-sample-list-item/ours.png)

### Оригинал

# 136 sample-list-item — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:69-82, design-sections.module.css:252-308

## Содержание/структура
`ActiveItemRow()` в Block «List item — active selection (sidebar pattern)»: `<ul class=itemList>` из 4 `<li><button class=listItem>`:
1. codicon-folder + «Sessions»
2. codicon-settings-gear + «Settings (active)» — `.listItem .listItemActive`
3. codicon-extensions + «Extensions»
4. codicon-debug-disconnect + «Disabled» — `disabled`

Зеркалит паттерн строки sidebar/customize: иконка + label, hover тонируется, active = tinted (dark) / filled-accent (light).

## Метрики
- `.itemList`: list-style none; margin/padding 0; flex column, gap 2px; width 100%; max-width 280px
- `.listItem`: flex, align-items center, gap 8px; width 100%; padding `8px 12px`; border none; border-radius 8px; background transparent; color `--text-secondary`; font inherit, size `--fs-md` (13px); text-align left; cursor pointer; transition `background 150ms ease`
- `.listItem .codicon` (`:global`): font-size 14px

## Состояния/варианты
- hover (`.listItem:hover:not([disabled])`): background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `--text-primary`
- active (`.listItemActive`): background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `--accent-primary`
- active hover (`.listItemActive:hover`): background `color-mix(... accent-primary 22% ...)`; color `--accent-primary` (без этого generic-hover перебил бы active)
- light theme (`[data-theme="light"] .listItemActive`): background `--accent-primary`; color `--accent-action-fg`; font-weight 600; codicon тоже `--accent-action-fg`; hover → background `--accent-action-hover`
- disabled (`.listItem[disabled]`): opacity 0.45; cursor not-allowed

### Наша реализация

# 136 sample-list-item — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:209-254 (`sample_list_item`), 805-809 (`block("List item — active selection (sidebar pattern)", …)`)

## Структура/содержание
```
block «LIST ITEM — ACTIVE SELECTION (SIDEBAR PATTERN)»
└─ list div.flex_col.gap(2).w_full.max_w(280)
   ├─ row «Sessions»          codicon \u{ea83} (folder)
   ├─ row «Settings (active)» codicon \u{eb51} (settings-gear)  ← active
   ├─ row «Extensions»        codicon \u{eae6} (extensions)
   └─ row «Disabled»          codicon \u{ead0}                  ← disabled
row = div.id(label).flex.items_center.gap(8).w_full.px(12).py(8).rounded(8)
```
Три ветки состояний (design_panel.rs:241-250): active → accent-tint + accent-текст + свой hover; disabled → `opacity(0.45)` и НИ ОДНОГО hover; обычная → hover-tint + подъём текста.

## Метрики (из кода, точные)
- отступы: строка px 12 (SPACE_3) / py 8 (SPACE_2); у списка padding/margin нет
- гэпы: список gap 2; строка gap 8 (SPACE_2) между глифом и подписью
- цвета: обычная строка — текст p.text_secondary #adb3c7, фон прозрачный; active — bg p.accent_primary #89b4fa α 0.14 + текст p.accent_primary #89b4fa; disabled — те же цвета обычной строки под `opacity 0.45`
- скругления: строка rounded 8 (RADIUS_SM)
- шрифты: строка font-size 13 (FS_MD), weight 400; глиф — codicon font-size 14; подпись блока 11 (FS_XS) BOLD uppercase
- фоны по ховеру: обычная — p.bg_surface #3d3f51 α 0.5 + текст поднимается до p.text_primary #cfd4e2; active — p.accent_primary α 0.22 (текст остаётся accent_primary); disabled — hover не навешивается вовсе

## Отличия от original.md той же папки
1. Состав (4 строки, те же подписи и codicon-глифы folder / settings-gear / extensions / debug-disconnect), `.itemList` (gap 2, width 100%, max-width 280), `.listItem` (padding 8×12, radius 8, fs-md 13, text-secondary, codicon 14), hover `bg-surface 50%` + text-primary, active `accent-primary 14%` + accent-текст, active-hover 22%, disabled `opacity 0.45` — совпадают с оригиналом 1:1.
2. `cursor: pointer` у строки НЕ задан (в оригинале есть у `.listItem`); `cursor: not-allowed` у disabled тоже нет.
3. `transition: background 150ms ease` отсутствует.
4. Light-вариант `[data-theme="light"] .listItemActive` (сплошной `--accent-primary` фон, текст и глиф `--accent-action-fg`, font-weight 600, hover `--accent-action-hover`) НЕ РЕАЛИЗОВАН: в светлой теме активная строка остаётся accent-tint 14% (цвета берутся из LIGHT-палитры, но формула та же, что в dark).
5. Строки — stateful `div`, а не `<li><button>`: нет роли списка/кнопки, нет атрибута `disabled` (только визуальная opacity), клика тоже нет — образец инертный.
6. Глифы заданы кодпоинтами codicon-шрифта напрямую, без класса `.codicon`; размер 14 совпадает с `:global .listItem .codicon`.

### Вердикты

# 136 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Реализовано 1:1 по геометрии и тонам (gap 8, p 8/12, r-sm, hover surface 50%, active accent 14%/22%, disabled .45, глифы совпали). ОСТАЛОСЬ: светлая ветка `[data-theme=light] .listItemActive` (заливка accent-primary, текст accent-action-fg, вес 600) отсутствует.

## Цикл 11: DIVERGES

Закрыто: светлая тема — активная строка сплошной заливкой accent-primary, текст `--accent-action-fg`, weight 600, ховер её сохраняет (`[data-theme=light] .listItemActive`).

Осталось: кадр светлой темы.

---

## 137. sample-input — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](137-sample-input/original.png)
![наш](137-sample-input/ours.png)

### Оригинал

# 137 sample-input — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:84-97, design-sections.module.css:237-250

## Содержание/структура
`InputRow()` в Block «Input»: контролируемый `<input type=text class=input placeholder="Sample input">` (useState + onInput).

## Метрики
`.input`:
- width 100%; max-width 360px
- padding `var(--space-2) var(--space-3)` = 8px 12px
- border `1px solid var(--bg-surface)` (#3d3f51); border-radius `--radius-sm` (8px)
- background `--bg-base` (#313240); color `--text-primary`
- font inherit; font-size `--fs-md` (13px)
- outline none; transition `border-color var(--transition-fast)` (150ms ease)

## Состояния/варианты
- `:focus` → border-color `--accent-primary` (#89b4fa dark)

### Наша реализация

# 137 sample-input — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:256-271 (`sample_input`), 810 (`block("Input", …)`)

## Структура/содержание
```
block «INPUT»
└─ div.w_full.max_w(360).px(12).py(8).rounded(8)
     .bg(bg_base).border_1(bg_surface).text_size(13).text_color(text_muted)
   └─ «Sample input»
```
Это СТАТИЧНЫЙ div-макет инпута, а не редактируемое поле: ни `InputState`, ни обработчиков ввода нет. Живые текстовые инпуты в приложении есть отдельно (gpui-component `InputState` — quick_open, find_in_files, command_palette, prompt-модалка).

## Метрики (из кода, точные)
- отступы: px 12 (SPACE_3) / py 8 (SPACE_2)
- гэпы: N/A: гэпы — у бокса ровно один текстовый ребёнок, `gap` не задан
- цвета: bg p.bg_base #313240; border 1px p.bg_surface #3d3f51; текст-заглушка «Sample input» p.text_muted #838aa0
- скругления: rounded 8 (RADIUS_SM)
- шрифты: font-size 13 (FS_MD), weight 400, семейство UI «Bricolage Grotesque» (наследуется, своего `font_family` нет)
- ховер: N/A: ховер — статичный div, ни `.hover(...)`, ни фокуса, ни курсора текста

## Отличия от original.md той же папки
1. Геометрия и цвета совпадают 1:1: width 100% + max-width 360, padding 8×12, border 1px `bg-surface`, radius-sm 8, background `bg-base`, font-size fs-md 13.
2. Это не `<input type=text>`, а статичный div: контролируемого значения, каретки, ввода и `useState`/`onInput` нет.
3. Как следствие — состояние `:focus` (border-color `--accent-primary`) и `transition: border-color 150ms` отсутствуют.
4. Строка «Sample input» — в оригинале это `placeholder` (цвет — UA-дефолт), у нас обычный текст цветом `--text-muted`; `color: var(--text-primary)` для введённого текста у нас неприменимо.
5. `outline: none` и `font: inherit` неактуальны (в gpui нет outline; семейство наследуется).

### Вердикты

# 137 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: MATCH

Инпут 360×32.8 лог. = max-width 360 / padding 8 12 / border 1. Минор: у оригинала настоящий <input> с :focus border accent-primary, у нас статичный div.

---

## 138. sample-dropdown — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](138-sample-dropdown/original.png)
![наш](138-sample-dropdown/ours.png)

### Оригинал

# 138 sample-dropdown — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:99-143, design-sections.module.css:310-374

## Содержание/структура
`DropdownRow()` в Block «Dropdown menu», форма зеркалит ThemeQuickToggle:
- `.dropdownAnchor` (relative inline-block) содержит trigger + меню
- trigger: `.btnSecondary .dropdownTrigger` — codicon-color-mode + «Theme» + codicon-chevron-down; клик тогглит open
- при open: `<ul class=dropdownMenu>`:
  - `<li class=dropdownGroupLabel>Built-in</li>`
  - 3 item'а (Dark/hint "default"/icon color-mode, Light/lightbulb, System/device-desktop): `<button class="dropdownItem [dropdownItemPicked]">` — codicon + `<span style=flex:1>label</span>` + опц. `.dropdownItemHint` + codicon-check у выбранного
- клик по item: setPicked + закрытие меню; начальный picked = "dark"

## Метрики
- `.dropdownAnchor`: position relative; display inline-block
- `.dropdownTrigger`: inline-flex; align-items center; gap 8px; codicon внутри font-size `--fs-md` (13px), line-height 1
- `.dropdownMenu`: position absolute; top `calc(100% + 4px)`; left 0; min-width 220px; background `--bg-mantle` (#262533); border-radius `--radius-md` (12px); box-shadow `--shadow-dropdown` (0 4px 16px rgba(0,0,0,0.5)); list-style none; margin 0; padding `--space-1` (4px); z-index `--z-dropdown` (100); flex column, gap 1px
- `.dropdownGroupLabel`: padding `4px 12px`; font-size 11px; uppercase; letter-spacing 0.04em; color `--text-muted`
- `.dropdownItem`: flex; align-items center; gap 8px; width 100%; padding `8px 12px`; background transparent; border none; color `--text-primary`; font inherit, size `--fs-sm` (12px); border-radius 8px; text-align left; cursor pointer
- `.dropdownItemHint`: font `--font-mono`, 11px, color `--text-muted`

## Состояния/варианты
- item hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- picked (`.dropdownItemPicked`): background `color-mix(in srgb, var(--accent-primary) 12%, transparent)`; color `--accent-primary`
- light theme picked: background `--accent-primary`; color `--accent-action-fg`; font-weight 600; codicon и hint тоже `--accent-action-fg`
- open/closed — по state; trigger визуально не меняется

### Наша реализация

# 138 sample-dropdown — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_dropdown`), design_panel.rs (блок «Dropdown menu»), root.rs (событие `DesignSample`, состояние `DesignState`)

## Структура/содержание
Форма ThemeQuickToggle 1:1:
```
div .relative .flex                         ← .dropdownAnchor
├─ ds_btn(Secondary) .flex .items_center .gap 8   ← .btnSecondary .dropdownTrigger
│   ├─ codicon color-mode 13 · "Theme" · codicon chevron-down 13
└─ (open) deferred(priority 60)             ← z-index: var(--z-dropdown)
    div .absolute .top(100%) .mt 4 .left 0 .min_w 220 .flex_col .gap 1 .p 4
        .rounded 12 .bg bg-mantle .shadow(dropdown)
    ├─ "BUILT-IN"                           ← .dropdownGroupLabel
    └─ 3 × item (dark / light / system)
```
Клик по триггеру — `DesignAction::ToggleDropdown`; по пункту — `Pick(id)` (выбор + закрытие). Стартовый picked = "dark".

## Метрики (из кода, точные)
- Меню: min-w 220, bg `--bg-mantle` #262533, radius RADIUS_MD 12, `shadows::dropdown()` = 0 4 16 rgba(0,0,0,.5), padding SPACE_1 4, gap 1.
- `.dropdownGroupLabel`: px 12 / py 4, fs FS_XS 11, uppercase (Rust `to_uppercase`), text-muted.
- `.dropdownItem`: gap 8, w-full, px 12 / py 8, radius RADIUS_SM 8, fs FS_SM 12, text-primary; глиф 13.
- `.dropdownItemHint`: JetBrains Mono, fs 11, text-muted.
- picked: bg accent-primary 12% + текст accent-primary + codicon-check; hover невыбранного: bg-surface 60%.

## Отличия от original.md той же папки
1. `letter-spacing .04em` у group-label в gpui недоступен (общий deviation порта).
2. Light-вариант picked (сплошная заливка accent + `--accent-action-fg` + weight 600) отдельной веткой не сделан — цвета берутся из активной палитры.

### Вердикты

# 138 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока Dropdown нет вовсе: в design_panel.rs 10 блоков, среди них его нет (component-samples.tsx:99-143 + .dropdownMenu/.dropdownItem*).

## Цикл 7: DIVERGES

Блок реализован (`design_samples.rs::sample_dropdown`), метрики меню/label/item/hint
сверены и совпали. Ревью нашло три дефекта — все исправлены: глифы пунктов и галка 16
(в `.dropdownItem` кегль кодикона не переопределён → база `.codicon{16px}` из
skeleton.css, не 13), лишний пустой flex-ребёнок в триггере убран (`ds_btn` не
добавляет пустую подпись), ховер работает и на выбранном пункте
(`.dropdownItem:hover` 0,2,0 бьёт `.dropdownItemPicked` 0,1,0).

Осталось: `letter-spacing .04em` (нет в gpui); нет пары кадров — вердикт по коду.

---

## 139. sample-tree — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](139-sample-tree/original.png)
![наш](139-sample-tree/ours.png)

### Оригинал

# 139 sample-tree — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:15-38,145-167, design-sections.module.css:376-383; сам Tree — components/tree/Tree.tsx (описан в зоне FileTree)

## Содержание/структура
`TreeRow()` в Block «Tree (file-explorer pattern)»: живой рекурсивный компонент `<Tree>` в рамке `.treeFrame`.
Данные SAMPLE_TREE: `src/` (dir) → `host/` (index.ts 13 KB, layout-store.ts 2.5 KB, json-file-store.ts 1.8 KB), `exthost/` (api.ts 3.0 KB, loader.ts 8.2 KB); корневые файлы package.json (1.2 KB, icon "json"), README.md (4.1 KB, icon "markdown"). У файлов `meta` = размер.
State: expanded = Set{"src","src/host"}, selected = "src/host/index.ts"; onToggle — переключение папки, onSelect — выбор ноды.

## Метрики
`.treeFrame`:
- width 100%; max-width 380px
- padding `--space-2` (8px)
- border `1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)`
- border-radius `--radius-sm` (8px)
- background `--bg-base` (#313240)

## Состояния/варианты
Интерактивный образец: expand/collapse папок, выделение ноды. Стили строк — из самого Tree (не этого модуля).

### Наша реализация

# 139 sample-tree — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_tree`, блок «Tree (file-explorer pattern)»)

## Структура/содержание
Четыре строки-рецепта файлового дерева: `src` (папка, раскрыта), `components` (папка, свёрнута), `App.tsx` (выделен), `main.tsx`. Колонка без собственного скролла, `w-full`, `max-w 280`.
```
row (flex, items-center, gap 6, h 22, border 1px transparent, radius-xs)
├─ бокс шеврона 16 (глиф 13, text-muted; у листа — пустой спейсер)
├─ иконка 16 (папка/файл codicon)
└─ имя (fs-sm)
```

## Метрики (из кода, точные)
- Строка: h 22, gap 6, `padding-left = depth*12 + 8`, pr SPACE_2 8, radius RADIUS_XS 4, рамка 1px transparent (резерв под выделение), fs FS_SM 12, цвет text-secondary #adb3c7.
- Шеврон: бокс 16, глиф 13, text-muted #838aa0.
- Иконка: codicon 16.
- Выделенная строка: линейный градиент 90° accent-primary 26% → 14%, рамка accent-primary 45%, текст text-primary #cfd4e2.
- Ховер невыделенной: bg-surface #3d3f51 при alpha 0.55 + text-primary.

## Отличия от original.md той же папки
1. Семпл статичный: раскрытие и выбор не переключаются кликом (в оригинале — `useState` на expanded/selected).
2. Иконки — codicon-папка/файл, а не иконочная тема Catppuccin, как в живом дереве оригинала.
3. Hint у блока убран — в оригинале у `TreeRow` его нет.

### Вердикты

# 139 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Дерево по чужому рецепту: нет .treeFrame (max-w 380, p 8, border 1px bg-surface 60%, r-sm, bg-base); у нас max-w 280, h22 вместо p 4/8, gap 6 против 8, text-secondary против text-primary, chevron 16/13 против 14/10, иконки text-muted 16 против accent-yellow/text-muted fs-sm, отступ 12 против 14, колонки meta нет, контент другой.

---

## 140. sample-chips-kbd-code-badge — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](140-sample-chips-kbd-code-badge/original.png)
![наш](140-sample-chips-kbd-code-badge/ours.png)

### Оригинал

# 140 sample-chips-kbd-code-badge — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:169-180, design-sections.module.css:385-434

## Содержание/структура
`ChipsRow()` в Block «Chips · Kbd · Code · Badge»:
- `<span class=chip>active</span>` (зелёный)
- `<span class="chip chipMuted">idle</span>`
- `<span class="chip chipDanger">error</span>`
- `<kbd class=kbd>Ctrl+Shift+P</kbd>`
- `<code class=codeInline>npm run check</code>`
- `<span class=badge>3</span>`

## Метрики
`.chip` (база, зелёный):
- inline-flex; align-items center; gap 4px; padding `1px var(--space-2)` = 1px 8px
- border-radius `--radius-xs` (4px); font-size `--fs-xs` (11px)
- background `color-mix(in srgb, var(--accent-green) 14%, transparent)`; color `--accent-green`; border `1px solid color-mix(... accent-green 30% ...)`

`.chipMuted`: background `color-mix(text-muted 12%)`; color `--text-muted`; border-color `color-mix(text-muted 25%)`.
`.chipDanger`: background `color-mix(accent-red 14%)`; color `--accent-red`; border-color `color-mix(accent-red 30%)`.

`.kbd`: font `--font-mono` 11px; color `--text-secondary`; background `color-mix(in srgb, var(--bg-overlay) 50%, transparent)`; padding 2px 6px; border-radius 4px; border `1px solid color-mix(in srgb, var(--bg-surface) 70%, transparent)`.

`.codeInline`: font `--font-mono` 11px; color `--accent-primary`; background `color-mix(in srgb, var(--accent-primary) 10%, transparent)`; padding 1px 6px; border-radius 4px.

`.badge`: inline-grid; place-items center; min-width 18px; height 18px; padding 0 6px; border-radius 9px; font-size 11px; font-weight 600; background `--accent-red` (#f38ba8); color `--bg-primary` (#313240).

## Состояния/варианты
Статичные; hover-состояний нет. Три варианта chip: default (green/active), muted (idle), danger (red/error).

### Наша реализация

# 140 sample-chips-kbd-code-badge — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (блок «Chips · Kbd · Code · Badge»)

## Структура/содержание
Ряд flex-wrap: три чипа (active/idle/error), kbd, inline-code, badge.

## Метрики (из кода, точные)
- Чип: px SPACE_2 8, py 1, radius RADIUS_XS 4, fs FS_XS 11; фон — цвет чипа при alpha 0.14, рамка 1px того же цвета при alpha 0.30, текст — сам цвет. active = accent-green #a6e3a1, error = accent-red #f38ba8.
- `idle` (muted-вариант): фон text-muted #838aa0 при alpha 0.12, рамка при 0.25.
- kbd: JetBrains Mono, fs 11, text-secondary #adb3c7, фон bg-overlay #515567 при alpha 0.5, px 6, py 2, radius 4, рамка bg-surface #3d3f51 при alpha 0.7.
- code: JetBrains Mono, fs 11, accent-primary #89b4fa, фон accent-primary при alpha 0.10, px 6, py 1, radius 4.
- badge: min-w 18, h 18, px 6, radius 9, fs 11, weight 600, фон accent-red #f38ba8, текст bg-primary #313240.
- Ряд: gap SPACE_2 8.

## Отличия от original.md той же папки
Ховера у элементов ряда нет — в оригинале его тоже нет.

### Вердикты

# 140 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Чипы/kbd/code/badge: геометрия и тона (chip 14%/30%, kbd, codeInline, badge min-w18 h18 r9 w600) верны. Расхождения: `.chipMuted` у оригинала bg 12% / border 25%, у нас общий `chip()` даёт 14%/30%; тексты не те («chip/muted/danger» вместо «active/idle/error», «Ctrl+K» вместо «Ctrl+Shift+P», «code()» вместо «npm run check»).

## Цикл 6: MATCH

Chips/kbd/code/badge: chipMuted 12%/25%, тексты active/idle/error, «Ctrl+Shift+P», «npm run check», badge 3 — живой кадр подтверждает.

---

## 141. sample-toast-triggers — **MATCH** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:MATCH*

![оригинал](141-sample-toast-triggers/original.png)
![наш](141-sample-toast-triggers/ours.png)

### Оригинал

# 141 sample-toast-triggers — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:182-192, design-sections.module.css:215-220 (.btnSecondary); pushToast — renderer/signals/state.ts

## Содержание/структура
`ToastTriggers()` в Block «In-app toasts»: 5 кнопок `.btnSecondary`, каждая вызывает `pushToast({...})`:
1. «Push info» → { severity: "info", message: "Sample info toast.", timestamp: Date.now() }
2. «Push success» → { severity: "success", message: "Sample success toast." }
3. «Push warning» → { severity: "warning", message: "Sample warning." }
4. «Push error» → { severity: "error", message: "Sample error." }
5. «With actions» → { severity: "info", message: "Pick an action.", actions: ["Save", "Discard"], sticky: true }

## Метрики
Кнопки — `.btnSecondary`: padding 4px 16px; border-radius 8px; font-size 12px; background transparent; color `--text-primary`; border `1px solid var(--bg-overlay)`; hover background `--bg-surface`; transition 150ms ease.
Сам тост — отдельный компонент (зона Overlays), здесь только триггеры.

## Состояния/варианты
4 severity (info/success/warning/error) + вариант с actions и sticky: true (не автоскрывается).

### Наша реализация

# 141 sample-toast-triggers — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_toast_triggers`), design_panel.rs (блок «In-app toasts»)

## Структура/содержание
5 кнопок `ds_btn(Secondary)` в `.compInline`-ряду (flex-wrap, gap 8): Push info / Push success / Push warning / Push error / With actions. Каждая шлёт `ShellEvent::Toast` в стек (`ui/toasts.rs`) с текстами оригинала; «With actions» — actions ["Save","Discard"] + sticky.

## Метрики (из кода, точные)
`ds_btn(Secondary)`: px SPACE_4 16 / py SPACE_1 4, radius RADIUS_SM 8, fs FS_SM 12, фон прозрачный, рамка 1px `--bg-overlay`, hover bg `--bg-surface`.

## Отличия от original.md той же папки
Переходов (`transition 150ms ease`) в gpui нет — общий deviation порта.

## Дополнение атрибутов (цикл 10)

- шрифты: кнопка font-size 12 (FS_SM) (design_samples.rs:129), font-weight 400 (Secondary — ветка `bold = false`, design_samples.rs:100,148-150); семейство UI «Bricolage Grotesque»

### Вердикты

# 141 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока триггеров тостов нет.

## Цикл 7: MATCH

Пять кнопок, тексты, severity, `actions ["Save","Discard"] + sticky` — дословно
(`design_samples.rs` vs `component-samples.tsx:185-189`); `.btnSecondary` 4/16, r8,
fs12, рамка bg-overlay, hover bg-surface. Отличие только в отсутствии CSS-перехода
150ms (нет в gpui).

---

## 142. sample-modal-triggers — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](142-sample-modal-triggers/original.png)
![наш](142-sample-modal-triggers/ours.png)

### Оригинал

# 142 sample-modal-triggers — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:194-202, design-sections.module.css:215-228 (.btnSecondary/.btnDanger); showConfirm/showPrompt — renderer/signals/overlays.ts

## Содержание/структура
`ModalTriggers()` в Block «Modals»: 3 кнопки:
1. «Confirm» (`.btnSecondary`) → `showConfirm({ title: "Sample confirm", bodyHtml: "This is a <code>ConfirmModal</code> demo." })`
2. «Confirm danger» (`.btnDanger`) → `showConfirm({ title: "Delete?", bodyHtml: "This action <strong>cannot be undone</strong>.", isDanger: true, confirmLabel: "Delete" })`
3. «Prompt» (`.btnSecondary`) → `showPrompt({ title: "Enter name", placeholder: "e.g. my-extension" })`

## Метрики
`.btnSecondary`: padding 4px 16px; radius 8px; fs 12px; transparent bg; border `1px solid var(--bg-overlay)`; hover `--bg-surface`.
`.btnDanger`: то же + background `--accent-red`; color `--bg-primary`; border none; font-weight 600; hover `--accent-maroon`.
Сами модалки — компоненты зоны Overlays; здесь только триггеры.

## Состояния/варианты
Confirm обычный / danger (isDanger + кастомный confirmLabel) / Prompt (текстовый ввод с placeholder). bodyHtml поддерживает HTML-разметку.

### Наша реализация

# 142 sample-modal-triggers — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_modal_triggers`), ui/modal.rs (`ModalAction::Noop`)

## Структура/содержание
3 кнопки: Confirm (`Secondary`), Confirm danger (`Danger`, danger + confirmLabel «Delete»), Prompt (`Secondary`, prompt-режим с инпутом). Каждая шлёт `ShellEvent::OpenModal(Modal{..})`, действие — `ModalAction::Noop`.

## Метрики (из кода, точные)
См. `ds_btn`: 4/16, radius 8, fs 12. Danger = bg accent-red, цвет bg-primary, weight 600, hover accent-maroon.

## Отличия от original.md той же папки
`bodyHtml` оригинала содержит разметку (`<code>`, `<strong>`); наша модалка принимает простой текст — теги сняты, содержание то же.

## Дополнение атрибутов (цикл 10)

- отступы: кнопки px 16 (SPACE_4) / py 4 (SPACE_1) (design_samples.rs:126-127); у ряда-обёртки padding/margin нет, только flex-wrap gap 8 (design_samples.rs:324-327)
- цвета: Secondary — фон прозрачный, text p.text_primary #cfd4e2, border 1px p.bg_overlay #515567, hover bg p.bg_surface #3d3f51 (design_samples.rs:94-101); Danger — bg p.accent_red #f38ba8, text p.bg_primary #313240, hover bg p.accent_maroon #eba0ac (design_samples.rs:102-109)

### Вердикты

# 142 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока триггеров модалок нет.

## Цикл 7: DIVERGES

Три кнопки и их модалки совпали, дефолты `confirmLabel` («Confirm»/«OK») тоже.
Найденный ревью пробел закрыт: у `Modal` появилось поле `placeholder`, семпл Prompt
передаёт «e.g. my-extension», overlay сажает его в `InputState`.

Осталось: `bodyHtml` оригинала несёт разметку (`<code>`, `<strong>`) — наша модалка
принимает простой текст; нет пары кадров.

---

## 143. sample-external-toast-triggers — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](143-sample-external-toast-triggers/original.png)
![наш](143-sample-external-toast-triggers/ours.png)

### Оригинал

# 143 sample-external-toast-triggers — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:204-237, design-sections.module.css:215-220 (.btnSecondary)

## Содержание/структура
`ExternalToastTriggers()` в Block «External toasts (out-of-app)», hint: «Standalone BrowserWindows — auto-fire when KaminIDE is unfocused. Bottom timer bar shrinks over 8 s; hover pauses both bar and dismiss timer. Buttons below force one regardless of focus.»
4 кнопки `.btnSecondary`, каждая вызывает `window.kamin?.externalToast.show({...})`:
1. «Info (timed)» → { kind: "info", title: "Build finished", message: "Sample with timer bar — hover to pause." }
2. «Success (timed)» → { kind: "success", title: "Sync complete", message: "All extensions synced — green accent + check glyph." }
3. «Warning (sticky)» → { kind: "warning", title: "Approval pending", message: "Sticky — no auto-dismiss, no timer bar.", sticky: true }
4. «Error (with actions)» → { kind: "error", title: "Activation failed", message: "Pick what to do — Retry runs activate() again, Show log opens the Output channel.", sticky: true, actions: ["Retry", "Show log"] }

## Метрики
Кнопки — `.btnSecondary` (см. 135): padding 4px 16px; radius 8px; fs 12px; border `1px solid var(--bg-overlay)`; hover `--bg-surface`.
Внешний тост — отдельное BrowserWindow (вне renderer-дерева); таймер-бар 8 s, hover ставит на паузу бар и dismiss-таймер (из hint).

## Состояния/варианты
kind: info / success / warning / error; timed (таймер-бар) vs sticky (без автозакрытия и бара); опциональные actions-кнопки.

### Наша реализация

# 143 sample-external-toast-triggers — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_external_toast_triggers`)

## Структура/содержание
Блок «External toasts (out-of-app)» с тем же hint'ом и 4 кнопками `Secondary`: Info (timed) / Success (timed) / Warning (sticky) / Error (with actions ["Retry","Show log"]) — title/message как в оригинале.

## Метрики (из кода, точные)
`ds_btn(Secondary)`: 4/16, radius 8, fs 12, рамка bg-overlay, hover bg-surface. Hint — `.compHint`: fs 11, lh 1.3, text-muted, отбивка снизу 4.

## Отличия от original.md той же папки
Внешних (standalone-окно) тостов в порте НЕТ: кнопки поднимают тот же тост во внутреннем стеке. Не портированы отдельное always-on-top окно, 8-секундный таймер-бар и пауза по ховеру. Форма блока (кнопки, подписи, hint) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: кнопки px 16 (SPACE_4) / py 4 (SPACE_1) (design_samples.rs:126-127); hint — margin-bottom 4 (SPACE_1) (design_panel.rs:105); у ряда-обёртки padding нет, gap 8 (design_samples.rs:415)
- цвета: все 4 кнопки Secondary — фон прозрачный, text p.text_primary #cfd4e2, border 1px p.bg_overlay #515567, hover bg p.bg_surface #3d3f51 (design_samples.rs:94-101); hint p.text_muted #838aa0 (design_panel.rs:108)

### Вердикты

# 143 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока внешних тостов нет (у оригинала ещё hint на 4 строки).

## Цикл 7: DIVERGES

Форма блока 1:1 (4 кнопки, kind/title/message/sticky/actions дословно, hint
посимвольно). Расхождение по существу: внешних (standalone-окно) тостов в порте нет —
кнопки поднимают внутренний тост; таймер-бара 8 с и паузы по ховеру нет.

---

## 144. sample-tooltip — **MATCH** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:MATCH*

![оригинал](144-sample-tooltip/original.png)
![наш](144-sample-tooltip/ours.png)

### Оригинал

# 144 sample-tooltip — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:239-245, design-sections.module.css:230-235 (.btnGhost)

## Содержание/структура
`TooltipDemo()` в Block «Tooltip»: одна кнопка `.btnGhost` «Hover me» с атрибутом `data-tooltip="This is a tooltip — hover for the full text. data-tooltip is set on the element, document-level listener does the rest."`

Механика: тултип объявляется атрибутом `data-tooltip` на элементе; document-level listener рисует сам тултип (компонент тултипа — зона Overlays).

## Метрики
`.btnGhost`: padding 4px 16px; border-radius 8px; font-size 12px; background transparent; color `--text-secondary`; border `1px solid transparent`; transition `background 150ms ease`.

## Состояния/варианты
- `.btnGhost:hover`: background `--bg-surface`; color `--text-primary`
- hover также вызывает показ тултипа через глобальный listener

### Наша реализация

# 144 sample-tooltip — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_tooltip`), ui/tooltip.rs

## Структура/содержание
Одна кнопка `ds_btn(Ghost)` «Hover me» с `.tooltip(...)` и тем же текстом, что в `data-tooltip` оригинала. Рисует общий механизм порта (gpui-ховер + overlay-копия) — аналог document-level listener'а.

## Метрики (из кода, точные)
`.btnGhost`: px 16 / py 4, radius 8, fs 12, фон прозрачный, цвет text-secondary, рамка 1px transparent (резерв ширины), hover bg-surface + text-primary.

## Отличия от original.md той же папки
Нет CSS-перехода фона 150ms (deviation порта).

## Дополнение атрибутов (цикл 10)

- цвета: кнопка Ghost — фон прозрачный, text p.text_secondary #adb3c7, border 1px rgba(0,0,0,0) (design_samples.rs:110-122); hover — bg p.bg_surface #3d3f51 + text p.text_primary #cfd4e2 (design_samples.rs:119-120); сам бокс тултипа — bg p.bg_surface #3d3f51, text p.text_primary #cfd4e2, shadow rgba(0,0,0,.3) (tooltip.rs:29-31,67-68)
- шрифты: кнопка font-size 12 (FS_SM), font-weight 400 (design_samples.rs:129); бокс тултипа font-size 11 (FS_XS), line-height 14.3 (tooltip.rs:72-73)

### Вердикты

# 144 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока тултипа нет.

## Цикл 7: MATCH

`.btnGhost` 4/16, r8, fs12, рамка 1px transparent, hover bg-surface + text-primary;
текст тултипа дословный; поверхность тултипа порта совпадает с `Tooltip.module.css`.
Из вьюпорт-клампа `min(640px, 100vw-16px)` реализовано только 640 — это зона элемента
129, не 144.

---

## 145. sample-block-wrapper — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](145-sample-block-wrapper/original.png)
![наш](145-sample-block-wrapper/ours.png)

### Оригинал

# 145 sample-block-wrapper — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:40-56,247-255 (дубликат Block в component-samples-extra.tsx:223-231), design-sections.module.css:155-193

## Содержание/структура
- `ComponentSamples()` — корневая обёртка `.compStack`, перечисляет все sample-блоки (ButtonsRow → … → ExtraSamples)
- `Block({ label, hint?, children })` — обёртка одного примера:
  - `.compRow` (контейнер)
  - `<h3 class=compLabel>{label}</h3>`
  - опц. `<p class=compHint>{hint}</p>`
  - `.compInline` — строка с самими образцами

Block определён дважды (идентично) — в component-samples.tsx и component-samples-extra.tsx.

## Метрики
- `.compStack`: flex column; gap `--space-4` (16px)
- `.compRow`: flex column; gap `--space-2` (8px)
- `.compLabel`: margin 0; font-size `--fs-xs` (11px); text-transform uppercase; letter-spacing 0.06em; color `--text-muted`
- `.compHint`: margin `0 0 var(--space-1)` (0 0 4px); font-size 11px; color `--text-muted`; line-height `--lh-snug` (1.3)
- `.compHint code`: font `--font-mono`; 11px; color `--text-secondary`
- `.compInline`: flex; flex-wrap wrap; gap 8px

## Состояния/варианты
hint опционален. Интерактива нет.

### Наша реализация

# 145 sample-block-wrapper — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn block`, `fn block_hint`)

## Структура/содержание
Обёртка каждого семпла — 1:1 с `.compRow` / `.compLabel` / `.compHint` / `.compInline` оригинала:
```
div (flex-col, gap SPACE_2 8)              ← .compRow
├─ подпись (uppercase)                     ← .compLabel
├─ [hint]                                  ← .compHint
└─ div (flex, flex-wrap, gap SPACE_2 8)    ← .compInline
    └─ тело семпла
```

## Метрики (из кода, точные)
- `.compRow`: flex-col, gap SPACE_2 8.
- `.compLabel`: fs FS_XS 11, weight 700 (UA-дефолт `<h3>`), цвет text-muted #838aa0, текст в верхнем регистре (`to_uppercase` в Rust).
- `.compHint`: mb SPACE_1 4, fs FS_XS 11, line-height 1.3, text-muted.
- `.compInline`: flex, flex-wrap, gap SPACE_2 8 — без него одиночный ребёнок (меню, дерево) растягивался на всю ширину панели.
- `.compStack` (между блоками): flex-col, gap SPACE_4 16.

## Отличия от original.md той же папки
`letter-spacing: 0.06em` у подписи в gpui недоступен.

## Дополнение атрибутов (цикл 10)

- отступы: собственных паддингов у обёртки нет — вертикальные интервалы задают `gap SPACE_2` 8 внутри блока и `gap SPACE_4` 16 между блоками; единственный отступ — `mb SPACE_1` 4 у hint (`crates/shell/src/ui/design_panel.rs`, `fn block_hint`)

### Вердикты

# 145 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

`Block`-враппер отсутствует целиком: у оригинала `.compRow` (col gap 8) + `.compLabel` (uppercase fs-xs muted) + `.compHint` (fs-xs lh-snug) + `.compInline` (wrap gap 8), а стек секции `--space-4` 16. У нас просто колонка gap 12 и ни одной подписи блока.

## Цикл 6: MATCH

Обёртка блока 1:1: колонка gap 8, подпись fs-xs uppercase muted, hint fs-xs lh 1.3 + mb space-1, стек gap 16. Остатки: порядок блоков не оригинальный (Tree 5-й/Chips 6-й у оригинала) и <h3> вес 700.

---

## 146. sample-horizontal-tab-strip — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](146-sample-horizontal-tab-strip/original.png)
![наш](146-sample-horizontal-tab-strip/ours.png)

### Оригинал

# 146 sample-horizontal-tab-strip — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:44-71, components/activity-bar/BottomTabBar.module.css, BottomTabBar.tsx:24 (TAB_ICON_SIZE_PX = 13)

## Содержание/структура
`TabsRow()` в Block «Horizontal tab strip», hint: «BottomTabBar / FileViewerTabs recipe — pill tabs, accent-tinted active state.»
Превью использует реальные классы BottomTabBar.module.css:
- `.strip` (inline style: width 100%, maxWidth 360) → `.tabs` → 3 кнопки `.tab` (+ `.tabActive` у активной): Terminal (icon terminal), Problems (warning), Output (output)
- каждая кнопка: `<ToolIcon size={13}>` + `<span class=tabLabel>` , `aria-pressed`, клик = setActive
- начальный active = "terminal"

## Метрики
- `.strip`: flex; align-items center; gap `--space-1` (4px); flex-shrink 0; padding `4px var(--space-2)` = 4px 8px; border-radius `--radius-sm` (8px)
- `.tabs`: flex; align-items center; gap 4px; flex 1; min-width 0; overflow-x auto; scrollbar-width none
- `.tab`: inline-flex; align-items center; gap 6px; padding 4px 10px; height 24px; background transparent; border none; border-radius 8px; color `--text-secondary`; font-size 11px; font-weight 500; letter-spacing 0.02em; white-space nowrap; cursor pointer; transition background+color 150ms ease
- `.tab .codicon`: font-size 13px; line-height 1; `.tabImage` (VSIX SVG/PNG): 13×13px, object-fit contain
- `.tabLabel`: overflow hidden; text-overflow ellipsis; min-width 0
- Иконка ToolIcon: TAB_ICON_SIZE_PX = 13

Остальные классы модуля (в превью не используются): `.tabDragging` opacity 0.3; `.dropPlaceholder` 36×24px, dashed `color-mix(accent-primary 70%)`, bg `color-mix(accent-primary 14%)`, radius 8px; `.pickerSlot` flex-shrink 0, margin-left auto.

## Состояния/варианты
- hover (`.tab:hover`): background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `--text-primary`
- active (`.tabActive`, и `.tabActive:hover`): background `color-mix(in srgb, var(--accent-primary) 16%, transparent)`; color `--text-primary` (без ring)

### Наша реализация

# 146 sample-horizontal-tab-strip — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_tab_strip`, блок «Horizontal tab strip»)

## Структура/содержание
Стрип-рецепт `BottomTabBar`: три пилюли — Terminal (активна), Problems, Output; иконка + подпись.

## Метрики (из кода, точные)
- Стрип: flex, items-center, gap SPACE_1 4, w-full, max-w 360, px SPACE_2 8, py 4, radius RADIUS_SM 8.
- Таб: h 24, px 10, gap 6, radius RADIUS_SM 8, fs 11, weight 500, цвет text-secondary #adb3c7; глиф codicon 13.
- Активный таб: фон accent-primary #89b4fa при alpha 0.16, текст text-primary #cfd4e2.

## Отличия от original.md той же папки
1. Семпл статичный: активная вкладка не переключается кликом.
2. Иконки — codicon вместо Phosphor-ассетов `ToolIcon` живого стрипа.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер

### Вердикты

# 146 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Геометрия верна (strip gap 4 p 4/8; таб h24 px10 gap6 fs11/500, глиф 13, active accent 16%). Нет `.tab:hover{bg-surface 50%; text-primary}` — в sample_tab_strip ни одного .hover() и нет .id().

---

## 147. sample-vertical-icon-column — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](147-sample-vertical-icon-column/original.png)
![наш](147-sample-vertical-icon-column/ours.png)

### Оригинал

# 147 sample-vertical-icon-column — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:73-107, components/activity-bar/ActivityBar.module.css

## Содержание/структура
`VerticalIconColumnRow()` в Block «Vertical icon column», hint: «ActivityBar recipe — square icon tiles + picker dot at the end.»
Превью на реальных классах ActivityBar.module.css:
- `<nav class=bar aria-label="Sample activity bar">` → `<ul class=list>` из 3 `<li><button class="btn [btnActive]">` с `<ToolIcon>`: Projects (folders), Folder tree (tree-view), Search (search); aria-pressed, aria-label, data-tooltip
- ниже `.pickerAnchor` → `<button class=picker aria-label="More" data-tooltip="Add or remove items">` c `codicon-more`
- начальный active = "projects"

## Метрики
- `.bar`: flex column; align-items center; gap `--space-2` (8px); padding `var(--space-3) 0` (12px 0); width `var(--layout-activity-bar-width, 44px)`; flex-shrink 0; фон прозрачный (гейт-градиент app-фона просвечивает)
- `.list`: list-style none; margin/padding 0; flex column; gap 2px; width 100%; align-items center
- `.btn`, `.picker`: 32×32px; display grid; place-items center; background transparent; border none; border-radius `--radius-sm` (8px); color `--text-muted`; font inherit; cursor pointer; transition background+color 150ms ease
- `.btn .codicon`, `.picker .codicon`: font-size 18px; line-height 1; img-варианты (`.btnImage`, `.menuItemImage`, `.btn img`, `.picker img`): 18×18px, object-fit contain
- `.pickerAnchor`: position relative; flex; justify-content center; width 100%

Не используемые в превью классы модуля: `.tileDragging > .btn` opacity 0.3; `.dropPlaceholder` 32×32, dashed accent; `.barReverse` justify-content flex-end; `.pickerAnchorInline`; `.menu` (min-width 220px, bg `--bg-surface`, border `1px solid var(--divider-soft)`, radius 12px, shadow `--shadow-dropdown`, padding 4px, gap 1px, z `--z-dropdown`); `.menuPortal` (fixed, max-height calc(100vh - 16px), max-width calc(100vw - 16px), overflow-y auto); `.menuLabel`; `.menuItem` (+hover `color-mix(text-primary 10%)`); `.menuLabelText` flex 1.

## Состояния/варианты
- hover (`.btn:hover`, `.picker:hover`): background `color-mix(in srgb, var(--bg-surface) 50%, transparent)`; color `--text-primary`
- active (`.btnActive`, `.btnActive:hover`): background `color-mix(in srgb, var(--accent-primary) 16%, transparent)`; color `--text-primary` (иконка остаётся PRIMARY, не accent; без ring)

### Наша реализация

# 147 sample-vertical-icon-column — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_icon_column`, блок «Vertical icon column»)

## Структура/содержание
Рецепт `ActivityBar`: колонка из трёх квадратных плиток (первая активна) и «…»-пикер под ними.

## Метрики (из кода, точные)
- Бар: ширина `ACTIVITY_BAR_WIDTH` 48, flex-col, items-center, gap SPACE_2 8, py SPACE_3 12.
- Список плиток: flex-col, items-center, gap 2.
- Плитка: 32×32, radius RADIUS_SM 8, глиф codicon 18, цвет text-muted #838aa0.
- Активная плитка: фон accent-primary при alpha 0.16, текст text-primary #cfd4e2.
- Пикер «…»: та же плитка 32×32, глиф 18, text-muted.

## Отличия от original.md той же папки
1. Семпл статичный: активная плитка не переключается кликом.
2. Тултипов у плиток нет (в оригинале `data-tooltip` на каждой).

## Дополнение атрибутов (цикл 10)

- шрифты: текстовых узлов нет — только глифы codicon 18 в плитках и в «…»-пикере (`crates/shell/src/ui/design_panel.rs`, `fn sample_icon_column`); кегль наследуется от панели FS_MD 13, но на отрисовку не влияет
- ховер: N/A: ховер

### Вердикты

# 147 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Бар 48 + py12, list gap 2, плитка 32 r-sm, глиф 18, active accent 16% — совпало. Нет `.btn:hover/.picker:hover`. Иконки: оригинал folders/tree-view/search, у нас folder/file/search.

---

## 148. sample-checkbox-dropdown — **MATCH** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:MATCH*

![оригинал](148-sample-checkbox-dropdown/original.png)
![наш](148-sample-checkbox-dropdown/ours.png)

### Оригинал

# 148 sample-checkbox-dropdown — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:109-141, components/titlebar/LayoutToggles.module.css

## Содержание/структура
`CheckboxDropdownRow()` в Block «Checkbox dropdown», hint: «LayoutToggles recipe — clicks toggle items WITHOUT closing the menu (only outside-click / Esc dismiss).»
Превью — статично встроенное меню (`style="position: static; boxShadow: none"`):
- `<ul class=menu role=menu>`:
  - `<li class=menuLabel>Sample</li>`
  - 3 `<li><button role=menuitemcheckbox aria-checked class=menuItem>`:
    - `<span class="check [checkOn]" aria-hidden>` — внутри `codicon-check` только когда включено
    - `<span class=itemLabel>Option A/B/C</span>`
- state: A=true, B=false, C=true; клик тогглит только свой пункт

## Метрики
- `.menu`: position fixed (в превью переопределено на static); z-index `--z-dropdown` (100); min-width 220px; background `--bg-surface` (#3d3f51); border `1px solid var(--divider-soft)`; border-radius `--radius-md` (12px); box-shadow `--shadow-dropdown`; list-style none; margin 0; padding `--space-1` (4px); flex column; gap 1px; max-height `calc(100vh - 16px)`; overflow-y auto
- `.menuLabel`: padding 4px 12px; font-size 11px; uppercase; letter-spacing 0.04em; color `--text-muted`
- `.menuItem`: flex; align-items center; gap 8px; width 100%; padding 8px 12px; background transparent; border none; border-radius 8px; color `--text-primary`; font inherit, 12px; text-align left; cursor pointer
- `.check`: inline-flex; центрирование; 16×16px; border-radius 3px; border `1px solid var(--bg-overlay)`; flex-shrink 0; `.check .codicon` 12px, line-height 1
- `.itemLabel`: flex 1

Сопутствующие классы модуля (не в превью): `.anchor` relative + `-webkit-app-region: no-drag`; `.trigger` 26×26px, grid, radius 12px, color `--text-secondary`, `> i` 13px, hover bg `--bg-surface` + `--text-primary`, `[aria-expanded="true"]` bg `color-mix(accent-primary 16%)`; `.itemIcon` color `--text-muted`; `.itemHint` 11px `--text-disabled`; `.divider` 1px, margin 4px 8px, bg `--divider-soft`; `.presetEmpty`; `.presetRow` flex gap 1px; `.presetApply` (flex 1, padding 8px 12px, hover `color-mix(text-primary 10%)`, label ellipsis); `.presetIconBtn` 26×26, hover `color-mix(text-primary 10%)` + `--text-primary`, `[aria-pressed="true"]` color `--accent-primary`, `> i` 13px.

## Состояния/варианты
- checked (`.checkOn`): background `--accent-primary`; border-color `--accent-primary`; color `--accent-action-fg` (галка)
- unchecked: пустой квадрат с рамкой `--bg-overlay`
- `.menuItem:hover:not([disabled])`: background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- `.menuItem[disabled]`: cursor not-allowed; color `--text-muted`; `.itemIcon` opacity 0.4
- Ключевое поведение: клик по пункту НЕ закрывает меню; закрытие — outside-click / Esc

### Наша реализация

# 148 sample-checkbox-dropdown — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_checkbox_dropdown`), root.rs (`DesignAction::ToggleCheck`)

## Структура/содержание
Статично встроенное меню (position static, без тени — как превью оригинала): «SAMPLE» + Option A/B/C со стартовыми true/false/true. Клик тогглит только свой пункт и НЕ закрывает меню.

## Метрики (из кода, точные)
- `.menu`: min-w 220, bg `--bg-surface` #3d3f51, рамка 1px divider-soft (text-primary 6%), radius RADIUS_MD 12, padding 4, gap 1.
- `.menuLabel`: px 12 / py 4, fs 11, uppercase, text-muted.
- `.menuItem`: gap 8, w-full, px 12 / py 8, radius 8, fs 12, text-primary; hover — text-primary 10%.
- `.check`: 16×16, radius 3, рамка 1px `--bg-overlay`; включённый — заливка и рамка accent-primary, галка codicon 12 цветом `--accent-action-fg`.

## Отличия от original.md той же папки
`letter-spacing .04em` у label недоступен в gpui. Скролл (`max-height: calc(100vh - 16px)`) статичному превью не нужен.

## Дополнение атрибутов (цикл 10)

- шрифты: `.menuLabel` font-size 11 (FS_XS), font-weight 400, текст через `to_uppercase()` (design_samples.rs:173-175); `.menuItem` font-size 12 (FS_SM), font-weight 400 (design_samples.rs:501); галка — codicon font-size 12 (FS_SM) (design_samples.rs:489)

### Вердикты

# 148 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока чекбокс-дропдауна нет (LayoutToggles-рецепт: menu 220 min-w, p 4, gap 1; check 16 r3 border bg-overlay, checkOn accent-primary + accent-action-fg).

## Цикл 7: MATCH

Меню/label/item/check сверены построчно с `LayoutToggles.module.css:38-111`: min-w 220,
bg-surface, divider-soft (= text-primary 6%), r12, p4, gap1; label 4/12/11/uppercase;
item 8/12, r8, fs12, hover text-primary 10%; check 16×16, r3, рамка bg-overlay,
включённый — accent + галка 12; состояния true/false/true; клик не закрывает меню.
Из ours.md убрано неверное «mr 4» — отступ даёт `gap 8`, как в оригинале.

---

## 149. sample-context-menu — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](149-sample-context-menu/original.png)
![наш](149-sample-context-menu/ours.png)

### Оригинал

# 149 sample-context-menu — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:143-159, components/activity-bar/ActivityContextMenu.module.css

## Содержание/структура
`ContextMenuRow()` в Block «Context menu», hint: «ActivityContextMenu recipe — right-click in the live UI; here a static preview of the same surface.»
Статичное превью (`style="position: static; boxShadow: none"`):
- `<div class=menu role=menu>`:
  1. `<button role=menuitem class=item>`: `codicon-eye-closed` + `<span class=itemLabel>Hide</span>`
  2. `<button role=menuitem aria-haspopup=menu class="item itemMoveTo">`: `codicon-arrow-right` + `<span class=itemLabel>Move to</span>` + `codicon-chevron-right` c классом `.chevron`

## Метрики
- `.menu`, `.submenu`: position fixed (превью — static); z-index `--z-dropdown` (100); min-width 180px; background `--bg-surface` (#3d3f51); border `1px solid var(--divider-soft)`; border-radius `--radius-md` (12px); box-shadow `--shadow-dropdown` (0 4px 16px rgba(0,0,0,0.5)); list-style none; margin 0; padding 4px; flex column; gap 1px; max-height `calc(100vh - 16px)`; max-width `calc(100vw - 16px)`; overflow-y auto
- `.item`, `.subItem`: flex; align-items center; gap 8px; width 100%; padding 8px 12px; background transparent; border none; border-radius 8px; color `--text-primary`; font inherit, `--fs-sm` (12px); text-align left; cursor pointer
- `.itemLabel`, `.subItemLabel`: flex 1
- `.chevron`: font-size 12px; color `--text-muted`
- `.subItemIcon`: inline-flex, центрирование, color `--text-muted`

## Состояния/варианты
- hover (`.item:hover`, `.subItem:hover`): background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- открытый сабменю (`.itemMoveTo[aria-expanded="true"]`): background `color-mix(in srgb, var(--accent-primary) 16%, transparent)`; color `--text-primary` — строка «Move to» остаётся подсвеченной как breadcrumb
- в живом UI меню и сабменю рендерятся порталом в `<body>` с position: fixed

### Наша реализация

# 149 sample-context-menu — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_context_menu`)

## Структура/содержание
Статичное превью поверхности ActivityContextMenu: «Hide» (codicon-eye-closed) и «Move to» (codicon-arrow-right + chevron-right справа).

## Метрики (из кода, точные)
- `.menu`: min-w 180, bg `--bg-surface`, рамка 1px divider-soft (text-primary 6%), radius 12, padding 4, gap 1, без тени.
- `.item`: gap 8, w-full, px 12 / py 8, radius 8, fs FS_SM 12, text-primary; hover — text-primary 10%.
- `.chevron`: fs 12, text-muted.

## Отличия от original.md той же папки
Состояние `.itemMoveTo[aria-expanded=true]` (accent 16%) в статичном превью не показывается — как и в оригинале.

## Дополнение атрибутов (цикл 10)

- шрифты: `.item` font-size 12 (FS_SM), font-weight 400 (design_samples.rs:529); глиф пункта — codicon font-size 16 (база `.codicon`, design_samples.rs:534); chevron — codicon font-size 12 (FS_SM) (design_samples.rs:538)

### Вердикты

# 149 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока контекстного меню нет (Hide + Move to ▸).

## Цикл 7: DIVERGES

Поверхность и пункты совпали (min-w 180, bg-surface, divider-soft, r12, p4, gap1, без
тени; item 8/12 r8 fs12, hover text-primary 10%, порядок Hide → Move to ▸, chevron 12).
Исправлено по ревью: глифы `eye-closed`/`arrow-right` 16 вместо 13 (`.item` кегль
кодикона не задаёт → база 16px).

Осталось: нет пары кадров — вердикт по коду.

---

## 150. sample-section-header — **MATCH** (цикл 6)

*История: ц5:DIVERGES, ц6:MATCH*

![оригинал](150-sample-section-header/original.png)
![наш](150-sample-section-header/ours.png)

### Оригинал

# 150 sample-section-header — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:161-178 (inline-стили; общего класса нет — живой стиль в SessionsMode.module.css и CustomizeMode.module.css по отдельности)

## Содержание/структура
`SectionHeaderRow()` в Block «Section header», hint: «Sidebar landmark — uppercase, muted, 0.08em letter-spacing.»
Один `<div>` с текстом «SECTION» и inline-стилем — рецепт лендмарков PROJECTS / CUSTOMIZE в сайдбаре.

## Метрики
Inline-стиль (точный):
```
padding: 8px 12px;
font-size: var(--fs-xs);        /* 11px */
font-weight: 500;
letter-spacing: 0.08em;
color: var(--text-muted);       /* #838aa0 dark */
font-feature-settings: 'ss01';
```

## Состояния/варианты
Статичный, интерактива нет. Текст в разметке уже uppercase («SECTION») — CSS text-transform не используется.

### Наша реализация

# 150 sample-section-header — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_section_header`, блок «Section header»)

## Структура/содержание
Ландмарк-заголовок сайдбара: одна строка «SECTION».

## Метрики (из кода, точные)
- px 12, py SPACE_2 8.
- fs FS_XS 11, weight 500 через `typo::ss01(MEDIUM)` (то же начертание, что у PROJECTS/CUSTOMIZE).
- Цвет text-muted #838aa0.
- Фона и скругления нет — прозрачная строка.

## Отличия от original.md той же папки
`letter-spacing: 0.08em` в gpui недоступен — единственное расхождение.

### Вердикты

# 150 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: MATCH

Заголовок секции: px 12 / py 8 / fs-xs / вес 500 / text-muted / ss01. letter-spacing 0.08em — ограничение gpui.

---

## 151. sample-status-bar-items — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](151-sample-status-bar-items/original.png)
![наш](151-sample-status-bar-items/ours.png)

### Оригинал

# 151 sample-status-bar-items — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:180-195, components/status-bar/StatusBar.module.css

## Содержание/структура
`StatusItemRow()` в Block «Status-bar items»: 4 кнопки на реальных классах StatusBar.module.css:
1. `.item .ok`: `codicon-circle-filled` + «3 active»
2. `.item .warn`: `codicon-warning` + «2 failed»
3. `.item`: «UTF-8» (нейтральный)
4. `.item .brand`: «KaminIDE 0.0.1»

## Метрики
- `.item`: flex; align-items center; gap 4px; padding `0 var(--space-2)` (0 8px); color `--text-muted`; border-radius `--radius-xs` (4px); font-size `--fs-xs` (11px)
- `.item .codicon`: font-size 12px !important

Контекст живого StatusBar (не в превью): `.statusBar` height `var(--layout-status-bar-height)`; background transparent; flex, align-items stretch; font-size 11px; color `--text-muted`; padding 0 8px; gap 4px. `.left`/`.right` gap 2px; `.right` margin-left auto.

## Состояния/варианты
- hover (`.item:hover`): background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`; color `--text-primary`
- `.ok`: color `--accent-green` (#a6e3a1)
- `.warn`: color `--accent-yellow` (#f9e2af)
- `.brand`: color `--accent-primary` (#89b4fa); font-weight 500
- в живом StatusBar также: `.clickable` cursor pointer; `.item:disabled` cursor default, hover нейтрализован; `.update` (accent-пилюля 22% tint, weight 600, hover 34%); `.downloading` + `.progressFill` (fill `color-mix(accent-primary 32%)`, transition width 120ms linear) + `.progressLabel` — в превью не показаны

### Наша реализация

# 151 sample-status-bar-items — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_status_items`, блок «Status-bar items»)

## Структура/содержание
Четыре элемента статус-бара: «3 active» (ok), «2 failed» (warn), «UTF-8» (нейтральный), «KaminIDE 0.0.1» (brand).

## Метрики (из кода, точные)
- Элемент: flex, items-center, gap 4, px SPACE_2 8, radius RADIUS_XS 4, fs 11; глиф codicon 12.
- ok: accent-green #a6e3a1; warn: accent-yellow #f9e2af; нейтральный: text-muted #838aa0.
- brand: accent-primary #89b4fa, weight 500.

## Отличия от original.md той же папки
Ховера у семпла нет (в живом статус-баре элемент подсвечивается) — семпл статичный.

### Вердикты

# 151 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

gap 4, px 8, r-xs, fs 11, глиф 12, ok=green, warn=yellow, brand=accent+500, глифы circle-filled/warning — 1:1. Нет `.item:hover{bg-surface 60%; text-primary}` и .id().

---

## 152. sample-panel-icon-family — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

![оригинал](152-sample-panel-icon-family/original.png)
![наш](152-sample-panel-icon-family/ours.png)

### Оригинал

# 152 sample-panel-icon-family — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:197-211, components/titlebar/PanelIcon.tsx, design-sections.module.css:418-425 (.codeInline)

## Содержание/структура
`PanelIconFamilyRow()` в Block «Panel icon family», hint: «Same SVG family used by LayoutToggles + PanelPlaceholder — frame + highlighted slot.»
8 слотов: left, main, main-bottom, center, center-bottom, right, right-top, right-bottom. Каждый:
`<span data-tooltip={slot} style="display:inline-flex;flex-direction:column;align-items:center;gap:4px;color:var(--text-secondary)">` → `<PanelIcon slot>` + `<code class=codeInline style="font-size:10px">{slot}</code>`

## Метрики
PanelIcon (SVG 14×12, viewBox 0 0 14 12, aria-hidden):
- рамка: rect x=1 y=1 w=12 h=10, rx/ry 1.5, fill none, stroke currentColor, stroke-width 1.2 (STROKE_INSET 1)
- highlight: fill currentColor, opacity 0.85, rx/ry 1 (SLOT_RADIUS), SLOT_INSET 1.5
- константы: LEFT_HIGHLIGHT_W = RIGHT_HIGHLIGHT_W = CENTER_HIGHLIGHT_W = 4.5; BOTTOM_HIGHLIGHT_INSET_Y = 5; RIGHT_HIGHLIGHT_INSET = 6 → RIGHT_HIGHLIGHT_X = 8; RIGHT_QUARTER_HEIGHT = (12 − 3)/2 = 4.5; RIGHT_QUARTER_BOTTOM_Y = 6
- слоты:
  - main / left: rect x=1.5 y=1.5 w=4.5 h=9 (левая колонка; main = зеркало right)
  - right: x=8 y=1.5 w=4.5 h=9
  - right-top: x=8 y=1.5 w=4.5 h=4.5
  - right-bottom: x=8 y=6 w=4.5 h=4.5
  - center: x=(14−4.5)/2=4.75 y=1.5 w=4.5 h=9
  - center-bottom: x=4.75 y=7 w=4.5 h=3.5
  - main-bottom: x=1.5 y=6 w=4.5 h=4.5
  - default (bottom, legacy): x=1.5 y=7 w=11 h=3.5
- порядок отрисовки: сначала highlight, поверх frame

Подпись: `.codeInline` (mono, color `--accent-primary`, bg `color-mix(accent-primary 10%)`, padding 1px 6px, radius 4px), inline override font-size 10px. Обёртка: gap 4px, color `--text-secondary` (SVG красится currentColor).

## Состояния/варианты
Тип PanelSlot имеет 9 значений (8 показанных + legacy alias "bottom" — full-width полоса, ветка default). data-tooltip на каждой обёртке показывает имя слота при hover.

### Наша реализация

# 152 sample-panel-icon-family — наша реализация
Файлы: `crates/shell/src/ui/design_panel.rs` (`fn sample_panel_icons`), `crates/shell/src/ui/panel_placeholder.rs` (`slot_glyph_small`, `enum SlotIcon`)

## Структура/содержание
Восемь подписанных иконок слотов в порядке оригинала: left, main, main-bottom, center, center-bottom, right, right-top, right-bottom. Каждая — рамка-канва с подсвеченным слотом плюс подпись под ней.

## Метрики (из кода, точные)
- Ряд: flex-wrap, gap SPACE_3 12.
- Ячейка: flex-col, items-center, gap 4, цвет text-secondary #adb3c7.
- Подпись: fs FS_XS 11, text-muted #838aa0.
- Иконка: канва 14×12, рамка rect 12×10 rx 1.5 штрих 1.2, подсвеченный бар — text-muted при alpha 0.85, инсет слота 1.5 (`SLOT_INSET` оригинала).

## Отличия от original.md той же папки
1. Иконка рисуется нативными div-барами, а не SVG (в gpui нет inline-SVG с произвольной геометрией) — форма выверена по `PanelIcon.tsx`.
2. `left` и `main` в оригинале дают одну и ту же фигуру — у нас обе подписи используют один вариант `SlotIcon::Main`; в перечислении есть ещё legacy-вариант `Bottom`, в витрине не показанный.
3. Подпись у нас — обычный текст, в оригинале `<code class=codeInline>` моно 10px.

## Дополнение атрибутов (цикл 10)

- скругления: у бара-подсветки слота radius 1×scale, у самой рамки-канвы — rx 1.5 штриха (`crates/shell/src/ui/panel_placeholder.rs`, `fn glyph`); внешнего скругления у ячейки нет
- ховер: N/A: ховер

### Вердикты

# 152 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Набор слотов: оригинал left/main/main-bottom/center/center-bottom/right/right-top/right-bottom — у нас «main» пропал, «bottom» лишний. Подписи: оригинал <code> accent-primary на accent 10%, r-xs, padding 1/6, кегль 10 — у нас простой текст fs-xs muted без плашки.

---

## 153. sample-placeholders — **DIVERGES** (цикл 7)

*История: ц5:DIVERGES, ц6:DIVERGES, ц7:DIVERGES*

![оригинал](153-sample-placeholders/original.png)
![наш](153-sample-placeholders/ours.png)

### Оригинал

# 153 sample-placeholders — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:213-221, components/panel-placeholder/ActivityPlaceholder.tsx, ActivityPlaceholder.module.css

## Содержание/структура
`PlaceholdersRow()` в Block «Empty / active panel placeholders», hint: «ActivityPlaceholder is shown once a tool is picked but its renderer isn't ready yet (Phase A).»
Карточка-обёртка (inline): `width:100%; max-width:280px; min-height:160px; border-radius:var(--radius-md); background:var(--bg-mantle); display:flex; flex-direction:column` → внутри `<ActivityPlaceholder icon="terminal" label="Terminal" />`.

ActivityPlaceholder — empty-state АКТИВНОЙ активности без готового рендерера (отличен от PanelPlaceholder — empty-state «активность не выбрана», с Open Tool picker; здесь пикер намеренно опущен):
- `.placeholder` → `<ToolIcon icon size={36} class=glyph>` + `<h2 class=label>{label}</h2>` + `<p class=hint>Nothing to show here yet.</p>`

## Метрики
- `.placeholder`: flex 1; flex column; align-items center; justify-content center; text-align center; gap `--space-2` (8px); padding `--space-5` (20px); color `--text-muted`
- `.glyph`: font-size 36px (GLYPH_SIZE_PX = 36); color `--text-disabled` (#60667b); margin-bottom `--space-1` (4px)
- `.label`: margin 0; font-size `--fs-md` (13px); font-weight 600; color `--text-primary`
- `.hint`: margin 0; font-size `--fs-xs` (11px); color `--text-muted`; line-height `--lh-snug` (1.3); max-width 240px
- карточка-обёртка превью: max-width 280px; min-height 160px; radius 12px; bg `--bg-mantle` (#262533)

## Состояния/варианты
Статичный. Props: icon (строка для ToolIcon), label. Текст hint фиксированный: «Nothing to show here yet.»

### Наша реализация

# 153 sample-placeholders — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_placeholders`), ui/panel_placeholder.rs (`activity_placeholder`)

## Структура/содержание
Карточка-обёртка вокруг `activity_placeholder("terminal", "Terminal", p)`: глиф 36 text-disabled + заголовок + «Nothing to show here yet.».

## Метрики (из кода, точные)
- Обёртка: w-full, max-w 280, min-h 160, radius RADIUS_MD 12, bg `--bg-mantle`, flex-col.
- Плейсхолдер: gap SPACE_2 8, padding SPACE_5 20, центровка; глиф 36 text-disabled; label fs FS_MD 13 / 600 text-primary; hint fs FS_XS 11 text-muted, lh 1.3, max-w 240.

## Отличия от original.md той же папки
Нет.

## Дополнение атрибутов (цикл 10)

- шрифты: label font-size 13 (FS_MD) / font-weight 600 SEMIBOLD (panel_placeholder.rs:179-180); hint font-size 11 (FS_XS), line-height 14.3 = 11×1.3, font-weight 400 (panel_placeholder.rs:186-188); глиф — svg/codicon 36px (panel_placeholder.rs:149)

### Вердикты

# 153 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Не реализовано: блока нет в нашей Design-панели (`design_panel.rs` заканчивается на кнопках и чипах). Оригинал — `main/component-samples*.tsx` + `design-sections.module.css`. Волна 8: строить блоки через общий `Block`-враппер (compRow + compLabel uppercase fs-xs muted + compHint + compInline wrap gap 8).

## Цикл 6: DIVERGES

Блока плейсхолдеров нет (обёртка 280×160 min-h, r-md, bg-mantle + ActivityPlaceholder).

## Цикл 7: DIVERGES

Обёртка 280/160/r12/bg-mantle, глиф 36 text-disabled + mb 4, label fs-md/600, hint
fs-xs/max-w 240/text-muted, текст дословно. Исправлено по ревью: у hint появился
`line-height: --lh-snug` 1.3 (был не задан, а ours.md утверждал обратное), и
`activity_placeholder` берёт путь Phosphor-иконки ИЗ мапы (алиасы вроде «problems»
давали несуществующий `icons/problems.svg`).

Осталось: `flex: 1` оригинала у нас `size_full()`; нет пары кадров.

---

## 154. global-scrollbar — **MATCH** (цикл 5)

*История: ц5:MATCH*

![оригинал](154-global-scrollbar/original.png)
![наш](154-global-scrollbar/ours.png)

### Оригинал

# 154 global-scrollbar — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:25-29; kamin-ide/src/renderer/theme/skeleton.css:20-23 (вебвью-вариант)

## Содержание/структура
Сквозной стиль webkit-скроллбара для всего renderer-документа; skeleton.css дублирует его для webview-страниц.

## Метрики
Полные правила (global.css:25-29):
```css
::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--bg-overlay); border-radius: var(--radius-xs); }
::-webkit-scrollbar-thumb:hover { background: var(--text-disabled); }
::-webkit-scrollbar-corner { background: transparent; }
```
skeleton.css:20-23 (отличия: radius захардкожен 4px, нет правила corner):
```css
::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--bg-overlay); border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: var(--text-disabled); }
```
Значения (dark): толщина 8×8px; трек и corner прозрачные; thumb `--bg-overlay` #515567, radius 4px; thumb hover `--text-disabled` #60667b.

## Состояния/варианты
default / thumb hover. Исключение по месту: `.tabs` в BottomTabBar прячет скроллбар (`scrollbar-width: none`).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — псевдоэлементы скроллбара задают только width/height 8px, background и border-radius; ни padding, ни margin в правилах нет (global.css:25-29, skeleton.css:20-23); глобальный сброс `* { margin: 0; padding: 0; box-sizing: border-box }` (global.css:12)

### Наша реализация

# 154 global-scrollbar — наша реализация
Файлы: vendor/gpui-component/src/scroll/scrollbar.rs:20-30, 390-460 (геометрия/стили), crates/shell/src/theme_sync.rs:415-424 (цвета+режим), применение — `.overflow_y_scrollbar()` в ui/{design_panel,logs_panel,problems,extensions_panel,quick_open,quick_pick,find_in_files,command_palette,workspace_symbols,sessions_list,file_list}.rs; отдельный редакторный — crates/shell/src/ui/editor_minimap.rs:202-280

## Структура/содержание
Сквозной скроллбар = vendored gpui-component Scrollbar (overlay-тип), навешивается per-контейнер методом `overflow_y_scrollbar()`. Режим `ScrollbarShow::Hover` — в покое скрыт, появляется при hover контейнера. Трек прозрачный. У редактора свой кастомный scrollbar (editor_minimap.rs) — вне этого элемента.

## Метрики (из кода, точные)
- Резерв ширины WIDTH = 4·2+8 = 16px; MIN_THUMB_SIZE 48px.
- Idle-thumb (режим Scrolling): ширина 6, radius 3, inset 4. Hover/активный thumb: ширина 8, radius 4, inset 4 → визуальная толщина совпадает с 8px оригинала.
- Цвета (theme_sync): track transparent; thumb = bg_overlay α 0.35; thumb hover = text_disabled α 0.5.

## Отличия от original.md той же папки
- Видимость: оригинал — постоянно видимый webkit-скроллбар; у нас — overlay, показывается только на hover (ScrollbarShow::Hover). Осознанное расхождение поведением.
- Цвет thumb: сплошной #515567 → у нас тот же токен, но α 0.35 (тусклее); hover #60667b → у нас α 0.5.
- Radius: оригинал 4px (radius-xs); у нас radius = width/2 (3 или 4) — на hover совпадает.
- Толщина: 8px совпадает в hover-состоянии; в промежуточном «scrolling»-стиле 6px.
- Per-container вместо глобального `::-webkit-scrollbar` — контейнеры без `.overflow_y_scrollbar()` скроллбара не имеют (аналог «`.tabs` прячет скроллбар» получается бесплатно).
- corner-правило не нужно (overlay, горизонталь+вертикаль не пересекаются визуально).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы

### Вердикты

# 154 global-scrollbar — verdict (review cycle 1)
VERDICT: MATCH
theme_sync.rs:418-421: track transparent, thumb bg-overlay #515567,
hover text-disabled #60667b, ScrollbarShow::Always; вендорная геометрия 8px/r4 = CSS.
Каверзы (не вердикт-брейкеры): 8px-thumb в 16px-лейне с инсетом 4 (не вплотную
к краю); webview-вариант skeleton.css не портирован (webview_theme.rs шлёт только
--vscode-scrollbarSlider-* переменные).

## Цикл 5: MATCH

Скроллбар: track прозрачный, thumb `bg_overlay` #515567, hover `text_disabled` #60667b, `ScrollbarShow::Always`; вендор при Always даёт width 8 / radius 4 = CSS. Каверза: thumb в 16px-лейне с инсетом 4 (не вплотную к краю); вариант из skeleton.css для вебвью не портирован.

---

## 155. glint-surface-card-ring — **MATCH** (цикл 5)

*История: ц5:MATCH*

![оригинал](155-glint-surface-card-ring/original.png)
![наш](155-glint-surface-card-ring/ours.png)

### Оригинал

# 155 glint-surface-card-ring — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:88-101; токен `--glint-border` — theme/dark-theme.css:31 (фоллбек variables.css:25)

## Содержание/структура
Фирменный вид «плавающей карточки»: fill `--bg-mantle` + диагональная подсвеченная рамка. Единственный источник рецепта — panel-модули подключают через `composes: glint-surface from global;` (карточки FilePanel / RightPanel / MainContent / MainBottomPanel + рамка sidebar-вебвью); обычный DOM добавляет класс напрямую.

## Метрики
Полное правило (global.css:96-101):
```css
.glint-surface {
  border: 1px solid transparent;
  background:
    linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box,
    var(--glint-border) border-box;
}
```
Токен (dark-theme.css:31):
```css
--glint-border: linear-gradient(135deg, rgba(255, 255, 255, 0.18) 0%, var(--bg-mantle) 22%, var(--bg-mantle) 78%, rgba(255, 255, 255, 0.18) 100%);
```
:root-фоллбек в variables.css:25 отличается: mid-стопы `var(--bg-base)` вместо `var(--bg-mantle)`.
Механика: рамка 1px transparent; двухслойный background — сплошной `--bg-mantle` в padding-box, градиент в border-box → рамка видна только в 1px-кольце. Mid-стопы = цвету панели, поэтому рамка «тает» в заливку и читается только диагональный блик (0% и 100% — rgba(255,255,255,0.18)). Значения dark: `--bg-mantle` #262533.

## Состояния/варианты
Состояний нет. Тема меняет `--glint-border` (white-tinted на dark, warm-ink на светлой) — рецепт один.

### Наша реализация

# 155 glint-surface-card-ring — наша реализация
Файлы: crates/shell/src/ui/glint.rs:28-59 (`glint_surface`), 64-233 (`hole_segments*`, `glint_surface_wv_holed`); токены — crates/theme/src/palette.rs:48-49, 86-87, 124-125 (glint_edge/glint_mid)

## Структура/содержание
`glint_surface(p, content)`: 4 слоя div — (1) сплошной mid (glint_mid), (2) linear_gradient 135° edge(α)→edge(0) на 0%→22%, (3) linear_gradient 135° edge(0)→edge(α) на 78%→100%, (4) внутренний rect p(1px) с заливкой bg_mantle radius 15. gpui 0.2.2 даёт максимум 2 стопа на градиент — 4-стоповый CSS-глинт собран из двух наложенных 2-стоповых слоёв (за пределами стопов градиент клампится → между 22% и 78% чистый mid). Пиксельно эквивалентно оригиналу.
Вариант `glint_surface_wv_holed`: те же 4 слоя paint_quad-ами через content-mask сегментов вокруг «дыр» composition-вебвью + антиалиасные угловые маски radius 12 (полилиния 12 сегментов) — зона остаётся прозрачной для underlay-вебвью.
Используется карточками MainContent / FilePanel / RightPanel / MainBottomPanel (те же потребители, что в оригинале).

## Метрики (из кода, точные)
- Кромка 1px (внутренний rect inset p(1)); внешний радиус RADIUS_LG 16, внутренний 15 (concentric).
- Стопы: 0% edge α.18 → 22% α0; 78% α0 → 100% α.18; угол 135°.
- dark: edge rgba(255,255,255,.18), mid #262533 (bg_mantle); light: edge rgba(60,40,20,.18), mid #e6e1d4 (bg_surface). Закрыто тестом glint_mid_matches_panel_fill.

## Отличия от original.md той же папки
- Значения совпадают полностью: угол, стопы 0/22/78/100, α .18, fill bg-mantle, кромка 1px.
- Расхождение в light glint-mid: оригинальный токен --glint-border формально всегда ставит mid = var(--bg-mantle) (light bg-mantle #fbf7f4), у нас light mid = bg_surface #e6e1d4. Требует сверки с light-theme.css оригинала (наш комментарий утверждает «light glint mid = bg_surface» — вероятно оригинальная light-тема переопределяет токен; original.md фиксирует только dark).
- Механика: два 2-стоповых слоя вместо одного 4-стопового + inner-rect вместо padding-box/border-box трюка — визуально идентично, но проверяется скриншотом, не кодом.
- Бонус против оригинала: вариант с «дырой» под нативный вебвью (в DOM-оригинале не нужен).

### Вердикты

# 155 glint-surface-card-ring — verdict (review cycle 1)
VERDICT: MATCH
Вопрос light glint-mid ЗАКРЫТ: light-theme.css:39 использует var(--bg-surface)
(«cream paper needs the border tone») — наш LIGHT.glint_mid #e6e1d4 = bg_surface
ВЕРЕН (palette.rs:125, тест palette.rs:140-143). Кольцо 1px padding-box, 135deg,
стопы 0/22/78/100 через 2 оверлея, edge .18 обеих тем — 1:1. glint_surface()
без вызовов (все сайты — glint_surface_wv_holed с теми же пикселями).

## Цикл 5: MATCH

Glint-ring: 135°, стопы 0/22/78/100 двумя слоями, edge α .18 в обеих темах, кромка 1px, внешний радиус 16 / внутренний 15; `glint_mid` dark #262533 (= bg-mantle), light #e6e1d4 (= bg-surface) — закрыто тестом.

---

## 156. focus-visible-ring — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

*кадр «оригинал» отсутствует*
*кадр «наш» отсутствует*

### Оригинал

# 156 focus-visible-ring — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:31-43; дубликат для вебвью — theme/skeleton.css:38-41

## Содержание/структура
Единый focus-ring только для клавиатурной навигации. Компоненты часто ставят `outline: none` для чистоты клика; здесь видимость восстанавливается для tab-пользователей — мышиный клик `:focus-visible` не триггерит.

## Метрики
Полные правила (global.css:34-43):
```css
:focus-visible {
  outline: 2px solid var(--accent-primary);
  outline-offset: 2px;
}
button:focus-visible,
[role='button']:focus-visible,
a:focus-visible {
  outline: 2px solid var(--accent-primary);
  outline-offset: 2px;
}
```
skeleton.css:38-41 (вебвью, только универсальное правило):
```css
:focus-visible {
  outline: 2px solid var(--accent-primary);
  outline-offset: 2px;
}
```
Значения (dark): outline 2px solid #89b4fa (`--accent-primary`); offset 2px.

## Состояния/варианты
Активен только при keyboard-focus (`:focus-visible`); при mouse-клике не показывается.

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding/margin у правил нет; единственный отступ — `outline-offset: 2px` в обоих блоках (global.css:36, 42) и в вебвью-варианте (skeleton.css:40); глобальный сброс `* { margin: 0; padding: 0 }` (global.css:12)

### Наша реализация

# 156 focus-visible-ring — наша реализация
Файлы: — (в crates/shell/src нет ни одного focus-ring стиля; grep по outline/focus_visible/on_focus пуст; track_focus только у root.rs:4775 и terminal_focus root.rs:3261 — без визуала)

## Структура/содержание
НЕ РЕАЛИЗОВАНО. Клавиатурного focus-ring нет вовсе: FocusHandle используются для маршрутизации ввода (терминал, глобальные хоткеи), но видимой обводки сфокусированного интерактивного элемента ни один компонент не рисует. Tab-навигации по кнопкам в gpui-порте пока нет как класса.

## Метрики
—

## Отличия от original.md той же папки
Отсутствует весь элемент: outline 2px solid accent-primary, offset 2px, только при keyboard-focus (аналог :focus-visible — в gpui пришлось бы отличать фокус от клавиатуры вручную). Портировать вместе с появлением tab-обхода интерактивных элементов.

## Дополнение атрибутов (цикл 10)

- цвета: focus-ring в порте не реализован — в `crates/shell/src` нет ни одного outline/ring-стиля (см. ours.md), красить нечего. N/A: цвета
- отступы: N/A: отступы

### Вердикты

# 156 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Focus-ring не реализован: grep по `outline|focus_visible|focus_ring` в `crates/shell/src` пуст. Оригинал `global.css`: outline 2px solid accent-primary, offset 2px. Скрина нет — состояние не воспроизводилось.

## Цикл 6: DIVERGES

focus-visible кольца нет вовсе: grep outline|focus_visible|focus_ring по crates/shell/src — только строковые совпадения. Оригинал global.css:34-43: outline 2px accent-primary, offset 2.

---

## 157. activity-drop-highlight — **DIVERGES** (цикл 6)

*История: ц5:DIVERGES, ц6:DIVERGES*

*кадр «оригинал» отсутствует*
*кадр «наш» отсутствует*

### Оригинал

# 157 activity-drop-highlight — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:45-67

## Содержание/структура
Глобальные utility-атрибуты для карточки-приёмника при drag pinned-активности. Любой card-host (sidebar, file-panel, right-panel cards, main-bottom, main) вешает `data-activity-drop` вместо форка рецепта в семи module.css. Визуал «over» совпадает со strip-уровневым placeholder внутри (accent-tint + dashed), чтобы карточка и insertion-gap читались одним drop-превью.

## Метрики
Полные правила:
```css
[data-activity-drop="over"] {
  background-color: color-mix(in srgb, var(--accent-primary) 10%, transparent);
  outline: 1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent);
  outline-offset: -2px;
  transition: background-color var(--transition-fast), outline-color var(--transition-fast);
}

[data-activity-drop="blocked"] {
  background-color: color-mix(in srgb, var(--accent-red) 12%, transparent);
  box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent);
  transition: background-color var(--transition-fast), box-shadow var(--transition-fast);
}
```
`--transition-fast` = 150ms ease. Базовые цвета (dark): `--accent-primary` #89b4fa, `--accent-red` #f38ba8.

## Состояния/варианты
- `over` — карточка примет drop: accent-tint 10% + dashed outline 1px (accent 60%), offset −2px
- `blocked` — drop будет no-op (назначение уже содержит активность): красный tint 12% + inset-«рамка» box-shadow 2px (red 60%); outline намеренно отсутствует

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding/margin правила не задают; единственный инсет — `outline-offset: -2px` у `[data-activity-drop="over"]` (global.css:56); у `blocked` вместо outline рамка внутрь через `box-shadow: inset 0 0 0 2px` (global.css:65)

### Наша реализация

# 157 activity-drop-highlight — наша реализация
Файлы: crates/shell/src/root.rs:4946-4973 (hit-test дроп-зон по probe_registry: sidebar/right-top/main-bottom/right-bottom), root.rs:5411-5441 (ghost у курсора: α 0.45 без цели / 0.85 над целью), root.rs:2176-2184 + ui/slot_panel.rs:111-113 (accent-полоса border_l_2 у таба-цели вставки)

## Структура/содержание
ЧАСТИЧНО. При drag плитки тула: (1) hit-test 4 дроп-зон по probe-bounds; (2) ghost-пилюля у курсора (label тула, bg accent_primary, α 0.45→0.85 когда над валидной зоной) — единственная индикация «зона примет дроп»; (3) внутри стрипа зоны — вставочная accent-полоса `border_l_2` accent_primary у таба под курсором. Подсветки САМОЙ карточки-приёмника (fill + dashed outline) нет; состояния «blocked» (красный, назначение уже содержит активность) нет.

## Метрики (из кода, точные)
Ghost: offset +10/+8 от курсора; px SPACE_3 / py 4; radius SM 8; fs SM 12; bg accent_primary α 0.85 (над зоной) / 0.45 (вне); текст accent_action_fg. Вставочная метка: border-left 2px accent_primary. Порог начала drag 4px (1:1 с activity-dnd).

## Отличия от original.md той же папки
- `[data-activity-drop="over"]` (bg accent 10% + dashed outline 1px accent 60%, offset −2, transition 150ms) — НЕ РЕАЛИЗОВАНО: карточка-приёмник не подсвечивается, сигнал перенесён на α ghost'а.
- `[data-activity-drop="blocked"]` (red tint 12% + inset box-shadow 2px red 60%) — НЕ РЕАЛИЗОВАНО, blocked-состояние не вычисляется.
- Insertion-метка у нас — сплошная полоса 2px (в оригинале strip-placeholder — dashed прямоугольник 32/36px, зона 41/50); совпадает только идея accent-цвета.
- Ghost-пилюля с label — наша замена ActivityDragGhost (элемент 47), в оригинале ghost — иконка, а не подпись.

## Дополнение атрибутов (цикл 10)

- гэпы: N/A: гэпы — и ghost, и drop-placeholder'ы это одиночные боксы без внутренних рядов; расстояния задают контейнеры (`.list` gap 2 в activity_bar.rs, стрип gap 4 в slot_panel.rs)
- цвета: ghost — фон = непрозрачная смесь p.accent_primary #89b4fa 22% над p.bg_surface #3d3f51, border 1px p.accent_primary #89b4fa α .5, глиф p.accent_primary #89b4fa, shadow 0 4 14 rgba(0,0,0,.35), opacity .92 (root.rs:6482-6528); drop-placeholder бара — border 1px dashed p.accent_primary α .7, bg p.accent_primary α .14 (activity_bar.rs:141-155); drop-placeholder стрипа — те же α .7 / α .14 (slot_panel.rs:126-136); подсветки самой карты-приёмника и `blocked` (p.accent_red #f38ba8) нет
- ОШИБКА В ours.md: ghost описан как «пилюля с label, px SPACE_3 / py 4, fs 12, bg accent_primary α 0.85 / 0.45, текст accent_action_fg» — в коде ghost это квадрат 28×28 с одним глифом (без подписи), rounded 8, фон-смесь accent 22% + bg_surface, opacity .92, вид от наличия цели НЕ зависит (root.rs:6470-6530)

### Вердикты

# 157 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: DIVERGES

Drop-подсветка частично: есть ghost-пилюля у курсора (α 0.85 над целью / 0.45 вне) и accent-полоса вставки `border_l_2`. Нет подсветки карточки-приёмника `[data-activity-drop="over"]` (bg accent 10% + dashed 1px accent 60%, offset −2) и состояния `blocked` (red 12% + inset 2px red 60%). Скрина нет — нужен активный drag.

## Цикл 6: DIVERGES

★ Подпункт ц.5 про ghost ЛОЖЕН: в порте одна прозрачность 0.92 без вариантов, ровно как в ActivityDragGhost.module.css. РЕАЛЬНО осталось: `[data-activity-drop=over]` (accent 10% + dashed accent 60%, offset −2) и `blocked` (red 12% + inset 2px red 60%) на семи картах-приёмниках — в порте подсветки карты нет, есть только accent-полоса на табе; blocked не вычисляется.

---

## 158. dragging-body-classes — **MATCH (N/A)** (цикл 5)

*История: ц5:MATCH (N/A)*

*кадр «оригинал» отсутствует*
*кадр «наш» отсутствует*

### Оригинал

# 158 dragging-body-classes — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:69-86 (классы вешает useDragHandler)

## Содержание/структура
Глобальные body-классы на время драга:
- `body.kamin-dragging` — драг сплиттера панелей: iframe'ы теряют pointer-events (иначе iframe-документ глотает mousemove и resize замирает); интерактивные элементы тоже теряют pointer-events, чтобы hover/tooltip не реагировали и elementFromPoint проваливался к tagged zone-контейнерам
- `body.kamin-tool-dragging` — драг тула между зонами: курсор grabbing на всём (scoped отдельным классом, чтобы не перебить col/row-resize курсор сплиттера)

## Метрики
Полные правила:
```css
body.kamin-dragging iframe { pointer-events: none; }

body.kamin-dragging :where(button, [role="button"], [role="tab"], a, [data-tooltip]) {
  pointer-events: none;
}

body.kamin-tool-dragging,
body.kamin-tool-dragging * { cursor: grabbing !important; }
```

## Состояния/варианты
Два независимых режима: kamin-dragging (сплиттер) и kamin-tool-dragging (перенос тула). Вне драга классы отсутствуют — правила неактивны.

## Дополнение атрибутов (цикл 10)

- цвета: оба правила задают только `pointer-events: none` и `cursor: grabbing`; ни одного цветового значения или токена в блоке нет (global.css:72, 79-81, 85-86). N/A: цвета
- отступы: N/A: отступы — padding/margin правила не трогают (global.css:72-86); глобальный сброс `* { margin: 0; padding: 0 }` (global.css:12)

### Наша реализация

# 158 dragging-body-classes — наша реализация
Файлы: crates/shell/src/root.rs:33-58 (DragKind/DragState), 2918-2940+ (begin_drag/drag_move), 4946-4990 (window-level mouse-move/up: drag сплиттеров, chip/tab/tool drag)

## Структура/содержание
Прямого аналога НЕТ — и по большей части он не нужен архитектурно:
- «iframe глотает mousemove» невозможен: вебвью — нативные composition-поверхности вне gpui-дерева, а drag сплиттера обрабатывается window-level `on_mouse_move` в root, не элементами.
- «elementFromPoint проваливается к zone-контейнерам» заменён явным hit-test по probe_registry bounds (root.rs:4963-4966).
Чего реально нет: (1) глобального курсора `grabbing` на время tool-drag (курсор остаётся обычным, ClosedHand не ставится); (2) подавления hover-эффектов и тултипов на время любого drag — hover-стили элементов продолжают срабатывать под ghost'ом.

## Метрики
Порог старта drag: 4px по любой оси (tab/chip/tool — root.rs:4931,4940,4950). Спец-обработка вебвью при drag: SetBounds-доводчик через 150мс после mouse-up (root.rs:4979-4990) — наша замена «замирающему resize».

## Отличия от original.md той же папки
- `body.kamin-dragging iframe { pointer-events:none }` — не требуется (нет iframe'ов; вопрос перехвата мыши вебвью решён в слое wv_visual, отдельная зона).
- `body.kamin-dragging :where(button…) { pointer-events:none }` — НЕ ВОСПРОИЗВЕДЕНО: hover/tooltip во время drag сплиттера и тула не гасятся (возможные визуальные артефакты: подсветка кнопок под курсором при перетаскивании).
- `cursor: grabbing !important` при tool-drag — НЕ РЕАЛИЗОВАНО (gpui CursorStyle::ClosedHand доступен, не используется).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы
- цвета: N/A: цвета

### Вердикты

# 158 — verdict (review cycle 1)
VERDICT: DIVERGES — not implemented
Нет реализации в gpui-порте (design_panel.rs рендерит только Colors/Typography/
Spacing/Radius/Shadows/Components-заглушку). Детальный разбор не проводился.

## Цикл 5: MATCH (N/A)

Legacy не затащен: CSS-файлов в порте нет вообще, body-классов нечем эмулировать — снимать нечего. Побочно (поведение, не визуал): `cursor: grabbing` при драге тула не ставится, подавления ховера/тултипа во время драга нет.

---

## 159. legacy-app-shell-css — **MATCH (N/A)** (цикл 5)

*История: ц5:MATCH (N/A)*

*кадр «оригинал» отсутствует*
*кадр «наш» отсутствует*

### Оригинал

# 159 legacy-app-shell-css — оригинал
Файлы: kamin-ide/src/renderer/App.module.css:1-22

## Содержание/структура
МЁРТВЫЙ файл: классы `.app`, `.workbench`, `.center` нигде не импортируются (по INVENTORY). В порт gpui не нужен — фиксируется только для полноты.

## Метрики
Полное содержимое файла:
```css
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  background: var(--bg-primary);
  color: var(--text-primary);
  overflow: hidden;
}

.workbench {
  flex: 1;
  display: flex;
  min-height: 0;
}

.center {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
```

## Состояния/варианты
Нет. Кандидат на удаление в исходном репо; в gpui-порт не переносится.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — в `App.module.css` нет ни padding, ни margin: только display/flex-direction, height 100vh / width 100vw, flex 1, min-height 0, min-width 0, overflow hidden (App.module.css:1-22); обнуление приходит глобально из `* { margin: 0; padding: 0 }` (global.css:12)

### Наша реализация

# 159 legacy-app-shell-css — наша реализация
Файлы: —

## Структура/содержание
Мёртвый в оригинале (App.module.css: `.app`/`.workbench`/`.center` нигде не импортируются) — портировать нечего. В gpui-порте отсутствует по определению; наш каркас окна — отдельные элементы (root.rs / зона 52 app-shell).

## Метрики
—

## Отличия от original.md той же папки
Нет предмета сравнения. Статус совпадает с оригиналом: кандидат на удаление в исходном репо, в порт не входит.

## Дополнение атрибутов (цикл 10)

- цвета: файл мёртв в оригинале и в порт не переносился; собственного элемента с цветами у нас нет (каркас окна — зона 52 app-shell). N/A: цвета
- отступы: N/A: отступы

### Вердикты

# 159 legacy-app-shell-css — verdict (review cycle 1)
VERDICT: MATCH
Мёртвый файл оригинала (App.module.css не референсится) — в порте корректно
отсутствует. Делать нечего.

## Цикл 5: MATCH (N/A)

Мёртвый `App.module.css` оригинала (.app/.workbench/.center) в порт не перенесён — аналога нет и не требуется.

---
