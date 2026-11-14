import { afterEach, expect, it, vi } from 'vitest'
const effects = vi.hoisted(() => [] as Array<() => void>)
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
import { useBridgeListeners } from './useBridgeListeners'
import { activeTabId, applyTabSwitch } from '../signals/tabs'
import { tabPromptReady } from '../signals/connection'
import { tabAgentTrees, tabAgentHistory } from '../signals/agents'
import { partitionAgents } from '../signals/agent-partition'
import type { KaminBridgeApi } from '../../shared/types'

afterEach(() => { for (const cleanup of effects.splice(0)) cleanup(); vi.clearAllTimers(); vi.useRealTimers() })
it('BR-04 rebinds the retained Agents panel and publishes completed teams from the replacement child', () => {
  vi.useFakeTimers()
  const listeners = new Map<string, (...args: any[]) => void>()
  const bridge = new Proxy({}, { get: (_, name: string) => {
    if (name.startsWith('on')) return (callback: (...args: any[]) => void) => { listeners.set(name, callback); return () => {} }
    return () => Promise.resolve(name === 'listTabs' ? [] : null)
  } }) as KaminBridgeApi
  useBridgeListeners(bridge, { current: new Map() }, { value: false }, 'tools', true)
  const selected = 'stable-selected'
  const rows = ['one', 'two'].flatMap((name, index) => [
    { type: 'assistant', uuid: name, _ord: index * 2, message: { role: 'assistant', content: [{ type: 'tool_use', id: `call-${name}`, name: 'Agent', input: { name, team_name: 'respawn-team' } }] } },
    { type: 'user', uuid: `idle-${name}`, _ord: index * 2 + 1, message: { role: 'user', content: `<teammate-message teammate_id="${name}">{"type":"idle_notification","idleReason":"available"}</teammate-message>` } },
  ])
  const panel = () => partitionAgents(tabAgentTrees.value.get(activeTabId.value!), tabAgentHistory.value.get(activeTabId.value!) ?? [])
  const replay = () => {
    listeners.get('onJsonlStatus')!(selected, { status: 'watching', replayComplete: false })
    listeners.get('onJsonlEntries')!(selected, rows)
    listeners.get('onJsonlStatus')!(selected, { status: 'watching', replayComplete: true })
    vi.advanceTimersByTime(50)
  }
  applyTabSwitch(selected, 40, 1)
  replay()
  expect(panel().completed.count).toBe(2)
  // Model state loss in a detached/re-attached panel; surviving frames also
  // retain a high switch sequence from the dead child.
  tabAgentTrees.value = new Map()
  tabAgentHistory.value = new Map()
  applyTabSwitch('old-selection', 41, 1)
  listeners.get('onTabSwitched')!(selected, 1, 2)
  expect(activeTabId.value).toBe(selected)
  tabPromptReady.value = new Map([[selected, false]])
  // The new connection may authenticate before the retained view reattaches.
  listeners.get('onTabListChanged')!([{
    id: selected, label: '#1', cwd: '/project', folderName: 'project', createdAt: '2026-11-13',
    status: 'connected', connectionAuthority: 'replacement', connectionAuthorityGeneration: 2,
    connectionAuthoritySequence: 1, connectionRevision: 3,
  }])
  expect(tabPromptReady.value.get(selected)).toBe(true)
  replay()
  expect(panel().completed.count).toBe(2)
  expect(panel().active.count).toBe(0)
})
