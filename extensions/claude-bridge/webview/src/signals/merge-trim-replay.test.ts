// The middle-of-chat data-loss gap: a model turn the CLI split into N JSONL rows
// (same message.id, different uuids) is merged into ONE store entry. Its `.uuid`
// is only the LAST row's — but appendJsonlEntries added EVERY row's uuid to the
// dedup `seen` set. If trim then forgets only `.uuid`, the earlier rows' uuids
// stay in `seen` forever, so when the transcript re-replays (reconnect / scroll-
// up) those rows are dropped as duplicates and the turn comes back with its
// earlier content blocks missing — or, if the surviving uuid is also stranded,
// missing entirely. This pins that merge tracks all folded uuids and trim forgets
// them all, so a re-replay restores the full turn.
import { describe, it, expect, beforeEach } from 'vitest'
import {
  appendJsonlEntries, trimTabToWindow, clearJsonlEntries, getJsonlEntries,
} from './jsonl'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 'gap-tab'

// One split block of a model turn: shared msgId, own uuid, one text block.
const block = (uuid: string, msgId: string, ord: number, text: string): JsonlEntryData =>
  ({ uuid, _ord: ord, type: 'assistant', message: { id: msgId, content: [{ type: 'text', text }] } } as unknown as JsonlEntryData)
const userRow = (uuid: string, ord: number, text: string): JsonlEntryData =>
  ({ uuid, _ord: ord, type: 'user', message: { content: [{ type: 'text', text }] } } as unknown as JsonlEntryData)

const texts = (e: JsonlEntryData | undefined): string[] => {
  const c = e?.message?.content
  return Array.isArray(c) ? (c as { text?: string }[]).map((b) => b.text ?? '').filter(Boolean) : []
}

describe('merge → trim → re-replay keeps the whole turn (no middle-of-chat gap)', () => {
  beforeEach(() => clearJsonlEntries(TAB))

  it('a merged 3-block turn survives trim + re-replay with ALL blocks', () => {
    // The split turn (m1: A,B,C) is the OLDEST (low _ord); newer filler follows.
    // Trim keeps the newest, so m1 is the one evicted.
    appendJsonlEntries(TAB, [
      block('a', 'm1', 10, 'block A'),
      block('b', 'm1', 11, 'block B'),
      block('c', 'm1', 12, 'block C'),
    ])
    appendJsonlEntries(TAB, Array.from({ length: 6 }, (_, i) => userRow(`new${String(i)}`, 100 + i, `new ${String(i)}`)))
    const merged = getJsonlEntries(TAB).find((e) => e.message?.id === 'm1')
    expect(texts(merged)).toEqual(['block A', 'block B', 'block C']) // one entry, all blocks

    trimTabToWindow(TAB, 3) // keep the 3 newest filler rows → the m1 turn is evicted
    expect(getJsonlEntries(TAB).some((e) => e.message?.id === 'm1')).toBe(false) // evicted

    // A reconnect re-replays the SAME split rows. Before the fix, a/b stayed in
    // `seen` and were dropped → the turn came back missing blocks (or not at all).
    appendJsonlEntries(TAB, [
      block('a', 'm1', 200, 'block A'),
      block('b', 'm1', 201, 'block B'),
      block('c', 'm1', 202, 'block C'),
    ])
    const restored = getJsonlEntries(TAB).find((e) => e.message?.id === 'm1')
    expect(texts(restored)).toEqual(['block A', 'block B', 'block C']) // fully restored
  })

  it('an un-merged single-block turn still trims + restores by its own uuid', () => {
    // m2 is the oldest; a newer row keeps the window from evicting it too.
    appendJsonlEntries(TAB, [block('solo', 'm2', 5, 'only block'), userRow('u0', 100, 'newer')])
    trimTabToWindow(TAB, 1) // keep the newest (u0) → m2 evicted
    expect(getJsonlEntries(TAB).some((e) => e.message?.id === 'm2')).toBe(false)
    appendJsonlEntries(TAB, [block('solo', 'm2', 200, 'only block')])
    expect(texts(getJsonlEntries(TAB).find((e) => e.message?.id === 'm2'))).toEqual(['only block'])
  })
})
