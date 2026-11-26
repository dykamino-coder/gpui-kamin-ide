// @vitest-environment jsdom
import { render } from 'preact'
import { act } from 'preact/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import type { TabInfo } from '../../../../shared/types'
import { activeTabId, tabs } from '../../../signals/tabs'
import { ActiveHooks } from './ActiveHooks'

const cache = vi.hoisted(() => ({ read: vi.fn(() => null), write: vi.fn() }))
vi.mock('../../../lib/panel-cache', () => ({ readPanelCache: cache.read, writePanelCache: cache.write }))
const container = document.createElement('div')

afterEach(() => {
  act(() => render(null, container))
  tabs.value = []
  activeTabId.value = null
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})

it('loads hooks when the selected chat connects and discards a superseded session response', async () => {
  let complete!: (value: { hooks: never[] }) => void
  const active = vi.fn((session: string) => session === 'first'
    ? new Promise<{ hooks: never[] }>(resolve => { complete = resolve })
    : Promise.resolve({ hooks: [] }))
  window.kaminBridge = { hooksList: async () => ({ hooks: {} }), hooksActive: active } as unknown as typeof window.kaminBridge
  const setTab = async (sessionId?: string) => {
    await act(async () => {
      tabs.value = [{ id: 'tab', status: sessionId ? 'connected' : 'connecting', sessionId } as TabInfo]
      activeTabId.value = 'tab'
    })
  }
  await setTab()
  await act(async () => { render(<ActiveHooks />, container) })
  await vi.waitFor(() => expect(container.textContent).toContain('No connected chat'))
  expect(active).not.toHaveBeenCalled()
  await setTab('first')
  await vi.waitFor(() => expect(active).toHaveBeenCalledWith('first'))
  await setTab('second')
  await vi.waitFor(() => expect(active).toHaveBeenCalledWith('second'))
  expect(active.mock.calls).toEqual([['first'], ['second']])
  await act(async () => { complete({ hooks: [] }) })
  expect(cache.write.mock.calls).toHaveLength(1)
  expect(cache.write).toHaveBeenCalledWith('hooks.active.v2', expect.objectContaining({ sessionId: 'second' }))
  expect(container.textContent).toContain('No active hooks.')
})

