import type { JSX } from 'preact'

interface SavedSessionsToggleProps {
  count: number
  isOpen: boolean
  onToggle: () => void
}

export function SavedSessionsToggle({ count, isOpen, onToggle }: SavedSessionsToggleProps): JSX.Element {
  const label = `${count} inactive session${count > 1 ? 's' : ''}`
  return (
    <div
      class={`toggle-inactive-btn${isOpen ? ' expanded' : ''}`}
      onClick={onToggle}
      role="button"
      tabIndex={0}
    >
      <i class="fas fa-caret-right" />
      {' '}{label}
    </div>
  )
}
