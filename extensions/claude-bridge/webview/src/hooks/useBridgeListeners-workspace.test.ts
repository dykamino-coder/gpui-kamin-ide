import { afterEach, expect, it, vi } from 'vitest'
import type { KaminBridgeApi } from '../../shared/types'

const state = vi.hoisted(() => ({
  effects: [] as Array<() => void>,
  receivers: new Set<(frame: unknown) => void>(),
  postMessage: vi.fn(),
}))
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => state.effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
vi.mock('../lib/webview-api', () => ({ vscodeApi: {
  postMessage: state.postMessage,
  onHostMessage: (receiver: (frame: unknown) => void) => {
    state.receivers.add(receiver)
    return () => state.receivers.delete(receiver)
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
  vi.clearAllMocks()
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
