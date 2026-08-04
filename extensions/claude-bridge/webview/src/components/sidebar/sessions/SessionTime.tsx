import type { JSX } from 'preact'

interface SessionTimeProps {
  createdAt: string
}

function formatRelativeTime(isoStr: string): string {
  const diff = Date.now() - new Date(isoStr).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'now'
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h`
  const days = Math.floor(hours / 24)
  return `${days}d`
}

function formatAbsoluteTime(isoStr: string): string {
  try {
    const d = new Date(isoStr)
    return d.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })
  } catch { return isoStr }
}

export function SessionTime({ createdAt }: SessionTimeProps): JSX.Element {
  return (
    <span class="session-time" data-tooltip={formatAbsoluteTime(createdAt)}>{formatRelativeTime(createdAt)}</span>
  )
}
