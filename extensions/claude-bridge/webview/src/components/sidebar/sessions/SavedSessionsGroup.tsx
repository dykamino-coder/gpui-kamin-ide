import type { JSX } from 'preact'
import type { SavedSession } from '../../../../shared/types'
import { SavedSessionItem } from './SavedSessionItem'

interface SavedSessionsGroupProps {
  sessions: SavedSession[]
  isVisible: boolean
  onRemove: (conversationId: string) => void
}

export function SavedSessionsGroup({ sessions, isVisible, onRemove }: SavedSessionsGroupProps): JSX.Element {
  async function handleResume(session: SavedSession): Promise<void> {
    const bridge = (window as any).kaminBridge
    if (!bridge) return
    const config = await bridge.getConfig()
    if (!config.serverUrl || !config.token) return
    await bridge.resumeSession({
      ...config,
      cwd: session.cwd,
      conversationId: session.conversationId,
      sessionLabel: session.label || undefined,
    })
  }

  return (
    <div class={`saved-sessions-group${isVisible ? ' visible' : ''}`}>
      {sessions.map(s => (
        <SavedSessionItem
          key={s.conversationId}
          session={s}
          onResume={() => handleResume(s)}
          onDelete={() => onRemove(s.conversationId)}
        />
      ))}
    </div>
  )
}
