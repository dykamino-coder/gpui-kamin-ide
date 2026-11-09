import { afterEach, expect, it, vi } from 'vitest'

const state = vi.hoisted(() => ({
  cleanup: undefined as undefined | (() => void),
  host: undefined as undefined | ((frame: unknown) => void),
  tree: undefined as undefined | ((event: { root: string; changedDir: string }) => void),
  children: new Map([['/workspace', []], ['/workspace/nested', []]]),
  read: vi.fn(async () => []), watch: vi.fn(), unwatch: vi.fn(),
}))
vi.mock('preact/hooks', () => ({
  useState: (initial: unknown) => {
    let value = typeof initial === 'function' ? initial() : initial
    if (value instanceof Map) value = state.children
    return [value, (next: unknown) => { value = typeof next === 'function' ? next(value) : next }]
  },
  useRef: (value: unknown) => ({ current: value }),
  useCallback: (fn: unknown) => fn,
  useEffect: (fn: () => (() => void)) => { state.cleanup = fn() },
}))
vi.mock('../../../hooks/useBridge', () => ({ useBridge: () => ({
  fileTreeListDir: state.read, fileTreeWatch: state.watch, fileTreeUnwatch: state.unwatch,
  onFileTreeChange: (fn: typeof state.tree) => { state.tree = fn; return () => { state.tree = undefined } },
}) }))
vi.mock('../../../lib/webview-api', () => ({ vscodeApi: {
  onHostMessage: (fn: typeof state.host) => { state.host = fn; return () => { state.host = undefined } },
} }))
import { useFileTree } from './useFileTree'

afterEach(() => { state.cleanup?.(); state.cleanup = undefined; vi.clearAllMocks() })

it('refreshes every mounted directory on host re-attach snapshot without replaying watch mutations', async () => {
  useFileTree('/workspace')
  await Promise.resolve()
  expect(state.watch).toHaveBeenCalledTimes(1)
  state.read.mockClear()
  state.host?.({ kind: 'event', channel: 'workspace:files-state', args: [{ id: 'host-view', trees: ['/workspace'] }] })
  await Promise.resolve()
  expect(state.read.mock.calls).toEqual([['/workspace'], ['/workspace/nested']])
  expect(state.watch).toHaveBeenCalledTimes(1)
  expect(state.unwatch).not.toHaveBeenCalled()
  state.read.mockClear()
  state.tree?.({ root: '/workspace', changedDir: '/workspace/nested' })
  await Promise.resolve()
  expect(state.read.mock.calls).toEqual([['/workspace/nested'], ['/workspace']])
  state.cleanup?.(); state.cleanup = undefined
  expect(state.host).toBeUndefined()
  expect(state.tree).toBeUndefined()
  expect(state.unwatch).toHaveBeenCalledWith('/workspace')
})
