import { storage } from "@bridge/storage"
// Main content slot extracted from App.tsx (Sprint 5 / Stage E1) — picks
// between the customize panel, the empty-state, and the live session
// MainPanel composition based on which mode the app is in.

import type { JSX } from 'preact'
import type { KaminBridgeApi, TabInfo } from '../../../shared/types'
import { NoSessionEmptyContent, RestoringContent } from './EmptyState'
import { initializing } from '../../signals/ui'
import { MainPanel } from './MainPanel'
import { ContentArea } from './ContentArea'
import { ChatHeader } from '../chat-header/ChatHeader'
import { JsonlViewer } from '../jsonl-viewer/JsonlViewer'
import { ActivityIndicator } from '../titlebar/ActivityIndicator'
import { ToolCounterToast } from '../titlebar/ToolCounterToast'
import { WidgetsPanel } from '../widgets/WidgetsPanel'
import { InputBar } from '../input-bar/InputBar'
import { CustomizeContentPanel } from '../customize/CustomizeContentPanel'
import { TerminalPanel } from '../terminal/TerminalPanel'
import { SubagentButtonsRow } from '../agent-tiles/SubagentButtonsRow'
import { SubagentFullscreen } from '../agent-tiles/SubagentFullscreen'
import { ToolUsageFullscreen } from '../tool-usage/ToolUsageFullscreen'
import { activeThinkingPreview } from '../../signals/jsonl'
import { viewerVisible, jsonlVisible, pinnedTitleColors } from '../../signals/ui'
import { getAgentColor, resolvePinnedColor } from '../../utils/agent-color'

interface Props {
  bridge: KaminBridgeApi
  isCustomizeOpen: boolean
  tabId: string | null
  activeTab: TabInfo | null
  connStatus: string
  activity: { rawTitle: string; isWorking: boolean } | null
  vscodeAvailable: boolean
}

export function AppMainContent({
  bridge, isCustomizeOpen, tabId, activeTab, connStatus, activity, vscodeAvailable,
}: Props): JSX.Element {
  if (isCustomizeOpen) {
    return (
      <MainPanel
        header={null}
        content={<CustomizeContentPanel />}
        inputBar={null}
        agentTiles={null}
      />
    )
  }
  if (!tabId) {
    // While the first restore pass is still in flight, show a "Restoring…" cue
    // instead of the "New session" welcome — a returning user whose session is
    // still loading shouldn't think it's gone or click New session by mistake.
    return (
      <MainPanel
        header={null}
        content={initializing.value ? <RestoringContent /> : <NoSessionEmptyContent />}
        inputBar={null}
        agentTiles={null}
      />
    )
  }
  return (
    <MainPanel
      header={
        <>
          <ChatHeader
            title={
              activeTab?.sessionTitle
                ? activeTab.sessionTitle
                : activeTab?.cwd
                  ? `${activeTab.folderName ?? ''} ${activeTab.label ?? ''}`.trim()
                  : 'Open Claude Bridge'
            }
            folderName={activeTab?.folderName}
            status={connStatus as 'connected' | 'connecting' | 'disconnected' | 'error'}
            statusError={undefined}
            cwd={activeTab?.cwd}
            vscodeAvailable={vscodeAvailable}
            settingsDir={activeTab?.settingsDir}
            conversationId={activeTab?.conversationId}
            showControls={!!tabId}
            terminalVisible={viewerVisible.value}
            jsonlVisible={jsonlVisible.value}
            onDisconnect={() => bridge.closeTab(tabId)}
            onToggleTerminal={() => {
              const next = !viewerVisible.value
              viewerVisible.value = next
              storage.setItem('jsonl-viewer-visible', String(next))
            }}
            onToggleJsonl={() => {
              const next = !jsonlVisible.value
              jsonlVisible.value = next
              storage.setItem('jsonl-panel-visible', String(next))
            }}
            onDownloadJsonl={() => bridge.downloadJsonl(tabId)}
          />
        </>
      }
      content={
        <ContentArea
          jsonlPanel={
            jsonlVisible.value ? (
              <div class="chat-panel" style="position:relative">
                <JsonlViewer />
                <ToolCounterToast />
              </div>
            ) : null
          }
          resizeHandle={null}
          terminalPanel={<TerminalPanel visible={viewerVisible.value} showResizeHandle={jsonlVisible.value} />}
        />
      }
      inputBar={
        <>
          <div style="position:relative;max-width:800px;width:100%;margin:0 auto;box-sizing:border-box">
            <ActivityIndicator
              visible={!!(activity?.isWorking)}
              text={activity?.rawTitle ?? ''}
              thinking={activeThinkingPreview.value}
              color={(() => {
                const t = activeTab?.sessionTitle
                if (!t) return undefined
                // Prefer the user's manual pin colour (per-session override),
                // else fall back to the deterministic agent palette pick. Both
                // helpers return a hex string usable directly as a CSS colour.
                return resolvePinnedColor(pinnedTitleColors.value[t]) ?? getAgentColor(t) ?? undefined
              })()}
            />
          </div>
          <WidgetsPanel />
          <InputBar />
        </>
      }
      agentTiles={<SubagentButtonsRow />}
      overlay={<><SubagentFullscreen /><ToolUsageFullscreen /></>}
    />
  )
}
