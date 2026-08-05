import { Fragment } from 'preact'
import type { JSX } from 'preact'
import { useRef, useEffect } from 'preact/hooks'
import { useSignal } from '@preact/signals'

import { activeTabId } from '../signals/tabs'
import { initializing } from '../signals/ui'

import { useBridge } from '../hooks/useBridge'
import { useInit } from '../hooks/useInit'
import { useBridgeListeners } from '../hooks/useBridgeListeners'
import { useGlobalEscapeCancel } from '../hooks/useGlobalEscapeCancel'
import { useCodeCopyButtons } from '../hooks/useCodeCopyButtons'
import { useMcpElicitationPrompt } from '../hooks/useMcpElicitationPrompt'
import { useThemeHotkey } from '../hooks/useThemeHotkey'
import { useDragDrop } from '../hooks/useDragDrop'

import { ContentArea } from '../components/layout/ContentArea'
import { ChatHeaderStrip } from '../components/chat-header/ChatHeaderStrip'
import { ConversationSegmentTabs } from '../components/jsonl-viewer/ConversationSegmentTabs'
import { NoSessionEmptyContent, RestoringContent } from '../components/layout/EmptyState'
import { JsonlViewer } from '../components/jsonl-viewer/JsonlViewer'
import { ToolCounterToast } from '../components/titlebar/ToolCounterToast'
import { ActivityStrip } from '../components/chat-header/ActivityStrip'
import { WidgetsPanel } from '../components/widgets/WidgetsPanel'
import { InputBar } from '../components/input-bar/InputBar'
import { Tooltip } from '../components/overlays/Tooltip'
import { ConfirmModalHost } from '../components/overlays/ConfirmModalHost'
import { DropOverlay } from '../components/overlays/DropOverlay'
import { NoTokenGateModal } from '../components/overlays/NoTokenGateModal'
import { PluginHookApprovalModal } from '../components/customize/hooks/PluginHookApprovalModal'
const COLUMN = 'display:flex;flex-direction:column;flex:1;min-width:0;height:100%;overflow:hidden'

// Center editor-area chat panel. Flush + transparent (no card/border) so it sits
// directly on the KaminIDE panel surface and follows the IDE theme. Renders the
// active KaminIDE session's Claude conversation — chat + tool renderers + CLI
// terminal + input + widgets + agent tiles + plan/todos. Sessions themselves are
// KaminIDE-native (bound by the extension's SessionsBridge); this panel just
// shows whichever is active. Runs its own bridge listeners (separate iframe).
export function ChatRoot(): JSX.Element {
  const bridge = useBridge()
  const { isDragging } = useDragDrop()
  const promptDebounceTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())
  const vscodeAvailable = useSignal(false)

  useBridgeListeners(bridge, promptDebounceTimers, vscodeAvailable, 'chat')
  useGlobalEscapeCancel(bridge)
  useCodeCopyButtons()
  useMcpElicitationPrompt(bridge)
  useThemeHotkey()
  useInit(bridge, 'chat')

  useEffect(() => { try { bridge.requestJsonlReplay() } catch { /* ignore */ } }, [])

  // Корень подписан ТОЛЬКО на activeTabId/initializing: per-frame сигналы
  // (thinking-превью, OSC-активность, цвета) ушли в <ActivityStrip/> —
  // раньше корень ререндерил всё дерево каждый кадр стрима (аудит #70 D1).
  const tabId = activeTabId.value

  const overlays = (
    <>
      <DropOverlay active={isDragging} />
      <Tooltip />
      <ConfirmModalHost />
      <NoTokenGateModal />
      <PluginHookApprovalModal />
    </>
  )

  if (!tabId) {
    // While the first restore pass (useInit) is still resolving, show a
    // "Restoring…" cue — NOT the "New session" welcome — so a returning user
    // whose saved session is still loading doesn't see the create-session button
    // flash on every cold launch (activeTabId is set asynchronously inside
    // useInit, so first paint always has tabId===null). The button is correct
    // ONLY once restore finished and there is genuinely no session.
    return (
      <Fragment>
        <div style={COLUMN}>{initializing.value ? <RestoringContent /> : <NoSessionEmptyContent />}</div>
        {overlays}
      </Fragment>
    )
  }

  return (
    <Fragment>
      <div style={COLUMN}>
        <ChatHeaderStrip />
        {/* Compact-segment tabs (Current · from … ) — sits above the scrollable
            chat so it stays visible; self-hides when there are no compactions. */}
        <ConversationSegmentTabs />
        {/* No terminal in the chat — the Bridge Console tool owns the CLI
            terminal now. A hidden (display:none, 0-width) TerminalPanel here
            would fit to ~2 cols and fight the Console's wide resize on the SAME
            PTY tabId → the console width oscillated. */}
        <ContentArea
          jsonlPanel={
            <div class="chat-panel" style="position:relative">
              <JsonlViewer />
              <ToolCounterToast />
            </div>
          }
          resizeHandle={null}
          terminalPanel={null}
        />
        <ActivityStrip />
        <WidgetsPanel />
        <InputBar />
      </div>
      {overlays}
    </Fragment>
  )
}
