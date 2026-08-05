import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocked = vi.hoisted(() => ({
  cleanup: undefined as void | (() => void),
  values: new Map<string, string>(),
  setItem: vi.fn(),
  removeItem: vi.fn(),
}))

vi.mock('preact/hooks', () => ({
  useRef: <T>(value: T) => ({ current: value }),
  useLayoutEffect: (effect: () => void | (() => void)) => {
    mocked.cleanup = effect()
  },
  useCallback: <T extends (...args: never[]) => unknown>(callback: T) => callback,
}))
vi.mock('../../lib/storage', () => ({
  storage: {
    getItem: (key: string) => mocked.values.get(key) ?? null,
    setItem: mocked.setItem,
    removeItem: mocked.removeItem,
  },
}))

const { useInputDraft } = await import('./useInputDraft')

beforeEach(() => {
  mocked.cleanup = undefined
  mocked.values.clear()
  mocked.setItem.mockReset()
  mocked.removeItem.mockReset()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    callback(0)
    return 1
  })
})

describe('useInputDraft', () => {
  it('restores a per-tab draft without any PTY or bridge dependency', () => {
    mocked.values.set('input-draft:tab-1', 'unsent chat draft')
    const listeners = new Map<string, EventListener>()
    const textarea = {
      value: '',
      addEventListener: vi.fn((type: string, listener: EventListener) => listeners.set(type, listener)),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn((event: Event) => {
        listeners.get(event.type)?.(event)
        return true
      }),
    } as unknown as HTMLTextAreaElement
    const setHasText = vi.fn()

    useInputDraft({
      tabId: 'tab-1',
      textareaRef: { current: textarea },
      setHasText,
    })

    expect(textarea.value).toBe('unsent chat draft')
    expect(setHasText).toHaveBeenCalledWith(true)
    expect(mocked.removeItem).not.toHaveBeenCalled()
    // A future bridge send during restore would require changing this boundary.
    expect(mocked.cleanup).toBeTypeOf('function')
  })
})
