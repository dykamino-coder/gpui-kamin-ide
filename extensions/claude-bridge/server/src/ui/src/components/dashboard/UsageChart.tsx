import { signal } from '@preact/signals'
import { useEffect, useRef } from 'preact/hooks'
import { api } from '../../services/api-client'
import { userColor } from '../../utils/user-colors'
import styles from './UsageChart.module.css'

// ── Types ──────────────────────────────────────────────────────────────────

interface TimeSeriesPoint {
  userName: string
  period: string
  requests: number
  inputTokens: number
  outputTokens: number
}

interface UserSeries {
  userName: string
  color: string
  values: number[]
}

interface HoverData {
  slotIdx: number
  xPx: number       // pixel x in container coords
  label: string
  activeUser: string // closest line
  items: { name: string; color: string; value: number }[]
}

// ── Config ─────────────────────────────────────────────────────────────────

type Tab = 'daily' | 'weekly' | 'monthly' | 'yearly'

const TABS: { key: Tab; label: string }[] = [
  { key: 'daily', label: 'Daily' },
  { key: 'weekly', label: 'Weekly' },
  { key: 'monthly', label: 'Monthly' },
  { key: 'yearly', label: 'Yearly' },
]

// ── State ──────────────────────────────────────────────────────────────────

const activeTab = signal<Tab>('daily')
const rawData = signal<TimeSeriesPoint[]>([])
const loading = signal(false)
const hover = signal<HoverData | null>(null)

// ── Data fetching ──────────────────────────────────────────────────────────

function apiParams(tab: Tab): { period: string; days: number } {
  switch (tab) {
    case 'daily': return { period: 'hour', days: 1 }
    case 'weekly': return { period: 'day', days: 7 }
    case 'monthly': return { period: 'day', days: 30 }
    // 12 monthly buckets. 400 days (not 365) so the OLDEST bucket — up to ~365
    // days back at its first day — is fully inside the query window, not clipped.
    case 'yearly': return { period: 'month', days: 400 }
  }
}

async function fetchData(tab: Tab) {
  const { period, days } = apiParams(tab)
  loading.value = true
  try {
    rawData.value = await api.getUserTimeSeries(period, days)
  } catch { rawData.value = [] }
  loading.value = false
}

// ── Slot generation ────────────────────────────────────────────────────────

function generateSlots(tab: Tab): string[] {
  const now = new Date()
  const slots: string[] = []

  // Generate slots in UTC to match backend strftime(..., 'localtime') which is UTC in Docker
  if (tab === 'daily') {
    for (let i = 23; i >= 0; i--) {
      const d = new Date(now.getTime() - i * 3600000)
      const y = d.getUTCFullYear()
      const m = String(d.getUTCMonth() + 1).padStart(2, '0')
      const day = String(d.getUTCDate()).padStart(2, '0')
      const h = String(d.getUTCHours()).padStart(2, '0')
      slots.push(`${y}-${m}-${day} ${h}`)
    }
  } else if (tab === 'weekly') {
    for (let i = 6; i >= 0; i--) {
      const d = new Date(now.getTime() - i * 86400000)
      const y = d.getUTCFullYear()
      const m = String(d.getUTCMonth() + 1).padStart(2, '0')
      const day = String(d.getUTCDate()).padStart(2, '0')
      slots.push(`${y}-${m}-${day}`)
    }
  } else if (tab === 'monthly') {
    for (let i = 29; i >= 0; i--) {
      const d = new Date(now.getTime() - i * 86400000)
      const y = d.getUTCFullYear()
      const m = String(d.getUTCMonth() + 1).padStart(2, '0')
      const day = String(d.getUTCDate()).padStart(2, '0')
      slots.push(`${y}-${m}-${day}`)
    }
  } else {
    // Yearly: 12 monthly buckets `YYYY-MM`, matching the backend's month
    // granularity (aggregates.ts periodOf → `${y}-${m}`). Step by calendar month
    // (not fixed ms) so month lengths don't drift the bucket boundaries.
    const y = now.getUTCFullYear()
    const mo = now.getUTCMonth()
    for (let i = 11; i >= 0; i--) {
      const d = new Date(Date.UTC(y, mo - i, 1))
      const yy = d.getUTCFullYear()
      const mm = String(d.getUTCMonth() + 1).padStart(2, '0')
      slots.push(`${yy}-${mm}`)
    }
  }
  return slots
}

function formatSlotLabel(slot: string, tab: Tab): string {
  if (tab === 'daily') {
    // Slot is UTC "YYYY-MM-DD HH", convert to local time for display
    const d = new Date(slot.replace(' ', 'T') + ':00:00Z')
    return `${String(d.getHours()).padStart(2, '0')}:00`
  }
  if (tab === 'weekly') {
    const d = new Date(slot + 'T00:00:00Z')
    return ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'][d.getDay()] || slot
  }
  const parts = slot.split('-')
  const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec']
  // Yearly slot is `YYYY-MM` (no day) → month + 2-digit year, e.g. "Jul '25".
  if (tab === 'yearly') {
    return `${months[parseInt(parts[1]!, 10) - 1]} '${parts[0]!.slice(2)}`
  }
  return `${months[parseInt(parts[1]!, 10) - 1]} ${parseInt(parts[2]!, 10)}`
}

// ── SVG helpers ────────────────────────────────────────────────────────────

function buildSmoothPath(pts: { x: number; y: number }[], yFloor: number): string {
  if (pts.length === 0) return ''
  if (pts.length === 1) return `M${pts[0]!.x},${pts[0]!.y}`

  let d = `M${pts[0]!.x},${pts[0]!.y}`
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[Math.max(i - 1, 0)]!
    const p1 = pts[i]!
    const p2 = pts[i + 1]!
    const p3 = pts[Math.min(i + 2, pts.length - 1)]!
    const t = 0.3
    const cp1y = Math.min(p1.y + (p2.y - p0.y) * t, yFloor)
    const cp2y = Math.min(p2.y - (p3.y - p1.y) * t, yFloor)
    d += ` C${p1.x + (p2.x - p0.x) * t},${cp1y} ${p2.x - (p3.x - p1.x) * t},${cp2y} ${p2.x},${p2.y}`
  }
  return d
}

function niceScale(maxVal: number): number[] {
  if (maxVal <= 0) return [0]
  const rough = maxVal / 4
  const pow = Math.pow(10, Math.floor(Math.log10(rough || 1)))
  const norm = rough / pow
  const step = norm <= 1 ? pow : norm <= 2 ? 2 * pow : norm <= 5 ? 5 * pow : 10 * pow
  const ticks: number[] = []
  for (let v = 0; v <= maxVal + step * 0.5; v += step) ticks.push(Math.round(v))
  if (ticks.length < 2) ticks.push(Math.round(step))
  // Ensure top tick is strictly above maxVal so data doesn't clip the top edge
  if (ticks[ticks.length - 1]! <= maxVal) ticks.push(ticks[ticks.length - 1]! + Math.round(step))
  return ticks
}

function fmtNum(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

// ── Component ──────────────────────────────────────────────────────────────

export function UsageChart() {
  const svgRef = useRef<SVGSVGElement>(null)

  useEffect(() => { fetchData(activeTab.value) }, [])

  const tab = activeTab.value
  const data = rawData.value
  const isLoading = loading.value

  function setTab(t: Tab) {
    activeTab.value = t
    hover.value = null
    fetchData(t)
  }

  // Prepare series
  const slots = generateSlots(tab)
  const users = [...new Set(data.map(d => d.userName))]

  // Plot NEW tokens produced in each period — input + cache_write
  // (server side excludes cache_read) plus output. This reflects actual
  // content the user drove through the API in that hour, not cumulative
  // re-sent context.
  const series: UserSeries[] = users.map((name) => {
    const byPeriod = new Map<string, number>()
    for (const pt of data) {
      if (pt.userName === name) {
        byPeriod.set(pt.period, (byPeriod.get(pt.period) || 0) + pt.inputTokens + pt.outputTokens)
      }
    }
    return {
      userName: name,
      color: userColor(name),
      values: slots.map(s => byPeriod.get(s) || 0),
    }
  })

  const allValues = series.flatMap(s => s.values)
  const maxVal = Math.max(...allValues, 1)
  const yTicks = niceScale(maxVal)
  const yMax = yTicks[yTicks.length - 1] || maxVal

  // Chart geometry
  const W = 720
  const H = 200
  const pL = 40, pR = 12, pT = 10, pB = 24
  const cW = W - pL - pR
  const cH = H - pT - pB

  const xAt = (i: number) => slots.length <= 1 ? pL + cW / 2 : pL + (i / (slots.length - 1)) * cW
  const yAt = (v: number) => pT + cH - (v / yMax) * cH

  const xStep = Math.max(1, Math.ceil(slots.length / 8))

  // Pre-compute all point arrays
  const seriesPts = series.map(s => s.values.map((v, i) => ({ x: xAt(i), y: yAt(v) })))

  function onMouseMove(e: MouseEvent) {
    const svg = svgRef.current
    if (!svg || slots.length === 0) return
    const rect = svg.getBoundingClientRect()
    const scaleX = W / rect.width
    const scaleY = H / rect.height
    const mx = (e.clientX - rect.left) * scaleX
    const my = (e.clientY - rect.top) * scaleY

    // Nearest slot by X
    let nearest = 0, minD = Infinity
    for (let i = 0; i < slots.length; i++) {
      const d = Math.abs(xAt(i) - mx)
      if (d < minD) { minD = d; nearest = i }
    }

    // Find closest line by Y at this slot
    let closestUser = ''
    let closestDist = Infinity
    for (let si = 0; si < series.length; si++) {
      const py = seriesPts[si]![nearest]!.y
      const dist = Math.abs(py - my)
      if (dist < closestDist) {
        closestDist = dist
        closestUser = series[si]!.userName
      }
    }

    const items = series
      .map(s => ({ name: s.userName, color: s.color, value: s.values[nearest]! }))
      .sort((a, b) => b.value - a.value)

    hover.value = {
      slotIdx: nearest,
      xPx: (xAt(nearest) / W) * rect.width,
      label: formatSlotLabel(slots[nearest]!, tab),
      activeUser: closestUser,
      items,
    }
  }

  const h = hover.value

  return (
    <div class={styles.container}>
      <div class={styles.header}>
        <div class={styles.titleRow}>
          <i class={`fa-solid fa-chart-line ${styles.titleIcon}`} />
          <span class={styles.title}>Usage</span>
        </div>
        <div class={styles.periodTabs}>
          {TABS.map(t => (
            <button
              key={t.key}
              class={`${styles.periodTab} ${tab === t.key ? styles.active : ''}`}
              onClick={() => setTab(t.key)}
            >
              {t.label}
            </button>
          ))}
        </div>
      </div>

      <div class={styles.chartArea}>
        {isLoading ? (
          <div class={styles.loading}><i class="fa-solid fa-spinner fa-spin" /></div>
        ) : series.length === 0 ? (
          <div class={styles.empty}><span>No usage data</span></div>
        ) : (
          <>
          <div class={styles.chartWrap}>
            <svg
              ref={svgRef}
              viewBox={`0 0 ${W} ${H}`}
              style={{ width: '100%', display: 'block' }}
              onMouseMove={onMouseMove}
              onMouseLeave={() => { hover.value = null }}
            >
              <defs>
                {series.map((s, i) => (
                  <linearGradient key={i} id={`ug${i}`} x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stop-color={s.color} stop-opacity="0.25" />
                    <stop offset="100%" stop-color={s.color} stop-opacity="0" />
                  </linearGradient>
                ))}
                {/* Glow filter for active line */}
                <filter id="lineGlow" x="-20%" y="-20%" width="140%" height="140%">
                  <feGaussianBlur stdDeviation="3" result="blur" />
                  <feMerge>
                    <feMergeNode in="blur" />
                    <feMergeNode in="SourceGraphic" />
                  </feMerge>
                </filter>
              </defs>

              {/* Grid lines */}
              {yTicks.map(t => (
                <g key={t}>
                  <line x1={pL} y1={yAt(t)} x2={W - pR} y2={yAt(t)}
                    stroke="var(--border-primary)" stroke-width="0.5" opacity="0.6" />
                  <text x={pL - 6} y={yAt(t) + 3.5} text-anchor="end"
                    font-size="9" fill="var(--text-muted)" font-family="var(--font-mono)">
                    {fmtNum(t)}
                  </text>
                </g>
              ))}

              {/* X labels */}
              {slots.map((s, i) => {
                if (i % xStep !== 0 && i !== slots.length - 1) return null
                return (
                  <text key={s} x={xAt(i)} y={H - 4} text-anchor="middle"
                    font-size="9" fill="var(--text-muted)">
                    {formatSlotLabel(s, tab)}
                  </text>
                )
              })}

              {/* Areas + lines + dots per series */}
              {series.map((s, si) => {
                const pts = seriesPts[si]!
                const linePath = buildSmoothPath(pts, pT + cH)
                const areaPath = pts.length > 0
                  ? linePath + ` L${pts[pts.length - 1]!.x},${pT + cH} L${pts[0]!.x},${pT + cH} Z`
                  : ''

                const isActive = h?.activeUser === s.userName
                const isDimmed = h && !isActive

                return (
                  <g key={s.userName} style={{ transition: 'opacity 0.15s ease' }}
                    opacity={isDimmed ? 0.25 : 1}>
                    {/* Area fill */}
                    <path d={areaPath} fill={`url(#ug${si})`}
                      opacity={isActive ? 1 : 0.7} />
                    {/* Line — glow when active */}
                    <path d={linePath} fill="none"
                      stroke={s.color}
                      stroke-width={isActive ? 3 : 2}
                      stroke-linecap="round" stroke-linejoin="round"
                      filter={isActive ? 'url(#lineGlow)' : undefined} />
                    {/* Dots */}
                    {pts.map((p, i) => (
                      <circle key={i} cx={p.x} cy={p.y}
                        r={isActive && h?.slotIdx === i ? 5 : 3}
                        fill={isActive && h?.slotIdx === i ? s.color : 'var(--bg-secondary)'}
                        stroke={s.color}
                        stroke-width={isActive ? 2 : 1.5} />
                    ))}
                  </g>
                )
              })}

              {/* Hover vertical line */}
              {h && (
                <line x1={xAt(h.slotIdx)} y1={pT} x2={xAt(h.slotIdx)} y2={pT + cH}
                  stroke="var(--text-muted)" stroke-width="0.5" stroke-dasharray="3,3" opacity="0.5" />
              )}

              {/* Active user name label on the line */}
              {h && (() => {
                const si = series.findIndex(s => s.userName === h.activeUser)
                if (si < 0) return null
                const pt = seriesPts[si]![h.slotIdx]!
                const color = series[si]!.color
                const value = series[si]!.values[h.slotIdx]!
                const pillW = h.activeUser.length * 6.5 + 40
                const gap = 8
                // Flip to left side if pill would overflow right edge
                const drawLeft = pt.x + gap + pillW > W - pR
                const pillX = drawLeft ? pt.x - gap - pillW : pt.x + gap
                const textX = pillX + 6
                return (
                  <g>
                    <rect x={pillX} y={pt.y - 11} width={pillW} height={18}
                      rx="4" fill="var(--bg-elevated)" stroke={color} stroke-width="0.5" opacity="0.95" />
                    <text x={textX} y={pt.y + 1} font-size="10" font-weight="600" fill={color}>
                      {h.activeUser}
                      <tspan fill="var(--text-primary)" font-weight="700" dx="6">{value}</tspan>
                    </text>
                  </g>
                )
              })()}
            </svg>

            {/* Legend */}
            <div class={styles.legend}>
              {series.map(s => {
                const total = s.values.reduce((a, b) => a + b, 0)
                const isActive = h?.activeUser === s.userName
                return (
                  <div key={s.userName} class={styles.legendItem}
                    style={{ opacity: h && !isActive ? 0.4 : 1 }}>
                    <span class={styles.legendDot} style={{ background: s.color }} />
                    <span class={styles.legendName}>{s.userName}</span>
                    <span class={styles.legendValue}>{fmtNum(total)} tokens</span>
                  </div>
                )
              })}
            </div>
          </div>
          </>
        )}
      </div>
    </div>
  )
}
