import type { JSX } from 'preact'

interface JsonlTimestampProps {
  timestamp: string
}

/** Short relative label for a message time: "just now", "2m ago", "3h ago",
 *  "5d ago", then a short absolute date once an entry is older than a week
 *  (relative distances stop being useful past that point). Pure/cheap — it
 *  reads Date.now() once per render, no timers. */
export function formatRelativeStamp(ts: string): { short: string; full: string } {
  try {
    const d = new Date(ts)
    const t = d.getTime()
    if (Number.isNaN(t)) return { short: '', full: '' }
    const full = d.toLocaleString([], { dateStyle: 'full', timeStyle: 'medium' })
    const diffMs = Date.now() - t
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
  const { short, full } = formatRelativeStamp(timestamp)
  return (
    <span style="color:var(--text-muted);font-size:10px" data-tooltip={full} title={full}>
      {short}
    </span>
  )
}
