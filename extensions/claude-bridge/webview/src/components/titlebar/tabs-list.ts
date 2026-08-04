// Active-tabs list builder extracted from TabsBar.tsx (Sprint 5 / Stage E1).
// Merges live tabs with ghost-pinned (sessions whose server tab is closed)
// and sorts so pinned/ghost lead, ordered by topTabsOrder, with creation
// time as the tiebreaker.

import type { TabInfo, PersistedTabState, SavedSession } from '../../../shared/types'

export function buildActiveTabs(
  liveTabs: TabInfo[],
  pinned: Set<string>,
  order: string[],
  ghostPinned: PersistedTabState[],
  savedByConv: Map<string, SavedSession>,
): TabInfo[] {
  const pos = new Map<string, number>(order.map((id, i) => [id, i]))
  const live = liveTabs.filter(t =>
    t.status === 'connected' || t.status === 'connecting' || pinned.has(t.id),
  )
  const liveConvs = new Set(live.map(t => t.conversationId).filter(Boolean) as string[])
  const ghosts: TabInfo[] = ghostPinned
    .filter(g => !g.conversationId || !liveConvs.has(g.conversationId))
    .map(g => {
      // Ghosts pinned BEFORE the session got a sessionTitle (or via crash) carry
      // an empty `g.title`, which collapses the tab label to the auto counter
      // ("#1"). The same session may be visible in the sidebar with a proper
      // saved label — pull that as a fallback so both views match.
      const saved = g.conversationId ? savedByConv.get(g.conversationId) : undefined
      const cleanTitle = (g.title?.trim() && !/^#\d+$/.test(g.title.trim())) ? g.title.trim() : ''
      const fallbackTitle = saved?.label?.trim() && !/^#\d+$/.test(saved.label.trim()) ? saved.label.trim() : ''
      const resolvedTitle = cleanTitle || fallbackTitle || ''
      return {
        id: `ghost:${g.conversationId || g.id}`,
        cwd: g.cwd,
        label: resolvedTitle || g.title,
        folderName: (g.cwd.split(/[\\/]/).filter(Boolean).pop() || 'session'),
        createdAt: '',
        status: 'disconnected' as const,
        conversationId: g.conversationId,
        sessionTitle: resolvedTitle || undefined,
        pinned: true,
      }
    })
  const merged = [...live, ...ghosts]
  return merged.sort((a, b) => {
    const pa = (pinned.has(a.id) || a.id.startsWith('ghost:')) ? 0 : 1
    const pb = (pinned.has(b.id) || b.id.startsWith('ghost:')) ? 0 : 1
    if (pa !== pb) return pa - pb
    const oa = pos.get(a.id) ?? Number.MAX_SAFE_INTEGER
    const ob = pos.get(b.id) ?? Number.MAX_SAFE_INTEGER
    if (oa !== ob) return oa - ob
    return a.createdAt.localeCompare(b.createdAt)
  })
}
