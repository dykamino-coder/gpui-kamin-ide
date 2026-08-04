import type { JSX } from 'preact'
import { useBridge } from '../../hooks/useBridge'
import { tabs, activeTabId } from '../../signals/tabs'
import { ConnectionStatusBadge } from './ConnectionStatusBadge'
import { SessionStats } from './SessionStats'
import { PlanProgress } from './PlanProgress'
import { DisconnectButton } from './DisconnectButton'
import { DiagnosticButton } from './DiagnosticButton'
import { MemoryInfoButton } from './MemoryInfoButton'
import { ReconnectButton } from './ReconnectButton'
import { RegenerateTitleButton } from './RegenerateTitleButton'
import { DownloadJsonlButton } from './DownloadJsonlButton'

// Slim telemetry strip above the chat — KaminIDE owns the session tab (title),
// but the per-session context/cost + connection status + connect controls used
// to live in the Bridge ChatHeader and the user wants them back over the chat.
// One row: status dot · context-bar + cost (SessionStats) + plan progress ·
// reconnect / disconnect. Renders nothing when there's no active session.
export function ChatHeaderStrip(): JSX.Element | null {
  const bridge = useBridge()
  const tabId = activeTabId.value
  if (!tabId) return null
  const activeTab = tabs.value.find(t => t.id === tabId) ?? null
  const status = (activeTab?.status ?? 'disconnected') as 'connected' | 'connecting' | 'disconnected' | 'error'
  const connected = status === 'connected' || status === 'connecting'
  // `sessionTitle` is the CLI's auto-generated topic ("Fix auth bug") and only
  // shows up a turn or two in; until then name the session by its folder.
  const sessionName = activeTab?.sessionTitle || activeTab?.folderName || activeTab?.label || ''

  return (
    <div
      class="chat-header"
      id="chat-header"
      // Two deliberate rows (the `.chat-header` class is already flex-column):
      //  1) connection status (left) + session control buttons (right),
      //  2) the telemetry — context/cost bar + plan progress — centered.
      // Splitting the chrome from the telemetry keeps the strip readable when the
      // chat panel is narrow, where a single row used to overflow/clip.
      style="flex-direction:column;align-items:stretch;gap:5px;padding:6px 12px"
    >
      <div style="display:flex;align-items:center;gap:var(--space-2);min-width:0">
        <ConnectionStatusBadge status={status} nextRetryAt={activeTab?.nextRetryAt} retryAttempt={activeTab?.retryAttempt} />
        {/* Session name next to the status dot. Falls back to the folder (then
            the "#1" label) until the CLI has generated a topic. Ellipsises so a
            long topic can never push the control buttons out of the row. */}
        <span
          data-tooltip={sessionName}
          style={`font-size:12px;font-weight:500;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;color:var(${activeTab?.sessionTitle ? "--text-secondary" : "--text-muted"})`}
        >
          {sessionName}
        </span>
        <div class="header-btn-group" style="margin-left:auto;flex:none">
          {/* #78 diagnostic — always available (diagnose AFTER a drop, even once
              disconnected): copies chat-store + drop-log to the clipboard. */}
          <DiagnosticButton tabId={tabId} />
          {/* Live memory readout (renderer heap + retained entries/tabs) — the
              same figures an OOM report leads with, next to the diagnostic dump. */}
          <MemoryInfoButton />
          {/* Download the session JSONL transcript — always available (the file
              exists on the server even after the CLI disconnects). Lost in the
              ChatHeader→ChatHeaderStrip decomposition; restored here. */}
          <DownloadJsonlButton onDownload={() => bridge.downloadJsonl(tabId)} />
          {/* Regenerate the AI title from chat — only while connected (needs a
              live CLI to receive `/rename`). */}
          {connected && <RegenerateTitleButton onClick={() => bridge.regenerateTitle(tabId)} />}
          {/* Reconnect is always available (re-opens the WS → server --resume's the
              same conversation). Disconnect only makes sense while connected — it
              drops the WS, which makes the server kill the Claude CLI process. */}
          <ReconnectButton onClick={() => bridge.reconnectTab(tabId)} />
          {connected && <DisconnectButton onClick={() => bridge.disconnectTab(tabId)} />}
        </div>
      </div>
      <div style="display:flex;align-items:center;justify-content:center;flex-wrap:wrap;gap:6px var(--space-3);min-width:0">
        <SessionStats />
        <PlanProgress />
      </div>
    </div>
  )
}
