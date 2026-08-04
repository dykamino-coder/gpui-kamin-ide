import { describe, it, expect } from 'vitest'
import { buildSegmentIndex, isVisibleEntry } from './segment-index'
import type { JsonlEntry } from '../../shared/jsonl-types'

const ts = (i: number): string => `2026-07-${String(10 + i).padStart(2, '0')}T00:00:00.000Z`
const user = (i: number, text = 'hi'): JsonlEntry =>
  ({ type: 'user', uuid: `u${i}`, timestamp: ts(i), message: { role: 'user', content: text } }) as JsonlEntry
const assistant = (i: number, text = 'ok'): JsonlEntry =>
  ({ type: 'assistant', uuid: `a${i}`, timestamp: ts(i), message: { role: 'assistant', content: [{ type: 'text', text }] } }) as JsonlEntry
const boundary = (i: number): JsonlEntry =>
  ({ type: 'system', uuid: `b${i}`, timestamp: ts(i), subtype: 'compact_boundary' } as unknown as JsonlEntry)
const noise = (type: string, i: number): JsonlEntry =>
  ({ type, uuid: `n${i}`, timestamp: ts(i) } as unknown as JsonlEntry)

describe('isVisibleEntry', () => {
  it('counts real user/assistant turns', () => {
    expect(isVisibleEntry(user(0))).toBe(true)
    expect(isVisibleEntry(assistant(0))).toBe(true)
  })

  it('drops protocol-noise bookkeeping types (the count inflators)', () => {
    for (const t of ['permission-mode', 'last-prompt', 'mode', 'queue-operation', 'file-history-snapshot', 'ai-title', 'summary'])
      expect(isVisibleEntry(noise(t, 0))).toBe(false)
  })

  it('drops compact_boundary dividers (counted structurally, not as content)', () => {
    expect(isVisibleEntry(boundary(3))).toBe(false)
  })

  it('drops a synthetic assistant "No response requested." turn', () => {
    expect(isVisibleEntry(assistant(0, 'No response requested.'))).toBe(false)
  })

  it('drops meta / caveat / tool-result user rows', () => {
    expect(isVisibleEntry({ ...user(0), isMeta: true } as JsonlEntry)).toBe(false)
    expect(isVisibleEntry(user(0, '<command-name>/reload-skills</command-name>'))).toBe(false)
    const toolResult = { type: 'user', uuid: 't', timestamp: ts(0), message: { content: [{ type: 'tool_result', tool_use_id: 'x', content: 'r' }] } } as JsonlEntry
    expect(isVisibleEntry(toolResult)).toBe(false)
  })

  it('counts a tool-only assistant (folds into a card in the chat, still a visible entry)', () => {
    const toolUse = { type: 'assistant', uuid: 'a', timestamp: ts(0), message: { content: [{ type: 'tool_use', name: 'Read', input: {} }] } } as JsonlEntry
    expect(isVisibleEntry(toolUse)).toBe(true)
  })
})

describe('buildSegmentIndex', () => {
  it('splits at boundaries; counts[0]=original, counts[k+1]=after boundaries[k]', () => {
    // seg0: u0,u1 (+ noise ignored) | seg1: a3,a4 | seg2: u6
    const all: JsonlEntry[] = [
      user(0), noise('permission-mode', 0), user(1),
      boundary(2),
      assistant(3), assistant(4),
      boundary(5),
      user(6),
    ]
    const bounds = [3, 6] // indices of the boundary rows
    const idx = buildSegmentIndex(all, bounds)
    expect(idx.boundaries).toEqual([{ ts: ts(2) }, { ts: ts(5) }])
    expect(idx.counts).toEqual([2, 2, 1])
  })

  it('no boundaries → one segment, counts = [visible]', () => {
    const idx = buildSegmentIndex([user(0), noise('mode', 0), assistant(1)], [])
    expect(idx.boundaries).toEqual([])
    expect(idx.counts).toEqual([2])
  })

  it('boundary rows themselves never inflate a count', () => {
    const all: JsonlEntry[] = [user(0), boundary(1), boundary(2), user(3)]
    // two consecutive boundaries: seg0=[u0], then an empty middle segment, then [u3]
    const idx = buildSegmentIndex(all, [1, 2])
    expect(idx.counts).toEqual([1, 0, 1])
  })

  it('a ts-less boundary is NOT emitted; its segment folds into the previous slot', () => {
    const noTs = { ...boundary(1), timestamp: undefined } as JsonlEntry
    const all: JsonlEntry[] = [user(0), noTs, user(2), user(3)]
    const idx = buildSegmentIndex(all, [1])
    expect(idx.boundaries).toEqual([]) // dropped — no place on the ts axis
    expect(idx.counts).toEqual([3]) // u0 + u2 + u3 merged (divider not counted)
  })

  it('keeps boundaries/counts aligned when only SOME boundaries lack ts', () => {
    const noTs = { ...boundary(4), timestamp: undefined } as JsonlEntry
    const all: JsonlEntry[] = [user(0), user(1), boundary(2), user(3), noTs, user(5)]
    const idx = buildSegmentIndex(all, [2, 4])
    expect(idx.boundaries).toEqual([{ ts: ts(2) }]) // only the ts-bearing one
    expect(idx.counts).toEqual([2, 2]) // [u0,u1] | [u3 + folded u5]
    expect(idx.counts.length).toBe(idx.boundaries.length + 1) // the invariant holds
  })
})
