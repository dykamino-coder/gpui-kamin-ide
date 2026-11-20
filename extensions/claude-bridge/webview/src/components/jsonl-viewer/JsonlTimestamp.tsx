import type { JSX } from 'preact'
import { signal } from '@preact/signals'

interface JsonlTimestampProps {
  timestamp: string
}

/** The clock every relative label reads. One shared timer, started by the
 *  first label: without it a label kept the age it had when its row was last
 *  rendered, so after a history replay a re-rendered question said "2h ago"
 *  while the untouched answer to it still said "just now". */
const TICK_MS = 30_000
export const stampClock = signal(Date.now())
let ticking: ReturnType<typeof setInterval> | null = null
function startClock(): void {
  if (ticking || typeof setInterval !== 'function') return
  ticking = setInterval(() => { stampClock.value = Date.now() }, TICK_MS)
  ;(ticking as { unref?: () => void }).unref?.()
}

/** Short relative label for a message time: "just now", "2m ago", "3h ago",
 *  "5d ago", then a short absolute date once an entry is older than a week
 *  (relative distances stop being useful past that point). */
export function formatRelativeStamp(ts: string, now = Date.now()): { short: string; full: string } {
  try {
    const d = new Date(ts)
    const t = d.getTime()
    if (Number.isNaN(t)) return { short: '', full: '' }
    const full = d.toLocaleString([], { dateStyle: 'full', timeStyle: 'medium' })
    const diffMs = now - t
    // Future or clock-skew: treat as "just now" rather than a negative age.
    const secs = Math.max(0, Math.floor(diffMs / 1000))
    let short: string
    if (secs < 60) short = 'just now'
    else if (secs < 3600) short = `${Math.floor(secs / 60)}m ago`
    else if (secs < 86400) short = `${Math.floor(secs / 3600)}h ago`
    else if (secs < 604800) short = `${Math.floor(secs / 86400)}d ago`
    else short = d.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' })
    return { short, full }
  } catch {
    return { short: '', full: '' }
  }
}

export function JsonlTimestamp({ timestamp }: JsonlTimestampProps): JSX.Element {
  startClock()
  // Reading the clock subscribes only this label: a tick re-renders the
  // labels, not their rows. Date.now() keeps a fresh render exact between ticks.
  void stampClock.value
  const { short, full } = formatRelativeStamp(timestamp, Date.now())
  return (
    <span style="color:var(--text-muted);font-size:10px" data-tooltip={full} title={full}>
      {short}
    </span>
  )
}
