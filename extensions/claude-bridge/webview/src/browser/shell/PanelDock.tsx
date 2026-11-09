import type { ComponentChildren } from 'preact'
import { useRef, useState } from 'preact/hooks'
import type { Panel, Slot } from '../../../shared/workspace-shell'
import { shellState, updateShell } from './state'

const titles: Record<Panel, string> = { chat: 'Claude Bridge', console: 'Bridge Console', agents: 'AGENTS' }
const icons: Record<Panel, string> = { chat: 'fa-comment', console: 'fa-terminal', agents: 'fa-users' }
const slots: Slot[] = ['main', 'bottom', 'right']

/** One mounted instance per tool: moving panels never remounts the chat listeners. */
export function PanelDock({ children }: { children: Record<Panel, ComponentChildren> }) {
  const state = shellState.value
  const ref = useRef<HTMLDivElement>(null)
  const [menu, setMenu] = useState<Slot | null>(null)
  const [dragging, setDragging] = useState<Panel | null>(null)
  const hasRight = state.panels.right.length > 0
  const hasBottom = state.panels.bottom.length > 0
  function resize(e: PointerEvent, axis: 'bottom' | 'right') {
    const element = e.currentTarget as HTMLElement
    const rect = ref.current!.getBoundingClientRect()
    element.setPointerCapture(e.pointerId)
    let value = axis === 'bottom' ? state.mainRatio : state.rightWidth
    const move = (event: PointerEvent) => {
      value = axis === 'bottom' ? Math.max(.2, Math.min(.85, (event.clientY - rect.top) / rect.height)) : Math.max(180, Math.min(700, rect.right - event.clientX))
      ref.current?.style.setProperty(axis === 'bottom' ? '--ws-main-ratio' : '--ws-right-width', axis === 'bottom' ? `${value}fr` : `${value}px`)
      if (axis === 'bottom') ref.current?.style.setProperty('--ws-bottom-ratio', `${1 - value}fr`)
    }
    const end = () => { element.removeEventListener('pointermove', move); element.removeEventListener('pointerup', end); element.removeEventListener('pointercancel', end)
      updateShell({ type: 'resize', ...(axis === 'bottom' ? { mainRatio: value } : { rightWidth: value }) }) }
    element.addEventListener('pointermove', move); element.addEventListener('pointerup', end); element.addEventListener('pointercancel', end)
  }
  const panelSlot = (panel: Panel) => slots.find(slot => state.panels[slot].includes(panel))!
  return <div class={'ws-dock' + (hasRight ? ' has-right' : '') + (hasBottom ? ' has-bottom' : '') + (dragging ? ' docking' : '')} ref={ref}
    style={{ '--ws-main-ratio': `${state.mainRatio}fr`, '--ws-bottom-ratio': `${1 - state.mainRatio}fr`, '--ws-right-width': `${state.rightWidth}px` }}>
    {slots.map(slot => <div class={'ws-slot-header ws-slot-' + slot} key={slot}>
      <div class="ws-panel-tabs" role="tablist" aria-label={slot + ' panels'}>{state.panels[slot].map(panel =>
        <button class={'ws-panel-tab' + (state.active[slot] === panel ? ' selected' : '')} role="tab" aria-selected={state.active[slot] === panel}
          draggable onDragStart={e => { setDragging(panel); e.dataTransfer?.setData('application/x-workspace-panel', panel) }} onDragEnd={() => setDragging(null)}
          onClick={() => updateShell({ type: 'activate', panel })}><i class={'fas ' + icons[panel]} />{titles[panel]}</button>)}</div>
      <button class="ws-icon" aria-label={slot + ' panel menu'} onClick={() => setMenu(menu === slot ? null : slot)}><i class="fas fa-ellipsis" /></button>
      {menu === slot && <div class="ws-dock-menu" onKeyDown={e => { if (e.key === 'Escape') setMenu(null) }}>
        {state.panels[slot].map(panel => <div key={panel}><strong>{titles[panel]}</strong>{slots.filter(s => s !== slot).map(target =>
          <button onClick={() => { updateShell({ type: 'move', panel, slot: target }); setMenu(null) }}>Move to {target}</button>)}</div>)}
      </div>}
    </div>)}
    {(['chat', 'console', 'agents'] as Panel[]).map(panel => {
      const slot = panelSlot(panel)
      return <section key={panel} aria-label={titles[panel]} class={'ws-panel-content ws-content-' + slot} hidden={state.active[slot] !== panel}>{children[panel]}</section>
    })}
    {hasBottom && <div class="ws-dock-split ws-bottom-split" role="separator" aria-label="Resize console area" aria-orientation="horizontal" tabIndex={0}
      onPointerDown={e => resize(e, 'bottom')} onKeyDown={e => { if (e.key === 'ArrowUp' || e.key === 'ArrowDown') { e.preventDefault(); updateShell({ type: 'resize', mainRatio: Math.max(.2, Math.min(.85, state.mainRatio + (e.key === 'ArrowDown' ? .02 : -.02))) }) } }} />}
    {hasRight && <div class="ws-dock-split ws-right-split" role="separator" aria-label="Resize agents area" aria-orientation="vertical" tabIndex={0}
      onPointerDown={e => resize(e, 'right')} onKeyDown={e => { if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') { e.preventDefault(); updateShell({ type: 'resize', rightWidth: Math.max(180, Math.min(700, state.rightWidth + (e.key === 'ArrowLeft' ? 20 : -20))) }) } }} />}
    {dragging && slots.map(slot => <div class={'ws-drop-target ws-drop-' + slot}
      onDragOver={e => { e.preventDefault(); if (e.dataTransfer) e.dataTransfer.dropEffect = 'move' }}
      onDrop={e => { e.preventDefault(); e.stopPropagation(); updateShell({ type: 'move', panel: dragging, slot }); setDragging(null) }}>Move to {slot}</div>)}
  </div>
}
