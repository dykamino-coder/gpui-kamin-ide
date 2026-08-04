import type { JSX } from 'preact'

interface FolderAddButtonProps {
  onAdd: () => void
}

export function FolderAddButton({ onAdd }: FolderAddButtonProps): JSX.Element {
  return (
    <span
      class="folder-btn folder-add-btn"
      data-tooltip="New session"
      onClick={(e) => { e.stopPropagation(); onAdd() }}
      role="button"
      tabIndex={0}
    >
      <i class="fas fa-plus" />
    </span>
  )
}
