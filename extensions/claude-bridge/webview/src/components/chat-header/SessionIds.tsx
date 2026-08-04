import type { JSX } from 'preact'

interface SessionIdsProps {
  settingsDir?: string
  conversationId?: string
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

export function SessionIds({ settingsDir, conversationId }: SessionIdsProps): JSX.Element | null {
  if (!settingsDir && !conversationId) return null

  const shortPath = settingsDir?.replace(/^.*\/bridge-sessions\//, '') ?? ''

  return (
    <div class="chat-header-ids">
      {settingsDir && (
        <div>
          <span class="id-label">path</span>{' '}
          <span class="id-value">{shortPath}</span>
        </div>
      )}
      {conversationId && (
        <div>
          <span class="id-label">conversation</span>{' '}
          <span class="id-value">{escapeHtml(conversationId)}</span>
        </div>
      )}
    </div>
  )
}
