// Bookkeeping rows that nothing reads are dropped at the store's door.
//
// The dangerous half of this change is what must NOT be dropped: three of the
// types that render nothing are still READ — `mode` and `permission-mode` drive
// the permissions dropdown, `queue-operation` rebuilds the pending queue. Losing
// them would break both with no error anywhere.
import { describe, it, expect, beforeEach } from 'vitest'
import { appendJsonlEntries, jsonlEntriesByTab, clearJsonlEntries } from './jsonl'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 'tab-1'
// `_ord` must ascend: the store sorts on it, so random values would make the
// expected order meaningless.
let ord = 0
const entry = (type: string, uuid: string): JsonlEntryData =>
  ({ type, uuid, _ord: ++ord } as unknown as JsonlEntryData)

const stored = () => jsonlEntriesByTab.value.get(TAB) ?? []
const types = () => stored().map((e) => e.type)

describe('unread entry filtering', () => {
  beforeEach(() => {
    jsonlEntriesByTab.value = new Map()
    clearJsonlEntries(TAB)
    jsonlEntriesByTab.value = new Map()
  })

  it('keeps the rows that ARE read despite rendering nothing', () => {
    appendJsonlEntries(TAB, [
      entry('mode', 'a'),
      entry('permission-mode', 'b'),
      entry('queue-operation', 'c'),
    ])
    expect(types().sort()).toEqual(['mode', 'permission-mode', 'queue-operation'])
  })

  it('drops the rows nothing reads', () => {
    appendJsonlEntries(TAB, [
      entry('ai-title', 'a'),
      entry('last-prompt', 'b'),
      entry('summary', 'c'),
      entry('todo-snapshot', 'd'),
    ])
    expect(stored()).toHaveLength(0)
  })

  it('keeps the conversation itself untouched', () => {
    appendJsonlEntries(TAB, [
      entry('user', 'u1'),
      entry('ai-title', 'x'),
      entry('assistant', 'a1'),
      entry('system', 's1'),
    ])
    expect(types()).toEqual(['user', 'assistant', 'system'])
  })

  it('reports nothing changed when a batch is entirely unread rows', () => {
    expect(appendJsonlEntries(TAB, [entry('ai-title', 'a'), entry('last-prompt', 'b')])).toBe(false)
    expect(stored()).toHaveLength(0)
  })

  it('still handles an empty batch', () => {
    expect(appendJsonlEntries(TAB, [])).toBe(false)
  })
})
