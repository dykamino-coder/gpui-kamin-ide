import type { JSX } from 'preact'

interface SessionStatusDotProps {
  status: string
  /** Optional override colour for the connected state — the agent /
   *  pinned colour of the session, so the dot matches the row tint. */
  connectedColor?: string | null
}

export function SessionStatusDot({ status, connectedColor }: SessionStatusDotProps): JSX.Element {
  const color = status === 'connected'
    ? (connectedColor || 'var(--accent-green)')
    : status === 'connecting'
      ? 'var(--accent-yellow)'
      : 'var(--accent-red)'

  return (
    <span class="session-status">
      <i class="fas fa-circle" style={`color:${color};font-size:8px`} />
    </span>
  )
}
