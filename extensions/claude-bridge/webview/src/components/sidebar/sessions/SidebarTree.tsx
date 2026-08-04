import type { JSX } from 'preact'
import { tabs, sessionTree, activeTabId, tabOrder, pinnedTabs } from '../../../signals/tabs'
import { FolderGroup } from './FolderGroup'
import { useSavedSessions } from '../../../hooks/useSavedSessions'
import type { TabInfo } from '../../../../shared/types'
import type { SavedSession } from '../../../../shared/types'

function sortFolderTabs(list: TabInfo[], orderIds: string[] | undefined, pinned: Set<string>): TabInfo[] {
  const orderIdx = new Map<string, number>()
  if (orderIds) orderIds.forEach((id, i) => orderIdx.set(id, i))
  return [...list].sort((a, b) => {
    const aPin = pinned.has(a.id) ? 0 : 1
    const bPin = pinned.has(b.id) ? 0 : 1
    if (aPin !== bPin) return aPin - bPin
    const ao = orderIdx.has(a.id) ? orderIdx.get(a.id)! : Number.POSITIVE_INFINITY
    const bo = orderIdx.has(b.id) ? orderIdx.get(b.id)! : Number.POSITIVE_INFINITY
    if (ao !== bo) return ao - bo
    return a.createdAt.localeCompare(b.createdAt)
  })
}

function normalizeCwd(cwd: string): string {
  return (cwd || '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase() || 'unknown'
}

function buildDisplayName(cwdKey: string, allKeys: string[]): string {
  if (cwdKey === 'unknown') return 'No folder'
  const parts = cwdKey.split(/[/\\]/).filter(Boolean)
  const base = parts[parts.length - 1] || cwdKey
  const sameBase = allKeys.filter(k => {
    const p = k.split(/[/\\]/).filter(Boolean)
    return (p[p.length - 1] || k) === base
  })
  if (sameBase.length <= 1) return base
  const parent = parts.length >= 2 ? parts[parts.length - 2] : ''
  const idx = sameBase.indexOf(cwdKey)
  return parent ? `${base} (${parent})` : `${base} #${idx + 1}`
}

export function SidebarTree(): JSX.Element {
  const { savedSessions, loaded, removeSaved, removeSavedBatch } = useSavedSessions()
  const currentTabs = tabs.value
  const currentActiveTabId = activeTabId.value
  const orderMap = tabOrder.value
  const pinnedSet = pinnedTabs.value

  // Group tabs by normalized cwd
  const groups = new Map<string, TabInfo[]>()
  for (const tab of currentTabs) {
    const key = normalizeCwd(tab.cwd)
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key)!.push(tab)
  }

  // Collect active conversationIds
  const activeConvIds = new Set<string>()
  for (const tab of currentTabs) {
    if (tab.conversationId) activeConvIds.add(tab.conversationId)
  }

  // Build orphan saved sessions (no active tabs in folder)
  const orphanSaved = new Map<string, SavedSession[]>()
  if (loaded) {
    const activeCwds = new Set([...groups.keys()])
    for (const s of savedSessions) {
      const sCwd = normalizeCwd(s.cwd)
      if (activeCwds.has(sCwd)) continue
      if (activeConvIds.has(s.conversationId)) continue
      if (!orphanSaved.has(sCwd)) orphanSaved.set(sCwd, [])
      orphanSaved.get(sCwd)!.push(s)
    }
  }

  // All folder keys
  const allFolderKeys = [...new Set([...groups.keys(), ...orphanSaved.keys()])]

  // Build display names
  const displayNames = new Map<string, string>()
  for (const key of allFolderKeys) {
    displayNames.set(key, buildDisplayName(key, allFolderKeys))
  }

  // Sort: unknown first, then alphabetically
  const sortedFolders = allFolderKeys.sort((a, b) => {
    if (a === 'unknown') return -1
    if (b === 'unknown') return 1
    return (displayNames.get(a) || a).localeCompare(displayNames.get(b) || b)
  })

  if (sortedFolders.length === 0) {
    return (
      <div class="sidebar-tree">
        <div class="sidebar-empty">No sessions</div>
      </div>
    )
  }

  return (
    <div class="sidebar-tree">
      {sortedFolders.map(folderKey => {
        const folderTabs = groups.get(folderKey) || []
        const folderName = displayNames.get(folderKey) || folderKey
        const isUnknown = folderKey === 'unknown'

        const savedForFolder = loaded
          ? savedSessions.filter(s => {
              const sCwd = normalizeCwd(s.cwd)
              return sCwd === folderKey && !activeConvIds.has(s.conversationId)
            }).sort((a, b) => b.lastActivity.localeCompare(a.lastActivity))
          : []

        const folderCwd = folderTabs[0]?.cwd || orphanSaved.get(folderKey)?.[0]?.cwd || ''

        return (
          <FolderGroup
            key={folderKey}
            folderKey={folderKey}
            folderName={folderName}
            isUnknown={isUnknown}
            sessions={sortFolderTabs(folderTabs, orderMap.get(folderKey), pinnedSet)}
            savedSessions={savedForFolder}
            activeTabId={currentActiveTabId}
            folderCwd={folderCwd}
            onRemoveSaved={removeSaved}
            onRemoveSavedBatch={removeSavedBatch}
          />
        )
      })}
    </div>
  )
}
