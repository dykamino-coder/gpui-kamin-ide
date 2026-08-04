import { describe, it, expect } from 'vitest'
import { orderEntries } from './order-entries'

interface Row { uuid?: string; parentUuid?: string; _ord?: number; timestamp?: string }
const ids = (rows: Row[]): (string | undefined)[] => rows.map((r) => r.uuid)
const ts = (s: string): string => `2026-07-21T${s}.000Z`

describe('orderEntries', () => {
  it('orders by TIMESTAMP, not arrival (the end-scramble)', () => {
    // T (later ts) is FIRST in the array — as if Q was re-delivered after a
    // reconnect and pushed to the end. Chronology wins over arrival.
    const T: Row = { uuid: 't', timestamp: ts('14:10:00') }
    const Q: Row = { uuid: 'q', timestamp: ts('14:05:00') }
    expect(ids(orderEntries([T, Q]))).toEqual(['q', 't'])
  })

  // ── The bug this key change fixes ──────────────────────────────────────────
  it('an OLD row with a ts but NO _ord sorts by its ts, NOT after newer _ord rows', () => {
    // Scroll-up / archived-segment loads deliver raw rows WITHOUT `_ord`. The old
    // key checked `_ord` first, so this June row fell to the ts scale (~1.7e12)
    // while today's replayed rows returned ~1e5 — so June sorted AFTER July and
    // showed up as the chat's newest message. ts-first puts them on one scale.
    const june: Row = { uuid: 'jun', timestamp: '2026-06-26T22:15:11.000Z' }
    const julA: Row = { uuid: 'a', _ord: 33005, timestamp: ts('09:04:00') }
    const julB: Row = { uuid: 'b', _ord: 33200, timestamp: ts('09:47:00') }
    // June arrives LAST (as a scroll-up would append it) — it must still land first.
    expect(ids(orderEntries([julA, julB, june]))).toEqual(['jun', 'a', 'b'])
  })

  it('real scenario: a dead-branch day among today\'s turns sinks to its date; the tail stays newest', () => {
    const rows: Row[] = [
      { uuid: 'today1', _ord: 33005, timestamp: ts('09:04:00') },
      { uuid: 'today2', _ord: 33100, timestamp: ts('09:20:00') },
      { uuid: 'today3', _ord: 33200, timestamp: ts('09:47:00') },
      // three old rows pulled in without _ord, delivered interleaved:
      { uuid: 'old1', timestamp: '2026-06-26T10:00:00.000Z' },
      { uuid: 'old2', timestamp: '2026-06-26T11:00:00.000Z' },
      { uuid: 'old3', timestamp: '2026-06-27T09:00:00.000Z' },
    ]
    const out = ids(orderEntries([rows[2]!, rows[5]!, rows[0]!, rows[3]!, rows[4]!, rows[1]!]))
    expect(out).toEqual(['old1', 'old2', 'old3', 'today1', 'today2', 'today3'])
  })

  it('rows carrying BOTH _ord and ts sort by ts (stable across an _ord re-base)', () => {
    // Two replays can tag the same logical entry with different _ords (per-process
    // counter). ts is invariant, so ordering must not depend on the _ord scale.
    const a: Row = { uuid: 'a', _ord: 5, timestamp: ts('10:00:00') }
    const b: Row = { uuid: 'b', _ord: 999999, timestamp: ts('10:05:00') } // huge _ord, later ts
    expect(ids(orderEntries([b, a]))).toEqual(['a', 'b'])
  })

  it('a live stub (no ts, no _ord) is genuinely newest → sorts last', () => {
    const replayed: Row = { uuid: 'r', _ord: 500, timestamp: ts('13:00:00') }
    const dated: Row = { uuid: 'd', timestamp: ts('14:00:00') }
    const stub: Row = { uuid: 's' } // streaming stub: no ts yet, no _ord
    expect(ids(orderEntries([stub, dated, replayed]))).toEqual(['r', 'd', 's'])
  })

  it('a child never renders before its parent, even with an earlier timestamp', () => {
    // CLI back-dates a synthetic attachment to ts(parent) - 1ms; the topo edge
    // must still keep it AFTER its parent.
    const parent: Row = { uuid: 'p', timestamp: ts('14:00:00') }
    const child: Row = { uuid: 'c', parentUuid: 'p', timestamp: ts('13:59:59') }
    expect(ids(orderEntries([child, parent]))).toEqual(['p', 'c'])
  })

  it('keeps _ord order among ts-LESS replayed rows (bookkeeping the viewer hides)', () => {
    const a: Row = { uuid: 'a', _ord: 10 }
    const b: Row = { uuid: 'b', _ord: 20 }
    const c: Row = { uuid: 'c', _ord: 15 }
    expect(ids(orderEntries([b, a, c]))).toEqual(['a', 'c', 'b'])
  })

  it('a full live turn threads in order after its prompt', () => {
    const prompt: Row = { uuid: 'u1', _ord: 100, timestamp: ts('14:00:00') }
    const asst: Row = { uuid: 'a1', parentUuid: 'u1', timestamp: ts('14:00:05') }
    const res: Row = { uuid: 'u2', parentUuid: 'a1', timestamp: ts('14:00:06') }
    expect(ids(orderEntries([res, prompt, asst]))).toEqual(['u1', 'a1', 'u2'])
  })

  it('same-ts rows keep a stable order (arrival tie-break)', () => {
    const a: Row = { uuid: 'a', timestamp: ts('14:00:00') }
    const b: Row = { uuid: 'b', timestamp: ts('14:00:00') }
    const c: Row = { uuid: 'c', timestamp: ts('14:00:00') }
    expect(ids(orderEntries([a, b, c]))).toEqual(['a', 'b', 'c'])
  })

  it('already-ordered input hits the fast path unchanged', () => {
    const rows: Row[] = [
      { uuid: 'a', timestamp: ts('10:00:00') },
      { uuid: 'b', timestamp: ts('11:00:00') },
      { uuid: 'c', timestamp: ts('12:00:00') },
    ]
    const out = orderEntries(rows)
    expect(out).toBe(rows) // same reference — bypassed the heap
  })
})
