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
