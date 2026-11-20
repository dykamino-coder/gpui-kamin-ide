import { afterEach, expect, it, vi } from 'vitest'
const effects = vi.hoisted(() => [] as Array<() => void>)
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
import { useBridgeListeners } from './useBridgeListeners'
import { tabs } from '../signals/tabs'
import { usageByTab, usageForEngine } from '../signals/session-usage'
import type { KaminBridgeApi, SessionUsage, TabInfo } from '../../shared/types'

afterEach(() => { for (const cleanup of effects.splice(0)) cleanup() })

const claudeUsage: SessionUsage = {
  windows: [{ id: 'seven_day', label: 'Week (all)', percent: 34, resetsAt: '2099-01-01T00:00:00Z' }],
  contextPercent: 12, contextWindowSize: 1_000_000, autoCompactAt: 900_000,
  model: 'claude-opus-5-5', effort: null, observedAt: '2026-11-19T00:00:00Z',
}

it('a chat switched to ChatGPT drops the Claude usage and ignores a late Claude snapshot', () => {
  const listeners = new Map<string, (...args: any[]) => void>()
  const bridge = new Proxy({}, { get: (_, name: string) => {
    if (name.startsWith('on')) return (callback: (...args: any[]) => void) => { listeners.set(name, callback); return () => {} }
    return () => Promise.resolve(name === 'listTabs' ? [] : null)
  } }) as KaminBridgeApi
  useBridgeListeners(bridge, { current: new Map() }, { value: false }, 'chat', true)
  tabs.value = [{ id: 't1', engine: 'claude', model: 'claude-opus-5-5' } as unknown as TabInfo]
  usageByTab.value = new Map()
  const shown = () => usageForEngine(usageByTab.value.get('t1'), tabs.value.find(t => t.id === 't1')?.engine)

  listeners.get('onSessionUsage')!('t1', claudeUsage)
  expect(shown()?.windows[0]?.percent).toBe(34)

  listeners.get('onSessionRestarted')!('t1', { engine: 'codex', model: 'gpt-5.5', inPlace: false })
  expect(tabs.value[0]!.engine).toBe('codex')
  expect(usageByTab.value.has('t1')).toBe(false)

  // A status-line frame of the old Claude CLI arrives after the switch.
  listeners.get('onSessionUsage')!('t1', claudeUsage)
  expect(shown()).toBeUndefined()

  // Back on Claude, the Claude snapshot is this chat's again.
  listeners.get('onSessionRestarted')!('t1', { engine: 'claude', model: 'claude-opus-5-5' })
  expect(shown()?.windows[0]?.percent).toBe(34)
})
