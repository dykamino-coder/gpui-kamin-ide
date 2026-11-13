// Revision-keyed stale-while-revalidate cache for dashboard read models.
//
// The stats rollup revision moves on every drained sweep, so on a busy server
// a plain revision-keyed cache would recompute the whole page for almost every
// poll. Instead an answer computed for an older revision is served at once
// while ONE background recompute per key runs, unless that recompute finishes
// within `freshWaitMs` (cheap answers stay read-your-writes exact); it is
// replaced when it finishes. Staleness is bounded: an answer older than `maxStaleMs` is not
// served, the caller waits for the fresh one. Concurrent misses share one
// computation.

import { warnLog } from '../../logging'

export interface Revisioned<T> {
  revision: string
  at: number
  data: T
}

export interface RevisionCache<T> {
  get(key: string, revision: string, compute: () => Promise<T>): Promise<Revisioned<T>>
  clear(): void
}

const all: Array<RevisionCache<unknown>> = []

/** Bench/test hook: forget every cached answer. */
export function clearRevisionCaches(): void {
  for (const c of all) c.clear()
}

export function createRevisionCache<T>(opts: {
  max: number
  maxStaleMs: number
  freshWaitMs?: number
}): RevisionCache<T> {
  const freshWaitMs = opts.freshWaitMs ?? 1_000
  const entries = new Map<string, Revisioned<T>>()
  const inflight = new Map<string, Promise<Revisioned<T>>>()

  const refresh = (key: string, revision: string, compute: () => Promise<T>): Promise<Revisioned<T>> => {
    let p = inflight.get(key)
    if (!p) {
      p = compute()
        .then((data) => {
          const e = { revision, at: Date.now(), data }
          entries.delete(key)
          entries.set(key, e)
          while (entries.size > opts.max) entries.delete(entries.keys().next().value!)
          return e
        })
        .finally(() => inflight.delete(key))
      inflight.set(key, p)
    }
    return p
  }

  const cache: RevisionCache<T> = {
    async get(key, revision, compute) {
      const e = entries.get(key)
      if (e && e.revision === revision) return e
      if (e && Date.now() - e.at < opts.maxStaleMs) {
        const fresh = refresh(key, revision, compute)
        fresh.catch((err) => warnLog('[stats] background refresh failed', { key, error: String(err) }))
        let timer: ReturnType<typeof setTimeout> | undefined
        const stale = new Promise<Revisioned<T>>((resolve) => {
          timer = setTimeout(() => resolve(e), freshWaitMs)
        })
        try {
          return await Promise.race([fresh, stale])
        } catch {
          return e
        } finally {
          clearTimeout(timer)
        }
      }
      return refresh(key, revision, compute)
    },
    clear() {
      entries.clear()
    },
  }
  all.push(cache as RevisionCache<unknown>)
  return cache
}
