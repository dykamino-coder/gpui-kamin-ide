// Waiting primitives for the command API.
//
// The trap this exists to close: `waitFor(s => !s.working)` resolves the instant
// a turn ends, which for a compaction is BEFORE the store is cleared and
// re-replayed. Anything read there is a half-rebuilt view. `waitForIdle` adds
// quiescence — the state must not only be actionable, it must hold still — so a
// script observes the settled result instead of the gap.
import { snapshot } from './command-state'
import { quiescenceKey, type BridgeState } from './command-model'

const POLL_MS = 100

/** Predicate over the state snapshot.
 *
 *  Functions only — source-text predicates were compiled with `new Function`,
 *  which put an eval gadget on `window.bridgeCmd` in every shipped build. A
 *  driver evaluating JS in the page (CDP, devtools) can pass a real closure, so
 *  the string form bought nothing and only widened what an injection could
 *  reach. */
export type StatePredicate = (s: BridgeState) => boolean

function toPredicate(p: StatePredicate): (s: BridgeState) => boolean {
  return p
}

function fail(reason: string, s: BridgeState): Error {
  return new Error(`${reason}; state=${JSON.stringify(s)}`)
}

/** Poll until `check` holds. Polling rather than signal subscription: a driver
 *  outside the frame cannot hold a subscription, and these conditions are about
 *  state that settles rather than a single event. */
export function waitFor(check: StatePredicate, timeoutMs = 30_000): Promise<BridgeState> {
  const pred = toPredicate(check)
  return new Promise<BridgeState>((resolve, reject) => {
    const started = Date.now()
    const tick = (): void => {
      let s: BridgeState
      try { s = snapshot() } catch (err) { reject(err as Error); return }
      let ok = false
      try { ok = pred(s) } catch (err) { reject(err as Error); return }
      if (ok) { resolve(s); return }
      if (Date.now() - started > timeoutMs) { reject(fail(`waitFor timed out after ${timeoutMs}ms`, s)); return }
      setTimeout(tick, POLL_MS)
    }
    tick()
  })
}

export interface IdleOptions {
  /** How long the state must hold still before it counts as settled. */
  stableMs?: number
  timeoutMs?: number
}

/** Resolve once the view is actionable AND has stopped changing.
 *
 *  This is what a script should await after `send()` — especially for
 *  `/compact`, whose post-turn rebuild is exactly the window that produces
 *  bogus reads. */
export function waitForIdle(opts: IdleOptions = {}): Promise<BridgeState> {
  const { stableMs = 1500, timeoutMs = 120_000 } = opts
  return new Promise<BridgeState>((resolve, reject) => {
    const started = Date.now()
    let key = ''
    let keySince = 0
    const tick = (): void => {
      let s: BridgeState
      try { s = snapshot() } catch (err) { reject(err as Error); return }
      const k = quiescenceKey(s)
      if (k !== key) { key = k; keySince = Date.now() }
      if (!s.busy && Date.now() - keySince >= stableMs) { resolve(s); return }
      if (Date.now() - started > timeoutMs) { reject(fail(`waitForIdle timed out after ${timeoutMs}ms`, s)); return }
      setTimeout(tick, POLL_MS)
    }
    tick()
  })
}

/** Resolve when `field` differs from its value at call time — for asserting that
 *  something actually moved, rather than that a condition happened to hold. */
export function waitForChange<K extends keyof BridgeState>(
  field: K, timeoutMs = 30_000,
): Promise<BridgeState> {
  const before = snapshot()[field]
  return waitFor((s) => s[field] !== before, timeoutMs)
}
