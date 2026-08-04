import type { JSX } from 'preact'

interface TreeStatusDotProps {
  status: string
}

export function TreeStatusDot({ status }: TreeStatusDotProps): JSX.Element {
  switch (status) {
    case 'active':
    case 'busy':
      return (
        <span class="session-status">
          <i class="fas fa-circle" style="color:var(--accent-green);font-size:8px" />
        </span>
      )
    case 'idle':
      return (
        <span class="session-status">
          <i class="fas fa-circle" style="color:var(--accent-yellow);font-size:8px" />
        </span>
      )
    case 'done':
      return (
        <span class="session-status">
          <i class="fas fa-check-circle" style="color:var(--accent-green);font-size:10px" />
        </span>
      )
    case 'error':
      return (
        <span class="session-status">
          <i class="fas fa-circle" style="color:var(--accent-red);font-size:8px" />
        </span>
      )
    case 'exited':
    default:
      return (
        <span class="session-status">
          <i class="far fa-circle" style="color:var(--text-muted);font-size:8px" />
        </span>
      )
  }
}
