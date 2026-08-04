// Scroll-up prepend for the windowed store: an older page from the mirror is
// added to the FRONT, re-tagged with `_ord` below the window so a later sort
// can't interleave it, deduped on uuid so an overlapping page can't double a row.
import { describe, it, expect, beforeEach } from 'vitest'
import { jsonlEntriesByTab, jsonlSeenUuidsByTab, prependJsonlEntries, appendJsonlEntries } from './jsonl'
import { orderEntries } from './order-entries'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 'tab-1'
const rec = (uuid: string, ord: number, pos: number): JsonlEntryData =>
  ({ type: 'user', uuid, _ord: ord, _pos: pos, message: { content: [] } } as unknown as JsonlEntryData)

const store = () => jsonlEntriesByTab.value.get(TAB) ?? []
const uuids = () => store().map((e) => e.uuid)

describe('prependJsonlEntries', () => {
  beforeEach(() => {
    jsonlEntriesByTab.value = new Map([[TAB, [rec('w0', 100, 5000), rec('w1', 101, 5100), rec('w2', 102, 5200)]]])
    jsonlSeenUuidsByTab.value = new Map([[TAB, new Set(['w0', 'w1', 'w2'])]])
  })

  it('puts the older page in front, in order', () => {
    prependJsonlEntries(TAB, [rec('o0', 0, 3000), rec('o1', 0, 3100)])
    expect(uuids()).toEqual(['o0', 'o1', 'w0', 'w1', 'w2'])
  })

  it('re-tags the page _ord strictly below the window minimum, ascending', () => {
    prependJsonlEntries(TAB, [rec('o0', 999, 3000), rec('o1', 999, 3100)])
    const ords = store().map((e) => (e as { _ord?: number })._ord!)
    expect(ords[0]!).toBeLessThan(ords[2]!) // o0 < w0
    expect(ords[0]!).toBeLessThan(ords[1]!) // ascending within the page
    expect(ords[1]!).toBeLessThan(ords[2]!) // page < window
  })

  it('survives a later orderEntries without interleaving the page into the window', () => {
    prependJsonlEntries(TAB, [rec('o0', 999, 3000), rec('o1', 999, 3100)])
    const sorted = orderEntries(store())
    expect(sorted.map((e) => e.uuid)).toEqual(['o0', 'o1', 'w0', 'w1', 'w2'])
  })

  it('dedups a page that overlaps the window', () => {
    prependJsonlEntries(TAB, [rec('o0', 0, 3000), rec('w0', 0, 5000)]) // w0 already held
    expect(uuids()).toEqual(['o0', 'w0', 'w1', 'w2'])
  })

  it('is a no-op for an empty page or an all-overlap page', () => {
    expect(prependJsonlEntries(TAB, [])).toBe(false)
    expect(prependJsonlEntries(TAB, [rec('w1', 0, 5100)])).toBe(false)
    expect(uuids()).toEqual(['w0', 'w1', 'w2'])
  })

  it('dedups duplicate uuids WITHIN the page, not just against the store', () => {
    prependJsonlEntries(TAB, [rec('o0', 0, 3000), rec('o0', 0, 3000), rec('o1', 0, 3100)])
    expect(uuids()).toEqual(['o0', 'o1', 'w0', 'w1', 'w2']) // o0 once, not twice
  })

  it('a subsequent live append still lands at the end, above the whole window', () => {
    prependJsonlEntries(TAB, [rec('o0', 0, 3000)])
    appendJsonlEntries(TAB, [rec('w3', 103, 5300)])
    expect(uuids()).toEqual(['o0', 'w0', 'w1', 'w2', 'w3'])
  })
})
