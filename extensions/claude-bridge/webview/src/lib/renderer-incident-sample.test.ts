import { describe, expect, it } from 'vitest'

import { SCROLL_UP_MAX, STORE_WINDOW } from '../signals/jsonl'
import { buildRendererIncidentSample } from './renderer-incident-sample'

describe('renderer incident samples', () => {
  it('contains store counters without tab ids or transcript contents', () => {
    const secretTab = 'secret-tab-id'
    const secretContent = { text: 'private transcript' }
    const store = new Map<string, unknown[]>([
      [secretTab, [secretContent, secretContent]],
      ['background', [secretContent]],
    ])
    const sample = buildRendererIncidentSample('chat', store, secretTab, 123)
    const serialized = JSON.stringify(sample)

    expect(sample).toEqual({
      role: 'chat', heapMB: 123, retainedTabs: 2, retainedEntries: 3,
      activeEntries: 2, storeWindow: STORE_WINDOW, scrollUpMax: SCROLL_UP_MAX,
      windowState: 'within-configured-window',
    })
    expect(serialized).not.toContain(secretTab)
    expect(serialized).not.toContain('private transcript')
  })

  it('reports configured-window pressure without inventing a crash threshold', () => {
    const store = new Map([['active', Array.from({ length: SCROLL_UP_MAX + 1 })]])
    const sample = buildRendererIncidentSample('chat', store, 'active')

    expect(sample.windowState).toBe('over-configured-window')
    expect(sample.activeEntries).toBe(SCROLL_UP_MAX + 1)
  })
})
