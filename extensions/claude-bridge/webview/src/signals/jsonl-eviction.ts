// Keeps the shared WebView2 renderer under its memory ceiling by dropping
// background tabs' transcripts. See jsonl-retention.ts for the policy and why
// whole-tab eviction (not tail-trimming) is the safe shape.
import { jsonlEntriesByTab, clearJsonlEntries } from './jsonl'
import { selectTabsToEvict, type TabRetention } from './jsonl-retention'

/** Total entries retained across all tabs before eviction kicks in.
 *
 *  Raised hard from 12k/1k. At those numbers a normal two-session workflow
 *  evicted on EVERY switch, so coming back always meant a full re-replay — and
 *  a refill that races an in-flight one leaves the chat wrong, which costs the
 *  user far more than the memory ever saved.
 *
 *  The original numbers were chosen when the renderer was running to 4GB and
 *  dying with OUT_OF_MEMORY. A heap sampler later named the real culprit — the
 *  tool-result map, duplicated per segment and rebuilt on every arriving entry
 *  — and that is fixed. What a session's store actually costs was then measured
 *  at ~90MB for 23.5k entries, so two big sessions are ~180MB: worth keeping.
 *
 *  These now mean "something is genuinely pathological", not "two sessions are
 *  open". Lower them again only against a measurement, not a hunch. */
const BUDGET_ENTRIES = 60_000
const MIN_TAB_ENTRIES = 15_000
// The sessions a user actively ping-pongs between must stay resident so the
// switch is instant (~32ms render) instead of a full server re-replay (~10s).
// Three covers the common "flip between two, glance at a third" without letting
// a big fleet all stay pinned. Genuinely idle tabs beyond these still evict.
const KEEP_RECENT_TABS = 3

// Recency, bumped both when a tab is activated AND when entries land in it. The
// append bump is what protects a background tab that's mid-response: it stays
// the most-recently-touched and is evicted last, without needing a separate
// "is working" signal to keep in sync.
const lastActiveSeq = new Map<string, number>()
let seq = 0

export function touchTab(tabId: string): void {
  seq += 1
  lastActiveSeq.set(tabId, seq)
}

export function forgetTabRecency(tabId: string): void {
  lastActiveSeq.delete(tabId)
}

/** Drop background tabs until the retained total fits the budget. The evicted
 *  tabs refill from the extension host's cache when next activated (JsonlViewer
 *  re-requests a replay for a tab it finds empty). Returns what it evicted. */
export function sweepTabMemory(activeTabId: string | null): string[] {
  const tabs: TabRetention[] = []
  for (const [tabId, entries] of jsonlEntriesByTab.value) {
    tabs.push({ tabId, entries: entries.length, lastActiveSeq: lastActiveSeq.get(tabId) ?? 0 })
  }
  const evicted = selectTabsToEvict(tabs, {
    keep: new Set(activeTabId ? [activeTabId] : []),
    budgetEntries: BUDGET_ENTRIES,
    minTabEntries: MIN_TAB_ENTRIES,
    keepRecentCount: KEEP_RECENT_TABS,
  })
  for (const tabId of evicted) clearJsonlEntries(tabId)
  return evicted
}
