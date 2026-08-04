import { computed, signal } from '@preact/signals'
import type { TabInfo, TreeNode, PersistedTabState } from '../../shared/types'

export const tabs = signal<TabInfo[]>([])
export const activeTabId = signal<string | null>(null)

/** Титул АКТИВНОЙ сессии — узкий срез широкого `tabs`: подписчики (каждый
 *  бабл ленты красит рамку цветом сессии) ререндерились на ЛЮБУЮ запись
 *  tabs (флап соединения, ретрай-счётчик); computed меняется только когда
 *  реально сменился титул или активный таб (аудит #70 D3). */
export const activeSessionTitle = computed<string | undefined>(() => {
  const id = activeTabId.value
  if (!id) return undefined
  return tabs.value.find(t => t.id === id)?.sessionTitle
})

// Highest switch sequence the webview has applied. `tab:switched` carries the
// seq the extension assigned it; a frame with a lower seq is a switch the user
// has already moved past — arriving late across the fork-IPC boundary — and is
// dropped instead of clobbering activeTabId (the overtake bug: "went back to A,
// the iframe finished loading B").
let lastSwitchSeq = -1

// The tab the user LOCALLY intends to be on (optimistic strip click, close
// fallback, toast route) — set immediately, before the extension round-trips a
// `tab:switched`. This is what closes the overtake race the seq alone couldn't:
// the optimistic writes never carried a seq, so a stale `tab:switched(B)` for a
// tab the user has since moved AWAY from was still accepted (5 > -1) and painted
// B over the A they clicked. Now such a frame is rejected unless it matches (or
// post-dates) the latest local intent. Cleared once the extension CONFIRMS the
// intended tab, so ordinary extension-initiated switches keep working.
let localIntentTab: string | null = null
let localIntentAt = 0
// The intent only SHIELDS against a stale frame for this long. Its confirming
// `tab:switched` normally lands in well under a second; if it never arrives (a
// dropped frame, or the host and webview disagreeing on the post-close tab), the
// intent must EXPIRE — otherwise applyTabSwitch would reject every future switch
// to a different tab forever and wedge tab switching until the user happens to
// click the stuck tab. This bounds any such wedge to the TTL.
const INTENT_TTL_MS = 4000

/** The user picked this tab locally (optimistic) — record it as the intent AND
 *  move the view now, so a stale extension switch can't overtake it. */
export function switchTabLocal(tabId: string | null): void {
  localIntentTab = tabId
  localIntentAt = Date.now()
  activeTabId.value = tabId
}

/** Apply an extension-driven tab switch, rejecting one that a newer switch — or
 *  a FRESH LOCAL intent — has already superseded. Returns whether it was applied. */
export function applyTabSwitch(tabId: string, seq: number): boolean {
  if (typeof seq === 'number' && seq <= lastSwitchSeq) return false
  // A pending local intent wins over a stale frame for a DIFFERENT tab (the
  // overtake) — but only until it's confirmed or expires, so it can't wedge.
  if (localIntentTab !== null && tabId !== localIntentTab && Date.now() - localIntentAt < INTENT_TTL_MS) {
    return false
  }
  if (typeof seq === 'number') lastSwitchSeq = seq
  if (tabId === localIntentTab || Date.now() - localIntentAt >= INTENT_TTL_MS) localIntentTab = null
  activeTabId.value = tabId
  return true
}
export const sessionTree = signal<TreeNode[] | null>(null)
export const tabModels = signal<Map<string, string>>(new Map())

/** Pinned set — source of truth for `pinned` UI state.
 *  Runtime pin toggles patch both this set and the `tabs` array so listeners
 *  don't need to query two places. */
export const pinnedTabs = signal<Set<string>>(new Set())
/** Per-folder ordering key list. When a folder is not in the map the default
 *  `createdAt` sort kicks in. Order applies within a folder group. */
export const tabOrder = signal<Map<string, string[]>>(new Map())
/** Linear ordering for the top TabsBar strip — ignores folder grouping, one
 *  list for the whole strip. Unknown ids fall to the end. */
export const topTabsOrder = signal<string[]>([])
/** Pinned sessions that are no longer live (tab was closed / never reopened
 *  after an app restart). Kept around so the top TabsBar can render them as
 *  "sleeping" chips — click revives the session via resume. When a live tab
 *  materialises with a matching conversationId, the ghost is reclaimed and
 *  its pin carries over to the real tab id. */
export const ghostPinnedTabs = signal<PersistedTabState[]>([])
