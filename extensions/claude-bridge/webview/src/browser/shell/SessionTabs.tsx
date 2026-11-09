import { activeTabId, tabs, pinnedTabs, topTabsOrder, ghostPinnedTabs } from '../../signals/tabs'
import { useState } from 'preact/hooks'
import { togglePinTab, moveTopTab, scheduleSaveTabsState } from '../../lib/tabs-persist'
import { activateSession, closeSession, newSession, renameSession, wakeSession } from './session-actions'

export function SessionTabs() {
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const order = topTabsOrder.value
  const list = [...tabs.value].sort((a, b) => {
    const ai = order.indexOf(a.id), bi = order.indexOf(b.id)
    return (ai < 0 ? 100000 : ai) - (bi < 0 ? 100000 : bi) || a.createdAt.localeCompare(b.createdAt)
  })
  return <div class="ws-session-tabs" role="tablist" aria-label="Open sessions">
    {ghostPinnedTabs.value.map(g => <button class="ws-session-tab sleeping" role="tab" aria-selected="false" key={'ghost-' + g.id}
      title={g.cwd} onClick={() => void wakeSession(g)}
      onContextMenu={e => { e.preventDefault(); ghostPinnedTabs.value = ghostPinnedTabs.value.filter(p => p.id !== g.id); scheduleSaveTabsState() }}>
      <i class="fas fa-thumbtack" /><span>{g.title || g.cwd.split('/').pop() || 'Session'}</span>
    </button>)}
    {list.map(tab => <div key={tab.id} class={'ws-session-tab' + (activeTabId.value === tab.id ? ' selected' : '')}
      role="tab" aria-selected={activeTabId.value === tab.id} tabIndex={0} data-tab-id={tab.id} title={tab.cwd}
      draggable onDragStart={e => { e.dataTransfer?.setData('application/x-workspace-session', tab.id) }}
      onDragOver={e => { if (e.dataTransfer?.types.includes('application/x-workspace-session')) e.preventDefault() }}
      onDrop={e => { e.preventDefault(); const id = e.dataTransfer?.getData('application/x-workspace-session'); if (id) moveTopTab(id, tab.id) }}
      onClick={() => activateSession(tab.id)} onDblClick={() => renameSession(tab.id)}
      onContextMenu={e => { e.preventDefault(); setMenu({ id: tab.id, x: e.clientX, y: e.clientY }) }}
      onKeyDown={e => { if (e.target === e.currentTarget && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); activateSession(tab.id) } }}>
      <button class="ws-pin" aria-label={pinnedTabs.value.has(tab.id) ? 'Unpin tab' : 'Pin tab'}
        onClick={e => { e.stopPropagation(); togglePinTab(tab.id) }}><i class={'fas ' + (pinnedTabs.value.has(tab.id) ? 'fa-thumbtack' : 'fa-circle')} /></button>
      <span>{tab.sessionTitle || tab.label || tab.folderName || 'Session'}</span>
      <button aria-label="Close tab" onClick={e => { e.stopPropagation(); closeSession(tab.id) }}><i class="fas fa-xmark" /></button>
    </div>)}
    <button class="ws-icon ws-new-tab" aria-label="New session" title="New session" onClick={() => void newSession(false)}><i class="fas fa-plus" /></button>
    {menu && <div class="ws-session-menu-backdrop" onClick={() => setMenu(null)} onContextMenu={e => { e.preventDefault(); setMenu(null) }}>
      <div class="ws-session-menu" role="menu" style={{ left: Math.min(menu.x, window.innerWidth - 180), top: menu.y }} onKeyDown={e => { if (e.key === 'Escape') setMenu(null) }}>
        <button role="menuitem" autoFocus onClick={() => { renameSession(menu.id); setMenu(null) }}>Rename session</button>
        <button role="menuitem" onClick={() => { togglePinTab(menu.id); setMenu(null) }}>{pinnedTabs.value.has(menu.id) ? 'Unpin session' : 'Pin session'}</button>
        <button role="menuitem" onClick={() => { closeSession(menu.id); setMenu(null) }}>Close session</button>
      </div>
    </div>}
  </div>
}
