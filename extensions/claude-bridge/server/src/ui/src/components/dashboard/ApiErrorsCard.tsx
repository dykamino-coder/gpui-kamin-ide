import { useEffect, useState } from 'preact/compat'
import { Card, Modal } from '../shared'

interface ErrorPoint {
  period: string
  count: number
  connectionRefused: number
  timeouts: number
  rateLimited: number
}

interface ErrorRow {
  timestamp: string
  sessionId: string | null
  userName: string | null
  errorType: string
  url: string
  attempt: number
  maxAttempts: number
  retryMs: number
  raw: string
}

function authFetch(url: string): Promise<Response> {
  const sessTok = localStorage.getItem('dashboard_session_token')
  return fetch(url, { headers: sessTok ? { Authorization: `Bearer ${sessTok}` } : {} })
}

/** Card with API error count + click → modal with timeseries chart and
 *  recent error list. Captures retry log lines from the CLI's terminal
 *  output (server-side regex in session-io). */
export function ApiErrorsCard() {
  const [counts, setCounts] = useState<{ total: number; last24h: number; lastAt: string | null } | null>(null)
  const [open, setOpen] = useState(false)

  async function refresh() {
    try {
      const res = await authFetch('/api/dashboard/errors/count')
      if (res.ok) setCounts(await res.json())
    } catch { /* ignore */ }
  }
  useEffect(() => {
    refresh()
    const id = setInterval(refresh, 30_000)
    return () => clearInterval(id)
  }, [])

  const total = counts?.total ?? 0
  const last24h = counts?.last24h ?? 0
  const color = last24h > 50 ? 'var(--accent-red)' : last24h > 0 ? 'var(--accent-yellow)' : 'var(--text-muted)'

  return (
    <>
      <Card>
        <div
          onClick={() => setOpen(true)}
          style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px', cursor: 'pointer' }}
          title="Click to open errors chart"
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <i class="fa-solid fa-triangle-exclamation" style={{ color, fontSize: 16 }} />
            <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)' }}>API Errors</span>
            {counts?.lastAt && (
              <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>last {timeAgo(counts.lastAt)}</span>
            )}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
            <span title="Last 24h" style={{ fontSize: 13, fontWeight: 700, color }}>{last24h}<span style={{ fontSize: 10, color: 'var(--text-muted)', marginLeft: 4 }}>24h</span></span>
            <span title="Total" style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{total}<span style={{ fontSize: 10, color: 'var(--text-muted)', marginLeft: 4 }}>total</span></span>
          </div>
        </div>
      </Card>
      <Modal isOpen={open} onClose={() => setOpen(false)} title={`API Errors — ${last24h} in last 24h, ${total} total`} size="lg">
        <ErrorsChartAndList />
      </Modal>
    </>
  )
}

function timeAgo(iso: string): string {
  const sec = Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  if (sec < 60) return `${sec}s ago`
  if (sec < 3600) return `${Math.floor(sec / 60)}m ago`
  if (sec < 86400) return `${Math.floor(sec / 3600)}h ago`
  return `${Math.floor(sec / 86400)}d ago`
}

function ErrorsChartAndList() {
  const [tab, setTab] = useState<'daily' | 'weekly' | 'monthly'>('weekly')
  const [points, setPoints] = useState<ErrorPoint[] | null>(null)
  const [recent, setRecent] = useState<ErrorRow[] | null>(null)

  useEffect(() => {
    const params = tab === 'daily' ? 'period=hour&days=1'
      : tab === 'weekly' ? 'period=day&days=7'
      : 'period=day&days=30'
    authFetch(`/api/dashboard/errors/timeseries?${params}`)
      .then(r => r.json()).then(setPoints).catch(() => setPoints([]))
  }, [tab])

  useEffect(() => {
    authFetch('/api/dashboard/errors/recent?limit=100')
      .then(r => r.json()).then(setRecent).catch(() => setRecent([]))
  }, [])

  return (
    <div style={{ padding: 14 }}>
      <div style={{ display: 'flex', gap: 6, marginBottom: 10 }}>
        {(['daily', 'weekly', 'monthly'] as const).map(t => (
          <button key={t} onClick={() => setTab(t)} style={{
            padding: '4px 12px', borderRadius: 4, border: 'none', cursor: 'pointer', fontSize: 12, fontWeight: 600,
            background: tab === t ? 'var(--bg-quaternary)' : 'transparent',
            color: tab === t ? 'var(--text-primary)' : 'var(--text-muted)',
          }}>{t.charAt(0).toUpperCase() + t.slice(1)}</button>
        ))}
      </div>

      {points === null ? <div style={{ padding: 20, color: 'var(--text-muted)' }}>Loading…</div>
        : points.length === 0 ? <div style={{ padding: 30, textAlign: 'center', color: 'var(--text-muted)' }}>No errors in this range — clean skies.</div>
        : <ErrorsBarChart points={points} period={tab === 'daily' ? 'hour' : 'day'} />}

      <div style={{ marginTop: 18 }}>
        <div style={{ fontSize: 11, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 8 }}>Recent</div>
        {recent === null ? <div style={{ color: 'var(--text-muted)' }}>Loading…</div>
          : recent.length === 0 ? <div style={{ color: 'var(--text-muted)', fontSize: 12 }}>None.</div>
          : (
            <div style={{ maxHeight: 300, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 4 }}>
              {recent.map((r, i) => (
                <div key={i} style={{ display: 'grid', gridTemplateColumns: 'auto auto 1fr auto', gap: 10, padding: '6px 10px', background: 'var(--bg-tertiary)', borderRadius: 4, fontSize: 11, fontFamily: 'var(--font-mono)' }}>
                  <span style={{ color: 'var(--text-muted)' }}>{new Date(r.timestamp).toLocaleString()}</span>
                  <span style={{ color: 'var(--accent-red)', fontWeight: 600 }}>{r.errorType}</span>
                  <span style={{ color: 'var(--text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={r.url}>{r.url}</span>
                  <span style={{ color: 'var(--text-muted)' }}>attempt {r.attempt}/{r.maxAttempts} · retry {r.retryMs}ms</span>
                </div>
              ))}
            </div>
          )}
      </div>
    </div>
  )
}

/** Simple SVG bar chart of error count per period bucket. Buckets are
 *  re-grouped to local timezone using the same logic as UsageChart so
 *  labels match the user's wall clock. */
function ErrorsBarChart({ points, period }: { points: ErrorPoint[]; period: 'hour' | 'day' }) {
  // Convert UTC bucket keys to local-day labels.
  const fmtLabel = (key: string): string => {
    if (period === 'hour') {
      const [d, h] = key.split(' ')
      const dt = new Date(`${d}T${h ?? '00'}:00:00Z`)
      return `${String(dt.getHours()).padStart(2, '0')}:00`
    }
    const dt = new Date(`${key}T00:00:00Z`)
    return `${String(dt.getMonth() + 1).padStart(2, '0')}.${String(dt.getDate()).padStart(2, '0')}`
  }
  const max = Math.max(1, ...points.map(p => p.count))
  const W = 720, H = 200, pL = 40, pR = 12, pT = 10, pB = 28
  const cW = W - pL - pR
  const cH = H - pT - pB
  const barW = Math.max(2, cW / points.length - 2)
  const stepX = cW / Math.max(1, points.length)
  const yAt = (v: number) => pT + cH - (v / max) * cH

  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{ width: '100%', display: 'block' }}>
      {[0, 0.25, 0.5, 0.75, 1].map((t, i) => (
        <g key={i}>
          <line x1={pL} y1={yAt(max * t)} x2={W - pR} y2={yAt(max * t)} stroke="var(--border-primary)" stroke-width="0.5" opacity="0.4" />
          <text x={pL - 6} y={yAt(max * t) + 3} text-anchor="end" font-size="9" fill="var(--text-muted)" font-family="var(--font-mono)">{Math.round(max * t)}</text>
        </g>
      ))}
      {points.map((p, i) => {
        const x = pL + i * stepX
        const h = (p.count / max) * cH
        const y = pT + cH - h
        return (
          <g key={i}>
            <rect x={x} y={y} width={barW} height={h} fill="var(--accent-red)" rx="1">
              <title>{`${fmtLabel(p.period)} — ${p.count} errors (CR:${p.connectionRefused}, TO:${p.timeouts}, 429:${p.rateLimited})`}</title>
            </rect>
            {i % Math.max(1, Math.floor(points.length / 8)) === 0 && (
              <text x={x + barW / 2} y={H - 8} text-anchor="middle" font-size="9" fill="var(--text-muted)">{fmtLabel(p.period)}</text>
            )}
          </g>
        )
      })}
    </svg>
  )
}
