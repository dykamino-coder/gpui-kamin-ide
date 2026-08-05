// Dashboard Stats card — same layout as the legacy Electron client's StatsPanel but
// re-implemented for the web dashboard's React/Preact compat tree.
// Reads /api/dashboard/stats/overview and /api/dashboard/tokens/:id/sessions.

import { useEffect, useMemo, useState } from 'preact/compat'
import { Card, SectionHeader } from '../shared'

type Range = 'all' | '30d' | '7d'

interface HourlyBucket {
  utcHour: string
  model: string | null
  userMessages: number
  tokens: number
}

interface OverviewRaw {
  range: Range
  sessions: number
  userMessages: number
  assistantMessages: number
  totalTokens: number
  favoriteModel: string | null
  models: Array<{ name: string; in: number; out: number; pct: number }>
  hourly: HourlyBucket[]
}

// Local-time aggregation — heatmap, peakHour, longestStreak, activeDays,
// modelsTimeseries are derived from raw UTC hourly buckets in the user's
// browser locale. Server never sees a timezone.
function fmtLocalDay(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}
interface HourlyBucketEx extends HourlyBucket {
  sessionId?: string
  assistantMessages?: number
}
function aggregateLocal(hourly: HourlyBucketEx[]) {
  interface DayAcc {
    date: string
    userMessages: number
    assistantMessages: number
    tokens: number
    sessionSet: Set<string>
    models: Record<string, number>
  }
  const byDay = new Map<string, DayAcc>()
  const userByHour = new Array<number>(24).fill(0)
  for (const b of hourly) {
    const d = new Date(b.utcHour + ':00:00.000Z')
    const day = fmtLocalDay(d)
    const hour = d.getHours()
    userByHour[hour] += b.userMessages
    let slot = byDay.get(day)
    if (!slot) {
      slot = { date: day, userMessages: 0, assistantMessages: 0, tokens: 0, sessionSet: new Set(), models: {} }
      byDay.set(day, slot)
    }
    slot.userMessages += b.userMessages
    slot.assistantMessages += b.assistantMessages ?? 0
    slot.tokens += b.tokens
    if (b.sessionId) slot.sessionSet.add(b.sessionId)
    if (b.model && b.tokens > 0) slot.models[b.model] = (slot.models[b.model] ?? 0) + b.tokens
  }
  const heatmap = Array.from(byDay.values())
    .map(d => ({
      date: d.date,
      userMessages: d.userMessages,
      assistantMessages: d.assistantMessages,
      tokens: d.tokens,
      sessions: d.sessionSet.size,
      models: d.models,
    }))
    .sort((a, b) => a.date.localeCompare(b.date))
  let peakHour: number | null = null, peakCnt = 0
  for (let h = 0; h < 24; h++) if (userByHour[h]! > peakCnt) { peakCnt = userByHour[h]!; peakHour = h }
  let longest = 0, run = 0
  let prev: Date | null = null
  for (const { date } of heatmap) {
    const d = new Date(date + 'T00:00:00')
    if (prev && (d.getTime() - prev.getTime()) === 86_400_000) run++
    else run = 1
    if (run > longest) longest = run
    prev = d
  }
  const modelsTimeseries = heatmap.map(d => ({ date: d.date, series: d.models }))
  return { heatmap, peakHour, longestStreak: longest, activeDays: heatmap.length, modelsTimeseries }
}

interface SessionRow {
  sessionId: string
  title: string | null
  folder: string | null
  startedAt: string
  lastActivityAt: string
  userMessages: number
  assistantMessages: number
  compactCount: number
  inTokens: number
  cacheReadTokens: number
  cacheWriteTokens: number
  outTokens: number
  contextTokens: number
  model: string | null
  jsonlAvailable: boolean
}

function fmtNum(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + 'K'
  return n.toLocaleString()
}

function fmtHour(h: number | null): string {
  if (h == null) return '—'
  return `${String(h).padStart(2, '0')}:00`
}

function shortModel(m: string | null): string {
  if (!m) return '—'
  const match = m.match(/claude-(opus|sonnet|haiku|fable|mythos)-(\d+)-?(\d+)?/i)
  if (match) {
    const family = match[1]!.charAt(0).toUpperCase() + match[1]!.slice(1).toLowerCase()
    const ver = match[3] ? `${match[2]}.${match[3]}` : match[2]
    return `${family} ${ver}`
  }
  return m
}

interface Props { tokenId?: string | null; compact?: boolean }

export function StatsCard({ tokenId, compact }: Props) {
  const [data, setData] = useState<OverviewRaw | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [range, setRange] = useState<Range>('all')

  useEffect(() => {
    let cancelled = false
    setLoading(true); setError(null)
    const url = `/api/dashboard/stats/overview?range=${range}${tokenId ? `&token=${encodeURIComponent(tokenId)}` : ''}`
    const sessTok = localStorage.getItem('dashboard_session_token')
    fetch(url, { signal: AbortSignal.timeout(15_000), headers: sessTok ? { Authorization: `Bearer ${sessTok}` } : {} })
      .then(r => r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`))
      .then((d: OverviewRaw) => { if (!cancelled) { setData(d); setLoading(false) } })
      .catch(e => { if (!cancelled) { setError(String(e)); setLoading(false) } })
    return () => { cancelled = true }
  }, [range, tokenId])

  const local = useMemo(() => data ? aggregateLocal(data.hourly) : null, [data])

  const cellSize = 12
  const cellGap = 3
  const weeks = range === '7d' ? 5 : range === '30d' ? 8 : 26

  const grid = local ? buildWeekGrid(local.heatmap, weeks) : null
  const max = grid ? Math.max(...grid.cells.map(c => c?.userMessages ?? 0)) : 0

  return (
    <Card>
      <div style={{ padding: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 14 }}>
          {compact
            ? <span style={{ fontSize: 11, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Stats</span>
            : <SectionHeader title={tokenId ? 'Token Stats' : 'Overall Stats'} icon="fa-solid fa-chart-column" />
          }
          <div style={{ display: 'flex', gap: 6 }}>
            {(['all', '30d', '7d'] as Range[]).map(r => (
              <button
                key={r}
                onClick={() => setRange(r)}
                style={{
                  padding: '4px 10px', borderRadius: 4, border: 'none', cursor: 'pointer',
                  background: r === range ? 'var(--bg-quaternary)' : 'transparent',
                  color: r === range ? 'var(--text-primary)' : 'var(--text-muted)',
                  fontSize: 12, fontWeight: 600,
                }}
              >{r === 'all' ? 'All' : r}</button>
            ))}
          </div>
        </div>

        {loading && <div style={{ padding: 20, textAlign: 'center', color: 'var(--text-muted)' }}>Loading…</div>}
        {error && !loading && <div style={{ padding: 12, color: 'var(--accent-red)', fontSize: 12 }}>{error}</div>}

        {data && local && !loading && (
          <>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 8, marginBottom: 8 }}>
              <Metric label="Sessions" value={fmtNum(data.sessions)} />
              <Metric label="User msg" value={fmtNum(data.userMessages)} />
              <Metric label="Assistant msg" value={fmtNum(data.assistantMessages)} />
              <Metric label="Session tokens" value={fmtNum(data.totalTokens)} />
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 8, marginBottom: 14 }}>
              <Metric label="Active days" value={String(local.activeDays)} />
              <Metric label="Longest streak" value={`${local.longestStreak}d`} />
              <Metric label="Peak hour" value={fmtHour(local.peakHour)} />
              <Metric label="Favorite model" value={shortModel(data.favoriteModel)} />
            </div>

            {grid && (
              <div style={{
                display: 'grid',
                gridTemplateColumns: `repeat(${grid.cols},${cellSize}px)`,
                gridAutoFlow: 'column',
                gridTemplateRows: `repeat(7,${cellSize}px)`,
                gap: cellGap,
              }}>
                {grid.cells.map((cell, i) => {
                  if (!cell) return <div key={i} style={{ width: cellSize, height: cellSize }} />
                  const lvl = intensityClass(cell.userMessages, max)
                  const bg = lvl === 0
                    ? 'var(--bg-tertiary)'
                    : `color-mix(in srgb, var(--accent-blue) ${[0, 22, 45, 70, 100][lvl]}%, var(--bg-tertiary))`
                  // Build tooltip: full per-day stat. Native `title` attr —
                  // multi-line via \n, browser renders as a tooltip popup.
                  const modelsLine = Object.entries(cell.models)
                    .sort((a, b) => b[1] - a[1])
                    .slice(0, 3)
                    .map(([m, t]) => `${shortModel(m)}: ${fmtNum(t)}`)
                    .join('\n  ')
                  const tooltip = [
                    cell.date,
                    `Sessions: ${cell.sessions}`,
                    `User msg: ${cell.userMessages}`,
                    `Assistant msg: ${cell.assistantMessages}`,
                    `Tokens: ${fmtNum(cell.tokens)}`,
                    modelsLine ? `Models:\n  ${modelsLine}` : null,
                  ].filter(Boolean).join('\n')
                  return (
                    <div key={i}
                      title={tooltip}
                      style={{ width: cellSize, height: cellSize, background: bg, borderRadius: 2, cursor: 'pointer' }}
                    />
                  )
                })}
              </div>
            )}

            {data.models.length > 0 && (
              <div style={{ marginTop: 14, paddingTop: 10, borderTop: '1px solid var(--border-secondary)' }}>
                <div style={{ fontSize: 11, color: 'var(--text-muted)', textTransform: 'uppercase', marginBottom: 6, letterSpacing: '0.05em' }}>Models</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                  {data.models.map(m => (
                    <div key={m.name} style={{ display: 'grid', gridTemplateColumns: '1fr auto auto', gap: 12, fontSize: 12 }}>
                      <span style={{ color: 'var(--text-primary)', fontWeight: 600 }}>{shortModel(m.name)}</span>
                      <span style={{ color: 'var(--text-muted)' }}>{fmtNum(m.in)} in · {fmtNum(m.out)} out</span>
                      <span style={{ color: 'var(--text-primary)', fontWeight: 700, minWidth: 50, textAlign: 'right' }}>{m.pct.toFixed(1)}%</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </Card>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      justifyContent: 'space-between',
      background: 'var(--bg-tertiary)',
      border: '1px solid var(--border-secondary)',
      borderRadius: 6,
      padding: '8px 10px',
      minHeight: 56,
    }}>
      <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>{label}</div>
      <div style={{ fontSize: 14, fontWeight: 700, color: 'var(--text-primary)' }}>{value}</div>
    </div>
  )
}

interface HeatmapCell {
  date: string
  userMessages: number
  assistantMessages: number
  tokens: number
  sessions: number
  models: Record<string, number>
}
function buildWeekGrid(heatmap: HeatmapCell[], weeks: number) {
  const today = new Date(); today.setHours(0, 0, 0, 0)
  const todayDow = (today.getDay() + 6) % 7
  const totalCells = weeks * 7
  const startDate = new Date(today.getTime() - (totalCells - 1 - (6 - todayDow)) * 86_400_000)
  const map = new Map(heatmap.map(d => [d.date, d]))
  const cells: Array<HeatmapCell | null> = []
  for (let i = 0; i < totalCells; i++) {
    const d = new Date(startDate.getTime() + i * 86_400_000)
    if (d > today) { cells.push(null); continue }
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    cells.push(map.get(key) ?? { date: key, userMessages: 0, assistantMessages: 0, tokens: 0, sessions: 0, models: {} })
  }
  return { cells, cols: weeks }
}

function intensityClass(count: number, max: number): number {
  if (count <= 0) return 0
  const pct = count / Math.max(1, max)
  if (pct < 0.25) return 1
  if (pct < 0.5) return 2
  if (pct < 0.75) return 3
  return 4
}

// ── Grid of per-user StatsCards (one per known user/token) ──────────
import { api } from '../../services/api-client'
import type { UserSummary } from '../../services/api-client'

export function TokensStatsGrid() {
  const [users, setUsers] = useState<UserSummary[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let cancelled = false
    api.getUsers()
      .then(list => { if (!cancelled) setUsers(list) })
      .catch(e => { if (!cancelled) setError(String(e)) })
    return () => { cancelled = true }
  }, [])
  if (error) return <Card><div style={{ padding: 16, color: 'var(--accent-red)', fontSize: 12 }}>{error}</div></Card>
  if (!users) return <Card><div style={{ padding: 20, color: 'var(--text-muted)' }}>Loading users…</div></Card>
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(420px, 1fr))', gap: 14 }}>
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: 12, color: 'var(--text-secondary)', fontWeight: 700, marginBottom: 4, textTransform: 'uppercase', letterSpacing: '0.04em' }}>
          <i class="fa-solid fa-globe" style={{ marginRight: 6, opacity: 0.6 }} />
          All tokens
        </div>
        <StatsCard tokenId={null} compact />
      </div>
      {users.filter(u => u.userName && u.userName.trim()).map(u => (
        <div key={u.userName} style={{ minWidth: 0 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary)', fontWeight: 700, marginBottom: 4, textTransform: 'uppercase', letterSpacing: '0.04em' }}>
            <i class="fa-solid fa-id-badge" style={{ marginRight: 6, opacity: 0.6 }} />
            {u.userName}
          </div>
          <StatsCard tokenId={u.userName} compact />
        </div>
      ))}
    </div>
  )
}

// ── Per-token sessions list with download ──────────────────────────
interface TokenSessionsListProps {
  tokenId: string
  onCountChange?: (count: number) => void
}

export function TokenSessionsList({ tokenId, onCountChange }: TokenSessionsListProps) {
  const [sessions, setSessions] = useState<SessionRow[] | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true); setError(null)
    const sessTok2 = localStorage.getItem('dashboard_session_token')
    fetch(`/api/dashboard/tokens/${encodeURIComponent(tokenId)}/sessions`, { signal: AbortSignal.timeout(15_000), headers: sessTok2 ? { Authorization: `Bearer ${sessTok2}` } : {} })
      .then(r => r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`))
      .then((d: { sessions: SessionRow[] }) => {
        if (!cancelled) { setSessions(d.sessions); setLoading(false); onCountChange?.(d.sessions.length) }
      })
      .catch(e => { if (!cancelled) { setError(String(e)); setLoading(false) } })
    return () => { cancelled = true }
  }, [tokenId])

  async function downloadJsonl(sessionId: string) {
    // Browser <a download> doesn't send Authorization headers — fetch
    // with Bearer (when auth is on), turn the response into a blob, and
    // trigger a download via object URL. Works whether auth is enabled
    // (with token) or not (header omitted).
    const url = `/api/dashboard/sessions/${encodeURIComponent(sessionId)}/jsonl`
    const sessTok = localStorage.getItem('dashboard_session_token')
    try {
      const res = await fetch(url, {
        headers: sessTok ? { Authorization: `Bearer ${sessTok}` } : {},
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const blob = await res.blob()
      const objUrl = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = objUrl
      link.download = `${sessionId}.jsonl`
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)
      // Defer revocation so the browser actually starts the download.
      setTimeout(() => URL.revokeObjectURL(objUrl), 1000)
    } catch (e) {
      alert(`Download failed: ${e instanceof Error ? e.message : String(e)}`)
    }
  }

  async function softDelete(sessionId: string) {
    if (!confirm('Soft-delete this session? It will drop out of stats and the JSONL file will be removed. History stays in the DB.')) return
    try {
      const sessTok3 = localStorage.getItem('dashboard_session_token')
      const res = await fetch(`/api/dashboard/sessions/${encodeURIComponent(sessionId)}?rmFile=1`, { method: 'DELETE', headers: sessTok3 ? { Authorization: `Bearer ${sessTok3}` } : {} })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      setSessions(prev => prev ? prev.filter(s => s.sessionId !== sessionId) : prev)
    } catch (e) {
      alert(`Delete failed: ${e instanceof Error ? e.message : String(e)}`)
    }
  }

  // Sessions arrive sorted by last_activity_at DESC from the server.
  const sorted = sessions ? [...sessions].sort((a, b) =>
    new Date(b.lastActivityAt).getTime() - new Date(a.lastActivityAt).getTime()
  ) : null

  const cellStyle = { padding: '8px 12px', fontSize: 12, verticalAlign: 'middle' as const }
  const headStyle = { ...cellStyle, color: 'var(--text-muted)', fontWeight: 500, textAlign: 'left' as const, borderBottom: '1px solid var(--border-primary)', fontSize: 11, textTransform: 'uppercase' as const, letterSpacing: '0.04em' }

  return (
    <div>
        {loading && <div style={{ padding: 16, color: 'var(--text-muted)' }}>Loading…</div>}
        {error && !loading && <div style={{ padding: 12, color: 'var(--accent-red)', fontSize: 12 }}>{error}</div>}
        {sorted && !loading && sorted.length === 0 && (
          <div style={{ padding: 16, textAlign: 'center', color: 'var(--text-muted)', fontSize: 13 }}>No sessions for this token yet.</div>
        )}
        {sorted && sorted.length > 0 && (
          <div style={{ maxHeight: 520, overflowY: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse' }}>
              <thead style={{ position: 'sticky', top: 0, background: 'var(--bg-secondary)', zIndex: 1 }}>
                <tr>
                  <th style={headStyle}>Title / ID</th>
                  <th style={headStyle}>Folder</th>
                  <th style={headStyle}>Model</th>
                  <th style={{ ...headStyle, textAlign: 'right' }}>Last Activity</th>
                  <th style={{ ...headStyle, textAlign: 'right' }}>User</th>
                  <th style={{ ...headStyle, textAlign: 'right' }}>Asst</th>
                  <th style={{ ...headStyle, textAlign: 'right' }} title="Number of /compact actions that forked off this session">Compacts</th>
                  <th style={{ ...headStyle, textAlign: 'right' }} title="Real text size: sum of every compact snapshot + current context (matches client progress bars across the whole conversation)">Context</th>
                  <th style={{ ...headStyle, textAlign: 'right' }} title="Total tokens flowed through API: input + cache_read + cache_write + output, summed over all turns">Total</th>
                  <th style={{ ...headStyle, textAlign: 'right' }}></th>
                </tr>
              </thead>
              <tbody>
                {sorted.map(s => (
                  <tr key={s.sessionId} style={{ borderBottom: '1px solid var(--border-secondary)' }}>
                    <td style={{ ...cellStyle, maxWidth: 300, overflow: 'hidden' }} title={s.title || s.sessionId}>
                      <div style={{
                        color: 'var(--text-primary)',
                        fontWeight: s.title ? 600 : 400,
                        overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                      }}>
                        {s.title || <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-secondary)' }}>{s.sessionId.slice(0, 8)}…</span>}
                      </div>
                      <div style={{ fontSize: 10, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>{s.sessionId.slice(0, 8)}</div>
                    </td>
                    <td style={{ ...cellStyle, color: 'var(--accent-blue)', maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={s.folder ?? ''}>
                      {s.folder ? s.folder.split('-').filter(p => p && !/^[0-9a-f]{8,}$/i.test(p)).slice(-3).join('/') || s.folder : '—'}
                    </td>
                    <td style={{ ...cellStyle, color: 'var(--text-secondary)' }}>{shortModel(s.model)}</td>
                    <td style={{ ...cellStyle, textAlign: 'right', color: 'var(--text-muted)', fontSize: 11, whiteSpace: 'nowrap' }}>
                      {new Date(s.lastActivityAt).toLocaleString()}
                    </td>
                    <td style={{ ...cellStyle, textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--accent-blue)' }} title="User messages">{s.userMessages}</td>
                    <td style={{ ...cellStyle, textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }} title="Assistant messages">{s.assistantMessages}</td>
                    <td style={{ ...cellStyle, textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }} title="Number of /compact actions that forked off this session">{s.compactCount}</td>
                    <td style={{ ...cellStyle, textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--accent-purple)' }} title="Real conversation text: sum of every compact snapshot + current context">{fmtNum(s.contextTokens)}</td>
                    <td style={{ ...cellStyle, textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }} title="Cumulative API throughput: input + cache_read + cache_write + output across all turns">{fmtNum(s.inTokens + s.cacheReadTokens + s.cacheWriteTokens + s.outTokens)}</td>
                    <td style={{ ...cellStyle, textAlign: 'right', whiteSpace: 'nowrap' }}>
                      <button
                        onClick={() => downloadJsonl(s.sessionId)}
                        disabled={!s.jsonlAvailable}
                        title={s.jsonlAvailable ? 'Download JSONL' : 'JSONL file no longer on disk'}
                        style={{
                          background: 'transparent', border: '1px solid var(--border-secondary)',
                          color: s.jsonlAvailable ? 'var(--accent-blue)' : 'var(--text-disabled)',
                          borderRadius: 4, padding: '3px 8px', cursor: s.jsonlAvailable ? 'pointer' : 'not-allowed',
                          fontSize: 11, marginRight: 6,
                        }}
                      >
                        <i class="fa-solid fa-download" />
                      </button>
                      <button
                        onClick={() => softDelete(s.sessionId)}
                        title="Soft-delete"
                        style={{
                          background: 'transparent', border: '1px solid var(--border-secondary)',
                          color: 'var(--accent-red)',
                          borderRadius: 4, padding: '3px 8px', cursor: 'pointer', fontSize: 11,
                        }}
                      >
                        <i class="fa-solid fa-trash" />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
    </div>
  )
}
