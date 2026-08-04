import type { JSX } from 'preact'

interface SessionCloseButtonProps {
  onClick: (e: Event) => void
}

export function SessionCloseButton({ onClick }: SessionCloseButtonProps): JSX.Element {
  return (
    <span
      class="session-close"
      data-tooltip="Disconnect session"
      onClick={onClick}
      role="button"
      tabIndex={0}
    >
      <i class="fas fa-link-slash" />
    </span>
  )
}
