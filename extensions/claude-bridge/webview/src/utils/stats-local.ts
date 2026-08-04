// Local-timezone aggregation for /api/dashboard/stats/overview hourly[].
//
// Server returns raw per-UTC-hour buckets keyed by 'YYYY-MM-DDTHH' with a
// per-model split. We re-bucket here using the user's browser locale so
// heatmap, peakHour, longestStreak and the daily models timeseries all
// reflect the user's wall clock.
//
// We request the compact `agg=hm` mode: buckets carry no sessionId (that
// per-session split ballooned the payload into megabytes on multi-user
// servers) — the per-day distinct session count arrives pre-computed in
// `dailySessions` instead, bucketed by OUR tz (sent as the getTimezoneOffset
// value). Legacy responses with sessionId still aggregate correctly.

export interface HourlyBucket {
  utcHour: string                 // 'YYYY-MM-DDTHH' interpreted as UTC
  model: string | null
  sessionId?: string              // legacy (non-compact) responses only
  userMessages: number
  assistantMessages: number
  tokens: number
}

export interface DailySessions {
  date: string                    // local YYYY-MM-DD (server bucketed by our tz)
  sessions: number
}

export interface DayStats {
  date: string                    // local YYYY-MM-DD
  userMessages: number
  assistantMessages: number
  tokens: number
  sessions: number                // distinct session_ids that touched this local day
  models: Record<string, number>  // tokens per model on this day
}

export interface LocalAggregates {
  heatmap: DayStats[]             // sorted ASC by date; one entry per active local day
  peakHour: number | null         // 0..23 in local time (by user msgs)
  longestStreak: number
  activeDays: number
  modelsTimeseries: Array<{ date: string; series: Record<string, number> }>
}

function parseUtcHour(s: string): Date {
  return new Date(s + ':00:00.000Z')
}

function fmtLocalDay(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

export function aggregateLocal(hourly: HourlyBucket[], dailySessions?: DailySessions[]): LocalAggregates {
  // Per-day aggregator. In compact mode the per-day session count comes
  // from `dailySessions`; legacy responses carry sessionId per bucket and
  // we keep a Set for the distinct count.
  interface InternalDay extends Omit<DayStats, 'sessions'> {
    sessionSet: Set<string>
  }
  const byDay = new Map<string, InternalDay>()
  const userByLocalHour = new Array<number>(24).fill(0)

  for (const b of hourly) {
    const d = parseUtcHour(b.utcHour)
    const localDay = fmtLocalDay(d)
    const localHour = d.getHours()
    userByLocalHour[localHour] += b.userMessages

    let slot = byDay.get(localDay)
    if (!slot) {
      slot = {
        date: localDay,
        userMessages: 0,
        assistantMessages: 0,
        tokens: 0,
        models: {},
        sessionSet: new Set<string>(),
      }
      byDay.set(localDay, slot)
    }
    slot.userMessages += b.userMessages
    slot.assistantMessages += b.assistantMessages
    slot.tokens += b.tokens
    if (b.sessionId) slot.sessionSet.add(b.sessionId)
    if (b.model && b.tokens > 0) {
      slot.models[b.model] = (slot.models[b.model] ?? 0) + b.tokens
    }
  }

  const sessionsByDay = new Map((dailySessions ?? []).map(d => [d.date, d.sessions]))
  const heatmap: DayStats[] = Array.from(byDay.values())
    .map(d => ({
      date: d.date,
      userMessages: d.userMessages,
      assistantMessages: d.assistantMessages,
      tokens: d.tokens,
      models: d.models,
      sessions: sessionsByDay.get(d.date) ?? d.sessionSet.size,
    }))
    .sort((a, b) => a.date.localeCompare(b.date))

  let peakHour: number | null = null
  let peakCnt = 0
  for (let h = 0; h < 24; h++) {
    if (userByLocalHour[h]! > peakCnt) {
      peakCnt = userByLocalHour[h]!
      peakHour = h
    }
  }

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

  return {
    heatmap,
    peakHour,
    longestStreak: longest,
    activeDays: heatmap.length,
    modelsTimeseries,
  }
}
