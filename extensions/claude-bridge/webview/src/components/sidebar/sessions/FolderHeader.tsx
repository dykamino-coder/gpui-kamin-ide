import type { JSX } from 'preact'
import { FolderAddButton } from './FolderAddButton'

interface FolderHeaderProps {
  name: string
  isUnknown: boolean
  onAdd: () => void
  onDelete: () => void
  showDelete: boolean
}

export function FolderHeader({ name, isUnknown, onAdd, onDelete, showDelete }: FolderHeaderProps): JSX.Element {
  return (
    <div class={`sidebar-folder${isUnknown ? ' folder-unknown' : ''}`}>
      <span class="folder-icon">
        {isUnknown
          ? <i class="fas fa-question-circle" />
          : <i class="fas fa-folder" />
        }
      </span>
      {' '}{name}
      <span class="folder-btns">
        <FolderAddButton onAdd={onAdd} />
        {showDelete && (
          <span
            class="folder-btn"
            data-tooltip="Delete folder and all sessions"
            onClick={(e) => { e.stopPropagation(); onDelete() }}
            role="button"
            tabIndex={0}
          >
            <i class="fas fa-trash" />
          </span>
        )}
      </span>
    </div>
  )
}
