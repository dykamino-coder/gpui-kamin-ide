// Stats panel — usage overview rendered in the centre of CustomizeContent.
// Fetches /api/dashboard/stats/overview from the active server using the
// stored serverUrl + token (token isn't required for dashboard endpoints
// at the moment but pass it through so a future auth-on flip just works).

import type { JSX } from 'preact'
import { useEffect, useMemo, useState } from 'preact/hooks'
import { CardTabBar } from '../../right-panel/CardTabBar'
import { useBridge } from '../../../hooks/useBridge'
import { readPanelCache, writePanelCache } from '../../../lib/panel-cache'
import { serverFetch } from '../../../lib/server-fetch'
import { PanelError } from '../../ui/PanelError'
import { aggregateLocal, type DailySessions, type HourlyBucket, type DayStats } from '../../../utils/stats-local'

type Range = 'all' | '30d' | '7d'

interface ModelTotal { name: string; in: number; out: number; pct: number }

interface OverviewRaw {
  range: Range
  sessions: number
  userMessages: number
  assistantMessages: number
  totalTokens: number
  favoriteModel: string | null
  models?: ModelTotal[]
  hourly: HourlyBucket[]
  /** compact `agg=hm` mode: exact per-local-day distinct session counts. */
  dailySessions?: DailySessions[]
}

// Fun "you've used X× more than" comparisons. Each entry is a rough
// whole-text token estimate (~1.2× word count for English). Picked at
// random per panel mount so the stat doesn't get stale on re-visits.
// Numbers are public-domain word-count guesses + tokenizer estimate;
// not a precise tokenization run, so the suffix "≈" is implied.
const TOKEN_COMPARISONS: Array<{ name: string; tokens: number }> = [
  { name: 'The Lord of the Rings (trilogy)', tokens: 576_000 },
  { name: 'War and Peace', tokens: 690_000 },
  { name: 'the King James Bible', tokens: 1_050_000 },
  { name: 'all 7 Harry Potter books', tokens: 1_400_000 },
  { name: 'A Song of Ice and Fire (5 books)', tokens: 2_100_000 },
  { name: 'Don Quixote', tokens: 600_000 },
  { name: 'James Joyce\'s Ulysses', tokens: 320_000 },
  { name: 'Moby-Dick', tokens: 250_000 },
  { name: 'Atlas Shrugged', tokens: 730_000 },
  { name: 'Pride and Prejudice', tokens: 160_000 },
]

function pickComparison(): { name: string; tokens: number } {
  return TOKEN_COMPARISONS[Math.floor(Math.random() * TOKEN_COMPARISONS.length)]!
}

function formatNum(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + 'K'
  return n.toLocaleString()
}

function formatHour(h: number | null): string {
  if (h == null) return '—'
  return `${String(h).padStart(2, '0')}:00`
}

function shortModel(m: string | null): string {
  if (!m) return '—'
  // claude-opus-5 → Opus 5; claude-opus-4-8-20250101 → Opus 4.8 (старые логи)
  const match = m.match(/claude-(opus|sonnet|haiku|fable)-(\d+)-?(\d+)?/i)
  if (match) {
    const family = match[1]!.charAt(0).toUpperCase() + match[1]!.slice(1).toLowerCase()
    const ver = match[3] ? `${match[2]}.${match[3]}` : match[2]
    return `${family} ${ver}`
  }
  return m
}

function buildWeekGrid(heatmap: DayStats[], weeks: number): {
  cells: Array<DayStats | null>
  cols: number
} {
  const today = new Date(); today.setHours(0, 0, 0, 0)
  const todayDow = (today.getDay() + 6) % 7 // Mon=0…Sun=6
  const totalCells = weeks * 7
  const startDate = new Date(today.getTime() - (totalCells - 1 - (6 - todayDow)) * 86_400_000)
  const map = new Map(heatmap.map(d => [d.date, d]))
  // Use local day key (matches aggregateLocal output).
  const fmtLocal = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  const cells: Array<DayStats | null> = []
  for (let i = 0; i < totalCells; i++) {
    const d = new Date(startDate.getTime() + i * 86_400_000)
    if (d > today) { cells.push(null); continue }
    const key = fmtLocal(d)
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

export function StatsPanel(): JSX.Element {
  const bridge = useBridge()
  const [data, setData] = useState<OverviewRaw | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [range, setRange] = useState<Range>('all')
  const [tab, setTab] = useState<'overview' | 'models'>('overview')
  const [hover, setHover] = useState<{ x: number; y: number; label: string } | null>(null)
  // Pick a random comparison once per panel mount. New roll on next open.
  const [comparison] = useState(pickComparison)
  // Retry: bumping the counter re-runs the fetch effect.
  const [reload, setReload] = useState(0)

  useEffect(() => {
    let cancelled = false
    setError(null)
    // Cache-first: paint the last-known overview for this range instantly
    // (heatmap + metrics), then revalidate against the server in the
    // background. A cold open no longer blanks to "Loading…" every time.
    const cacheKey = `stats.overview.v1.${range}`
    const cached = readPanelCache<OverviewRaw>(cacheKey)
    if (cached) { setData(cached); setLoading(false) } else { setData(null); setLoading(true) }
    ;(async () => {
      try {
        const cfg = await bridge.getConfig()
        if (!cfg?.serverUrl) { if (!cached) setError('No server configured'); setLoading(false); return }
        const httpUrl = cfg.serverUrl.replace(/^ws/, 'http').replace(/\/ws\/session$/, '')
        const headers: Record<string, string> = {}
        if (cfg.token) headers['Authorization'] = `Bearer ${cfg.token}`
        // Compact aggregation (`agg=hm`) keeps the payload small on busy
        // multi-user servers; tz = getTimezoneOffset so the server buckets
        // the per-day session counts by OUR local day.
        const res = await serverFetch(bridge, httpUrl, `/api/dashboard/stats/overview?range=${range}&agg=hm&tz=${new Date().getTimezoneOffset()}`, { headers })
        if (!res.ok) { if (!cached) setError(res.error || `Server returned ${res.status}`); setLoading(false); return }
        const payload = res.data as OverviewRaw
        if (!cancelled) { setData(payload); writePanelCache(cacheKey, payload); setLoading(false) }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e))
          setLoading(false)
        }
      }
    })()
    return () => { cancelled = true }
  }, [range, reload])

  // Local-time aggregation — heatmap, peakHour, longestStreak, activeDays,
  // modelsTimeseries are computed from the raw UTC hourly buckets in the
  // user's browser/Electron locale.
  const local = useMemo(() => data ? aggregateLocal(data.hourly, data.dailySessions) : null, [data])

  const weeks = range === '7d' ? 5 : range === '30d' ? 8 : 26
  const grid = local ? buildWeekGrid(local.heatmap, weeks) : null
  const max = grid ? Math.max(...grid.cells.map(c => c?.userMessages ?? 0)) : 0
  const comparisonMul = data && data.totalTokens > 0 ? data.totalTokens / comparison.tokens : 0

  const cellSize = 14
  const cellGap = 3

  return (
    <div style="flex:1;display:flex;align-items:center;justify-content:center;padding:24px;overflow:auto">
      <div style="
        width:660px;max-width:100%;background:var(--bg-mantle);border:1px solid var(--bg-surface);
        border-radius:var(--radius-lg);padding:18px;box-shadow:var(--shadow-card);
        font-variant-numeric:tabular-nums;position:relative
      ">
        {/* Header */}
        <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:14px;gap:12px">
          <CardTabBar
            tabs={[
              { id: 'overview', label: 'Overview', icon: 'fa-calendar' },
              { id: 'models', label: 'Models', icon: 'fa-robot' },
            ]}
            active={tab}
            onSelect={(id) => setTab(id as 'overview' | 'models')}
          />
          <div style="display:flex;gap:6px">
            <RangeBtn active={range === 'all'} onClick={() => setRange('all')}>All</RangeBtn>
            <RangeBtn active={range === '30d'} onClick={() => setRange('30d')}>30d</RangeBtn>
            <RangeBtn active={range === '7d'} onClick={() => setRange('7d')}>7d</RangeBtn>
          </div>
        </div>

        {loading && (
          <div style="padding:40px;text-align:center;color:var(--text-muted);font-size:12px">Loading…</div>
        )}
        {error && !loading && (
          // A failed load gets the shared "Couldn't load" surface with Retry —
          // the bare one-line error read like a broken panel, not like an
          // unreachable server.
          <PanelError message={error} onRetry={() => { setReload((n) => n + 1) }} />
        )}

        {data && local && !loading && tab === 'overview' && (
          <>
            <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:10px">
              <Metric label="Sessions" value={formatNum(data.sessions)} />
              <Metric label="User msg" value={formatNum(data.userMessages)} />
              <Metric label="Assistant msg" value={formatNum(data.assistantMessages)} />
              <Metric label="Session tokens" value={formatNum(data.totalTokens)} />
            </div>
            <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:14px">
              <Metric label="Active days" value={String(local.activeDays)} />
              <Metric label="Longest streak" value={`${local.longestStreak}d`} />
              <Metric label="Peak hour" value={formatHour(local.peakHour)} />
              <Metric label="Favorite model" value={shortModel(data.favoriteModel)} />
            </div>

            {/* Heatmap */}
            {grid && (
              <div style="position:relative">
                <div style={`display:grid;grid-template-columns:repeat(${grid.cols},${cellSize}px);grid-auto-flow:column;grid-template-rows:repeat(7,${cellSize}px);gap:${cellGap}px`}>
                  {grid.cells.map((cell, i) => {
                    if (!cell) return <div key={i} style={`width:${cellSize}px;height:${cellSize}px`} />
                    const lvl = intensityClass(cell.userMessages, max)
                    const bg = lvl === 0
                      ? 'var(--bg-surface)'
                      : `color-mix(in srgb, var(--accent-action) ${[0, 22, 45, 70, 100][lvl]}%, var(--bg-surface))`
                    return (
                      <div
                        key={i}
                        style={`width:${cellSize}px;height:${cellSize}px;background:${bg};border-radius:3px;cursor:pointer`}
                        onMouseEnter={(e) => {
                          const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
                          const modelsLine = Object.entries(cell.models)
                            .sort((a, b) => b[1] - a[1])
                            .slice(0, 3)
                            .map(([m, t]) => `${shortModel(m)}: ${formatNum(t)}`)
                            .join(' · ')
                          const lines = [
                            formatDate(cell.date),
                            `Sessions ${cell.sessions} · User ${cell.userMessages} · Asst ${cell.assistantMessages}`,
                            `Tokens ${formatNum(cell.tokens)}`,
                            modelsLine,
                          ].filter(Boolean)
                          setHover({
                            x: r.left + r.width / 2,
                            y: r.top - 6,
                            label: lines.join('\n'),
                          })
                        }}
                        onMouseLeave={() => setHover(null)}
                      />
                    )
                  })}
                </div>
              </div>
            )}

            {comparisonMul > 0 && (
              <div style="margin-top:14px;padding-top:10px;border-top:1px solid var(--bg-surface);font-size:11px;color:var(--text-muted)">
                You've used {comparisonMul >= 1
                  ? `~${comparisonMul.toFixed(comparisonMul >= 10 ? 0 : 1)}× more tokens than ${comparison.name}`
                  : `about ${(comparisonMul * 100).toFixed(0)}% of ${comparison.name}`}.
              </div>
            )}
          </>
        )}

        {data && local && !loading && tab === 'models' && (
          <ModelsView models={data.models ?? []} timeseries={local.modelsTimeseries} onHover={setHover} />
        )}
      </div>
      {hover && (
        <div
          style={`position:fixed;left:${hover.x}px;top:${hover.y}px;transform:translate(-50%,-100%);background:var(--bg-sidebar);border:1px solid var(--bg-surface);border-radius:var(--radius-xs);padding:4px 10px;font-size:11px;line-height:1.45;color:var(--text-primary);white-space:pre;pointer-events:none;box-shadow:var(--shadow-mini);z-index:9999`}
        >
          {hover.label}
        </div>
      )}
    </div>
  )
}

function Metric({ label, value }: { label: string; value: string }): JSX.Element {
  return (
    <div style="background:var(--bg-base);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);padding:10px 12px">
      <div style="font-size:11px;color:var(--text-muted);margin-bottom:2px">{label}</div>
      <div style="font-size:16px;font-weight:700;color:var(--text-primary)">{value}</div>
    </div>
  )
}

function RangeBtn({ active, onClick, children }: { active: boolean; onClick: () => void; children: any }): JSX.Element {
  return (
    <button
      type="button"
      onClick={onClick}
      style={`
        padding:5px 12px;border-radius:var(--radius-xs);border:none;cursor:pointer;font-size:12px;font-weight:600;
        background:${active ? 'var(--bg-surface)' : 'transparent'};
        color:${active ? 'var(--text-primary)' : 'var(--text-muted)'};
      `}
    >{children}</button>
  )
}

function formatDate(iso: string): string {
  const d = new Date(iso)
  const month = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'][d.getMonth()]
  return `${month} ${d.getDate()}`
}

interface ModelsViewProps {
  models: ModelTotal[]
  timeseries: Array<{ date: string; series: Record<string, number> }>
  onHover: (h: { x: number; y: number; label: string } | null) => void
}

/** Models tab — stacked bar chart by day + per-model totals list.
 *  Each model gets a deterministic shade derived from its position in
 *  the totals list (top model = strongest accent-action, tail models =
 *  faded mixes towards bg-surface). */
function ModelsView({ models, timeseries, onHover }: ModelsViewProps): JSX.Element {
  if (models.length === 0 || timeseries.length === 0) {
    return (
      <div style="padding:30px;text-align:center;color:var(--text-muted);font-size:12px">
        No model usage recorded yet.
      </div>
    )
  }

  // Color scheme — top model gets full accent, others fade towards bg.
  const colorFor = (idx: number): string => {
    const pct = Math.max(15, 100 - idx * 18)
    return `color-mix(in srgb, var(--accent-action) ${pct}%, var(--bg-surface))`
  }

  // Build stacked bars data
  const W = 600
  const H = 200
  const PAD_L = 50
  const PAD_R = 8
  const PAD_T = 10
  const PAD_B = 24
  const innerW = W - PAD_L - PAD_R
  const innerH = H - PAD_T - PAD_B

  const dayTotals = timeseries.map(d => ({
    date: d.date,
    total: Object.values(d.series).reduce((s, v) => s + v, 0),
    series: d.series,
  }))
  const maxTotal = Math.max(1, ...dayTotals.map(d => d.total))
  // Round max up to a "nice" number for axis ticks
  const niceMax = niceCeil(maxTotal)
  const ticks = [0, niceMax * 0.25, niceMax * 0.5, niceMax * 0.75, niceMax]

  const barW = Math.max(2, innerW / dayTotals.length - 2)
  const stepX = innerW / dayTotals.length

  // X-axis labels — pick ~6 evenly spaced months
  const monthLabels: Array<{ x: number; label: string }> = []
  let lastMonth = -1
  dayTotals.forEach((d, i) => {
    const date = new Date(d.date)
    const m = date.getMonth()
    if (m !== lastMonth && i % Math.max(1, Math.floor(dayTotals.length / 6)) === 0) {
      lastMonth = m
      const label = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'][m] + ' ' + date.getDate()
      monthLabels.push({ x: PAD_L + i * stepX + barW / 2, label })
    }
  })

  return (
    <div>
      <svg width={W} height={H} style="display:block;margin:0 auto">
        {/* Y-axis grid + labels */}
        {ticks.map((t, i) => {
          const y = PAD_T + innerH - (t / niceMax) * innerH
          return (
            <g key={i}>
              <line x1={PAD_L} y1={y} x2={W - PAD_R} y2={y} stroke="var(--bg-surface)" stroke-width="1" />
              <text x={PAD_L - 6} y={y + 3} text-anchor="end" font-size="10" fill="var(--text-muted)" font-family="var(--font-sans)">
                {formatNum(t)}
              </text>
            </g>
          )
        })}
        {/* Stacked bars */}
        {dayTotals.map((d, i) => {
          let yAcc = 0
          const x = PAD_L + i * stepX
          return (
            <g key={d.date}>
              {models.map((m, mi) => {
                const v = d.series[m.name] ?? 0
                if (v <= 0) return null
                const h = (v / niceMax) * innerH
                const y = PAD_T + innerH - yAcc - h
                yAcc += h
                return (
                  <rect
                    key={m.name}
                    x={x}
                    y={y}
                    width={barW}
                    height={h}
                    fill={colorFor(mi)}
                    rx="1"
                    style="cursor:pointer"
                    onMouseEnter={(e: any) => {
                      const r = (e.currentTarget as SVGElement).getBoundingClientRect()
                      onHover({
                        x: r.left + r.width / 2,
                        y: r.top - 6,
                        label: `${formatDate(d.date)} · ${shortModel(m.name)} ${formatNum(v)}`,
                      })
                    }}
                    onMouseLeave={() => onHover(null)}
                  />
                )
              })}
            </g>
          )
        })}
        {/* X-axis labels */}
        {monthLabels.map((l, i) => (
          <text key={i} x={l.x} y={H - 6} text-anchor="middle" font-size="10" fill="var(--text-muted)" font-family="var(--font-sans)">
            {l.label}
          </text>
        ))}
      </svg>

      {/* Legend / per-model totals */}
      <div style="margin-top:16px;display:flex;flex-direction:column;gap:6px">
        {models.map((m, i) => (
          <div key={m.name} style="display:grid;grid-template-columns:auto 1fr auto auto;gap:14px;align-items:center;font-size:12px">
            <span style="display:inline-flex;align-items:center;gap:8px;color:var(--text-primary)">
              <span style={`width:10px;height:10px;border-radius:2px;background:${colorFor(i)};display:inline-block;flex-shrink:0`} />
              <span style="font-weight:600">{shortModel(m.name)}</span>
            </span>
            <span />
            <span style="color:var(--text-muted);font-variant-numeric:tabular-nums">
              {formatNum(m.in)} in · {formatNum(m.out)} out
            </span>
            <span style="color:var(--text-primary);font-weight:700;min-width:50px;text-align:right;font-variant-numeric:tabular-nums">
              {m.pct.toFixed(1)}%
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function niceCeil(n: number): number {
  if (n <= 0) return 1
  const exp = Math.pow(10, Math.floor(Math.log10(n)))
  const mul = n / exp
  if (mul <= 1) return 1 * exp
  if (mul <= 2) return 2 * exp
  if (mul <= 5) return 5 * exp
  return 10 * exp
}
