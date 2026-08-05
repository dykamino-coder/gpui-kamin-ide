import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ElectronBridge } from '../../shared/types'

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

function fakeBridge(): ElectronBridge {
  return {
    sendInput: vi.fn(),
    submitText: vi.fn(),
  } as unknown as ElectronBridge
}

beforeEach(() => {
  mocked.lastSendAt.value = new Map()
  mocked.tabPromptReady.value = new Map([['tab-1', true]])
  mocked.tabActivity.value = new Map()
  mocked.enqueueLocal.mockReset()
})

describe('sendMessageToTab', () => {
  it('sends one semantic submit frame and leaves Ctrl+U to the server', () => {
    const bridge = fakeBridge()
    sendMessageToTab(bridge, 'tab-1', 'hello')

    expect(bridge.sendInput).not.toHaveBeenCalled()
    expect(bridge.submitText).toHaveBeenCalledTimes(1)
    expect(bridge.submitText).toHaveBeenCalledWith('tab-1', 'hello')
    expect(mocked.tabPromptReady.value.get('tab-1')).toBe(false)
  })
})
