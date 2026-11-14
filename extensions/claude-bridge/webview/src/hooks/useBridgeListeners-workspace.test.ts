import { afterEach, expect, it, vi } from 'vitest'
import type { KaminBridgeApi } from '../../shared/types'
import { createBridgeTransport } from '../../../browser-client/src/transport'
import { EVENT } from '../lib/bridge-channels'

const state = vi.hoisted(() => ({
  effects: [] as Array<() => void>,
  receivers: new Set<(frame: unknown) => void>(),
  postMessage: vi.fn(),
  privatePort: true,
}))
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => state.effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
vi.mock('../lib/webview-api', () => ({ vscodeApi: {
  postMessage: state.postMessage,
  get onHostMessage() {
    return state.privatePort ? (receiver: (frame: unknown) => void) => {
      state.receivers.add(receiver)
      return () => state.receivers.delete(receiver)
    } : undefined
  },
} }))
import { useBridgeListeners } from './useBridgeListeners'
import { activeWidgets } from '../signals/widgets'
import { activeTabId } from '../signals/tabs'
import { jsonlEntriesByTab } from '../signals/jsonl'

afterEach(() => {
  for (const cleanup of state.effects.splice(0)) cleanup()
  activeWidgets.value = []
  activeTabId.value = null
  jsonlEntriesByTab.value = new Map()
  state.receivers.clear()
  state.privatePort = true
  vi.clearAllMocks()
})

it.each(['browser', 'desktop'] as const)('INC-2026-0077 removes only the owning permission via the %s transport', surface => {
  state.privatePort = surface === 'browser'
  const hostWindow = new EventTarget()
  const port = {
    postMessage: state.postMessage,
    getState: () => null,
    setState: () => {},
    onHostMessage: state.privatePort ? (receiver: (frame: unknown) => void) => {
      state.receivers.add(receiver)
      return () => { state.receivers.delete(receiver) }
    } : undefined,
  }
  const transport = createBridgeTransport(port, hostWindow as unknown as Window)
  const bridge = new Proxy({}, { get: (_, name: string) => {
    if (name.startsWith('on')) return (callback: (...args: unknown[]) => void) => transport.sub(EVENT[name], callback)
    return () => Promise.resolve(name === 'listTabs' ? [] : null)
  } }) as KaminBridgeApi
  useBridgeListeners(bridge, { current: new Map() }, { value: false })
  activeWidgets.value = [
    { requestId: 'pending', resolved: false, type: 'permission', data: { tabId: 'a' } },
    { requestId: 'other', resolved: false, type: 'permission', data: { tabId: 'a' } },
    { requestId: 'pending', resolved: false, type: 'permission', data: { tabId: 'b' } },
    { requestId: 'pending', resolved: false, type: 'elicitation', data: { tabId: 'a' } },
  ]
  const resolved = { kind: 'event', channel: 'permission-resolved', args: ['a', { requestId: 'pending' }] }
  const receive = () => {
    if (state.privatePort) for (const receiver of state.receivers) receiver(resolved)
    else hostWindow.dispatchEvent(new MessageEvent('message', { data: resolved }))
  }
  receive()
  receive()
  expect(activeWidgets.value.map(widget => [widget.requestId, widget.type, widget.data.tabId])).toEqual([
    ['other', 'permission', 'a'], ['pending', 'permission', 'b'], ['pending', 'elicitation', 'a'],
  ])
  for (const cleanup of state.effects.splice(0)) cleanup()
  activeWidgets.value = [{ requestId: 'pending', resolved: false, type: 'permission', data: { tabId: 'a' } }]
  receive()
  expect(activeWidgets.value).toHaveLength(1)
})

it('installs the host snapshot receiver before sync, replaces stale projection and detaches subscriptions', () => {
  const bridge = new Proxy({}, { get: (_, name: string) => {
    if (name.startsWith('on')) return () => () => {}
    return () => Promise.resolve(name === 'listTabs' ? [] : null)
  } }) as KaminBridgeApi
  state.postMessage.mockImplementation(frame => {
    if (frame.channel === 'workspace:sync') {
      expect(state.receivers.size).toBe(3)
      for (const receiver of state.receivers) receiver({ kind: 'event', channel: 'workspace:snapshot-start', args: [[], 'host-tab'] })
    }
  })
  activeWidgets.value = [{ requestId: 'stale' }] as typeof activeWidgets.value
  jsonlEntriesByTab.value = new Map([['host-tab', [{ type: 'user', uuid: 'stale-row' }]]])
  useBridgeListeners(bridge, { current: new Map() }, { value: false }, 'chat', true)
  expect(state.postMessage).toHaveBeenCalledWith({ kind: 'send', channel: 'workspace:sync', args: [] })
  expect(activeTabId.value).toBe('host-tab')
  expect(activeWidgets.value).toEqual([])
  expect(jsonlEntriesByTab.value.get('host-tab') ?? []).toEqual([])
  activeWidgets.value = [{ requestId: 'answered' }, { requestId: 'still-pending' }] as typeof activeWidgets.value
  for (const receiver of state.receivers) receiver({ kind: 'event', channel: 'workspace:question-resolved', args: ['answered'] })
  expect(activeWidgets.value.map(widget => widget.requestId)).toEqual(['still-pending'])
  for (const cleanup of state.effects.splice(0)) cleanup()
  expect(state.receivers.size).toBe(0)
})
