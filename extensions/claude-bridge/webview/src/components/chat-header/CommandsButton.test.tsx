import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'preact'
import type { CommandSnapshot, RunningCommand } from '../../../shared/commands'

const f = vi.hoisted(() => ({
  hooks: [] as any[],
  cursor: 0,
  pending: [] as Array<() => void>,
  snapshot: undefined as undefined | ((tab: string, snapshot: CommandSnapshot) => void),
  output: undefined as undefined | ((tab: string, result: { id: string; output: string }) => void),
  action: vi.fn(),
  off: vi.fn(),
  listeners: new Map<string, (e: any) => void>(),
}))
vi.mock('preact/hooks', () => ({
  useRef: (initial: unknown) => {
    const i = f.cursor++
    return (f.hooks[i] ??= { current: initial })
  },
  useState: (initial: unknown) => {
    const i = f.cursor++
    if (!f.hooks[i]) f.hooks[i] = { value: initial }
    return [
      f.hooks[i].value,
      (value: unknown) => {
        f.hooks[i].value = value
      },
    ]
  },
  useEffect: (effect: () => unknown, deps: unknown[]) => {
    const i = f.cursor++
    const previous = f.hooks[i]
    if (!previous || deps.some((d, j) => d !== previous.deps[j])) {
      const hook = { deps, cleanup: undefined as any }
      f.hooks[i] = hook
      f.pending.push(() => {
        previous?.cleanup?.()
        hook.cleanup = effect()
      })
    }
  },
}))
vi.mock('preact/compat', () => ({ createPortal: (node: unknown) => node }))
vi.mock('../../hooks/useBridge', () => {
  const bridge = {
    commandAction: f.action,
    onCommandsSnapshot: (cb: typeof f.snapshot) => {
      f.snapshot = cb
      return f.off
    },
    onCommandOutput: (cb: typeof f.output) => {
      f.output = cb
      return f.off
    },
  }
  return { useBridge: () => bridge }
})
import { CommandsButton } from './CommandsButton'
import { CommandCard, CommandOutput, commandDuration } from './CommandCard'

function render(component = () => CommandsButton({ tabId: 'current' })) {
  f.cursor = 0
  const result = component()
  for (const effect of f.pending.splice(0)) effect()
  return nodes(result)
}
function nodes(value: unknown): VNode<any>[] {
  if (Array.isArray(value)) return value.flatMap(nodes)
  if (!value || typeof value !== 'object') return []
  const node = value as VNode<any>
  return [node, ...nodes(node.props?.children)]
}
function label(tree: VNode<any>[], name: string) {
  return tree.find((n) => n.props['aria-label'] === name)!
}
function section(tree: VNode<any>[], name: string) {
  return nodes(label(tree, name)).find((n) => n.type === 'button')!
}
const task: RunningCommand = {
  id: 'task',
  tool: 'Bash',
  command: 'echo full command',
  description: 'Watch agents',
  background: true,
  status: 'running',
  startedAt: 1000,
}
const finished: RunningCommand = {
  ...task,
  id: 'finished',
  status: 'completed',
  finishedAt: 126000,
  exitCode: 0,
}
beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1000)
  f.hooks = []
  f.pending = []
  f.listeners.clear()
  vi.clearAllMocks()
  vi.stubGlobal('document', {
    body: {},
    addEventListener: (key: string, fn: any) => f.listeners.set(key, fn),
    removeEventListener: (key: string) => f.listeners.delete(key),
  })
  vi.stubGlobal('window', {
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  )
})
afterEach(() => {
  for (const hook of f.hooks) hook?.cleanup?.()
  vi.unstubAllGlobals()
  vi.useRealTimers()
})
it('isolates tabs, counts active tasks from both sources, and initially hides completed tasks', () => {
  render()
  f.snapshot!('other', { sessionId: 'other', commands: [task] })
  expect(render().some((n) => n.props.class === 'commands-badge')).toBe(false)
  f.snapshot!('current', {
    sessionId: 's',
    commands: [task, { ...task, id: 'shell:1' }, finished],
  })
  let tree = render()
  expect(tree.find((n) => n.props.class === 'commands-badge')?.props.children).toBe(2)
  label(tree, 'Background tasks').props.onClick()
  tree = render()
  expect(section(tree, 'Running tasks').props['aria-expanded']).toBe(true)
  expect(section(tree, 'Completed tasks').props['aria-expanded']).toBe(false)
  expect(tree.filter((n) => n.type === CommandCard).map((n) => n.props.command.id)).toEqual(['task', 'shell:1'])
  section(tree, 'Completed tasks').props.onClick()
  tree = render()
  expect(tree.filter((n) => n.type === CommandCard)).toHaveLength(3)
  section(tree, 'Running tasks').props.onClick()
  expect(render().filter((n) => n.type === CommandCard)).toHaveLength(1)
  expect(f.action.mock.calls.filter((c) => c[1] === 'output')).toHaveLength(0)
})
it('clears completed tasks, expands the popover itself, and closes on Escape', () => {
  render()
  f.snapshot!('current', { sessionId: 's', commands: [task, finished] })
  label(render(), 'Background tasks').props.onClick()
  let tree = render()
  label(tree, 'Clear completed tasks').props.onClick()
  expect(f.action).toHaveBeenLastCalledWith('current', 'clear')
  label(tree, 'Expand popover').props.onClick()
  tree = render()
  expect(tree.some((n) => n.props.class === 'commands-popover commands-expanded' && n.props.role === 'dialog')).toBe(
    true,
  )
  expect(label(tree, 'Restore popover size').props['aria-pressed']).toBe(true)
  f.listeners.get('keydown')!({
    key: 'Escape',
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  })
  expect(render().some((n) => n.props.role === 'dialog')).toBe(false)
})
it('polls old-server local tasks quietly and ticks durations with fake time', () => {
  render()
  f.snapshot!('current', {
    sessionId: 'old',
    commands: [task],
    serverAvailable: false,
  })
  label(render(), 'Background tasks').props.onClick()
  let tree = render()
  expect(tree.find((n) => n.props.class === 'commands-note')?.props.children).toContain('Local tasks')
  vi.advanceTimersByTime(125000)
  tree = render()
  expect(f.action).toHaveBeenCalledTimes(126)
  expect(commandDuration(task, tree.find((n) => n.type === CommandCard)!.props.now)).toBe('2m 5s')
  expect(commandDuration(finished, Date.now() + 3600000)).toBe('2m 5s')
  expect(commandDuration(task, 11254000)).toBe('3h 7m 33s')
  expect(commandDuration(task, 0)).toBe('0s')
})
it('expands a description card to the full command, supports stop, and reports final status', () => {
  const card = () => CommandCard({ command: task, tabId: 'current', now: 6000 })
  let tree = render(card)
  expect(tree.find((n) => n.props.class === 'commands-title')?.props.children).toBe('Watch agents')
  expect(tree.some((n) => n.type === CommandOutput)).toBe(false)
  label(tree, 'Stop task: Watch agents').props.onClick()
  expect(f.action).toHaveBeenLastCalledWith('current', 'stop', 'task')
  label(tree, 'Expand task: Watch agents').props.onClick()
  tree = render(card)
  expect(tree.find((n) => n.props.class === 'commands-command')?.props.children).toEqual(['> ', task.command])
  expect(tree.filter((n) => n.type === CommandOutput)).toHaveLength(1)
  tree = render(() =>
    CommandCard({
      command: { ...finished, status: 'failed', exitCode: 7, description: '' },
      tabId: 'current',
      now: 900000,
    }),
  )
  expect(tree.find((n) => n.props.class === 'commands-status')?.props.children).toBe('Failed (exit 7)')
  expect(tree.some((n) => String(n.props['aria-label']).startsWith('Stop task'))).toBe(false)
  tree = render(() =>
    CommandCard({
      command: { ...finished, status: 'stopped' },
      tabId: 'current',
      now: 900000,
    }),
  )
  expect(tree.find((n) => n.props.class === 'commands-status')?.props.children).toBe('Stopped')
})
it('reads output only for its expanded card and tab, follows the tail, and cancels polling on unmount', () => {
  const output = () => CommandOutput({ tabId: 'current', id: 'task' })
  let tree = render(output)
  expect(f.action).toHaveBeenLastCalledWith('current', 'output', 'task')
  expect(tree[0].props.children).toBe('No output yet')
  f.output!('other', { id: 'task', output: 'private' })
  f.output!('current', { id: 'other', output: 'wrong task' })
  expect(render(output)[0].props.children).toBe('No output yet')
  const tail = { scrollTop: 0, scrollHeight: 900 }
  ;(tree[0].ref as { current: unknown }).current = tail
  f.output!('current', { id: 'task', output: 'tail\n' })
  tree = render(output)
  expect(tree[0].props.children).toBe('tail\n')
  expect(tail.scrollTop).toBe(900)
  vi.advanceTimersByTime(2000)
  expect(f.action).toHaveBeenCalledTimes(3)
  for (const hook of f.hooks) {
    hook?.cleanup?.()
    if (hook) hook.cleanup = undefined
  }
  vi.advanceTimersByTime(2000)
  expect(f.action).toHaveBeenCalledTimes(3)
  expect(f.off).toHaveBeenCalledOnce()
})
