import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const effects = vi.hoisted(() => [] as Array<() => void>)
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
import { useBridgeListeners } from './useBridgeListeners'
import { activeTabId } from '../signals/tabs'
import { agentEntriesWithLive, subagentTileState, clearAgentTabState, agentSelectionKey, fullscreenAgentId, recordAgentHistory, tabAgentHistory, type AgentInfo } from '../signals/agents'
import { partitionAgents } from '../signals/agent-partition'
import type { KaminBridgeApi } from '../../shared/types'

let listeners: Map<string, (...args: any[]) => void>
beforeEach(() => {
  listeners = new Map()
  subagentTileState.value = new Map()
  tabAgentHistory.value = new Map()
  const bridge = new Proxy({}, { get: (_, name: string) => {
    if (name.startsWith('on')) return (callback: (...args: any[]) => void) => { listeners.set(name, callback); return () => {} }
    return () => Promise.resolve(name === 'listTabs' ? [] : null)
  } }) as KaminBridgeApi
  useBridgeListeners(bridge, { current: new Map() }, { value: false }, 'chat', true)
})
afterEach(() => { for (const cleanup of effects.splice(0)) cleanup(); vi.clearAllTimers() })
function ingest(tab: string, name: string, id: string, uuid: string) {
  activeTabId.value = tab
  listeners.get('onJsonlSubagentEntries')!(tab, name, [{ uuid }], id)
}
function entries(tab: string, id?: string) {
  return (agentEntriesWithLive as any)(tab, 'worker', undefined, id).map((entry: any) => entry.uuid)
}

describe('INC-2026-0007 actual subagent listener and reader', () => {
  it('isolates same-name transcripts through A to B to A and close A', () => {
    ingest('a', 'worker', 'agent-id-a', 'a1')
    ingest('b', 'worker', 'agent-id-b', 'b1')
    expect(entries('b', 'id-b')).toEqual(['b1'])
    ingest('a', 'worker', 'agent-id-a', 'a2')
    expect(entries('a', 'id-a')).toEqual(['a1', 'a2'])
    expect(entries('b', 'id-b')).toEqual(['b1'])
    activeTabId.value = 'b'
    listeners.get('onJsonlSubagentEntries')!('a', 'worker', [{ uuid: 'late-a' }], 'agent-id-a')
    expect(entries('b', 'id-b')).toEqual(['b1'])
    fullscreenAgentId.value = agentSelectionKey('b', { id: 'b', name: 'worker' } as AgentInfo)
    const selection = fullscreenAgentId.value
    clearAgentTabState('a')
    expect(entries('b', 'id-b')).toEqual(['b1'])
    expect(fullscreenAgentId.value).toBe(selection)
  })
  it('requires stable identity when two runs or teams share a display name', () => {
    ingest('a', 'worker', 'agent-first', 'first')
    ingest('a', 'worker', 'agent-second', 'second')
    expect(entries('a', 'first')).toEqual(['first'])
    expect(entries('a', 'second')).toEqual(['second'])
    expect(entries('a')).toEqual([])
    expect(entries('a', 'unknown')).toEqual([])
  })

  it('keeps two same-name teams/runs selectable in completed history', () => {
    const first = { id: 'spawn-first', teamName: 'team-a', name: 'worker', status: 'done', messages: [] } as unknown as AgentInfo
    const second = { ...first, id: 'spawn-second', teamName: 'team-b' }
    recordAgentHistory('a', first)
    recordAgentHistory('a', second)
    const history = tabAgentHistory.value.get('a')!
    expect(history).toHaveLength(2)
    expect(agentSelectionKey('a', first)).not.toBe(agentSelectionKey('a', second))
    expect(agentSelectionKey('a', first)).not.toBe(agentSelectionKey('b', first))
    expect(partitionAgents(undefined, history).completed.count).toBe(2)
  })

  it('allows a unique legacy/type alias for team mailbox IDs but rejects ambiguity', () => {
    ingest('a', 'general-purpose', 'agent-hash-one', 'one')
    expect(agentEntriesWithLive('a', 'worker', 'general-purpose', 'worker@team-a')).toEqual([{ uuid: 'one' }])
    ingest('a', 'general-purpose', 'agent-hash-two', 'two')
    expect(agentEntriesWithLive('a', 'worker', 'general-purpose', 'worker@team-a')).toEqual([])
  })
})
