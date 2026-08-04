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
