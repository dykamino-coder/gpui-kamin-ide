import type { JSX } from 'preact'

interface SessionPinButtonProps {
  pinned: boolean
  onClick: () => void
}

export function SessionPinButton({ pinned, onClick }: SessionPinButtonProps): JSX.Element {
  return (
    <span
      class={`session-pin${pinned ? ' pinned' : ''}`}
      data-tooltip={pinned ? 'Unpin tab' : 'Pin tab'}
      onClick={(e) => { e.stopPropagation(); onClick() }}
      role="button"
      tabIndex={0}
    >
      <i class="fas fa-thumbtack" />
    </span>
  )
}
