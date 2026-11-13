// Dashboard Stats cards — one card per user plus the all-users card.
//
// Data comes from ONE batched request for the whole grid
// (/api/dashboard/stats/users-overview, served from the incremental stats
// rollup) through the shared store in signals/users-stats.ts; a card's own
// All/30d/7d switch fetches just that card. Cards mount lazily when they come
// near the viewport and are memoized, so a poll that changes one user's
// numbers re-renders one card, not the grid.

import { memo, useEffect, useMemo, useRef, useState } from 'preact/compat'
import { createJsonlDownloader } from '../../utils/jsonl-download'
import { Card } from '../shared'
import { cards, cardsError, fetchCard, subscribeUserStats, users } from '../../signals/users-stats'
import type { CardData, StatsRange } from '../../signals/users-stats'

type Range = StatsRange
import { ENGINE_LABEL, EngineBadge, EngineFilterTabs, asEngine, engineFilter, engineParam, type Engine } from './engine'

interface SessionRow {
  sessionId: string
  title: string | null
  folder: string | null
  startedAt: string
  lastActivityAt: string
  /** Engines the chat ran on (both for a chat continued on the other one). */
  engines?: string[]
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

const WEEKS: Record<Range, number> = { all: 26, '30d': 8, '7d': 5 }
const CELL = 12
const GAP = 3

interface HeatmapCell {
  date: string
  userMessages: number
  assistantMessages: number
  tokens: number
  sessions: number
  models: Record<string, number>
}

function localKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function buildWeekGrid(days: CardData['days'], weeks: number) {
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  const todayDow = (today.getDay() + 6) % 7
  const total = weeks * 7
  const map = new Map(days.map((d) => [d[0], d]))
  const cells: Array<HeatmapCell | null> = []
  const start = new Date(today)
  start.setDate(start.getDate() - (total - 1 - (6 - todayDow)))
  for (let i = 0; i < total; i++) {
    const d = new Date(start)
    d.setDate(start.getDate() + i)
    if (d > today) {
      cells.push(null)
      continue
    }
    const key = localKey(d)
    const r = map.get(key)
    cells.push(
      r
        ? { date: key, userMessages: r[1], assistantMessages: r[2], tokens: r[3], sessions: r[4], models: r[5] }
        : { date: key, userMessages: 0, assistantMessages: 0, tokens: 0, sessions: 0, models: {} },
    )
  }
  return cells
}

function intensityClass(count: number, max: number): number {
  if (count <= 0) return 0
  const pct = count / Math.max(1, max)
  if (pct < 0.25) return 1
  if (pct < 0.5) return 2
  if (pct < 0.75) return 3
  return 4
}

const Heatmap = memo(function Heatmap({ days, range }: { days: CardData['days']; range: Range }) {
  const cells = buildWeekGrid(days, WEEKS[range])
  const max = Math.max(0, ...cells.map((c) => c?.userMessages ?? 0))
  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: `repeat(${WEEKS[range]},${CELL}px)`,
        gridAutoFlow: 'column',
        gridTemplateRows: `repeat(7,${CELL}px)`,
        gap: GAP,
      }}
    >
      {cells.map((cell, i) => {
        if (!cell) return <div key={i} style={{ width: CELL, height: CELL }} />
        const lvl = intensityClass(cell.userMessages, max)
        const bg =
          lvl === 0
            ? 'var(--bg-tertiary)'
            : `color-mix(in srgb, var(--accent-blue) ${[0, 22, 45, 70, 100][lvl]}%, var(--bg-tertiary))`
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
        ]
          .filter(Boolean)
          .join('\n')
        return (
          <div
            key={i}
            title={tooltip}
            style={{ width: CELL, height: CELL, background: bg, borderRadius: 2, cursor: 'pointer' }}
          />
        )
      })}
    </div>
  )
})

interface ViewProps {
  data: CardData | null
  range: Range
  onRange: (r: Range) => void
  loading: boolean
  error: string | null
}

/** Pure card body: re-renders only when its own data/range/state change. */
const StatsCardView = memo(function StatsCardView({ data, range, onRange, loading, error }: ViewProps) {
  const engine = engineFilter.value
  return (
    <Card>
      <div style={{ padding: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 14 }}>
          <span
            style={{ fontSize: 11, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}
          >
            Stats
          </span>
          <div style={{ display: 'flex', gap: 6 }}>
            {(['all', '30d', '7d'] as Range[]).map((r) => (
              <button
                key={r}
                onClick={() => onRange(r)}
                style={{
                  padding: '4px 10px',
                  borderRadius: 4,
                  border: 'none',
                  cursor: 'pointer',
                  background: r === range ? 'var(--bg-quaternary)' : 'transparent',
                  color: r === range ? 'var(--text-primary)' : 'var(--text-muted)',
                  fontSize: 12,
                  fontWeight: 600,
                }}
              >
                {r === 'all' ? 'All' : r}
              </button>
            ))}
          </div>
        </div>

        {loading && !data && (
          <div style={{ padding: 20, textAlign: 'center', color: 'var(--text-muted)' }}>Loading…</div>
        )}
        {error && !loading && <div style={{ padding: 12, color: 'var(--accent-red)', fontSize: 12 }}>{error}</div>}

        {data && (
          <div style={{ opacity: loading ? 0.6 : 1 }}>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 8, marginBottom: 8 }}>
              <Metric label="Sessions" value={fmtNum(data.sessions)} />
              <Metric label="User msg" value={fmtNum(data.userMessages)} />
              <Metric label="Assistant msg" value={fmtNum(data.assistantMessages)} />
              <Metric label="Session tokens" value={fmtNum(data.totalTokens)} />
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 8, marginBottom: 14 }}>
              <Metric label="Active days" value={String(data.activeDays)} />
              <Metric label="Longest streak" value={`${data.longestStreak}d`} />
              <Metric label="Peak hour" value={fmtHour(data.peakHour)} />
              <Metric label="Favorite model" value={shortModel(data.favoriteModel)} />
            </div>
            {engine === 'all' && <EngineSplit engines={data.engines} />}

            <Heatmap days={data.days} range={range} />

            {data.models.length > 0 && (
              <div style={{ marginTop: 14, paddingTop: 10, borderTop: '1px solid var(--border-secondary)' }}>
                <div
                  style={{
                    fontSize: 11,
                    color: 'var(--text-muted)',
                    textTransform: 'uppercase',
                    marginBottom: 6,
                    letterSpacing: '0.05em',
                  }}
                >
                  Models
                </div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                  {data.models.map((m) => (
                    <div
                      key={`${m.engine ?? 'claude'}|${m.name}`}
                      style={{ display: 'grid', gridTemplateColumns: '1fr auto auto', gap: 12, fontSize: 12 }}
                    >
                      <span
                        style={{
                          color: 'var(--text-primary)',
                          fontWeight: 600,
                          display: 'flex',
                          gap: 6,
                          alignItems: 'center',
                        }}
                      >
                        {shortModel(m.name)}
                        {m.engine === 'codex' && <EngineBadge engine="codex" />}
                      </span>
                      <span style={{ color: 'var(--text-muted)' }}>
                        {fmtNum(m.in)} in · {fmtNum(m.out)} out
                      </span>
                      <span style={{ color: 'var(--text-primary)', fontWeight: 700, minWidth: 50, textAlign: 'right' }}>
                        {m.pct.toFixed(1)}%
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </Card>
  )
})

/** Claude vs ChatGPT share of the same counters; hidden while one engine has
 *  all the activity (nothing to split). */
function EngineSplit({ engines }: { engines: CardData['engines'] }) {
  const rows = (['claude', 'codex'] as Engine[]).flatMap((e) => (engines?.[e] ? [{ engine: e, v: engines[e]! }] : []))
  if (rows.length < 2) return null
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 14 }}>
      {rows.map(({ engine, v }) => (
        <div
          key={engine}
          title={`${ENGINE_LABEL[engine]}: a chat continued on the other engine counts under both`}
          style={{ display: 'grid', gridTemplateColumns: '70px 1fr', gap: 8, alignItems: 'center', fontSize: 12 }}
        >
          <EngineBadge engine={engine} />
          <span style={{ color: 'var(--text-muted)' }}>
            {fmtNum(v.sessions)} sessions · {fmtNum(v.userMessages)} user · {fmtNum(v.assistantMessages)} assistant ·{' '}
            {fmtNum(v.inTokens)} in · {fmtNum(v.outTokens)} out
          </span>
        </div>
      ))}
    </div>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'space-between',
        background: 'var(--bg-tertiary)',
        border: '1px solid var(--border-secondary)',
        borderRadius: 6,
        padding: '8px 10px',
        minHeight: 56,
      }}
    >
      <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>{label}</div>
      <div style={{ fontSize: 14, fontWeight: 700, color: 'var(--text-primary)' }}>{value}</div>
    </div>
  )
}

const EMPTY_CARD: CardData = {
  sessions: 0,
  userMessages: 0,
  assistantMessages: 0,
  totalTokens: 0,
  favoriteModel: null,
  models: [],
  activeDays: 0,
  longestStreak: 0,
  peakHour: null,
  days: [],
}

/** One card: default range from the shared batch, other ranges on demand. */
const UserStatsCard = memo(function UserStatsCard({ subject }: { subject: string }) {
  const engine = engineFilter.value
  const [range, setRange] = useState<Range>('all')
  const [own, setOwn] = useState<{ range: Range; data: CardData | null; error: string | null } | null>(null)
  // Visible before the page batch arrived: fetch just this card (tens of ms)
  // so on-screen cards render as each is ready; the batch fills the rest.
  const [early, setEarly] = useState<CardData | null>(null)
  useEffect(() => {
    setEarly(null)
    if (cards.peek()) return
    let cancelled = false
    fetchCard(subject, 'all')
      .then((data) => !cancelled && !cards.peek() && setEarly(data ?? EMPTY_CARD))
      .catch(() => {}) // the batch reports errors
    return () => {
      cancelled = true
    }
  }, [subject, engine])
  // A known user without rollup rows still gets a zero card, as before.
  const shared = cards.value ? (cards.value[subject] ?? EMPTY_CARD) : early
  useEffect(() => {
    if (range === 'all') {
      setOwn(null)
      return
    }
    let cancelled = false
    setOwn((o) => ({ range, data: o?.data ?? null, error: null }))
    fetchCard(subject, range)
      .then((data) => !cancelled && setOwn({ range, data: data ?? EMPTY_CARD, error: null }))
      .catch((e) => !cancelled && setOwn({ range, data: null, error: String(e) }))
    return () => {
      cancelled = true
    }
  }, [range, subject, engine])
  const data = range === 'all' ? shared : (own?.data ?? null)
  const loading =
    range === 'all' ? !shared && !cardsError.value : !own || own.range !== range || (!own.data && !own.error)
  const error = range === 'all' ? cardsError.value : (own?.error ?? null)
  return <StatsCardView data={data} range={range} onRange={setRange} loading={loading} error={error} />
})

/** Mounts its card when it comes within one screen of the viewport, then keeps it. */
function LazyCard({ subject }: { subject: string }) {
  const engine = engineFilter.value
  const ref = useRef<HTMLDivElement>(null)
  const [seen, setSeen] = useState(false)
  useEffect(() => {
    const el = ref.current
    if (!el || seen) return
    if (typeof IntersectionObserver === 'undefined') {
      setSeen(true)
      return
    }
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setSeen(true)
          io.disconnect()
        }
      },
      { rootMargin: '100% 0px' },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [seen])
  return (
    <div ref={ref} style={{ minWidth: 0, contentVisibility: 'auto', containIntrinsicSize: 'auto 420px' } as any}>
      {seen ? <UserStatsCard key={`${subject}|${engine}`} subject={subject} /> : <div style={{ height: 420 }} />}
    </div>
  )
}

const headerStyle = {
  fontSize: 12,
  color: 'var(--text-secondary)',
  fontWeight: 700,
  marginBottom: 4,
  textTransform: 'uppercase' as const,
  letterSpacing: '0.04em',
}

// ── Grid of per-user stats cards (one per known user/token) ──────────
export function TokensStatsGrid() {
  useEffect(() => subscribeUserStats({ cards: true }), [])
  const list = users.value
  // Card order is fixed on first load (new users appended) so the grid
  // never reshuffles while someone reads it.
  const order = useRef<string[]>([])
  const names = useMemo(() => {
    const present = new Set(list.filter((u) => u.userName && u.userName.trim()).map((u) => u.userName))
    const kept = order.current.filter((n) => present.has(n))
    for (const n of present) if (!kept.includes(n)) kept.push(n)
    order.current = kept
    return kept
  }, [list])
  if (cardsError.value && !cards.value && list.length === 0)
    return (
      <Card>
        <div style={{ padding: 16, color: 'var(--accent-red)', fontSize: 12 }}>{cardsError.value}</div>
      </Card>
    )
  if (list.length === 0 && !cards.value)
    return (
      <Card>
        <div style={{ padding: 20, color: 'var(--text-muted)' }}>Loading users…</div>
      </Card>
    )
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(420px, 1fr))', gap: 14 }}>
      <div style={{ minWidth: 0 }}>
        <div
          style={{
            fontSize: 12,
            color: 'var(--text-secondary)',
            fontWeight: 700,
            marginBottom: 4,
            textTransform: 'uppercase',
            letterSpacing: '0.04em',
          }}
        >
          <span style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
            <span>
              <i class="fa-solid fa-globe" style={{ marginRight: 6, opacity: 0.6 }} />
              All tokens
            </span>
            {/* One engine filter for every card of the grid. */}
            <EngineFilterTabs />
          </span>
        </div>
        <LazyCard subject="*" />
      </div>
      {names.map((name) => (
        <div key={name} style={{ minWidth: 0 }}>
          <div style={headerStyle}>
            <i class="fa-solid fa-id-badge" style={{ marginRight: 6, opacity: 0.6 }} />
            {name}
          </div>
          <LazyCard subject={name} />
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
  const engine = engineFilter.value

  const [downloadBusy, setDownloadBusy] = useState(false)
  const downloader = useMemo(() => createJsonlDownloader(setDownloadBusy), [tokenId])
  useEffect(() => {
    setDownloadBusy(false)
    return () => downloader.dispose()
  }, [downloader])

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    const sessTok2 = localStorage.getItem('dashboard_session_token')
    fetch(`/api/dashboard/tokens/${encodeURIComponent(tokenId)}/sessions?${engineParam(engine).slice(1)}`, {
      signal: AbortSignal.timeout(15_000),
      headers: sessTok2 ? { Authorization: `Bearer ${sessTok2}` } : {},
    })
      .then((r) => (r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`)))
      .then((d: { sessions: SessionRow[] }) => {
        if (!cancelled) {
          setSessions(d.sessions)
          setLoading(false)
          onCountChange?.(d.sessions.length)
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e))
          setLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [tokenId, engine])

  async function downloadJsonl(sessionId: string) {
    try {
      await downloader.start(sessionId, localStorage.getItem('dashboard_session_token'))
    } catch (e) {
      alert(`Download failed: ${e instanceof Error ? e.message : String(e)}`)
    }
  }

  async function softDelete(sessionId: string) {
    if (
      !confirm(
        'Soft-delete this session? It will drop out of stats and the JSONL file will be removed. History stays in the DB.',
      )
    )
      return
    try {
      const sessTok3 = localStorage.getItem('dashboard_session_token')
      const res = await fetch(`/api/dashboard/sessions/${encodeURIComponent(sessionId)}?rmFile=1`, {
        method: 'DELETE',
        headers: sessTok3 ? { Authorization: `Bearer ${sessTok3}` } : {},
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      setSessions((prev) => (prev ? prev.filter((s) => s.sessionId !== sessionId) : prev))
    } catch (e) {
      alert(`Delete failed: ${e instanceof Error ? e.message : String(e)}`)
    }
  }

  // Sessions arrive sorted by last_activity_at DESC from the server.
  const sorted = sessions
    ? [...sessions].sort((a, b) => new Date(b.lastActivityAt).getTime() - new Date(a.lastActivityAt).getTime())
    : null

  const cellStyle = { padding: '8px 12px', fontSize: 12, verticalAlign: 'middle' as const }
  const headStyle = {
    ...cellStyle,
    color: 'var(--text-muted)',
    fontWeight: 500,
    textAlign: 'left' as const,
    borderBottom: '1px solid var(--border-primary)',
    fontSize: 11,
    textTransform: 'uppercase' as const,
    letterSpacing: '0.04em',
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>
        <EngineFilterTabs />
      </div>
      {loading && <div style={{ padding: 16, color: 'var(--text-muted)' }}>Loading…</div>}
      {error && !loading && <div style={{ padding: 12, color: 'var(--accent-red)', fontSize: 12 }}>{error}</div>}
      {sorted && !loading && sorted.length === 0 && (
        <div style={{ padding: 16, textAlign: 'center', color: 'var(--text-muted)', fontSize: 13 }}>
          {engine === 'all' ? 'No sessions for this token yet.' : `No ${ENGINE_LABEL[engine]} sessions for this token.`}
        </div>
      )}
      {sorted && sorted.length > 0 && (
        <div style={{ maxHeight: 520, overflowY: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead style={{ position: 'sticky', top: 0, background: 'var(--bg-secondary)', zIndex: 1 }}>
              <tr>
                <th style={headStyle}>Title / ID</th>
                <th style={headStyle}>Folder</th>
                <th style={headStyle}>Engine / Model</th>
                <th style={{ ...headStyle, textAlign: 'right' }}>Last Activity</th>
                <th style={{ ...headStyle, textAlign: 'right' }}>User</th>
                <th style={{ ...headStyle, textAlign: 'right' }}>Asst</th>
                <th
                  style={{ ...headStyle, textAlign: 'right' }}
                  title="Number of /compact actions that forked off this session"
                >
                  Compacts
                </th>
                <th
                  style={{ ...headStyle, textAlign: 'right' }}
                  title="Real text size: sum of every compact snapshot + current context (matches client progress bars across the whole conversation)"
                >
                  Context
                </th>
                <th
                  style={{ ...headStyle, textAlign: 'right' }}
                  title="Total tokens flowed through API: input + cache_read + cache_write + output, summed over all turns"
                >
                  Total
                </th>
                <th style={{ ...headStyle, textAlign: 'right' }}></th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((s) => (
                <tr key={s.sessionId} style={{ borderBottom: '1px solid var(--border-secondary)' }}>
                  <td style={{ ...cellStyle, maxWidth: 300, overflow: 'hidden' }} title={s.title || s.sessionId}>
                    <div
                      style={{
                        color: 'var(--text-primary)',
                        fontWeight: s.title ? 600 : 400,
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {s.title || (
                        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-secondary)' }}>
                          {s.sessionId.slice(0, 8)}…
                        </span>
                      )}
                    </div>
                    <div style={{ fontSize: 10, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                      {s.sessionId.slice(0, 8)}
                    </div>
                  </td>
                  <td
                    style={{
                      ...cellStyle,
                      color: 'var(--accent-blue)',
                      maxWidth: 200,
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                    title={s.folder ?? ''}
                  >
                    {s.folder
                      ? s.folder
                          .split('-')
                          .filter((p) => p && !/^[0-9a-f]{8,}$/i.test(p))
                          .slice(-3)
                          .join('/') || s.folder
                      : '—'}
                  </td>
                  <td style={{ ...cellStyle, color: 'var(--text-secondary)' }}>
                    <div style={{ display: 'flex', gap: 4, marginBottom: 2 }}>
                      {(s.engines?.length ? s.engines : ['claude']).map((e) => (
                        <EngineBadge key={e} engine={asEngine(e)} />
                      ))}
                    </div>
                    {shortModel(s.model)}
                  </td>
                  <td
                    style={{
                      ...cellStyle,
                      textAlign: 'right',
                      color: 'var(--text-muted)',
                      fontSize: 11,
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {new Date(s.lastActivityAt).toLocaleString()}
                  </td>
                  <td
                    style={{
                      ...cellStyle,
                      textAlign: 'right',
                      fontFamily: 'var(--font-mono)',
                      color: 'var(--accent-blue)',
                    }}
                    title="User messages"
                  >
                    {s.userMessages}
                  </td>
                  <td
                    style={{
                      ...cellStyle,
                      textAlign: 'right',
                      fontFamily: 'var(--font-mono)',
                      color: 'var(--text-secondary)',
                    }}
                    title="Assistant messages"
                  >
                    {s.assistantMessages}
                  </td>
                  <td
                    style={{
                      ...cellStyle,
                      textAlign: 'right',
                      fontFamily: 'var(--font-mono)',
                      color: 'var(--text-secondary)',
                    }}
                    title="Number of /compact actions that forked off this session"
                  >
                    {s.compactCount}
                  </td>
                  <td
                    style={{
                      ...cellStyle,
                      textAlign: 'right',
                      fontFamily: 'var(--font-mono)',
                      color: 'var(--accent-purple)',
                    }}
                    title="Real conversation text: sum of every compact snapshot + current context"
                  >
                    {fmtNum(s.contextTokens)}
                  </td>
                  <td
                    style={{
                      ...cellStyle,
                      textAlign: 'right',
                      fontFamily: 'var(--font-mono)',
                      color: 'var(--text-secondary)',
                    }}
                    title="Cumulative API throughput: input + cache_read + cache_write + output across all turns"
                  >
                    {fmtNum(s.inTokens + s.cacheReadTokens + s.cacheWriteTokens + s.outTokens)}
                  </td>
                  <td style={{ ...cellStyle, textAlign: 'right', whiteSpace: 'nowrap' }}>
                    <button
                      onClick={() => downloadJsonl(s.sessionId)}
                      disabled={!s.jsonlAvailable || downloadBusy}
                      title={
                        downloadBusy
                          ? 'Download requested — wait briefly before another export; cancel in browser Downloads'
                          : s.jsonlAvailable
                            ? 'Download JSONL'
                            : 'JSONL file no longer on disk'
                      }
                      style={{
                        background: 'transparent',
                        border: '1px solid var(--border-secondary)',
                        color: s.jsonlAvailable ? 'var(--accent-blue)' : 'var(--text-disabled)',
                        borderRadius: 4,
                        padding: '3px 8px',
                        cursor: s.jsonlAvailable ? 'pointer' : 'not-allowed',
                        fontSize: 11,
                        marginRight: 6,
                      }}
                    >
                      <i class="fa-solid fa-download" />
                    </button>
                    <button
                      onClick={() => softDelete(s.sessionId)}
                      title="Soft-delete"
                      style={{
                        background: 'transparent',
                        border: '1px solid var(--border-secondary)',
                        color: 'var(--accent-red)',
                        borderRadius: 4,
                        padding: '3px 8px',
                        cursor: 'pointer',
                        fontSize: 11,
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
