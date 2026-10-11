import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const effects = vi.hoisted(() => [] as Array<() => void>)
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
import { useBridgeListeners } from './useBridgeListeners'
import { activeTabId } from '../signals/tabs'
import { tabJsonlLive, tabAgentTrees, tabAgentHistory, findAgentByName } from '../signals/agents'
import { stagedAgentEntryCount } from '../signals/agent-replay'
import type { KaminBridgeApi } from '../../shared/types'

const boundaries = vi.fn()
let listeners: Map<string, (...args: any[]) => void>
beforeEach(() => {
  boundaries.mockClear()
  listeners = new Map()
  vi.useFakeTimers()
  tabAgentTrees.value = new Map()
  tabJsonlLive.value = new Set()
  tabAgentHistory.value = new Map()
  const bridge = new Proxy({}, { get: (_, name: string) => {
    if (name === 'requestBoundaries') return boundaries
    if (name.startsWith('on')) return (callback: (...args: any[]) => void) => { listeners.set(name, callback); return () => {} }
    return () => Promise.resolve(name === 'listTabs' ? [] : null)
  } }) as KaminBridgeApi
  useBridgeListeners(bridge, { current: new Map() }, { value: false }, 'chat', true)
})
afterEach(() => { for (const cleanup of effects.splice(0)) cleanup(); vi.clearAllTimers(); vi.useRealTimers() })

const agent = (name: string): any => ({ type: 'assistant', uuid: name, message: { role: 'assistant', content: [{ type: 'tool_use', id: `toolu_${name}`, name: 'Agent', input: { name, team_name: 'test', subagent_type: 'general-purpose', prompt: 'test' } }] } })
const status = (complete: boolean) => listeners.get('onJsonlStatus')!('a', { status: 'watching', replayComplete: complete })
describe('INC-2026-0009 actual status listener completion', () => {
  it('rejects G1 completion before changing G2 live/staging state', () => {
    status(false)
    status(true)
    status(false)
    vi.advanceTimersByTime(50)
    expect(tabJsonlLive.value.has('a')).toBe(false)
    expect(boundaries).not.toHaveBeenCalled()
    listeners.get('onJsonlEntries')!('a', [agent('new')])
    status(true)
    vi.advanceTimersByTime(50)
    expect(tabJsonlLive.value.has('a')).toBe(true)
    expect(stagedAgentEntryCount('a')).toBe(0)
    expect(tabAgentTrees.value.has('a')).toBe(true)
    expect(findAgentByName(tabAgentTrees.value.get('a'), 'new')).toBeDefined()
    expect(boundaries).toHaveBeenCalledTimes(1)
    listeners.get('onJsonlEntries')!('a', [agent('live')])
    expect(findAgentByName(tabAgentTrees.value.get('a'), 'live')?.status).toBe('running')
    expect(stagedAgentEntryCount('a')).toBe(0)
  })
  it('publishes the newest generation when both completion timers are queued', () => {
    status(false); status(true); status(false); status(true)
    vi.advanceTimersByTime(50)
    expect(tabJsonlLive.value.has('a')).toBe(true)
    expect(tabAgentTrees.value.has('a')).toBe(true)
    expect(stagedAgentEntryCount('a')).toBe(0)
  })
  it('cannot resurrect a closed tab or mutate after listener cleanup', () => {
    status(false); status(true)
    listeners.get('onTabClosed')!('a')
    vi.advanceTimersByTime(50)
    expect(tabJsonlLive.value.has('a')).toBe(false)
    status(false); status(true)
    for (const cleanup of effects.splice(0)) cleanup()
    vi.advanceTimersByTime(50)
    expect(tabJsonlLive.value.has('a')).toBe(false)
    expect(stagedAgentEntryCount('a')).toBe(0)
  })
})
