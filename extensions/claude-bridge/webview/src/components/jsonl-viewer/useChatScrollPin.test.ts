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
import { chatAtBottom } from '../../signals/ui'

afterEach(() => { harness.effects = []; vi.unstubAllGlobals() })

it('releases the pin on the first upward scroll after a no-op bottom snap', () => {
  const frames: Array<() => void> = []
  const flush = () => { for (let n = 0; n < 20 && frames.length; n++) frames.shift()!() }
  vi.stubGlobal('requestAnimationFrame', (callback: () => void) => { frames.push(callback); return frames.length })
  vi.stubGlobal('window', {})
  vi.stubGlobal('MutationObserver', class {
    constructor(callback: () => void) { harness.observe = callback }
    observe() {}
    disconnect() {}
  })
  let top = 1500
  const host = {
    get scrollTop() { return top },
    set scrollTop(value: number) { top = Math.min(1500, Math.max(0, value)) },
    scrollHeight: 2000, clientHeight: 500,
    addEventListener: (_name: string, callback: () => void) => { harness.scroll = callback },
    removeEventListener() {},
  }
  const hook = useChatScrollPin('tab', [1])
  hook.containerRef.current = { closest: () => host } as unknown as HTMLDivElement
  for (const effect of harness.effects) effect()
  flush()
  // Already at bottom: the browser emits NO scroll event for these snaps.
  harness.observe(); flush()
  host.scrollTop = 1050
  harness.scroll()
  expect(chatAtBottom.value).toBe(false)
  // A late streamed block or widget mutation must leave the reader in place.
  harness.observe(); flush()
  expect(host.scrollTop).toBe(1050)
  host.scrollTop = 1500; harness.scroll()
  expect(chatAtBottom.value).toBe(true)
})

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
