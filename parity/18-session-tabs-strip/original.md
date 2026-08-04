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
