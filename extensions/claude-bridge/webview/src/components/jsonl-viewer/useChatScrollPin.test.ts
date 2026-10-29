import { afterEach, expect, it, vi } from 'vitest'

const harness = vi.hoisted(() => ({ effects: [] as Array<() => unknown>, observe: () => {}, scroll: () => {} }))
vi.mock('preact/hooks', () => ({
  useRef: (current: unknown) => ({ current }),
  useEffect: (effect: () => unknown) => harness.effects.push(effect),
}))
vi.mock('../../signals/tabs', () => ({ activeTabId: { value: 'tab' } }))
vi.mock('../../signals/ui', () => ({ chatAtBottom: { value: true } }))
vi.mock('../../signals/scroll-memory', () => ({ tabScrollMemory: new Map() }))
import { useChatScrollPin } from './useChatScrollPin'

afterEach(() => { harness.effects = []; vi.unstubAllGlobals() })

it('preserves the browser anchor when prepended content has estimated offscreen heights', () => {
  vi.stubGlobal('requestAnimationFrame', () => 0)
  vi.stubGlobal('window', {})
  vi.stubGlobal('MutationObserver', class {
    constructor(callback: () => void) { harness.observe = callback }
    observe() {}
    disconnect() {}
  })
  const host = {
    scrollTop: 900, scrollHeight: 2000, clientHeight: 500,
    addEventListener: (_name: string, callback: () => void) => { harness.scroll = callback },
    removeEventListener() {},
  }
  const loadOlder = vi.fn()
  const hook = useChatScrollPin('tab', [1], loadOlder)
  hook.containerRef.current = { closest: () => host } as unknown as HTMLDivElement
  // Attach the scroll observer; tab restoration is deliberately still pending.
  // Complete restoration first through its two animation frames.
  const frames: Array<() => void> = []
  vi.stubGlobal('requestAnimationFrame', (callback: () => void) => { frames.push(callback); return frames.length })
  for (const effect of harness.effects) effect()
  for (let n = 0; n < 10 && frames.length; n++) frames.shift()!()
  host.scrollTop = 900; harness.scroll()
  host.scrollTop = 250; harness.scroll()
  expect(loadOlder).toHaveBeenCalledOnce()
  // Chromium already preserved the visible row. Total scrollHeight also
  // includes estimates that change as content-visibility lays out new rows.
  host.scrollHeight = 4000
  host.scrollTop = 1900
  harness.observe()
  expect(host.scrollTop).toBe(1900)
})