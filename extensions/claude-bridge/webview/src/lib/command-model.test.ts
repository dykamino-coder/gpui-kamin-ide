import { describe, it, expect } from 'vitest'
import { previewEntry, quiescenceKey, type BridgeState } from './command-model'

const base: BridgeState = {
  activeTabId: 't1', boundTabId: 't1', mcpLoading: false, promptReady: true,
  working: false, replayPct: null, entryCount: 10, segmentCount: 2,
  activeSegment: 1, archived: false, settling: false, busy: false, queued: 0,
}

describe('previewEntry', () => {
  it('names a tool call instead of previewing it as blank', () => {
    const e = { type: 'assistant', message: { content: [{ type: 'tool_use', name: 'Bash', input: {} }] } }
    expect(previewEntry(e)).toEqual({ type: 'assistant', tool: 'Bash', text: '[tool:Bash]' })
  })

  it('keeps the subtype so bookkeeping rows are recognisable as such', () => {
    expect(previewEntry({ type: 'system', subtype: 'compact_boundary' }).type).toBe('system:compact_boundary')
  })

  it('flattens string and text-block content', () => {
    expect(previewEntry({ type: 'user', message: { content: 'hello' } }).text).toBe('hello')
    expect(previewEntry({ type: 'assistant', message: { content: [{ type: 'text', text: 'a  b' }] } }).text).toBe('a b')
  })

  it('marks tool results', () => {
    const e = { type: 'user', message: { content: [{ type: 'tool_result', content: 'x' }] } }
    expect(previewEntry(e).text).toBe('[tool_result]')
  })
})

describe('quiescenceKey', () => {
  it('changes when the view rebuilds', () => {
    const before = quiescenceKey(base)
    expect(quiescenceKey({ ...base, entryCount: 11 })).not.toBe(before)
    expect(quiescenceKey({ ...base, segmentCount: 3 })).not.toBe(before)
    expect(quiescenceKey({ ...base, boundTabId: 't2' })).not.toBe(before)
  })

  it('ignores the replay percent, which ticks by design', () => {
    expect(quiescenceKey({ ...base, replayPct: 42 })).toBe(quiescenceKey(base))
  })
})
