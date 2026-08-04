import type { JSX } from 'preact'

interface DisconnectButtonProps {
  onClick: () => void
}

export function DisconnectButton({ onClick }: DisconnectButtonProps): JSX.Element {
  return (
    <button class="header-btn" data-tooltip="Disconnect session" type="button" onClick={onClick}>
      <i class="fas fa-link-slash" />
    </button>
  )
}
