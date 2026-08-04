import type { JSX } from 'preact'

interface ReconnectButtonProps {
  onClick: () => void
}

export function ReconnectButton({ onClick }: ReconnectButtonProps): JSX.Element {
  return (
    <button class="header-btn" data-tooltip="Reconnect session" type="button" onClick={onClick}>
      <i class="fas fa-rotate-right" />
    </button>
  )
}
