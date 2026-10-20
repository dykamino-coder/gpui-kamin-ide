import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const effects = vi.hoisted(() => [] as Array<() => void>)
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
import { useBridgeListeners } from './useBridgeListeners'
import { activeTabId } from '../signals/tabs'
import { tabJsonlLive, tabAgentTrees, tabAgentHistory, findAgentByName } from '../signals/agents'
import { stagedAgentEntryCount, isAgentReplayStaging } from '../signals/agent-replay'
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


describe('INC-0067 metadata must not start another agent replay', () => {
  const entries = (tab: string, rows: any[]): void => listeners.get('onJsonlEntries')!(tab, rows)
  const meta = (data: any): void => listeners.get('onJsonlStatus')!('a', { status: 'watching', ...data })
  const idle = (name: string): any => ({ type: 'user', uuid: `idle-${name}`, message: { role: 'user', content: `<teammate-message teammate_id="${name}">${JSON.stringify({ type: 'idle_notification', idleReason: 'available' })}</teammate-message>` } })
  const hydrate = (): void => {
    status(false)
    entries('a', ['one', 'two', 'three', 'four', 'five'].map(agent))
    status(true)
    vi.advanceTimersByTime(50)
  }
  it('applies four idle events live after a compact-index status', () => {
    hydrate()
    meta({}) // the host sends boundaries separately; this status has no completion flag
    entries('a', ['one', 'two', 'three', 'four'].map(idle))
    expect(tabJsonlLive.value.has('a')).toBe(true)
    expect(isAgentReplayStaging('a')).toBe(false)
    const workers = tabAgentTrees.value.get('a')!.teams.get('test')!.agents
    expect([...workers.values()].filter(a => a.status === 'running').map(a => a.name)).toEqual(['five'])
  })
  it('keeps staged rows and the completion timer through progress-only updates', () => {
    status(false)
    entries('a', [agent('first')])
    meta({ replayProgress: { sent: 1, total: 2 } })
    entries('a', [agent('second')])
    status(true)
    meta({ replayProgress: { sent: 2, total: 2 } })
    vi.advanceTimersByTime(50)
    expect(tabJsonlLive.value.has('a')).toBe(true)
    expect(findAgentByName(tabAgentTrees.value.get('a'), 'first')).toBeDefined()
    expect(findAgentByName(tabAgentTrees.value.get('a'), 'second')).toBeDefined()
    expect(stagedAgentEntryCount('a')).toBe(0)
  })
  it('preserves another parent while the same-name child completes in this tab', () => {
    hydrate()
    listeners.get('onJsonlStatus')!('b', { status: 'watching', replayComplete: false })
    entries('b', [agent('one')])
    listeners.get('onJsonlStatus')!('b', { status: 'watching', replayComplete: true })
    vi.advanceTimersByTime(50)
    meta({})
    entries('a', [idle('one')])
    expect(findAgentByName(tabAgentTrees.value.get('a'), 'one')?.status).toBe('done')
    expect(findAgentByName(tabAgentTrees.value.get('b'), 'one')?.status).toBe('running')
    expect(tabJsonlLive.value.has('b')).toBe(true)
  })
})
