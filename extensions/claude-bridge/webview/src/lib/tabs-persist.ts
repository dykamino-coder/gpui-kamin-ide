import type { TabInfo, PersistedTabState } from '../../shared/types'
import { tabs, pinnedTabs, tabOrder, topTabsOrder, ghostPinnedTabs } from '../signals/tabs'

const SAVE_DEBOUNCE_MS = 200
let saveTimer: ReturnType<typeof setTimeout> | null = null

/** Mirror of `normalizeCwd` in SidebarTree.tsx — keep grouping consistent
 *  between the tree (which groups by this key) and the persisted order map. */
function normalizeCwdKey(cwd: string | undefined): string {
  return (cwd || '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase() || 'unknown'
}

function matchKey(tab: { conversationId?: string; cwd?: string; label?: string; sessionTitle?: string }): string {
  if (tab.conversationId) return `conv:${tab.conversationId}`
  return `cwd:${(tab.cwd || '').toLowerCase()}|title:${tab.sessionTitle || tab.label || ''}`
}

export function buildPersistedSnapshot(tabList: TabInfo[], pinned: Set<string>, order: Map<string, string[]>): PersistedTabState[] {
  const ordered: TabInfo[] = []
  const byFolder = new Map<string, TabInfo[]>()
  for (const t of tabList) {
    const key = normalizeCwdKey(t.cwd)
    if (!byFolder.has(key)) byFolder.set(key, [])
    byFolder.get(key)!.push(t)
  }
  for (const [folderKey, list] of byFolder.entries()) {
    const ids = order.get(folderKey)
    if (ids && ids.length > 0) {
      const idx = new Map<string, number>(ids.map((id, i) => [id, i]))
      list.sort((a, b) => (idx.get(a.id) ?? 1e9) - (idx.get(b.id) ?? 1e9) || a.createdAt.localeCompare(b.createdAt))
    } else {
      list.sort((a, b) => a.createdAt.localeCompare(b.createdAt))
    }
    ordered.push(...list)
  }
  const topIdx = new Map<string, number>(topTabsOrder.value.map((id, i) => [id, i]))
  const liveEntries: PersistedTabState[] = ordered.map((t, i) => ({
    id: t.id,
    title: t.sessionTitle || t.label || '',
    cwd: t.cwd || '',
    conversationId: t.conversationId,
    pinned: pinned.has(t.id),
    order: i,
    topOrder: topIdx.get(t.id) ?? -1,
  }))
  // Include ghost pinned entries so sleeping pins survive restarts. Ghosts
  // never match any live tab's conversationId by definition.
  const liveConvs = new Set(liveEntries.map(e => e.conversationId).filter(Boolean))
  const ghosts = ghostPinnedTabs.value.filter(g => !g.conversationId || !liveConvs.has(g.conversationId))
  return [...liveEntries, ...ghosts]
}

export function scheduleSaveTabsState(): void {
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(async () => {
    saveTimer = null
    const bridge = (window as any).electronBridge
    if (!bridge?.saveTabsState) return
    try {
      const snapshot = buildPersistedSnapshot(tabs.value, pinnedTabs.value, tabOrder.value)
      await bridge.saveTabsState(snapshot)
    } catch { /* best-effort */ }
  }, SAVE_DEBOUNCE_MS)
}

/** Apply a restored snapshot to the signals. Matches tabs by conversationId
 *  first, then falls back to (cwd + title). Unknown persisted entries are
 *  silently dropped — the tab is gone, its pin goes with it. */
export function applyRestoredState(persisted: PersistedTabState[], tabList: TabInfo[]): void {
  const pinned = new Set<string>()
  const byConv = new Map<string, TabInfo>()
  const byFallback = new Map<string, TabInfo>()
  for (const t of tabList) {
    if (t.conversationId) byConv.set(t.conversationId, t)
    byFallback.set(matchKey(t), t)
  }
  // Folder ordering groups by cwd (lowercased) like SidebarTree grouping.
  const orderByFolder = new Map<string, Array<{ id: string; order: number }>>()
  for (const p of persisted) {
    let match: TabInfo | undefined
    if (p.conversationId) match = byConv.get(p.conversationId)
    if (!match) match = byFallback.get(matchKey({ conversationId: p.conversationId, cwd: p.cwd, sessionTitle: p.title }))
    if (!match) continue
    if (p.pinned) pinned.add(match.id)
    const folderKey = normalizeCwdKey(match.cwd)
    if (!orderByFolder.has(folderKey)) orderByFolder.set(folderKey, [])
    orderByFolder.get(folderKey)!.push({ id: match.id, order: p.order })
  }
  const orderMap = new Map<string, string[]>()
  for (const [k, list] of orderByFolder.entries()) {
    list.sort((a, b) => a.order - b.order)
    orderMap.set(k, list.map(x => x.id))
  }
  pinnedTabs.value = pinned
  tabOrder.value = orderMap
  // Restore top-strip linear order.
  const topEntries: Array<{ id: string; topOrder: number }> = []
  for (const p of persisted) {
    const topOrder = (p as any).topOrder
    if (typeof topOrder !== 'number' || topOrder < 0) continue
    let match: TabInfo | undefined
    if (p.conversationId) match = byConv.get(p.conversationId)
    if (!match) match = byFallback.get(matchKey({ conversationId: p.conversationId, cwd: p.cwd, sessionTitle: p.title }))
    if (!match) continue
    topEntries.push({ id: match.id, topOrder })
  }
  topEntries.sort((a, b) => a.topOrder - b.topOrder)
  topTabsOrder.value = topEntries.map(e => e.id)
  // Ghost pinned entries — pinned persisted entries we couldn't match to a
  // live tab. These render as sleeping chips in the top TabsBar; clicking
  // one triggers resumeSession.
  const ghosts: PersistedTabState[] = []
  for (const p of persisted) {
    if (!p.pinned) continue
    let match: TabInfo | undefined
    if (p.conversationId) match = byConv.get(p.conversationId)
    if (!match) match = byFallback.get(matchKey({ conversationId: p.conversationId, cwd: p.cwd, sessionTitle: p.title }))
    if (match) continue
    ghosts.push(p)
  }
  ghostPinnedTabs.value = ghosts
  // Sync the `pinned` flag onto the tabs signal so SessionItem can read it.
  tabs.value = tabList.map(t => ({ ...t, pinned: pinned.has(t.id) }))
}

/** When a pinned tab's session is closed server-side, snapshot it into the
 *  ghost list so it keeps rendering and the user can revive it by click. */
export function ghostifyClosedPinned(tab: TabInfo): void {
  const topIdx = new Map<string, number>(topTabsOrder.value.map((id, i) => [id, i]))
  const snap: PersistedTabState = {
    id: tab.id,
    title: tab.sessionTitle || tab.label || '',
    cwd: tab.cwd || '',
    conversationId: tab.conversationId,
    pinned: true,
    order: 0,
    topOrder: topIdx.get(tab.id) ?? -1,
  }
  // Dedupe by conversationId when possible, else by id.
  const cur = ghostPinnedTabs.value.filter(g =>
    snap.conversationId ? g.conversationId !== snap.conversationId : g.id !== snap.id,
  )
  ghostPinnedTabs.value = [...cur, snap]
  scheduleSaveTabsState()
}

/** When a fresh tab appears that matches a ghost's conversationId, reclaim:
 *  remove the ghost, pin the new tab id, carry over its topOrder slot. */
export function reclaimGhostIfMatch(tab: TabInfo): void {
  if (!tab.conversationId) return
  const ghosts = ghostPinnedTabs.value
  const match = ghosts.find(g => g.conversationId === tab.conversationId)
  if (!match) return
  ghostPinnedTabs.value = ghosts.filter(g => g !== match)
  // Carry pin over.
  const nextPinned = new Set(pinnedTabs.value)
  nextPinned.add(tab.id)
  pinnedTabs.value = nextPinned
  tabs.value = tabs.value.map(t => t.id === tab.id ? { ...t, pinned: true } : t)
  // Carry the top-strip slot over if it was placed.
  if (typeof match.topOrder === 'number' && match.topOrder >= 0) {
    const order = [...topTabsOrder.value.filter(id => id !== tab.id)]
    const insertAt = Math.min(match.topOrder, order.length)
    order.splice(insertAt, 0, tab.id)
    topTabsOrder.value = order
  }
  scheduleSaveTabsState()
}

/** Move `srcId` to the index of `dstId` in the top strip linear order.
 *  `after=true` drops the source AFTER the destination (right-half drop),
 *  `after=false` places it BEFORE (left-half drop, the default). */
export function moveTopTab(srcId: string, dstId: string, after = false): void {
  if (srcId === dstId) return
  const list = tabs.value
  if (!list.some(t => t.id === srcId) || !list.some(t => t.id === dstId)) return
  const existing = topTabsOrder.value
  const known = new Set(existing)
  const base = [...existing, ...list.map(t => t.id).filter(id => !known.has(id))]
  const srcIdx = base.indexOf(srcId)
  const dstIdx = base.indexOf(dstId)
  if (srcIdx < 0 || dstIdx < 0) return
  base.splice(srcIdx, 1)
  const newDstIdx = base.indexOf(dstId)
  const insertAt = newDstIdx < 0 ? dstIdx : (after ? newDstIdx + 1 : newDstIdx)
  base.splice(insertAt, 0, srcId)
  topTabsOrder.value = base
  scheduleSaveTabsState()
}

/** Reset all tab-persist signals — call when the user's auth token changes
 *  at runtime (account switch from Settings panel). The on-disk state file
 *  is tokenHash-scoped, so the next save will naturally produce a new file
 *  under the new token identity; we just need to clear in-memory first so
 *  the user doesn't see stale pins during the transition. */
export function resetTabsStateForAccountSwitch(): void {
  pinnedTabs.value = new Set()
  tabOrder.value = new Map()
  topTabsOrder.value = []
  ghostPinnedTabs.value = []
  tabs.value = tabs.value.map(t => ({ ...t, pinned: false }))
  // Fire a save so the new tokenHash immediately owns an empty snapshot on disk.
  scheduleSaveTabsState()
}

/** Wrap `bridge.setConfig` so a token change triggers the reset above.
 *  Caller passes current config getter + new config; we compare tokens and
 *  invalidate tab state only when the auth token actually changed. Whisper/
 *  layout/notification fields updating alone don't trigger a reset. */
export async function setConfigAndHandleTokenChange(
  bridge: { getConfig: () => Promise<any>; setConfig: (c: any) => void },
  next: any,
): Promise<void> {
  let prevToken = ''
  try {
    const prev = await bridge.getConfig()
    prevToken = (prev?.token || '') as string
  } catch { /* ignore */ }
  const nextToken = (next?.token || '') as string
  bridge.setConfig(next)
  if (prevToken && nextToken && prevToken !== nextToken) {
    resetTabsStateForAccountSwitch()
  }
}

export function togglePinTab(tabId: string): void {
  const cur = new Set(pinnedTabs.value)
  if (cur.has(tabId)) cur.delete(tabId)
  else cur.add(tabId)
  pinnedTabs.value = cur
  tabs.value = tabs.value.map(t => t.id === tabId ? { ...t, pinned: cur.has(t.id) } : t)
  scheduleSaveTabsState()
}

/** Move `srcId` to be positioned at `dstId`'s spot within their shared folder.
 *  If the two tabs belong to different folders, the move is ignored. */
export function moveTabTo(srcId: string, dstId: string): void {
  if (srcId === dstId) return
  const list = tabs.value
  const src = list.find(t => t.id === srcId)
  const dst = list.find(t => t.id === dstId)
  if (!src || !dst) return
  const srcFolder = normalizeCwdKey(src.cwd)
  const dstFolder = normalizeCwdKey(dst.cwd)
  if (srcFolder !== dstFolder) return
  const folderTabs = list.filter(t => (normalizeCwdKey(t.cwd)) === srcFolder)
  const existing = tabOrder.value.get(srcFolder)
  const current = existing && existing.length > 0
    ? [...existing.filter(id => folderTabs.some(t => t.id === id)),
       ...folderTabs.filter(t => !existing.includes(t.id)).sort((a, b) => a.createdAt.localeCompare(b.createdAt)).map(t => t.id)]
    : [...folderTabs].sort((a, b) => a.createdAt.localeCompare(b.createdAt)).map(t => t.id)
  const srcIdx = current.indexOf(srcId)
  const dstIdx = current.indexOf(dstId)
  if (srcIdx < 0 || dstIdx < 0) return
  current.splice(srcIdx, 1)
  const insertAt = current.indexOf(dstId)
  current.splice(insertAt < 0 ? dstIdx : insertAt, 0, srcId)
  const next = new Map(tabOrder.value)
  next.set(srcFolder, current)
  tabOrder.value = next
  scheduleSaveTabsState()
}
