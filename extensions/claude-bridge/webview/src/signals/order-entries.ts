// Stable topological ordering for the chat entry array.
//
// Render order == array order (nothing downstream re-sorts). Two channels feed
// the store and race: the MITM proxy stream (stub appended at the end, no `_ord`)
// and the polled JSONL tail + reverse-replay (`_ord`-tagged). A plain
// "no-`_ord` floats to the bottom" sort is wrong whenever a live entry isn't
// actually the newest — e.g. the assistant stub arrives before the tail delivers
// its own user prompt → assistant renders above its prompt and stays there.
//
// Fix: a STABLE topological sort. Primary key is `_ord` (ground truth for
// everything the server replayed — the only thing that correctly orders
// reverse-emitted segments + compaction boundaries), tie-broken by arrival
// index. The parentUuid→uuid chain is used ONLY to guarantee a child never
// renders before its parent, which repositions the live stub after its prompt
// once the canonical entry (with a real parentUuid) lands. Roots (incl. live
// stubs with parentUuid:null) stay in `(_ord, arrival)` order.
//
// O(n log n), and called ONLY from appendJsonlEntries (batch delivery, a few
// times/sec) — never on per-token streaming deltas — so it can't reintroduce
// the per-delta re-sort freeze.

interface Orderable {
  uuid?: string
  parentUuid?: string
  _ord?: number
  timestamp?: string
  /** Ленивый кэш Date.parse(timestamp): записи иммутабельны, а parse гонялся
   *  по ВСЕМУ стору на каждый батч (аудит #70 B2). NaN тоже кэшируется. */
  _tsMs?: number
}

const MAX = Number.MAX_SAFE_INTEGER

/** Sort key — `timestamp` FIRST, `_ord` only as a fallback.
 *
 *  `timestamp` is the one globally-chronological, replay-STABLE key: it is the
 *  same no matter how many times the file is re-replayed, and it is present on
 *  EVERY rendered row (user/assistant + every rendered system subtype — verified
 *  on a 34k-line prod transcript: zero rendered rows lack it). `_ord` is only a
 *  per-PROCESS file-position counter — it resets on a server restart and is
 *  re-based on every replay, and it is ABSENT on records that come back by a
 *  different path (scroll-up + archived-segment loads read raw rows with no
 *  `_ord`). The old key checked `_ord` first, so those no-`_ord` rows fell to the
 *  ts branch (~1.7e12) while `_ord` rows returned ~1e5 — two INCOMPARABLE scales,
 *  so any real message that arrived without `_ord` (an old dead-branch turn pulled
 *  in by scroll-up) sorted AFTER all of today's replayed history and masqueraded
 *  as the newest message (the "chat tail shows a message from weeks ago" report).
 *
 *  Keying on ts first puts every dated row on ONE scale, in true chronological
 *  order, regardless of delivery path. `_ord` survives only to order the handful
 *  of ts-LESS replayed rows (bookkeeping the viewer doesn't render) among
 *  themselves; a live stub with neither ts nor `_ord` is genuinely newest → MAX.
 *  The parentUuid→child topo below still overrides any sub-second ts skew (a
 *  back-dated synthetic attachment never floats above its own parent). */
function sortKey(e: Orderable): number {
  let ts = e._tsMs
  if (ts === undefined) {
    ts = e.timestamp ? Date.parse(e.timestamp) : NaN
    e._tsMs = ts // мемо на записи: один parse на строку за всю её жизнь
  }
  if (Number.isFinite(ts)) return ts
  if (typeof e._ord === 'number') return e._ord
  return MAX
}

export function orderEntries<T extends Orderable>(entries: T[]): T[] {
  const n = entries.length
  if (n < 2) return entries

  // Precompute the sort key ONCE per entry. `sortKey` does a `Date.parse` (not
  // free), and it is consulted in the fast-path scan AND O(n log n) times across
  // the heap comparisons — computing it n times up front instead turns that into
  // one parse per row, the dominant cost on a full 4000-row window.
  const keys = new Float64Array(n)
  for (let i = 0; i < n; i++) keys[i] = sortKey(entries[i]!)

  // First occurrence wins (uuid dedup happens upstream, but be defensive).
  const idxByUuid = new Map<string, number>()
  for (let i = 0; i < n; i++) {
    const u = entries[i]!.uuid
    if (u && !idxByUuid.has(u)) idxByUuid.set(u, i)
  }

  // Fast path (restores the cheap bypass the old `batchHasOrd` gate gave us):
  // if the array is ALREADY in `_ord`-ascending order with every child after
  // its parent, it's a valid output as-is — skip the whole heap machinery. This
  // is the common case: a streaming stub or a fresh tail entry appended at the
  // end already sits in the right place (its `_ord`/arrival is highest). Safe —
  // never a false positive: if every element is ready (parent already emitted)
  // and keys are non-decreasing, the heap would pop them in exactly this order.
  let inOrder = true
  let prevKey = -1
  for (let i = 0; i < n; i++) {
    const k = keys[i]!
    if (k < prevKey) { inOrder = false; break }
    prevKey = k
    const p = entries[i]!.parentUuid
    if (p) { const pi = idxByUuid.get(p); if (pi !== undefined && pi > i) { inOrder = false; break } }
  }
  if (inOrder) return entries

  const indeg = new Int32Array(n)
  const children: number[][] = Array.from({ length: n }, () => [])
  for (let i = 0; i < n; i++) {
    const p = entries[i]!.parentUuid
    if (!p) continue
    const pi = idxByUuid.get(p)
    if (pi !== undefined && pi !== i) { indeg[i] = 1; children[pi]!.push(i) }
  }

  const key = (i: number): number => keys[i]!
  // Ready-set = a min-heap of indegree-0 indices ordered by (`_ord`, arrival).
  const heap: number[] = []
  const less = (a: number, b: number): boolean => {
    const ka = key(a), kb = key(b)
    return ka !== kb ? ka < kb : a < b // arrival-index tie-break = stable
  }
  const up = (from: number): void => {
    let i = from
    while (i > 0) {
      const par = (i - 1) >> 1
      if (less(heap[i]!, heap[par]!)) { const t = heap[i]!; heap[i] = heap[par]!; heap[par] = t; i = par } else break
    }
  }
  const push = (x: number): void => { heap.push(x); up(heap.length - 1) }
  const pop = (): number => {
    const top = heap[0]!
    const last = heap.pop()!
    if (heap.length) {
      heap[0] = last
      let i = 0
      for (;;) {
        const l = 2 * i + 1, r = 2 * i + 2
        let m = i
        if (l < heap.length && less(heap[l]!, heap[m]!)) m = l
        if (r < heap.length && less(heap[r]!, heap[m]!)) m = r
        if (m === i) break
        const t = heap[i]!; heap[i] = heap[m]!; heap[m] = t; i = m
      }
    }
    return top
  }

  for (let i = 0; i < n; i++) if (indeg[i] === 0) push(i)

  const out: T[] = []
  while (heap.length) {
    const i = pop()
    out.push(entries[i]!)
    for (const c of children[i]!) if (--indeg[c]! === 0) push(c)
  }

  // Safety net: a cycle or a parent-that-points-forward-and-loops would leave
  // entries unemitted (measured chain gaps = 0, so this ~never fires). Append
  // them in (`_ord`, arrival) order rather than dropping anything.
  if (out.length < n) {
    const seen = new Set(out)
    const rest: number[] = []
    for (let i = 0; i < n; i++) if (!seen.has(entries[i]!)) rest.push(i)
    rest.sort((a, b) => (less(a, b) ? -1 : 1)) // same (`_ord`, arrival) order as the heap
    for (const i of rest) out.push(entries[i]!)
  }
  return out
}
