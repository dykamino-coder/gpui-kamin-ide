import { storage } from "@bridge/storage"
import type { JSX } from 'preact'
import { TabTitle } from './TabTitle'
import { ConnectionStatusBadge } from './ConnectionStatusBadge'
import { FolderCrumb } from './FolderCrumb'
import { SessionIds } from './SessionIds'
import { DisconnectButton } from './DisconnectButton'
import { DownloadJsonlButton } from './DownloadJsonlButton'
import { ToggleServerPathButton } from './ToggleServerPathButton'
import { SessionStats } from './SessionStats'
import { PlanProgress } from './PlanProgress'
import { ConversationSegmentTabs } from '../jsonl-viewer/ConversationSegmentTabs'
import { serverPathVisible } from '../../signals/ui'

interface ChatHeaderProps {
  title: string
  folderName?: string
  status: 'connected' | 'connecting' | 'disconnected' | 'error'
  statusError?: string
  cwd?: string
  vscodeAvailable?: boolean
  settingsDir?: string
  conversationId?: string
  showControls?: boolean
  terminalVisible?: boolean
  jsonlVisible?: boolean
  onDisconnect: () => void
  onToggleTerminal: () => void
  onToggleJsonl: () => void
  /** Resolves with the outcome — the button reports success/failure itself. */
  onDownloadJsonl: () => Promise<{ success: boolean; error?: string }>
}

export function ChatHeader({
  title,
  folderName,
  status,
  statusError,
  cwd,
  vscodeAvailable = false,
  settingsDir,
  conversationId,
  showControls = false,
  terminalVisible = false,
  jsonlVisible = true,
  onDisconnect,
  onToggleTerminal,
  onToggleJsonl,
  onDownloadJsonl,
}: ChatHeaderProps): JSX.Element {
  return (
    <div class="chat-header" id="chat-header">
      {/* Row 1 — status / folder / title on the left, action buttons
          on the right. */}
      <div class="chat-header-row" style="padding:4px 8px 0 12px">
        <ConnectionStatusBadge status={status} error={statusError} />
        <FolderCrumb cwd={cwd} folderName={folderName} vscodeAvailable={vscodeAvailable} />
        <TabTitle title={title} folderName={folderName} />
        <div class="header-spacer" />
        {showControls && (
          <>
            <div class="header-btn-group">
              <DownloadJsonlButton onDownload={onDownloadJsonl} />
              <ToggleServerPathButton
                visible={serverPathVisible.value}
                onClick={() => {
                  const next = !serverPathVisible.value
                  serverPathVisible.value = next
                  storage.setItem('server-path-visible', String(next))
                }}
              />
            </div>
            <DisconnectButton onClick={onDisconnect} />
          </>
        )}
      </div>
      {/* Row 2 — conversation-segment tabs (Current · from … etc).
          Sits between the title row and the metrics strip so segments
          read as the conversation's table of contents, separated from
          per-session telemetry below. Self-hides when the session has
          no compactions. */}
      <ConversationSegmentTabs />
      {/* Row 3 — context-bar metrics (%, tokens, dollars) + plan
          progress, pinned to the left so the eye scans them as a
          coherent telemetry strip independent of the title row. */}
      <div class="chat-header-row" style="justify-content:center;gap:var(--space-3);padding:12px">
        <SessionStats />
        <PlanProgress />
      </div>
      {serverPathVisible.value && (
        <SessionIds settingsDir={settingsDir} conversationId={conversationId} />
      )}
    </div>
  )
}
