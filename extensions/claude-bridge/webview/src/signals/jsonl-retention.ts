// Which background tabs to drop from renderer memory.
//
// Every open tab kept its FULL transcript in the webview renderer — and all the
// Bridge panels are same-origin iframes, so they share ONE WebView2 renderer
// process and one memory ceiling. Several big sessions open at once walked that
// ceiling until the renderer died with OUT_OF_MEMORY (webview_watchdog:
// FRAME_RENDER_PROCESS_EXITED / OUT_OF_MEMORY). Closing a tab already released
// its graph; staying open was the unbounded case.
//
// Dropping a background tab is safe because the transcript also lives in the
// extension host (`ConnectionManager.cachedJsonlEntries`, uncapped) and the
// viewer re-requests a replay whenever it activates a tab with no entries. We
// evict WHOLE tabs rather than trimming to a tail: a trimmed tab is non-empty,
// so nothing would refill it and the user would silently lose history.
//
// Pure selection so the policy is testable without signals or a DOM.

export interface TabRetention {
  tabId: string
  entries: number
  /** Lower = less recently active. The active tab is never evicted regardless. */
  lastActiveSeq: number
}

export interface EvictionPolicy {
  /** Never evict these (active tab, tabs mid-response — their live stream would
   *  be dropped and re-requested for nothing). */
  keep: ReadonlySet<string>
  /** Total entries to stay under, summed across ALL tabs. */
  budgetEntries: number
  /** Don't bother evicting a tab smaller than this — the refill would cost more
   *  than the memory it frees. */
  minTabEntries: number
  /** Keep this many MOST-RECENTLY-ACTIVE tabs resident regardless of the budget.
   *
   *  The budget alone made switching between two BIG sessions slow: two 30k
   *  sessions total 60k+, over budget, so every switch evicted the tab just
   *  left (it becomes the LRU) and switching back re-replayed the whole
   *  transcript from the server — the reported "10 seconds to switch, and both
   *  are supposedly in memory". They were not: the budget had freed one. Holding
   *  the few most-recent tabs means the sessions a user actually ping-pongs
   *  between stay resident and switch instantly; only genuinely idle ones are
   *  dropped. A cold render of the largest real segment is ~32ms, so a resident
   *  switch is imperceptible — the whole cost was the eviction. */
  keepRecentCount: number
}

/** Least-recently-active background tabs first, until the total fits the budget.
 *  Returns tab ids to clear, in eviction order. */
export function selectTabsToEvict(tabs: readonly TabRetention[], policy: EvictionPolicy): string[] {
  let total = 0
  for (const t of tabs) total += t.entries
  if (total <= policy.budgetEntries) return []

  // The N most-recently-active tabs are protected too — the ones the user is
  // switching between must not be evicted just because their combined size is
  // over budget, or the switch pays a full re-replay every time.
  const recentProtected = new Set(
    [...tabs].sort((a, b) => b.lastActiveSeq - a.lastActiveSeq)
      .slice(0, Math.max(0, policy.keepRecentCount))
      .map((t) => t.tabId),
  )

  const candidates = tabs
    .filter((t) => !policy.keep.has(t.tabId) && !recentProtected.has(t.tabId) && t.entries >= policy.minTabEntries)
    .sort((a, b) => a.lastActiveSeq - b.lastActiveSeq)

  const evict: string[] = []
  for (const t of candidates) {
    if (total <= policy.budgetEntries) break
    evict.push(t.tabId)
    total -= t.entries
  }
  return evict
}
