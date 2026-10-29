import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'preact'
import type { CommandSnapshot } from '../../../shared/commands'

const fixture = vi.hoisted(() => ({
  states: [] as any[],
  cursor: 0,
  effects: [] as Array<() => unknown>,
  snapshot: undefined as undefined | ((tab: string, snapshot: CommandSnapshot) => void),
  action: vi.fn(),
  off: vi.fn(),
}))
vi.mock('preact/hooks', () => ({
  useRef: () => ({ current: null }),
  useState: (initial: unknown) => {
    const index = fixture.cursor++
    if (index >= fixture.states.length) fixture.states[index] = initial
    return [
      fixture.states[index],
      (value: unknown) => {
        fixture.states[index] = value
      },
    ]
  },
  useEffect: (effect: () => unknown) => {
    fixture.effects.push(effect)
  },
}))
vi.mock('preact/compat', () => ({ createPortal: (node: unknown) => node }))
vi.mock('../../hooks/useBridge', () => ({
  useBridge: () => ({
    commandAction: fixture.action,
    onCommandsSnapshot: (cb: typeof fixture.snapshot) => {
      fixture.snapshot = cb
      return fixture.off
    },
    onCommandOutput: () => fixture.off,
  }),
}))
import { CommandsButton } from './CommandsButton'

function render() {
  fixture.cursor = 0
  fixture.effects = []
  return CommandsButton({ tabId: 'current' })
}
function nodes(value: unknown): VNode<any>[] {
  if (Array.isArray(value)) return value.flatMap(nodes)
  if (!value || typeof value !== 'object') return []
  const node = value as VNode<any>
  return [node, ...nodes(node.props?.children)]
}
beforeEach(() => {
  fixture.states = []
  fixture.action.mockClear()
  fixture.off.mockClear()
  vi.stubGlobal('document', { body: {} })
})
afterEach(() => vi.unstubAllGlobals())
it('counts only background commands and isolates snapshots by tab', () => {
  render()
  const cleanup = fixture.effects[0]() as () => void
  expect(fixture.action).toHaveBeenCalledWith('current', 'list')
  const foreground = {
    id: 'fg',
    tool: 'Bash',
    command: 'foreground',
    background: false,
    status: 'running' as const,
    startedAt: 1,
  }
  fixture.snapshot!('other', { sessionId: 'other', commands: [{ ...foreground, background: true }] })
  expect(nodes(render()).some((n) => n.props.class === 'commands-badge')).toBe(false)
  fixture.snapshot!('current', { sessionId: 'session', commands: [foreground] })
  expect(nodes(render()).some((n) => n.props.class === 'commands-badge')).toBe(false)
  fixture.snapshot!('current', {
    sessionId: 'session',
    commands: [
      { ...foreground, id: 'task', background: true },
      { ...foreground, id: 'shell:1', background: true },
      foreground,
    ],
  })
  const tree = nodes(render())
  expect(tree.find((n) => n.props.class === 'commands-badge')?.props.children).toBe(2)
  tree.find((n) => n.props['aria-label'] === 'Running commands')!.props.onClick()
  const opened = nodes(render())
  expect(opened.filter((n) => n.props.class === 'commands-row')).toHaveLength(3)
  opened.find((n) => n.type === 'button' && n.props.children === 'Stop')!.props.onClick()
  expect(fixture.action).toHaveBeenLastCalledWith('current', 'stop', 'task')
  opened.find((n) => n.type === 'button' && n.props.children === 'Open output')!.props.onClick()
  expect(fixture.action).toHaveBeenLastCalledWith('current', 'output', 'task')
  cleanup()
  expect(fixture.off).toHaveBeenCalledTimes(2)
})
