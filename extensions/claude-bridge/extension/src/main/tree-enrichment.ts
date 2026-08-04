// Sidebar tree enrichment extracted from main/index.ts (Sprint 5 / E1).
// The server emits a tree of session nodes keyed by its own server-side
// sessionIds; we map them to our local TabInfo state so labels, status
// dots, and connection state line up. Drops ghost server sessions and
// adds tabs not yet in the tree (still connecting, no sessionId yet).

import type { TabManager } from './tab-manager'

export function enrichTreeWithTabs(tree: any[], tabManager: TabManager): any[] {
  const sessionMap = tabManager.getSessionTabMap() // serverSessionId → TabInfo
  const mappedTabIds = new Set<string>()

  function enrich(nodes: any[]): any[] {
    const result: any[] = []
    for (const node of nodes) {
      const tab = sessionMap.get(node.id)
      if (tab && node.type === 'session') {
        node.id = tab.id
        node.label = tab.label
        node.folderName = tab.folderName
        // Prefer tab sessionTitle, fallback to server node's sessionTitle
        node.sessionTitle = tab.sessionTitle || node.sessionTitle
        if (tab.status === 'connected') node.status = 'active'
        else if (tab.status === 'connecting') node.status = 'busy'
        else if (tab.status === 'disconnected') node.status = 'idle'
        if (node.children?.length > 0) node.children = enrich(node.children)
        mappedTabIds.add(tab.id)
        result.push(node)
      } else if (node.type !== 'session') {
        if (node.children?.length > 0) node.children = enrich(node.children)
        result.push(node)
      }
      // Unmapped server sessions are dropped
    }
    return result
  }

  const enriched = enrich(tree)

  // Add tabs not yet in server tree (still connecting, no sessionId yet).
  for (const tab of tabManager.listTabs()) {
    if (!mappedTabIds.has(tab.id)) {
      enriched.push({
        id: tab.id,
        type: 'session',
        label: tab.label,
        status: tab.status === 'connected' ? 'active' : tab.status === 'connecting' ? 'busy' : 'idle',
        cwd: tab.cwd,
        children: [],
        folderName: tab.folderName,
        sessionTitle: tab.sessionTitle,
      })
    }
  }

  return enriched
}
