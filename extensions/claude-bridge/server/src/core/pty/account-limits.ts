// Last 5-hour / 7-day windows the CLI reported through its status line,
// shared by every session of this Bridge server.
//
// Why: Claude Code fills `rate_limits` only from the response headers of the
// CURRENT process. Its startup quota probe (`probeQuotaStatus`, a 1-token
// request) is skipped in `essential-traffic` mode, which Bridge enables with
// CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC (verified in the 2.1.296 binary:
// `probeQuotaStatus` returns null when that mode is on). So every fresh,
// resumed or respawned CLI reports only the context fill until its first model
// response, and the row under the chat input lost 5h/7d after each reconnect.
//
// The limits belong to the account, and all sessions of the container run
// under the same CLI login. The newest windows any session's CLI reported are
// therefore valid for all of them until their `resets_at`. They are kept here
// and persisted next to the server data so a server restart keeps them too.
// The values come only from the CLI's status line: no extra request, no
// `/usage` capture.

import fs from 'fs'
import path from 'path'

import { getDataDir } from '../stats/database/lifecycle'
import { warnLog } from '../logging'

export interface LimitWindow {
  id: string
  label: string
  percent: number
  resetsAt: string
}

export interface AccountLimits {
  windows: LimitWindow[]
  /** When the CLI reported these windows. */
  observedAt: string
}

const MAX_FILE_BYTES = 4096
let filePath: string | null = null
let cache: AccountLimits | null | undefined
const listeners = new Set<() => void>()

function storePath(): string {
  return filePath ?? path.join(getDataDir(), 'statusline-limits.json')
}

function valid(window: unknown): window is LimitWindow {
  if (!window || typeof window !== 'object') return false
  const w = window as Record<string, unknown>
  return (
    (w.id === 'five_hour' || w.id === 'seven_day') &&
    typeof w.label === 'string' &&
    w.label.length <= 64 &&
    typeof w.percent === 'number' &&
    Number.isFinite(w.percent) &&
    w.percent >= 0 &&
    w.percent <= 100 &&
    typeof w.resetsAt === 'string' &&
    Number.isFinite(Date.parse(w.resetsAt))
  )
}

function load(): AccountLimits | null {
  if (cache !== undefined) return cache
  cache = null
  try {
    const p = storePath()
    if (fs.statSync(p).size > MAX_FILE_BYTES) return cache
    const raw = JSON.parse(fs.readFileSync(p, 'utf8')) as Partial<AccountLimits>
    if (
      Array.isArray(raw.windows) &&
      typeof raw.observedAt === 'string' &&
      Number.isFinite(Date.parse(raw.observedAt))
    ) {
      const windows = raw.windows.filter(valid)
      if (windows.length > 0) cache = { windows, observedAt: raw.observedAt }
    }
  } catch {
    /* no snapshot yet */
  }
  return cache
}

function persist(value: AccountLimits): void {
  try {
    const p = storePath()
    fs.mkdirSync(path.dirname(p), { recursive: true })
    const tmp = `${p}.${process.pid}.tmp`
    fs.writeFileSync(tmp, JSON.stringify(value), { mode: 0o600 })
    fs.renameSync(tmp, p)
  } catch (error) {
    warnLog('Status-line limits store write failed', { error: String(error) })
  }
}

/** Windows that have not reset yet, or null when none are known. */
export function currentAccountLimits(now: number = Date.now()): AccountLimits | null {
  const value = load()
  if (!value) return null
  const windows = value.windows.filter((w) => Date.parse(w.resetsAt) > now)
  return windows.length > 0 ? { windows, observedAt: value.observedAt } : null
}

/** Record windows a CLI just reported. Older observations never replace a
 *  newer one. Returns true when the stored snapshot changed. */
export function recordAccountLimits(windows: LimitWindow[], observedAt: string): boolean {
  const fresh = windows.filter(valid)
  if (fresh.length === 0 || !Number.isFinite(Date.parse(observedAt))) return false
  const current = load()
  if (current && Date.parse(current.observedAt) > Date.parse(observedAt)) return false
  const next: AccountLimits = { windows: fresh, observedAt }
  const changed = !current || JSON.stringify(current.windows) !== JSON.stringify(fresh)
  cache = next
  persist(next)
  if (changed) for (const listener of [...listeners]) listener()
  return changed
}

export function onAccountLimitsChanged(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Tests only: isolate the store file and drop the in-memory snapshot. */
export function resetAccountLimitsForTests(file: string | null): void {
  filePath = file
  cache = undefined
}
