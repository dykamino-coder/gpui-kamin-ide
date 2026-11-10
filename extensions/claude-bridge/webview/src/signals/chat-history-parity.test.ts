// Console-vs-chat parity for the resident chat store.
//
// A production session showed messages in the console while the chat drew
// nothing. Its diagnostic store held only bookkeeping rows nothing renders:
// mostly `cost-state`, plus a few `mode`, `permission-mode` and CLI telemetry
// rows. The uuid set was empty, and the replayed conversation had been rejected
// earlier as `uuid-dup`. Three defects combined:
//
//  1. Rows that nothing reads (cost-state, telemetry) were stored and took
//     window slots.
//  2. Uuid-less rows were deduplicated on `_ord`, which every replay re-tags.
//     So each reattach/reconnect replay stored them AGAIN, and the window cap
//     then evicted the real conversation.
//  3. Live bookkeeping rows arriving after a compaction clear made the store
//     "non-empty". The post-compaction watchdog took that as "the replay
//     landed" and never asked for the history again.
import { describe, it, expect, beforeEach } from 'vitest'
import {
  appendJsonlEntries, clearJsonlEntries, clearRefillDebt, jsonlEntriesByTab,
  jsonlSeenUuidsByTab, replaceWindowWithSegment, setArchivedView, tabOwesHistory,
  trimTabToWindow, STORE_WINDOW, STORE_SLACK,
} from './jsonl'
import { activeTabId } from './tabs'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 'tab-parity'
const store = () => jsonlEntriesByTab.value.get(TAB) ?? []
const count = (type: string) => store().filter((e) => e.type === type).length
const conversation = () => store().filter((e) => e.type === 'user' || e.type === 'assistant')

/** One transcript file: `n` user/assistant turns, then a tail of bookkeeping
 *  rows. `_pos` is the line's byte offset (stable). `_ord` is replay-local and
 *  is re-based on every replay, as the server's global ord counter does. */
function transcript(n: number, tail: string[]): Omit<JsonlEntryData, '_ord'>[] {
  const rows: Record<string, unknown>[] = []
  let pos = 0
  let parent: string | undefined
  for (let i = 0; i < n; i++) {
    const uuid = `u${String(i)}`
    rows.push({ type: i % 2 ? 'assistant' : 'user', uuid, parentUuid: parent, _pos: pos,
      message: { id: i % 2 ? `m${String(i)}` : undefined, role: i % 2 ? 'assistant' : 'user', content: [{ type: 'text', text: uuid }] } })
    parent = uuid
    pos += 1000
  }
  for (const type of tail) { rows.push({ type, _pos: pos, mode: 'default' }); pos += 1000 }
  return rows as unknown as Omit<JsonlEntryData, '_ord'>[]
}

let ordBase = 0
/** Deliver the whole file as one replay with fresh ords: newest batch first,
 *  like the server's initial replay, or oldest first, like a mirror re-send. */
function replay(file: Omit<JsonlEntryData, '_ord'>[], newestFirst = true): void {
  const base = ordBase
  ordBase += file.length
  const tagged = file.map((r, k) => ({ ...structuredClone(r), _ord: base + k }) as JsonlEntryData)
  if (!newestFirst) {
    for (let from = 0; from < tagged.length; from += 200) appendJsonlEntries(TAB, tagged.slice(from, from + 200))
    return
  }
  for (let end = tagged.length; end > 0; end -= 200) {
    appendJsonlEntries(TAB, tagged.slice(Math.max(0, end - 200), end))
  }
}

function storeUuids(): Set<string> {
  const out = new Set<string>()
  for (const e of store()) for (const u of e.__uuids ?? (e.uuid ? [e.uuid] : [])) out.add(u)
  return out
}

beforeEach(() => {
  clearJsonlEntries(TAB)
  clearRefillDebt(TAB)
  jsonlEntriesByTab.value = new Map()
  jsonlSeenUuidsByTab.value = new Map()
  activeTabId.value = 'some-other-tab' // TAB is a background tab
  ordBase = 0
})

describe('chat store keeps console parity', () => {
  it('never stores rows that nothing reads (cost-state, CLI telemetry)', () => {
    replay(transcript(2, ['cost-state', 'atis-latch', 'tools-used', 'api-request', 'api-request-blob']))
    expect(store().map((e) => e.type)).toEqual(['user', 'assistant'])
  })

  it('keeps the read bookkeeping types', () => {
    replay(transcript(0, ['mode', 'permission-mode', 'queue-operation']))
    expect(store().map((e) => e.type)).toEqual(['mode', 'permission-mode', 'queue-operation'])
  })

  it('does not duplicate uuid-less rows when a replay re-tags their _ord', () => {
    const file = transcript(4, ['mode', 'permission-mode', 'queue-operation'])
    for (let i = 0; i < 5; i++) replay(file)
    expect(count('mode')).toBe(1)
    expect(count('permission-mode')).toBe(1)
    expect(count('queue-operation')).toBe(1)
    expect(conversation()).toHaveLength(4)
  })

  it('keeps the conversation resident through a replay storm on a bookkeeping-heavy tail', () => {
    // The evidence shape: a small conversation and a long tail of rows that do
    // not render, replayed over and over (reattach / reconnect).
    const tail = Array.from({ length: 200 }, (_, i) => (i % 3 ? 'cost-state' : 'mode'))
    const file = transcript(40, tail)
    for (let i = 0; i < (STORE_WINDOW + STORE_SLACK) / 50; i++) replay(file, i % 2 === 0)
    expect(conversation()).toHaveLength(40)
    expect(count('cost-state')).toBe(0)
  })

  it('keeps the seen set equal to the uuids the store holds after the cap trims', () => {
    replay(transcript(STORE_WINDOW + STORE_SLACK + 10, []))
    expect(store()).toHaveLength(STORE_WINDOW)
    expect(jsonlSeenUuidsByTab.value.get(TAB)).toEqual(storeUuids())
    trimTabToWindow(TAB, 100)
    expect(jsonlSeenUuidsByTab.value.get(TAB)).toEqual(storeUuids())
  })

  it('re-admits trimmed history when it is delivered again', () => {
    const file = transcript(10, [])
    replay(file)
    trimTabToWindow(TAB, 2)
    expect(conversation()).toHaveLength(2)
    replay(file)
    expect(conversation()).toHaveLength(10)
  })

  it('restores the live window and its seen set after an archived segment view', () => {
    activeTabId.value = TAB
    const file = transcript(6, ['mode'])
    replay(file)
    setArchivedView(TAB, '2026-01-01T00:00:00Z')
    replaceWindowWithSegment(TAB, structuredClone(file.slice(0, 2)) as JsonlEntryData[])
    replay(file) // live replay while parked: buffered, not shown
    setArchivedView(TAB, null)
    expect(conversation()).toHaveLength(6)
    expect(jsonlSeenUuidsByTab.value.get(TAB)).toEqual(storeUuids())
    replay(file) // the "back to Current" replay must not duplicate anything
    expect(conversation()).toHaveLength(6)
    expect(count('mode')).toBe(1)
  })

  it('still owes the history after a clear when only bookkeeping rows arrived', () => {
    replay(transcript(4, []))
    clearJsonlEntries(TAB) // compaction clear / memory eviction
    replay(transcript(0, ['mode', 'cost-state'])) // live tail after the switch
    expect(tabOwesHistory(TAB)).toBe(true)
    replay(transcript(4, [])) // the refill lands
    expect(tabOwesHistory(TAB)).toBe(false)
  })

  it('does not owe history for a tab that was never cleared', () => {
    replay(transcript(0, ['mode']))
    expect(tabOwesHistory(TAB)).toBe(false)
  })
})
