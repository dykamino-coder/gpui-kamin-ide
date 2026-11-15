import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { KaminBridgeApi } from '../../shared/types'
import { vscodeApi } from './webview-api'

const mocked = vi.hoisted(() => ({
  lastSendAt: { value: new Map<string, number>() },
  tabPromptReady: { value: new Map<string, boolean>() },
  tabActivity: { value: new Map<string, { isWorking: boolean }>() },
  enqueueLocal: vi.fn(),
}))
vi.mock('../signals/connection', () => ({
  lastSendAt: mocked.lastSendAt,
  tabPromptReady: mocked.tabPromptReady,
}))
vi.mock('../signals/ui', () => ({ tabActivity: mocked.tabActivity }))
vi.mock('../signals/queue', () => ({ enqueueLocal: mocked.enqueueLocal }))

const { sendMessageToTab } = await import('./send-message')

function fakeBridge(): KaminBridgeApi {
  return {
    sendInput: vi.fn(),
    submitText: vi.fn(),
  } as unknown as KaminBridgeApi
}

beforeEach(() => {
  mocked.lastSendAt.value = new Map()
  mocked.tabPromptReady.value = new Map([['tab-1', true]])
  mocked.tabActivity.value = new Map()
  mocked.enqueueLocal.mockReset()
  delete vscodeApi.onHostMessage
})

describe('sendMessageToTab', () => {
  it('gives identical native reconnect attempts distinct identities without a separate visual queue', () => {
    mocked.tabPromptReady.value.set('tab-1', false)
    const bridge = fakeBridge()
    sendMessageToTab(bridge, 'tab-1', 'same text')
    sendMessageToTab(bridge, 'tab-1', 'same text')
    const calls = vi.mocked(bridge.submitText).mock.calls
    expect(calls[0][2]).toBeTypeOf('string')
    expect(calls[1][2]).not.toBe(calls[0][2])
    expect(mocked.enqueueLocal).not.toHaveBeenCalled()
  })
  it('sends one semantic submit frame and leaves Ctrl+U to the server', () => {
    const bridge = fakeBridge()
    sendMessageToTab(bridge, 'tab-1', 'hello')

    expect(bridge.sendInput).not.toHaveBeenCalled()
    expect(bridge.submitText).toHaveBeenCalledTimes(1)
    expect(bridge.submitText).toHaveBeenCalledWith('tab-1', 'hello', expect.any(String))
    expect(mocked.tabPromptReady.value.get('tab-1')).toBe(false)
  })

  it('gives browser submissions a stable identity before upstream mod capabilities arrive', () => {
    vscodeApi.onHostMessage = () => () => {}
    const bridge = fakeBridge()
    sendMessageToTab(bridge, 'tab-1', 'survives browser close')
    expect(bridge.submitText).toHaveBeenCalledWith('tab-1', 'survives browser close', expect.any(String))
    expect(mocked.enqueueLocal).not.toHaveBeenCalled()
    delete vscodeApi.onHostMessage
  })
})
