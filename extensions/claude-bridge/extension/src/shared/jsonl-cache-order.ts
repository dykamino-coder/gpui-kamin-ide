// File order of the host's transcript cache (`ConnectionManager.cachedJsonlEntries`).
//
// The cache mixes two kinds of rows. Page, replay and cursor-resume rows carry
// `_pos` (byte offset in the CLI file) or a legacy `_ord`. Live-tail rows carry
// neither: the server keeps the tail wire contract untagged
// (server/src/core/pty/jsonl-watcher.ts, "Live records stay untagged").
//
// The cap and the renderer replay used to rank rows by `_pos ?? _ord ?? 0`, so
// every live row ranked as the OLDEST row of the file. Two losses followed:
//  - trimJsonlCache computed its cutoff as 0 once more than `over` rows were
//    untagged and then kept only rows ranked above it, i.e. it dropped the WHOLE
//    live session at once instead of `over` rows;
//  - the renderer replay sorted live rows below the tagged first page and cut
//    the middle of the session out of the tail it sends.
//
// A live row is newer than anything already cached when it arrives, and rows
// arrive in file order on the tail. So an untagged row ranks after every tagged
// row, by arrival. When uncertain this keeps a row rather than losing it.

interface Positioned { _pos?: number; _ord?: number }

function tagOf(e: Positioned): number | undefined {
  if (typeof e._pos === 'number') return e._pos
  if (typeof e._ord === 'number') return e._ord
  return undefined
}

/** Rank of every cached row in file order; untagged live rows follow the tagged ones by arrival. */
export function cacheFileOrderRanks(entries: readonly Positioned[]): Float64Array {
  const ranks = new Float64Array(entries.length)
  let maxTagged = -1
  for (const e of entries) {
    const t = tagOf(e)
    if (t !== undefined && t > maxTagged) maxTagged = t
  }
  for (let i = 0; i < entries.length; i++) {
    const t = tagOf(entries[i]!)
    ranks[i] = t !== undefined ? t : maxTagged + 1 + i
  }
  return ranks
}

/** Row indices sorted by file-order rank (stable on ties). */
export function cacheFileOrder(entries: readonly Positioned[]): number[] {
  const ranks = cacheFileOrderRanks(entries)
  const idx = Array.from({ length: entries.length }, (_, i) => i)
  idx.sort((a, b) => ranks[a]! - ranks[b]! || a - b)
  return idx
}

/** The newest `count` rows of the cache, oldest first. */
export function newestInFileOrder<T extends Positioned>(entries: readonly T[], count: number): T[] {
  const order = cacheFileOrder(entries)
  return order.slice(Math.max(0, order.length - count)).map((i) => entries[i]!)
}
