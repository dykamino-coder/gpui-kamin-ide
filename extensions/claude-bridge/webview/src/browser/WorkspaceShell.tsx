import { useEffect, useRef, useState } from 'preact/hooks'
import kaminoid from '../assets/kaminoid.svg'
import { ChatRoot } from '../roots/ChatRoot'
import { TerminalPanel } from '../components/terminal/TerminalPanel'
import { AgentsPanel } from './shell/AgentsPanel'
import { CommandPalette, type WorkspaceCommand } from './shell/CommandPalette'
import { tabs, activeTabId } from '../signals/tabs'
import { togglePinTab } from '../lib/tabs-persist'
import { activateSession, closeSession, newSession, renameSession } from './shell/session-actions'
import { CustomizeContentPanel } from '../components/customize/CustomizeContentPanel'
import { SECTIONS } from '../components/customize/sections'
import { toasts, dismissToast } from '../signals/toasts'
import { sidebarMode, activeCustomizePanel } from '../signals/ui'
import { resolvedTheme, setTheme } from '../theme/apply-theme'
import { SessionSidebar } from './shell/SessionSidebar'
import { SessionTabs } from './shell/SessionTabs'
import { PanelDock } from './shell/PanelDock'
import { connectShell, shellError, shellState, updateShell } from './shell/state'
import './shell/browser-theme.css'
import './shell/workspace-shell.css'

/** Browser frame around the current tool roots; workspace host owns layout. */
export function WorkspaceShell() {
  useEffect(connectShell, [])
  const frame = useRef<HTMLDivElement>(null)
  const [more, setMore] = useState(false)
  const settings = sidebarMode.value === 'customize'
  const state = shellState.value
  const showSettings = () => { sidebarMode.value = 'customize'; activeCustomizePanel.value = 'settings' }
  function showPanel(panel: 'chat' | 'console' | 'agents') {
    sidebarMode.value = 'sessions'; activeCustomizePanel.value = null
    updateShell({ type: 'activate', panel })
  }
  const restoreLayout = () => {
    updateShell({ type: 'move', panel: 'chat', slot: 'main' })
    updateShell({ type: 'move', panel: 'console', slot: 'bottom' })
    updateShell({ type: 'move', panel: 'agents', slot: 'right' })
  }
  const toggleTheme = () => setTheme(resolvedTheme.value === 'dark' ? 'light' : 'dark')
  const commands: WorkspaceCommand[] = [
    { id: 'session.new', title: 'New session', category: 'Sessions', run: () => newSession(false) },
    { id: 'session.folder', title: 'New session (folder…)', category: 'Sessions', run: () => newSession(true) },
    { id: 'settings', title: 'Open settings', run: showSettings },
    { id: 'sidebar.toggle', title: 'Toggle sessions sidebar', run: () => updateShell({ type: 'sidebar', visible: !state.sidebarVisible }) },
    { id: 'layout.restore', title: 'Restore panel layout', run: restoreLayout },
    { id: 'theme.toggle', title: 'Toggle theme', run: toggleTheme },
    ...(['chat', 'console', 'agents'] as const).map(panel => ({ id: 'panel.' + panel,
      title: 'Show ' + ({ chat: 'Claude Bridge', console: 'Bridge Console', agents: 'Agents' })[panel], category: 'Panels', run: () => showPanel(panel) })),
    ...tabs.value.map(tab => ({ id: 'session.switch.' + tab.id, title: tab.sessionTitle || tab.label || tab.folderName || 'Session', category: 'Switch session', run: () => activateSession(tab.id) })),
    ...(activeTabId.value ? [
      { id: 'session.pin', title: 'Toggle session pin', category: 'Sessions', run: () => togglePinTab(activeTabId.value!) },
      { id: 'session.rename', title: 'Rename session', category: 'Sessions', run: () => renameSession(activeTabId.value!) },
      { id: 'session.close', title: 'Close session', category: 'Sessions', run: () => closeSession(activeTabId.value!) },
    ] : []),
  ]
  function resizeSidebar(e: PointerEvent) {
    const el = e.currentTarget as HTMLElement
    el.setPointerCapture(e.pointerId)
    let width = state.sidebarWidth
    const left = frame.current!.getBoundingClientRect().left + 48
    const move = (event: PointerEvent) => { width = Math.max(200, Math.min(600, event.clientX - left)); frame.current?.style.setProperty('--ws-sidebar-width', `${width}px`) }
    const end = () => { el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', end); el.removeEventListener('pointercancel', end); updateShell({ type: 'resize', sidebarWidth: width }) }
    el.addEventListener('pointermove', move); el.addEventListener('pointerup', end); el.addEventListener('pointercancel', end)
  }
  return <div id="app-wrapper" class="ws-shell" ref={frame} style={{ '--ws-sidebar-width': `${state.sidebarWidth}px` }}>
    <div class="ws-notices" aria-live="polite">{toasts.value.map(notice => <div role="status" key={notice.id}><strong>{notice.title}</strong><p>{notice.message}</p><button aria-label="Dismiss notification" onClick={() => dismissToast(notice.id)}>×</button></div>)}</div>
    <header class="ws-topbar">
      <div class="ws-logo" title="KaminIDE"><img src={kaminoid} alt="KaminIDE" width={26} height={26} /></div>
      <button class="ws-icon" aria-label="Toggle sessions sidebar" onClick={() => updateShell({ type: 'sidebar', visible: !state.sidebarVisible })}><i class="fas fa-table-columns" /></button>
      <SessionTabs />
      <button class="ws-command-search" onClick={() => window.dispatchEvent(new CustomEvent('workspace:command-palette'))}><i class="fas fa-magnifying-glass" />Type a command…<kbd>Ctrl Shift P</kbd></button>
      <button class="ws-icon" title="Restore panel layout" aria-label="Restore panel layout" onClick={restoreLayout}><i class="fas fa-border-all" /></button>
      <button class="ws-icon" title="Toggle theme" aria-label="Toggle theme" onClick={toggleTheme}><i class={'fas ' + (resolvedTheme.value === 'dark' ? 'fa-moon' : 'fa-sun')} /></button>
    </header>
    <CommandPalette commands={commands} />
    <div class="ws-body">
      <nav class="ws-rail" aria-label="Workspace navigation">
        <button class={'ws-icon' + (settings ? ' selected' : '')} aria-label="Customize" title="Settings" onClick={showSettings}><i class="fas fa-gear" /></button>
        <button class={'ws-icon' + (!settings ? ' selected' : '')} aria-label="Sessions and projects" title="Sessions and projects" onClick={() => { sidebarMode.value = 'sessions'; activeCustomizePanel.value = null; if (!state.sidebarVisible) updateShell({ type: 'sidebar', visible: true }) }}><i class="fas fa-layer-group" /></button>
        <button class="ws-icon" aria-label="More workspace actions" onClick={() => setMore(!more)}><i class="fas fa-ellipsis" /></button>
        {more && <div class="ws-more-menu"><button onClick={() => { window.dispatchEvent(new CustomEvent('workspace:command-palette')); setMore(false) }}>Commands</button><button onClick={() => { showSettings(); setMore(false) }}>Settings</button></div>}
      </nav>
      {state.sidebarVisible && <div class="ws-sidebar-frame">
        <div hidden={settings}><SessionSidebar /></div>
        {settings && <aside class="ws-settings-nav"><button onClick={() => { sidebarMode.value = 'sessions'; activeCustomizePanel.value = null }}>‹ Sessions</button>
          {SECTIONS.map(section => <button id={'sidebar-' + section.panel + '-item'} class={activeCustomizePanel.value === section.panel ? 'selected' : ''}
            onClick={() => { activeCustomizePanel.value = section.panel }}><i class={section.icon} />{section.label}</button>)}
        </aside>}
        <div class="ws-sidebar-resize" role="separator" aria-label="Resize sessions sidebar" aria-orientation="vertical" tabIndex={0} onPointerDown={resizeSidebar}
          onKeyDown={e => { if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') { e.preventDefault(); updateShell({ type: 'resize', sidebarWidth: Math.max(200, Math.min(600, state.sidebarWidth + (e.key === 'ArrowRight' ? 20 : -20))) }) } }} />
      </div>}
      <main class="ws-main">
        {shellError.value && <div class="ws-layout-error" role="status">{shellError.value}</div>}
        <div class="ws-work-area" hidden={settings}><PanelDock>{{ chat: <ChatRoot />, console: <TerminalPanel visible showResizeHandle={false} fill />, agents: <div class="ws-agents"><AgentsPanel /></div> }}</PanelDock></div>
        {settings && <div class="ws-settings-content"><CustomizeContentPanel /></div>}
      </main>
      <nav class="ws-right-rail" aria-label="Panels">
        <button class="ws-icon" title="Claude Bridge" aria-label="Show Claude Bridge" onClick={() => showPanel('chat')}><i class="far fa-comment" /></button>
        <button class="ws-icon" title="Bridge Console" aria-label="Show Bridge Console" onClick={() => showPanel('console')}><i class="fas fa-terminal" /></button>
        <button class="ws-icon" title="Agents" aria-label="Show Agents" onClick={() => showPanel('agents')}><i class="fas fa-users" /></button>
      </nav>
    </div>
  </div>
}
