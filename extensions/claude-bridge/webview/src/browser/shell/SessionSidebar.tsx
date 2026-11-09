import { useSavedSessions } from '../../hooks/useSavedSessions'
import { activeTabId, tabs, pinnedTabs } from '../../signals/tabs'
import { togglePinTab } from '../../lib/tabs-persist'
import { shellState, updateShell } from './state'
import { activateSession, newSession, renameSession, resumeSession } from './session-actions'

function SessionAge({ createdAt }: { createdAt: string }) {
  const minutes = Math.max(0, Math.floor((Date.now() - Date.parse(createdAt)) / 60000))
  const label = !Number.isFinite(minutes) ? '' : minutes < 1 ? 'now' : minutes < 60 ? `${minutes}m` : minutes < 1440 ? `${Math.floor(minutes / 60)}h` : `${Math.floor(minutes / 1440)}d`
  return <span class="session-time" title={createdAt}>{label}</span>
}

export function SessionSidebar() {
  const { savedSessions } = useSavedSessions()
  const live = tabs.value
  const opened = new Set(live.map(t => t.conversationId).filter(Boolean))
  const saved = savedSessions.filter(s => !opened.has(s.conversationId))
  const projects = [...new Set([...live.map(t => t.cwd || ''), ...saved.map(s => s.cwd || '')])]
    .sort((a, b) => !a ? -1 : !b ? 1 : Number(live.some(t => t.cwd === b)) - Number(live.some(t => t.cwd === a)) || a.localeCompare(b))
  return <aside class="ws-sidebar" aria-label="Sessions and projects">
    <div class="ws-session-actions">
      <button onClick={() => void newSession(true)}><i class="fas fa-folder-plus" />New session (folder…)</button>
      <button onClick={() => void newSession(false)}><i class="fas fa-plus" />No folder session</button>
    </div>
    <div class="ws-section-label">PROJECTS</div>
    <div class="ws-projects">
      {!projects.length && <p class="ws-empty">No projects yet. Open a folder or start a session.</p>}
      {projects.map(cwd => {
        const sessions = live.filter(t => (t.cwd || '') === cwd)
        const history = saved.filter(s => (s.cwd || '') === cwd).sort((a, b) => b.lastActivity.localeCompare(a.lastActivity))
        const expanded = shellState.value.expandedProjects.includes(cwd)
        return <section key={cwd} class="ws-project">
          <button class="ws-project-header" title={cwd || 'No folder'} aria-expanded={expanded}
            onClick={() => updateShell({ type: 'project', cwd, expanded: !expanded })}>
            <i class={'fas ' + (expanded ? 'fa-chevron-down' : 'fa-chevron-right')} />
            <i class="far fa-folder" /><span>{cwd.split(/[\\/]/).filter(Boolean).pop() || 'No folder'}</span>
            <small>{sessions.length + history.length}</small>
          </button>
          {sessions.map(tab => <div class={'ws-session-row' + (activeTabId.value === tab.id ? ' selected' : '')} key={tab.id}>
            <button class="ws-session-open" onClick={() => activateSession(tab.id)} onDblClick={() => renameSession(tab.id)} title={tab.sessionTitle || tab.label}>
              <i class={'ws-status ' + (tab.status === 'connected' ? 'connected' : '')} />
              <span>{tab.sessionTitle || tab.label || 'Session'}</span><SessionAge createdAt={tab.createdAt} />
            </button>
            <button class={'ws-row-pin' + (pinnedTabs.value.has(tab.id) ? ' pinned' : '')} aria-label={pinnedTabs.value.has(tab.id) ? 'Unpin session' : 'Pin session'}
              onClick={() => togglePinTab(tab.id)}><i class="fas fa-thumbtack" /></button>
          </div>)}
          {expanded && history.map(session => <button class="ws-session-row ws-saved" key={session.conversationId}
            title={session.label} onClick={() => void resumeSession(session.conversationId, cwd, session.label)}>
            <i class="ws-status" /><span>{session.label || new Date(session.lastActivity).toLocaleDateString('en-CA')}</span>
            <SessionAge createdAt={session.lastActivity} />
          </button>)}
        </section>
      })}
    </div>
  </aside>
}
