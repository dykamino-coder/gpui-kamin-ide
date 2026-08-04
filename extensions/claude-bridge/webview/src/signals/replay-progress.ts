import { signal } from '@preact/signals'

// Live percent of a session's historical replay, per tab, while the transcript
// streams from the server (the "15–20s download" on a big session). The server
// sends byte progress per batch; cleared to null at replayComplete. Keyed by
// tab so switching away and back doesn't cross wires.
//
// Kept in its own module rather than in ui.ts: `signals/jsonl.ts` clears it on
// tab wipe, and jsonl.ts is imported by pure-signal unit tests that must not
// transitively pull ui.ts's `@bridge/storage` (which isn't aliased in those
// tests' environment).
export const replayProgressByTab = signal<Map<string, number>>(new Map())

export function setReplayProgress(tabId: string, pct: number | null): void {
  const cur = replayProgressByTab.value
  const has = cur.has(tabId)
  if (pct === null) {
    if (!has) return
    const next = new Map(cur); next.delete(tabId); replayProgressByTab.value = next
    return
  }
  if (cur.get(tabId) === pct) return
  const next = new Map(cur); next.set(tabId, pct); replayProgressByTab.value = next
}
