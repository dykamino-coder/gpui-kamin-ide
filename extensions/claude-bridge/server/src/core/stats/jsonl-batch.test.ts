import { describe, it, expect } from 'vitest'
import { dedupeByUuid } from './jsonl-batch'

describe('dedupeByUuid', () => {
  it('keeps the first row for a repeated uuid and preserves order', () => {
    const rows = [
      ['a', 's1', 'user'],
      ['b', 's1', 'assistant'],
      ['a', 's1', 'user-again'],
      ['c', 's1', 'assistant'],
      ['b', 's1', 'assistant-again'],
    ]
    expect(dedupeByUuid(rows)).toEqual([
      ['a', 's1', 'user'],
      ['b', 's1', 'assistant'],
      ['c', 's1', 'assistant'],
    ])
  })

  it('treats derived tool_use ids as their own keys', () => {
    const rows = [
      ['m#tool0', 's1', 'tool_use'],
      ['m#tool1', 's1', 'tool_use'],
      ['m', 's1', 'assistant'],
      ['m#tool0', 's1', 'tool_use'],
    ]
    expect(dedupeByUuid(rows).map(r => r[0])).toEqual(['m#tool0', 'm#tool1', 'm'])
  })

  it('returns an empty batch unchanged', () => {
    expect(dedupeByUuid([])).toEqual([])
  })
})
