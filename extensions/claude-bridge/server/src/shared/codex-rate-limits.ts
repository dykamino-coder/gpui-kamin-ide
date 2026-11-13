// ============================================================================
// ChatGPT plan limits of the codex account (`account/rateLimits/read`).
//
// The plan bills codex by usage windows, not per token: a primary (short,
// e.g. 5 h) and a secondary (weekly) window, each with the percent used and
// its reset time. The dashboard shows them beside Claude's session/weekly
// usage. Only what codex reports is shown; a missing window stays missing.
// ============================================================================

export interface CodexLimitWindow {
  usedPercent: number
  windowMinutes: number | null
  /** Epoch ms of the reset, or null. */
  resetsAt: number | null
}

export interface CodexLimitBucket {
  id: string | null
  name: string | null
  primary: CodexLimitWindow | null
  secondary: CodexLimitWindow | null
  reached: string | null
}

export interface CodexRateLimits {
  /** The account-wide view, then any per-limit buckets other than it. */
  buckets: CodexLimitBucket[]
  credits: { hasCredits: boolean; unlimited: boolean; balance: string | null } | null
  planType: string | null
  /** False when the backend refuses ordinary usage now; null = unknown. */
  usageAllowed: boolean | null
  readAt: number
}

type Obj = Record<string, unknown>
const obj = (v: unknown): Obj | null => (v && typeof v === 'object' && !Array.isArray(v) ? (v as Obj) : null)
const num = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null)
const str = (v: unknown): string | null => (typeof v === 'string' && v ? v : null)

function window(raw: unknown): CodexLimitWindow | null {
  const w = obj(raw)
  const used = num(w?.usedPercent)
  if (!w || used === null) return null
  // codex reports resetsAt in epoch seconds.
  const resets = num(w.resetsAt)
  return {
    usedPercent: Math.max(0, Math.min(100, used)),
    windowMinutes: num(w.windowDurationMins),
    resetsAt: resets === null ? null : resets * 1000,
  }
}

function bucket(raw: unknown): CodexLimitBucket | null {
  const s = obj(raw)
  if (!s) return null
  const b: CodexLimitBucket = {
    id: str(s.limitId),
    name: str(s.limitName),
    primary: window(s.primary),
    secondary: window(s.secondary),
    reached: str(s.rateLimitReachedType),
  }
  return b.primary || b.secondary ? b : null
}

/** "5-hour limit", "Weekly limit", … from the window length codex reports. */
export function limitWindowLabel(minutes: number | null, fallback: string): string {
  if (!minutes || minutes <= 0) return fallback
  if (minutes === 7 * 24 * 60) return 'Weekly limit'
  if (minutes % (24 * 60) === 0) return `${minutes / (24 * 60)}-day limit`
  if (minutes % 60 === 0) return `${minutes / 60}-hour limit`
  return `${minutes}-minute limit`
}

/** Normalizes an `account/rateLimits/read` response. */
export function parseCodexRateLimits(raw: unknown, now = Date.now()): CodexRateLimits {
  const res = obj(raw) ?? {}
  const main = obj(res.rateLimits)
  const buckets: CodexLimitBucket[] = []
  const first = bucket(main)
  if (first) buckets.push(first)
  const byId = obj(res.rateLimitsByLimitId)
  for (const [id, value] of Object.entries(byId ?? {})) {
    if (first && (first.id ?? 'codex') === id) continue
    const b = bucket(value)
    if (b) buckets.push({ ...b, id: b.id ?? id })
  }
  const credits = obj(main?.credits)
  return {
    buckets,
    credits: credits
      ? {
          hasCredits: credits.hasCredits === true,
          unlimited: credits.unlimited === true,
          balance: str(credits.balance),
        }
      : null,
    planType: str(main?.planType),
    usageAllowed: typeof res.ordinaryUsageAllowed === 'boolean' ? res.ordinaryUsageAllowed : null,
    readAt: now,
  }
}
