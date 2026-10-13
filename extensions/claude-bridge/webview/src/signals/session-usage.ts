import { signal } from '@preact/signals'
import type { SessionUsage } from '../../shared/types'

// Live subscription limits (5-hour / 7-day) and context fill per tab, pushed by
// the server from the CLI status line (`session:usage`). Own module, like
// replay-progress.ts, so pure-signal tests do not pull ui.ts storage.
export const usageByTab = signal<Map<string, SessionUsage>>(new Map())

export function setTabUsage(tabId: string, usage: SessionUsage | null): void {
  const cur = usageByTab.value
  if (usage === null) {
    if (!cur.has(tabId)) return
    const next = new Map(cur); next.delete(tabId); usageByTab.value = next
    return
  }
  const next = new Map(cur); next.set(tabId, usage); usageByTab.value = next
}
