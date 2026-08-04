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
