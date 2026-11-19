import { afterEach, expect, it, vi } from 'vitest'

// A second call of the hook is a re-render: refs keep their slot, and an effect
// runs again only when its dependencies changed.
const harness = vi.hoisted(() => ({ effects: [] as Array<() => unknown>, refs: [] as Array<{ current: unknown }>, deps: [] as Array<unknown[] | undefined>, slot: 0, fx: 0, scroll: () => {} }))
vi.mock('preact/hooks', () => ({
  useRef: (current: unknown) => (harness.refs[harness.slot++] ??= { current }),
  useEffect: (effect: () => unknown, deps?: unknown[]) => {
    const i = harness.fx++, old = harness.deps[i]
    if (i in harness.deps && deps && old && deps.every((d, n) => Object.is(d, old[n]))) return
    harness.deps[i] = deps
    harness.effects.push(effect)
  },
}))
vi.mock('../../signals/tabs', () => ({ activeTabId: { value: 'tab' } }))
vi.mock('../../signals/ui', () => ({ chatAtBottom: { value: true } }))
vi.mock('../../signals/scroll-memory', () => ({ tabScrollMemory: new Map() }))
import { useChatScrollPin } from './useChatScrollPin'
import { chatAtBottom } from '../../signals/ui'

afterEach(() => { harness.effects = []; harness.refs = []; harness.deps = []; vi.unstubAllGlobals() })

it('follows the end again when another segment of the same tab is opened (INC-2026-0104)', () => {
  const frames: Array<() => void> = []
  const flush = () => { for (let n = 0; n < 20 && frames.length; n++) frames.shift()!() }
  vi.stubGlobal('requestAnimationFrame', (callback: () => void) => { frames.push(callback); return frames.length })
  vi.stubGlobal('window', {})
  vi.stubGlobal('MutationObserver', class { observe() {} disconnect() {} })
  let top = 0
  const host = {
    get scrollTop() { return top },
    set scrollTop(value: number) { top = Math.min(this.scrollHeight - this.clientHeight, Math.max(0, value)) },
    scrollHeight: 2000, clientHeight: 500,
    addEventListener: (_name: string, callback: () => void) => { harness.scroll = callback },
    removeEventListener() {},
  }
  const render = (entries: number[], view: string) => {
    harness.slot = 0; harness.fx = 0; harness.effects = []
    const hook = useChatScrollPin('tab', entries, undefined, view)
    hook.containerRef.current = { closest: () => host } as unknown as HTMLDivElement
    for (const effect of harness.effects) effect()
    flush()
  }
  render([1], 'past|')
  expect(host.scrollTop).toBe(1500)
  // The reader scrolls to the start of the past segment: the pin is released.
  host.scrollTop = 0; harness.scroll()
  expect(chatAtBottom.value).toBe(false)
  // Same segment, new rows: the reader stays where they are.
  render([1, 2], 'past|')
  expect(host.scrollTop).toBe(0)
  // Back to Current: the view lands at the end and follows new messages.
  render([3], '|')
  expect(chatAtBottom.value).toBe(true)
  expect(host.scrollTop).toBe(1500)
  host.scrollHeight = 3000
  render([3, 4], '|')
  expect(host.scrollTop).toBe(2500)
})