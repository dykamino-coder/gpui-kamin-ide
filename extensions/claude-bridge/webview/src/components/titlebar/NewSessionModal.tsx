import type { JSX } from 'preact'
import { useEffect, useMemo, useState } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { tabs } from '../../signals/tabs'
import { useSavedSessions } from '../../hooks/useSavedSessions'
import { sidebarMode, activeCustomizePanel } from '../../signals/ui'
import { SessionTime } from '../sidebar/sessions/SessionTime'
import type { SavedSession } from '../../../shared/types'

/** Picker shown when the user clicks the `+` button at the end of the
 *  tabs strip (Chrome-tab-style "open new tab" affordance). The picker
 *  surfaces the two existing creation flows — start without a folder /
 *  start with an OS folder picker — at the top, then lists every folder
 *  the user has previously worked in (derived from saved sessions +
 *  currently-open tabs) so they can re-open a project with one click. */

interface ProjectGroup {
  cwd: string                  // raw cwd as stored
  key: string                  // normalized key
  displayName: string
  inactiveSessions: SavedSession[]
}

function normalizeCwd(cwd: string): string {
  return (cwd || '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase() || ''
}

function basename(p: string): string {
  const parts = p.split(/[/\\]/).filter(Boolean)
  return parts[parts.length - 1] || p
}

interface Props {
  onClose: () => void
}

export function NewSessionModal({ onClose }: Props): JSX.Element {
  const bridge = useBridge()
  const { savedSessions, loaded } = useSavedSessions()
  const [busy, setBusy] = useState(false)
  const [filter, setFilter] = useState('')

  // ESC dismisses. Focus trap is overkill for a one-shot picker; we only
  // care about the keyboard escape hatch so the user isn't stuck.
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') { e.preventDefault(); onClose() }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  // Group inactive saved sessions by folder. Skip sessions whose
  // conversationId is currently open in a tab — they're not really
  // "inactive" from the user's perspective. Active-tab cwds still earn
  // their group entry (so the user can spawn ANOTHER session there)
  // but with an empty inactive list.
  const groups: ProjectGroup[] = useMemo(() => {
    const activeConvIds = new Set<string>()
    const activeCwds = new Set<string>()
    for (const t of tabs.value) {
      if (t.conversationId) activeConvIds.add(t.conversationId)
      activeCwds.add(normalizeCwd(t.cwd))
    }
    const map = new Map<string, ProjectGroup>()
    for (const cwd of activeCwds) {
      if (!cwd) continue
      map.set(cwd, { cwd, key: cwd, displayName: basename(cwd), inactiveSessions: [] })
    }
    for (const s of savedSessions) {
      const k = normalizeCwd(s.cwd)
      if (!k) continue
      if (activeConvIds.has(s.conversationId)) continue
      let g = map.get(k)
      if (!g) {
        g = { cwd: s.cwd, key: k, displayName: basename(s.cwd), inactiveSessions: [] }
        map.set(k, g)
      }
      g.inactiveSessions.push(s)
    }
    // Sort folders alphabetically; sort sessions inside by recency.
    const out = [...map.values()]
    out.sort((a, b) => a.displayName.localeCompare(b.displayName))
    for (const g of out) g.inactiveSessions.sort((a, b) => b.lastActivity.localeCompare(a.lastActivity))
    return out
  }, [savedSessions, tabs.value])

  const filteredGroups = useMemo(() => {
    const q = filter.trim().toLowerCase()
    if (!q) return groups
    return groups.filter(g =>
      g.displayName.toLowerCase().includes(q)
      || g.cwd.toLowerCase().includes(q)
      || g.inactiveSessions.some(s => (s.label ?? '').toLowerCase().includes(q)),
    )
  }, [filter, groups])

  async function getConfig(): Promise<{ serverUrl: string; token: string } | null> {
    const cfg = await bridge.getConfig()
    if (!cfg?.serverUrl || !cfg?.token) return null
    return { serverUrl: cfg.serverUrl, token: cfg.token }
  }

  async function newNoFolder(): Promise<void> {
    if (busy) return
    setBusy(true)
    try {
      const cfg = await getConfig(); if (!cfg) return
      await bridge.createTab({ ...cfg, cwd: '' })
      onClose()
    } finally { setBusy(false) }
  }

  async function newPickFolder(): Promise<void> {
    if (busy) return
    setBusy(true)
    try {
      const folder = await bridge.selectFolder?.()
      if (!folder) return
      const cfg = await getConfig(); if (!cfg) return
      await bridge.createTab({ ...cfg, cwd: folder })
      onClose()
    } finally { setBusy(false) }
  }

  async function newInFolder(cwd: string): Promise<void> {
    if (busy) return
    setBusy(true)
    try {
      const cfg = await getConfig(); if (!cfg) return
      await bridge.createTab({ ...cfg, cwd })
      onClose()
    } finally { setBusy(false) }
  }

  async function resumeSession(s: SavedSession): Promise<void> {
    if (busy) return
    setBusy(true)
    try {
      const cfg = await getConfig(); if (!cfg) return
      await bridge.resumeSession({ ...cfg, cwd: s.cwd, conversationId: s.conversationId })
      onClose()
    } finally { setBusy(false) }
  }

  return (
    <div
      onClick={onClose}
      style="position:fixed;inset:0;z-index:9000;background:rgba(0,0,0,0.5);display:flex;align-items:flex-start;justify-content:center;padding-top:80px;-webkit-app-region:no-drag"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style="width:580px;max-width:92vw;max-height:82vh;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:14px;box-shadow:0 16px 48px rgba(0,0,0,0.5);display:flex;flex-direction:column;overflow:hidden"
      >
        {/* Header — title + two top-level create buttons + close. */}
        <div style="padding:14px 16px 12px;border-bottom:1px solid var(--bg-surface);display:flex;align-items:center;gap:8px">
          <span style="font-size:13px;font-weight:600;color:var(--text-primary);flex:1">New session</span>
          <button
            type="button"
            onClick={newNoFolder}
            disabled={busy}
            data-tooltip="Start without folder"
            style="display:inline-flex;align-items:center;gap:6px;padding:6px 12px;border:1px solid var(--bg-overlay);border-radius:8px;background:transparent;color:var(--text-primary);font-size:11px;cursor:pointer;font-weight:500"
          >
            <i class="fas fa-square-plus" style="font-size:11px" />
            <span>No folder</span>
          </button>
          <button
            type="button"
            onClick={newPickFolder}
            disabled={busy}
            data-tooltip="Pick a folder"
            style="display:inline-flex;align-items:center;gap:6px;padding:6px 12px;border:1px solid var(--accent-primary);border-radius:8px;background:color-mix(in srgb, var(--accent-primary) 16%, transparent);color:var(--accent-primary);font-size:11px;cursor:pointer;font-weight:500"
          >
            <i class="fas fa-circle-plus" style="font-size:11px" />
            <span>Pick folder…</span>
          </button>
          <button
            type="button"
            onClick={onClose}
            data-tooltip="Close (Esc)"
            style="width:28px;height:28px;display:inline-flex;align-items:center;justify-content:center;border:none;background:transparent;color:var(--text-muted);cursor:pointer;border-radius:8px"
            onMouseEnter={(e: any) => { e.currentTarget.style.background = 'var(--bg-surface)' }}
            onMouseLeave={(e: any) => { e.currentTarget.style.background = 'transparent' }}
          >
            <i class="fas fa-xmark" />
          </button>
        </div>

        {/* Filter input. */}
        <div style="padding:10px 16px;border-bottom:1px solid var(--bg-surface)">
          <input
            type="text"
            placeholder="Filter projects or sessions…"
            value={filter}
            onInput={(e) => setFilter((e.currentTarget as HTMLInputElement).value)}
            style="width:100%;box-sizing:border-box;padding:8px 12px;background:var(--bg-base);border:1px solid var(--bg-overlay);border-radius:10px;color:var(--text-primary);font-size:12px;outline:none"
          />
        </div>

        {/* Folder list. */}
        <div style="flex:1;overflow-y:auto;padding:6px 8px 12px">
          {!loaded && (
            <div style="padding:12px 16px;font-size:12px;color:var(--text-muted)">Loading…</div>
          )}
          {loaded && filteredGroups.length === 0 && (
            <div style="padding:12px 16px;font-size:12px;color:var(--text-muted)">
              {filter ? 'No matches.' : 'No project folders yet — start a new session above.'}
            </div>
          )}
          {filteredGroups.map(g => (
            <div key={g.key} style="padding:6px 8px 10px">
              <div
                style="display:flex;align-items:center;gap:10px;padding:6px 8px;border-radius:10px;font-size:var(--fs-base);color:var(--text-secondary)"
              >
                <i class="fas fa-folder" style="font-size:var(--fs-base);color:var(--text-muted)" />
                <div style="flex:1;min-width:0">
                  <div style="font-size:var(--fs-base);color:var(--text-primary);font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
                    {g.displayName}
                  </div>
                  <div style="font-size:var(--fs-xs);color:var(--text-muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
                    {g.cwd}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => newInFolder(g.cwd)}
                  disabled={busy}
                  data-tooltip="New session in this folder"
                  style="width:28px;height:28px;display:inline-flex;align-items:center;justify-content:center;border:1px solid var(--accent-primary);background:color-mix(in srgb, var(--accent-primary) 14%, transparent);color:var(--accent-primary);border-radius:8px;cursor:pointer;font-size:var(--fs-sm)"
                >
                  <i class="fas fa-plus" />
                </button>
              </div>
              {g.inactiveSessions.length > 0 && (
                <div style="margin-left:22px;margin-top:4px;border-left:1px solid var(--bg-surface);padding-left:10px">
                  {g.inactiveSessions.slice(0, 12).map(s => (
                    <button
                      key={s.conversationId}
                      type="button"
                      onClick={() => resumeSession(s)}
                      disabled={busy}
                      style="display:flex;width:100%;align-items:center;gap:8px;text-align:left;padding:4px 8px;border:none;background:transparent;color:var(--text-muted);font-size:var(--fs-md);font-weight:500;line-height:var(--lh-snug);cursor:pointer;border-radius:8px;overflow:hidden;opacity:0.7"
                      onMouseEnter={(e: any) => { e.currentTarget.style.background = 'var(--bg-surface)'; e.currentTarget.style.opacity = '1' }}
                      onMouseLeave={(e: any) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.opacity = '0.7' }}
                    >
                      <i class="far fa-clock" style="font-size:var(--fs-xs);color:var(--text-disabled);flex-shrink:0" />
                      <span style="min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{s.label || 'Untitled session'}</span>
                      <span style="flex-shrink:0;font-size:var(--fs-xs);color:var(--text-disabled);font-variant-numeric:tabular-nums">
                        <SessionTime createdAt={s.lastActivity} />
                      </span>
                      <span style="flex:1" />
                    </button>
                  ))}
                  {g.inactiveSessions.length > 12 && (
                    <div style="padding:4px 8px;font-size:var(--fs-xs);color:var(--text-disabled)">
                      +{g.inactiveSessions.length - 12} more — open the sidebar to see all
                    </div>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

/** Shortcut so titlebar callers can flip back out of customize when the
 *  modal opens (mirrors the existing TitlebarQuickActions behaviour). */
export function leaveCustomize(): void {
  if (sidebarMode.value === 'customize') {
    sidebarMode.value = 'sessions'
    activeCustomizePanel.value = null
  }
}
