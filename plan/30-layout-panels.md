# 30 — Layout: регионы, сплиттеры, ресайзы, персист

Источники: `AppLayout.tsx`, `Sidebar.tsx`, `FilePanel.tsx`, `RightPanel.tsx`, `MainBottomPanel.tsx`, `hooks/useDragHandler.ts`, `utils/centre-column-width.ts`, `signals/layout*.ts`, `signals/activity*.ts`, `src/config/constants.ts`.

Идея: НЕ VS Code-регионы, а **Bridge-style три ряда** (титлбар / body / статус-бар). GPUI-реализация — на dock-системе gpui-component + свои сплиттеры по спецификации ниже.

## Дерево композиции
```
appWrapper (колонка; фон = радиальные градиенты поверх bg-sidebar)
├─ Titlebar (42px)
├─ body (ряд; gap 8px; padding 0 4px; .bodyNoSidebar → левый gutter при скрытом сайдбаре)
│   ├─ ActivityBar slot="sidebar" (48px, вертикальная)
│   ├─ Sidebar (aside; width=sidebarWidth)                ← ресайз правой кромкой
│   ├─ mainColumn (flex 1; minWidth 100)                  [показан если inCustomize || mainVisible]
│   │    ├─ MainContent («Left», height=mainSplit%)
│   │    └─ MainBottomPanel («Left Bottom», 1-mainSplit%) ← row-ресайз ручкой сверху (10px)
│   ├─ FilePanel (aside «File»; width=filePanelWidth)     ← ресайз левой кромкой + внутр. split
│   └─ RightPanel («Right»; width=rightPanelWidth)        ← ресайз левой кромкой + внутр. split
├─ StatusBar (24px)
├─ Toasts, CommandPalette
```

## Правила показа
- **Customize-режим**: рендерятся ТОЛЬКО sidebar+main (File/Right/MainBottom пропущены, их сигналы видимости не трогаются).
- **Нет открытых сессий**: main = WelcomePlaceholder на всю высоту; File/Right/MainBottom скрыты.
- **Ровно один flex-филлер**: main, если он есть; иначе File (fileFills); иначе Right (rightFills). Филлер = flex:1 и теряет свою width-ручку.
- Видимости (меню LayoutToggles; child-чекбокс disabled без родителя):
  - Left = mainVisible (default true); Left Bottom = mainBottomVisible (требует Left)
  - File = filePanelVisible (false); Center Bottom = filePanelBottomVisible (требует File)
  - Right = rightPanelVisible (false); Right Bottom = rightPanelBottomVisible (требует Right)
  - Sidebar = sidebarVisible (титлбар-кнопка / Ctrl+B)

## Сплиттеры (все — общий drag-механизм)
useDragHandler-эквивалент: document-level move/up, курсор на body, userSelect off, режим «kamin-dragging» ГАСИТ pointer-events вебвью-оверлеев (в GPUI: во время драга скрывать/замораживать wry-оверлеи или ставить прозрачный щит). Во время драга редактор релэйаутится немедленно (без мерцания минимапы).

| Сплиттер | Кромка | Мин/границы | Семантика |
|---|---|---|---|
| Sidebar | правая | min 100px; клампится против MAIN_MIN 100 | абсолютные px |
| FilePanel width | левая | min 100px | торгует место с main |
| FilePanel inner split | row-ручка | низ min 100px | ratio высоты нижней карты |
| RightPanel width | левая | min 100px | если File виден — ТОРГУЕТ File↔Right (центр не трогается); иначе против центра |
| RightPanel split | row | ratio 0.15–0.85, default 0.55 | верх/низ карты |
| Main split | row-ручка MainBottom | ratio 0.2–0.85, default 0.7 | Left / Left Bottom |

clampGrowth защищает минимум центра при любом росте боковых.

## Активити-модель (7 слотов)
Слоты: sidebar, main, mainBottom, centralTop, centralBottom, rightTop, rightBottom. Каждый: {pinned: string[], active: string|null}.
- ActivityBar (вертикальная, для sidebar/rightTop/rightBottom): клик = активировать; правый клик = контекст-меню (Hide/unpin + «Move to ▸» с PanelIcon-иконками остальных слотов); pointer-drag = перенос между зонами (ActivityDragGhost следует за курсором); «…»-пикер в конце; у sidebar сверху фикс-тайл Customize-шестерёнка. align top/bottom зеркалит порядок (низ у rightBottom).
- BottomTabBar (горизонтальные табы, для main/mainBottom/centralBottom): те же данные как подписанные табы + пикер «Open Tool ▾».
- ActivityPicker: портал-поповер (clampToViewport) pin/unpin активностей в pinned[] слота.
- Встроенные активности: projects→SessionsMode, tree→FileTreeView, terminal→TerminalView(slot), extensions→ExtensionsPanel, problems→ProblemsPanel; contributed-контейнеры→ContributedContainerBody; иначе ActivityPlaceholder.

## Числовые константы (src/config/constants.ts — источник истины)
- Мин ширины: все 100px (sidebar/file/right/main/bottom-pane); max нет
- Дефолты: sidebar 270, file 360 (ratio 0.42), file-bottom 180 (ratio 0.3), right 280
- Сплиты: main 0.7 (0.2–0.85), right 0.55 (0.15–0.85)
- Ratio-клампы File-панели: FILE_PANEL_RATIO 0.05–0.6 (ширина), BOTTOM_PANE_RATIO 0.1–0.8 (нижняя карта) — рабочие границы драга (constants.ts:85-86, 94-95)
- ⚠ Рассинхрон в коде (не баг плана): CSS-токен сайдбара 280/min 200 (layout-tokens.css) vs рантайм-дефолт 270/min 100 (constants.ts) — рантайм авторитетен; при порте взять constants.ts
- Окно 1400×900, min 800×600; palette 50 строк; toast 4s

## Персист (детали схемы в 50-state-ipc)
- layout.json: sidebar/right — абсолютные px; file — ratio от вьюпорта; splits — ratio; per-slot activity {pinned,active}
- Debounce 250ms, первый эмит пропускается; viewport-adapter на resize окна (file из ratio; sidebar+right масштабируются фактором окна; неправдоподобно малые размеры (minimize) игнорируются; settle 80ms)
- Пресеты layout (LS) + пер-сессионный layout (host sessions.json)

## Чеклист паритета (layout)
- [ ] Три ряда + порядок регионов + gap/gutter 8/4px
- [ ] Правила показа (customize / no-sessions / один филлер)
- [ ] 6 сплиттеров с точными мин/границами/семантикой торговли местом
- [ ] Драг: глобальный курсор, freeze вебвью-оверлеев, мгновенный релэйаут редактора
- [ ] 7 активити-слотов: pinned/active, drag-перенос, контекст-меню, пикеры, зеркальный align
- [ ] Все константы из constants.ts
- [ ] Персист + viewport-adapter + пресеты + пер-сессионный layout
