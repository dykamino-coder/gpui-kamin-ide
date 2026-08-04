import type { JSX } from 'preact'

interface SessionRenameButtonProps {
  onClick: () => void
}

export function SessionRenameButton({ onClick }: SessionRenameButtonProps): JSX.Element {
  return (
    <span
      class="session-rename"
      data-tooltip="Auto-rename"
      onClick={(e) => { e.stopPropagation(); onClick() }}
      role="button"
      tabIndex={0}
    >
      <i class="fas fa-pen" />
    </span>
  )
}
