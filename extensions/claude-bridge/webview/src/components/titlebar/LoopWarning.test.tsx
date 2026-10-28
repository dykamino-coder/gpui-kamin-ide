import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'preact'
import type { KaminBridgeApi } from '../../../shared/types'
import { LiveLoopWarning, LoopWarning } from './LoopWarning'

const fixture = vi.hoisted(() => ({
  states: [] as unknown[],
  cursor: 0,
  effects: [] as Array<() => void | (() => void)>,
  observations: new Map<string, unknown>(),
  live: new Set<string>(),
  waiting: new Map<string, boolean>(),
  widgets: [] as unknown[],
  segments: new Map<string, string>(),
  archived: false,
  tabs: [] as Array<{ id: string; status: string }>,
  activity: new Map<string, { isWorking: boolean }>(),
}))
vi.mock('preact/hooks', () => ({
  useState: (initial: unknown) => {
    const index = fixture.cursor++
    if (index >= fixture.states.length) fixture.states[index] = typeof initial === 'function' ? initial() : initial
    return [
      fixture.states[index],
      (value: unknown) => {
        fixture.states[index] = typeof value === 'function' ? value(fixture.states[index]) : value
      },
    ]
  },
  useEffect: (effect: () => void | (() => void)) => {
    fixture.effects.push(effect)
  },
}))
vi.mock('../../signals/loop-observation', () => ({
  loopObservations: {
    get value() {
      return fixture.observations
    },
  },
}))
vi.mock('../../signals/agents', () => ({
  tabJsonlLive: {
    get value() {
      return fixture.live
    },
  },
}))
vi.mock('../../signals/jsonl', () => ({ archivedViewTs: () => (fixture.archived ? 'old' : undefined) }))
vi.mock('../../signals/compact-segments', () => ({
  activeSegmentTsByTab: {
    get value() {
      return fixture.segments
    },
  },
}))
vi.mock('../../signals/ui', () => ({
  tabWaiting: {
    get value() {
      return fixture.waiting
    },
  },
  tabActivity: {
    get value() {
      return fixture.activity
    },
  },
}))
vi.mock('../../signals/tabs', () => ({
  tabs: {
    get value() {
      return fixture.tabs
    },
  },
}))
vi.mock('../../signals/widgets', () => ({
  activeWidgets: {
    get value() {
      return fixture.widgets
    },
  },
}))

const interrupt = vi.fn()
const sendMessage = vi.fn()
const bridge = { interruptSession: interrupt, sendMessage } as unknown as KaminBridgeApi
const props = { bridge, tabId: 'tab', connected: true, working: true }
function render(overrides: Partial<typeof props> = {}) {
  fixture.cursor = 0
  return LoopWarning({ ...props, ...overrides })
}
function nodes(value: unknown): VNode<Record<string, any>>[] {
  if (!value || typeof value !== 'object') return []
  if (Array.isArray(value)) return value.flatMap(nodes)
  const node = value as VNode<Record<string, any>>
  return [node, ...nodes(node.props?.children)]
}
function click(label: string) {
  const button = nodes(render()).find((n) => n.type === 'button' && n.props.children === label)
  expect(button).toBeDefined()
  button!.props.onClick()
}
beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1_700_000_100_000)
  interrupt.mockClear()
  sendMessage.mockClear()
  fixture.states = []
  fixture.cursor = 0
  fixture.effects = []
  fixture.live = new Set(['tab'])
  fixture.waiting = new Map()
  fixture.widgets = []
  fixture.segments = new Map()
  fixture.archived = false
  fixture.tabs = [{ id: 'tab', status: 'connected' }]
  fixture.activity = new Map([['tab', { isWorking: true }]])
  fixture.observations = new Map([
    [
      'tab',
      {
        current: { key: 'turn', startedAt: 1_700_000_000_000, toolCalls: 4, repeatedCalls: 4, repeatedErrors: 0 },
        baseline: null,
      },
    ],
  ])
})
afterEach(() => {
  vi.useRealTimers()
})

it('connects the shipped chat notice to the selected tab state without subscribing the root', () => {
  const child = LiveLoopWarning({ bridge, tabId: 'tab' })
  expect(child.type).toBe(LoopWarning)
  expect(child.props).toMatchObject({ tabId: 'tab', connected: true, working: true })
  fixture.tabs[0].status = 'connecting'
  fixture.activity.clear()
  expect(LiveLoopWarning({ bridge, tabId: 'tab' }).props).toMatchObject({ connected: false, working: false })
})

it('renders a polite inline notice and timer without sending input or interrupting', () => {
  const notice = render()
  expect(notice?.props.role).toBe('status')
  expect(notice?.props['aria-live']).toBe('polite')
  const cleanup = fixture.effects[0]()
  vi.advanceTimersByTime(60_000)
  expect(interrupt).not.toHaveBeenCalled()
  expect(sendMessage).not.toHaveBeenCalled()
  if (typeof cleanup === 'function') cleanup()
  expect(vi.getTimerCount()).toBe(0)
})
it('interrupts exactly the selected tab only after a user clicks Interrupt', () => {
  render()
  expect(interrupt).not.toHaveBeenCalled()
  click('Interrupt')
  expect(interrupt).toHaveBeenCalledExactlyOnceWith('tab')
  expect(sendMessage).not.toHaveBeenCalled()
  expect(render()).toBeNull()
})
it('dismisses for this turn without touching the agent and permits a later turn notice', () => {
  click('Dismiss')
  expect(render()).toBeNull()
  expect(interrupt).not.toHaveBeenCalled()
  const current = fixture.observations.get('tab') as { current: { key: string } }
  current.current.key = 'next-turn'
  expect(render()).not.toBeNull()
})
it.each(['replay', 'archived', 'segment', 'waiting', 'widget', 'disconnected', 'idle'])(
  'suppresses alerts during %s',
  (state) => {
    if (state === 'replay') fixture.live.clear()
    if (state === 'archived') fixture.archived = true
    if (state === 'segment') fixture.segments.set('tab', 'old')
    if (state === 'waiting') fixture.waiting.set('tab', true)
    if (state === 'widget') fixture.widgets.push({ resolved: false, data: { tabId: 'tab' } })
    expect(render({ connected: state !== 'disconnected', working: state !== 'idle' })).toBeNull()
    expect(interrupt).not.toHaveBeenCalled()
  },
)
it('keeps other tabs permission prompts from hiding the current tab signal', () => {
  fixture.widgets.push({ resolved: false, data: { tabId: 'other' } })
  expect(render()).not.toBeNull()
})
