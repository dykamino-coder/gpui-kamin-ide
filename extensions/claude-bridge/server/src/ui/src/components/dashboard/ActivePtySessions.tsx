import { Fragment } from 'preact'
import { useState } from 'preact/compat'
import { Card, SectionHeader } from '../shared'
import { api } from '../../services/api-client'
import type { PtySessionInfo } from '../../signals/server'
import { EngineBadge, asEngine } from './engine'
import { groupPtySessions, type SessionSort } from './pty-groups'
import styles from './ActivePtySessions.module.css'

const STORAGE_KEY = 'bridge.pty-groups.v2'
function readPreferences(): { expanded: string[]; sort: SessionSort } {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}')
    return {
      expanded: Array.isArray(saved.expanded) ? saved.expanded.filter((s: unknown) => typeof s === 'string') : [],
      sort: saved.sort === 'count' ? 'count' : 'activity',
    }
  } catch {
    return { expanded: [], sort: 'activity' }
  }
}
const compact = (n: number) =>
  n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}K` : String(n)
const memory = (n: number | null | undefined) =>
  typeof n !== 'number' ? '—' : n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(1)}G` : `${Math.round(n / 1024 ** 2)}M`
const cpu = (n: number | null | undefined) => (typeof n === 'number' ? `${Math.round(n * 10) / 10}%` : '—')
function elapsed(at: string | number) {
  const sec = Math.max(0, Math.round((Date.now() - (typeof at === 'number' ? at : Date.parse(at))) / 1000))
  return sec < 60
    ? `${sec}s`
    : sec < 3600
      ? `${Math.floor(sec / 60)}m ${sec % 60}s`
      : `${Math.floor(sec / 3600)}h ${Math.floor((sec % 3600) / 60)}m`
}

export function ActivePtySessions({ sessions }: { sessions: PtySessionInfo[] }) {
  const [prefs, setPrefs] = useState(readPreferences)
  const [error, setError] = useState<string | null>(null)
  const groups = groupPtySessions(sessions, prefs.sort)
  const save = (next: typeof prefs) => {
    setPrefs(next)
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
    } catch {
      /* Storage can be disabled. */
    }
  }
  const toggle = (name: string) =>
    save({
      ...prefs,
      expanded: prefs.expanded.includes(name) ? prefs.expanded.filter((n) => n !== name) : [...prefs.expanded, name],
    })
  const kill = async (s: PtySessionInfo) => {
    if (!confirm(`Kill session "${s.sessionTitle || s.id}"?\nThis terminates its CLI process on the server.`)) return
    setError(null)
    try {
      await api.killSession(s.id)
    } catch (e) {
      setError(`Could not kill session: ${e instanceof Error ? e.message : String(e)}`)
    }
  }
  return (
    <Card>
      <div class={styles.header}>
        <SectionHeader
          title="Active PTY Sessions"
          icon="fa-solid fa-terminal"
          subtitle={`${sessions.length} active · ${groups.length} users`}
        />
        <label>
          Sort{' '}
          <select
            aria-label="Sort session groups"
            value={prefs.sort}
            onChange={(e) => save({ ...prefs, sort: e.currentTarget.value as SessionSort })}
          >
            <option value="activity">Activity</option>
            <option value="count">Session count</option>
          </select>
        </label>
      </div>
      {error && (
        <div class={styles.error} role="alert">
          {error}
        </div>
      )}
      {sessions.length === 0 ? (
        <div class={styles.empty}>No active sessions</div>
      ) : (
        <div class={styles.scroll}>
          <table class={styles.table}>
            <thead>
              <tr>
                <th>User / Session</th>
                <th>Folder</th>
                <th>State / MCP</th>
                <th>User</th>
                <th>Asst</th>
                <th>MCP</th>
                <th title="Sum of current context windows">Context</th>
                <th title="Cumulative token throughput">Total</th>
                <th>CPU</th>
                <th>Mem</th>
                <th>Duration</th>
                <th>Idle</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {groups.map((g) => (
                <Fragment key={g.key}>
                  <tr class={styles.group} data-user={g.name}>
                    <td colSpan={3}>
                      <button
                        type="button"
                        class={styles.toggle}
                        aria-expanded={prefs.expanded.includes(g.key)}
                        onClick={() => toggle(g.key)}
                      >
                        <span aria-hidden="true">{prefs.expanded.includes(g.key) ? '▾' : '▸'}</span> {g.name}{' '}
                        <span class={styles.badge}>{g.sessions.length}</span>
                      </button>
                    </td>
                    <td>{g.userMessages}</td>
                    <td>{g.assistantMessages}</td>
                    <td>{g.mcpCallCount}</td>
                    <td>{compact(g.contextTokens)}</td>
                    <td>{compact(g.totalTokens)}</td>
                    <td>{cpu(g.cpuPercent)}</td>
                    <td>{memory(g.memBytes)}</td>
                    <td>—</td>
                    <td>{elapsed(g.lastActivity)}</td>
                    <td />
                  </tr>
                  {prefs.expanded.includes(g.key) &&
                    g.sessions.map((s) => (
                      <tr key={s.id} data-session={s.id}>
                        <td class={styles.title} title={s.sessionTitle || s.id}>
                          <div>{s.sessionTitle || s.id.slice(0, 8)}</div>
                          <small>
                            {s.id.slice(0, 8)} <EngineBadge engine={asEngine(s.engine)} />
                          </small>
                        </td>
                        <td class={styles.folder} title={s.cwd}>
                          {s.cwd?.split(/[/\\]/).pop() || '—'}
                        </td>
                        <td title={s.mcpLastError || ''}>
                          <div>{s.state}</div>
                          <small>MCP: {s.mcpInitialized ? 'OK' : s.mcpLastError ? 'Error' : 'Pending'}</small>
                          <small class={s.attached === false ? styles.detached : styles.attached}>
                            {s.attached === false
                              ? `⚠ detached${s.detachedForSec != null ? ` · ${Math.round(s.detachedForSec)}s` : ''}`
                              : '● client'}
                          </small>
                        </td>
                        <td>{s.userMessages ?? 0}</td>
                        <td>{s.assistantMessages ?? 0}</td>
                        <td>{s.mcpCallCount}</td>
                        <td>{compact(s.contextTokens ?? 0)}</td>
                        <td>{compact(s.totalTokens ?? 0)}</td>
                        <td>{cpu(s.cpuPercent)}</td>
                        <td>{memory(s.memBytes)}</td>
                        <td>{elapsed(s.createdAt)}</td>
                        <td>{elapsed(s.lastActivityAt)}</td>
                        <td>
                          <button
                            type="button"
                            class={styles.kill}
                            onClick={() => void kill(s)}
                            aria-label={`Kill session ${s.sessionTitle || s.id}`}
                          >
                            Kill
                          </button>
                        </td>
                      </tr>
                    ))}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  )
}
