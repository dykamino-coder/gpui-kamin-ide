// The store's memory cap: trim to the last N entries when the reader is at the
// bottom. The load-bearing correctness property is the round trip — an evicted
// record must be able to come BACK via the scroll-up prepend, which means its
// uuid has to leave the seen-set on eviction, or the prepend would dedup away
// the very history it refetched.
import { describe, it, expect, beforeEach } from 'vitest'
import { jsonlEntriesByTab, jsonlSeenUuidsByTab, trimTabToWindow, prependJsonlEntries } from './jsonl'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 'tab-1'
const rec = (uuid: string, ord: number, pos: number): JsonlEntryData =>
  ({ type: 'user', uuid, _ord: ord, _pos: pos, message: { content: [] } } as unknown as JsonlEntryData)

const store = () => jsonlEntriesByTab.value.get(TAB) ?? []
const uuids = () => store().map((e) => e.uuid)

function seed(n: number): void {
  const entries: JsonlEntryData[] = []
  const seen = new Set<string>()
  for (let i = 0; i < n; i++) { entries.push(rec(`e${i}`, i, i * 100)); seen.add(`e${i}`) }
  jsonlEntriesByTab.value = new Map([[TAB, entries]])
  jsonlSeenUuidsByTab.value = new Map([[TAB, seen]])
}

describe('trimTabToWindow', () => {
  beforeEach(() => seed(10))

  it('keeps the last N entries, drops the oldest', () => {
    expect(trimTabToWindow(TAB, 4)).toBe(true)
    expect(uuids()).toEqual(['e6', 'e7', 'e8', 'e9'])
  })

  it('is a no-op when already within the window', () => {
    expect(trimTabToWindow(TAB, 20)).toBe(false)
    expect(store()).toHaveLength(10)
  })

  it('removes evicted uuids from the seen-set so scroll-up can refetch them', () => {
    trimTabToWindow(TAB, 4)
    const seen = jsonlSeenUuidsByTab.value.get(TAB)!
    expect(seen.has('e0')).toBe(false) // evicted → refetchable
    expect(seen.has('e9')).toBe(true) // kept → still deduped
  })

  it('ROUND TRIP: an evicted record comes back via the scroll-up prepend', () => {
    trimTabToWindow(TAB, 4) // window = e6..e9
    // Scroll-up refetches the page before e6: e2..e5 (from the disk mirror).
    prependJsonlEntries(TAB, [rec('e2', 2, 200), rec('e3', 3, 300), rec('e4', 4, 400), rec('e5', 5, 500)])
    expect(uuids()).toEqual(['e2', 'e3', 'e4', 'e5', 'e6', 'e7', 'e8', 'e9'])
  })

  it('WITHOUT the seen-removal the round trip would fail — guards the regression', () => {
    // Prove the property the fix depends on: if we DON'T evict from seen, the
    // prepend dedups the refetched page away. (We test the real path keeps seen
    // correct; this asserts the mechanism by re-adding to seen manually.)
    trimTabToWindow(TAB, 4)
    const seen = jsonlSeenUuidsByTab.value.get(TAB)!
    seen.add('e5') // simulate the bug: evicted uuid still marked seen
    prependJsonlEntries(TAB, [rec('e4', 4, 400), rec('e5', 5, 500)])
    expect(uuids()).toEqual(['e4', 'e6', 'e7', 'e8', 'e9']) // e5 wrongly dropped
  })
})
