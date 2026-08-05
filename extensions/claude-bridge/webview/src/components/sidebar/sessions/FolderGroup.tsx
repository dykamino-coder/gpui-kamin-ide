import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import type { TabInfo, SavedSession } from '../../../../shared/types'
import { FolderHeader } from './FolderHeader'
import { SessionItem } from './SessionItem'
import { SessionAgentTree } from './SessionAgentTree'
import { SavedSessionsToggle } from './SavedSessionsToggle'
import { SavedSessionsGroup } from './SavedSessionsGroup'
import { sidebarMode, activeCustomizePanel } from '../../../signals/ui'

interface FolderGroupProps {
  folderKey: string
  folderName: string
  isUnknown: boolean
  sessions: TabInfo[]
  savedSessions: SavedSession[]
  activeTabId: string | null
  folderCwd: string
  onRemoveSaved: (conversationId: string) => void
  onRemoveSavedBatch: (conversationIds: string[]) => void
}

export function FolderGroup({
  folderKey,
  folderName,
  isUnknown,
  sessions,
  savedSessions,
  activeTabId,
  folderCwd,
  onRemoveSaved,
  onRemoveSavedBatch,
}: FolderGroupProps): JSX.Element {
  const [savedExpanded, setSavedExpanded] = useState(false)

  async function handleAddSession(): Promise<void> {
    const bridge = (window as any).kaminBridge
    if (!bridge) return
    const config = await bridge.getConfig()
    if (!config.serverUrl || !config.token) return
    // For the "No folder" group folderCwd is empty — pass it explicitly so
    // the main process won't fall back to config.cwd (last project dir).
    await bridge.createTab({ serverUrl: config.serverUrl, token: config.token, cwd: folderCwd || '' })
    // If the user added a session while in Customize mode, drop them
    // back into the chat view so they immediately see the new tab —
    // otherwise the just-created chat would be hidden behind the
    // customize panel and feel unresponsive.
    if (sidebarMode.value === 'customize') {
      sidebarMode.value = 'sessions'
      activeCustomizePanel.value = null
    }
  }

  async function handleDeleteFolder(): Promise<void> {
    if (savedSessions.length === 0 && sessions.length === 0) return
    const totalCount = savedSessions.length + sessions.length
    const escape = (s: string) => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!))
    const show = (window as any).__showConfirmModal as
      | ((opts: { title: string; bodyHtml: string; confirmLabel?: string; isDanger?: boolean }) => Promise<boolean>)
      | undefined
    const ok = show
      ? await show({
          title: 'Delete folder?',
          bodyHtml: `<strong>${escape(folderName)}</strong> with all ${totalCount} session${totalCount === 1 ? '' : 's'} will be deleted. Active chats will close and saved sessions will be removed. This cannot be undone.`,
          confirmLabel: 'Delete folder',
          isDanger: true,
        })
      : false
    if (!ok) return
    const ids = savedSessions.map(s => s.conversationId)
    const bridge = (window as any).kaminBridge
    for (const tab of sessions) {
      bridge?.closeTab(tab.id)
    }
    if (ids.length > 0) {
      onRemoveSavedBatch(ids)
    }
  }

  const hasSaved = savedSessions.length > 0

  return (
    <>
      <FolderHeader
        name={folderName}
        isUnknown={isUnknown}
        onAdd={handleAddSession}
        onDelete={handleDeleteFolder}
        showDelete={sessions.length > 0 || hasSaved}
      />
      {sessions.map(tab => (
        <div key={tab.id}>
          <SessionItem
            tab={tab}
            isActive={tab.id === activeTabId}
            onClick={() => {
              const bridge = (window as any).kaminBridge
              bridge?.switchTab(tab.id)
            }}
          />
          <SessionAgentTree tabId={tab.id} />
        </div>
      ))}
      {hasSaved && (
        <>
          <SavedSessionsToggle
            count={savedSessions.length}
            isOpen={savedExpanded}
            onToggle={() => setSavedExpanded(v => !v)}
          />
          <SavedSessionsGroup
            sessions={savedSessions}
            isVisible={savedExpanded}
            onRemove={onRemoveSaved}
          />
        </>
      )}
    </>
  )
}
