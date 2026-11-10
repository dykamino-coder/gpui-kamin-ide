// Stable batch-only topological ordering for the shared chat store.
// File positions win when the canonical window has complete position metadata.
// Legacy/mirror rows can lack it: in a mixed window use timestamps on ONE scale
// for everyone, rather than moving every unpositioned old row behind the live
// tail. Parent edges still override back-dated synthetic attachments.
// Never called on per-token deltas; O(n log n) with an already-ordered fast path.

interface Orderable {
  uuid?: string
  parentUuid?: string
  __uuids?: string[]
  __streaming?: boolean
  _ord?: number
  _pos?: number
  timestamp?: string
}

const MAX = Number.MAX_SAFE_INTEGER
// Do not spread a cached timestamp from one canonical block onto its merged
// bubble, whose timestamp belongs to a different block. Identity cache also
// releases entries when the resident transcript window is evicted.
const timestamps = new WeakMap<Orderable, number>()

/** Timestamp is stable across reconnects; `_ord` is only a replay-local fallback. */
function sortKey(e: Orderable): number {
  let ts = timestamps.get(e)
  if (ts === undefined) {
    ts = e.timestamp ? Date.parse(e.timestamp) : NaN
    timestamps.set(e, ts)
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
  const canonical = entries.filter(e => e.__streaming === undefined)
  const positioned = canonical.length > 0 && canonical.every(e => typeof e._pos === 'number')
  for (let i = 0; i < n; i++) keys[i] = positioned ? entries[i]!._pos ?? MAX : sortKey(entries[i]!)

  // First occurrence wins (uuid dedup happens upstream, but be defensive).
  const idxByUuid = new Map<string, number>()
  for (let i = 0; i < n; i++) {
    const entry = entries[i]!
    for (const u of entry.__uuids ?? (entry.uuid ? [entry.uuid] : [])) {
      if (!idxByUuid.has(u)) idxByUuid.set(u, i)
    }
  }

  // Already ordered keys with parents before children need no heap pass.
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
  // Ready-set = a min-heap of indegree-0 indices ordered by (key, arrival).
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
  // them in (key, arrival) order rather than dropping anything.
  if (out.length < n) {
    const seen = new Set(out)
    const rest: number[] = []
    for (let i = 0; i < n; i++) if (!seen.has(entries[i]!)) rest.push(i)
    rest.sort((a, b) => (less(a, b) ? -1 : 1)) // same (key, arrival) order as the heap
    for (const i of rest) out.push(entries[i]!)
  }
  return out
}
