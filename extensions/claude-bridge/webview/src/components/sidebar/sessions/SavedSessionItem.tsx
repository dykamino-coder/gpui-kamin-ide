import type { JSX } from 'preact'
import type { SavedSession } from '../../../../shared/types'
import { SessionTime } from './SessionTime'

interface SavedSessionItemProps {
  session: SavedSession
  onResume: () => void
  onDelete: () => void
}

export function SavedSessionItem({ session, onResume, onDelete }: SavedSessionItemProps): JSX.Element {
  const displayLabel = session.label || session.conversationId.slice(0, 8)

  async function handleDelete(e: MouseEvent): Promise<void> {
    e.stopPropagation()
    const show = (window as any).__showConfirmModal as
      | ((opts: { title: string; bodyHtml: string; confirmLabel?: string; isDanger?: boolean }) => Promise<boolean>)
      | undefined
    const escape = (s: string) => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!))
    const ok = show
      ? await show({
          title: 'Delete saved session?',
          bodyHtml: `Session <strong>${escape(displayLabel)}</strong> will be permanently removed. This cannot be undone.`,
          confirmLabel: 'Delete',
          isDanger: true,
        })
      : false
    if (ok) onDelete()
  }

  return (
    <div class="sidebar-session saved-session" onClick={onResume}>
      <span class="session-status">
        <i class="fas fa-circle" style="color:var(--text-disabled);font-size:8px" />
      </span>
      <span class="session-label saved">{displayLabel}</span>
      <SessionTime createdAt={session.lastActivity} />
      <span
        class="session-delete"
        data-tooltip="Delete session"
        onClick={handleDelete}
        role="button"
        tabIndex={0}
      >
        <i class="fas fa-trash" />
      </span>
    </div>
  )
}
