import type { JSX } from 'preact'

interface Props {
  visible: boolean
  onClick: () => void
}

export function ToggleServerPathButton({ visible, onClick }: Props): JSX.Element {
  return (
    <button
      class="header-btn"
      data-tooltip={visible ? 'Hide system info' : 'Show system info'}
      type="button"
      onClick={onClick}
      style={visible ? 'color:var(--text-primary)' : ''}
    >
      <i class="fas fa-circle-info" />
    </button>
  )
}
