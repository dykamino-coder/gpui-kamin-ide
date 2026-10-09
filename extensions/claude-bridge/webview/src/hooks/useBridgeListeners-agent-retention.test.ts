import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const effects = vi.hoisted(() => [] as Array<() => void>)
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
import { useBridgeListeners } from './useBridgeListeners'
import { activeTabId } from '../signals/tabs'
import { subagentTileState, clearAgentTabState, tabAgentHistory } from '../signals/agents'
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

const retained = () => [...new Set(subagentTileState.value.values())]
const batch = (start: number, count: number) => Array.from({ length: count }, (_, i) => ({ uuid: String(start + i), timestamp: new Date(1_700_000_000_000 + start + i).toISOString() }))
function send(entries: any[]) {
  activeTabId.value = 'a'
  listeners.get('onJsonlSubagentEntries')!('a', 'worker', entries, 'agent-a')
}
describe('INC-2026-0008 actual retained state', () => {
  it('caps the first large batch and duplicate replay', () => {
    send(batch(0, 5000))
    expect(retained()[0].entries).toHaveLength(600)
    send(batch(0, 5000))
    expect(retained()[0].entries).toHaveLength(600)
    expect(retained()[0].seen?.size).toBe(600)
  })
  it('caps dedup and preserves the newest window after an older replay', () => {
    send(batch(0, 5000))
    send(batch(5000, 5000))
    expect(retained()[0].seen?.size).toBe(600)
    const uuids = retained()[0].entries.map(entry => entry.uuid)
    send(batch(0, 5000))
    expect(retained()[0].entries.map(entry => entry.uuid)).toEqual(uuids)
  })
  it('releases a closed session and caps many distinct agent slots', () => {
    for (let i = 0; i < 200; i++) ingest('a', `worker-${i}`, `agent-${i}`, `u-${i}`)
    expect(retained().length).toBeLessThanOrEqual(64)
    clearAgentTabState('a')
    expect(retained()).toHaveLength(0)
  })
})
