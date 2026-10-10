import { describe, expect, it } from 'vitest'

import { SCROLL_UP_MAX, STORE_WINDOW } from '../signals/jsonl'
import { buildRendererIncidentSample, countAgentRetention } from './renderer-incident-sample'

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

describe('agent retention counters (INC-2026-0008 debug gate)', () => {
  it('counts unique slots, stored entries, UUID index and closed-tab slots without ids', () => {
    const open = { tabId: 'open-tab', entries: [1, 2, 3], seen: new Set(['a', 'b', 'c']) }
    const closed = { tabId: 'closed-tab', entries: [1], seen: new Set(['a', 'b', 'c', 'd']) }
    const slots = new Map<string, { tabId?: string; entries: unknown[]; seen?: Set<string> }>([
      ['k1', open], ['alias', open], ['k2', closed], ['legacy', { entries: [1, 2] }],
    ])
    const counters = countAgentRetention(slots, new Set(['open-tab']))
    expect(counters).toEqual({ slots: 3, storedEntries: 6, uuidIndex: 7, closedTabSlots: 1 })
    const sample = buildRendererIncidentSample('tools', new Map(), null, undefined, counters)
    expect(sample.agentRetention).toEqual(counters)
    expect(JSON.stringify(sample)).not.toContain('open-tab')
  })

  it('omits the counters when none are supplied', () => {
    expect(buildRendererIncidentSample('chat', new Map(), null, 1)).not.toHaveProperty('agentRetention')
  })
})
