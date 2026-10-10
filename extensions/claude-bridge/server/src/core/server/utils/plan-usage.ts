import type { PlanUsageData, UsageWindow } from '../../../shared/plan-usage'

const COMMON = ['five_hour', 'seven_day']
const MAX_WINDOWS = 16
export function emptyPlanUsage(reason = 'not-reported', now = new Date().toISOString()): PlanUsageData {
  return {
    windows: [],
    observedAt: now,
    attemptedAt: now,
    source: 'unavailable',
    claudeCodeVersion: null,
    state: 'unavailable',
    reason,
    diagnostics: { missing: [...COMMON], finalScreenLines: 0, outputLimited: false },
    extra: null,
    session: null,
    weekAll: null,
    weekSonnet: null,
    timestamp: now,
  }
}
function percent(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 100
}
function version(value: unknown): string | null {
  return typeof value === 'string' && /^\d+\.\d+\.\d+(?:[-+][\w.-]{1,32})?$/.test(value) ? value : null
}
export function parseStatusline(raw: unknown): { windows: UsageWindow[]; version: string | null } {
  if (!raw || typeof raw !== 'object') return { windows: [], version: null }
  const data = raw as Record<string, unknown>
  const observedAt =
    typeof data.observedAt === 'string' && Number.isFinite(Date.parse(data.observedAt))
      ? data.observedAt
      : new Date().toISOString()
  const limits =
    data.rate_limits && typeof data.rate_limits === 'object' ? (data.rate_limits as Record<string, unknown>) : {}
  const windows: UsageWindow[] = []
  for (const id of COMMON) {
    const rawWindow = limits[id]
    if (!rawWindow || typeof rawWindow !== 'object') continue
    const value = rawWindow as Record<string, unknown>
    if (
      !percent(value.used_percentage) ||
      typeof value.resets_at !== 'number' ||
      !Number.isSafeInteger(value.resets_at) ||
      value.resets_at <= 0 ||
      value.resets_at > 253402300799
    )
      continue
    if (value.resets_at * 1000 <= Date.now()) continue
    windows.push({
      id,
      label: id === 'five_hour' ? '5 hours' : 'Week (all)',
      percent: value.used_percentage,
      resetsAt: new Date(value.resets_at * 1000).toISOString(),
      resetText: null,
      observedAt,
      source: 'statusline',
      freshness: 'fresh',
      reason: null,
    })
  }
  return { windows, version: version(data.version) }
}

/** Input is the final visible terminal screen, never concatenated redraws. */
export function parseUsageScreen(screen: string, observedAt: string): PlanUsageData {
  const data = emptyPlanUsage('missing-common-window', observedAt)
  const lines = screen.split(/\r?\n/).map((line) => line.trim())
  data.diagnostics.finalScreenLines = Math.min(1000, lines.length)
  for (let i = 0; i < lines.length && data.windows.length < MAX_WINDOWS; i++) {
    const heading = lines[i]!.match(/^Current\s+(session|week)(?:\s*\(([^)\r\n]{1,64})\))?\s*$/i)
    if (!heading) continue
    let end = i + 1
    while (end < lines.length && !/^(?:Current\s+|Extra\s+usage|Esc\b)/i.test(lines[end]!)) end++
    const section = lines.slice(i + 1, end)
    const utilization = section.join('\n').match(/(?:^|\s)(\d+(?:\.\d+)?)%\s*used\b/i)
    if (!utilization || !percent(Number(utilization[1]))) continue
    const modelText = heading[2]?.trim()
    const common =
      heading[1]!.toLowerCase() === 'session'
        ? 'five_hour'
        : !modelText || /^all\s+models?$/i.test(modelText)
          ? 'seven_day'
          : null
    const model = common ? undefined : modelText!.replace(/\s+only$/i, '')
    const id = common ?? `seven_day_model:${model!.toLowerCase()}`
    const reset =
      section
        .find((line) => /^Resets?\s+/i.test(line))
        ?.replace(/^Resets?\s+/i, '')
        .slice(0, 160) ?? null
    // Human reset labels lack an authoritative epoch/timezone contract. Keep
    // them explicitly uncertain rather than guessing a timestamp or next week.
    const window: UsageWindow = {
      id,
      label: common === 'five_hour' ? '5 hours' : common === 'seven_day' ? 'Week (all)' : `Week (${model})`,
      ...(model ? { model } : {}),
      percent: Number(utilization[1]),
      resetsAt: null,
      resetText: reset,
      observedAt,
      source: 'tui-compatibility',
      freshness: 'fresh',
      reason: 'reset-timestamp-unavailable',
    }
    const duplicate = data.windows.findIndex((entry) => entry.id === id)
    if (duplicate < 0) data.windows.push(window)
    else data.windows[duplicate] = window
  }
  // Extra is descriptive only; arbitrary terminal text is not exposed as diagnostics.
  const extra = lines.findIndex((line) => /^Extra\s+usage\s*$/i.test(line))
  if (extra >= 0 && /not\s+enabled/i.test(lines.slice(extra + 1, extra + 3).join(' '))) data.extra = 'not enabled'
  return finalizePlanUsage(data)
}
export function finalizePlanUsage(data: PlanUsageData): PlanUsageData {
  const rank = (id: string) => (id === 'five_hour' ? 0 : id === 'seven_day' ? 1 : 2)
  data.windows.sort((a, b) => rank(a.id) - rank(b.id))
  data.diagnostics.missing = COMMON.filter(
    (id) => !data.windows.some((window) => window.id === id && window.freshness === 'fresh'),
  )
  for (const id of COMMON)
    if (data.windows.some((window) => window.id === id && window.freshness === 'fresh' && !window.resetsAt))
      data.diagnostics.missing.push(`${id}.reset-timestamp`)
  if (!data.claudeCodeVersion) data.diagnostics.missing.push('claude-code-version')
  data.source = data.windows.some((window) => window.source === 'statusline' && window.freshness === 'fresh')
    ? 'statusline'
    : data.windows.some((window) => window.freshness === 'fresh')
      ? 'tui-compatibility'
      : 'unavailable'
  data.state = !data.windows.some((window) => window.freshness === 'fresh')
    ? 'unavailable'
    : data.diagnostics.missing.length
      ? 'partial'
      : 'complete'
  data.reason = data.diagnostics.outputLimited
    ? 'output-limit'
    : data.diagnostics.missing.length
      ? (data.reason ?? 'missing-common-window')
      : data.windows.some((window) => window.reason !== null)
        ? 'reset-timestamp-unavailable'
        : null
  // Compatibility projections are labelled stale through the accompanying windows.
  const project = (id: string) => {
    const window = data.windows.find((value) => value.id === id)
    return window ? { percent: window.percent, resets: window.resetText ?? window.resetsAt ?? 'unknown' } : null
  }
  data.session = project('five_hour')
  data.weekAll = project('seven_day')
  data.weekSonnet = project('seven_day_model:sonnet')
  data.timestamp = data.observedAt
  return data
}
export function combinePlanUsage(
  current: PlanUsageData,
  structured: unknown,
  previous: PlanUsageData | null,
): PlanUsageData {
  const statusline = parseStatusline(structured)
  current.claudeCodeVersion = statusline.version
  for (const window of statusline.windows) {
    const index = current.windows.findIndex((value) => value.id === window.id)
    if (index < 0) current.windows.push(window)
    else current.windows[index] = window
  }
  if (previous) {
    for (const window of previous.windows) {
      if (current.windows.length >= MAX_WINDOWS) break
      if (!current.windows.some((value) => value.id === window.id))
        current.windows.push({ ...window, freshness: 'stale', reason: 'not-reported-current-capture' })
    }
  }
  if (current.diagnostics.outputLimited)
    current.windows = current.windows.map((window) => ({ ...window, freshness: 'stale', reason: 'output-limit' }))
  current.observedAt =
    current.windows
      .filter((value) => value.freshness === 'fresh')
      .map((value) => value.observedAt)
      .sort()
      .at(-1) ??
    previous?.observedAt ??
    current.observedAt
  return finalizePlanUsage(current)
}
