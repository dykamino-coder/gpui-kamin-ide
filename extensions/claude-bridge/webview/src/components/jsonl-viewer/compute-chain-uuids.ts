// Chain-membership filter extracted from JsonlViewer.tsx (Sprint 5 / E1).
//
// CLI's `buildConversationChain` walks backwards from the SINGLE newest
// non-sidechain leaf. That made the bridge drop user messages whenever
// the newest leaf turned out to be a synthetic record CLI inserts between
// turns (system/api_error/attachment) whose parent chain didn't run
// through the user's own message — its ancestors are loaded directly,
// and the user-entry sat orphaned by the filter even though it's clearly
// part of the conversation.
//
// Multi-tip variant: walk back from EVERY non-sidechain leaf and union
// the chains. Orphan filtering still works (a true orphan branch has its
// OWN chain that doesn't reach the live tip — but it's also not a child
// of anyone outside it, so its own walk covers only itself; downstream
// rules then have a chance to flag it). In practice for bridge the union
// covers every legitimate record the CLI wrote; orphan abort'ed turns
// from Esc are rare and mostly visually identical to a normal early-cut
// turn anyway.
//
// Streaming-stub records (uuid=`stream-msg_*`, __streaming defined) are
// excluded from the walk so their `parentUuid: null` doesn't make them
// the newest leaf and break everything else.

import type { JsonlEntryData } from '../../types/jsonl'

export function computeChainUuids(entries: JsonlEntryData[]): Set<string> | null {
  if (entries.length === 0) return null
  const byUuid = new Map<string, JsonlEntryData>()
  const childUuids = new Set<string>()
  for (const e of entries) {
    // Skip live MITM streaming entries from chain construction. They have a
    // synthetic uuid (`stream-msg_*`) and parentUuid=null because the bridge
    // sees only the response side. Including them would make the streaming
    // entry the "tip", and walking back from a null parentUuid would yield
    // a chain of just {streaming.uuid} — filtering ALL history entries out
    // of the viewer until JSONL flush replaces the stub.
    if ((e as any).__streaming !== undefined) continue
    const uuid = (e as any).uuid
    if (typeof uuid !== 'string') continue
    byUuid.set(uuid, e)
    const parent = (e as any).parentUuid
    if (typeof parent === 'string') childUuids.add(parent)
  }
  // Multi-tip walk: every non-sidechain leaf seeds a back-walk; results
  // unioned. Covers cases where the newest leaf is a synthetic CLI record
  // whose chain misses the user's actual messages.
  const chain = new Set<string>()
  for (const [uuid, e] of byUuid) {
    if (childUuids.has(uuid)) continue
    if ((e as any).isSidechain) continue
    let cur: JsonlEntryData | undefined = e
    while (cur) {
      const u = (cur as any).uuid as string | undefined
      if (!u || chain.has(u)) break
      chain.add(u)
      const p = (cur as any).parentUuid as string | undefined
      cur = p ? byUuid.get(p) : undefined
    }
  }
  // Safety net: still missing a user-entry? Disable filter completely —
  // better to show all than to drop the user's own message.
  for (const e of entries) {
    if ((e as any).type !== 'user') continue
    if ((e as any).isSidechain) continue
    if ((e as any).isMeta) continue
    const u = (e as any).uuid as string | undefined
    if (!u) continue
    if (!chain.has(u)) {
      const now = Date.now()
      if (now - lastDiagAt > 2000) {
        lastDiagAt = now
        // eslint-disable-next-line no-console
        console.warn('[ocb] chain-uuids: multi-tip walk still missed user entry — disabling filter', {
          orphanUserUuid: u,
          chainSize: chain.size,
          totalEntries: byUuid.size,
          orphanUserParent: (e as any).parentUuid,
        })
      }
      return null
    }
  }
  if (chain.size === 0) return null
  return chain
}

let lastDiagAt = 0
