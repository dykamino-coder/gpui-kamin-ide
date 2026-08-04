import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'

interface ConnectionStatusBadgeProps {
  status: 'connected' | 'connecting' | 'disconnected' | 'error'
  error?: string
  /** Epoch ms of the scheduled reconnect, when one is pending. */
  nextRetryAt?: number
  /** Which attempt that retry will be. */
  retryAttempt?: number
}

const COLOR: Record<ConnectionStatusBadgeProps['status'], string> = {
  connected: 'var(--accent-green)',
  connecting: 'var(--accent-yellow)',
  disconnected: 'var(--text-disabled)',
  error: 'var(--accent-red)',
}

/** Backoff ceiling in `connection-manager.ts` — stated so the tooltip can say
 *  how bad the wait can get rather than leaving the user to guess. */
const MAX_BACKOFF_S = 30

/** Tooltip text for a pending reconnect.
 *
 *  Answers the two questions a stalled connection raises — WHEN is the next try,
 *  and do they stop? Retries are unbounded (the delay doubles to a 30s ceiling
 *  and stays there), so saying so is the difference between "wait" and "this is
 *  dead, restart it". A live count of seconds also proves the loop is running. */
export function retryLabel(secondsLeft: number, attempt: number): string {
  const when = secondsLeft <= 0 ? 'now' : `in ${String(secondsLeft)}s`
  const which = attempt > 0 ? `attempt ${String(attempt + 1)} ` : ''
  return `Reconnecting — ${which}${when}. Retries continue (up to ${String(MAX_BACKOFF_S)}s apart).`
}

/** Compact "traffic-light" status dot — replaces the previous text badge.
 *  The chat title is the dominant chrome of the header now; status reads
 *  as a single 12-px coloured dot with the full label in a tooltip. */
export function ConnectionStatusBadge({ status, error, nextRetryAt, retryAttempt }: ConnectionStatusBadgeProps): JSX.Element {
  const pending = nextRetryAt !== undefined && status !== 'connected'
  const [, tick] = useState(0)
  useEffect(() => {
    if (!pending) return
    const id = window.setInterval(() => { tick((n) => n + 1) }, 1000)
    return () => { window.clearInterval(id) }
  }, [pending, nextRetryAt])

  const label = status.charAt(0).toUpperCase() + status.slice(1)
  let tooltip = error ? `${label}: ${error}` : label
  if (pending) {
    const left = Math.max(0, Math.ceil((nextRetryAt - Date.now()) / 1000))
    tooltip = retryLabel(left, retryAttempt ?? 0)
  }
  const color = COLOR[status]
  return (
    <span
      data-tooltip={tooltip}
      style={`width:12px;height:12px;border-radius:50%;flex-shrink:0;background:${color};box-shadow:0 0 0 3px color-mix(in srgb, ${color} 18%, transparent)`}
    />
  )
}
