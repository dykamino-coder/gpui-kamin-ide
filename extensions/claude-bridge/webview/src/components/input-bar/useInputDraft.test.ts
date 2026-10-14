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

// signals/ui pulls the bridge storage alias; the hook only needs the signal.
vi.mock('../../signals/ui', async () => {
  const { signal } = await import('@preact/signals')
  return { pendingAttachments: signal<unknown[]>([]) }
})

const { useInputDraft } = await import('./useInputDraft')

beforeEach(() => {
  // Unmount the previous test's hook (its attachment subscription stays live otherwise).
  mocked.cleanup?.()
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

describe('useInputDraft — attachments per session', () => {
  it('keeps each session its own attached files across a switch', async () => {
    const { pendingAttachments } = await import('../../signals/ui')
    mocked.setItem.mockImplementation((k: string, v: string) => { mocked.values.set(k, v) })
    mocked.removeItem.mockImplementation((k: string) => { mocked.values.delete(k) })
    const textarea = {
      value: '',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(() => true),
    } as unknown as HTMLTextAreaElement
    const file = { path: 'C:/work/spec.md', name: 'spec.md', ext: 'md' }

    useInputDraft({ tabId: 'tab-1', textareaRef: { current: textarea }, setHasText: vi.fn() })
    pendingAttachments.value = [file]
    mocked.cleanup?.()
    expect(JSON.parse(mocked.values.get('input-attachments:tab-1') ?? '[]')).toEqual([file])

    // Another chat starts without tab-1's file…
    useInputDraft({ tabId: 'tab-2', textareaRef: { current: textarea }, setHasText: vi.fn() })
    expect(pendingAttachments.value).toEqual([])
    mocked.cleanup?.()

    // …and switching back restores it.
    useInputDraft({ tabId: 'tab-1', textareaRef: { current: textarea }, setHasText: vi.fn() })
    expect(pendingAttachments.value).toEqual([file])
    mocked.cleanup?.()
  })
})
